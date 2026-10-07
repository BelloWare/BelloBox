//! Pointer-free validation runs in ordinary builds, before AV/CoreVideo allocation.
use super::*;
pub(super) const TIMESCALE: i32 = 600;
pub(super) const MAX_RGBA_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_PIXEL_BUFFER_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy)]
pub(super) struct Spec {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}
impl Spec {
    pub fn validate(self) -> RecordingResult<usize> {
        if self.width < 2
            || self.height < 2
            || self.width > 3840
            || self.height > 3840
            || self.width % 2 != 0
            || self.height % 2 != 0
            || !(1..=30).contains(&self.fps)
        {
            return Err(RecordingError::InvalidFrame);
        }
        let bytes = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(RecordingError::LimitExceeded)?;
        if bytes > MAX_RGBA_BYTES {
            return Err(RecordingError::LimitExceeded);
        }
        Ok(bytes)
    }
}
pub(super) struct Frame {
    pub rgba: Vec<u8>,
    pub stride: usize,
    pub pts: i64,
    pub duration: i64,
}
#[derive(Default)]
pub(super) struct Timeline {
    count: usize,
    last_pts: Option<i64>,
    pub end: i64,
}
impl Timeline {
    pub fn validate(&self, spec: Spec, frame: &Frame) -> RecordingResult<()> {
        spec.validate()?;
        let row = spec.width as usize * 4;
        let extent = frame
            .stride
            .checked_mul(spec.height as usize)
            .ok_or(RecordingError::LimitExceeded)?;
        if frame.stride < row || frame.rgba.len() != extent {
            return Err(RecordingError::InvalidFrame);
        }
        if extent > MAX_RGBA_BYTES
            || frame.rgba.capacity() > MAX_RGBA_BYTES
            || self.count >= MAX_RECORDING_FRAMES
        {
            return Err(RecordingError::LimitExceeded);
        }
        let end = frame
            .pts
            .checked_add(frame.duration)
            .ok_or(RecordingError::InvalidTimestamp)?;
        if frame.pts < 0
            || frame.duration <= 0
            || frame.duration > TIMESCALE as i64
            || end > i64::from(MAX_RECORDING_SECONDS) * i64::from(TIMESCALE)
            || self.last_pts.is_some_and(|last| frame.pts <= last)
            || (self.count == 0 && frame.pts != 0)
        {
            return Err(RecordingError::InvalidTimestamp);
        }
        for row in frame.rgba.chunks_exact(frame.stride) {
            if row[..spec.width as usize * 4]
                .chunks_exact(4)
                .any(|pixel| pixel[3] != 255)
            {
                return Err(RecordingError::InvalidFrame);
            }
        }
        Ok(())
    }
    pub fn appended(&mut self, frame: &Frame) {
        self.count += 1;
        self.last_pts = Some(frame.pts);
        self.end = frame.pts + frame.duration;
    }
    pub fn count(&self) -> usize {
        self.count
    }
}
pub(super) fn validate_native_storage(
    spec: Spec,
    stride: usize,
    size: usize,
) -> RecordingResult<()> {
    spec.validate()?;
    let row = spec.width as usize * 4;
    let extent = stride
        .checked_mul(spec.height as usize)
        .ok_or(RecordingError::LimitExceeded)?;
    if stride < row || extent > size {
        return Err(RecordingError::InvalidFrame);
    }
    if size > MAX_PIXEL_BUFFER_BYTES || extent > MAX_PIXEL_BUFFER_BYTES {
        return Err(RecordingError::LimitExceeded);
    }
    Ok(())
}
