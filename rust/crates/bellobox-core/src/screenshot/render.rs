use super::*;
use image::ImageEncoder;
use tiny_skia::{
    BlendMode, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke,
    Transform,
};

const TILE_SIZE: u32 = 512;
const MAX_RENDER_OVERDRAW: u64 = 256_000_000;
const MAX_PATH_TILE_WORK: u64 = 8_000_000;

pub(super) fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    encode_png_with_limit(
        image,
        MAX_PNG_BYTES,
        "PNG export exceeds the 64 MB encoded-image limit",
    )
}

fn encode_png_with_limit(
    image: &RgbaImage,
    max_bytes: usize,
    limit_message: &'static str,
) -> Result<Vec<u8>, String> {
    struct BoundedPng {
        bytes: Vec<u8>,
        max_bytes: usize,
        limit_message: &'static str,
        exceeded_limit: bool,
    }
    impl std::io::Write for BoundedPng {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.exceeded_limit || buf.len() > self.max_bytes.saturating_sub(self.bytes.len()) {
                self.exceeded_limit = true;
                return Err(std::io::Error::other(self.limit_message));
            }
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = BoundedPng {
        bytes: Vec::new(),
        max_bytes,
        limit_message,
        exceeded_limit: false,
    };
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Could not encode annotated PNG: {e}"))?;
    // The encoder may write its final IEND chunk during Drop, where write
    // failures cannot be returned. Never publish an apparently successful but
    // truncated PNG if any write (including that final chunk) was rejected.
    if bytes.exceeded_limit {
        return Err(format!("Could not encode annotated PNG: {limit_message}"));
    }
    Ok(bytes.bytes)
}

/// This accepts only an already completed render; no tile can bypass masking or
/// recover from a compositor failure by exposing the original screenshot.
pub(super) fn encode_preview_tiles(
    image: &RgbaImage,
    max_bytes: usize,
) -> Result<Vec<PreviewTile>, String> {
    validate_image_size(image.width(), image.height())?;
    let mut remaining_bytes = max_bytes.min(MAX_PREVIEW_PNG_BYTES);
    let mut tiles = Vec::new();
    for y in (0..image.height()).step_by(MAX_PREVIEW_TILE_EDGE as usize) {
        for x in (0..image.width()).step_by(MAX_PREVIEW_TILE_EDGE as usize) {
            let width = MAX_PREVIEW_TILE_EDGE.min(image.width() - x);
            let height = MAX_PREVIEW_TILE_EDGE.min(image.height() - y);
            let tile = image::imageops::crop_imm(image, x, y, width, height).to_image();
            // Bound each encoder by the *remaining* aggregate allowance, not a
            // separate allowance per tile. A late failure drops all earlier PNGs.
            let png = encode_png_with_limit(
                &tile,
                remaining_bytes,
                "Screenshot preview exceeds the 64 MB cumulative encoded-preview limit",
            )?;
            remaining_bytes -= png.len();
            tiles.push(PreviewTile {
                x,
                y,
                width,
                height,
                png,
            });
        }
    }
    Ok(tiles)
}
pub(super) fn paint_pass(kind: &AnnotationKind) -> u8 {
    match kind {
        AnnotationKind::Highlight(_) => 0,
        AnnotationKind::Freehand { .. }
        | AnnotationKind::Arrow { .. }
        | AnnotationKind::Rectangle(_) => 1,
        AnnotationKind::Text { .. } => 2,
        AnnotationKind::Blur(_) => 3,
    }
}
pub(super) fn painted_rect(
    annotation: &ScreenshotAnnotation,
    font: Option<&[u8]>,
) -> Result<Rect, String> {
    annotation.kind.validate()?;
    let style = annotation.style.normalized();
    let width = style.line_width.max(1.);
    Ok(match &annotation.kind {
        AnnotationKind::Freehand { .. } => annotation.kind.bounds().expanded(width),
        AnnotationKind::Arrow { .. } => annotation
            .kind
            .bounds()
            .expanded((width * 3.).max(10.) + width),
        AnnotationKind::Rectangle(r) => r.standardized().expanded(width),
        AnnotationKind::Highlight(r) | AnnotationKind::Blur(r) => r.standardized(),
        AnnotationKind::Text {
            text,
            origin,
            max_width,
        } => {
            let mut renderer = text::TextRenderer::new(font.ok_or(
                "Text annotations need an explicitly supplied local font before preview or export.",
            )?)?;
            renderer.layout(text, *origin, *max_width, style)?.rect
        }
    })
}

struct Prepared<'a> {
    annotation: &'a ScreenshotAnnotation,
    bounds: Rect,
    path: Option<Path>,
    text: Option<Pixmap>,
    erasures: Vec<(Option<Path>, Option<Point>, f32)>,
}
impl<'a> Prepared<'a> {
    fn new(
        annotation: &'a ScreenshotAnnotation,
        text_renderer: &mut Option<text::TextRenderer>,
    ) -> Result<Self, String> {
        let mut raster = None;
        let mut path = None;
        let bounds = match &annotation.kind {
            AnnotationKind::Text {
                text,
                origin,
                max_width,
            } => {
                let renderer=text_renderer.as_mut().ok_or("Text annotations need an explicitly supplied local font before preview or export.")?;
                let layout = renderer.layout(text, *origin, *max_width, annotation.style)?;
                raster = Some(renderer.raster(&layout, annotation.style)?);
                layout.rect
            }
            AnnotationKind::Freehand { points } => {
                path = polyline(points);
                painted_rect(annotation, None)?
            }
            AnnotationKind::Arrow { start, end } => {
                let mut b = PathBuilder::new();
                let (p1, p2) = arrow_head(*start, *end, annotation.style.line_width);
                for (a, z) in [(*start, *end), (*end, p1), (*end, p2)] {
                    b.move_to(a.x, a.y);
                    b.line_to(z.x, z.y)
                }
                path = b.finish();
                painted_rect(annotation, None)?
            }
            AnnotationKind::Rectangle(r) => {
                let mut b = PathBuilder::new();
                b.push_rect(skia_rect(r.standardized())?);
                path = b.finish();
                painted_rect(annotation, None)?
            }
            _ => painted_rect(annotation, None)?,
        };
        let erasures = annotation
            .erasures
            .iter()
            .map(|e| {
                (
                    polyline(&e.points),
                    (e.points.len() == 1).then(|| e.points[0]),
                    e.width,
                )
            })
            .collect();
        Ok(Self {
            annotation,
            bounds,
            path,
            text: raster,
            erasures,
        })
    }
    fn paint(&self, tile: &mut Pixmap, tile_x: f32, tile_y: f32) -> Result<(), String> {
        let annotation = self.annotation;
        let style = annotation.style.normalized();
        let transform = Transform::from_translate(-tile_x, -tile_y);
        let stroke = Stroke {
            width: style.line_width.max(1.),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        match &annotation.kind {
            AnnotationKind::Blur(rect) => draw_mask(tile, *rect, style, tile_x, tile_y),
            AnnotationKind::Highlight(rect) => tile.fill_rect(
                skia_rect(rect.standardized())?,
                &paint(
                    style
                        .fill_color
                        .unwrap_or(AnnotationStyle::highlight().stroke_color),
                    style.opacity,
                ),
                transform,
                None,
            ),
            AnnotationKind::Rectangle(rect) => {
                if let Some(color) = style.fill_color {
                    tile.fill_rect(
                        skia_rect(rect.standardized())?,
                        &paint(color, style.opacity),
                        transform,
                        None,
                    )
                }
                if let Some(path) = &self.path {
                    tile.stroke_path(
                        path,
                        &paint(style.stroke_color, style.opacity),
                        &stroke,
                        transform,
                        None,
                    )
                }
            }
            AnnotationKind::Freehand { points } if points.len() == 1 => draw_dot(
                tile,
                points[0],
                stroke.width,
                &paint(style.stroke_color, style.opacity),
                transform,
            ),
            AnnotationKind::Freehand { .. } | AnnotationKind::Arrow { .. } => {
                if let Some(path) = &self.path {
                    tile.stroke_path(
                        path,
                        &paint(style.stroke_color, style.opacity),
                        &stroke,
                        transform,
                        None,
                    )
                }
            }
            AnnotationKind::Text { origin, .. } => {
                if let Some(raster) = &self.text {
                    tile.draw_pixmap(
                        0,
                        0,
                        raster.as_ref(),
                        &PixmapPaint::default(),
                        Transform::from_translate(origin.x - tile_x, origin.y - tile_y),
                        None,
                    );
                }
            }
        }
        if !self.erasures.is_empty() {
            let mut erase = paint(RgbaColor::new(0., 0., 0., 1.), 1.);
            erase.blend_mode = BlendMode::DestinationOut;
            for (path, point, width) in &self.erasures {
                if let Some(point) = point {
                    draw_dot(tile, *point, width.max(1.), &erase, transform)
                } else if let Some(path) = path {
                    tile.stroke_path(
                        path,
                        &erase,
                        &Stroke {
                            width: width.max(1.),
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Round,
                            ..Stroke::default()
                        },
                        transform,
                        None,
                    )
                }
            }
        }
        Ok(())
    }
}

pub(super) fn render(
    document: &ScreenshotDocument,
    decorative: bool,
    target: Option<Rect>,
) -> Result<RgbaImage, String> {
    let mut crop = document.visible_rect();
    if let Some(target) = target {
        if !target.valid() {
            return Err("OCR crop must contain finite document pixels.".into());
        }
        crop = crop
            .intersection(target)
            .ok_or("OCR crop does not intersect the visible screenshot.")?
            .integral();
    }
    let width = crop.width as u32;
    let height = crop.height as u32;
    validate_image_size(width, height)?;
    let mut text_renderer = None;
    if decorative
        && document
            .annotations
            .iter()
            .any(|a| matches!(a.kind, AnnotationKind::Text { .. }))
    {
        text_renderer = Some(text::TextRenderer::new(
            document.text_font.as_deref().map(Vec::as_slice).ok_or(
                "Text annotations need an explicitly supplied local font before preview or export.",
            )?,
        )?);
    }
    // Reject excessive work before allocating the output or emitting any bytes.
    let mut overdraw = 0_u64;
    let mut path_work = 0_u64;
    let mut text_pixels = 0_u64;
    let mut prepared = Vec::new();
    for pass in 0..4 {
        if !decorative && pass != 3 {
            continue;
        }
        for annotation in document
            .annotations
            .iter()
            .filter(|a| paint_pass(&a.kind) == pass)
        {
            annotation.kind.validate()?;
            let p = Prepared::new(annotation, &mut text_renderer)?;
            if let Some(raster) = &p.text {
                text_pixels += u64::from(raster.width()) * u64::from(raster.height());
                if text_pixels > 32_000_000 {
                    return Err(
                        "Annotation text exceeds the aggregate rendering memory limit.".into(),
                    );
                }
            }
            if let Some(clip) = p.bounds.integral().intersection(crop) {
                let w = clip.width as u64;
                let h = clip.height as u64;
                overdraw += w * h;
                let points = match &annotation.kind {
                    AnnotationKind::Freehand { points } => points.len(),
                    _ => 4,
                } + annotation
                    .erasures
                    .iter()
                    .map(|e| e.points.len())
                    .sum::<usize>();
                path_work += w.div_ceil(u64::from(TILE_SIZE))
                    * h.div_ceil(u64::from(TILE_SIZE))
                    * points as u64;
                if overdraw > MAX_RENDER_OVERDRAW || path_work > MAX_PATH_TILE_WORK {
                    return Err("Screenshot annotations exceed the bounded rendering budget; no image was exported.".into());
                }
                prepared.push(p);
            }
        }
    }
    let mut output = image::imageops::crop_imm(
        document.base_image.as_ref(),
        crop.x as u32,
        crop.y as u32,
        width,
        height,
    )
    .to_image();
    for prepared in prepared {
        let clip = prepared
            .bounds
            .integral()
            .intersection(crop)
            .ok_or("Invalid annotation rendering bounds.")?;
        let min_x = clip.x as u32;
        let min_y = clip.y as u32;
        let max_x = clip.right() as u32;
        let max_y = clip.bottom() as u32;
        for y in (min_y..max_y).step_by(TILE_SIZE as usize) {
            for x in (min_x..max_x).step_by(TILE_SIZE as usize) {
                let tw = TILE_SIZE.min(max_x - x);
                let th = TILE_SIZE.min(max_y - y);
                let mut tile =
                    Pixmap::new(tw, th).ok_or("Could not allocate screenshot annotation tile.")?;
                prepared.paint(&mut tile, x as f32, y as f32)?;
                composite(&mut output, &tile, x - crop.x as u32, y - crop.y as u32);
            }
        }
    }
    Ok(output)
}
fn paint(color: RgbaColor, opacity: f32) -> Paint<'static> {
    let c = color.normalized();
    let mut p = Paint::default();
    p.set_color_rgba8(
        (c.red * 255.).round() as u8,
        (c.green * 255.).round() as u8,
        (c.blue * 255.).round() as u8,
        (c.alpha * opacity * 255.).round() as u8,
    );
    p.anti_alias = true;
    p
}
fn skia_rect(rect: Rect) -> Result<tiny_skia::Rect, String> {
    tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
        .ok_or("Invalid annotation rendering rectangle.".into())
}
fn polyline(points: &[Point]) -> Option<Path> {
    if points.len() < 2 {
        return None;
    }
    let mut b = PathBuilder::new();
    b.move_to(points[0].x, points[0].y);
    for p in &points[1..] {
        b.line_to(p.x, p.y)
    }
    b.finish()
}
fn draw_dot(tile: &mut Pixmap, p: Point, width: f32, paint: &Paint<'_>, transform: Transform) {
    if let Some(path) = PathBuilder::from_circle(p.x, p.y, width / 2.) {
        tile.fill_path(&path, paint, FillRule::Winding, transform, None)
    }
}
/// Masks round their outer boundary outwards to full pixels so a fractional
/// redaction edge cannot blend protected pixels into an export. Patterns are
/// anchored to the rectangle and computed per tile, never to a crop viewport.
fn draw_mask(tile: &mut Pixmap, rect: Rect, style: AnnotationStyle, tile_x: f32, tile_y: f32) {
    let rect = rect.standardized();
    let coverage = rect.integral();
    let fill = style.mask_fill();
    let [r, g, b, _] = fill.rgba8();
    let luminance = 0.2126 * fill.red + 0.7152 * fill.green + 0.0722 * fill.blue;
    let (ink, alpha) = if luminance > 0.55 {
        (0., 0.18)
    } else {
        (255., 0.16)
    };
    for y in 0..tile.height() {
        for x in 0..tile.width() {
            let gx = tile_x + x as f32 + 0.5;
            let gy = tile_y + y as f32 + 0.5;
            if gx < coverage.x
                || gx >= coverage.right()
                || gy < coverage.y
                || gy >= coverage.bottom()
            {
                continue;
            }
            let pattern = match style.mask_pattern {
                MaskPattern::Solid => false,
                // Swift's y-up stripes begin at rect's bottom edge and head right.
                MaskPattern::Stripes => {
                    let start = (gx - rect.x) - (rect.bottom() - gy);
                    start >= 0. && (start.rem_euclid(8.)).min(8. - start.rem_euclid(8.)) <= 0.7
                }
                MaskPattern::Dots => {
                    let dx = (gx - rect.x - 4.).rem_euclid(8.);
                    let dy = (rect.bottom() - gy - 4.).rem_euclid(8.);
                    let dx = dx.min(8. - dx);
                    let dy = dy.min(8. - dy);
                    dx * dx + dy * dy <= 1.25 * 1.25
                }
            };
            let mix = |c: u8| {
                if pattern {
                    (c as f32 * (1. - alpha) + ink * alpha).round() as u8
                } else {
                    c
                }
            };
            let idx = ((y * tile.width() + x) * 4) as usize;
            tile.data_mut()[idx..idx + 4].copy_from_slice(&[mix(r), mix(g), mix(b), 255]);
        }
    }
}
fn composite(output: &mut RgbaImage, tile: &Pixmap, x: u32, y: u32) {
    for ty in 0..tile.height() {
        for tx in 0..tile.width() {
            let source = tile.pixels()[(ty * tile.width() + tx) as usize];
            let sa = u32::from(source.alpha());
            if sa == 0 {
                continue;
            }
            let target = output.get_pixel_mut(x + tx, y + ty);
            let da = u32::from(target[3]);
            let out_alpha = sa + (da * (255 - sa) + 127) / 255;
            for (i, src) in [source.red(), source.green(), source.blue()]
                .into_iter()
                .enumerate()
            {
                let numerator =
                    u32::from(src) * 255 + (u32::from(target[i]) * da * (255 - sa) + 127) / 255;
                target[i] = ((numerator + out_alpha / 2) / out_alpha).min(255) as u8;
            }
            target[3] = out_alpha as u8;
        }
    }
}
