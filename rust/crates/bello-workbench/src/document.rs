//! Bounded UTF-8 file loading and conflict-detecting atomic saves. These APIs
//! block and belong on a background worker. Atomic rename is not a filesystem
//! compare-and-swap: a non-cooperating writer can still race the last check.
use crate::editor::MAX_EDIT_BYTES;
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const PREVIEW_BYTES: usize = 256 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum FileError {
    Io(io::Error),
    OutsideRoot,
    NotRegular,
    SymbolicLink,
    HardLinked,
    ChangedOnDisk,
    TooLarge,
    NotUtf8,
    Binary,
}
impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::OutsideRoot => write!(f, "File is outside the selected project"),
            Self::NotRegular => write!(f, "Only regular files can be opened"),
            Self::SymbolicLink => write!(
                f,
                "Symbolic links are read-only; open the resolved file explicitly"
            ),
            Self::HardLinked => write!(
                f,
                "Saving a multiply linked file is disabled to preserve its links"
            ),
            Self::ChangedOnDisk => write!(
                f,
                "File changed on disk. Your draft is preserved; reopen after copying it"
            ),
            Self::TooLarge => write!(f, "File exceeds the 8 MiB editing limit"),
            Self::NotUtf8 => write!(f, "File is not valid UTF-8; no lossy editing is allowed"),
            Self::Binary => write!(f, "Binary file cannot be edited as text"),
        }
    }
}
impl std::error::Error for FileError {}
impl From<io::Error> for FileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    len: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(unix)]
    changed: (i64, i64),
}
impl Identity {
    fn of(m: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            len: m.len(),
            modified: m.modified().ok(),
            #[cfg(unix)]
            dev: m.dev(),
            #[cfg(unix)]
            ino: m.ino(),
            #[cfg(unix)]
            changed: (m.ctime(), m.ctime_nsec()),
        }
    }
}
#[derive(Clone, Debug)]
pub struct FileDocument {
    path: PathBuf,
    root: PathBuf,
    identity: Identity,
    baseline: String,
}
#[derive(Clone, Debug)]
pub struct FilePreview {
    pub path: PathBuf,
    pub text: String,
    pub total_bytes: u64,
    pub reason: String,
}
#[derive(Clone, Debug)]
pub enum OpenedFile {
    Editable(FileDocument),
    Preview(FilePreview),
}
#[derive(Clone, Copy, Debug)]
pub struct SaveOutcome {
    pub directory_synced: bool,
}

fn checked_path(root: &Path, path: &Path) -> Result<(PathBuf, PathBuf, Metadata), FileError> {
    let root = root.canonicalize()?;
    let requested = if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    };
    let m = fs::symlink_metadata(&requested)?;
    if m.file_type().is_symlink() {
        return Err(FileError::SymbolicLink);
    }
    if !m.is_file() {
        return Err(FileError::NotRegular);
    }
    let canonical = requested.canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(FileError::OutsideRoot);
    }
    Ok((root, canonical, m))
}
impl FileDocument {
    pub fn open(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Result<OpenedFile, FileError> {
        let (root, path, meta) = checked_path(root.as_ref(), path.as_ref())?;
        let identity = Identity::of(&meta);
        let limit = if meta.len() > MAX_EDIT_BYTES as u64 {
            PREVIEW_BYTES
        } else {
            MAX_EDIT_BYTES
        };
        let mut file = File::open(&path)?;
        if Identity::of(&file.metadata()?) != identity {
            return Err(FileError::ChangedOnDisk);
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)?;
        if Identity::of(&file.metadata()?) != identity
            || Identity::of(&fs::symlink_metadata(&path)?) != identity
        {
            return Err(FileError::ChangedOnDisk);
        }
        if bytes.contains(&0) {
            return Err(FileError::Binary);
        }
        let truncated = bytes.len() > limit || meta.len() > limit as u64;
        if truncated {
            bytes.truncate(limit);
        }
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                let v = e.utf8_error().valid_up_to();
                if truncated && e.utf8_error().error_len().is_none() {
                    String::from_utf8(e.into_bytes()[..v].to_vec()).expect("valid UTF-8 prefix")
                } else {
                    return Err(FileError::NotUtf8);
                }
            }
        };
        if truncated {
            Ok(OpenedFile::Preview(FilePreview {
                path,
                text,
                total_bytes: meta.len(),
                reason: format!(
                    "Read-only preview of first {} KiB; file is {} bytes (8 MiB edit limit)",
                    limit / 1024,
                    meta.len()
                ),
            }))
        } else {
            Ok(OpenedFile::Editable(Self {
                path,
                root,
                identity,
                baseline: text,
            }))
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn text(&self) -> &str {
        &self.baseline
    }
    pub fn is_dirty(&self, text: &str) -> bool {
        text != self.baseline
    }
    fn check_current(&self) -> Result<Metadata, FileError> {
        let (root, path, m) = checked_path(&self.root, &self.path)?;
        if root != self.root || path != self.path || Identity::of(&m) != self.identity {
            return Err(FileError::ChangedOnDisk);
        }
        Ok(m)
    }
    pub fn save(&mut self, text: &str) -> Result<SaveOutcome, FileError> {
        if text.len() > MAX_EDIT_BYTES {
            return Err(FileError::TooLarge);
        }
        if text.as_bytes().contains(&0) {
            return Err(FileError::Binary);
        }
        let metadata = self.check_current()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() > 1 {
                return Err(FileError::HardLinked);
            }
        }
        let parent = self.path.parent().ok_or(FileError::NotRegular)?;
        let mut temp = None;
        for _ in 0..32 {
            let path = parent.join(format!(
                ".bello-save-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(f) => {
                    temp = Some((TempFile(path), f));
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        let (mut staged, mut writer) = temp.ok_or_else(|| {
            FileError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Could not reserve a save file",
            ))
        })?;
        writer.set_permissions(metadata.permissions())?;
        writer.write_all(text.as_bytes())?;
        writer.sync_all()?;
        drop(writer);
        self.check_current()?;
        let mut current = Vec::new();
        File::open(&self.path)?
            .take(MAX_EDIT_BYTES as u64 + 1)
            .read_to_end(&mut current)?;
        if current != self.baseline.as_bytes() {
            return Err(FileError::ChangedOnDisk);
        }
        self.check_current()?;
        fs::rename(&staged.0, &self.path)?;
        staged.0 = PathBuf::new();
        self.baseline = text.to_owned();
        self.identity = Identity::of(&fs::metadata(&self.path)?);
        let directory_synced = File::open(parent).and_then(|d| d.sync_all()).is_ok();
        Ok(SaveOutcome { directory_synced })
    }
}
struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "bello-document-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn editable(root: &Path) -> FileDocument {
        match FileDocument::open(root, "test.txt").unwrap() {
            OpenedFile::Editable(d) => d,
            _ => panic!("expected editable"),
        }
    }
    #[test]
    fn atomic_save_and_dirty() {
        let p = fixture();
        fs::write(p.join("test.txt"), "a\r\n😀").unwrap();
        let mut d = editable(&p);
        assert!(!d.is_dirty("a\r\n😀"));
        d.save("changed\r\n😀").unwrap();
        assert_eq!(
            fs::read_to_string(p.join("test.txt")).unwrap(),
            "changed\r\n😀"
        );
        assert!(!d.is_dirty("changed\r\n😀"));
        assert_eq!(fs::read_dir(&p).unwrap().count(), 1);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn external_write_preserves_both_drafts() {
        let p = fixture();
        fs::write(p.join("test.txt"), "original").unwrap();
        let mut d = editable(&p);
        fs::write(p.join("test.txt"), "external").unwrap();
        assert!(matches!(
            d.save("local draft"),
            Err(FileError::ChangedOnDisk)
        ));
        assert_eq!(d.text(), "original");
        assert_eq!(fs::read_to_string(p.join("test.txt")).unwrap(), "external");
        assert_eq!(fs::read_dir(&p).unwrap().count(), 1);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn large_file_preview_never_editable() {
        let p = fixture();
        let f = File::create(p.join("test.txt")).unwrap();
        f.set_len(MAX_EDIT_BYTES as u64 + 1).unwrap();
        assert!(matches!(
            FileDocument::open(&p, "test.txt"),
            Err(FileError::Binary)
        ));
        fs::write(p.join("test.txt"), "a".repeat(MAX_EDIT_BYTES + 1)).unwrap();
        match FileDocument::open(&p, "test.txt").unwrap() {
            OpenedFile::Preview(d) => assert_eq!(d.text.len(), PREVIEW_BYTES),
            _ => panic!("large file editable"),
        };
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn invalid_utf8_never_lossy() {
        let p = fixture();
        fs::write(p.join("test.txt"), [0xff, 0xfe]).unwrap();
        assert!(matches!(
            FileDocument::open(&p, "test.txt"),
            Err(FileError::NotUtf8)
        ));
        fs::remove_dir_all(p).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_replacement_refused() {
        use std::os::unix::fs::symlink;
        let p = fixture();
        fs::write(p.join("test.txt"), "original").unwrap();
        let mut d = editable(&p);
        fs::write(p.join("other.txt"), "other").unwrap();
        fs::remove_file(p.join("test.txt")).unwrap();
        symlink(p.join("other.txt"), p.join("test.txt")).unwrap();
        assert!(d.save("new").is_err());
        assert_eq!(fs::read_to_string(p.join("other.txt")).unwrap(), "other");
        fs::remove_dir_all(p).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn permissions_preserved() {
        use std::os::unix::fs::PermissionsExt;
        let p = fixture();
        fs::write(p.join("test.txt"), "one").unwrap();
        fs::set_permissions(p.join("test.txt"), fs::Permissions::from_mode(0o640)).unwrap();
        let mut d = editable(&p);
        d.save("two").unwrap();
        assert_eq!(
            fs::metadata(p.join("test.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
        fs::remove_dir_all(p).unwrap();
    }
}
