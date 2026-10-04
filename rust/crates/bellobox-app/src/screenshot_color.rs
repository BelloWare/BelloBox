//! Opaque screenshot color controls. Alpha is never accepted from UI input.
use bellobox_core::screenshot::RgbaColor;

pub fn parse_hex(text: &str) -> Result<RgbaColor, &'static str> {
    let text = text.trim();
    let digits = text.strip_prefix('#').unwrap_or(text);
    if digits.len() != 6 || !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Use six hexadecimal digits, such as #F26B14. Colors are always opaque.");
    }
    let n = u32::from_str_radix(digits, 16).map_err(|_| "Invalid RGB color.")?;
    Ok(RgbaColor::from_rgb8(
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
    ))
}
/// Opening a color well formats RGB for display; equivalent input must not
/// quantize the original source palette's floating-point channels.
pub fn updated_from_hex(current: RgbaColor, text: &str) -> Result<RgbaColor, &'static str> {
    let parsed = parse_hex(text)?;
    Ok(if parsed.rgba8() == current.rgba8() {
        current
    } else {
        parsed
    })
}
pub fn hex(color: RgbaColor) -> String {
    let [r, g, b, _] = color.rgba8();
    format!("#{r:02X}{g:02X}{b:02X}")
}
pub fn set_channel(color: RgbaColor, index: usize, value: f32) -> RgbaColor {
    let mut c = color.normalized();
    let value = if value.is_finite() {
        value.clamp(0., 255.).round() / 255.
    } else {
        0.
    };
    match index {
        0 => c.red = value,
        1 => c.green = value,
        2 => c.blue = value,
        _ => {}
    }
    c.alpha = 1.;
    c
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_colors_round_trip_all_channels_opaque() {
        for rgb in [0x000000, 0xffffff, 0xf26b14, 0x0066cc, 0xaa00ff] {
            let source = format!("#{rgb:06X}");
            let color = parse_hex(&source).unwrap();
            assert_eq!(hex(color), source);
            assert_eq!(color.alpha, 1.);
        }
    }
    #[test]
    fn alpha_nonascii_and_malformed_inputs_are_rejected() {
        for input in [
            "#abcd",
            "#aabbcc00",
            "rgb(1,2,3)",
            "１２３４５６",
            "#xyzxyz",
            "#F26B14\nmore",
            "",
        ] {
            assert!(parse_hex(input).is_err());
        }
    }
    #[test]
    fn channel_updates_are_bounded_and_never_translucent() {
        let c = RgbaColor::new(0.5, 0.5, 0.5, 0.01);
        assert_eq!(set_channel(c, 0, 300.).red, 1.);
        assert_eq!(set_channel(c, 1, -1.).green, 0.);
        assert_eq!(set_channel(c, 2, f32::NAN).blue, 0.);
        assert_eq!(set_channel(c, 0, 12.).alpha, 1.);
        assert_eq!(hex(set_channel(c, 0, 12.)), "#0C8080");
    }

    #[test]
    fn opening_well_preserves_original_source_channels() {
        let source = RgbaColor::new(0.95, 0.42, 0.08, 1.);
        assert_eq!(updated_from_hex(source, &hex(source)).unwrap(), source);
        assert_eq!(
            updated_from_hex(source, "#00FF80").unwrap().rgba8(),
            [0, 255, 128, 255]
        );
    }
}
