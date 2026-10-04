//! Small Unix process runner. Nonblocking pipes avoid a stalled helper freezing
//! stdin writes or output reads. Each subprocess owns its process group, so a
//! failed/timed-out action can also terminate its descendants. A successful
//! clipboard writer is deliberately allowed to retain a clipboard-owner child.

use crate::{ErrorKind, PlatformError, Result};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MAX_STDERR_BYTES: usize = 64 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const READS_PER_POLL: usize = 16;

pub(crate) fn run(
    executable: &Path,
    args: &[OsString],
    input: Option<&[u8]>,
    max_output: usize,
    timeout: Duration,
    operation: &'static str,
) -> Result<Vec<u8>> {
    let started = Instant::now();
    let mut child = Command::new(executable)
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| {
            error(
                ErrorKind::Io,
                operation,
                "Could not start the installed platform helper.",
            )
        })?;
    let outcome = communicate(&mut child, input, max_output, started, timeout, operation);
    if outcome.is_err() {
        terminate(&mut child);
    }
    outcome
}

fn communicate(
    child: &mut Child,
    input: Option<&[u8]>,
    max_output: usize,
    started: Instant,
    timeout: Duration,
    operation: &'static str,
) -> Result<Vec<u8>> {
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    set_nonblocking(stdout.as_raw_fd())
        .and_then(|_| set_nonblocking(stderr.as_raw_fd()))
        .and_then(|_| {
            stdin
                .as_ref()
                .map(|stream| set_nonblocking(stream.as_raw_fd()))
                .unwrap_or(Ok(()))
        })
        .map_err(|_| {
            error(
                ErrorKind::Io,
                operation,
                "Could not configure bounded helper I/O.",
            )
        })?;
    let input = input.unwrap_or_default();
    let mut written = 0;
    let mut output = Vec::new();
    let mut discarded_stderr = 0;
    loop {
        if started.elapsed() >= timeout {
            return Err(error(ErrorKind::TimedOut, operation, "Platform helper timed out and was stopped. Check display access, permissions, and local helper setup."));
        }
        if let Some(stream) = stdin.as_mut() {
            match stream.write(&input[written..]) {
                Ok(n) => written += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => {
                    return Err(error(
                        ErrorKind::BackendFailed,
                        operation,
                        "Platform helper stopped accepting input.",
                    ))
                }
            }
            if written == input.len() {
                stdin.take();
            }
        }
        drain(
            &mut stdout,
            Some(&mut output),
            max_output,
            &mut 0,
            operation,
        )?;
        drain(
            &mut stderr,
            None,
            MAX_STDERR_BYTES,
            &mut discarded_stderr,
            operation,
        )?;
        if let Some(status) = child.try_wait().map_err(|_| {
            error(
                ErrorKind::Io,
                operation,
                "Could not inspect the platform helper status.",
            )
        })? {
            // Do not await pipe EOF: clipboard owners may inherit a pipe after
            // the foreground writer exits. A final bounded drain preserves all
            // data already buffered by the completed foreground helper.
            drain(
                &mut stdout,
                Some(&mut output),
                max_output,
                &mut 0,
                operation,
            )?;
            drain(
                &mut stderr,
                None,
                MAX_STDERR_BYTES,
                &mut discarded_stderr,
                operation,
            )?;
            if !status.success() {
                let mut err = error(ErrorKind::BackendFailed, operation,
                    "Platform helper failed. Check display access, system permission, image format, and installed language data as applicable.");
                err.exit_code = status.code();
                return Err(err);
            }
            if written != input.len() {
                return Err(error(
                    ErrorKind::BackendFailed,
                    operation,
                    "Platform helper exited before accepting the complete input.",
                ));
            }
            return Ok(output);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn drain(
    reader: &mut impl Read,
    mut output: Option<&mut Vec<u8>>,
    limit: usize,
    discarded: &mut usize,
    operation: &'static str,
) -> Result<()> {
    let mut buffer = [0; 8192];
    for _ in 0..READS_PER_POLL {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let count = output.as_ref().map_or(*discarded, |bytes| bytes.len());
                if !fits_output_limit(count, n, limit) {
                    return Err(error(
                        ErrorKind::OutputTooLarge,
                        operation,
                        "Platform helper output exceeded its bounded size limit and was stopped.",
                    ));
                }
                if let Some(output) = output.as_mut() {
                    output.extend_from_slice(&buffer[..n]);
                } else {
                    *discarded += n;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return Err(error(
                    ErrorKind::Io,
                    operation,
                    "Could not read platform helper output.",
                ))
            }
        }
    }
    Ok(())
}

fn fits_output_limit(current: usize, additional: usize, limit: usize) -> bool {
    current
        .checked_add(additional)
        .is_some_and(|total| total <= limit)
}

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    #[cfg(target_os = "linux")]
    const O_NONBLOCK: i32 = 0o4000;
    #[cfg(target_os = "macos")]
    const O_NONBLOCK: i32 = 0x0004;
    extern "C" {
        fn fcntl(fd: i32, command: i32, ...) -> i32;
    }
    // SAFETY: The borrowed descriptor is a live pipe owned by this function's
    // caller. F_GETFL takes no third argument; F_SETFL takes an integer flags
    // argument. Neither call transfers descriptor ownership.
    unsafe {
        let flags = fcntl(fd, F_GETFL);
        if flags == -1 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn terminate(child: &mut Child) {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    // SAFETY: process_group(0) gave the child its own group whose ID is its PID.
    // Negating that positive PID targets only this command's process group.
    if let Ok(pid) = i32::try_from(child.id()) {
        if pid > 0 {
            unsafe {
                kill(-pid, 9);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn error(kind: ErrorKind, operation: &'static str, message: &'static str) -> PlatformError {
    PlatformError::new(kind, operation, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_bound_never_wraps() {
        assert!(fits_output_limit(0, 10, 10));
        assert!(!fits_output_limit(9, 2, 10));
        assert!(!fits_output_limit(usize::MAX, 1, usize::MAX));
    }

    #[test]
    fn drain_preserves_literal_text_and_rejects_oversized_output() {
        let mut output = vec![];
        drain(&mut &b"text\n"[..], Some(&mut output), 5, &mut 0, "test").unwrap();
        assert_eq!(output, b"text\n");
        let err = drain(&mut &b"x"[..], Some(&mut output), 5, &mut 0, "test").unwrap_err();
        assert_eq!(err.kind, ErrorKind::OutputTooLarge);
    }

    #[test]
    fn diagnostics_are_discarded_but_still_bounded() {
        let mut discarded = 0;
        drain(
            &mut &b"private content"[..],
            None,
            15,
            &mut discarded,
            "test",
        )
        .unwrap();
        assert_eq!(discarded, 15);
        let err = drain(&mut &b"x"[..], None, 15, &mut discarded, "test").unwrap_err();
        assert_eq!(err.kind, ErrorKind::OutputTooLarge);
        assert!(!err.to_string().contains("private content"));
    }

    #[test]
    #[ignore = "opt-in subprocess smoke test; never touches the desktop"]
    fn subprocess_large_stdin_round_trip() {
        let input = vec![b'x'; 500 * 1024];
        let output = run(
            Path::new("/bin/cat"),
            &[],
            Some(&input),
            input.len(),
            Duration::from_secs(3),
            "test",
        )
        .unwrap();
        assert_eq!(output, input);
    }

    #[test]
    #[ignore = "opt-in subprocess timeout test; never touches the desktop"]
    fn subprocess_timeout_is_bounded() {
        let started = Instant::now();
        let err = run(
            Path::new("/bin/sleep"),
            &["10".into()],
            None,
            1024,
            Duration::from_millis(50),
            "test",
        )
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    #[ignore = "opt-in subprocess output-bound test; never touches the desktop"]
    fn subprocess_output_limit_stops_helper() {
        let err = run(
            Path::new("/usr/bin/yes"),
            &[],
            None,
            1024,
            Duration::from_secs(3),
            "test",
        )
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::OutputTooLarge);
    }
}
