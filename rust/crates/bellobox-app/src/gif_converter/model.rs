//! Source-shaped converter lifecycle. Worker results are generation fenced; an
//! admitted export remains busy until its worker settles, including cancellation.
use bello_platform::movie::{
    MovieAsset, MovieCancellation, MovieInfo, MovieReadRange, MovieReader,
};
use bellobox_core::recording::gif::{
    self, ExportControl, GifError, GifExportOptions, GifExportPlan, GifExportResult, GifSourceInfo,
    ReplacePolicy, SequentialFrameSource,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Source {
    Movie(PathBuf),
    #[cfg(any(test, debug_assertions))]
    Synthetic,
}
impl Source {
    pub fn name(&self) -> String {
        match self {
            Self::Movie(path) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            #[cfg(any(test, debug_assertions))]
            Self::Synthetic => "Synthetic color movie (no input file)".into(),
        }
    }
    pub fn inspect(&self, cancellation: MovieCancellation) -> Result<GifSourceInfo, String> {
        match self {
            Self::Movie(path) => MovieAsset::open(path, cancellation)
                .map(|asset| info(asset.info()))
                .map_err(|e| e.to_string()),
            #[cfg(any(test, debug_assertions))]
            Self::Synthetic => Ok(GifSourceInfo {
                duration: 3.,
                display_width: 320.,
                display_height: 180.,
                nominal_frame_rate: 20.,
            }),
        }
    }
}
fn info(value: MovieInfo) -> GifSourceInfo {
    GifSourceInfo {
        duration: value.duration,
        display_width: value.display_width,
        display_height: value.display_height,
        nominal_frame_rate: value.nominal_frame_rate,
    }
}
#[derive(Default)]
pub(super) struct Model {
    pub source: Option<Source>,
    pub info: Option<GifSourceInfo>,
    pub options: GifExportOptions,
    pub result: Option<GifExportResult>,
    pub result_revision: u64,
    pub result_loops: bool,
    pub status: String,
    pub error: bool,
    pub generation: u64,
    pub loading: bool,
    pub dialog: bool,
    pub job: Option<Job>,
    metadata_cancel: MovieCancellation,
    closed: bool,
}
pub(super) struct Job {
    pub control: ExportControl,
    pub movie_cancel: MovieCancellation,
    pub progress: Arc<AtomicUsize>,
    pub planned: usize,
    loops: bool,
}
pub(super) struct Work {
    pub generation: u64,
    source: Source,
    source_info: GifSourceInfo,
    plan: GifExportPlan,
    destination: PathBuf,
    replace: ReplacePolicy,
    control: ExportControl,
    movie_cancel: MovieCancellation,
    progress: Arc<AtomicUsize>,
}
impl Model {
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.dialog
    }
    pub fn plan(&self) -> Option<GifExportPlan> {
        GifExportPlan::make(self.info?, &self.options).ok()
    }
    pub fn load(&mut self, source: Source) -> Option<(u64, MovieCancellation)> {
        if self.busy() || self.closed {
            return None;
        }
        self.metadata_cancel.cancel();
        self.metadata_cancel = MovieCancellation::default();
        self.generation += 1;
        self.source = Some(source);
        self.info = None;
        self.result = None;
        self.options.trim_start = 0.;
        self.options.trim_end = None;
        self.status = "Reading movie metadata…".into();
        self.error = false;
        self.loading = true;
        Some((self.generation, self.metadata_cancel.clone()))
    }
    pub fn loaded(&mut self, generation: u64, result: Result<GifSourceInfo, String>) {
        if self.closed || generation != self.generation || !self.loading {
            return;
        }
        self.loading = false;
        match result {
            Ok(info) => {
                self.options.trim_end = Some(info.duration.min(gif::MAX_DURATION_SECONDS));
                self.info = Some(info);
                self.status = "Ready. The source is kept after conversion.".into();
            }
            Err(error) => {
                self.status = error;
                self.error = true;
            }
        }
    }
    pub fn begin(&mut self, destination: PathBuf, replace: ReplacePolicy) -> Option<Work> {
        if self.busy() || self.closed {
            return None;
        }
        let source = self.source.clone()?;
        let source_info = self.info?;
        let plan = self.plan()?;
        self.generation += 1;
        let control = ExportControl::default();
        let movie_cancel = MovieCancellation::default();
        let progress = Arc::new(AtomicUsize::new(0));
        self.job = Some(Job {
            control: control.clone(),
            movie_cancel: movie_cancel.clone(),
            progress: progress.clone(),
            planned: plan.frame_count(),
            loops: plan.loops(),
        });
        self.status = "Encoding GIF locally…".into();
        self.error = false;
        Some(Work {
            generation: self.generation,
            source,
            source_info,
            plan,
            destination,
            replace,
            control,
            movie_cancel,
            progress,
        })
    }
    pub fn finished(&mut self, generation: u64, result: Result<GifExportResult, GifError>) {
        if self.closed || generation != self.generation || self.job.is_none() {
            return;
        }
        let loops = self.job.take().is_some_and(|job| job.loops);
        match result {
            Ok(result) => {
                self.result_revision += 1;
                self.result_loops = loops;
                self.status = format!("Saved {} · {} bytes", result.path.file_name().unwrap_or_default().to_string_lossy(), result.file_size);
                self.result = Some(result);
            }
            Err(GifError::Cancelled) => self.status = "Conversion cancelled before publication. The source and any previous GIF are kept.".into(),
            Err(error) => { self.status = error.to_string(); self.error = true; }
        }
    }
    pub fn cancel(&mut self) {
        if let Some(job) = &self.job {
            job.movie_cancel.cancel();
            match job.control.cancel() {
                Ok(gif::CancelOutcome::AlreadyPublished) => {
                    self.status = "GIF was already saved; finishing…".into()
                }
                Ok(_) => self.status = "Cancelling conversion…".into(),
                Err(error) => {
                    self.status = error.to_string();
                    self.error = true;
                }
            }
        }
    }
    pub fn close(&mut self) {
        self.cancel();
        self.metadata_cancel.cancel();
        self.closed = true;
        self.generation += 1;
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        self.close();
    }
}
impl Work {
    pub fn run(self) -> Result<GifExportResult, GifError> {
        self.control.check_active()?;
        let progress = |written, _| {
            self.progress.store(written, Ordering::Release);
        };
        match self.source {
            Source::Movie(path) => {
                let asset = MovieAsset::open(&path, self.movie_cancel).map_err(movie_error)?;
                if info(asset.info()) != self.source_info {
                    return Err(GifError::Source(
                        "Movie metadata changed; choose it again.".into(),
                    ));
                }
                let range = MovieReadRange::new(
                    self.plan.start(),
                    self.plan.end(),
                    self.plan.frame_delay(),
                )
                .map_err(movie_error)?;
                let mut reader = NativeFrames(asset.reader(range).map_err(movie_error)?);
                gif::export_gif(
                    &mut reader,
                    &self.plan,
                    Some(&path),
                    &self.destination,
                    self.replace,
                    &self.control,
                    progress,
                )
            }
            #[cfg(any(test, debug_assertions))]
            Source::Synthetic => gif::export_gif(
                &mut SyntheticFrames(0),
                &self.plan,
                None,
                &self.destination,
                self.replace,
                &self.control,
                progress,
            ),
        }
    }
}
fn movie_error(error: bello_platform::movie::MovieError) -> GifError {
    if error == bello_platform::movie::MovieError::Cancelled {
        GifError::Cancelled
    } else {
        GifError::Source(error.to_string())
    }
}
struct NativeFrames(MovieReader);
impl SequentialFrameSource for NativeFrames {
    fn next_frame(
        &mut self,
        control: &ExportControl,
    ) -> Result<Option<gif::DisplayRgbaFrame>, GifError> {
        self.0
            .next_frame(|| control.check_active().is_err())
            .map_err(movie_error)?
            .map(|frame| {
                let (pts, width, height, pixels) = frame.into_parts();
                gif::DisplayRgbaFrame::new(pts, width, height, pixels)
            })
            .transpose()
    }
}
#[cfg(any(test, debug_assertions))]
struct SyntheticFrames(usize);
#[cfg(any(test, debug_assertions))]
impl SequentialFrameSource for SyntheticFrames {
    fn next_frame(
        &mut self,
        control: &ExportControl,
    ) -> Result<Option<gif::DisplayRgbaFrame>, GifError> {
        control.check_active()?;
        if self.0 >= 60 {
            return Ok(None);
        }
        let index = self.0;
        self.0 += 1;
        let mut pixels = vec![0; 320 * 180 * 4];
        for (position, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = position % 320;
            let y = position / 320;
            pixel.copy_from_slice(&[
                if x < index * 320 / 60 { 230 } else { 40 },
                if y < 90 { 130 } else { 60 },
                180,
                255,
            ]);
        }
        gif::DisplayRgbaFrame::new(index as f64 / 20., 320, 180, pixels).map(Some)
    }
}

/// Exact paired-slider coupling from GIFTrimControls, including its 0.1s UI minimum.
pub(super) fn move_trim(
    options: &mut GifExportOptions,
    duration: f64,
    start_handle: bool,
    value: f64,
) {
    if !duration.is_finite() || !value.is_finite() {
        return;
    }
    let total = duration.max(0.1);
    if start_handle {
        let current_end = options.trim_end.unwrap_or(total).min(total);
        let length = (current_end - options.trim_start).max(0.1);
        let start = value.clamp(0., total - 0.1);
        let mut end = current_end;
        if end < start + 0.1 {
            end = total.min(start + length);
        }
        if end - start > 120. {
            end = start + 120.;
        }
        options.trim_start = start;
        options.trim_end = Some(end);
    } else {
        let end = value.clamp(0.1, total);
        let mut start = options.trim_start;
        if start > end - 0.1 {
            start = (end - 0.1).max(0.);
        }
        if end - start > 120. {
            start = end - 120.;
        }
        options.trim_start = start;
        options.trim_end = Some(end);
    }
}
pub(super) fn open_preview(result: &GifExportResult) -> Result<gif::GifPreview, GifError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(&result.path)?;
        gif::GifPreview::open(file, result)
    }
    #[cfg(not(unix))]
    {
        let _ = result;
        Err(GifError::UnsupportedFilesystem)
    }
}
