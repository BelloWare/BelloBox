use super::GifError;
use std::{
    fs::{self, File, Metadata, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

/// No filesystem export is available without both private-file creation and
/// stable identity checks. This gate runs before path reads, source callbacks,
/// or writes, including synthetic exports and initially absent destinations.
pub(super) fn ensure_filesystem_supported() -> Result<(), GifError> {
    if cfg!(unix) {
        Ok(())
    } else {
        Err(GifError::UnsupportedFilesystem)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReplacePolicy {
    /// Atomic no-clobber publication. An existing destination always survives.
    #[default]
    RefuseExisting,
    /// Explicit permission to atomically replace a regular destination file.
    /// Symlinks, directories and any alias of the source remain forbidden.
    ReplaceExistingFile,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ExportStatus {
    #[default]
    Ready = 0,
    Running = 1,
    Cancelled = 2,
    /// Publication owns the outcome, but filesystem work may still fail.
    PublicationClaimed = 3,
    Published = 4,
    Failed = 5,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
    /// Cancellation lost the claim race; this does not mean a file was saved.
    PublicationClaimed,
    AlreadyPublished,
    AlreadyFailed,
}

/// Single-use atomic control. Cancellation and publication-start compete for one
/// state transition, without waiting for filesystem work. Accepted cancellation
/// prevents publication; after a publication claim, await the worker's result.
/// Neither cancellation nor a claim implies that physical work has retired.
/// Clone this into the owner/UI and pass another clone to the export worker.
#[derive(Clone, Debug, Default)]
pub struct ExportControl {
    state: Arc<AtomicU8>,
}
impl ExportControl {
    pub fn status(&self) -> Result<ExportStatus, GifError> {
        Ok(match self.state.load(Ordering::Acquire) {
            0 => ExportStatus::Ready,
            1 => ExportStatus::Running,
            2 => ExportStatus::Cancelled,
            3 => ExportStatus::PublicationClaimed,
            4 => ExportStatus::Published,
            5 => ExportStatus::Failed,
            _ => unreachable!("invalid private GIF export state"),
        })
    }
    pub fn cancel(&self) -> Result<CancelOutcome, GifError> {
        // Keep the declared workspace MSRV: try_update requires Rust 1.95.
        #[allow(deprecated)] // fetch_update is the same operation, stable since 1.45.
        let previous = self
            .state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                matches!(state, 0 | 1).then_some(ExportStatus::Cancelled as u8)
            });
        Ok(match previous {
            Ok(_) | Err(2) => CancelOutcome::Cancelled,
            Err(3) => CancelOutcome::PublicationClaimed,
            Err(4) => CancelOutcome::AlreadyPublished,
            Err(5) => CancelOutcome::AlreadyFailed,
            Err(_) => unreachable!("invalid private GIF export state"),
        })
    }
    pub fn check_active(&self) -> Result<(), GifError> {
        match self.status()? {
            ExportStatus::Ready | ExportStatus::Running => Ok(()),
            ExportStatus::Cancelled => Err(GifError::Cancelled),
            _ => Err(GifError::ControlAlreadyUsed),
        }
    }
    pub(super) fn begin(&self) -> Result<JobGuard<'_>, GifError> {
        self.transition(ExportStatus::Ready, ExportStatus::Running)?;
        Ok(JobGuard(self))
    }
    pub(super) fn publish<T>(
        &self,
        action: impl FnOnce() -> Result<T, GifError>,
    ) -> Result<T, GifError> {
        self.transition(ExportStatus::Running, ExportStatus::PublicationClaimed)?;
        let _publication = PublicationGuard(self);
        let result = action()?;
        self.state
            .store(ExportStatus::Published as u8, Ordering::Release);
        Ok(result)
    }
    fn transition(&self, from: ExportStatus, to: ExportStatus) -> Result<(), GifError> {
        self.state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|state| {
                if state == ExportStatus::Cancelled as u8 {
                    GifError::Cancelled
                } else {
                    GifError::ControlAlreadyUsed
                }
            })
    }
}
pub(super) struct JobGuard<'a>(&'a ExportControl);
impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        let _ = self
            .0
            .transition(ExportStatus::Running, ExportStatus::Failed);
    }
}
/// Error or unwind after claiming must not leave the outcome indefinitely pending.
struct PublicationGuard<'a>(&'a ExportControl);
impl Drop for PublicationGuard<'_> {
    fn drop(&mut self) {
        let _ = self
            .0
            .transition(ExportStatus::PublicationClaimed, ExportStatus::Failed);
    }
}

struct SourceIdentity {
    canonical: PathBuf,
    file: File,
}
pub(super) struct Target {
    pub path: PathBuf,
    parent: PathBuf,
    parent_identity: Metadata,
    source: Option<SourceIdentity>,
    policy: ReplacePolicy,
}
impl Target {
    pub fn new(
        source: Option<&Path>,
        destination: &Path,
        policy: ReplacePolicy,
    ) -> Result<Self, GifError> {
        ensure_filesystem_supported()?;
        let leaf = destination
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(GifError::UnsafeDestination)?;
        let input_parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(input_parent)?;
        let parent_identity = fs::metadata(&parent)?;
        if !parent_identity.is_dir() {
            return Err(GifError::UnsafeDestination);
        }
        let source = source
            .map(|path| -> Result<SourceIdentity, GifError> {
                let canonical = fs::canonicalize(path)?;
                let file = File::open(&canonical)?;
                if !file.metadata()?.is_file() {
                    return Err(GifError::InvalidFrame);
                }
                Ok(SourceIdentity { canonical, file })
            })
            .transpose()?;
        let target = Self {
            path: parent.join(leaf),
            parent,
            parent_identity,
            source,
            policy,
        };
        target.check()?;
        Ok(target)
    }
    fn check(&self) -> Result<(), GifError> {
        if fs::canonicalize(&self.parent)? != self.parent {
            return Err(GifError::UnsafeDestination);
        }
        let parent = fs::metadata(&self.parent)?;
        if !parent.is_dir() || !same_directory(&parent, &self.parent_identity) {
            return Err(GifError::UnsafeDestination);
        }
        if self
            .source
            .as_ref()
            .is_some_and(|source| source.canonical == self.path)
        {
            return Err(GifError::SameFile);
        }
        let entry = match fs::symlink_metadata(&self.path) {
            Ok(entry) => entry,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        // Read identity even for a symlink so an alias of the source gets the
        // explicit same-file error. Never follow the leaf for the final write.
        if let Some(source) = &self.source {
            if fs::canonicalize(&self.path).is_ok_and(|path| path == source.canonical) {
                return Err(GifError::SameFile);
            }
            if let Ok(destination) = fs::metadata(&self.path)
                && same_file(&source.file.metadata()?, &destination)?
            {
                return Err(GifError::SameFile);
            }
        }
        if entry.file_type().is_symlink() || !entry.is_file() {
            return Err(GifError::UnsafeDestination);
        }
        if self.policy == ReplacePolicy::RefuseExisting {
            return Err(GifError::DestinationExists);
        }
        Ok(())
    }
    pub fn stage(&self) -> Result<Stage, GifError> {
        ensure_filesystem_supported()?;
        let path = self
            .parent
            .join(format!(".BelloBox-export-{}.gif", uuid::Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        Ok(Stage { path, file })
    }
    pub fn publish(&self, stage: &Stage, control: &ExportControl) -> Result<PathBuf, GifError> {
        control.publish(|| {
            self.check()?;
            if !stage.is_owned()? {
                return Err(GifError::StageIdentityChanged);
            }
            match self.policy {
                ReplacePolicy::RefuseExisting => {
                    // Unlike exists()+rename(), hard_link is an atomic no-clobber
                    // operation on Unix and Windows. Never fall back to overwrite.
                    fs::hard_link(&stage.path, &self.path).map_err(|error| {
                        if error.kind() == std::io::ErrorKind::AlreadyExists {
                            GifError::DestinationExists
                        } else {
                            error.into()
                        }
                    })?;
                }
                ReplacePolicy::ReplaceExistingFile => fs::rename(&stage.path, &self.path)?,
            }
            Ok(self.path.clone())
        })
    }
}

pub(super) struct Stage {
    pub path: PathBuf,
    pub file: File,
}
impl Stage {
    fn is_owned(&self) -> Result<bool, GifError> {
        let entry = match fs::symlink_metadata(&self.path) {
            Ok(entry) => entry,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !entry.is_file() || entry.file_type().is_symlink() {
            return Ok(false);
        }
        same_file(&entry, &self.file.metadata()?)
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        // Never knowingly unlink a replacement belonging to another writer.
        // This check is not a hostile-directory TOCTOU security boundary.
        if self.is_owned().unwrap_or(false) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn same_file(a: &Metadata, b: &Metadata) -> Result<bool, GifError> {
    use std::os::unix::fs::MetadataExt;
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}
#[cfg(not(unix))]
fn same_file(_: &Metadata, _: &Metadata) -> Result<bool, GifError> {
    // Fail closed until a native stable file-ID adapter exists. Path equality
    // alone is insufficient to reject hard links on these platforms.
    Err(GifError::UnsupportedFileIdentity)
}
#[cfg(unix)]
fn same_directory(a: &Metadata, b: &Metadata) -> bool {
    same_file(a, b).unwrap_or(false)
}
#[cfg(not(unix))]
fn same_directory(_: &Metadata, _: &Metadata) -> bool {
    false
}
