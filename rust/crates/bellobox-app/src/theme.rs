//! Bello Box's source design tokens, mirrored from `UI/Theme.swift`.
use bellobox_core::settings::Appearance;
use gpui::{Hsla, Rgba, Window, WindowAppearance};
use std::sync::atomic::{AtomicU8, Ordering};

static SAVED_APPEARANCE: AtomicU8 = AtomicU8::new(0);

/// The preference is process-wide so every existing tool resolves the same ink.
/// Updating it does not recreate editors or their native selection/undo state.
pub fn set_saved_appearance(appearance: Appearance) {
    SAVED_APPEARANCE.store(
        match appearance {
            Appearance::System => 0,
            Appearance::Light => 1,
            Appearance::Dark => 2,
        },
        Ordering::Relaxed,
    );
}

// The complete common palette is kept as additional tool windows are ported.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Hsla,
    pub surface: Hsla,
    pub well: Hsla,
    pub border: Hsla,
    pub separator: Hsla,
    pub primary: Hsla,
    pub secondary: Hsla,
    pub accent: Hsla,
    pub accent_fill: Hsla,
    pub success: Hsla,
    pub danger: Hsla,
    pub teal: Hsla,
    pub brand: Hsla,
}

fn color(r: f32, g: f32, b: f32) -> Hsla {
    Rgba { r, g, b, a: 1. }.into()
}

pub fn opacity(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

pub fn for_window(window: &Window) -> Palette {
    // Fixture overrides affect rendering only, never the saved preference.
    let dark = resolve_dark(
        std::env::var("BELLOBOX_APPEARANCE").ok().as_deref(),
        SAVED_APPEARANCE.load(Ordering::Relaxed),
        matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    );
    for_dark(dark)
}

fn resolve_dark(fixture: Option<&str>, saved: u8, system_dark: bool) -> bool {
    match fixture {
        Some("light") => false,
        Some("dark") => true,
        _ => match saved {
            1 => false,
            2 => true,
            _ => system_dark,
        },
    }
}

pub fn for_dark(dark: bool) -> Palette {
    let adaptive = |light: (f32, f32, f32), night: (f32, f32, f32)| {
        let (r, g, b) = if dark { night } else { light };
        color(r, g, b)
    };
    Palette {
        bg: adaptive((0.965, 0.960, 0.951), (0.085, 0.083, 0.080)),
        surface: adaptive((1., 0.998, 0.994), (0.145, 0.141, 0.136)),
        well: adaptive((0.947, 0.941, 0.931), (0.112, 0.108, 0.103)),
        border: adaptive((0.865, 0.849, 0.824), (0.280, 0.266, 0.248)),
        separator: opacity(adaptive((0.64, 0.60, 0.55), (0.90, 0.88, 0.85)), 0.16),
        primary: adaptive((0.10, 0.095, 0.09), (0.98, 0.975, 0.97)),
        secondary: adaptive((0.26, 0.245, 0.23), (0.87, 0.855, 0.84)),
        accent: adaptive((0.54, 0.21, 0.025), (1., 0.73, 0.52)),
        accent_fill: color(0.74, 0.34, 0.05),
        success: adaptive((0.08, 0.42, 0.27), (0.40, 0.83, 0.63)),
        danger: adaptive((0.70, 0.14, 0.26), (1., 0.53, 0.59)),
        teal: adaptive((0.06, 0.40, 0.43), (0.38, 0.81, 0.81)),
        brand: adaptive((0.89, 0.46, 0.15), (0.95, 0.53, 0.22)),
    }
}

pub fn ui_font() -> &'static str {
    if cfg!(target_os = "macos") {
        ".SystemUIFont"
    } else {
        "DejaVu Sans"
    }
}

pub use crate::home::{tool_icon, tool_subtitle, tool_title};

#[cfg(test)]
mod tests {
    use super::{color, for_dark, resolve_dark};
    #[test]
    fn fixture_precedes_saved_appearance_then_system() {
        assert!(!resolve_dark(Some("light"), 2, true));
        assert!(resolve_dark(Some("dark"), 1, false));
        assert!(!resolve_dark(None, 1, true));
        assert!(resolve_dark(None, 2, false));
        assert!(resolve_dark(None, 0, true));
        assert!(!resolve_dark(None, 0, false));
        assert!(resolve_dark(Some("unknown"), 0, true));
    }
    #[test]
    fn light_palette_matches_swift_source() {
        let p = for_dark(false);
        assert_eq!(p.bg, color(0.965, 0.960, 0.951));
        assert_eq!(p.primary, color(0.10, 0.095, 0.09));
        assert_eq!(p.accent, color(0.54, 0.21, 0.025));
        assert_eq!(p.separator.a, 0.16);
    }
    #[test]
    fn dark_palette_matches_swift_source() {
        let p = for_dark(true);
        assert_eq!(p.bg, color(0.085, 0.083, 0.080));
        assert_eq!(p.primary, color(0.98, 0.975, 0.97));
        assert_eq!(p.accent, color(1.0, 0.73, 0.52));
        assert_eq!(p.separator.a, 0.16);
    }
}
