use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Default)]
pub struct SaveControl(Arc<super::control::Publication>);
impl SaveControl {
    pub fn cancel(&self) -> CancelOutcome {
        self.0.cancel()
    }
    fn check(&self) -> RecordingResult<()> {
        self.0.check()
    }
}

pub(super) fn private_directory(parent: &Path) -> RecordingResult<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RecordingError::Io)?
        .as_nanos();
    let name = format!(
        ".BelloBox-recording-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let path = parent.join(name);
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&path).map_err(|_| RecordingError::Io)?;
    // Use one canonical spelling across macOS /var and /private/var.
    fs::canonicalize(path).map_err(|_| RecordingError::Io)
}
pub(super) fn new_file(path: &Path) -> RecordingResult<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| RecordingError::Io)
}

pub(super) struct OutputTransaction {
    lifetime: Arc<RecordingLifetime>,
    pub capture: PathBuf,
    pub destination: PathBuf,
    completed: bool,
    #[cfg(test)]
    pub before_claim: Option<Box<dyn FnOnce() + Send>>,
    #[cfg(test)]
    pub after_claim: Option<Box<dyn FnOnce() + Send>>,
}
impl OutputTransaction {
    pub fn new(fixture: bool) -> RecordingResult<Self> {
        let directory = private_directory(&std::env::temp_dir())?;
        Ok(Self {
            capture: directory.join("capture.mov"),
            destination: directory.join("recording.mov"),
            lifetime: Arc::new(RecordingLifetime {
                directory,
                remove_on_drop: AtomicBool::new(fixture),
                #[cfg(test)]
                drop_thread: std::sync::Mutex::new(None),
            }),
            completed: false,
            #[cfg(test)]
            before_claim: None,
            #[cfg(test)]
            after_claim: None,
        })
    }
    #[cfg(test)]
    pub fn observe_drop(&self, sender: std::sync::mpsc::Sender<std::thread::ThreadId>) {
        *self
            .lifetime
            .drop_thread
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(sender);
    }
    /// Call only after native objects, their pool and all copied callbacks drain.
    pub fn finish(
        mut self,
        info: MovieInfo,
        generated_native: bool,
        known_frames: bool,
        control: &RecordingControl,
    ) -> RecordingResult<FinalizedRecording> {
        control.check()?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
        }
        let file = options
            .open(&self.capture)
            .map_err(|_| RecordingError::Io)?;
        let metadata = file.metadata().map_err(|_| RecordingError::Io)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(RecordingError::Io);
        }
        if metadata.len() > MAX_RECORDING_FILE_BYTES {
            return Err(RecordingError::LimitExceeded);
        }
        // File preparation and fsync precede the atomic publication claim. Even
        // failed durability/permission preparation preserves the usable capture
        // inside its owner-only directory and exposes the recovery warning.
        let mut prepared = file.sync_all().is_ok();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            prepared &= file
                .set_permissions(fs::Permissions::from_mode(0o600))
                .is_ok();
        }
        // This CAS is publication-start linearization; UI never waits on I/O.
        #[cfg(test)]
        if let Some(hook) = self.before_claim.take() {
            hook();
        }
        control.claim_publication()?;
        #[cfg(test)]
        if let Some(hook) = self.after_claim.take() {
            hook();
        }
        // Protect the already playable capture even if the following bridge or
        // publication fails. No completed ordinary movie is removed by Drop.
        self.completed = true;
        // Preserve completed fixtures too; explicit test/QA cleanup is separate.
        self.lifetime.remove_on_drop.store(false, Ordering::Release);
        let (path, publication) =
            if prepared && fs::hard_link(&self.capture, &self.destination).is_ok() {
                // Unlink before recording ctime; our own publication must not look
                // like later source mutation. A failed unlink preserves both names.
                let _ = fs::remove_file(&self.capture);
                (self.destination.clone(), RecordingPublication::Published)
            } else {
                (self.capture.clone(), RecordingPublication::RecoveredCapture)
            };
        let proof = FinalizationProof {
            path,
            file,
            lifetime: self.lifetime.clone(),
            generated_native,
        };
        let selected =
            SelectedMovie::from_recording(proof).map_err(|_| RecordingError::SourceChanged)?;
        control.published();
        let _ = known_frames;
        Ok(FinalizedRecording {
            selected,
            info,
            byte_len: metadata.len(),
            publication,
            #[cfg(any(test, feature = "recording-fixtures"))]
            known_frames,
        })
    }
}
impl Drop for OutputTransaction {
    fn drop(&mut self) {
        if !self.completed {
            let _ = fs::remove_dir_all(&self.lifetime.directory);
        }
    }
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn save_copy(
    movie: &FinalizedRecording,
    destination: &Path,
    control: &SaveControl,
) -> RecordingResult<PathBuf> {
    control.check()?;
    movie.verify()?;
    if !destination
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mov"))
    {
        return Err(RecordingError::Io);
    }
    let name = destination.file_name().ok_or(RecordingError::Io)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|_| RecordingError::Io)?;
    let destination = parent.join(name);
    // Symlinks (including dangling links), directories and source aliases fail
    // the same final atomic no-replace link, without touching the old entry.
    let stage = Stage(private_directory(&parent)?);
    let staged = stage.0.join("complete.mov");
    let mut output = new_file(&staged)?;
    movie
        .selected
        .copy_recording_to(&mut output, || control.check().is_err())
        .map_err(|error| match error {
            crate::movie::MovieError::Cancelled => RecordingError::Cancelled,
            crate::movie::MovieError::SourceChanged => RecordingError::SourceChanged,
            _ => RecordingError::Io,
        })?;
    output
        .flush()
        .and_then(|_| output.sync_all())
        .map_err(|_| RecordingError::Io)?;
    movie.verify()?;
    control.0.claim()?;
    fs::hard_link(&staged, &destination).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            RecordingError::AlreadyExists
        } else {
            RecordingError::Io
        }
    })?;
    // No error is reported after publication; cancellation after this point does
    // not revoke an already completed Save. Stage removal does not affect source.
    control.0.published();
    Ok(destination)
}
