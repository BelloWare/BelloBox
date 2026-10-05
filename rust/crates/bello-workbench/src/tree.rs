//! A lazy, bounded directory browser. All I/O is synchronous: call from a
//! background worker and discard superseded results before updating a UI.
//!
//! There is no recursive index. Paths are stable, lossless, root-relative IDs.
//! Symbolic links remain visible, but directory links cannot be expanded.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const MAX_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_RELATIVE_PATH_BYTES: usize = 32_768;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    /// Stable root-relative identifier, preserving non-UTF-8 filenames.
    pub path: PathBuf,
    pub name: OsString,
    pub kind: EntryKind,
    /// False for an escaping, broken, or unreadable symbolic-link target.
    /// This is descriptive, not authority to open the target without checking.
    pub target_within_root: bool,
}

impl DirectoryEntry {
    pub fn is_expandable(&self) -> bool {
        self.kind == EntryKind::Directory && self.target_within_root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryPage {
    pub entries: Vec<DirectoryEntry>,
    /// More entries exist than this request can inspect. For a truncated
    /// directory, the inspected subset is sorted; it is not a global sorted
    /// prefix. No unbounded scan is performed to find that prefix.
    pub truncated: bool,
}

#[derive(Clone, Debug)]
pub struct DirectoryTree {
    root: PathBuf,
}

impl DirectoryTree {
    /// Canonicalizes the root but does not enumerate it or any descendant.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !fs::metadata(&root)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tree root is not a directory",
            ));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads one level only, with at most `min(limit, MAX_DIRECTORY_ENTRIES)`
    /// results and one look-ahead entry. Directories sort first, then names in
    /// lossless platform order. `.git` is omitted; other hidden files remain.
    ///
    /// Absolute paths, `..`, and expansion through *any* symbolic link are
    /// rejected, including links to directories inside the root (no cycles).
    /// Like ordinary filesystem APIs, this is not an OS sandbox against an
    /// adversary concurrently replacing ancestors; callers opening files must
    /// independently validate their paths at the moment they open them.
    pub fn read_dir(&self, relative: impl AsRef<Path>, limit: usize) -> io::Result<DirectoryPage> {
        let relative = normalized_relative(relative.as_ref())?;
        let mut absolute = self.root.clone();
        for component in relative.components() {
            absolute.push(component.as_os_str());
            let metadata = fs::symlink_metadata(&absolute)?;
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "directory links are not expanded",
                ));
            }
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "requested path is not a directory",
                ));
            }
        }
        let canonical = fs::canonicalize(&absolute)?;
        if !canonical.starts_with(&self.root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "directory escapes the tree root",
            ));
        }
        let limit = limit.min(MAX_DIRECTORY_ENTRIES);
        let mut entries = Vec::with_capacity(limit.min(256));
        let mut truncated = false;
        for entry in fs::read_dir(&canonical)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == ".git" {
                continue;
            }
            if entries.len() == limit {
                truncated = true;
                break;
            }
            let file_type = entry.file_type()?;
            let kind = if file_type.is_symlink() {
                EntryKind::Symlink
            } else if file_type.is_dir() {
                EntryKind::Directory
            } else if file_type.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            let target_within_root = if kind == EntryKind::Symlink {
                fs::canonicalize(entry.path()).is_ok_and(|path| path.starts_with(&self.root))
            } else {
                true
            };
            entries.push(DirectoryEntry {
                path: relative.join(&name),
                name,
                kind,
                target_within_root,
            });
        }
        entries.sort_by(|a, b| {
            (a.kind != EntryKind::Directory)
                .cmp(&(b.kind != EntryKind::Directory))
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(DirectoryPage { entries, truncated })
    }
}

fn normalized_relative(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().len() > MAX_RELATIVE_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "relative path exceeds the size limit",
        ));
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "a root-relative path without '..' is required",
                ));
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "bello-tree-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lazy_single_level_sorted_and_stable_relative_paths() {
        let temp = Temp::new();
        fs::create_dir(temp.0.join("z folder")).unwrap();
        fs::create_dir(temp.0.join(".git")).unwrap();
        fs::write(temp.0.join("z folder/child.txt"), "nested").unwrap();
        fs::write(temp.0.join("a.txt"), "a").unwrap();
        fs::write(temp.0.join("b.txt"), "b").unwrap();
        let tree = DirectoryTree::new(&temp.0).unwrap();
        let page = tree.read_dir(".", 20).unwrap();
        assert!(!page.truncated);
        assert_eq!(
            page.entries
                .iter()
                .map(|e| e.path.clone())
                .collect::<Vec<_>>(),
            [
                PathBuf::from("z folder"),
                PathBuf::from("a.txt"),
                PathBuf::from("b.txt")
            ]
        );
        assert!(page.entries[0].is_expandable());
        let child = tree.read_dir("./z folder", 20).unwrap();
        assert_eq!(child.entries[0].path, PathBuf::from("z folder/child.txt"));
        fs::write(temp.0.join("z folder/later.txt"), "later").unwrap();
        assert_eq!(tree.read_dir("z folder", 20).unwrap().entries.len(), 2);
    }

    #[test]
    fn bounded_and_rejects_traversal() {
        let temp = Temp::new();
        for i in 0..10 {
            fs::write(temp.0.join(format!("{i}")), "").unwrap();
        }
        let tree = DirectoryTree::new(&temp.0).unwrap();
        let page = tree.read_dir("", 3).unwrap();
        assert_eq!(page.entries.len(), 3);
        assert!(page.truncated);
        assert!(page.entries.windows(2).all(|w| w[0].name <= w[1].name));
        assert!(tree.read_dir("", 0).unwrap().truncated);
        assert!(tree.read_dir("../", 10).is_err());
        assert!(tree.read_dir(&temp.0, 10).is_err());
        assert!(tree.read_dir("missing", 10).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_escape_or_form_expansion_cycles() {
        use std::os::unix::fs::symlink;
        let temp = Temp::new();
        let outside = Temp::new();
        fs::create_dir(temp.0.join("nested")).unwrap();
        fs::write(temp.0.join("local"), "local").unwrap();
        fs::write(outside.0.join("secret"), "secret").unwrap();
        symlink(&outside.0, temp.0.join("escape")).unwrap();
        symlink("nested", temp.0.join("inside")).unwrap();
        symlink("local", temp.0.join("file-link")).unwrap();
        symlink("missing", temp.0.join("broken")).unwrap();
        symlink("..", temp.0.join("nested/cycle")).unwrap();
        let tree = DirectoryTree::new(&temp.0).unwrap();
        assert!(tree.read_dir("escape", 10).is_err());
        assert!(tree.read_dir("inside", 10).is_err());
        assert!(tree.read_dir("nested/cycle", 10).is_err());
        let page = tree.read_dir("", 10).unwrap();
        let entry = |name: &str| page.entries.iter().find(|e| e.name == name).unwrap();
        assert!(!entry("escape").target_within_root);
        assert!(!entry("broken").target_within_root);
        assert!(entry("file-link").target_within_root);
        assert!(!entry("inside").is_expandable());
    }

    // macOS filesystems reject invalid UTF-8 names before directory enumeration.
    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_names_are_lossless_on_disk() {
        use std::os::unix::ffi::OsStringExt;
        let temp = Temp::new();
        let name = OsString::from_vec(vec![b'x', 0xff]);
        fs::write(temp.0.join(&name), "bytes").unwrap();
        let page = DirectoryTree::new(&temp.0)
            .unwrap()
            .read_dir("", 1)
            .unwrap();
        assert_eq!(page.entries[0].name, name);
        assert!(!page.truncated);
    }

    #[test]
    fn unicode_names_are_lossless_on_disk() {
        let temp = Temp::new();
        let name = "日本語.txt";
        fs::write(temp.0.join(name), "unicode").unwrap();
        let page = DirectoryTree::new(&temp.0)
            .unwrap()
            .read_dir("", 1)
            .unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].name, name);
        assert_eq!(page.entries[0].path, Path::new(name));
        assert!(!page.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_relative_paths_normalize_losslessly_without_filesystem_io() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let path = PathBuf::from(OsString::from_vec(b"./nested/./x\xff".to_vec()));
        let normalized = normalized_relative(&path).unwrap();
        assert_eq!(normalized.as_os_str().as_bytes(), b"nested/x\xff");
        assert!(normalized_relative(&Path::new("..").join(&normalized)).is_err());
        assert!(normalized_relative(&Path::new("/").join(&normalized)).is_err());
    }
}
