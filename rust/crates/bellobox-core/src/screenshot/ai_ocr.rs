//! Immutable, privacy-prepared single-image OCR. No I/O, credentials discovery,
//! settings access or network occurs here. Payloads, secrets and provider output
//! intentionally have no Debug implementation and never enter diagnostics.
mod request;
mod response;
#[cfg(test)]
mod tests;
pub use request::{
    GenerationOptions, ImageRequest, ProviderAuthority, ProviderLease, ReasoningEffort, Thinking,
    build_request,
};
pub use response::{MAX_RESPONSE_BYTES, OcrResult, parse_ocr_text, parse_response};

use super::{ScreenshotDocument, render};
use image::{ImageEncoder, RgbaImage};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    sync::{Arc, Weak},
};

pub const DEFAULT_UPLOAD_LONG_EDGE: u32 = 2_200;
pub const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_BASE64_BYTES: usize = MAX_UPLOAD_BYTES.div_ceil(3) * 4;
pub const MAX_REQUEST_BYTES: usize = MAX_BASE64_BYTES + 16 * 1024;
pub const BACKGROUND_NOTICE: &str =
    "Transparency is flattened onto an opaque white background before resizing and upload.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    PlainText,
    #[default]
    PlainTextAndMarkdown,
    TableMarkdown,
}
impl OutputFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::PlainText => "Plain text",
            Self::PlainTextAndMarkdown => "Plain text and Markdown",
            Self::TableMarkdown => "Markdown tables",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadOptions {
    pub max_long_edge: u32,
    pub output_format: OutputFormat,
}
impl Default for UploadOptions {
    fn default() -> Self {
        Self {
            max_long_edge: DEFAULT_UPLOAD_LONG_EDGE,
            output_format: OutputFormat::default(),
        }
    }
}
impl UploadOptions {
    pub fn validate(self) -> Result<(), String> {
        if !(800..=5_000).contains(&self.max_long_edge) {
            return Err("AI OCR upload edge must be between 800 and 5,000 pixels.".into());
        }
        Ok(())
    }
}

/// A weak, process-local source identity, not a content hash. Used only to bind
/// the sealed generated-image transport facility to its own base image. Keeping
/// a weak owner prevents allocator address reuse from reviving a retired source.
#[derive(Clone)]
pub struct DocumentBinding(Weak<RgbaImage>);
impl DocumentBinding {
    pub fn new(document: &ScreenshotDocument) -> Self {
        Self(Arc::downgrade(&document.base_image))
    }
    pub fn matches_document(&self, document: &ScreenshotDocument) -> bool {
        self.0
            .upgrade()
            .is_some_and(|image| Arc::ptr_eq(&image, &document.base_image))
    }
    pub fn matches_prepared(&self, image: &PreparedImage) -> bool {
        self.0.strong_count() > 0 && Weak::ptr_eq(&self.0, &image.0.source)
    }
}

#[derive(Clone)]
pub struct PreparedImage(Arc<PreparedInner>);
struct PreparedInner {
    png: Vec<u8>,
    dimensions: (u32, u32),
    digest: String,
    warnings: Vec<String>,
    source: Weak<RgbaImage>,
    options: UploadOptions,
}
impl PreparedImage {
    /// Render crop and masks (with eraser holes) first, exclude decoration, then
    /// canonicalize all alpha onto white before any resize. Hidden RGB in fully
    /// transparent pixels is destroyed before interpolation can spread it.
    pub fn prepare(document: &ScreenshotDocument, options: &UploadOptions) -> Result<Self, String> {
        options.validate()?;
        let mut image = render::render(document, false, None)
            .map_err(|_| "Could not prepare the redaction-aware OCR image.")?;
        for pixel in image.pixels_mut() {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel.0[..3] {
                *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
            pixel[3] = 255;
        }
        let mut warnings = vec![BACKGROUND_NOTICE.into()];
        let edge = image.width().max(image.height());
        if edge > options.max_long_edge {
            let width = (u64::from(image.width()) * u64::from(options.max_long_edge)
                / u64::from(edge))
            .max(1) as u32;
            let height = (u64::from(image.height()) * u64::from(options.max_long_edge)
                / u64::from(edge))
            .max(1) as u32;
            image = image::imageops::resize(
                &image,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            );
            warnings.push("Image was downscaled before AI OCR upload.".into());
        }
        let png = encode_upload_png(&image, MAX_UPLOAD_BYTES)?;
        let digest = Sha256::digest(&png)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect();
        Ok(Self(Arc::new(PreparedInner {
            png,
            dimensions: image.dimensions(),
            digest,
            warnings,
            source: Arc::downgrade(&document.base_image),
            options: *options,
        })))
    }
    pub fn png(&self) -> &[u8] {
        &self.0.png
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions
    }
    pub fn digest(&self) -> &str {
        &self.0.digest
    }
    pub fn warnings(&self) -> &[String] {
        &self.0.warnings
    }
    pub fn byte_count(&self) -> usize {
        self.0.png.len()
    }
    pub fn background_notice(&self) -> &'static str {
        BACKGROUND_NOTICE
    }
    pub fn options(&self) -> UploadOptions {
        self.0.options
    }
    /// Preview decodes the retained final bytes. It never re-renders a document.
    pub fn decode_preview(&self) -> Result<RgbaImage, String> {
        image::load_from_memory_with_format(self.png(), image::ImageFormat::Png)
            .map(|v| v.to_rgba8())
            .map_err(|_| "Could not decode the approved OCR preview.".into())
    }
}

fn encode_upload_png(image: &RgbaImage, limit: usize) -> Result<Vec<u8>, String> {
    let mut output = BoundedBytes::new(limit);
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| "AI OCR image encoding failed or exceeded the 20 MiB limit.")?;
    // The PNG writer can fail a final IEND write from Drop after write_image
    // reports success. The sticky bound must be checked before retaining bytes.
    if output.exceeded {
        return Err("AI OCR image exceeds the 20 MiB limit.".into());
    }
    Ok(output.bytes)
}

struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl BoundedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) || self.exceeded {
            self.exceeded = true;
            return Err(std::io::Error::other("OCR payload size limit exceeded."));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
