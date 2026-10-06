//! The only borrowed-native edges in the caller. Linux test builds compile the
//! real GPUI ownership/control flow, but these unsupported edges cannot capture,
//! show, hide, or configure a native window there.
use super::*;
#[cfg(target_os = "macos")]
use raw_window_handle::HasWindowHandle;

pub(super) fn context(window: &Window) -> Result<host::OwnedWindowContext, String> {
    #[cfg(target_os = "macos")]
    {
        host::owned_window_context(
            HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err(host::OverlayError::Unsupported.to_string())
    }
}
pub(super) fn hide(window: &Window) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        host::order_out_owned(HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err(host::OverlayError::Unsupported.to_string())
    }
}
pub(super) fn restore(window: &Window) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        host::restore_owned_without_activation(
            HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err(host::OverlayError::Unsupported.to_string())
    }
}
pub(super) fn configure(
    window: &Window,
    layout: host::MainDisplayOverlayLayout,
    viewport: capture::CaptureSize,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        host::configure_hidden_owned_overlay(
            HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
            layout,
            viewport,
        )
        .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, layout, viewport);
        Err(host::OverlayError::Unsupported.to_string())
    }
}
pub(super) fn present(
    window: &Window,
    requester: &Window,
    layout: host::MainDisplayOverlayLayout,
    viewport: capture::CaptureSize,
    cancellation: &capture::CaptureCancellation,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        host::present_configured_owned_overlay(
            HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
            HasWindowHandle::window_handle(requester).map_err(|e| e.to_string())?,
            layout,
            viewport,
            cancellation,
        )
        .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, requester, layout, viewport, cancellation);
        Err(host::OverlayError::Unsupported.to_string())
    }
}
