//! Shared QR-only external image clipboard capability. No clipboard probing.
/// GPUI 0.2.2 Linux backends publish text/private cache data, not external PNG.
/// Read the actual backend from App, never infer it from environment variables.
pub(crate) fn image_copy_notice(compositor: &str, native_macos: bool) -> Option<&'static str> {
    match compositor {
        "X11" => Some("Copy Image is unavailable on X11. Use Save… to export PNG."),
        "Wayland" => Some("Copy Image is unavailable on Wayland. Use Save… to export PNG."),
        "headless" => Some("Image clipboard is unavailable. Use Save… to export PNG."),
        // macOS writes the image UTType to NSPasteboard; an empty name alone
        // must never grant the capability on another/unknown platform.
        "" if native_macos => None,
        _ => Some("Image clipboard is unsupported on this backend. Use Save… to export PNG."),
    }
}

pub(crate) fn for_app(cx: &gpui::App) -> Option<&'static str> {
    image_copy_notice(cx.compositor_name(), cfg!(target_os = "macos"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_backend_contract_never_infers_support_from_an_empty_name() {
        for native_macos in [false, true] {
            for compositor in ["X11", "Wayland", "headless", "unknown compositor"] {
                assert!(
                    image_copy_notice(compositor, native_macos)
                        .unwrap()
                        .contains("Save…")
                );
            }
        }
        assert!(image_copy_notice("", true).is_none());
        assert!(image_copy_notice("", false).is_some());
    }
}
