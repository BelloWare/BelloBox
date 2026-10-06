//! Portable contracts for explicit local movie decoding on a caller-owned worker.
//!
//! Reconstructed after the previous execution workspace was lost. The actual
//! typed macOS implementation is compiled/exercised under cfg(test) for native
//! CI. Production entry points remain Unavailable on every platform, before I/O.
//! No application route or native capability is enabled by this checkpoint.
//! This is not screen recording, an importer UI or GIF decoding. Intended frames
//! feed the existing Rust GIF encoder. No blanket hardware acceleration guarantee.
#[cfg(all(target_os = "macos", test))]
mod macos;
#[cfg(all(test, unix))]
mod source_file;
#[cfg(test)]
mod tests;

use std::{
    fmt,
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// Production availability is deliberately disabled pending native CI evidence.
/// macOS unit tests compile and exercise the actual implementation below.
pub const NATIVE_MOVIE_READER_IMPLEMENTED: bool = false;
pub const MAX_MOVIE_FRAME_PIXELS: u64 = 8_388_608;
pub const MAX_MOVIE_FRAME_DIMENSION: u32 = 16_384;
pub const MAX_PIXEL_BUFFER_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_MOVIE_SAMPLES: usize = 30_000;
pub const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Cross-thread cancellation contains no native object or callback. A future
/// native reader observes it between serialized calls; it cannot forcibly
/// interrupt copyNextSampleBuffer, nor call cancelReading concurrently with it.
#[derive(Clone, Debug, Default)]
pub struct MovieCancellation(Arc<AtomicBool>);
impl MovieCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    #[cfg(test)]
    fn check(&self) -> MovieResult<()> {
        if self.is_cancelled() {
            Err(MovieError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovieInfo {
    pub duration: f64,
    /// Exact preferred-transform bounding dimensions; frame dimensions round them.
    pub display_width: f64,
    pub display_height: f64,
    pub nominal_frame_rate: f64,
}

/// Bounded source-time range from a core GIF plan. Decoder tolerance is one output
/// frame. Sampling, final trim cut and GIF delay allocation remain core-owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovieReadRange {
    start: f64,
    end: f64,
    frame_interval: f64,
}
impl MovieReadRange {
    pub fn new(start: f64, end: f64, frame_interval: f64) -> MovieResult<Self> {
        if !start.is_finite()
            || !end.is_finite()
            || !frame_interval.is_finite()
            || start < 0.
            || end - start < 0.05
            || end - start > 120.000_001
            || !(0.05..=0.2).contains(&frame_interval)
        {
            return Err(MovieError::InvalidRange);
        }
        Ok(Self {
            start,
            end,
            frame_interval,
        })
    }
    pub fn start(&self) -> f64 {
        self.start
    }
    pub fn end(&self) -> f64 {
        self.end
    }
    pub fn decode_duration(&self) -> f64 {
        self.end - self.start + self.frame_interval
    }
}

/// Owned tightly packed, top-left/y-down opaque RGBA. Debug excludes pixels;
/// into_parts moves the allocation into a caller's core frame without a copy.
pub struct MovieFrame {
    presentation_seconds: f64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl MovieFrame {
    pub fn presentation_seconds(&self) -> f64 {
        self.presentation_seconds
    }
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn into_parts(self) -> (f64, u32, u32, Vec<u8>) {
        (
            self.presentation_seconds,
            self.width,
            self.height,
            self.rgba,
        )
    }
    #[cfg(test)]
    fn new(presentation_seconds: f64, width: u32, height: u32, rgba: Vec<u8>) -> MovieResult<Self> {
        let bytes = frame_bytes(width, height)?;
        if !presentation_seconds.is_finite() || presentation_seconds < 0. {
            return Err(MovieError::InvalidTimestamp);
        }
        if bytes != rgba.len() {
            return Err(MovieError::InvalidFrame);
        }
        Ok(Self {
            presentation_seconds,
            width,
            height,
            rgba,
        })
    }
}
impl fmt::Debug for MovieFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MovieFrame")
            .field("presentation_seconds", &self.presentation_seconds)
            .field("size", &self.size())
            .field("rgba_bytes", &self.rgba.len())
            .finish()
    }
}

pub struct MovieAsset {
    #[cfg(all(target_os = "macos", test))]
    native: macos::Asset,
    info: MovieInfo,
    source_path: PathBuf,
    cancellation: MovieCancellation,
    _thread_bound: PhantomData<Rc<()>>,
}
impl MovieAsset {
    /// Reserved explicit local-file action. Production currently fails before
    /// any path I/O on every platform. macOS cfg(test) exercises the real decoder
    /// with synthetic fixtures; no UI, capture or permission prompt is involved.
    pub fn open(path: &Path, cancellation: MovieCancellation) -> MovieResult<Self> {
        #[cfg(all(target_os = "macos", test))]
        {
            cancellation.check()?;
            let native = macos::Asset::open(path, cancellation.clone())?;
            Ok(Self {
                info: native.info(),
                source_path: native.source_path().to_owned(),
                cancellation,
                native,
                _thread_bound: PhantomData,
            })
        }
        #[cfg(not(all(target_os = "macos", test)))]
        {
            let _ = (path, cancellation);
            Err(MovieError::Unavailable)
        }
    }
    pub fn info(&self) -> MovieInfo {
        self.info
    }
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }
    pub fn cancellation(&self) -> MovieCancellation {
        self.cancellation.clone()
    }
    pub fn reader(self, range: MovieReadRange) -> MovieResult<MovieReader> {
        #[cfg(all(target_os = "macos", test))]
        {
            self.cancellation.check()?;
            if range.start >= self.info.duration || range.end > self.info.duration + 0.001 {
                return Err(MovieError::InvalidRange);
            }
            let native = self.native.reader(range)?;
            Ok(MovieReader {
                native,
                info: self.info,
                source_path: self.source_path,
                cancellation: self.cancellation,
                _thread_bound: PhantomData,
            })
        }
        #[cfg(not(all(target_os = "macos", test)))]
        {
            let _ = range;
            Err(MovieError::Unavailable)
        }
    }
}
pub struct MovieReader {
    #[cfg(all(target_os = "macos", test))]
    native: macos::Reader,
    info: MovieInfo,
    source_path: PathBuf,
    cancellation: MovieCancellation,
    _thread_bound: PhantomData<Rc<()>>,
}
impl MovieReader {
    pub fn info(&self) -> MovieInfo {
        self.info
    }
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }
    pub fn cancellation(&self) -> MovieCancellation {
        self.cancellation.clone()
    }
    /// Probe a caller-owned publication cancellation fence before/after native
    /// calls. The callback runs only on this reader's worker. Cancellation does
    /// not interrupt an in-flight synchronous native sample read.
    pub fn next_frame(
        &mut self,
        should_cancel: impl Fn() -> bool,
    ) -> MovieResult<Option<MovieFrame>> {
        #[cfg(all(target_os = "macos", test))]
        {
            self.native.next_frame(&should_cancel)
        }
        #[cfg(not(all(target_os = "macos", test)))]
        {
            let _ = should_cancel;
            Err(MovieError::Unavailable)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovieError {
    Unavailable,
    Busy,
    Cancelled,
    TimedOut,
    InvalidSource,
    SourceChanged,
    InvalidRange,
    InvalidMetadata,
    InvalidTimestamp,
    InvalidFrame,
    LimitExceeded,
    NoVideoTrack,
    ProtectedContent,
    MetadataFailed,
    DecodeFailed,
    NativeFailure,
    Io,
}
pub type MovieResult<T> = Result<T, MovieError>;
impl fmt::Display for MovieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Native local movie decoding is unavailable on this platform.",
            Self::Busy => "Another native movie job is still active.",
            Self::Cancelled => "Movie decoding was cancelled.",
            Self::TimedOut => "Movie metadata did not finish loading within the limit.",
            Self::InvalidSource => "Choose an existing local regular movie file.",
            Self::SourceChanged => "The movie source changed while it was being read.",
            Self::InvalidRange => "The movie read range is invalid or exceeds the GIF limits.",
            Self::InvalidMetadata => "The movie has invalid or unsupported display metadata.",
            Self::InvalidTimestamp => {
                "The movie has invalid or out-of-order presentation timestamps."
            }
            Self::InvalidFrame => "The movie decoder returned invalid frame storage.",
            Self::LimitExceeded => "The movie exceeds a bounded frame, buffer or sample limit.",
            Self::NoVideoTrack => "The movie has no video track.",
            Self::ProtectedContent => "Protected movie content cannot be decoded.",
            Self::MetadataFailed => "Movie metadata could not be loaded.",
            Self::DecodeFailed => "The movie decoder failed before reaching the end of its range.",
            Self::NativeFailure => "The native movie framework could not complete the operation.",
            Self::Io => "The local movie file could not be read.",
        })
    }
}
impl std::error::Error for MovieError {}

#[cfg(test)]
fn frame_bytes(width: u32, height: u32) -> MovieResult<usize> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(MovieError::LimitExceeded)?;
    if width == 0
        || height == 0
        || width > MAX_MOVIE_FRAME_DIMENSION
        || height > MAX_MOVIE_FRAME_DIMENSION
        || pixels > MAX_MOVIE_FRAME_PIXELS
    {
        return Err(MovieError::LimitExceeded);
    }
    usize::try_from(pixels.checked_mul(4).ok_or(MovieError::LimitExceeded)?)
        .map_err(|_| MovieError::LimitExceeded)
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct DisplayGeometry {
    min_x: f64,
    min_y: f64,
    width: u32,
    height: u32,
    extent_width: f64,
    extent_height: f64,
}
#[cfg(test)]
fn display_geometry(
    natural_width: f64,
    natural_height: f64,
    transform: [f64; 6],
) -> MovieResult<DisplayGeometry> {
    if !natural_width.is_finite()
        || !natural_height.is_finite()
        || natural_width < 1.
        || natural_height < 1.
        || !transform.iter().all(|v| v.is_finite())
    {
        return Err(MovieError::InvalidMetadata);
    }
    if natural_width > f64::from(MAX_MOVIE_FRAME_DIMENSION)
        || natural_height > f64::from(MAX_MOVIE_FRAME_DIMENSION)
    {
        return Err(MovieError::LimitExceeded);
    }
    frame_bytes(natural_width.round() as u32, natural_height.round() as u32)?;
    let [a, b, c, d, tx, ty] = transform;
    let determinant = a * d - b * c;
    if !determinant.is_finite() || determinant.abs() < 1e-12 {
        return Err(MovieError::InvalidMetadata);
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (x, y) in [
        (0., 0.),
        (natural_width, 0.),
        (0., natural_height),
        (natural_width, natural_height),
    ] {
        let (x, y) = (a * x + c * y + tx, b * x + d * y + ty);
        if !x.is_finite() || !y.is_finite() {
            return Err(MovieError::InvalidMetadata);
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let (extent_width, extent_height) = (max_x - min_x, max_y - min_y);
    if !extent_width.is_finite()
        || !extent_height.is_finite()
        || extent_width < 1.
        || extent_height < 1.
        || extent_width > f64::from(MAX_MOVIE_FRAME_DIMENSION)
        || extent_height > f64::from(MAX_MOVIE_FRAME_DIMENSION)
    {
        return Err(MovieError::LimitExceeded);
    }
    let (width, height) = (extent_width.round() as u32, extent_height.round() as u32);
    frame_bytes(width, height)?;
    Ok(DisplayGeometry {
        min_x,
        min_y,
        width,
        height,
        extent_width,
        extent_height,
    })
}
#[cfg(test)]
fn validate_pixel_storage(
    width: u32,
    height: u32,
    stride: usize,
    data_size: usize,
) -> MovieResult<()> {
    frame_bytes(width, height)?;
    let row = (width as usize)
        .checked_mul(4)
        .ok_or(MovieError::LimitExceeded)?;
    // CG storage requires the complete final padded row, not just visible pixels.
    let extent = stride
        .checked_mul(height as usize)
        .ok_or(MovieError::LimitExceeded)?;
    if stride < row || extent > data_size {
        return Err(MovieError::InvalidFrame);
    }
    if data_size > MAX_PIXEL_BUFFER_BYTES || extent > MAX_PIXEL_BUFFER_BYTES {
        return Err(MovieError::LimitExceeded);
    }
    Ok(())
}
