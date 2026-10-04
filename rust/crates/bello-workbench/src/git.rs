//! Read-only Git data for the workbench.
//!
//! These methods are **blocking**. Invoke them on a bounded background worker
//! pool, cancel superseded requests, and publish only the newest generation.
//! There is no async runtime dependency and no UI-thread work is hidden here.
//! Commands use argv, never a shell; Git is system-installed `/usr/bin/git`.
//! Linux and macOS are supported. Output is bounded and partial results are
//! never returned after timeout, cancellation, overflow, or a parsing error.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MAX_HISTORY_PAGE: usize = 200;
const MAX_HISTORY_SKIP: usize = 10_000_000;
const HARD_OUTPUT_LIMIT: usize = 64 * 1024 * 1024;
const HARD_DIAGNOSTIC_LIMIT: usize = 256 * 1024;
const MAX_PATHS: usize = 1_000;
const MAX_PATH_BYTES: usize = 128 * 1024;
const MAX_STATUS_ENTRIES: usize = 100_000;

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub struct GitReadOptions {
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub max_diagnostic_bytes: usize,
    /// Clone this token into the UI owner and call `cancel()` to stop a read.
    /// A cancelled token is one-shot; create a fresh token for a new request.
    pub cancellation: CancellationToken,
}

impl Default for GitReadOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            max_output_bytes: 16 * 1024 * 1024,
            max_diagnostic_bytes: 64 * 1024,
            cancellation: CancellationToken::new(),
        }
    }
}

#[derive(Debug)]
pub enum GitError {
    Io(io::Error),
    InvalidInput(String),
    Failed {
        code: Option<i32>,
        diagnostic: String,
    },
    Cancelled,
    TimedOut,
    OutputLimit {
        stream: &'static str,
        limit: usize,
    },
    InvalidOutput(&'static str),
    UnsupportedPlatform,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Git I/O failed: {error}"),
            Self::InvalidInput(message) => f.write_str(message),
            Self::Failed { code, diagnostic } if diagnostic.is_empty() => {
                write!(f, "Git exited with status {code:?}")
            }
            Self::Failed { diagnostic, .. } => f.write_str(diagnostic),
            Self::Cancelled => f.write_str("Git read cancelled; no partial result was applied"),
            Self::TimedOut => f.write_str("Git read timed out; no partial result was applied"),
            Self::OutputLimit { stream, limit } => write!(
                f,
                "Git {stream} exceeded {limit} bytes; select a smaller scope"
            ),
            Self::InvalidOutput(message) => write!(f, "Invalid Git output: {message}"),
            Self::UnsupportedPlatform => f.write_str("Git reads are supported on Linux and macOS"),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
impl From<io::Error> for GitError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub original_path: Option<PathBuf>,
    /// Porcelain v2 XY codes: `.` means unchanged.
    pub index_status: char,
    pub worktree_status: char,
    pub untracked: bool,
    pub conflicted: bool,
}

impl StatusEntry {
    pub fn staged(&self) -> bool {
        !self.untracked && self.index_status != '.'
    }
    pub fn unstaged(&self) -> bool {
        self.untracked || self.worktree_status != '.'
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepositoryStatus {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub entries: Vec<StatusEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub author_time: i64,
    pub subject: String,
    pub parents: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryPage {
    pub commits: Vec<Commit>,
    /// Present only when one look-ahead commit establishes that another page
    /// exists. Offset pages reflect live HEAD; refresh from zero when it moves.
    pub next_skip: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffRequest {
    /// Tracked working-tree changes relative to the index. Untracked files are
    /// reported by `status`, not synthesized into this patch.
    Worktree { paths: Vec<PathBuf> },
    /// Index changes relative to HEAD (the empty tree for an unborn branch).
    Staged { paths: Vec<PathBuf> },
    /// One commit versus its first parent, or the empty tree for a root commit.
    /// Use both old and new paths when retaining a rename in a filtered patch.
    Commit { hash: String, paths: Vec<PathBuf> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitDiff {
    /// Exact patch bytes, including non-UTF-8 file contents. Binary changes
    /// receive Git's binary-change notice; binary payloads are never requested.
    pub bytes: Vec<u8>,
}

impl GitDiff {
    pub fn lossy_text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }
}

#[derive(Clone, Debug)]
pub struct GitRepository {
    root: PathBuf,
}

impl GitRepository {
    /// Discovers the worktree top level from a directory and canonicalizes it.
    /// This executes Git and must also run on a background worker.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, GitError> {
        Self::open_with_options(root, &GitReadOptions::default())
    }

    pub fn open_with_options(
        root: impl AsRef<Path>,
        options: &GitReadOptions,
    ) -> Result<Self, GitError> {
        let directory = fs::canonicalize(root)?;
        if !directory.is_dir() {
            return Err(GitError::InvalidInput(
                "repository location is not a directory".into(),
            ));
        }
        let output = git(
            &directory,
            strings(&["rev-parse", "--show-toplevel"]),
            options,
        )?
        .require_success()?;
        // Remove only Git's one terminator: a real directory may end in LF.
        let bytes = output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout);
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(GitError::InvalidOutput("missing repository root"));
        }
        let root = fs::canonicalize(path_from_bytes(bytes)?)?;
        if !root.is_dir() || !directory.starts_with(&root) {
            return Err(GitError::InvalidOutput(
                "repository root does not contain the requested directory",
            ));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn status(&self, options: &GitReadOptions) -> Result<RepositoryStatus, GitError> {
        let output = git(
            &self.root,
            strings(&[
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=all",
                "--ignore-submodules=none",
                "-z",
            ]),
            options,
        )?
        .require_success()?;
        parse_status(&output.stdout)
    }

    /// Reads at most 200 commits plus one look-ahead, with lossless NUL-framed
    /// fields. `limit == 0` and excessive offsets fail before spawning Git.
    pub fn history(
        &self,
        skip: usize,
        limit: usize,
        options: &GitReadOptions,
    ) -> Result<HistoryPage, GitError> {
        if limit == 0 || limit > MAX_HISTORY_PAGE || skip > MAX_HISTORY_SKIP {
            return Err(GitError::InvalidInput(format!(
                "history requires a page size of 1..={MAX_HISTORY_PAGE} and offset <= {MAX_HISTORY_SKIP}"
            )));
        }
        let mut arguments = strings(&[
            "log",
            "--format=%H%x00%h%x00%an%x00%at%x00%s%x00%P",
            "-z",
            "--date-order",
            "--no-show-signature",
        ]);
        arguments.push(format!("--max-count={}", limit + 1).into());
        arguments.push(format!("--skip={skip}").into());
        arguments.extend(strings(&["HEAD", "--"]));
        let output = git(&self.root, arguments, options)?;
        if !output.status.success() {
            // Check an unborn branch explicitly instead of interpreting a
            // locale-dependent diagnostic or hiding general log failures.
            let status = self.status(options)?;
            if status.head.is_none() {
                return Ok(HistoryPage {
                    commits: Vec::new(),
                    next_skip: None,
                });
            }
            output.require_success()?;
            unreachable!("failed output cannot become successful");
        }
        let mut commits = parse_history(&output.stdout)?;
        if commits.len() > limit + 1 {
            return Err(GitError::InvalidOutput(
                "history exceeded the requested page size",
            ));
        }
        let next_skip = (commits.len() > limit).then_some(skip + limit);
        commits.truncate(limit);
        Ok(HistoryPage { commits, next_skip })
    }

    pub fn diff(
        &self,
        request: &DiffRequest,
        options: &GitReadOptions,
    ) -> Result<GitDiff, GitError> {
        let (mut arguments, paths) = match request {
            DiffRequest::Worktree { paths } => (strings(&["diff"]), paths),
            DiffRequest::Staged { paths } => (strings(&["diff", "--cached"]), paths),
            DiffRequest::Commit { hash, paths } => {
                validate_hash(hash)?;
                let mut args = strings(&["show", "--format=", "--root", "-m", "--first-parent"]);
                // Peeling prevents a tag/tree/blob object from being interpreted
                // as a commit while retaining an option-safe validated hash.
                args.push(format!("{hash}^{{commit}}").into());
                (args, paths)
            }
        };
        arguments.extend(strings(&[
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-relative",
            "--find-renames",
            "-l1000",
            "--unified=3",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--ignore-submodules=none",
            "--",
        ]));
        arguments.extend(validated_paths(paths)?);
        let output = git(&self.root, arguments, options)?.require_success()?;
        Ok(GitDiff {
            bytes: output.stdout,
        })
    }
}

fn strings(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn validate_hash(hash: &str) -> Result<(), GitError> {
    if !matches!(hash.len(), 40 | 64) || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(GitError::InvalidInput(
            "a full 40- or 64-character hexadecimal commit hash is required".into(),
        ));
    }
    Ok(())
}

fn validated_paths(paths: &[PathBuf]) -> Result<Vec<OsString>, GitError> {
    if paths.len() > MAX_PATHS {
        return Err(GitError::InvalidInput(
            "too many paths in one Git read".into(),
        ));
    }
    let mut bytes = 0usize;
    let mut result = Vec::with_capacity(paths.len());
    for path in paths {
        bytes = bytes.saturating_add(path.as_os_str().len());
        if bytes > MAX_PATH_BYTES
            || path.as_os_str().is_empty()
            || os_contains_nul(path.as_os_str())
        {
            return Err(GitError::InvalidInput(
                "Git paths must be nonempty and fit within 128 KiB".into(),
            ));
        }
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => normalized.push(part),
                Component::CurDir => {}
                _ => {
                    return Err(GitError::InvalidInput(
                        "Git paths must be root-relative without '..'".into(),
                    ));
                }
            }
        }
        if normalized.as_os_str().is_empty() {
            return Err(GitError::InvalidInput(
                "use an empty path list for the complete repository".into(),
            ));
        }
        result.push(normalized.into_os_string());
    }
    Ok(result)
}

#[cfg(unix)]
fn os_contains_nul(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().contains(&0)
}
#[cfg(not(unix))]
fn os_contains_nul(value: &OsStr) -> bool {
    value.to_string_lossy().contains('\0')
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, GitError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, GitError> {
    Ok(PathBuf::from(std::str::from_utf8(bytes).map_err(|_| {
        GitError::InvalidOutput("non-UTF-8 filename")
    })?))
}

fn parse_status(bytes: &[u8]) -> Result<RepositoryStatus, GitError> {
    let mut result = RepositoryStatus::default();
    if bytes.is_empty() {
        return Ok(result);
    }
    let mut records = nul_payload(bytes)?.split(|b| *b == 0);
    while let Some(record) = records.next() {
        if record.starts_with(b"# ") {
            let (name, value) = split_once(&record[2..], b' ')?;
            match name {
                b"branch.head" => result.branch = Some(lossy(value)),
                b"branch.oid" => {
                    if value != b"(initial)" {
                        let hash = ascii(value)?;
                        validate_hash(hash)
                            .map_err(|_| GitError::InvalidOutput("invalid HEAD hash"))?;
                        result.head = Some(hash.to_owned());
                    }
                }
                b"branch.upstream" => result.upstream = Some(lossy(value)),
                b"branch.ab" => {
                    let (ahead, behind) = split_once(value, b' ')?;
                    result.ahead = parse_count(ahead, b'+')?;
                    result.behind = parse_count(behind, b'-')?;
                }
                _ => {}
            }
            continue;
        }
        let (path, original_path, xy, untracked, conflicted) = match record.first() {
            Some(b'1') => {
                let fields = record.splitn(9, |b| *b == b' ').collect::<Vec<_>>();
                if fields.len() != 9 {
                    return Err(GitError::InvalidOutput("incomplete ordinary status"));
                }
                (fields[8], None, fields[1], false, false)
            }
            Some(b'2') => {
                let fields = record.splitn(10, |b| *b == b' ').collect::<Vec<_>>();
                if fields.len() != 10 {
                    return Err(GitError::InvalidOutput("incomplete rename status"));
                }
                let original = records
                    .next()
                    .ok_or(GitError::InvalidOutput("missing rename source"))?;
                (
                    fields[9],
                    Some(path_from_bytes(original)?),
                    fields[1],
                    false,
                    false,
                )
            }
            Some(b'u') => {
                let fields = record.splitn(11, |b| *b == b' ').collect::<Vec<_>>();
                if fields.len() != 11 {
                    return Err(GitError::InvalidOutput("incomplete conflict status"));
                }
                (fields[10], None, fields[1], false, true)
            }
            Some(b'?') if record.starts_with(b"? ") => {
                (&record[2..], None, &b".."[..], true, false)
            }
            Some(b'!') if record.starts_with(b"! ") => continue,
            _ => return Err(GitError::InvalidOutput("unrecognized status record")),
        };
        if xy.len() != 2 || !xy.iter().all(|b| b".MADRCUT".contains(b)) || path.is_empty() {
            return Err(GitError::InvalidOutput("invalid status codes or path"));
        }
        if result.entries.len() >= MAX_STATUS_ENTRIES {
            return Err(GitError::InvalidOutput(
                "status exceeds 100,000 entries; no partial result was applied",
            ));
        }
        result.entries.push(StatusEntry {
            path: path_from_bytes(path)?,
            original_path,
            index_status: char::from(xy[0]),
            worktree_status: char::from(xy[1]),
            untracked,
            conflicted,
        });
    }
    result.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn parse_history(bytes: &[u8]) -> Result<Vec<Commit>, GitError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let mut fields = Vec::new();
    for field in nul_payload(bytes)?.split(|b| *b == 0) {
        if fields.len() >= (MAX_HISTORY_PAGE + 1) * 6 {
            return Err(GitError::InvalidOutput(
                "history exceeded the maximum page size",
            ));
        }
        fields.push(field);
    }
    if fields.len() % 6 != 0 {
        return Err(GitError::InvalidOutput("incomplete history record"));
    }
    let mut commits = Vec::with_capacity(fields.len() / 6);
    for fields in fields.as_chunks::<6>().0 {
        let hash = ascii(fields[0])?;
        validate_hash(hash).map_err(|_| GitError::InvalidOutput("invalid commit hash"))?;
        let short_hash = ascii(fields[1])?;
        if short_hash.is_empty() || !hash.starts_with(short_hash) {
            return Err(GitError::InvalidOutput("invalid abbreviated hash"));
        }
        let author_time = ascii(fields[3])?
            .parse()
            .map_err(|_| GitError::InvalidOutput("invalid author timestamp"))?;
        let mut parents = Vec::new();
        for parent in fields[5].split(|b| *b == b' ').filter(|v| !v.is_empty()) {
            let parent = ascii(parent)?;
            validate_hash(parent).map_err(|_| GitError::InvalidOutput("invalid parent hash"))?;
            parents.push(parent.to_owned());
        }
        commits.push(Commit {
            hash: hash.to_owned(),
            short_hash: short_hash.to_owned(),
            author: lossy(fields[2]),
            author_time,
            subject: lossy(fields[4]),
            parents,
        });
    }
    Ok(commits)
}

fn nul_payload(bytes: &[u8]) -> Result<&[u8], GitError> {
    bytes
        .strip_suffix(&[0])
        .ok_or(GitError::InvalidOutput("missing NUL terminator"))
}
fn split_once(bytes: &[u8], separator: u8) -> Result<(&[u8], &[u8]), GitError> {
    let index = bytes
        .iter()
        .position(|b| *b == separator)
        .ok_or(GitError::InvalidOutput("missing field separator"))?;
    Ok((&bytes[..index], &bytes[index + 1..]))
}
fn ascii(bytes: &[u8]) -> Result<&str, GitError> {
    std::str::from_utf8(bytes).map_err(|_| GitError::InvalidOutput("invalid UTF-8 metadata"))
}
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
fn parse_count(bytes: &[u8], prefix: u8) -> Result<u64, GitError> {
    let value = bytes
        .strip_prefix(&[prefix])
        .ok_or(GitError::InvalidOutput("invalid ahead/behind count"))?;
    ascii(value)?
        .parse()
        .map_err(|_| GitError::InvalidOutput("invalid ahead/behind count"))
}

pub(crate) struct Output {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) status: ExitStatus,
}
impl Output {
    pub(crate) fn require_success(self) -> Result<Self, GitError> {
        if self.status.success() {
            Ok(self)
        } else {
            Err(GitError::Failed {
                code: self.status.code(),
                diagnostic: lossy(&self.stderr).trim().to_owned(),
            })
        }
    }
}

pub(crate) fn git(
    root: &Path,
    arguments: Vec<OsString>,
    options: &GitReadOptions,
) -> Result<Output, GitError> {
    let mut command = Command::new("/usr/bin/git");
    command
        .current_dir(root)
        .args([
            "--no-pager",
            "--literal-pathspecs",
            "-c",
            "color.ui=false",
            "-c",
            "core.quotepath=true",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "log.showSignature=false",
            "-c",
            "diff.submodule=short",
        ])
        .args(arguments)
        // Reject inherited GIT_DIR, worktree/index redirects, config injection,
        // pager/external-diff commands, and prompt helpers from the host process.
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_PAGER", "cat")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env_remove("GIT_EXTERNAL_DIFF");
    run_bounded(command, options)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_bounded(mut command: Command, options: &GitReadOptions) -> Result<Output, GitError> {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    if options.cancellation.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    if options.timeout.is_zero() {
        return Err(GitError::TimedOut);
    }
    if options.max_output_bytes > HARD_OUTPUT_LIMIT
        || options.max_diagnostic_bytes > HARD_DIAGNOSTIC_LIMIT
    {
        return Err(GitError::InvalidInput(
            "Git output limits exceed the supported hard bounds".into(),
        ));
    }
    let timeout = options.timeout.min(Duration::from_secs(600));
    let started = Instant::now();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn()?;
    // Drop guard stops and reaps the process group even on setup/read errors.
    let mut process = ProcessGuard {
        child,
        reaped: false,
        clean_exit: false,
    };
    let mut stdout = process
        .child
        .stdout
        .take()
        .ok_or(GitError::InvalidOutput("missing stdout pipe"))?;
    let mut stderr = process
        .child
        .stderr
        .take()
        .ok_or(GitError::InvalidOutput("missing stderr pipe"))?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut out_eof = false;
    let mut err_eof = false;
    let mut status = None;
    let mut exited_at = None;
    loop {
        if options.cancellation.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        if started.elapsed() >= timeout {
            return Err(GitError::TimedOut);
        }
        // Only one chunk from each pipe per turn: a noisy stream cannot starve
        // the other stream or prevent timeout/cancellation from being checked.
        let progressed_out = drain_once(
            &mut stdout,
            &mut out,
            &mut out_eof,
            options.max_output_bytes,
            "stdout",
        )?;
        let progressed_err = drain_once(
            &mut stderr,
            &mut err,
            &mut err_eof,
            options.max_diagnostic_bytes,
            "stderr",
        )?;
        if status.is_none()
            && let Some(value) = process.child.try_wait()?
        {
            process.reaped = true;
            status = Some(value);
            exited_at = Some(Instant::now());
        }
        if let Some(status) = status {
            if out_eof && err_eof {
                process.clean_exit = true;
                return Ok(Output {
                    stdout: out,
                    stderr: err,
                    status,
                });
            }
            if exited_at.is_some_and(|time| time.elapsed() >= Duration::from_millis(250)) {
                // A descendant retaining a pipe must not hang the caller.
                return Err(GitError::InvalidOutput(
                    "Git exited without closing its output pipes",
                ));
            }
        }
        if !progressed_out && !progressed_err {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn run_bounded(_command: Command, _options: &GitReadOptions) -> Result<Output, GitError> {
    Err(GitError::UnsupportedPlatform)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn drain_once(
    reader: &mut impl Read,
    bytes: &mut Vec<u8>,
    eof: &mut bool,
    limit: usize,
    stream: &'static str,
) -> Result<bool, GitError> {
    if *eof {
        return Ok(false);
    }
    let mut buffer = [0u8; 16 * 1024];
    match reader.read(&mut buffer) {
        Ok(0) => {
            *eof = true;
            Ok(true)
        }
        Ok(count) => {
            if count > limit.saturating_sub(bytes.len()) {
                return Err(GitError::OutputLimit { stream, limit });
            }
            bytes.extend_from_slice(&buffer[..count]);
            Ok(true)
        }
        Err(error)
            if error.kind() == io::ErrorKind::WouldBlock
                || error.kind() == io::ErrorKind::Interrupted =>
        {
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe extern "C" {
    fn fcntl(fd: std::os::raw::c_int, command: std::os::raw::c_int, ...) -> std::os::raw::c_int;
    fn kill(pid: std::os::raw::c_int, signal: std::os::raw::c_int) -> std::os::raw::c_int;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn nonblocking(fd: std::os::raw::c_int) -> io::Result<()> {
    const F_GETFL: std::os::raw::c_int = 3;
    const F_SETFL: std::os::raw::c_int = 4;
    #[cfg(target_os = "linux")]
    const O_NONBLOCK: std::os::raw::c_int = 0x800;
    #[cfg(target_os = "macos")]
    const O_NONBLOCK: std::os::raw::c_int = 0x4;
    // SAFETY: fd is a live owned pipe descriptor. These fcntl operations take
    // integer flags only and do not retain pointers or alter descriptor owners.
    let flags = unsafe { fcntl(fd, F_GETFL) };
    if flags < 0 || unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct ProcessGuard {
    child: std::process::Child,
    reaped: bool,
    clean_exit: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        // Every command starts a fresh process group. Killing the group also
        // closes inherited pipes held by unexpected descendants. SIGKILL is
        // appropriate here: these are read-only commands, never transactions.
        if !self.clean_exit {
            // SAFETY: the negative PID is this child's process group, created
            // before exec by CommandExt::process_group(0), never the host's.
            unsafe {
                kill(-(self.child.id() as i32), 9);
            }
        }
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicU64;

    struct TempRepo(PathBuf);
    impl TempRepo {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "bello-git-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let repo = Self(path);
            repo.command(&["init", "-q", "--initial-branch=main"]);
            repo
        }
        fn command(&self, arguments: &[&str]) -> Vec<u8> {
            let output = Command::new("/usr/bin/git")
                .current_dir(&self.0)
                .args([
                    "--literal-pathspecs",
                    "-c",
                    "core.fsmonitor=false",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(arguments)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "Workbench Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
                .env("GIT_COMMITTER_NAME", "Workbench Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {:?}: {}",
                arguments,
                lossy(&output.stderr)
            );
            output.stdout
        }
        fn write(&self, path: impl AsRef<Path>, text: &str) {
            fs::write(self.0.join(path), text).unwrap();
        }
        fn commit(&self, message: &str) {
            self.command(&["add", "-A", "--"]);
            self.command(&["commit", "-qm", message]);
        }
        fn open(&self) -> GitRepository {
            GitRepository::open(&self.0).unwrap()
        }
    }
    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unborn_history_and_staged_diff_work_without_head() {
        let temp = TempRepo::new();
        fs::create_dir(temp.0.join("nested")).unwrap();
        let repo = GitRepository::open(temp.0.join("nested")).unwrap();
        assert_eq!(repo.root(), fs::canonicalize(&temp.0).unwrap());
        let options = GitReadOptions::default();
        let status = repo.status(&options).unwrap();
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.head, None);
        assert_eq!(repo.history(0, 10, &options).unwrap().commits, []);
        temp.write("first.txt", "first content\n");
        temp.command(&["add", "--", "first.txt"]);
        let patch = repo
            .diff(&DiffRequest::Staged { paths: vec![] }, &options)
            .unwrap();
        assert!(patch.lossy_text().contains("+first content"));
    }

    #[test]
    fn literal_magic_spaces_newlines_and_non_utf8_names() {
        use std::os::unix::ffi::OsStringExt;
        let temp = TempRepo::new();
        let names = [
            "with spaces.txt",
            "[ab].txt",
            "a.txt",
            ":(glob)*.txt",
            "-flag.txt",
            "line\nbreak\t.txt",
        ];
        let raw_name = OsString::from_vec(vec![b'x', 0xff, b'.', b't']);
        for name in &names {
            temp.write(name, "before\n");
        }
        temp.write(&raw_name, "before\n");
        temp.commit("initial");
        for (i, name) in names.iter().enumerate() {
            temp.write(name, &format!("after {i}\n"));
        }
        temp.write(&raw_name, "after raw\n");
        let repo = temp.open();
        let options = GitReadOptions::default();
        let status = repo.status(&options).unwrap();
        assert_eq!(status.entries.len(), names.len() + 1);
        assert!(status.entries.iter().all(|e| !e.staged() && e.unstaged()));
        for (i, name) in names.iter().enumerate() {
            assert!(status.entries.iter().any(|e| e.path == Path::new(name)));
            let patch = repo
                .diff(
                    &DiffRequest::Worktree {
                        paths: vec![PathBuf::from(name)],
                    },
                    &options,
                )
                .unwrap();
            assert_eq!(
                patch.lossy_text().matches("diff --git ").count(),
                1,
                "{name:?}"
            );
            assert!(
                patch.lossy_text().contains(&format!("+after {i}")),
                "{name:?}"
            );
        }
        assert!(
            status
                .entries
                .iter()
                .any(|e| e.path.as_os_str() == raw_name)
        );
        let raw_patch = repo
            .diff(
                &DiffRequest::Worktree {
                    paths: vec![PathBuf::from(raw_name)],
                },
                &options,
            )
            .unwrap();
        assert!(raw_patch.lossy_text().contains("+after raw"));
    }

    #[test]
    fn separates_staged_unstaged_and_untracked_without_mutating_index() {
        let temp = TempRepo::new();
        temp.write("file", "base\n");
        temp.commit("base");
        temp.write("file", "staged\n");
        temp.command(&["add", "--", "file"]);
        temp.write("file", "worktree\n");
        temp.write("new file", "untracked\n");
        let index_before = fs::read(temp.0.join(".git/index")).unwrap();
        let options = GitReadOptions::default();
        let repo = temp.open();
        let status = repo.status(&options).unwrap();
        let tracked = status
            .entries
            .iter()
            .find(|e| e.path == Path::new("file"))
            .unwrap();
        assert!(tracked.staged() && tracked.unstaged());
        assert_eq!((tracked.index_status, tracked.worktree_status), ('M', 'M'));
        assert!(
            status
                .entries
                .iter()
                .any(|e| e.untracked && e.path == Path::new("new file"))
        );
        let staged = repo
            .diff(&DiffRequest::Staged { paths: vec![] }, &options)
            .unwrap();
        assert!(staged.lossy_text().contains("-base\n+staged"));
        let unstaged = repo
            .diff(&DiffRequest::Worktree { paths: vec![] }, &options)
            .unwrap();
        assert!(unstaged.lossy_text().contains("-staged\n+worktree"));
        assert_eq!(fs::read(temp.0.join(".git/index")).unwrap(), index_before);
    }

    #[test]
    fn history_pagination_and_root_commit_diff() {
        let temp = TempRepo::new();
        for i in 0..5 {
            temp.write("file", &format!("content {i}\n"));
            temp.commit(&format!("commit {i}"));
        }
        let options = GitReadOptions::default();
        let repo = temp.open();
        let first = repo.history(0, 2, &options).unwrap();
        assert_eq!(first.commits[0].subject, "commit 4");
        assert_eq!(first.commits[1].subject, "commit 3");
        assert_eq!(first.next_skip, Some(2));
        let second = repo.history(first.next_skip.unwrap(), 2, &options).unwrap();
        assert_eq!(second.commits[0].subject, "commit 2");
        assert_eq!(second.next_skip, Some(4));
        let third = repo.history(4, 2, &options).unwrap();
        assert_eq!(third.commits.len(), 1);
        assert_eq!(third.next_skip, None);
        assert!(third.commits[0].parents.is_empty());
        assert_eq!(third.commits[0].author, "Workbench Test");
        assert!(third.commits[0].author_time > 0);
        let root_patch = repo
            .diff(
                &DiffRequest::Commit {
                    hash: third.commits[0].hash.clone(),
                    paths: vec![],
                },
                &options,
            )
            .unwrap();
        assert!(root_patch.lossy_text().contains("+content 0"));
        let tip_patch = repo
            .diff(
                &DiffRequest::Commit {
                    hash: first.commits[0].hash.clone(),
                    paths: vec![],
                },
                &options,
            )
            .unwrap();
        assert!(tip_patch.lossy_text().contains("-content 3\n+content 4"));
        assert!(repo.history(100, 2, &options).unwrap().commits.is_empty());
    }

    #[test]
    fn rename_paths_are_lossless_and_first_parent_merges_are_explicit() {
        let temp = TempRepo::new();
        temp.write("original name", "unchanged body\n");
        temp.commit("root");
        temp.command(&["mv", "--", "original name", "renamed\nname"]);
        let options = GitReadOptions::default();
        let status = temp.open().status(&options).unwrap();
        assert_eq!(status.entries.len(), 1);
        assert_eq!(status.entries[0].path, PathBuf::from("renamed\nname"));
        assert_eq!(
            status.entries[0].original_path,
            Some(PathBuf::from("original name"))
        );
        assert_eq!(status.entries[0].index_status, 'R');
        temp.commit("rename");
        temp.command(&["checkout", "-qb", "side"]);
        temp.write("side file", "side content\n");
        temp.commit("side");
        temp.command(&["checkout", "-q", "main"]);
        temp.write("main file", "main content\n");
        temp.commit("main");
        temp.command(&["merge", "--no-ff", "-qm", "merge", "side"]);
        let repo = temp.open();
        let head = repo.history(0, 1, &options).unwrap().commits.remove(0);
        assert_eq!(head.parents.len(), 2);
        let patch = repo
            .diff(
                &DiffRequest::Commit {
                    hash: head.hash,
                    paths: vec![],
                },
                &options,
            )
            .unwrap();
        assert!(patch.lossy_text().contains("+side content"));
        assert!(!patch.lossy_text().contains("+main content"));
    }

    #[test]
    fn unsafe_revision_and_path_inputs_fail_before_execution() {
        let temp = TempRepo::new();
        let repo = temp.open();
        let options = GitReadOptions::default();
        for hash in [
            "HEAD",
            "--output=/tmp/no",
            "abc",
            "0123456789012345678901234567890123456789zz",
        ] {
            assert!(matches!(
                repo.diff(
                    &DiffRequest::Commit {
                        hash: hash.into(),
                        paths: vec![]
                    },
                    &options
                ),
                Err(GitError::InvalidInput(_))
            ));
        }
        for path in [
            "../outside",
            "/absolute",
            "a/../../outside",
            "",
            ".",
            "nul\0name",
        ] {
            assert!(matches!(
                repo.diff(
                    &DiffRequest::Worktree {
                        paths: vec![path.into()]
                    },
                    &options
                ),
                Err(GitError::InvalidInput(_))
            ));
        }
        assert!(repo.history(0, 0, &options).is_err());
        assert!(repo.history(0, MAX_HISTORY_PAGE + 1, &options).is_err());
    }

    #[test]
    fn external_diff_textconv_fsmonitor_and_pager_are_disabled() {
        let temp = TempRepo::new();
        temp.write("file.txt", "before\n");
        temp.write(".gitattributes", "*.txt diff=danger\n");
        temp.commit("base");
        temp.command(&["config", "diff.external", "false"]);
        temp.command(&["config", "diff.danger.textconv", "false"]);
        temp.command(&["config", "core.fsmonitor", "false"]);
        temp.command(&["config", "core.pager", "false"]);
        temp.write("file.txt", "after\n");
        let repo = temp.open();
        let options = GitReadOptions::default();
        assert_eq!(repo.status(&options).unwrap().entries.len(), 1);
        let patch = repo
            .diff(&DiffRequest::Worktree { paths: vec![] }, &options)
            .unwrap();
        assert!(patch.lossy_text().contains("+after"));
    }

    #[test]
    fn rejects_overflow_and_cancelled_reads_without_partial_results() {
        let temp = TempRepo::new();
        temp.write("large", "before\n");
        temp.commit("base");
        temp.write("large", &"changed line\n".repeat(10_000));
        let repo = temp.open();
        let options = GitReadOptions {
            max_output_bytes: 1024,
            ..GitReadOptions::default()
        };
        assert!(matches!(
            repo.diff(&DiffRequest::Worktree { paths: vec![] }, &options),
            Err(GitError::OutputLimit {
                stream: "stdout",
                limit: 1024
            })
        ));
        let options = GitReadOptions::default();
        options.cancellation.cancel();
        assert!(matches!(repo.status(&options), Err(GitError::Cancelled)));
        assert!(matches!(
            GitRepository::open_with_options(&temp.0, &options),
            Err(GitError::Cancelled)
        ));
    }

    fn fixture_command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["process_fixture", "--ignored", "--nocapture"])
            .env("BELLO_GIT_TEST_FIXTURE", mode);
        command
    }

    #[test]
    #[ignore = "subprocess fixture, invoked by runner tests"]
    #[allow(clippy::zombie_processes)] // Intentionally orphaned descendant tests pipe cleanup.
    fn process_fixture() {
        match std::env::var("BELLO_GIT_TEST_FIXTURE").unwrap().as_str() {
            "sleep" => std::thread::sleep(Duration::from_secs(30)),
            "flood" => {
                let bytes = [b'x'; 8192];
                for _ in 0..32 {
                    std::io::stdout().write_all(&bytes).unwrap();
                    std::io::stderr().write_all(&bytes).unwrap();
                }
            }
            "hold-pipe" => {
                fixture_command("sleep")
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap();
            }
            mode => panic!("unknown fixture {mode}"),
        }
    }

    #[test]
    fn drains_both_pipes_and_bounds_diagnostics() {
        let options = GitReadOptions {
            max_diagnostic_bytes: HARD_DIAGNOSTIC_LIMIT,
            ..GitReadOptions::default()
        };
        let output = run_bounded(fixture_command("flood"), &options).unwrap();
        assert!(output.status.success());
        assert!(output.stdout.len() >= 256 * 1024);
        assert_eq!(output.stderr.len(), 256 * 1024);
        let options = GitReadOptions {
            max_diagnostic_bytes: 1000,
            ..GitReadOptions::default()
        };
        assert!(matches!(
            run_bounded(fixture_command("flood"), &options),
            Err(GitError::OutputLimit {
                stream: "stderr",
                limit: 1000
            })
        ));
    }

    #[test]
    fn timeout_and_live_cancellation_kill_processes_promptly() {
        let started = Instant::now();
        let options = GitReadOptions {
            timeout: Duration::from_millis(30),
            ..GitReadOptions::default()
        };
        assert!(matches!(
            run_bounded(fixture_command("sleep"), &options),
            Err(GitError::TimedOut)
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
        let options = GitReadOptions::default();
        let token = options.cancellation.clone();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            token.cancel();
        });
        assert!(matches!(
            run_bounded(fixture_command("sleep"), &options),
            Err(GitError::Cancelled)
        ));
        canceller.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn inherited_open_pipes_do_not_hang_after_parent_exit() {
        let started = Instant::now();
        assert!(matches!(
            run_bounded(fixture_command("hold-pipe"), &GitReadOptions::default()),
            Err(GitError::InvalidOutput(_))
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn malformed_frames_and_metadata_fail_closed() {
        assert!(parse_status(b"? no-terminator").is_err());
        assert!(parse_status(b"2 R. N... 100644 100644 100644 abc abc R100 new\0").is_err());
        assert!(parse_status(b"1 X. N... 100644 100644 100644 abc abc file\0").is_err());
        assert!(parse_history(b"bad\0fields\0").is_err());
        let conflict =
            parse_status(b"u UU N... 100644 100644 100644 100644 a b c conflict path\0").unwrap();
        assert!(conflict.entries[0].conflicted);
        assert_eq!(conflict.entries[0].path, PathBuf::from("conflict path"));
    }
}
