//! Local screenshot annotation, crop and privacy-aware export.
//!
//! All geometry is in original image pixels with a top-left origin. The base
//! image is immutable and shared across undo snapshots. Every eraser stroke
//! belongs to the annotations present when it was made, never to the image.
//! Rendering is tiled, bounded, and shared by preview/export/OCR. This module
//! performs no filesystem or network access and never logs image or OCR data.
mod render;
pub mod scroll;
pub mod selection;
#[cfg(test)]
mod tests;
mod text;

use image::{ImageDecoder, RgbaImage};
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub const MAX_IMAGE_PIXELS: u64 = 64_000_000;
pub const MAX_IMAGE_DIMENSION: u32 = 60_000;
pub const MAX_PNG_BYTES: usize = 64_000_000;
/// Preview images must never become a single, potentially very tall GPU texture.
pub const MAX_PREVIEW_TILE_EDGE: u32 = 1_024;
pub const MAX_PREVIEW_PNG_BYTES: usize = 64_000_000;
pub const MAX_ANNOTATIONS: usize = 2_048;
pub const MAX_POINTS: usize = 65_536;
pub const MAX_TEXT_BYTES: usize = 16_384;
pub const MAX_DOCUMENT_BYTES: usize = 4_000_000;
pub const MAX_HISTORY_STEPS: usize = 64;
const MAX_HISTORY_BYTES: usize = 16_000_000;
const MAX_COORDINATE: f32 = 1_000_000.;
static NEXT_ANNOTATION_ID: AtomicU64 = AtomicU64::new(1);
pub type AnnotationId = u64;

/// One lossless, native-resolution preview tile in active-crop coordinates.
///
/// Tiles are returned in row-major order. A UI can cache their compressed PNGs
/// and decode/upload only tiles intersecting its viewport. Copy and Save must
/// use `RenderSnapshot::render_png` instead of reusing a preview texture.
#[derive(Clone, PartialEq, Eq)]
pub struct PreviewTile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}
impl std::fmt::Debug for PreviewTile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewTile")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("png_bytes", &self.png.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
    fn valid(self) -> bool {
        valid_coordinate(self.x) && valid_coordinate(self.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn from_points(a: Point, b: Point) -> Self {
        Self::new(
            a.x.min(b.x),
            a.y.min(b.y),
            (b.x - a.x).abs(),
            (b.y - a.y).abs(),
        )
    }
    pub fn standardized(self) -> Self {
        Self::new(
            self.x.min(self.x + self.width),
            self.y.min(self.y + self.height),
            self.width.abs(),
            self.height.abs(),
        )
    }
    pub fn right(self) -> f32 {
        self.x + self.width
    }
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
    pub fn contains(self, p: Point) -> bool {
        let r = self.standardized();
        p.x >= r.x && p.y >= r.y && p.x <= r.right() && p.y <= r.bottom()
    }
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }
    pub fn expanded(self, amount: f32) -> Self {
        let r = self.standardized();
        Self::new(
            r.x - amount,
            r.y - amount,
            r.width + 2. * amount,
            r.height + 2. * amount,
        )
    }
    pub fn intersection(self, other: Self) -> Option<Self> {
        let a = self.standardized();
        let b = other.standardized();
        let x = a.x.max(b.x);
        let y = a.y.max(b.y);
        let right = a.right().min(b.right());
        let bottom = a.bottom().min(b.bottom());
        (right > x && bottom > y).then(|| Self::new(x, y, right - x, bottom - y))
    }
    /// Expand to whole pixels, matching the screenshot crop contract.
    pub fn integral(self) -> Self {
        let r = self.standardized();
        Self::new(
            r.x.floor(),
            r.y.floor(),
            r.right().ceil() - r.x.floor(),
            r.bottom().ceil() - r.y.floor(),
        )
    }
    fn valid(self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.right(),
            self.bottom(),
        ]
        .into_iter()
        .all(valid_coordinate)
    }
}
fn valid_coordinate(v: f32) -> bool {
    v.is_finite() && v.abs() <= MAX_COORDINATE
}
fn normalized(v: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        fallback
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbaColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}
impl RgbaColor {
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
    pub fn from_rgb8(red: u8, green: u8, blue: u8) -> Self {
        Self::new(
            red as f32 / 255.,
            green as f32 / 255.,
            blue as f32 / 255.,
            1.,
        )
    }
    pub fn normalized(self) -> Self {
        Self::new(
            normalized(self.red, 0., 0., 1.),
            normalized(self.green, 0., 0., 1.),
            normalized(self.blue, 0., 0., 1.),
            normalized(self.alpha, 1., 0., 1.),
        )
    }
    pub fn rgba8(self) -> [u8; 4] {
        let c = self.normalized();
        [c.red, c.green, c.blue, c.alpha].map(|v| (v * 255.).round() as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MaskPattern {
    Solid,
    #[default]
    Stripes,
    Dots,
}
impl MaskPattern {
    pub const ALL: [Self; 3] = [Self::Solid, Self::Stripes, Self::Dots];
    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Stripes => "Stripes",
            Self::Dots => "Dots",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnnotationTool {
    #[default]
    Select,
    Pen,
    Arrow,
    Rectangle,
    Highlight,
    Text,
    Crop,
    Blur,
    Eraser,
}
impl AnnotationTool {
    pub const ALL: [Self; 9] = [
        Self::Select,
        Self::Pen,
        Self::Arrow,
        Self::Rectangle,
        Self::Highlight,
        Self::Text,
        Self::Crop,
        Self::Blur,
        Self::Eraser,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pen => "Pen",
            Self::Arrow => "Arrow",
            Self::Rectangle => "Rectangle",
            Self::Highlight => "Highlight",
            Self::Text => "Text",
            Self::Crop => "Crop",
            Self::Blur => "Mask",
            Self::Eraser => "Eraser",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationStyle {
    pub stroke_color: RgbaColor,
    pub fill_color: Option<RgbaColor>,
    pub line_width: f32,
    pub opacity: f32,
    pub font_size: f32,
    pub mask_pattern: MaskPattern,
}
impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            stroke_color: RgbaColor::new(0.95, 0.42, 0.08, 1.),
            fill_color: None,
            line_width: 4.,
            opacity: 1.,
            font_size: 18.,
            mask_pattern: MaskPattern::Stripes,
        }
    }
}
impl AnnotationStyle {
    pub const REDACTION_FILL: RgbaColor = RgbaColor::new(0.16, 0.16, 0.16, 1.);
    pub const MASK_FILL_PRESETS: [(&'static str, RgbaColor); 6] = [
        ("Charcoal", Self::REDACTION_FILL),
        ("Black", RgbaColor::new(0.02, 0.02, 0.02, 1.)),
        ("White", RgbaColor::new(0.98, 0.97, 0.95, 1.)),
        ("Orange", RgbaColor::new(0.89, 0.46, 0.15, 1.)),
        ("Teal", RgbaColor::new(0.06, 0.40, 0.43, 1.)),
        ("Plum", RgbaColor::new(0.46, 0.26, 0.70, 1.)),
    ];
    pub fn highlight() -> Self {
        Self {
            stroke_color: RgbaColor::new(1., 0.78, 0.12, 1.),
            fill_color: Some(RgbaColor::new(1., 0.85, 0.12, 0.32)),
            line_width: 0.,
            opacity: 0.45,
            ..Self::default()
        }
    }
    pub fn redaction() -> Self {
        Self {
            stroke_color: RgbaColor::new(0.12, 0.12, 0.12, 1.),
            fill_color: Some(Self::REDACTION_FILL),
            line_width: 0.,
            ..Self::default()
        }
    }
    pub fn mask(fill: RgbaColor, pattern: MaskPattern) -> Self {
        Self {
            fill_color: Some(RgbaColor {
                alpha: 1.,
                ..fill.normalized()
            }),
            mask_pattern: pattern,
            ..Self::redaction()
        }
    }
    pub fn mask_fill(self) -> RgbaColor {
        RgbaColor {
            alpha: 1.,
            ..self.fill_color.unwrap_or(Self::REDACTION_FILL).normalized()
        }
    }
    pub fn normalized(self) -> Self {
        Self {
            stroke_color: self.stroke_color.normalized(),
            fill_color: self.fill_color.map(RgbaColor::normalized),
            line_width: normalized(self.line_width, 4., 0., 256.),
            opacity: normalized(self.opacity, 1., 0., 1.),
            font_size: normalized(self.font_size, 18., 1., 512.),
            ..self
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationKind {
    Freehand {
        points: Vec<Point>,
    },
    Arrow {
        start: Point,
        end: Point,
    },
    Rectangle(Rect),
    Highlight(Rect),
    Text {
        text: String,
        origin: Point,
        max_width: f32,
    },
    /// Retains the Swift domain name. This is always an opaque mask, never blur.
    Blur(Rect),
}
impl AnnotationKind {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Freehand { points } => bounds_of_points(points),
            Self::Arrow { start, end } => Rect::from_points(*start, *end),
            Self::Rectangle(r) | Self::Highlight(r) | Self::Blur(r) => r.standardized(),
            Self::Text {
                origin, max_width, ..
            } => Rect::new(origin.x, origin.y, *max_width, 48.),
        }
    }
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        match self {
            Self::Freehand { points } => Self::Freehand {
                points: points.iter().map(|p| p.offset(dx, dy)).collect(),
            },
            Self::Arrow { start, end } => Self::Arrow {
                start: start.offset(dx, dy),
                end: end.offset(dx, dy),
            },
            Self::Rectangle(r) => Self::Rectangle(r.offset(dx, dy)),
            Self::Highlight(r) => Self::Highlight(r.offset(dx, dy)),
            Self::Blur(r) => Self::Blur(r.offset(dx, dy)),
            Self::Text {
                text,
                origin,
                max_width,
            } => Self::Text {
                text: text.clone(),
                origin: origin.offset(dx, dy),
                max_width: *max_width,
            },
        }
    }
    fn validate(&self) -> Result<(), String> {
        let valid = match self {
            Self::Freehand { points } => {
                !points.is_empty() && points.len() <= MAX_POINTS && points.iter().all(|p| p.valid())
            }
            Self::Arrow { start, end } => start.valid() && end.valid(),
            Self::Rectangle(r) | Self::Highlight(r) | Self::Blur(r) => {
                r.valid() && r.width != 0. && r.height != 0.
            }
            Self::Text {
                text,
                origin,
                max_width,
            } => {
                !text.is_empty()
                    && text.len() <= MAX_TEXT_BYTES
                    && origin.valid()
                    && max_width.is_finite()
                    && *max_width >= 1.
                    && *max_width <= MAX_COORDINATE
            }
        };
        if valid {
            Ok(())
        } else {
            Err(
                "Annotation geometry or text exceeds the supported bounds; nothing was changed."
                    .into(),
            )
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct EraserStroke {
    pub points: Vec<Point>,
    pub width: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenshotAnnotation {
    pub id: AnnotationId,
    pub kind: AnnotationKind,
    pub style: AnnotationStyle,
    pub erasures: Vec<EraserStroke>,
}
impl ScreenshotAnnotation {
    pub fn new(kind: AnnotationKind, style: AnnotationStyle) -> Self {
        let style = style.normalized();
        let style = if matches!(kind, AnnotationKind::Blur(_)) {
            AnnotationStyle::mask(style.mask_fill(), style.mask_pattern)
        } else {
            style
        };
        Self {
            id: NEXT_ANNOTATION_ID.fetch_add(1, Ordering::Relaxed),
            kind,
            style,
            erasures: vec![],
        }
    }
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        Self {
            id: self.id,
            kind: self.kind.offset(dx, dy),
            style: self.style,
            erasures: self
                .erasures
                .iter()
                .map(|s| EraserStroke {
                    points: s.points.iter().map(|p| p.offset(dx, dy)).collect(),
                    width: s.width,
                })
                .collect(),
        }
    }
    pub fn is_erased(&self) -> bool {
        !self.erasures.is_empty()
    }
    fn byte_cost(&self) -> usize {
        std::mem::size_of::<Self>()
            + match &self.kind {
                AnnotationKind::Freehand { points } => points.len() * 8,
                AnnotationKind::Text { text, .. } => text.len(),
                _ => 0,
            }
            + self
                .erasures
                .iter()
                .map(|s| std::mem::size_of::<EraserStroke>() + s.points.len() * 8)
                .sum::<usize>()
    }
}

#[derive(Clone)]
pub struct ScreenshotDocument {
    base_image: Arc<RgbaImage>,
    annotations: Arc<Vec<ScreenshotAnnotation>>,
    crop_rect: Option<Rect>,
    text_font: Option<Arc<Vec<u8>>>,
}
impl std::fmt::Debug for ScreenshotDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenshotDocument")
            .field("dimensions", &self.dimensions())
            .field("annotation_count", &self.annotations.len())
            .field("crop_rect", &self.crop_rect)
            .finish_non_exhaustive()
    }
}
impl ScreenshotDocument {
    pub fn from_png(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_PNG_BYTES {
            return Err("Screenshot PNG exceeds the 64 MB input limit.".into());
        }
        let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(|e| format!("Could not decode screenshot PNG: {e}"))?;
        let (width, height) = decoder.dimensions();
        validate_image_size(width, height)?;
        let image = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("Could not decode screenshot PNG: {e}"))?
            .to_rgba8();
        Self::from_rgba(image)
    }
    pub fn from_rgba(image: RgbaImage) -> Result<Self, String> {
        validate_image_size(image.width(), image.height())?;
        Ok(Self {
            base_image: Arc::new(image),
            annotations: Arc::new(vec![]),
            crop_rect: None,
            text_font: None,
        })
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.base_image.dimensions()
    }
    pub fn width(&self) -> u32 {
        self.base_image.width()
    }
    pub fn height(&self) -> u32 {
        self.base_image.height()
    }
    pub fn annotations(&self) -> &[ScreenshotAnnotation] {
        &self.annotations
    }
    pub fn crop_rect(&self) -> Option<Rect> {
        self.crop_rect
    }
    pub fn visible_rect(&self) -> Rect {
        self.crop_rect
            .unwrap_or(Rect::new(0., 0., self.width() as f32, self.height() as f32))
    }
    pub fn visible_dimensions(&self) -> (u32, u32) {
        let r = self.visible_rect();
        (r.width as u32, r.height as u32)
    }
    pub fn visible_to_document(&self, point: Point) -> Point {
        let r = self.visible_rect();
        point.offset(r.x, r.y)
    }
    pub fn document_to_visible(&self, point: Point) -> Point {
        let r = self.visible_rect();
        point.offset(-r.x, -r.y)
    }
    pub fn visible_annotations(&self) -> Vec<ScreenshotAnnotation> {
        let r = self.visible_rect();
        self.annotations
            .iter()
            .map(|a| a.offset(-r.x, -r.y))
            .collect()
    }
    pub fn has_text_font(&self) -> bool {
        self.text_font.is_some()
    }
    /// The caller supplies a locally licensed font. Core never discovers fonts,
    /// opens paths, or silently falls back to a different renderer.
    pub fn set_text_font(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut renderer = text::TextRenderer::new(bytes)?;
        for annotation in self.annotations.iter() {
            if let AnnotationKind::Text {
                text,
                origin,
                max_width,
            } = &annotation.kind
            {
                renderer.layout(text, *origin, *max_width, annotation.style)?;
            }
        }
        self.text_font = Some(Arc::new(bytes.to_vec()));
        Ok(())
    }
    pub fn painted_rect(&self, annotation: &ScreenshotAnnotation) -> Result<Rect, String> {
        render::painted_rect(annotation, self.text_font.as_deref().map(Vec::as_slice))
    }
    pub fn hit_test(&self, point: Point, tolerance: f32) -> Option<AnnotationId> {
        if !point.valid() || !tolerance.is_finite() {
            return None;
        }
        // Match compositor order: masks, text, vectors, then highlights.
        for pass in (0..4).rev() {
            for annotation in self
                .annotations
                .iter()
                .rev()
                .filter(|a| render::paint_pass(&a.kind) == pass)
            {
                if is_covered(point, &annotation.erasures) {
                    continue;
                }
                let text_bounds = if matches!(annotation.kind, AnnotationKind::Text { .. }) {
                    self.painted_rect(annotation).ok()
                } else {
                    None
                };
                if brush_touches(point, point, tolerance.max(0.), annotation, text_bounds) {
                    return Some(annotation.id);
                }
            }
        }
        None
    }
    pub fn render_rgba(&self) -> Result<RgbaImage, String> {
        render::render(self, true, None)
    }
    pub fn render_png(&self) -> Result<Vec<u8>, String> {
        render::encode_png(&self.render_rgba()?)
    }
    /// Render the complete masked/annotated crop once, then losslessly split it
    /// into bounded PNGs. Errors return no tiles, never raw or unmasked pixels.
    pub fn render_preview_tiles(&self) -> Result<Vec<PreviewTile>, String> {
        render::encode_preview_tiles(&self.render_rgba()?, MAX_PREVIEW_PNG_BYTES)
    }
    pub fn render_scaled_png(&self, scale: f32) -> Result<Vec<u8>, String> {
        if !scale.is_finite() || scale <= 0. || scale > 4. {
            return Err("Screenshot export scale must be above zero and at most 400%.".into());
        }
        let (w, h) = self.visible_dimensions();
        let w = (w as f32 * scale).max(1.) as u32;
        let h = (h as f32 * scale).max(1.) as u32;
        validate_image_size(w, h)?;
        let raster = self.render_rgba()?;
        let resized = image::imageops::resize(&raster, w, h, image::imageops::FilterType::Lanczos3);
        render::encode_png(&resized)
    }
    /// Safe provider payload: active crop + opaque masks, never decoration.
    /// This only returns bytes. Sending them requires explicit UI confirmation.
    pub fn render_for_external_ocr_png(&self) -> Result<Vec<u8>, String> {
        render::encode_png(&render::render(self, false, None)?)
    }
    pub fn render_for_local_ocr_png(&self) -> Result<Vec<u8>, String> {
        self.render_for_external_ocr_png()
    }
    /// An OCR subregion can further restrict, but never expand, the active crop.
    pub fn render_for_external_ocr_target_png(&self, target: Rect) -> Result<Vec<u8>, String> {
        render::encode_png(&render::render(self, false, Some(target))?)
    }
}
fn validate_image_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        Err(
            "Screenshot exceeds the supported bounds (60,000 px per side, 64 million pixels)."
                .into(),
        )
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq)]
struct Snapshot {
    annotations: Arc<Vec<ScreenshotAnnotation>>,
    crop_rect: Option<Rect>,
}
impl Snapshot {
    fn byte_cost(&self) -> usize {
        self.annotations
            .iter()
            .map(ScreenshotAnnotation::byte_cost)
            .sum::<usize>()
            + std::mem::size_of::<Self>()
    }
}
/// An immutable, cheap, Send + Sync document clone for background rendering.
pub type RenderSnapshot = ScreenshotDocument;

/// One call to `erase_stroke` is one completed drag and one undo step. Include
/// the mouse-up point. All edit methods validate before publishing a new state.
pub struct ScreenshotEditSession {
    document: ScreenshotDocument,
    initial: Snapshot,
    undo: VecDeque<Snapshot>,
    redo: VecDeque<Snapshot>,
    revision: u64,
}
impl ScreenshotEditSession {
    pub fn new(document: ScreenshotDocument) -> Self {
        let initial = Snapshot {
            annotations: document.annotations.clone(),
            crop_rect: document.crop_rect,
        };
        Self {
            document,
            initial,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            revision: 0,
        }
    }
    pub fn document(&self) -> &ScreenshotDocument {
        &self.document
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn render_snapshot(&self) -> RenderSnapshot {
        self.document.clone()
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn has_edits(&self) -> bool {
        self.snapshot() != self.initial
    }
    pub fn has_text_font(&self) -> bool {
        self.document.has_text_font()
    }
    pub fn set_text_font(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.document.set_text_font(bytes)?;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn add_annotation(
        &mut self,
        kind: AnnotationKind,
        style: AnnotationStyle,
    ) -> Result<AnnotationId, String> {
        kind.validate()?;
        if self.document.annotations.len() >= MAX_ANNOTATIONS {
            return Err("Too many screenshot annotations; nothing was changed.".into());
        }
        let annotation = ScreenshotAnnotation::new(kind, style);
        if matches!(annotation.kind, AnnotationKind::Text { .. }) {
            self.document.painted_rect(&annotation)?;
        }
        let id = annotation.id;
        let mut annotations = self.document.annotations.as_ref().clone();
        annotations.push(annotation);
        self.commit(annotations, self.document.crop_rect)?;
        Ok(id)
    }
    pub fn add_visible_annotation(
        &mut self,
        kind: AnnotationKind,
        style: AnnotationStyle,
    ) -> Result<AnnotationId, String> {
        let r = self.document.visible_rect();
        self.add_annotation(kind.offset(r.x, r.y), style)
    }
    pub fn remove_annotation(&mut self, id: AnnotationId) -> Result<bool, String> {
        let mut annotations = self.document.annotations.as_ref().clone();
        let len = annotations.len();
        annotations.retain(|a| a.id != id);
        if annotations.len() == len {
            return Ok(false);
        }
        self.commit(annotations, self.document.crop_rect)?;
        Ok(true)
    }
    pub fn move_annotation(&mut self, id: AnnotationId, dx: f32, dy: f32) -> Result<bool, String> {
        if !valid_coordinate(dx) || !valid_coordinate(dy) {
            return Err("Annotation movement must use finite document pixels.".into());
        }
        if dx == 0. && dy == 0. {
            return Ok(false);
        }
        let mut annotations = self.document.annotations.as_ref().clone();
        let Some(a) = annotations.iter_mut().find(|a| a.id == id) else {
            return Ok(false);
        };
        let moved = a.offset(dx, dy);
        moved.kind.validate()?;
        if moved
            .erasures
            .iter()
            .flat_map(|s| &s.points)
            .any(|p| !p.valid())
        {
            return Err("Annotation movement exceeds the supported bounds.".into());
        }
        *a = moved;
        self.commit(annotations, self.document.crop_rect)?;
        Ok(true)
    }
    pub fn hit_test(&self, point: Point, tolerance: f32) -> Option<AnnotationId> {
        self.document.hit_test(point, tolerance)
    }
    pub fn erase_stroke(&mut self, points: Vec<Point>, width: f32) -> Result<bool, String> {
        if points.is_empty() {
            return Ok(false);
        }
        if points.len() > MAX_POINTS
            || points.iter().any(|p| !p.valid())
            || !width.is_finite()
            || width <= 0.
            || width > 512.
        {
            return Err("Eraser stroke exceeds the supported bounds; nothing was changed.".into());
        }
        let geometry_work = points.len() as u64
            * self
                .document
                .annotations
                .iter()
                .map(|annotation| match &annotation.kind {
                    AnnotationKind::Freehand { points } => points.len() as u64,
                    _ => 4,
                })
                .sum::<u64>();
        if geometry_work > 8_000_000 {
            return Err(
                "Eraser gesture exceeds the bounded geometry budget; nothing was changed.".into(),
            );
        }
        let stroke_cost =
            std::mem::size_of::<EraserStroke>() + points.len() * std::mem::size_of::<Point>();
        let mut byte_cost = self.snapshot().byte_cost();
        let mut annotations = self.document.annotations.as_ref().clone();
        let mut changed = false;
        for annotation in &mut annotations {
            let bounds = if matches!(annotation.kind, AnnotationKind::Text { .. }) {
                Some(self.document.painted_rect(annotation)?)
            } else {
                None
            };
            let touches = if points.len() == 1 {
                brush_touches(points[0], points[0], width / 2., annotation, bounds)
            } else {
                points
                    .windows(2)
                    .any(|p| brush_touches(p[0], p[1], width / 2., annotation, bounds))
            };
            if touches {
                if annotation.erasures.len() >= 4_096 {
                    return Err("Too many eraser strokes; nothing was changed.".into());
                }
                byte_cost = byte_cost
                    .checked_add(stroke_cost)
                    .ok_or("Eraser data exceeds the annotation memory limit.")?;
                if byte_cost > MAX_DOCUMENT_BYTES {
                    return Err(
                        "Eraser data exceeds the 4 MB annotation limit; nothing was changed."
                            .into(),
                    );
                }
                annotation.erasures.push(EraserStroke {
                    points: points.clone(),
                    width,
                });
                changed = true;
            }
        }
        if changed {
            self.commit(annotations, self.document.crop_rect)?;
        }
        Ok(changed)
    }
    pub fn erase_visible_stroke(&mut self, points: Vec<Point>, width: f32) -> Result<bool, String> {
        let r = self.document.visible_rect();
        self.erase_stroke(
            points.into_iter().map(|p| p.offset(r.x, r.y)).collect(),
            width,
        )
    }
    pub fn set_crop(&mut self, crop: Option<Rect>) -> Result<bool, String> {
        let crop = match crop {
            None => None,
            Some(rect) => {
                if !rect.valid() || rect.width == 0. || rect.height == 0. {
                    return Err("Crop must be a finite, non-empty rectangle.".into());
                }
                let full = Rect::new(
                    0.,
                    0.,
                    self.document.width() as f32,
                    self.document.height() as f32,
                );
                Some(
                    rect.intersection(full)
                        .ok_or("Crop does not intersect the screenshot.")?
                        .integral(),
                )
            }
        };
        if self.document.crop_rect == crop {
            return Ok(false);
        }
        self.commit(self.document.annotations.as_ref().clone(), crop)?;
        Ok(true)
    }
    pub fn apply_visible_crop(&mut self, crop: Rect) -> Result<bool, String> {
        if !crop.valid() {
            return Err("Crop must contain finite document pixels.".into());
        }
        let current = self.document.visible_rect();
        let next = crop
            .offset(current.x, current.y)
            .intersection(current)
            .ok_or("Crop does not intersect the visible screenshot.")?;
        self.set_crop(Some(next))
    }
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push_back(self.snapshot());
        self.restore(previous);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop_back() else {
            return false;
        };
        self.undo.push_back(self.snapshot());
        self.restore(next);
        true
    }
    pub fn render_png(&self) -> Result<Vec<u8>, String> {
        self.document.render_png()
    }
    pub fn render_for_external_ocr_png(&self) -> Result<Vec<u8>, String> {
        self.document.render_for_external_ocr_png()
    }
    pub fn render_for_local_ocr_png(&self) -> Result<Vec<u8>, String> {
        self.document.render_for_local_ocr_png()
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            annotations: self.document.annotations.clone(),
            crop_rect: self.document.crop_rect,
        }
    }
    fn restore(&mut self, s: Snapshot) {
        self.document.annotations = s.annotations;
        self.document.crop_rect = s.crop_rect;
        self.revision = self.revision.wrapping_add(1);
    }
    fn commit(
        &mut self,
        annotations: Vec<ScreenshotAnnotation>,
        crop_rect: Option<Rect>,
    ) -> Result<(), String> {
        let next = Snapshot {
            annotations: Arc::new(annotations),
            crop_rect,
        };
        if next.byte_cost() > MAX_DOCUMENT_BYTES {
            return Err(
                "Screenshot annotation data exceeds the 4 MB editing limit; nothing was changed."
                    .into(),
            );
        }
        self.undo.push_back(self.snapshot());
        self.redo.clear();
        self.restore(next);
        while self.undo.len() > MAX_HISTORY_STEPS
            || self.undo.iter().map(Snapshot::byte_cost).sum::<usize>() > MAX_HISTORY_BYTES
        {
            self.undo.pop_front();
        }
        Ok(())
    }
}

fn bounds_of_points(points: &[Point]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::default();
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);
    for p in &points[1..] {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y)
    }
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}
pub fn distance_to_segment(point: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = dx * dx + dy * dy;
    if len <= f32::EPSILON {
        return (point.x - a.x).hypot(point.y - a.y);
    }
    let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / len).clamp(0., 1.);
    (point.x - a.x - t * dx).hypot(point.y - a.y - t * dy)
}
fn segment_distance(a: Point, b: Point, c: Point, d: Point) -> f32 {
    fn cross(a: Point, b: Point, c: Point) -> f32 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }
    let ac = cross(a, b, c);
    let ad = cross(a, b, d);
    let ca = cross(c, d, a);
    let cb = cross(c, d, b);
    if ((ac > 0. && ad < 0.) || (ac < 0. && ad > 0.))
        && ((ca > 0. && cb < 0.) || (ca < 0. && cb > 0.))
    {
        return 0.;
    }
    distance_to_segment(a, c, d)
        .min(distance_to_segment(b, c, d))
        .min(distance_to_segment(c, a, b))
        .min(distance_to_segment(d, a, b))
}
fn rect_corners(rect: Rect) -> [Point; 4] {
    let r = rect.standardized();
    [
        Point::new(r.x, r.y),
        Point::new(r.right(), r.y),
        Point::new(r.right(), r.bottom()),
        Point::new(r.x, r.bottom()),
    ]
}
fn distance_to_rect(a: Point, b: Point, r: Rect) -> f32 {
    if r.contains(a) || r.contains(b) {
        return 0.;
    }
    let corners = rect_corners(r);
    (0..4)
        .map(|i| segment_distance(a, b, corners[i], corners[(i + 1) % 4]))
        .fold(f32::INFINITY, f32::min)
}
fn brush_touches(
    a: Point,
    b: Point,
    radius: f32,
    annotation: &ScreenshotAnnotation,
    text_bounds: Option<Rect>,
) -> bool {
    let half_width = annotation.style.line_width.max(1.) / 2.;
    match &annotation.kind {
        AnnotationKind::Freehand { points } => {
            if points.len() == 1 {
                distance_to_segment(points[0], a, b) <= radius + half_width
            } else {
                points
                    .windows(2)
                    .any(|p| segment_distance(a, b, p[0], p[1]) <= radius + half_width)
            }
        }
        AnnotationKind::Arrow { start, end } => {
            let (p1, p2) = arrow_head(*start, *end, annotation.style.line_width);
            [(*start, *end), (*end, p1), (*end, p2)]
                .into_iter()
                .any(|(c, d)| segment_distance(a, b, c, d) <= radius + half_width)
        }
        AnnotationKind::Rectangle(r) => {
            if annotation.style.fill_color.is_some() {
                distance_to_rect(a, b, *r) <= radius + half_width
            } else {
                let corners = rect_corners(*r);
                (0..4).any(|i| {
                    segment_distance(a, b, corners[i], corners[(i + 1) % 4]) <= radius + half_width
                })
            }
        }
        AnnotationKind::Highlight(r) | AnnotationKind::Blur(r) => {
            distance_to_rect(a, b, *r) <= radius
        }
        AnnotationKind::Text { .. } => {
            text_bounds.is_some_and(|r| distance_to_rect(a, b, r) <= radius)
        }
    }
}
fn is_covered(point: Point, erasures: &[EraserStroke]) -> bool {
    erasures.iter().any(|s| {
        if s.points.len() == 1 {
            distance_to_segment(point, s.points[0], s.points[0]) <= s.width / 2.
        } else {
            s.points
                .windows(2)
                .any(|p| distance_to_segment(point, p[0], p[1]) <= s.width / 2.)
        }
    })
}
fn arrow_head(start: Point, end: Point, line_width: f32) -> (Point, Point) {
    let angle = (end.y - start.y).atan2(end.x - start.x);
    let length = (line_width.max(1.) * 3.).max(10.);
    let spread = std::f32::consts::PI / 7.;
    (
        Point::new(
            end.x - length * (angle - spread).cos(),
            end.y - length * (angle - spread).sin(),
        ),
        Point::new(
            end.x - length * (angle + spread).cos(),
            end.y - length * (angle + spread).sin(),
        ),
    )
}
