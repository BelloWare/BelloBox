//! Strict completion for the bounded, full-canvas, opaque GIFs we export.
//! gif 0.14.2's pixel fill can return before consuming EOI. Metadata advancement
//! uses a discard sink, which may skip that remaining compressed input. Always
//! drain with a real one-pixel sink first, keeping strict EOI checking enabled.
use super::{GifError, MAX_FRAMES, MAX_OUTPUT_BYTES};
use std::{
    borrow::Cow,
    io::{Read, Take},
};

pub(super) struct Decoder<R: Read> {
    inner: Option<::gif::Decoder<Take<R>>>,
    size: (u16, u16),
    count: usize,
    failed: bool,
}
impl<R: Read> Decoder<R> {
    pub fn new(reader: R) -> Result<Self, GifError> {
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::RGBA);
        options.check_frame_consistency(true);
        options.check_lzw_end_code(true);
        options.set_memory_limit(::gif::MemoryLimit::Bytes(
            (1080 * 1080 * 4).try_into().expect("positive constant"),
        ));
        let inner = options
            .read_info(reader.take(MAX_OUTPUT_BYTES + 1))
            .map_err(|_| GifError::InvalidOutput)?;
        let size = (inner.width(), inner.height());
        if size.0 == 0 || size.1 == 0 || size.0 > 1080 || size.1 > 1080 {
            return Err(GifError::InvalidOutput);
        }
        Ok(Self {
            inner: Some(inner),
            size,
            count: 0,
            failed: false,
        })
    }
    pub fn size(&self) -> (u16, u16) {
        self.size
    }
    pub fn next_frame(
        &mut self,
        check: impl Fn() -> Result<(), GifError>,
    ) -> Result<Option<::gif::Frame<'static>>, GifError> {
        if self.failed {
            return Err(GifError::InvalidOutput);
        }
        let result = self.decode(&check);
        if result.is_err() {
            self.failed = true;
        }
        // Preserve cancellation even when a cancellation-aware Read caused the
        // codec to report an I/O/format error. A failed decoder cannot resume.
        if result.is_err() {
            check()?;
        }
        result
    }
    fn decode(
        &mut self,
        check: &impl Fn() -> Result<(), GifError>,
    ) -> Result<Option<::gif::Frame<'static>>, GifError> {
        check()?;
        let Some(inner) = self.inner.as_mut() else {
            return Ok(None);
        };
        let Some(mut frame) = inner
            .next_frame_info()
            .map_err(|_| GifError::InvalidOutput)?
            .cloned()
        else {
            // The codec has consumed the trailer. Recover its buffered reader,
            // including prefetched bytes, so preview also requires exact EOF.
            // The extra byte in Take distinguishes a real EOF at the byte cap
            // from EOF manufactured by Take after an oversized input.
            let mut tail = self.inner.take().expect("active decoder").into_inner();
            let mut trailing = [0];
            let read = tail
                .read(&mut trailing)
                .map_err(|_| GifError::InvalidOutput)?;
            check()?;
            if read != 0 || tail.get_ref().limit() == 0 {
                return Err(GifError::InvalidOutput);
            }
            return Ok(None);
        };
        if self.count >= MAX_FRAMES
            || frame.left != 0
            || frame.top != 0
            || (frame.width, frame.height) != self.size
            || frame.transparent.is_some()
        {
            return Err(GifError::InvalidOutput);
        }
        let length = usize::from(frame.width) * usize::from(frame.height) * 4;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length)
            .map_err(|_| GifError::LimitExceeded("GIF readback allocation"))?;
        rgba.resize(length, 0);
        check()?;
        inner
            .read_into_buffer(&mut rgba)
            .map_err(|_| GifError::InvalidOutput)?;
        check()?;
        // False means the codec reached DataEnd without another complete pixel.
        // With strict EOI still enabled, missing EOI/truncation is an error,
        // not false. True is additional decoded output and is always rejected.
        if inner
            .fill_buffer(&mut [0; 4])
            .map_err(|_| GifError::InvalidOutput)?
        {
            return Err(GifError::InvalidOutput);
        }
        check()?;
        // gif's palette expansion leaves an unknown index as zero RGBA. Our
        // writer emits only opaque pixels, so reject invalid palette references.
        for row in rgba.chunks(usize::from(frame.width) * 4) {
            check()?;
            if row.chunks_exact(4).any(|pixel| pixel[3] != 255) {
                return Err(GifError::InvalidOutput);
            }
        }
        frame.buffer = Cow::Owned(rgba);
        frame.interlaced = false;
        self.count += 1;
        Ok(Some(frame))
    }
}
