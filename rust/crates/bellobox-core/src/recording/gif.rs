//! Bounded GIF planning and sequential encoding of injected display-oriented RGBA.
//!
//! Planning has no side effects. [`export_gif`] explicitly writes a private staged
//! sibling and publishes only after complete readback. No capture, movie decoding,
//! provider, network, subprocess, or ffmpeg implementation is provided here.
mod gif_decode;
#[cfg(test)]
mod gif_decode_tests;
mod gif_encode;
mod gif_file;
mod gif_preview;
mod gif_structure;
#[cfg(test)]
mod gif_tests;

pub use gif_encode::export_gif;
pub use gif_file::{CancelOutcome, ExportControl, ExportStatus, ReplacePolicy};
pub use gif_preview::{GifPreview, GifPreviewFrame};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};

pub const MAX_DURATION_SECONDS: f64 = 120.;
pub const MAX_FRAMES: usize = 3_000;
pub const MIN_DELAY_CENTISECONDS: u16 = 2;
pub const MAX_SOURCE_FRAME_PIXELS: u64 = 8_388_608;
pub const MAX_SOURCE_DIMENSION: u32 = 16_384;
pub const MAX_SOURCE_SAMPLES: usize = 30_000;
pub const MAX_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifExportOptions {
    pub frames_per_second: i64,
    /// A longest-edge cap, despite the source option's historical name.
    pub max_width: i64,
    pub loops: bool,
    pub trim_start: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_end: Option<f64>,
}
// Swift uses decodeIfPresent for every key: explicit null, like absence,
// selects the default. A plain serde(default) field does not accept null.
impl<'de> Deserialize<'de> for GifExportOptions {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize, Default)]
        #[serde(default, rename_all = "camelCase")]
        struct Wire {
            frames_per_second: Option<i64>,
            max_width: Option<i64>,
            loops: Option<bool>,
            trim_start: Option<f64>,
            trim_end: Option<f64>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let defaults = Self::default();
        Ok(Self {
            frames_per_second: wire.frames_per_second.unwrap_or(defaults.frames_per_second),
            max_width: wire.max_width.unwrap_or(defaults.max_width),
            loops: wire.loops.unwrap_or(defaults.loops),
            trim_start: wire.trim_start.unwrap_or(defaults.trim_start),
            trim_end: wire.trim_end,
        })
    }
}

impl Default for GifExportOptions {
    fn default() -> Self {
        Self {
            frames_per_second: 15,
            max_width: 640,
            loops: true,
            trim_start: 0.,
            trim_end: None,
        }
    }
}
impl GifExportOptions {
    pub const FRAME_RATE_CHOICES: [i64; 3] = [10, 15, 20];
    pub const WIDTH_CHOICES: [i64; 5] = [320, 480, 640, 800, 1080];

    pub fn normalized(&self) -> Self {
        let trim_start = if self.trim_start.is_finite() {
            self.trim_start.max(0.)
        } else {
            0.
        };
        Self {
            frames_per_second: self.frames_per_second.clamp(5, 20),
            max_width: self.max_width.clamp(64, 1080),
            loops: self.loops,
            trim_start,
            trim_end: self
                .trim_end
                .filter(|end| end.is_finite() && *end > trim_start),
        }
    }
}

/// Adapter-reported dimensions after applying the movie's preferred transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GifSourceInfo {
    pub duration: f64,
    pub display_width: f64,
    pub display_height: f64,
    pub nominal_frame_rate: f64,
}

/// Validated and immutable. Construction is only through [`Self::make`].
#[derive(Clone, Debug, PartialEq)]
pub struct GifExportPlan {
    start: f64,
    end: f64,
    frames_per_second: u16,
    output_width: u16,
    output_height: u16,
    loops: bool,
    source_size: (u32, u32),
    delays: Vec<u16>,
}
impl GifExportPlan {
    pub fn make(source: GifSourceInfo, options: &GifExportOptions) -> Result<Self, GifError> {
        let options = options.normalized();
        if !source.duration.is_finite() || source.duration <= 0. {
            return Err(GifError::NoFrames);
        }
        if !source.display_width.is_finite()
            || !source.display_height.is_finite()
            || source.display_width < 1.
            || source.display_height < 1.
        {
            return Err(GifError::InvalidDimensions);
        }
        let source_width = source.display_width.round();
        let source_height = source.display_height.round();
        if source_width > f64::from(MAX_SOURCE_DIMENSION)
            || source_height > f64::from(MAX_SOURCE_DIMENSION)
            || source_width * source_height > MAX_SOURCE_FRAME_PIXELS as f64
        {
            return Err(GifError::LimitExceeded("source frame dimensions"));
        }
        let start = options.trim_start.min((source.duration - 0.05).max(0.));
        let mut end = options
            .trim_end
            .unwrap_or(source.duration)
            .min(source.duration)
            .min(start + MAX_DURATION_SECONDS);
        let fps = options.frames_per_second as f64;
        if !end.is_finite() || end - start < 0.05 {
            return Err(GifError::ClipTooShort);
        }
        let mut frame_count = (((end - start) * fps - 1e-6).ceil() as usize).max(1);
        if frame_count > MAX_FRAMES {
            frame_count = MAX_FRAMES;
            end = start + frame_count as f64 / fps;
        }
        let scale =
            (options.max_width as f64 / source.display_width.max(source.display_height)).min(1.);
        let width = (source.display_width * scale).round().max(1.) as u16;
        let height = (source.display_height * scale).round().max(1.) as u16;
        Ok(Self {
            start,
            end,
            frames_per_second: options.frames_per_second as u16,
            output_width: width,
            output_height: height,
            loops: options.loops,
            source_size: (source_width as u32, source_height as u32),
            delays: delays(frame_count, options.frames_per_second as u16, end - start),
        })
    }
    pub fn start(&self) -> f64 {
        self.start
    }
    pub fn end(&self) -> f64 {
        self.end
    }
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
    pub fn frames_per_second(&self) -> u16 {
        self.frames_per_second
    }
    pub fn frame_count(&self) -> usize {
        self.delays.len()
    }
    pub fn output_size(&self) -> (u16, u16) {
        (self.output_width, self.output_height)
    }
    pub fn loops(&self) -> bool {
        self.loops
    }
    pub fn delays_centiseconds(&self) -> &[u16] {
        &self.delays
    }
    pub fn encoded_duration(&self) -> f64 {
        self.delays.iter().map(|d| u64::from(*d)).sum::<u64>() as f64 / 100.
    }
    pub fn frame_delay(&self) -> f64 {
        1. / f64::from(self.frames_per_second)
    }
}

fn delays(count: usize, fps: u16, duration: f64) -> Vec<u16> {
    // All callers have a validated <=120s, <=3000-frame plan and 5..=20 fps.
    let minimum = usize::from(MIN_DELAY_CENTISECONDS);
    let total = (count * minimum).max((duration * 100.).round() as usize);
    let mut previous = 0;
    let mut result: Vec<u16> = (1..=count)
        .map(|frame| {
            let next = if frame == count {
                total
            } else {
                total.min((frame as f64 * 100. / f64::from(fps)).round() as usize)
            };
            let delay = (next - previous) as u16;
            previous = next;
            delay
        })
        .collect();
    for i in 0..count {
        if result[i] >= MIN_DELAY_CENTISECONDS {
            continue;
        }
        let mut need = MIN_DELAY_CENTISECONDS - result[i];
        result[i] = MIN_DELAY_CENTISECONDS;
        for donor in (0..i).rev() {
            let take = result[donor]
                .saturating_sub(MIN_DELAY_CENTISECONDS)
                .min(need);
            result[donor] -= take;
            need -= take;
            if need == 0 {
                break;
            }
        }
        debug_assert_eq!(need, 0, "validated plan has enough preceding delay budget");
    }
    result
}

/// One top-left/y-down display-oriented frame with straight (unpremultiplied) RGBA.
/// Pixels are private and omitted from Debug. Adapters must orient before returning
/// them and bound their own allocations; this type validates retained core memory.
pub struct DisplayRgbaFrame {
    presentation_seconds: f64,
    width: u32,
    height: u32,
    pixels: Box<[u8]>,
}
impl DisplayRgbaFrame {
    pub fn new(
        presentation_seconds: f64,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<Self, GifError> {
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(GifError::InvalidDimensions)?;
        if width == 0
            || height == 0
            || width > MAX_SOURCE_DIMENSION
            || height > MAX_SOURCE_DIMENSION
            || pixels > MAX_SOURCE_FRAME_PIXELS
        {
            return Err(GifError::LimitExceeded("source frame dimensions"));
        }
        if !presentation_seconds.is_finite() || presentation_seconds < 0. {
            return Err(GifError::InvalidTimestamp);
        }
        if pixels.checked_mul(4) != Some(rgba.len() as u64) {
            return Err(GifError::InvalidFrame);
        }
        Ok(Self {
            presentation_seconds,
            width,
            height,
            pixels: rgba.into_boxed_slice(),
        })
    }
    pub fn presentation_seconds(&self) -> f64 {
        self.presentation_seconds
    }
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
impl fmt::Debug for DisplayRgbaFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisplayRgbaFrame")
            .field("presentation_seconds", &self.presentation_seconds)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgba_bytes", &self.pixels.len())
            .finish()
    }
}

/// Sequential adapter seam, not a movie decoder. Frames must be nondecreasing in
/// presentation time and already display-oriented. Return None only at clean EOF;
/// a decoder failure must be Err. Long/blocking adapters must observe `control`.
/// The core does not seek; an adapter may start at/before the plan's trim start.
/// No adapter is permitted to retain an unbounded stream on the core's behalf.
pub trait SequentialFrameSource {
    fn next_frame(&mut self, control: &ExportControl)
    -> Result<Option<DisplayRgbaFrame>, GifError>;

    /// Validate the source after the last planned frame and before output
    /// publication. A bounded trim need not consume the source to EOF. Native
    /// adapters must check terminal decoder failure and selected-source identity
    /// on the same owner thread, without racing cancellation against sample reads.
    /// This hook runs once after successful encoding and staged readback; failure
    /// preserves the prior destination and removes staging. Synthetic sources
    /// need only cancellation.
    fn finish(&mut self, control: &ExportControl) -> Result<(), GifError> {
        control.check_active()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GifExportResult {
    pub path: PathBuf,
    pub frame_count: usize,
    pub size: (u16, u16),
    pub duration: f64,
    pub file_size: u64,
}

#[derive(Debug)]
pub enum GifError {
    NoFrames,
    ClipTooShort,
    InvalidDimensions,
    InvalidTimestamp,
    InvalidFrame,
    SameFile,
    DestinationExists,
    UnsafeDestination,
    StageIdentityChanged,
    UnsupportedFileIdentity,
    UnsupportedFilesystem,
    LimitExceeded(&'static str),
    Cancelled,
    ControlAlreadyUsed,
    ControlPoisoned,
    Source(String),
    Encode(String),
    InvalidOutput,
    Io(std::io::Error),
}
impl fmt::Display for GifError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => f.write_str("No source frames were available."),
            Self::ClipTooShort => f.write_str("The selected clip is shorter than 0.05 seconds."),
            Self::InvalidDimensions => f.write_str("The display dimensions must be finite and positive."),
            Self::InvalidTimestamp => f.write_str("Frame timestamps must be finite, nonnegative and nondecreasing."),
            Self::InvalidFrame => f.write_str("RGBA pixels do not match the frame dimensions."),
            Self::SameFile => f.write_str("The GIF cannot replace its source file."),
            Self::DestinationExists => f.write_str("The destination already exists; replacement was not authorized."),
            Self::StageIdentityChanged => f.write_str("The private staged GIF path no longer names the file owned by this export."),
            Self::UnsafeDestination => f.write_str("The destination must be a regular file in an unchanged existing directory; symlinks are not replaced."),
            Self::UnsupportedFilesystem => f.write_str("Secure GIF filesystem export is unavailable on this platform until native private-file and stable file-identity adapters are implemented."),
            Self::UnsupportedFileIdentity => f.write_str("This platform cannot safely compare existing source and destination file identities."),
            Self::LimitExceeded(limit) => write!(f, "GIF safety limit exceeded: {limit}."),
            Self::Cancelled => f.write_str("GIF export was cancelled before publication."),
            Self::ControlAlreadyUsed => f.write_str("An export control can be used for only one job."),
            Self::ControlPoisoned => f.write_str("GIF export state is unavailable."),
            Self::Source(message) => write!(f, "The frame source failed: {message}"),
            Self::Encode(message) => write!(f, "GIF encoding failed: {message}"),
            Self::InvalidOutput => f.write_str("GIF readback validation failed; output was discarded."),
            Self::Io(error) => write!(f, "GIF file operation failed: {error}"),
        }
    }
}
impl std::error::Error for GifError {}
impl From<std::io::Error> for GifError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
