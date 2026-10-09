use qrcode::{EcLevel, QrCode, render::svg};

pub const MAX_QR_BYTES: usize = 2_000;

const COMPACT_CARD_POINTS: u32 = 128;
const CARD_INSET_POINTS: u32 = 6;
const CARD_PIXELS_PER_POINT: u32 = 2;

/// The current payload's bounded palette rasters and full-resolution export.
///
/// Card rasters have integral physical pixels per module at two pixels per
/// logical point. Center them on a white card at `*_pixels / 2.0` logical points;
/// stretching them to fill the card would lose the integral module geometry.
/// These 2x rasters do not guarantee integral module pixels on a 1x display.
#[derive(Debug)]
pub struct Palette {
    /// Modules per side, including the four-module quiet zone on each edge.
    pub modules: usize,
    pub compact_png: Vec<u8>,
    /// Physical pixel width and height of the compact raster.
    pub compact_pixels: u32,
    pub enlarged_png: Vec<u8>,
    /// Physical pixel width and height of the enlarged raster.
    pub enlarged_pixels: u32,
    /// Enlarged white-card side in logical points, within 192..=384.
    pub enlarged_side: u32,
    /// Whether the compact inner card has fewer than two points per module.
    pub dense: bool,
    pub export_png: Vec<u8>,
}

/// Encode one current palette payload. Call off the UI thread after cancellation
/// and generation checks; no payload or historical images are retained here.
pub fn palette(text: &str) -> Result<Palette, String> {
    if text.len() > MAX_QR_BYTES || text.trim().is_empty() {
        return Err("QR content must be 1–2,000 UTF-8 bytes.".into());
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| e.to_string())?;
    let modules = code.width() + 8;
    let enlarged_side = enlarged_card_side(modules as u32);
    let (compact_png, compact_pixels) = card_png(&code, COMPACT_CARD_POINTS)?;
    let (enlarged_png, enlarged_pixels) = card_png(&code, enlarged_side)?;
    Ok(Palette {
        modules,
        compact_png,
        compact_pixels,
        enlarged_png,
        enlarged_pixels,
        enlarged_side,
        dense: modules * 2 > (COMPACT_CARD_POINTS - 2 * CARD_INSET_POINTS) as usize,
        export_png: png(text)?,
    })
}

fn enlarged_card_side(modules: u32) -> u32 {
    ((modules * 2 + 2 * CARD_INSET_POINTS + 8).div_ceil(8) * 8).clamp(192, 384)
}

fn card_png(code: &QrCode, card_points: u32) -> Result<(Vec<u8>, u32), String> {
    let modules = code.width() as u32 + 8;
    let budget = (card_points - 2 * CARD_INSET_POINTS) * CARD_PIXELS_PER_POINT;
    // The compact budget is 232 pixels, larger than every supported QR version
    // including its quiet zone. Every accepted payload fits at one physical
    // pixel per module or better.
    let scale = (budget / modules).max(1);
    Ok((render_png(code, scale)?, modules * scale))
}

/// Medium error correction and a four-module quiet zone, with integral module sizing.
pub fn svg(text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Enter text or a URL to make a QR code.".into());
    }
    if text.len() > MAX_QR_BYTES {
        return Err("QR content exceeds 2,000 UTF-8 bytes.".into());
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| e.to_string())?;
    let modules = code.width() + 8;
    let scale = 512_usize.div_ceil(modules).max(4);
    Ok(code
        .render::<svg::Color>()
        .quiet_zone(true)
        .module_dimensions(scale as u32, scale as u32)
        .build())
}

/// Lossless PNG export with a full quiet zone and at least four pixels per module.
pub fn png(text: &str) -> Result<Vec<u8>, String> {
    if text.trim().is_empty() || text.len() > MAX_QR_BYTES {
        return Err("QR content must be 1–2,000 UTF-8 bytes.".into());
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| e.to_string())?;
    let modules = code.width() + 8;
    let scale = 512_usize.div_ceil(modules).max(4) as u32;
    render_png(&code, scale)
}

fn render_png(code: &QrCode, scale: u32) -> Result<Vec<u8>, String> {
    let pixels = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(scale, scale)
        .build();
    let mut data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(pixels)
        .write_to(&mut data, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(data.into_inner())
}

/// Accessible console preview; the export is a real QR, not a decorative placeholder.
pub fn terminal(text: &str) -> Result<String, String> {
    if text.trim().is_empty() || text.len() > MAX_QR_BYTES {
        return Err("QR content must be 1–2,000 UTF-8 bytes.".into());
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| e.to_string())?;
    Ok(code
        .render::<char>()
        .quiet_zone(true)
        .dark_color('█')
        .light_color(' ')
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_has_real_geometry_and_quiet_zone() {
        let result = svg("https://belloware.com").unwrap();
        assert!(result.contains("<svg"));
        assert!(result.contains("<path"));
        assert!(result.contains("viewBox"));
    }
    #[test]
    fn png_is_crisp_with_white_quiet_zone() {
        let data = png("https://belloware.com").unwrap();
        let image = image::load_from_memory(&data).unwrap().into_luma8();
        assert!(image.width() >= 512);
        assert_eq!(image.width(), image.height());
        assert!(image.pixels().all(|p| p[0] == 0 || p[0] == 255));
        for x in 0..image.width() {
            assert_eq!(image.get_pixel(x, 0)[0], 255);
        }
    }
    #[test]
    fn limits_are_bytes_not_characters() {
        assert!(svg("").is_err());
        assert!(svg(&"界".repeat(667)).is_err());
        assert!(svg(&"x".repeat(2_000)).is_ok());
    }

    fn assert_card_pixels(data: &[u8], pixels: u32, code: &QrCode, card_points: u32) {
        let image = image::load_from_memory(data).unwrap().into_luma8();
        let modules = code.width() as u32 + 8;
        let budget = (card_points - 2 * CARD_INSET_POINTS) * CARD_PIXELS_PER_POINT;
        let scale = budget / modules;
        assert!(scale >= 1);
        assert_eq!(pixels, modules * scale);
        assert_eq!(image.dimensions(), (pixels, pixels));
        assert!(pixels <= budget);

        // Check every raster pixel against the actual encoder's module matrix,
        // including all four complete quiet-zone edges and integral blocks.
        for (x, y, pixel) in image.enumerate_pixels() {
            let module_x = x / scale;
            let module_y = y / scale;
            let expected = if module_x < 4
                || module_y < 4
                || module_x >= modules - 4
                || module_y >= modules - 4
            {
                255
            } else if code[((module_x - 4) as usize, (module_y - 4) as usize)]
                == qrcode::Color::Dark
            {
                0
            } else {
                255
            };
            assert_eq!(pixel[0], expected, "pixel ({x}, {y})");
        }
    }

    #[test]
    fn palette_rasters_preserve_modules_and_fit_both_cards() {
        for input in [
            "x".to_string(),
            "https://belloware.com".into(),
            "synthetic dense payload ".repeat(30),
            "界🙂".repeat(200),
            "x".repeat(MAX_QR_BYTES),
        ] {
            let result = palette(&input).unwrap();
            let code = QrCode::with_error_correction_level(input.as_bytes(), EcLevel::M).unwrap();
            assert_eq!(result.modules, code.width() + 8);
            assert_eq!(result.dense, result.modules * 2 > 116);
            assert!((192..=384).contains(&result.enlarged_side));
            assert_eq!(result.enlarged_side % 8, 0);
            assert_eq!(
                result.enlarged_side,
                ((result.modules as u32 * 2 + 20).div_ceil(8) * 8).clamp(192, 384)
            );
            assert_card_pixels(
                &result.compact_png,
                result.compact_pixels,
                &code,
                COMPACT_CARD_POINTS,
            );
            assert_card_pixels(
                &result.enlarged_png,
                result.enlarged_pixels,
                &code,
                result.enlarged_side,
            );
            assert_eq!(result.export_png, png(&input).unwrap());
        }
    }

    #[test]
    fn palette_density_and_enlargement_cover_sparse_and_dense_payloads() {
        let sparse = palette("x").unwrap();
        assert!(!sparse.dense);
        assert_eq!(sparse.enlarged_side, 192);

        let dense = palette(&"x".repeat(MAX_QR_BYTES)).unwrap();
        assert!(dense.dense);
        assert!(dense.modules > 116);
        assert!(dense.compact_pixels <= 232);
        assert!(dense.enlarged_pixels > dense.compact_pixels);
        assert!(dense.enlarged_pixels / dense.modules as u32 >= 4);
    }

    #[test]
    fn enlarged_card_rounds_up_and_clamps_both_ends() {
        assert_eq!(enlarged_card_side(29), 192);
        assert_eq!(enlarged_card_side(87), 200);
        assert_eq!(enlarged_card_side(177), 376);
        assert_eq!(enlarged_card_side(185), 384);
    }

    #[test]
    fn palette_limits_reject_blank_and_oversize_without_truncation() {
        for input in [
            "".to_string(),
            " \n\t".into(),
            "x".repeat(MAX_QR_BYTES + 1),
            "界".repeat(667),
        ] {
            assert!(palette(&input).is_err());
        }
        let exact_bytes = format!("{}xx", "界".repeat(666));
        assert_eq!(exact_bytes.len(), MAX_QR_BYTES);
        assert!(palette(&exact_bytes).is_ok());
        assert_ne!(
            palette(" x ").unwrap().export_png,
            palette("x").unwrap().export_png
        );
    }
}
