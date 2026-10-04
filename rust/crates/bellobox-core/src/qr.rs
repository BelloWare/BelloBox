use qrcode::{EcLevel, QrCode, render::svg};

pub const MAX_QR_BYTES: usize = 2_000;

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
}
