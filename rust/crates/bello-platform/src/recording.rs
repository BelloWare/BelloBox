//! Bounded silent movie ownership. This is not screen capture or audio recording.
//!
//! Ordinary admission remains closed before any I/O. Native AV objects stay on
//! one worker; control handles contain only Rust state. Finite generated fixtures
//! exercise the same compiled writer without granting arbitrary-input admission.
// Dormant internals remain compiled for the closed production admission gate.
#[allow(dead_code)]
mod control;
#[cfg(any(test, feature = "recording-fixtures"))]
pub mod fixture;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
mod macos;
#[allow(dead_code)]
mod output;
#[cfg(test)]
mod tests;
#[allow(dead_code)]
mod validation;

use crate::movie::{MovieInfo, SelectedMovie};
pub use control::{CancelOutcome, RecordingControl};
pub use output::SaveControl;
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

pub const NATIVE_RECORDING_IMPLEMENTED: bool = false;
pub const MAX_RECORDING_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_RECORDING_SECONDS: u32 = 120;
pub const MAX_RECORDING_FRAMES: usize = 3_600;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingError {
    Unavailable,
    Busy,
    Cancelled,
    TimedOut,
    InvalidFrame,
    InvalidTimestamp,
    LimitExceeded,
    NoFrames,
    NativeFailure,
    Io,
    AlreadyExists,
    SourceChanged,
}
pub type RecordingResult<T> = Result<T, RecordingError>;
impl fmt::Display for RecordingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Native recording is unavailable; no capture was started.",
            Self::Busy => "A recording worker is still physically active.",
            Self::Cancelled => "Recording was cancelled.",
            Self::TimedOut => "The movie writer exceeded its completion limit.",
            Self::InvalidFrame => "The recording frame has invalid dimensions or pixel storage.",
            Self::InvalidTimestamp => "The recording frame has invalid or out-of-order timing.",
            Self::LimitExceeded => "The recording exceeds its bounded frame, time or file limit.",
            Self::NoFrames => "The recording received no video frames.",
            Self::NativeFailure => "The native silent movie writer could not finish.",
            Self::Io => "The recording file could not be read or saved.",
            Self::AlreadyExists => "The destination already exists; no file was replaced.",
            Self::SourceChanged => "The completed recording changed while it was being used.",
        })
    }
}
impl std::error::Error for RecordingError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingPublication {
    Published,
    RecoveredCapture,
}

/// The immutable identity selected from the writer's exact retained descriptor.
/// Dropping ordinary review ownership never deletes completed/recoverable media.
#[derive(Clone, Debug)]
pub struct FinalizedRecording {
    selected: SelectedMovie,
    info: MovieInfo,
    byte_len: u64,
    publication: RecordingPublication,
    #[cfg(any(test, feature = "recording-fixtures"))]
    known_frames: bool,
}
impl FinalizedRecording {
    pub fn selected_movie(&self) -> SelectedMovie {
        self.selected.clone()
    }
    pub fn path(&self) -> &Path {
        self.selected.path()
    }
    pub fn info(&self) -> MovieInfo {
        self.info
    }
    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }
    pub fn publication(&self) -> RecordingPublication {
        self.publication
    }
    pub fn recovery_warning(&self) -> Option<&'static str> {
        (self.publication == RecordingPublication::RecoveredCapture).then_some(
            "The completed movie was preserved because final publication failed. Use Save As to keep a copy.")
    }
    pub fn verify(&self) -> RecordingResult<()> {
        self.selected
            .verify()
            .map_err(|_| RecordingError::SourceChanged)
    }
    /// Worker-only, bounded descriptor copy. Staging is on the destination volume.
    /// Cancellation and no-replace publication are linearized by SaveControl.
    pub fn save_copy(&self, destination: &Path, control: &SaveControl) -> RecordingResult<PathBuf> {
        output::save_copy(self, destination, control)
    }
}

#[derive(Debug)]
pub enum RecordingEvent {
    Started,
    Finalized(FinalizedRecording),
    Failed(RecordingError),
    Cancelled,
}

/// Control-only owner: generated admission never exposes an arbitrary RGBA producer.
pub struct RecordingHandle {
    control: RecordingControl,
}
impl RecordingHandle {
    /// Deliberately zero-action on every platform, including fixture-feature builds.
    pub fn start() -> RecordingResult<Self> {
        Err(RecordingError::Unavailable)
    }
    /// Retain an app retirement ticket until the native worker's Admission
    /// retires, including callback copies and terminal payload disposal. If it
    /// already drained, release immediately. This does not extend event ownership.
    pub fn retain_until_drained(&self, owner: impl Send + 'static) {
        self.control.retain_until_drained(owner);
    }
    pub fn control(&self) -> RecordingControl {
        self.control.clone()
    }
    pub fn stop(&self) {
        self.control.stop();
    }
    pub fn cancel(&self) -> CancelOutcome {
        self.control.cancel()
    }
    pub fn is_drained(&self) -> bool {
        self.control.is_drained()
    }
    pub fn try_event(&self) -> Option<RecordingEvent> {
        self.control.try_event()
    }
}
impl fmt::Debug for RecordingHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordingHandle")
            .field("drained", &self.is_drained())
            .finish_non_exhaustive()
    }
}
impl Drop for RecordingHandle {
    fn drop(&mut self) {
        self.control.owner_dropped();
    }
}

/// Private fields mean only completed output transactions can mint this proof.
/// The movie module consumes the exact File, never reopens a pathname.
pub(crate) struct FinalizationProof {
    path: PathBuf,
    file: std::fs::File,
    lifetime: Arc<RecordingLifetime>,
    generated_native: bool,
}
impl FinalizationProof {
    pub(crate) fn into_parts(self) -> (PathBuf, std::fs::File, Arc<RecordingLifetime>, bool) {
        (self.path, self.file, self.lifetime, self.generated_native)
    }
}

/// Separate generated-file lifetime. Ordinary completed output is never removed.
#[derive(Debug)]
pub(crate) struct RecordingLifetime {
    directory: PathBuf,
    remove_on_drop: AtomicBool,
    #[cfg(test)]
    drop_thread: std::sync::Mutex<Option<std::sync::mpsc::Sender<std::thread::ThreadId>>>,
}
impl Drop for RecordingLifetime {
    fn drop(&mut self) {
        #[cfg(test)]
        if let Some(sender) = self
            .drop_thread
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = sender.send(std::thread::current().id());
        }
        if self
            .remove_on_drop
            .load(std::sync::atomic::Ordering::Acquire)
        {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}
