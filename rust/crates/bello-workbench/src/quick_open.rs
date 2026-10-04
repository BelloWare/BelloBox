//! Source-backed Quick Open indexing and fzf-v1 path search.
//!
//! Ported from BelloAgent's `Sources/FileFinder` (listing/ignore/search) and
//! `PiApp/Files/QuickOpen.swift`. Empty queries deliberately return no search
//! results: the host supplies its project-local recent files. All filesystem
//! and Git work is blocking and belongs on a background executor. Cancel and
//! discard superseded generations in the host before publishing their result.
//!
//! Listings bound files, path bytes, inspected entries, depth, ignore data,
//! pattern work, and wall time. Directory links are never traversed; file
//! links are included only when their resolved target is inside the root.
//! Paths remain lossless, root-relative identifiers, distinct from UI text.
//! Recheck confinement when opening a result: this is not an OS sandbox
//! against a process concurrently replacing filesystem ancestors.

use crate::git::{self, CancellationToken, GitError, GitReadOptions};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::ops::Range;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

pub const DEFAULT_SEARCH_LIMIT: usize = 50;
pub const MAX_SEARCH_RESULTS: usize = 1_000;
pub const QUERY_CHARACTER_LIMIT: usize = 128;
const QUERY_BYTE_LIMIT: usize = 8_192;
const PATH_BYTE_LIMIT: usize = 32_768;
const IGNORE_FILE_LIMIT: usize = 1_048_576;
const IGNORE_TOTAL_LIMIT: usize = 16 * IGNORE_FILE_LIMIT;
const WORK_LIMIT: usize = 50_000_000;
const NESTED_REPOSITORY_LIMIT: usize = 4;

#[derive(Clone, Debug)]
pub struct FileListingOptions {
    pub max_files: usize,
    pub max_path_bytes: usize,
    /// Counts inspected directory entries and Git records, including ignored
    /// entries, so a tree of empty or ignored directories is still bounded.
    pub max_entries: usize,
    pub max_depth: usize,
    pub timeout: Duration,
    pub cancellation: CancellationToken,
    /// An explicit global ignore file, otherwise Git's configured excludes
    /// file or the conventional XDG/HOME Git ignore file is discovered.
    pub global_ignore: Option<PathBuf>,
}
impl Default for FileListingOptions {
    fn default() -> Self {
        Self {
            max_files: 500_000,
            max_path_bytes: 64 * 1_048_576,
            max_entries: 2_000_000,
            max_depth: 128,
            timeout: Duration::from_secs(20),
            cancellation: CancellationToken::new(),
            global_ignore: None,
        }
    }
}

#[derive(Debug)]
pub enum FinderError {
    Io(io::Error),
    Git(GitError),
    InvalidInput(&'static str),
    Cancelled,
    TimedOut,
}
impl fmt::Display for FinderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Files could not be listed: {e}"),
            Self::Git(e) => write!(f, "{e}"),
            Self::InvalidInput(s) => f.write_str(s),
            Self::Cancelled => f.write_str("File search cancelled; no partial result was applied"),
            Self::TimedOut => f.write_str("File listing timed out; no partial result was applied"),
        }
    }
}
impl std::error::Error for FinderError {}
impl From<io::Error> for FinderError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<GitError> for FinderError {
    fn from(error: GitError) -> Self {
        match error {
            GitError::Cancelled => Self::Cancelled,
            GitError::TimedOut => Self::TimedOut,
            other => Self::Git(other),
        }
    }
}

/// The query without its optional positive ASCII `:N` line suffix. Spaces
/// do not participate in fuzzy matching. Bounds are explicit to the host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileFinderQuery {
    pub text: String,
    pub line: Option<usize>,
    pub truncated: bool,
    units: Vec<Vec<u8>>,
    mask: u64,
    crosses_folders: bool,
}
impl FileFinderQuery {
    pub fn new(typed: &str) -> Self {
        // Do not copy or segment an unbounded pasted query. A suffix beyond
        // the input bound is not interpreted as an instruction to jump.
        let mut end = typed.len().min(QUERY_BYTE_LIMIT);
        while !typed.is_char_boundary(end) {
            end -= 1;
        }
        let mut text = typed[..end].trim();
        let mut truncated = end < typed.len();
        let mut line = None;
        if !truncated && let Some((head, tail)) = text.rsplit_once(':') {
            let digits = tail.strip_prefix('+').unwrap_or(tail);
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                line = tail.parse::<usize>().ok().filter(|n| *n > 0);
                if line.is_some() {
                    text = head;
                }
            }
        }
        let mut units = Vec::new();
        for grapheme in text
            .graphemes(true)
            .filter(|g| !g.chars().all(char::is_whitespace))
        {
            if units.len() == QUERY_CHARACTER_LIMIT {
                truncated = true;
                break;
            }
            units.push(fold_unit(grapheme));
        }
        let mask = units
            .iter()
            .flatten()
            .fold(0, |mask, b| mask | mask_bit(*b));
        let crosses_folders = units.iter().any(|unit| unit == b"/");
        Self {
            text: text.to_owned(),
            line,
            truncated,
            units,
            mask,
            crosses_folders,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileFinderMatch {
    pub index: usize,
    /// Lossless root-relative path for opening; never reconstruct from text.
    pub path: PathBuf,
    pub display_path: String,
    pub score: i32,
    /// UTF-8 ranges in `display_path`, always at complete character boundaries.
    pub highlights: Vec<Range<usize>>,
}

#[derive(Clone, Debug)]
struct IndexedPath {
    path: PathBuf,
    display: String,
    folded: Vec<u8>,
    name_start: usize,
    mask: u64,
    ascii: bool,
}
impl IndexedPath {
    fn new(path: PathBuf) -> Self {
        let display = path.to_string_lossy().into_owned();
        let ascii = display.is_ascii();
        let folded = if ascii {
            display.to_ascii_lowercase().into_bytes()
        } else {
            display.graphemes(true).flat_map(fold_unit).collect()
        };
        let name_start = folded.iter().rposition(|b| *b == b'/').map_or(0, |p| p + 1);
        let mask = folded.iter().fold(0, |mask, b| mask | mask_bit(*b));
        Self {
            path,
            display,
            folded,
            name_start,
            mask,
            ascii,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileFinderIndex {
    root: PathBuf,
    paths: Vec<IndexedPath>,
    truncated: bool,
    warnings: Vec<String>,
}
impl FileFinderIndex {
    pub fn build(
        root: impl AsRef<Path>,
        options: &FileListingOptions,
    ) -> Result<Self, FinderError> {
        let mut scan = Scan::new(options)?;
        scan.check()?;
        let root = fs::canonicalize(root)?;
        if !fs::metadata(&root)?.is_dir() {
            return Err(FinderError::InvalidInput(
                "The project root is not a directory",
            ));
        }
        let mut index = Self {
            root,
            paths: Vec::new(),
            truncated: false,
            warnings: Vec::new(),
        };
        let global = resolve_global_ignore(&index.root, &scan)?;
        let mut paths = Vec::new();
        let git_available = Path::new("/usr/bin/git").is_file();
        let in_repository = if git_available {
            let out = scan.git(&index.root, &["rev-parse", "--is-inside-work-tree"], None)?;
            out.status.success() && out.stdout == b"true\n"
        } else {
            false
        };
        if in_repository {
            let root = index.root.clone();
            list_repository(
                &root,
                Path::new(""),
                0,
                global.as_deref(),
                &mut scan,
                &mut index,
                &mut paths,
            )?;
        } else {
            let global_rules = global
                .as_ref()
                .and_then(|p| read_ignore(p, b"", true, &mut scan, &mut index).transpose())
                .transpose()?;
            walk(
                &mut scan,
                &mut index,
                &mut paths,
                global_rules.map(Arc::new),
            )?;
        }
        for path in paths {
            scan.check()?;
            index.paths.push(IndexedPath::new(path));
        }
        scan.check()?;
        Ok(index)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn count(&self) -> usize {
        self.paths.len()
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn path(&self, index: usize) -> Option<&Path> {
        self.paths.get(index).map(|p| p.path.as_path())
    }
    pub fn display_path(&self, index: usize) -> Option<&str> {
        self.paths.get(index).map(|p| p.display.as_str())
    }

    /// Best scores first, then shorter folded paths, then listing order.
    /// Cancellations never return partially scored results. The immutable
    /// index can be shared between background searches through `Arc`.
    pub fn search(
        &self,
        query: &FileFinderQuery,
        limit: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<FileFinderMatch>, FinderError> {
        check_cancel(cancellation)?;
        let limit = limit.min(MAX_SEARCH_RESULTS);
        if limit == 0 || query.is_empty() {
            return Ok(Vec::new());
        }
        let mut best: Vec<(usize, i32)> = Vec::with_capacity(limit + 1);
        for (file, path) in self.paths.iter().enumerate() {
            check_cancel(cancellation)?;
            if query.mask & !path.mask != 0 {
                continue;
            }
            let Some((score, _)) = score(query, path, false) else {
                continue;
            };
            let key = (-score, path.folded.len(), file);
            let at = best.partition_point(|(other, points)| {
                (-*points, self.paths[*other].folded.len(), *other) <= key
            });
            if at < limit {
                best.insert(at, (file, score));
                if best.len() > limit {
                    best.pop();
                }
            }
        }
        let mut results = Vec::with_capacity(best.len());
        for (file, points) in best {
            check_cancel(cancellation)?;
            let path = &self.paths[file];
            let positions = score(query, path, true).map(|s| s.1).unwrap_or_default();
            results.push(FileFinderMatch {
                index: file,
                path: path.path.clone(),
                display_path: path.display.clone(),
                score: points,
                highlights: highlights(query, path, &positions),
            });
        }
        check_cancel(cancellation)?;
        Ok(results)
    }
    fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        // Diagnostics themselves must not grow with an unreadable tree.
        if self.warnings.len() < 64 && !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }
    fn limited(&mut self, message: &str) {
        self.truncated = true;
        self.warn(message);
    }
}
fn check_cancel(token: &CancellationToken) -> Result<(), FinderError> {
    if token.is_cancelled() {
        Err(FinderError::Cancelled)
    } else {
        Ok(())
    }
}
fn fold_unit(grapheme: &str) -> Vec<u8> {
    grapheme
        .nfc()
        .flat_map(char::to_lowercase)
        .nfc()
        .collect::<String>()
        .into_bytes()
}
fn mask_bit(byte: u8) -> u64 {
    1u64 << match byte {
        b'a'..=b'z' => byte - b'a',
        b'0'..=b'9' => 26 + byte - b'0',
        b'.' => 36,
        b'_' => 37,
        b'-' => 38,
        b'/' => 39,
        b' ' => 40,
        128..=255 => 63,
        _ => 62,
    }
}
fn matching(unit: &[u8], path: &[u8], at: usize, end: usize) -> bool {
    at <= end && unit.len() <= end - at && path[at..at + unit.len()] == *unit
}
fn window(units: &[Vec<u8>], path: &[u8], range: Range<usize>) -> Option<Range<usize>> {
    let (mut unit, mut at) = (0, range.start);
    let end = loop {
        if at >= range.end {
            return None;
        }
        if matching(&units[unit], path, at, range.end) {
            at += units[unit].len();
            unit += 1;
            if unit == units.len() {
                break at;
            }
        } else {
            at += 1;
        }
    };
    unit -= 1;
    let mut back = end - units[unit].len();
    loop {
        if matching(&units[unit], path, back, range.end) {
            if unit == 0 {
                return Some(back..end);
            }
            unit -= 1;
            back = back.checked_sub(units[unit].len())?;
        } else {
            back = back.checked_sub(1)?;
        }
        if back < range.start {
            return None;
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    White,
    NonWord,
    Delimiter,
    Lower,
    Upper,
    Letter,
    Number,
}
fn class(byte: u8) -> Class {
    match byte {
        b'a'..=b'z' => Class::Lower,
        b'A'..=b'Z' => Class::Upper,
        b'0'..=b'9' => Class::Number,
        b'/' => Class::Delimiter,
        b' ' | b'\t' | b'\n' | b'\r' => Class::White,
        128..=255 => Class::Letter,
        _ => Class::NonWord,
    }
}
fn bonus(previous: Class, current: Class) -> i32 {
    if matches!(
        current,
        Class::Lower | Class::Upper | Class::Letter | Class::Number
    ) {
        match previous {
            Class::White | Class::NonWord => return 8,
            Class::Delimiter => return 9,
            _ => {}
        }
    }
    if previous == Class::Lower && current == Class::Upper
        || previous != Class::Number && current == Class::Number
    {
        return 7;
    }
    if matches!(current, Class::White | Class::NonWord | Class::Delimiter) {
        8
    } else {
        0
    }
}
fn score(query: &FileFinderQuery, path: &IndexedPath, capture: bool) -> Option<(i32, Vec<usize>)> {
    let mut extra = 0;
    let range = if !query.crosses_folders {
        window(
            &query.units,
            &path.folded,
            path.name_start..path.folded.len(),
        )
        .inspect(|range| {
            extra = 64;
            if range.start == path.name_start
                && query.units.iter().map(Vec::len).sum::<usize>() == range.len()
                && (range.end == path.folded.len() || path.folded[range.end] == b'.')
            {
                extra += 48;
            }
        })
    } else {
        None
    }
    .or_else(|| window(&query.units, &path.folded, 0..path.folded.len()))?;
    let class_at = |at| {
        class(if path.ascii {
            path.display.as_bytes()[at]
        } else {
            path.folded[at]
        })
    };
    let (mut unit, mut points, mut gap, mut consecutive, mut first) = (0, extra, false, 0, 0);
    let mut previous = if range.start == 0 {
        Class::Delimiter
    } else {
        class_at(range.start - 1)
    };
    let mut at = range.start;
    let mut positions = Vec::new();
    while at < range.end {
        let current = class_at(at);
        if unit < query.units.len() && matching(&query.units[unit], &path.folded, at, range.end) {
            if capture {
                positions.push(at);
            }
            let mut reward = bonus(previous, current);
            if consecutive == 0 {
                first = reward;
            } else {
                if reward >= 8 && reward > first {
                    first = reward;
                }
                reward = reward.max(first).max(4);
            }
            points += 16 + if unit == 0 { reward * 2 } else { reward };
            gap = false;
            consecutive += 1;
            let length = query.units[unit].len();
            unit += 1;
            previous = class_at(at + length - 1);
            at += length;
        } else {
            points += if gap { -1 } else { -3 };
            gap = true;
            consecutive = 0;
            first = 0;
            previous = current;
            at += match path.folded[at] {
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf7 => 4,
                _ => 1,
            };
        }
    }
    Some((points, positions))
}
fn highlights(
    query: &FileFinderQuery,
    path: &IndexedPath,
    positions: &[usize],
) -> Vec<Range<usize>> {
    let mut origins = Vec::new();
    if !path.ascii {
        for (offset, grapheme) in path.display.grapheme_indices(true) {
            origins.extend(std::iter::repeat_n(
                (offset, offset + grapheme.len()),
                fold_unit(grapheme).len(),
            ));
        }
    }
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (unit, at) in query.units.iter().zip(positions) {
        let range = if path.ascii {
            *at..*at + unit.len()
        } else {
            origins[*at].0..origins[*at + unit.len() - 1].1
        };
        if let Some(last) = ranges.last_mut()
            && last.end >= range.start
        {
            last.end = last.end.max(range.end);
            continue;
        }
        ranges.push(range);
    }
    ranges
}

// MARK: Bounded filesystem and Git listing.
struct Scan<'a> {
    options: &'a FileListingOptions,
    started: Instant,
    entries: usize,
    path_bytes: usize,
    ignore_bytes: usize,
    work: usize,
}
impl<'a> Scan<'a> {
    fn new(options: &'a FileListingOptions) -> Result<Self, FinderError> {
        if options.timeout.is_zero() {
            return Err(FinderError::TimedOut);
        }
        Ok(Self {
            options,
            started: Instant::now(),
            entries: 0,
            path_bytes: 0,
            ignore_bytes: 0,
            work: 0,
        })
    }
    fn check(&self) -> Result<(), FinderError> {
        check_cancel(&self.options.cancellation)?;
        if self.started.elapsed() >= self.options.timeout.min(Duration::from_secs(600)) {
            return Err(FinderError::TimedOut);
        }
        Ok(())
    }
    fn spend(&mut self) -> Result<bool, FinderError> {
        self.work += 1;
        if self.work & 255 == 0 {
            self.check()?;
        }
        Ok(self.work <= WORK_LIMIT)
    }
    fn entry(&mut self, index: &mut FileFinderIndex) -> Result<bool, FinderError> {
        self.check()?;
        if self.entries >= self.options.max_entries.min(2_000_000) || !self.spend()? {
            index.limited(
                "The file listing reached its inspection limit; some files are not indexed.",
            );
            return Ok(false);
        }
        self.entries += 1;
        Ok(true)
    }
    fn add(
        &mut self,
        relative: PathBuf,
        index: &mut FileFinderIndex,
        paths: &mut Vec<PathBuf>,
    ) -> bool {
        let bytes = relative.as_os_str().len();
        if paths.len() >= self.options.max_files.min(500_000)
            || bytes
                > self
                    .options
                    .max_path_bytes
                    .min(64 * 1_048_576)
                    .saturating_sub(self.path_bytes)
        {
            index.limited(
                "The file listing reached its file or path-byte limit; some files are not indexed.",
            );
            return false;
        }
        self.path_bytes += bytes;
        paths.push(relative);
        true
    }
    fn git(
        &self,
        directory: &Path,
        args: &[&str],
        global: Option<&Path>,
    ) -> Result<git::Output, FinderError> {
        let args = args.iter().map(OsString::from).collect();
        self.git_args(directory, args, global)
    }
    fn git_args(
        &self,
        directory: &Path,
        mut args: Vec<OsString>,
        global: Option<&Path>,
    ) -> Result<git::Output, FinderError> {
        self.check()?;
        if let Some(global) = global {
            let mut config = OsString::from("core.excludesFile=");
            config.push(global);
            args.splice(0..0, [OsString::from("-c"), config]);
        }
        let options = GitReadOptions {
            timeout: self
                .options
                .timeout
                .min(Duration::from_secs(600))
                .saturating_sub(self.started.elapsed()),
            max_output_bytes: self
                .options
                .max_path_bytes
                .saturating_add(self.options.max_files)
                .clamp(65_536, 64 * 1_048_576),
            cancellation: self.options.cancellation.clone(),
            ..GitReadOptions::default()
        };
        Ok(git::git(directory, args, &options)?)
    }
}
fn resolve_global_ignore(root: &Path, scan: &Scan<'_>) -> Result<Option<PathBuf>, FinderError> {
    if let Some(path) = &scan.options.global_ignore {
        return Ok(Some(path.clone()));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".config")));
    if Path::new("/usr/bin/git").is_file() {
        // Read only this one data-valued setting, not global command aliases,
        // includes or hooks. Repository configuration has higher precedence.
        let mut candidates = vec![None];
        if let Some(home) = &home {
            candidates.push(Some(home.join(".gitconfig")));
        }
        if let Some(config) = &config {
            candidates.push(Some(config.join("git/config")));
        }
        for file in candidates {
            let mut args: Vec<OsString> = vec!["config".into()];
            if let Some(file) = file {
                args.extend([OsString::from("--file"), file.into_os_string()]);
            }
            args.extend(["--get".into(), "core.excludesFile".into()]);
            let output = scan.git_args(root, args, None)?;
            if output.status.success() {
                let value = output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout);
                if !value.is_empty() && !value.contains(&0) {
                    let path = path_from_bytes(value)?;
                    return Ok(Some(if value.starts_with(b"~/") {
                        home.as_ref()
                            .map(|home| home.join(path_from_bytes(&value[2..]).unwrap_or_default()))
                            .unwrap_or(path)
                    } else if path.is_absolute() {
                        path
                    } else {
                        root.join(path)
                    }));
                }
            }
        }
    }
    Ok(config.map(|config| config.join("git/ignore")))
}
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, FinderError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(bytes.to_vec()).into())
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes).map(PathBuf::from).map_err(|_| {
            FinderError::InvalidInput("Git returned a non-UTF-8 path on this platform")
        })
    }
}
fn valid_relative(relative: &Path) -> bool {
    !relative.as_os_str().is_empty()
        && relative.as_os_str().len() <= PATH_BYTE_LIMIT
        && relative
            .components()
            .all(|c| matches!(c, Component::Normal(name) if name != ".git"))
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum FileKind {
    File,
    Directory,
}
fn checked_kind(root: &Path, relative: &Path) -> Option<FileKind> {
    if !valid_relative(relative) {
        return None;
    }
    let mut absolute = root.to_owned();
    let count = relative.components().count();
    for (i, part) in relative.components().enumerate() {
        absolute.push(part);
        let metadata = fs::symlink_metadata(&absolute).ok()?;
        if metadata.file_type().is_symlink() {
            if i + 1 != count {
                return None;
            }
            let canonical = fs::canonicalize(&absolute).ok()?;
            return (canonical.starts_with(root) && fs::metadata(canonical).ok()?.is_file())
                .then_some(FileKind::File);
        }
        if i + 1 == count {
            return if metadata.is_file() {
                Some(FileKind::File)
            } else if metadata.is_dir() {
                Some(FileKind::Directory)
            } else {
                None
            };
        }
        if !metadata.is_dir() {
            return None;
        }
    }
    None
}
fn list_repository(
    root: &Path,
    prefix: &Path,
    depth: usize,
    global: Option<&Path>,
    scan: &mut Scan<'_>,
    index: &mut FileFinderIndex,
    paths: &mut Vec<PathBuf>,
) -> Result<(), FinderError> {
    let output = match scan.git(
        &root.join(prefix),
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        global,
    ) {
        Ok(output) => output.require_success()?,
        Err(FinderError::Git(GitError::OutputLimit { .. })) => {
            index.limited(
                "Git's file listing exceeded its output limit; select a smaller project folder.",
            );
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if !output.stdout.is_empty() && output.stdout.last() != Some(&0) {
        return Err(FinderError::InvalidInput(
            "Git returned an incomplete file listing",
        ));
    }
    let mut seen = HashSet::new();
    let mut inner = Vec::new();
    for bytes in output.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        if !scan.entry(index)? {
            return Ok(());
        }
        let path = path_from_bytes(bytes)?;
        let relative = prefix.join(path);
        if !valid_relative(&relative) {
            index.warn("Some invalid or overlong file paths could not be indexed.");
            continue;
        }
        if !seen.insert(relative.clone()) {
            continue;
        }
        match checked_kind(root, &relative) {
            Some(FileKind::File) => {
                if !scan.add(relative, index, paths) {
                    return Ok(());
                }
            }
            Some(FileKind::Directory) => inner.push(relative),
            None => {}
        }
    }
    for relative in inner {
        scan.check()?;
        if !root.join(&relative).join(".git").exists() {
            continue;
        }
        if depth >= NESTED_REPOSITORY_LIMIT {
            index.limited("Repositories nested more than 4 levels deep are not indexed.");
            continue;
        }
        let own = scan.git(&root.join(&relative), &["rev-parse", "--show-prefix"], None)?;
        if own.status.success() && own.stdout == b"\n" {
            list_repository(root, &relative, depth + 1, global, scan, index, paths)?;
            if index.truncated {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct IgnoreRules {
    base: Vec<u8>,
    patterns: Vec<IgnorePattern>,
    parent: Option<Arc<IgnoreRules>>,
}
impl IgnoreRules {
    fn ignored(
        &self,
        relative: &[u8],
        directory: bool,
        scan: &mut Scan<'_>,
    ) -> Result<Option<bool>, FinderError> {
        let name = relative.rsplit(|b| *b == b'/').next().unwrap_or(relative);
        let mut rules = Some(self);
        while let Some(file) = rules {
            if let Some(below) = relative.strip_prefix(file.base.as_slice()) {
                for pattern in file.patterns.iter().rev() {
                    if !scan.spend()? {
                        return Ok(None);
                    }
                    if pattern.directory_only && !directory {
                        continue;
                    }
                    let text = if pattern.basename_only { name } else { below };
                    match pattern.matches(text, scan)? {
                        Some(true) => return Ok(Some(!pattern.negated)),
                        Some(false) => {}
                        None => return Ok(None),
                    }
                }
            }
            rules = file.parent.as_deref();
        }
        Ok(Some(false))
    }
}
fn read_ignore(
    path: &Path,
    base: &[u8],
    following: bool,
    scan: &mut Scan<'_>,
    index: &mut FileFinderIndex,
) -> Result<Option<IgnoreRules>, FinderError> {
    scan.check()?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    if metadata.file_type().is_symlink() && !following {
        return Ok(None);
    }
    // Never block opening a FIFO/device masquerading as an ignore file.
    let metadata = if metadata.file_type().is_symlink() {
        match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        }
    } else {
        metadata
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > IGNORE_FILE_LIMIT as u64 {
        index.warn(format!(
            "{} is over 1 MiB, so its ignore rules are not applied.",
            path.display()
        ));
        return Ok(None);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(target_os = "linux")]
        let (nonblock, nofollow) = (0x800, 0x20000);
        #[cfg(target_os = "macos")]
        let (nonblock, nofollow) = (0x4, 0x100);
        options.custom_flags(nonblock | if following { 0 } else { nofollow });
    }
    let file: File = match options.open(path) {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };
    if !file.metadata()?.is_file() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(IGNORE_FILE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > IGNORE_FILE_LIMIT
        || bytes.len() > IGNORE_TOTAL_LIMIT.saturating_sub(scan.ignore_bytes)
    {
        index.warn(
            "Some ignore rules exceeded the bounded ignore-data limit and could not be applied.",
        );
        return Ok(None);
    }
    scan.ignore_bytes += bytes.len();
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let patterns = bytes
        .split(|b| *b == b'\n')
        .filter_map(IgnorePattern::parse)
        .collect();
    Ok(Some(IgnoreRules {
        base: base.to_vec(),
        patterns,
        parent: None,
    }))
}
fn walk(
    scan: &mut Scan<'_>,
    index: &mut FileFinderIndex,
    paths: &mut Vec<PathBuf>,
    global: Option<Arc<IgnoreRules>>,
) -> Result<(), FinderError> {
    let mut pending = vec![(PathBuf::new(), 0usize, global)];
    let mut pending_bytes = 0usize;
    while let Some((relative, depth, inherited)) = pending.pop() {
        scan.check()?;
        pending_bytes = pending_bytes.saturating_sub(relative.as_os_str().len());
        if !relative.as_os_str().is_empty()
            && checked_kind(&index.root, &relative) != Some(FileKind::Directory)
        {
            continue;
        }
        let absolute = index.root.join(&relative);
        let mut base = relative.as_os_str().as_encoded_bytes().to_vec();
        if !base.is_empty() {
            base.push(b'/');
        }
        let rules = match read_ignore(&absolute.join(".gitignore"), &base, false, scan, index)? {
            Some(mut rules) => {
                rules.parent = inherited;
                Some(Arc::new(rules))
            }
            None => inherited,
        };
        let directory = match fs::read_dir(&absolute) {
            Ok(directory) => directory,
            Err(error) if relative.as_os_str().is_empty() => return Err(error.into()),
            Err(_) => {
                index.warn("Some folders could not be read and are not indexed.");
                continue;
            }
        };
        let mut entries = Vec::new();
        let mut names_bytes = 0;
        let mut complete = true;
        for entry in directory {
            if !scan.entry(index)? {
                complete = false;
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    index.warn("Some directory entries could not be read.");
                    continue;
                }
            };
            let name = entry.file_name();
            if name == ".git" {
                continue;
            }
            if names_bytes + name.len() > scan.options.max_path_bytes.min(64 * 1_048_576)
                || entries.len() >= scan.options.max_files.min(500_000).saturating_add(1)
            {
                index.limited("A folder exceeded the listing's entry or path-byte limit; some files are not indexed.");
                complete = false;
                break;
            }
            names_bytes += name.len();
            entries.push(name);
        }
        entries.sort();
        let mut folders = Vec::new();
        for name in entries {
            scan.check()?;
            let child = relative.join(name);
            let Some(kind) = checked_kind(&index.root, &child) else {
                continue;
            };
            if let Some(rules) = &rules {
                match rules.ignored(
                    child.as_os_str().as_encoded_bytes(),
                    kind == FileKind::Directory,
                    scan,
                )? {
                    Some(true) => continue,
                    Some(false) => {}
                    None => {
                        index.limited(
                            "Ignore matching reached its work limit; some files are not indexed.",
                        );
                        return Ok(());
                    }
                }
            }
            if kind == FileKind::File {
                if !scan.add(child, index, paths) {
                    return Ok(());
                }
            } else if depth >= scan.options.max_depth.min(128) {
                index.limited("Some folders exceeded the listing depth limit and are not indexed.");
            } else {
                let bytes = child.as_os_str().len();
                if pending_bytes.saturating_add(bytes)
                    > scan.options.max_path_bytes.min(64 * 1_048_576)
                {
                    index.limited(
                        "Pending folders exceeded the path-byte limit; some files are not indexed.",
                    );
                    return Ok(());
                }
                pending_bytes += bytes;
                folders.push((child, depth + 1, rules.clone()));
            }
        }
        if !complete {
            return Ok(());
        }
        pending.extend(folders.into_iter().rev());
    }
    Ok(())
}

// Gitignore syntax, including anchored paths, directory-only patterns,
// negation, escaped punctuation/spaces, globstar and POSIX character classes.
// An explicit NFA replaces recursive glob backtracking: pathological patterns
// cannot exhaust the call stack, and both state memory and work are bounded.
#[derive(Debug)]
struct IgnorePattern {
    bytes: Vec<u8>,
    negated: bool,
    directory_only: bool,
    basename_only: bool,
}
impl IgnorePattern {
    fn parse(line: &[u8]) -> Option<Self> {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() || line[0] == b'#' {
            return None;
        }
        let mut trailing = None;
        let mut at = 0;
        while at < line.len() {
            match line[at] {
                b' ' => {
                    if trailing.is_none() {
                        trailing = Some(at);
                    }
                }
                b'\\' => {
                    at += 1;
                    trailing = None;
                }
                _ => trailing = None,
            }
            at += 1;
        }
        let mut bytes = &line[..trailing.unwrap_or(line.len())];
        let negated = bytes.first() == Some(&b'!');
        if negated {
            bytes = &bytes[1..];
        }
        let directory_only = bytes.last() == Some(&b'/');
        if directory_only {
            bytes = &bytes[..bytes.len() - 1];
        }
        if bytes.is_empty() {
            return None;
        }
        let basename_only = !bytes.contains(&b'/');
        let bytes = bytes.strip_prefix(b"/").unwrap_or(bytes).to_vec();
        Some(Self {
            bytes,
            negated,
            directory_only,
            basename_only,
        })
    }
    fn matches(&self, text: &[u8], scan: &mut Scan<'_>) -> Result<Option<bool>, FinderError> {
        wildmatch(&self.bytes, text, !self.basename_only, scan)
    }
}
fn wildmatch(
    pattern: &[u8],
    text: &[u8],
    pathname: bool,
    scan: &mut Scan<'_>,
) -> Result<Option<bool>, FinderError> {
    if !pattern
        .iter()
        .any(|b| matches!(*b, b'*' | b'?' | b'[' | b'\\'))
    {
        return Ok(Some(pattern == text));
    }
    let mut pending = VecDeque::from([(0usize, 0usize)]);
    let mut visited = HashSet::new();
    while let Some((pi, ti)) = pending.pop_back() {
        if !scan.spend()? || visited.len() >= 65_536 {
            return Ok(None);
        }
        if !visited.insert((pi, ti)) {
            continue;
        }
        if pi == pattern.len() {
            if ti == text.len() {
                return Ok(Some(true));
            }
            continue;
        }
        match pattern[pi] {
            b'*' => {
                let mut end = pi + 1;
                while end < pattern.len() && pattern[end] == b'*' {
                    end += 1;
                }
                let globstar = end - pi > 1
                    && (pi == 0 || pattern[pi - 1] == b'/')
                    && (end == pattern.len()
                        || pattern[end] == b'/'
                        || pattern.get(end..end + 2) == Some(b"\\/"));
                let slash = !pathname || globstar;
                if globstar && pattern.get(end) == Some(&b'/') {
                    if ti == 0 || text[ti - 1] == b'/' {
                        pending.push_back((end + 1, ti));
                    }
                } else {
                    pending.push_back((end, ti));
                }
                if ti < text.len() && (slash || text[ti] != b'/') {
                    pending.push_back((pi, ti + 1));
                }
            }
            b'?' => {
                if ti < text.len() && (!pathname || text[ti] != b'/') {
                    pending.push_back((pi + 1, ti + 1));
                }
            }
            b'\\' => {
                if ti < text.len() && pattern.get(pi + 1) == Some(&text[ti]) {
                    pending.push_back((pi + 2, ti + 1));
                }
            }
            b'[' => {
                if ti == text.len() || pathname && text[ti] == b'/' {
                    continue;
                }
                if let Some((matched, end)) = bracket(pattern, pi + 1, text[ti], scan)?
                    && matched
                {
                    pending.push_back((end, ti + 1));
                }
            }
            byte => {
                if text.get(ti) == Some(&byte) {
                    pending.push_back((pi + 1, ti + 1));
                }
            }
        }
    }
    Ok((scan.work <= WORK_LIMIT).then_some(false))
}
fn bracket(
    pattern: &[u8],
    mut pi: usize,
    text: u8,
    scan: &mut Scan<'_>,
) -> Result<Option<(bool, usize)>, FinderError> {
    let negated = matches!(pattern.get(pi), Some(b'!' | b'^'));
    if negated {
        pi += 1;
    }
    let mut previous = None;
    let mut matched = false;
    loop {
        if !scan.spend()? {
            return Ok(None);
        }
        let Some(&mut_byte) = pattern.get(pi) else {
            return Ok(None);
        };
        let mut byte = mut_byte;
        match byte {
            b'\\' => {
                pi += 1;
                let Some(&next) = pattern.get(pi) else {
                    return Ok(None);
                };
                byte = next;
                matched |= text == byte;
            }
            b'-' if previous.is_some() && pattern.get(pi + 1).is_some_and(|next| *next != b']') => {
                pi += 1;
                byte = pattern[pi];
                if byte == b'\\' {
                    pi += 1;
                    let Some(&next) = pattern.get(pi) else {
                        return Ok(None);
                    };
                    byte = next;
                }
                matched |= previous.unwrap() <= text && text <= byte;
                previous = None;
                pi += 1;
                if pattern.get(pi) == Some(&b']') {
                    return Ok(Some((matched != negated, pi + 1)));
                }
                continue;
            }
            b'[' if pattern.get(pi + 1) == Some(&b':') => {
                if let Some(end) = pattern[pi + 2..].windows(2).position(|w| w == b":]") {
                    let end = pi + 2 + end;
                    let Some(in_class) = posix_class(&pattern[pi + 2..end], text) else {
                        return Ok(None);
                    };
                    matched |= in_class;
                    pi = end + 1;
                    previous = None;
                    pi += 1;
                    if pattern.get(pi) == Some(&b']') {
                        return Ok(Some((matched != negated, pi + 1)));
                    }
                    continue;
                } else {
                    matched |= text == b'[';
                }
            }
            _ => matched |= text == byte,
        }
        previous = Some(byte);
        pi += 1;
        if pattern.get(pi) == Some(&b']') {
            return Ok(Some((matched != negated, pi + 1)));
        }
    }
}
fn posix_class(name: &[u8], byte: u8) -> Option<bool> {
    let printable = (0x20..0x7f).contains(&byte);
    Some(match name {
        b"alnum" => byte.is_ascii_alphanumeric(),
        b"alpha" => byte.is_ascii_alphabetic(),
        b"blank" => matches!(byte, b' ' | b'\t'),
        b"cntrl" => byte.is_ascii_control(),
        b"digit" => byte.is_ascii_digit(),
        b"graph" => printable && byte != b' ',
        b"lower" => byte.is_ascii_lowercase(),
        b"print" => printable,
        b"punct" => byte.is_ascii_punctuation(),
        b"space" => byte == b' ' || (9..=13).contains(&byte),
        b"upper" => byte.is_ascii_uppercase(),
        b"xdigit" => byte.is_ascii_hexdigit(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "bello-finder-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: impl AsRef<Path>, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        fn git(&self, args: &[&str]) -> Vec<u8> {
            let output = Command::new("/usr/bin/git")
                .current_dir(&self.0)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .args(["-c", "core.fsmonitor=false", "-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        }
        fn options(&self) -> FileListingOptions {
            FileListingOptions {
                global_ignore: Some(self.0.join("missing-global-ignore")),
                ..FileListingOptions::default()
            }
        }
        fn build(&self) -> FileFinderIndex {
            FileFinderIndex::build(&self.0, &self.options()).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(paths: &[&str]) -> FileFinderIndex {
        FileFinderIndex {
            root: PathBuf::from("/fixture"),
            paths: paths
                .iter()
                .map(|p| IndexedPath::new(PathBuf::from(p)))
                .collect(),
            truncated: false,
            warnings: Vec::new(),
        }
    }
    fn found(query: &str, paths: &[&str]) -> Vec<String> {
        fixture(paths)
            .search(&FileFinderQuery::new(query), 50, &CancellationToken::new())
            .unwrap()
            .into_iter()
            .map(|m| m.display_path)
            .collect()
    }
    fn highlighted(query: &str, path: &str) -> Vec<String> {
        let found = fixture(&[path])
            .search(&FileFinderQuery::new(query), 50, &CancellationToken::new())
            .unwrap();
        found
            .first()
            .map(|m| {
                m.highlights
                    .iter()
                    .map(|r| m.display_path[r.clone()].to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn paths(index: &FileFinderIndex) -> Vec<String> {
        index.paths.iter().map(|p| p.display.clone()).collect()
    }

    #[test]
    fn source_query_order_case_whitespace_and_line_suffix() {
        let files = [
            "Sources/App/Main.swift",
            "Sources/Views/MainView.swift",
            "Tests/AppTests.swift",
            "README.md",
        ];
        assert_eq!(found("main", &files), found("MAIN", &files));
        assert_eq!(found("main", &files).len(), 2);
        assert!(found("nmai", &files).is_empty());
        assert!(found("", &files).is_empty());
        assert_eq!(found("main swift:42", &files)[0], "Sources/App/Main.swift");
        let query = FileFinderQuery::new("  Main.swift:42 \n");
        assert_eq!(query.text, "Main.swift");
        assert_eq!(query.line, Some(42));
        for text in ["a:0", "a:", "a:-1", "a:１２", "a:18446744073709551616000"] {
            assert!(FileFinderQuery::new(text).line.is_none(), "{text}");
        }
        assert!(FileFinderQuery::new(":42").is_empty());
        assert!(FileFinderQuery::new(&"x".repeat(129)).truncated);
        assert!(FileFinderQuery::new(&"a".repeat(100_000)).text.len() <= QUERY_BYTE_LIMIT);
    }
    #[test]
    fn source_ranking_preserves_names_boundaries_runs_and_ties() {
        assert_eq!(
            found(
                "main",
                &[
                    "main/Setup.swift",
                    "Sources/Domain/Maintenance.swift",
                    "Sources/App/Main.swift"
                ]
            ),
            [
                "Sources/App/Main.swift",
                "Sources/Domain/Maintenance.swift",
                "main/Setup.swift"
            ]
        );
        assert_eq!(
            found("ft", &["Sources/after.swift", "Sources/FileTab.swift"])[0],
            "Sources/FileTab.swift"
        );
        assert_eq!(
            found(
                "fts",
                &[
                    "Sources/Files/FileTextSource.swift",
                    "Sources/Files/fits.swift"
                ]
            )[0],
            "Sources/Files/FileTextSource.swift"
        );
        assert_eq!(
            found("tab", &["Sources/TheAppBar.swift", "Sources/TabHost.swift"])[0],
            "Sources/TabHost.swift"
        );
        assert_eq!(
            found(
                "app/main",
                &["Sources/App/Main.swift", "Sources/Views/MainView.swift"]
            ),
            ["Sources/App/Main.swift"]
        );
        assert_eq!(
            found(
                "view",
                &["ViewModel.swift", "Sources/Deep/Folder/View.swift"]
            )[0],
            "Sources/Deep/Folder/View.swift"
        );
        assert_eq!(
            found(
                "x",
                &["long/folder/x.swift", "x.swift", "b/x.swift", "a/x.swift"]
            ),
            ["x.swift", "b/x.swift", "a/x.swift", "long/folder/x.swift"]
        );
    }
    #[test]
    fn source_highlights_include_normalized_graphemes_and_expanding_casefolds() {
        assert_eq!(highlighted("main", "Sources/App/Main.swift"), ["Main"]);
        assert_eq!(highlighted("ft", "Sources/FileTab.swift"), ["F", "T"]);
        assert_eq!(
            highlighted("app/main", "Sources/App/Main.swift"),
            ["App/Main"]
        );
        assert_eq!(highlighted("résumé", "docs/Résumé.txt"), ["Résumé"]);
        assert_eq!(highlighted("i", "İstanbul.txt"), ["İ"]);
        assert!(found("é", &["docs/Ã©.txt"]).is_empty());
        assert!(found("e", &["é.txt"]).is_empty());
        assert_eq!(highlighted("e", "é-e.txt"), ["e"]);
        assert_eq!(highlighted("CAFÉ", "Docs/Cafe\u{301}.md"), ["Cafe\u{301}"]);
        assert_eq!(highlighted("CAFE\u{301}", "Docs/Café.txt"), ["Café"]);
        let decomposed: String = "노트/한글.md".nfd().collect();
        let expected: String = "글".nfd().collect();
        assert_eq!(highlighted("글", &decomposed), [expected]);
    }
    #[test]
    fn top_results_equal_full_scoring_and_cancellation_never_returns_partial_results() {
        let mut index = fixture(&[]);
        for n in 0..25_000 {
            index.paths.push(IndexedPath::new(PathBuf::from(format!(
                "folder{}/FileTab{n}.swift",
                n % 97
            ))));
        }
        let query = FileFinderQuery::new("ft");
        let mut all: Vec<_> = index
            .paths
            .iter()
            .enumerate()
            .filter_map(|(i, p)| score(&query, p, false).map(|s| (i, s.0)))
            .collect();
        all.sort_by_key(|(i, s)| (-*s, index.paths[*i].folded.len(), *i));
        let best = index.search(&query, 50, &CancellationToken::new()).unwrap();
        assert_eq!(
            best.iter().map(|m| (m.index, m.score)).collect::<Vec<_>>(),
            all[..50]
        );
        assert_eq!(
            index
                .search(&query, usize::MAX, &CancellationToken::new())
                .unwrap()
                .len(),
            MAX_SEARCH_RESULTS
        );
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            index.search(&query, 50, &token),
            Err(FinderError::Cancelled)
        ));
        let temp = Temp::new();
        let mut options = temp.options();
        options.cancellation = token;
        assert!(matches!(
            FileFinderIndex::build(&temp.0, &options),
            Err(FinderError::Cancelled)
        ));
        options.cancellation = CancellationToken::new();
        options.timeout = Duration::ZERO;
        assert!(matches!(
            FileFinderIndex::build(&temp.0, &options),
            Err(FinderError::TimedOut)
        ));
    }
    #[test]
    fn plain_folder_uses_nested_ignore_rules_and_keeps_dotfiles() {
        let temp = Temp::new();
        temp.write(".gitignore", "*.o\n*.log\n!keep.log\nbuild/\n");
        for name in [
            "a.c",
            "a.o",
            "debug.log",
            "keep.log",
            ".hidden",
            "build/no.txt",
            "src/main.rs",
            "src/debug.log",
        ] {
            temp.write(name, "x");
        }
        temp.write("src/.gitignore", "!*.log\n");
        let index = temp.build();
        assert!(!index.truncated(), "{:?}", index.warnings());
        assert_eq!(
            paths(&index),
            [
                ".gitignore",
                ".hidden",
                "a.c",
                "keep.log",
                "src/.gitignore",
                "src/debug.log",
                "src/main.rs"
            ]
        );
        assert_eq!(index.root(), fs::canonicalize(&temp.0).unwrap());
        assert!(index.path(0).unwrap().is_relative());
    }
    #[test]
    fn global_ignore_rules_have_lower_precedence() {
        let temp = Temp::new();
        let outside = Temp::new();
        outside.write("ignore", "*.orig\n.DS_Store\n");
        temp.write(".gitignore", "!wanted.orig\n");
        for name in ["a.orig", "wanted.orig", ".DS_Store", "kept.txt"] {
            temp.write(name, "x");
        }
        let mut options = temp.options();
        options.global_ignore = Some(outside.0.join("ignore"));
        let index = FileFinderIndex::build(&temp.0, &options).unwrap();
        assert_eq!(paths(&index), [".gitignore", "kept.txt", "wanted.orig"]);
    }
    #[test]
    fn listing_limits_are_explicit_even_with_only_directories() {
        let temp = Temp::new();
        for i in 0..30 {
            temp.write(format!("file-{i:02}.txt"), "x");
        }
        let mut options = temp.options();
        options.max_files = 10;
        let index = FileFinderIndex::build(&temp.0, &options).unwrap();
        assert_eq!(index.count(), 10);
        assert!(index.truncated());
        options.max_files = 1_000;
        options.max_path_bytes = 55;
        let index = FileFinderIndex::build(&temp.0, &options).unwrap();
        assert_eq!(index.count(), 5);
        assert!(index.truncated());
        options.max_files = 0;
        assert!(
            FileFinderIndex::build(&temp.0, &options)
                .unwrap()
                .truncated()
        );
        let empty = Temp::new();
        for i in 0..30 {
            fs::create_dir(empty.0.join(format!("empty-{i}"))).unwrap();
        }
        let mut options = empty.options();
        options.max_entries = 5;
        let index = FileFinderIndex::build(&empty.0, &options).unwrap();
        assert!(index.truncated());
        assert_eq!(index.count(), 0);
        let deep = Temp::new();
        deep.write("one/two/three/file", "x");
        let mut options = deep.options();
        options.max_depth = 1;
        let index = FileFinderIndex::build(&deep.0, &options).unwrap();
        assert!(index.truncated());
        assert_eq!(index.count(), 0);
    }
    #[test]
    fn git_lists_tracked_ignored_and_untracked_existing_files_in_project_scope() {
        let temp = Temp::new();
        temp.git(&["init", "-q"]);
        temp.write(".gitignore", "*.log\nbuild/\n");
        for name in ["src/main.rs", "logs/forced.log", "gone.txt"] {
            temp.write(name, "x");
        }
        temp.git(&["add", ".gitignore", "src/main.rs", "gone.txt"]);
        temp.git(&["add", "-f", "logs/forced.log"]);
        fs::remove_file(temp.0.join("gone.txt")).unwrap();
        for name in ["notes/todo.md", "debug.log", "build/out.o", "src/new.rs"] {
            temp.write(name, "x");
        }
        let mut listed = paths(&temp.build());
        listed.sort();
        assert_eq!(
            listed,
            [
                ".gitignore",
                "logs/forced.log",
                "notes/todo.md",
                "src/main.rs",
                "src/new.rs"
            ]
        );
        let index = FileFinderIndex::build(temp.0.join("src"), &temp.options()).unwrap();
        let mut listed = paths(&index);
        listed.sort();
        assert_eq!(listed, ["main.rs", "new.rs"]);
    }
    #[test]
    fn nested_repositories_keep_their_own_ignore_rules() {
        let temp = Temp::new();
        temp.git(&["init", "-q"]);
        temp.write("App.rs", "x");
        fs::create_dir(temp.0.join("nested")).unwrap();
        let nested = Temp(temp.0.join("nested"));
        nested.git(&["init", "-q"]);
        nested.write(".gitignore", "*.tmp\n");
        nested.write("lib.rs", "x");
        nested.write("skip.tmp", "x");
        let mut listed = paths(&temp.build());
        listed.sort();
        assert_eq!(listed, ["App.rs", "nested/.gitignore", "nested/lib.rs"]);
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_escape_cycle_or_supply_ignore_rules_and_non_utf8_paths_remain_lossless() {
        use std::os::unix::{ffi::OsStringExt, fs::symlink};
        let temp = Temp::new();
        let outside = Temp::new();
        outside.write("secret", "secret");
        temp.write("local", "local");
        temp.write("nested/child", "x");
        symlink("local", temp.0.join("alias")).unwrap();
        symlink(&outside.0, temp.0.join("escape-dir")).unwrap();
        symlink(outside.0.join("secret"), temp.0.join("escape-file")).unwrap();
        symlink("nested", temp.0.join("folder-link")).unwrap();
        symlink("..", temp.0.join("nested/cycle")).unwrap();
        symlink("gone", temp.0.join("broken")).unwrap();
        outside.write("ignore", "*\n");
        symlink(outside.0.join("ignore"), temp.0.join(".gitignore")).unwrap();
        let strange = OsString::from_vec(vec![b'a', 0xff, b'.', b'r', b's']);
        temp.write(&strange, "x");
        let index = temp.build();
        let listed = paths(&index);
        assert!(listed.contains(&"alias".into()));
        assert!(listed.contains(&"nested/child".into()));
        for forbidden in [
            "escape-dir",
            "escape-file",
            "folder-link",
            "broken",
            "nested/cycle",
            ".gitignore",
        ] {
            assert!(!listed.contains(&forbidden.into()), "{listed:?}");
        }
        let matched = index
            .search(&FileFinderQuery::new("ars"), 50, &CancellationToken::new())
            .unwrap();
        assert_eq!(matched[0].path.as_os_str(), strange.as_os_str());
        assert_eq!(fs::read(index.root().join(&matched[0].path)).unwrap(), b"x");
        assert!(!valid_relative(Path::new("../outside")));
        assert!(!valid_relative(Path::new("/absolute")));
    }
    #[cfg(unix)]
    #[test]
    fn fifo_ignore_files_do_not_hang() {
        let temp = Temp::new();
        temp.write("visible", "x");
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(temp.0.join(".gitignore"))
                .status()
                .unwrap()
                .success()
        );
        let index = temp.build();
        assert_eq!(paths(&index), ["visible"]);
    }
    #[test]
    fn oversized_ignore_file_is_reported_without_hiding_files() {
        let temp = Temp::new();
        temp.write(".gitignore", &"*".repeat(IGNORE_FILE_LIMIT + 1));
        temp.write("visible", "x");
        let index = temp.build();
        assert_eq!(index.count(), 2);
        assert!(!index.warnings().is_empty());
    }
    #[test]
    fn source_wildmatch_cases() {
        let cases = [
            ("foo", "foo", true),
            ("foo", "bar", false),
            ("foo", "???", true),
            ("foo", "??", false),
            ("foo", "*", true),
            ("foo", "f*", true),
            ("foo", "*f", false),
            ("foo", "*foo*", true),
            ("foobar", "*ob*a*r*", true),
            ("aaaaaaabababab", "*ab", true),
            ("foo*", "foo\\*", true),
            ("foobar", "foo\\*bar", false),
            ("ball", "*[al]?", true),
            ("ten", "[ten]", false),
            ("ten", "t[a-g]n", true),
            ("ton", "t[!a-g]n", true),
            ("ton", "t[^a-g]n", true),
            ("a]b", "a[]]b", true),
            ("a-b", "a[]-]b", true),
            ("aab", "a[]-]b", false),
            ("aab", "a[]a-]b", true),
            ("]", "]", true),
            ("foo/baz/bar", "foo*bar", false),
            ("foo/baz/bar", "foo**bar", false),
            ("foobazbar", "foo**bar", true),
            ("foo/baz/bar", "foo/**/bar", true),
            ("foo/b/a/z/bar", "foo/**/bar", true),
            ("foo/bar", "foo/**/bar", true),
            ("foo/bar", "foo/**/**/bar", true),
            ("foo/bar", "foo?bar", false),
            ("foo/bar", "foo[/]bar", false),
            ("foo", "**/foo", true),
            ("bar/baz/foo", "**/foo", true),
            ("bar/baz/foo", "*/foo", false),
            ("deep/foo/bar/baz", "**/bar/*", true),
            ("deep/foo/bar/baz/", "**/bar/*", false),
            ("deep/foo/bar/baz/", "**/bar/**", true),
            ("deep/foo/bar", "**/bar/*", false),
            ("foo/bar/baz/x", "*/bar/**", true),
            ("deep/foo/bar/baz/x", "*/bar/**", false),
            ("1", "[[:digit:]]", true),
            ("a", "[[:digit:]]", false),
            ("a", "[[:alpha:]]", true),
            ("-", "[[:punct:]]", true),
        ];
        let options = FileListingOptions::default();
        let mut scan = Scan::new(&options).unwrap();
        for (text, pattern, expected) in cases {
            assert_eq!(
                wildmatch(pattern.as_bytes(), text.as_bytes(), true, &mut scan).unwrap(),
                Some(expected),
                "{text} against {pattern}"
            );
        }
        assert_eq!(
            wildmatch(b"x*y", b"x/y", false, &mut scan).unwrap(),
            Some(true)
        );
    }
    #[test]
    fn ignore_lines_preserve_escaped_spaces_comments_and_negation() {
        assert!(IgnorePattern::parse(b"# comment").is_none());
        assert!(IgnorePattern::parse(b"").is_none());
        assert_eq!(IgnorePattern::parse(b"trail   ").unwrap().bytes, b"trail");
        assert_eq!(
            IgnorePattern::parse(b"spaced\\ ").unwrap().bytes,
            b"spaced\\ "
        );
        assert_eq!(IgnorePattern::parse(b"crlf\r").unwrap().bytes, b"crlf");
        let pattern = IgnorePattern::parse(b"!keep/").unwrap();
        assert!(pattern.negated && pattern.directory_only && pattern.basename_only);
        assert!(!IgnorePattern::parse(b"a/b").unwrap().basename_only);
        assert!(!IgnorePattern::parse(b"\\!bang").unwrap().negated);
    }
    #[cfg(unix)]
    #[test]
    fn repository_configuration_cannot_run_file_listing_helpers() {
        use std::os::unix::fs::PermissionsExt;
        let temp = Temp::new();
        let outside = Temp::new();
        temp.git(&["init", "-q"]);
        temp.write("tracked", "x");
        temp.git(&["add", "tracked"]);
        let marker = outside.0.join("helper-ran");
        let helper = outside.0.join("helper");
        fs::write(
            &helper,
            format!("#!/bin/sh\nprintf executed > '{}'\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        temp.git(&["config", "core.fsmonitor", helper.to_str().unwrap()]);
        temp.git(&["config", "core.pager", helper.to_str().unwrap()]);
        temp.git(&["config", "core.hooksPath", helper.to_str().unwrap()]);
        temp.git(&[
            "config",
            "alias.ls-files",
            &format!("!{}", helper.display()),
        ]);
        outside.write("ignore", "*.ignored\n");
        temp.git(&[
            "config",
            "core.excludesFile",
            outside.0.join("ignore").to_str().unwrap(),
        ]);
        temp.write("untracked", "x");
        temp.write("skip.ignored", "x");
        // None exercises the repo-local config lookup as well as ls-files.
        let options = FileListingOptions {
            global_ignore: None,
            ..temp.options()
        };
        let mut files = paths(&FileFinderIndex::build(&temp.0, &options).unwrap());
        files.sort();
        assert_eq!(files, ["tracked", "untracked"]);
        assert!(
            !marker.exists(),
            "A repository-controlled helper was executed"
        );
    }
    #[test]
    fn source_ignore_corpus_agrees_with_gits_decisions_without_a_repository() {
        let temp = Temp::new();
        temp.git(&["init", "-q"]);
        temp.write(".gitignore", "\u{FEFF}# a comment\n\n*.log\n!keep.log\n/build\ndocs/*.tmp\n**/cache/\nout/**\na/**/z.txt\ntrailing.txt   \nspaced\\ \n\\#hash.txt\n\\!bang.txt\n[abc].c\n[!x]y.c\n[a-c]range.c\nfile[[:digit:]].txt\n?.q\ntmp*\nx**y\nignored-dir/\n!ignored-dir/back.txt\nsub/deep/\ncrlf.txt\r\nlast-line-without-newline.txt");
        temp.write("pkg/.gitignore", "!*.log\nlocal.txt\n/anchored.txt\n");
        temp.write("pkg/inner/.gitignore", "*.log\n!important.log\n");
        let files = [
            "app.log",
            "keep.log",
            "build/x.txt",
            "src/build/y.txt",
            "docs/a.tmp",
            "docs/deeper/b.tmp",
            "cache/c.txt",
            "src/cache/d.txt",
            "cachefile",
            "out/e.txt",
            "out/f/g.txt",
            "a/z.txt",
            "a/b/c/z.txt",
            "b/a/z.txt",
            "trailing.txt",
            "spaced ",
            "spaced",
            "#hash.txt",
            "!bang.txt",
            "a.c",
            "b.c",
            "d.c",
            "xy.c",
            "zy.c",
            "arange.c",
            "drange.c",
            "file1.txt",
            "filex.txt",
            "a.q",
            "ab.q",
            "tmpfile",
            "tmp/inside.txt",
            "xy",
            "x1y",
            "x/y",
            "ignored-dir/back.txt",
            "ignored-dir/other.txt",
            "sub/deep/h.txt",
            "other/sub/deep/h.txt",
            "crlf.txt",
            "last-line-without-newline.txt",
            "pkg/app.log",
            "pkg/local.txt",
            "pkg/anchored.txt",
            "pkg/x/anchored.txt",
            "pkg/inner/y.log",
            "pkg/inner/important.log",
            "pkg/inner/local.txt",
            "README.md",
            "src/main.rs",
        ];
        for name in &files {
            temp.write(name, "x");
        }
        use std::io::Write;
        use std::process::Stdio;
        let mut child = Command::new("/usr/bin/git")
            .current_dir(&temp.0)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(["check-ignore", "--no-index", "-z", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(files.join("\0").as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let ignored: HashSet<_> = output
            .stdout
            .split(|b| *b == 0)
            .filter(|b| !b.is_empty())
            .map(|b| std::str::from_utf8(b).unwrap().to_owned())
            .collect();
        assert_eq!(ignored.len(), 32);
        fs::remove_dir_all(temp.0.join(".git")).unwrap();
        let actual: HashSet<_> = paths(&temp.build()).into_iter().collect();
        let expected: HashSet<_> = files
            .into_iter()
            .filter(|p| !ignored.contains(*p))
            .chain([".gitignore", "pkg/.gitignore", "pkg/inner/.gitignore"])
            .map(str::to_owned)
            .collect();
        assert_eq!(actual, expected);
    }
    #[test]
    fn exhausted_ignore_budget_never_masquerades_as_a_successful_nonmatch() {
        let options = FileListingOptions::default();
        let mut scan = Scan::new(&options).unwrap();
        scan.work = WORK_LIMIT;
        assert_eq!(wildmatch(b"[a-z]", b"x", true, &mut scan).unwrap(), None);
        let rules = IgnoreRules {
            base: Vec::new(),
            patterns: vec![IgnorePattern::parse(b"[a-z]").unwrap()],
            parent: None,
        };
        assert_eq!(rules.ignored(b"x", false, &mut scan).unwrap(), None);
    }
}
