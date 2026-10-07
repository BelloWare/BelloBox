use super::{MovieError, MovieResult};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Ordinary source-change detection, not a hostile-filesystem TOCTOU boundary.
/// The future URL-based AVAsset does not inherit this descriptor automatically.
#[derive(Debug)]
pub(super) struct SourceFile {
    path: PathBuf,
    file: File,
    length: u64,
    modified: Option<SystemTime>,
    changed: (i64, i64),
}
impl SourceFile {
    /// Snapshot only after the transaction's intentional hard-link/unlink work.
    pub fn from_owned_file(path: PathBuf, file: File) -> MovieResult<Self> {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata().map_err(|_| MovieError::Io)?;
        if !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > crate::recording::MAX_RECORDING_FILE_BYTES
        {
            return Err(MovieError::InvalidSource);
        }
        let result = Self {
            path,
            file,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        };
        result.verify()?;
        Ok(result)
    }
    pub fn copy_recording_to(
        &self,
        output: &mut File,
        cancelled: impl Fn() -> bool,
    ) -> MovieResult<()> {
        use std::{io::Write, os::unix::fs::FileExt};
        self.verify()?;
        if self.length > crate::recording::MAX_RECORDING_FILE_BYTES {
            return Err(MovieError::LimitExceeded);
        }
        let mut buffer = [0u8; 64 * 1024];
        let mut offset = 0;
        while offset < self.length {
            if cancelled() {
                return Err(MovieError::Cancelled);
            }
            let count = (self.length - offset).min(buffer.len() as u64) as usize;
            let read = self
                .file
                .read_at(&mut buffer[..count], offset)
                .map_err(|_| MovieError::Io)?;
            if read == 0 {
                return Err(MovieError::SourceChanged);
            }
            output
                .write_all(&buffer[..read])
                .map_err(|_| MovieError::Io)?;
            offset += read as u64;
        }
        if cancelled() {
            return Err(MovieError::Cancelled);
        }
        self.verify()
    }

    pub fn open(path: &Path) -> MovieResult<Self> {
        let path = fs::canonicalize(path).map_err(|_| MovieError::InvalidSource)?;
        let entry = fs::metadata(&path).map_err(|_| MovieError::InvalidSource)?;
        if !entry.is_file() || entry.len() == 0 {
            return Err(MovieError::InvalidSource);
        }
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        // Nonblocking open prevents a path replaced with a FIFO from hanging.
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| MovieError::Io)?;
        let metadata = file.metadata().map_err(|_| MovieError::Io)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(MovieError::InvalidSource);
        }
        Ok(Self {
            path,
            file,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn verify(&self) -> MovieResult<()> {
        use std::os::unix::fs::MetadataExt;
        let current = fs::metadata(&self.path).map_err(|_| MovieError::SourceChanged)?;
        let opened = self
            .file
            .metadata()
            .map_err(|_| MovieError::SourceChanged)?;
        if !current.is_file()
            || !same_identity(&current, &opened)
            || opened.len() != self.length
            || opened.modified().ok() != self.modified
            || (opened.ctime(), opened.ctime_nsec()) != self.changed
        {
            return Err(MovieError::SourceChanged);
        }
        Ok(())
    }
}
fn same_identity(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}
