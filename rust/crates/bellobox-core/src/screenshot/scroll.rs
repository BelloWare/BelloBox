//! Portable scrolling-session foundation, ported from Swift ScrollCaptureEngine,
//! ScrollSampleAnalyzer and ImageStitcher. No screen, clipboard, file or network I/O.
//! The application supplies bounded frames; native region acquisition is not implemented.
mod analyze;
mod session;
mod stitch;
#[cfg(test)]
mod tests;

pub use analyze::{AnalysisJob, AnalyzedSample, analyze_sample};
pub use session::{FinishJob, FinishedScroll, ScrollPhase, ScrollSession, finish_scroll};
pub use stitch::stitch;

use super::{
    MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS, MAX_PNG_BYTES, ScreenshotDocument, validate_image_size,
};
use image::{ImageDecoder, RgbaImage};
use std::{
    fmt,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_WORKING_BYTES: u64 = 512 * 1024 * 1024;
const SEAM_SLACK: u32 = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrollError {
    Cancelled,
    NoFrames,
    AreaChanged,
    InvalidInput,
    MemoryLimit,
    OutputTooTall(u64),
    InvalidState,
    CannotRender,
}
impl fmt::Display for ScrollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Scrolling capture was cancelled.",
            Self::NoFrames => "No frames were captured for scrolling screenshot.",
            Self::AreaChanged => "The capture area changed size.",
            Self::InvalidInput => "The scrolling frame or options are outside supported limits.",
            Self::MemoryLimit => {
                "Scrolling capture reached its memory limit. Finish the frames already captured."
            }
            Self::OutputTooTall(_) => "The stitched screenshot would be too tall.",
            Self::InvalidState => "This scrolling capture operation is no longer active.",
            Self::CannotRender => "Could not render the scrolling screenshot.",
        })
    }
}
impl std::error::Error for ScrollError {}
pub type ScrollResultValue<T> = Result<T, ScrollError>;

/// Images deliberately omit pixels, paths and image payloads from Debug.
#[derive(Clone)]
pub struct ScrollFrame(Arc<RgbaImage>);
impl fmt::Debug for ScrollFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollFrame")
            .field("dimensions", &self.dimensions())
            .finish()
    }
}
impl ScrollFrame {
    pub fn from_rgba(image: RgbaImage) -> ScrollResultValue<Self> {
        validate_image_size(image.width(), image.height())
            .map_err(|_| ScrollError::InvalidInput)?;
        Ok(Self(Arc::new(image)))
    }
    pub fn from_png(bytes: &[u8]) -> ScrollResultValue<Self> {
        if bytes.len() > MAX_PNG_BYTES {
            return Err(ScrollError::InvalidInput);
        }
        let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(|_| ScrollError::InvalidInput)?;
        let (w, h) = decoder.dimensions();
        validate_image_size(w, h).map_err(|_| ScrollError::InvalidInput)?;
        Self::from_rgba(
            image::DynamicImage::from_decoder(decoder)
                .map_err(|_| ScrollError::InvalidInput)?
                .to_rgba8(),
        )
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions()
    }
    pub fn width(&self) -> u32 {
        self.0.width()
    }
    pub fn height(&self) -> u32 {
        self.0.height()
    }
    pub fn byte_len(&self) -> u64 {
        self.0.as_raw().len() as u64
    }
    /// For the bounded, app-owned fixture viewport only; final export uses the editor.
    pub fn png(&self) -> ScrollResultValue<Vec<u8>> {
        super::render::encode_png(&self.0).map_err(|_| ScrollError::CannotRender)
    }
}

#[derive(Clone, Debug)]
pub struct Cancellation(Arc<AtomicBool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> ScrollResultValue<()> {
        if self.is_cancelled() {
            Err(ScrollError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Down,
    Up,
}
#[derive(Clone, Debug)]
pub struct StitchConfig {
    pub direction: ScrollDirection,
    pub min_overlap: u32,
    pub max_overlap_fraction: f64,
    pub downsample_width: u32,
    pub score_threshold: f64,
    pub remove_repeated_bars: bool,
    pub max_output_height: u32,
    pub max_working_bytes: u64,
}
impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            direction: ScrollDirection::Down,
            min_overlap: 80,
            max_overlap_fraction: 0.70,
            downsample_width: 420,
            score_threshold: 0.08,
            remove_repeated_bars: true,
            max_output_height: MAX_IMAGE_DIMENSION,
            max_working_bytes: MAX_WORKING_BYTES,
        }
    }
}
impl StitchConfig {
    fn validate(&self) -> ScrollResultValue<()> {
        if self.min_overlap == 0
            || self.min_overlap > MAX_IMAGE_DIMENSION
            || self.downsample_width == 0
            || self.downsample_width > 420
            || !self.max_overlap_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.max_overlap_fraction)
            || !self.score_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.score_threshold)
            || self.max_output_height == 0
            || self.max_output_height > MAX_IMAGE_DIMENSION
            || self.max_working_bytes == 0
            || self.max_working_bytes > MAX_WORKING_BYTES
        {
            return Err(ScrollError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct ScrollConfig {
    pub sample_interval_ms: u64,
    pub change_threshold: f64,
    pub settle_threshold: f64,
    pub eager_overlap_fraction: f64,
    pub unmatched_change_threshold: f64,
    pub minimum_new_rows: u32,
    pub max_frames: usize,
    pub stitch: StitchConfig,
}
impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            sample_interval_ms: 140,
            change_threshold: 0.01,
            settle_threshold: 0.004,
            eager_overlap_fraction: 0.45,
            unmatched_change_threshold: 0.08,
            minimum_new_rows: 8,
            max_frames: 20,
            stitch: StitchConfig {
                max_overlap_fraction: 0.985,
                ..Default::default()
            },
        }
    }
}
impl ScrollConfig {
    fn validate(&self) -> ScrollResultValue<()> {
        self.stitch.validate()?;
        if !(2..=60).contains(&self.max_frames)
            || !(1..=10_000).contains(&self.sample_interval_ms)
            || self.minimum_new_rows == 0
            || self.minimum_new_rows > MAX_IMAGE_DIMENSION
            || [
                self.change_threshold,
                self.settle_threshold,
                self.eager_overlap_fraction,
                self.unmatched_change_threshold,
            ]
            .iter()
            .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        {
            return Err(ScrollError::InvalidInput);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FramePlacement {
    pub frame_index: usize,
    pub y: i64,
    pub overlap: u32,
    pub confidence: f64,
    pub cropped_top: u32,
    pub cropped_bottom: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrollCaptureNote {
    FinalSampleFailed,
    DroppedFrames(usize),
    UnmatchedSeam(usize),
    NearlyUnchanged(usize),
}
impl fmt::Display for ScrollCaptureNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FinalSampleFailed => f.write_str("The last scroll could not be sampled. The screenshot includes the frames already captured."),
            Self::DroppedFrames(n) => write!(f, "The last {n} frame(s) were left out to keep the screenshot within the maximum height."),
            Self::UnmatchedSeam(n) => write!(f, "Frame {n} could not be matched to the frame before it, so the picture may be missing or repeating content at that seam."),
            Self::NearlyUnchanged(n) => write!(f, "Frame {n} looks almost the same as the frame before it, so it adds little or nothing."),
        }
    }
}
#[derive(Debug)]
pub struct ScrollResult {
    pub document: ScreenshotDocument,
    pub placements: Vec<FramePlacement>,
    pub notes: Vec<ScrollCaptureNote>,
    pub frame_count: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewPiece {
    pub frame_index: usize,
    pub from_row: u32,
    pub to_row: u32,
}
impl PreviewPiece {
    pub fn rows(&self) -> u32 {
        self.to_row.saturating_sub(self.from_row)
    }
}

fn bytes_for(w: u32, h: u32, channels: u64) -> ScrollResultValue<u64> {
    u64::from(w)
        .checked_mul(u64::from(h))
        .and_then(|n| n.checked_mul(channels))
        .ok_or(ScrollError::MemoryLimit)
}
fn check_budget(parts: impl IntoIterator<Item = u64>, limit: u64) -> ScrollResultValue<u64> {
    let sum = parts
        .into_iter()
        .try_fold(0u64, |sum, part| sum.checked_add(part))
        .ok_or(ScrollError::MemoryLimit)?;
    if sum > limit {
        Err(ScrollError::MemoryLimit)
    } else {
        Ok(sum)
    }
}

/// A deterministic app-owned page. This cannot read the screen or other applications.
#[cfg(any(test, debug_assertions))]
pub fn synthetic_page_frame(
    offset: u32,
    width: u32,
    height: u32,
    page_height: u32,
) -> ScrollResultValue<ScrollFrame> {
    validate_image_size(width, height).map_err(|_| ScrollError::InvalidInput)?;
    if page_height < height || page_height > MAX_IMAGE_DIMENSION || offset > page_height - height {
        return Err(ScrollError::InvalidInput);
    }
    // Swift's HUD fixture uses broad colored bands and narrow row markers. Avoid
    // full-width one-pixel noise/repeating stripes: they can alias the source's
    // four-row coarse overlap search before its full-resolution refinement.
    // Interpolated, absolute-page color anchors keep adjacent rows similar and
    // distant sections distinct; every viewport renders the same document pixels.
    fn anchor(index: u32, channel: u32) -> u32 {
        let mut hash = index
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add(channel.wrapping_mul(0x85eb_ca6b))
            .wrapping_add(0x2545_f491);
        hash ^= hash >> 16;
        hash = hash.wrapping_mul(0x7feb_352d);
        hash ^= hash >> 15;
        55 + hash % 170
    }
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        let row = offset + y;
        let anchor_index = row / 16;
        let position = row % 16;
        let mut color = [0u8; 4];
        for channel in 0..3 {
            color[channel as usize] = ((anchor(anchor_index, channel) * (16 - position)
                + anchor(anchor_index + 1, channel) * position)
                / 16) as u8;
        }
        color[3] = 255;
        let marker_width = 14 + ((row / 8 * 7) % 11) * 6;
        for x in 0..width {
            let mut pixel = color;
            if row.is_multiple_of(8) && x >= 6 && x < 6 + marker_width {
                for channel in &mut pixel[..3] {
                    *channel = (u16::from(*channel) * 65 / 100) as u8;
                }
            }
            image.put_pixel(x, y, image::Rgba(pixel));
        }
    }
    ScrollFrame::from_rgba(image)
}

/// A small HUD-only composite. It must never be used for final export or OCR.
#[derive(Clone, Debug)]
pub struct ScrollPreview {
    frames: Vec<ScrollFrame>,
    pieces: Vec<PreviewPiece>,
}
impl ScrollPreview {
    pub fn png(&self, cancel: &Cancellation) -> ScrollResultValue<Vec<u8>> {
        cancel.check()?;
        let first = self.frames.first().ok_or(ScrollError::NoFrames)?;
        let rows: u64 = self.pieces.iter().map(|p| u64::from(p.rows())).sum();
        if rows == 0 {
            return Err(ScrollError::NoFrames);
        }
        let scale = (120. / f64::from(first.width()))
            .min(2048. / rows as f64)
            .min(1.);
        let width = (f64::from(first.width()) * scale).round().max(1.) as u32;
        let height = (rows as f64 * scale).round().max(1.) as u32;
        let mut out = RgbaImage::new(width, height);
        let mut piece_index = 0;
        let mut row_start = 0u64;
        for y in 0..height {
            cancel.check()?;
            let source_y = (u64::from(y) * rows / u64::from(height)).min(rows - 1);
            while piece_index + 1 < self.pieces.len()
                && source_y >= row_start + u64::from(self.pieces[piece_index].rows())
            {
                row_start += u64::from(self.pieces[piece_index].rows());
                piece_index += 1;
            }
            let piece = &self.pieces[piece_index];
            let frame = &self.frames[piece.frame_index];
            let sy = piece.from_row + (source_y - row_start) as u32;
            for x in 0..width {
                let sx = (u64::from(x) * u64::from(frame.width()) / u64::from(width)) as u32;
                out.put_pixel(x, y, *frame.0.get_pixel(sx, sy));
            }
        }
        cancel.check()?;
        super::render::encode_png(&out).map_err(|_| ScrollError::CannotRender)
    }
}

/// Pure monotonic-clock gate for Finish's second sample. The caller supplies
/// elapsed time so tests can control the clock without sleeping. Swift waits
/// max(50 ms, sampleInterval) *after* the first final sample has been processed.
#[derive(Clone, Debug, Default)]
pub struct FinalSampleSchedule {
    not_before: Option<std::time::Duration>,
}
impl FinalSampleSchedule {
    pub fn reset(&mut self) {
        self.not_before = None;
    }
    pub fn ready(&self, elapsed: std::time::Duration) -> bool {
        self.not_before.is_none_or(|deadline| elapsed >= deadline)
    }
    pub fn first_sample_accepted(
        &mut self,
        elapsed: std::time::Duration,
        sample_interval_ms: u64,
    ) -> ScrollResultValue<()> {
        if !(1..=10_000).contains(&sample_interval_ms) {
            return Err(ScrollError::InvalidInput);
        }
        let delay = std::time::Duration::from_millis(sample_interval_ms.max(50));
        self.not_before = Some(
            elapsed
                .checked_add(delay)
                .ok_or(ScrollError::InvalidInput)?,
        );
        Ok(())
    }
}
