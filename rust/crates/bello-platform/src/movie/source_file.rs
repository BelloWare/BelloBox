use super::{MovieError, MovieResult};
use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Ordinary source-change detection, not a hostile-filesystem TOCTOU boundary.
/// The future URL-based AVAsset does not inherit this descriptor automatically.
pub(super) struct SourceFile {
    path: PathBuf,
    file: File,
    length: u64,
    modified: Option<SystemTime>,
}
impl SourceFile {
    pub fn open(path: &Path) -> MovieResult<Self> {
        let path = fs::canonicalize(path).map_err(|_| MovieError::InvalidSource)?;
        let entry = fs::metadata(&path).map_err(|_| MovieError::InvalidSource)?;
        if !entry.is_file() || entry.len() == 0 {
            return Err(MovieError::InvalidSource);
        }
        let file = File::open(&path).map_err(|_| MovieError::Io)?;
        let metadata = file.metadata().map_err(|_| MovieError::Io)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(MovieError::InvalidSource);
        }
        Ok(Self {
            path,
            file,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn verify(&self) -> MovieResult<()> {
        let current = fs::metadata(&self.path).map_err(|_| MovieError::SourceChanged)?;
        let opened = self
            .file
            .metadata()
            .map_err(|_| MovieError::SourceChanged)?;
        if !current.is_file()
            || !same_identity(&current, &opened)
            || opened.len() != self.length
            || opened.modified().ok() != self.modified
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
