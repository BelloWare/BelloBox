use super::{
    DisplayRgbaFrame, ExportControl, GifError, GifExportPlan, GifExportResult, MAX_OUTPUT_BYTES,
    MAX_SOURCE_SAMPLES, ReplacePolicy, SequentialFrameSource,
    gif_file::{Target, ensure_filesystem_supported},
    gif_structure::validate,
};
use std::{
    io::{self, Write},
    path::Path,
};

/// Encode an injected frame stream, validate it, and atomically publish locally.
/// `source_path` is required for a file-backed adapter so source alias protection
/// can be enforced; None is for genuinely synthetic/non-file-backed frames only.
/// Destination directories must already exist and be trusted against hostile
/// concurrent mutation of parents or relevant source/destination/stage entries.
/// Identity rechecks detect ordinary interference, but do not form a hostile
/// filesystem TOCTOU security boundary. No parent directories are created.
///
/// `progress(written, planned)` describes encoding only; publication is successful
/// only when this function returns Ok. A callback may request cancellation, but
/// should not block. At most current+pending input frames are retained. Each job
/// gets a new single-use control. Cancelled/failed jobs leave prior files intact.
/// No source frames, movie bytes, or pixel contents are logged or persisted.
/// Filesystem export is Unix-only until equivalent private-file and stable file
/// identity adapters exist elsewhere; unsupported targets fail before any work.
pub fn export_gif<S: SequentialFrameSource>(
    source: &mut S,
    plan: &GifExportPlan,
    source_path: Option<&Path>,
    destination: &Path,
    replace: ReplacePolicy,
    control: &ExportControl,
    mut progress: impl FnMut(usize, usize),
) -> Result<GifExportResult, GifError> {
    ensure_filesystem_supported()?;
    let _job = control.begin()?;
    let result = (|| {
        let target = Target::new(source_path, destination, replace)?;
        control.check_active()?;
        let mut stage = target.stage()?;
        encode(source, plan, &mut stage.file, control, &mut progress)?;
        control.check_active()?;
        stage.file.sync_all()?;
        validate(&mut stage.file, plan, control)?;
        let file_size = stage.file.metadata()?.len();
        let path = target.publish(&stage, control)?;
        Ok(GifExportResult {
            path,
            frame_count: plan.frame_count(),
            size: plan.output_size(),
            duration: plan.encoded_duration(),
            file_size,
        })
    })();
    if result.is_err() && matches!(control.status(), Ok(super::ExportStatus::Cancelled)) {
        return Err(GifError::Cancelled);
    }
    result
}

fn encode<S: SequentialFrameSource>(
    source: &mut S,
    plan: &GifExportPlan,
    output: &mut impl Write,
    control: &ExportControl,
    progress: &mut impl FnMut(usize, usize),
) -> Result<(), GifError> {
    let writer = BoundedWriter {
        inner: output,
        written: 0,
        maximum: MAX_OUTPUT_BYTES,
        control,
    };
    let mut encoder = ::gif::Encoder::new(writer, plan.output_width, plan.output_height, &[])
        .map_err(encode_error)?;
    if plan.loops {
        encoder
            .set_repeat(::gif::Repeat::Infinite)
            .map_err(encode_error)?;
    }
    // Once: deliberately never call set_repeat(Finite(0/1)). Browsers need NO
    // repeat block, not a count that means one or more additional plays.
    let mut stream = Stream {
        source,
        control,
        count: 0,
        previous: None,
        size: plan.source_size,
    };
    let mut pending = stream.next()?;
    let mut current: Option<DisplayRgbaFrame> = None;
    let latest = plan.end + 0.001;
    for (index, delay) in plan.delays.iter().copied().enumerate() {
        control.check_active()?;
        let target = plan.start + index as f64 * plan.frame_delay();
        let cutoff = (target + plan.frame_delay() / 2.).min(latest);
        while pending
            .as_ref()
            .is_some_and(|frame| frame.presentation_seconds <= cutoff)
        {
            current = pending.take();
            pending = stream.next()?;
        }
        if current.is_none()
            && pending
                .as_ref()
                .is_some_and(|frame| frame.presentation_seconds <= latest)
        {
            current = pending.take();
            pending = stream.next()?;
        }
        let frame = current.as_ref().ok_or(GifError::NoFrames)?;
        let mut rgba = render(frame, plan.output_width, plan.output_height, control)?;
        control.check_active()?;
        let mut gif =
            ::gif::Frame::from_rgba_speed(plan.output_width, plan.output_height, &mut rgba, 10);
        gif.delay = delay;
        gif.dispose = ::gif::DisposalMethod::Keep;
        encoder.write_frame(&gif).map_err(encode_error)?;
        control.check_active()?;
        progress(index + 1, plan.frame_count());
    }
    control.check_active()?;
    // into_inner, unlike Drop, reports trailer/finalization failures.
    encoder.into_inner().map_err(encode_error)?.flush()?;
    control.check_active()
}
fn encode_error(error: ::gif::EncodingError) -> GifError {
    GifError::Encode(error.to_string())
}

struct Stream<'a, S> {
    source: &'a mut S,
    control: &'a ExportControl,
    count: usize,
    previous: Option<f64>,
    size: (u32, u32),
}
impl<S: SequentialFrameSource> Stream<'_, S> {
    fn next(&mut self) -> Result<Option<DisplayRgbaFrame>, GifError> {
        self.control.check_active()?;
        // Stop before asking an adapter for another allocation. The sentinel read
        // at the limit is intentionally not made, even if it might return EOF.
        if self.count >= MAX_SOURCE_SAMPLES {
            return Err(GifError::LimitExceeded("sequential source samples"));
        }
        let frame = self.source.next_frame(self.control)?;
        self.control.check_active()?;
        if let Some(frame) = &frame {
            if frame.size() != self.size {
                return Err(GifError::InvalidDimensions);
            }
            if self
                .previous
                .is_some_and(|previous| frame.presentation_seconds < previous)
            {
                return Err(GifError::InvalidTimestamp);
            }
            self.previous = Some(frame.presentation_seconds);
            self.count += 1;
        }
        Ok(frame)
    }
}

/// Bilinear sampling in display coordinates, compositing straight RGBA over
/// opaque black as the Swift renderer does. Exact Core Graphics interpolation
/// and color-management parity remain native-adapter validation work.
fn render(
    frame: &DisplayRgbaFrame,
    width: u16,
    height: u16,
    control: &ExportControl,
) -> Result<Vec<u8>, GifError> {
    let width = usize::from(width);
    let height = usize::from(height);
    let bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(GifError::InvalidDimensions)?;
    let mut out = Vec::new();
    out.try_reserve_exact(bytes)
        .map_err(|_| GifError::LimitExceeded("render allocation"))?;
    out.resize(bytes, 0);
    for y in 0..height {
        control.check_active()?;
        let sy = ((y as f64 + 0.5) * f64::from(frame.height) / height as f64 - 0.5)
            .clamp(0., f64::from(frame.height - 1));
        let y0 = sy.floor() as u32;
        let y1 = (y0 + 1).min(frame.height - 1);
        let fy = sy - f64::from(y0);
        for x in 0..width {
            let sx = ((x as f64 + 0.5) * f64::from(frame.width) / width as f64 - 0.5)
                .clamp(0., f64::from(frame.width - 1));
            let x0 = sx.floor() as u32;
            let x1 = (x0 + 1).min(frame.width - 1);
            let fx = sx - f64::from(x0);
            let taps = [
                (x0, y0, (1. - fx) * (1. - fy)),
                (x1, y0, fx * (1. - fy)),
                (x0, y1, (1. - fx) * fy),
                (x1, y1, fx * fy),
            ];
            let output = (y * width + x) * 4;
            for c in 0..3 {
                let mut channel = 0.;
                for (sx, sy, weight) in taps {
                    let input = (sy as usize * frame.width as usize + sx as usize) * 4;
                    channel += f64::from(frame.pixels[input + c])
                        * f64::from(frame.pixels[input + 3])
                        / 255.
                        * weight;
                }
                out[output + c] = channel.round() as u8;
            }
            out[output + 3] = 255;
        }
    }
    Ok(out)
}

pub(super) struct BoundedWriter<'a, W> {
    pub inner: W,
    pub written: u64,
    pub maximum: u64,
    pub control: &'a ExportControl,
}
impl<W: Write> Write for BoundedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.control.check_active().map_err(io::Error::other)?;
        if self
            .written
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > self.maximum)
        {
            return Err(io::Error::other("encoded GIF byte limit exceeded"));
        }
        let count = self.inner.write(bytes)?;
        self.written += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.control.check_active().map_err(io::Error::other)?;
        self.inner.flush()
    }
}
