//! Explicit-font text shaping. Platform fonts are chosen by the app, never read
//! here. The portable renderer is not an exact AppKit font-metrics substitute.
use super::*;
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap, fontdb,
};
use tiny_skia::{Pixmap, PremultipliedColorU8};

const MAX_FONT_BYTES: usize = 16_000_000;
const MAX_TEXT_PIXELS: u64 = 8_000_000;
pub(super) struct TextRenderer {
    fonts: FontSystem,
    cache: SwashCache,
    family: String,
}
pub(super) struct TextLayout {
    buffer: Buffer,
    pub rect: Rect,
}
impl TextRenderer {
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_FONT_BYTES {
            return Err("Annotation font must be a supported local font of at most 16 MB.".into());
        }
        let mut db = fontdb::Database::new();
        db.load_font_data(bytes.to_vec());
        let family = db
            .faces()
            .next()
            .and_then(|f| f.families.first().map(|(name, _)| name.clone()))
            .ok_or("Could not read the supplied annotation font.")?;
        db.set_sans_serif_family(&family);
        Ok(Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            cache: SwashCache::new(),
            family,
        })
    }
    pub fn layout(
        &mut self,
        text: &str,
        origin: Point,
        max_width: f32,
        style: AnnotationStyle,
    ) -> Result<TextLayout, String> {
        if text.len() > MAX_TEXT_BYTES
            || !origin.valid()
            || !max_width.is_finite()
            || max_width < 1.
        {
            return Err("Text annotation exceeds the supported bounds.".into());
        }
        let mut buffer = Buffer::new(
            &mut self.fonts,
            Metrics::new(style.font_size, style.font_size * 1.4),
        );
        buffer.set_size(&mut self.fonts, Some(max_width), None);
        buffer.set_wrap(&mut self.fonts, Wrap::WordOrGlyph);
        buffer.set_text(
            &mut self.fonts,
            text,
            &Attrs::new()
                .family(Family::Name(&self.family))
                .weight(Weight::SEMIBOLD),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.fonts, false);
        let mut height = (style.font_size * 1.4).ceil();
        for run in buffer.layout_runs() {
            if run.glyphs.iter().any(|g| g.glyph_id == 0) {
                return Err("The supplied annotation font does not support every character. Choose a font with the needed glyphs before adding or exporting this text.".into());
            }
            height = height.max((run.line_top + run.line_height).ceil() + 4.);
        }
        if max_width > MAX_IMAGE_DIMENSION as f32
            || height > MAX_IMAGE_DIMENSION as f32
            || max_width.ceil() as u64 * height.ceil() as u64 > MAX_TEXT_PIXELS
        {
            return Err(
                "Wrapped annotation text exceeds the 8 million pixel text-rendering limit.".into(),
            );
        }
        Ok(TextLayout {
            buffer,
            rect: Rect::new(origin.x, origin.y, max_width, height),
        })
    }
    pub fn raster(
        &mut self,
        layout: &TextLayout,
        style: AnnotationStyle,
    ) -> Result<Pixmap, String> {
        let width = layout.rect.width.ceil() as u32;
        let height = layout.rect.height.ceil() as u32;
        let mut pixmap =
            Pixmap::new(width, height).ok_or("Could not allocate annotation text image.")?;
        let [r, g, b, _] = style.stroke_color.rgba8();
        // AppKit's text path overrides the stroke color alpha with style opacity.
        let color = Color::rgba(r, g, b, (style.opacity * 255.).round() as u8);
        // A missing raster for a visible glyph must not silently disappear.
        for run in layout.buffer.layout_runs() {
            for glyph in run.glyphs {
                let content = run.text.get(glyph.start..glyph.end).unwrap_or("");
                if !content.chars().all(char::is_whitespace) {
                    let key = glyph.physical((0., 0.), 1.).cache_key;
                    if self.cache.get_image(&mut self.fonts, key).is_none() {
                        return Err(
                            "Could not rasterize an annotation glyph with the supplied font."
                                .into(),
                        );
                    }
                }
            }
        }
        layout.buffer.draw(
            &mut self.fonts,
            &mut self.cache,
            color,
            |x, y, _, _, color| {
                if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                    return;
                }
                let [r, g, b, a] = color.as_rgba();
                let idx = y as usize * width as usize + x as usize;
                let target = pixmap.pixels()[idx];
                let over = |value: u8, old: u8| -> u8 {
                    ((u32::from(value) * u32::from(a)
                        + u32::from(old) * (255 - u32::from(a))
                        + 127)
                        / 255)
                        .min(255) as u8
                };
                let oa = (u32::from(a)
                    + (u32::from(target.alpha()) * (255 - u32::from(a)) + 127) / 255)
                    .min(255) as u8;
                pixmap.pixels_mut()[idx] = PremultipliedColorU8::from_rgba(
                    over(r, target.red()),
                    over(g, target.green()),
                    over(b, target.blue()),
                    oa,
                )
                .unwrap_or(PremultipliedColorU8::TRANSPARENT);
            },
        );
        Ok(pixmap)
    }
}
