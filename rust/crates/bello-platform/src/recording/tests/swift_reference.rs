use super::*;
use sha2::{Digest, Sha256};

const ORIGINAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../BelloBox/Recording/GIF/GIFTranscoder.swift"
));
const HARNESS: &str = include_str!("swift_reference.swift");
const FULL_HASH: &str = "7628606696213e100ac9f59cf17c26517132620bf0ff7819c5d94ecc59813fe4";
const RENDERER_HASH: &str = "18efae846785029a354c20ddb126dbaaf2cad19c83ce26173d89dbdb00bf5f9f";
const READER_HASH: &str = "9fbc0e3ab43560630bfa22d979d362123718dea769896fed6382528a9284639b";
const FRAME_BYTES: usize = fixture::WIDTH as usize * fixture::HEIGHT as usize * 4;
const OUTPUT_BYTES: usize = 12 + fixture::FRAME_COUNT * (20 + FRAME_BYTES);

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn excerpt<'a>(source: &'a str, begin: &str, end: &str, hash: &str) -> Result<&'a str, String> {
    if source.matches(begin).count() != 1 || source.matches(end).count() != 1 {
        return Err("Swift extraction markers are not unique".into());
    }
    let start = source.find(begin).unwrap();
    let end = source.find(end).unwrap();
    let text = source.get(start..end).ok_or("Swift extraction order")?;
    if digest(text) != hash {
        return Err("Swift excerpt changed; review the reference source".into());
    }
    Ok(text)
}
fn source(original: &str) -> Result<String, String> {
    for marker in ["/* ORIGINAL_RENDERER */", "/* ORIGINAL_READER_OUTPUT */"] {
        if HARNESS.matches(marker).count() != 1 {
            return Err("Swift driver substitution markers changed".into());
        }
    }
    if digest(original) != FULL_HASH {
        return Err("Swift source changed; review and rebind its SHA-256".into());
    }
    let renderer = excerpt(
        original,
        "struct GIFFrameRenderer {",
        "\nprivate final class CancellationFlag",
        RENDERER_HASH,
    )?;
    let reader = excerpt(
        original,
        "        let output = AVAssetReaderTrackOutput(track: track, outputSettings: [",
        "\n        guard reader.canAdd(output)",
        READER_HASH,
    )?;
    Ok(HARNESS
        .replace("/* ORIGINAL_RENDERER */", renderer)
        .replace("/* ORIGINAL_READER_OUTPUT */", reader))
}

fn parse(bytes: &[u8]) -> Result<Vec<fixture::KnownFrame>, String> {
    if bytes.len() != OUTPUT_BYTES || bytes.get(..8) != Some(b"BBSWIFT1") {
        return Err("Swift output magic or exact byte count".into());
    }
    let mut offset = 8;
    fn take<const N: usize>(bytes: &[u8], offset: &mut usize) -> [u8; N] {
        let value = bytes[*offset..*offset + N].try_into().unwrap();
        *offset += N;
        value
    }
    let count = u32::from_le_bytes(take(bytes, &mut offset)) as usize;
    if count != fixture::FRAME_COUNT {
        return Err("Swift frame count".into());
    }
    let mut result = Vec::with_capacity(fixture::FRAME_COUNT);
    for index in 0..fixture::FRAME_COUNT {
        let presentation_seconds = f64::from_bits(u64::from_le_bytes(take(bytes, &mut offset)));
        let width = u32::from_le_bytes(take(bytes, &mut offset));
        let height = u32::from_le_bytes(take(bytes, &mut offset));
        let length = u32::from_le_bytes(take(bytes, &mut offset)) as usize;
        if (width, height, length) != (fixture::WIDTH, fixture::HEIGHT, FRAME_BYTES)
            || !presentation_seconds.is_finite()
            || (presentation_seconds - index as f64 / f64::from(fixture::FPS)).abs() >= 0.001
        {
            return Err(format!(
                "Swift frame {index} dimensions, length or recipe PTS"
            ));
        }
        let rgba = bytes[offset..offset + FRAME_BYTES].to_vec();
        if rgba.chunks_exact(4).any(|pixel| pixel[3] != 255) {
            return Err(format!("Swift frame {index} is not opaque RGBA"));
        }
        offset += FRAME_BYTES;
        result.push(fixture::KnownFrame {
            presentation_seconds,
            width,
            height,
            rgba,
        });
    }
    Ok(result)
}

#[cfg(unix)]
mod process {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::{
        io::Read,
        process::{Child, Command, Stdio},
    };
    pub(super) struct Directory(pub(super) PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    struct Process(Child);
    impl Drop for Process {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                // This process group contains only our compiler/reference child.
                unsafe {
                    libc::kill(-(self.0.id() as i32), libc::SIGKILL);
                }
                let _ = self.0.wait();
            }
        }
    }
    pub(super) fn run(
        command: &mut Command,
        directory: &Path,
        name: &str,
        timeout: Duration,
        stderr_limit: u64,
    ) {
        let log_path = directory.join(name);
        let log = output::new_file(&log_path).unwrap();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .process_group(0);
        let mut child = Process(
            command
                .spawn()
                .expect("launch installed Apple Swift toolchain"),
        );
        let deadline = Instant::now() + timeout;
        let status = loop {
            assert!(
                Instant::now() < deadline,
                "Swift oracle process exceeded its time limit"
            );
            assert!(
                fs::metadata(&log_path).unwrap().len() <= stderr_limit,
                "Swift oracle stderr exceeded the observed size limit"
            );
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut diagnostic = String::new();
        fs::File::open(log_path)
            .unwrap()
            .take(4096)
            .read_to_string(&mut diagnostic)
            .unwrap();
        assert!(
            status.success(),
            "Swift oracle process failed: {diagnostic}"
        );
    }
}

#[cfg(unix)]
pub(super) fn decode(movie: &FinalizedRecording) -> Vec<fixture::KnownFrame> {
    use std::{io::Read, process::Command};
    movie.verify().unwrap();
    let directory = process::Directory(output::private_directory(&std::env::temp_dir()).unwrap());
    let swift = directory.0.join("reference.swift");
    let binary = directory.0.join("reference");
    let frames = directory.0.join("reference.frames");
    fs::write(&swift, source(ORIGINAL).unwrap()).unwrap();
    process::run(
        Command::new("/usr/bin/xcrun")
            .args(["swiftc", "-parse-as-library", "-swift-version", "5"])
            .arg(&swift)
            .arg("-o")
            .arg(&binary),
        &directory.0,
        "compile.log",
        Duration::from_secs(120),
        65536,
    );
    process::run(
        Command::new(&binary).arg(movie.path()).arg(&frames),
        &directory.0,
        "decode.log",
        Duration::from_secs(120),
        65536,
    );
    movie.verify().unwrap();
    assert_eq!(fs::metadata(&frames).unwrap().len(), OUTPUT_BYTES as u64);
    let mut bytes = Vec::new();
    fs::File::open(frames)
        .unwrap()
        .take(OUTPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    parse(&bytes).unwrap()
}

#[test]
fn swift_source_and_extraction_are_bound_to_reviewed_bytes() {
    // Type-check the bounded native driver on Linux without executing Swift.
    #[cfg(unix)]
    let _driver: fn(&FinalizedRecording) -> Vec<fixture::KnownFrame> = decode;
    let generated = source(ORIGINAL).unwrap();
    assert!(!generated.contains("/* ORIGINAL_"));
    assert!(source(&(ORIGINAL.to_owned() + "\n")).is_err());
    assert!(source(&ORIGINAL.replace(
        "context.interpolationQuality = .medium",
        "context.interpolationQuality = .high"
    ))
    .is_err());
    assert!(excerpt(
        ORIGINAL,
        "GIFFrameRenderer {",
        "\nprivate final class CancellationFlag",
        RENDERER_HASH
    )
    .is_err());
    assert!(excerpt(
        ORIGINAL,
        "missing renderer",
        "\nprivate final class CancellationFlag",
        RENDERER_HASH
    )
    .is_err());
}

#[test]
fn swift_protocol_rejects_count_length_metadata_and_alpha_corruption() {
    let known = movie(true, false).known_frames().unwrap();
    let mut bytes = b"BBSWIFT1".to_vec();
    bytes.extend_from_slice(&(fixture::FRAME_COUNT as u32).to_le_bytes());
    for index in 0..fixture::FRAME_COUNT {
        let frame = known.frame(index).unwrap();
        bytes.extend_from_slice(&frame.presentation_seconds.to_bits().to_le_bytes());
        bytes.extend_from_slice(&frame.width.to_le_bytes());
        bytes.extend_from_slice(&frame.height.to_le_bytes());
        bytes.extend_from_slice(&(frame.rgba.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&frame.rgba);
    }
    assert_eq!(parse(&bytes).unwrap().len(), fixture::FRAME_COUNT);
    for offset in [0, 8, 20, 24, 28, 35, OUTPUT_BYTES - 1] {
        let mut corrupt = bytes.clone();
        corrupt[offset] ^= 1;
        assert!(parse(&corrupt).is_err(), "protocol offset {offset}");
    }
    for pts in [0.1_f64, f64::NAN, f64::INFINITY] {
        let mut corrupt = bytes.clone();
        corrupt[12..20].copy_from_slice(&pts.to_bits().to_le_bytes());
        assert!(parse(&corrupt).is_err(), "protocol PTS {pts}");
    }
    assert!(parse(&bytes[..bytes.len() - 1]).is_err());
    bytes.push(0);
    assert!(parse(&bytes).is_err());
}

#[cfg(unix)]
#[test]
fn swift_driver_timeout_retires_the_owned_child_process_group() {
    use std::process::Command;
    let directory = process::Directory(output::private_directory(&std::env::temp_dir()).unwrap());
    let result = std::panic::catch_unwind(|| {
        process::run(
            Command::new("/bin/sh")
                .args([
                    "-c",
                    "(/bin/sleep 1; printf survived > late-write) & printf started > started; wait",
                ])
                .current_dir(&directory.0),
            &directory.0,
            "timeout.log",
            Duration::from_millis(250),
            65536,
        );
    });
    assert!(result.is_err());
    assert!(
        directory.0.join("started").exists(),
        "child reached its held work"
    );
    std::thread::sleep(Duration::from_millis(1200));
    assert!(
        !directory.0.join("late-write").exists(),
        "descendant outlived timeout cleanup"
    );
}

#[cfg(unix)]
#[test]
fn swift_driver_aborts_when_stderr_exceeds_the_observed_limit() {
    use std::process::Command;
    let directory = process::Directory(output::private_directory(&std::env::temp_dir()).unwrap());
    let started = Instant::now();
    let result = std::panic::catch_unwind(|| {
        process::run(
            Command::new("/bin/sh").args(["-c", "printf '%0512d' 0 >&2; /bin/sleep 2"]),
            &directory.0,
            "output.log",
            Duration::from_secs(3),
            128,
        );
    });
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(fs::metadata(directory.0.join("output.log")).unwrap().len() > 128);
}
