//! Streaming preview of an owned export result, never an arbitrary movie importer.
//! Only a single full-canvas RGBA frame and its PNG are materialized at a time.
use super::{GifError, GifExportResult, MAX_FRAMES, MAX_OUTPUT_BYTES};
use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom, Take},
    time::SystemTime,
};

pub struct GifPreview {
    reader: ::gif::Decoder<Take<File>>,
    identity: File,
    length: u64,
    modified: Option<SystemTime>,
    size: (u16, u16),
    expected: usize,
    index: usize,
    expected_centiseconds: u64,
    elapsed_centiseconds: u64,
}
pub struct GifPreviewFrame {
    pub png: Vec<u8>,
    pub delay_centiseconds: u16,
}
impl GifPreview {
    pub fn open(file: File, result: &GifExportResult) -> Result<Self, GifError> {
        if !result.duration.is_finite()
            || result.duration <= 0.
            || result.duration > 120.
            || result.frame_count == 0
            || result.frame_count > MAX_FRAMES
            || result.size.0 == 0
            || result.size.1 == 0
            || result.size.0 > 1080
            || result.size.1 > 1080
        {
            return Err(GifError::InvalidOutput);
        }
        // The caller supplies a nonblocking, no-follow regular-file handle.
        // Retain a metadata-only clone; decoding pins that opened inode and caps
        // total bytes even if the source grows while it is being read.
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.len() != result.file_size
            || metadata.len() > MAX_OUTPUT_BYTES
        {
            return Err(GifError::InvalidOutput);
        }
        let identity = file.try_clone()?;
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::RGBA);
        options.set_memory_limit(::gif::MemoryLimit::Bytes(
            std::num::NonZeroU64::new(16 * 1024 * 1024).unwrap(),
        ));
        let reader = options
            .read_info(file.take(MAX_OUTPUT_BYTES))
            .map_err(|e| GifError::Source(e.to_string()))?;
        if (reader.width(), reader.height()) != result.size {
            return Err(GifError::InvalidOutput);
        }
        Ok(Self {
            reader,
            identity,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            size: result.size,
            expected: result.frame_count,
            index: 0,
            expected_centiseconds: (result.duration * 100.).round() as u64,
            elapsed_centiseconds: 0,
        })
    }
    /// Restart the exact opened file, never the path that may now name a replacement.
    /// Consuming self prevents concurrent decoder reads while the shared offset resets.
    pub fn rewind(self) -> Result<Self, GifError> {
        let Self {
            reader,
            mut identity,
            length,
            modified,
            size,
            expected,
            expected_centiseconds,
            ..
        } = self;
        drop(reader);
        let metadata = identity.metadata()?;
        if metadata.len() != length || metadata.modified().ok() != modified {
            return Err(GifError::InvalidOutput);
        }
        identity.seek(SeekFrom::Start(0))?;
        let expected_result = GifExportResult {
            path: Default::default(),
            frame_count: expected,
            size,
            duration: expected_centiseconds as f64 / 100.,
            file_size: length,
        };
        let restarted = Self::open(identity, &expected_result)?;
        if restarted.modified != modified {
            return Err(GifError::InvalidOutput);
        }
        Ok(restarted)
    }
    pub fn next_frame(&mut self) -> Result<Option<GifPreviewFrame>, GifError> {
        let metadata = self.identity.metadata()?;
        if !metadata.is_file()
            || metadata.len() != self.length
            || metadata.modified().ok() != self.modified
        {
            return Err(GifError::InvalidOutput);
        }
        let frame = self
            .reader
            .read_next_frame()
            .map_err(|e| GifError::Source(e.to_string()))?;
        let after = self.identity.metadata()?;
        if after.len() != self.length || after.modified().ok() != self.modified {
            return Err(GifError::InvalidOutput);
        }
        let Some(frame) = frame else {
            return if self.index == self.expected
                && self.elapsed_centiseconds == self.expected_centiseconds
            {
                Ok(None)
            } else {
                Err(GifError::InvalidOutput)
            };
        };
        if self.index >= self.expected
            || frame.left != 0
            || frame.top != 0
            || (frame.width, frame.height) != self.size
            // Own export: 5–20fps with a 2cs browser-safe partial minimum.
            || !(2..=20).contains(&frame.delay)
            || self.elapsed_centiseconds + u64::from(frame.delay) > self.expected_centiseconds
        {
            return Err(GifError::InvalidOutput);
        }
        self.index += 1;
        self.elapsed_centiseconds += u64::from(frame.delay);
        let image = image::RgbaImage::from_raw(
            u32::from(frame.width),
            u32::from(frame.height),
            frame.buffer.to_vec(),
        )
        .ok_or(GifError::InvalidFrame)?;
        let delay_centiseconds = frame.delay;
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| GifError::Source(e.to_string()))?;
        Ok(Some(GifPreviewFrame {
            png: png.into_inner(),
            delay_centiseconds,
        }))
    }
}
