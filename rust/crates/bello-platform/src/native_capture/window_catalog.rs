//! Owned Window catalog transport. No core/UI dependency or borrowed CF objects.
//! CG selector identity is exact f64 metadata; raw ordered occlusion rows are a
//! separate optional query and retain missing fields rather than filtering them.
use super::{
    CaptureCancellation, CaptureError, CaptureRect, WindowCaptureError, WindowCaptureResult,
};
use crate::window_capture::{
    WindowDisplayGeometry, WindowFrame, WindowIdentity, WindowObservation,
};
use std::time::Instant;

/// Independent platform admission. Enabling an app entry cannot enable native
/// enumeration, permissions or acquisition through this boundary.
const NATIVE_WINDOW_ENABLED: bool = false;

pub fn check_window_available() -> WindowCaptureResult<()> {
    admit(|| {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Ok(())
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            Err(WindowCaptureError::Unavailable)
        }
    })
}
fn admit<T>(work: impl FnOnce() -> WindowCaptureResult<T>) -> WindowCaptureResult<T> {
    if !NATIVE_WINDOW_ENABLED {
        return Err(WindowCaptureError::Unavailable);
    }
    work()
}

/// An absolute deadline is shared across observation, acquisition and final
/// validation. Logical timeout does not mean opaque native work has stopped.
#[derive(Clone, Debug)]
pub struct WindowObservationRequest {
    cancellation: CaptureCancellation,
    boundary: Option<CaptureCancellation>,
    deadline: Instant,
}
impl WindowObservationRequest {
    pub fn new(cancellation: CaptureCancellation, deadline: Instant) -> Self {
        Self {
            cancellation,
            boundary: None,
            deadline,
        }
    }
    pub fn with_boundary(mut self, boundary: CaptureCancellation) -> Self {
        self.boundary = Some(boundary);
        self
    }
    pub fn check(&self) -> WindowCaptureResult<()> {
        if self.cancellation.is_cancelled()
            || self
                .boundary
                .as_ref()
                .is_some_and(CaptureCancellation::is_cancelled)
        {
            Err(CaptureError::Cancelled.into())
        } else if Instant::now() >= self.deadline {
            Err(CaptureError::TimedOut.into())
        } else {
            Ok(())
        }
    }
    pub fn cancellation(&self) -> CaptureCancellation {
        self.cancellation.clone()
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Debug)]
pub struct OwnedWindowObservation {
    pub window_id: u32,
    pub owner_process_id: i32,
    pub owner_bundle_id: Option<String>,
    pub frame: CaptureRect,
    pub layer: i64,
    pub alpha: f64,
    pub on_screen: bool,
}
impl OwnedWindowObservation {
    pub fn borrowed(&self) -> WindowObservation<'_> {
        WindowObservation {
            identity: WindowIdentity {
                window_id: self.window_id,
                owner_process_id: self.owner_process_id,
                owner_bundle_id: self.owner_bundle_id.as_deref(),
            },
            frame: WindowFrame(self.frame),
            layer: self.layer,
            alpha: self.alpha,
            on_screen: self.on_screen,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RawWindowOcclusionRow {
    pub window_id: Option<u32>,
    pub owner_process_id: Option<i32>,
    pub layer: Option<i64>,
    pub alpha: Option<f64>,
    /// CG top-left desktop points. The app adapts to its scoped main-display
    /// local space; no f32 rounding is permitted in this platform transport.
    pub frame: Option<CaptureRect>,
}
#[derive(Clone, Copy, Debug)]
pub struct WindowCatalogLayers {
    pub normal: i64,
    pub floating: i64,
    pub modal_panel: i64,
    pub main_menu: i64,
    pub status: i64,
    pub popup_menu: i64,
    pub screen_saver: i64,
}
#[derive(Clone, Debug)]
pub struct WindowCatalogSnapshot {
    pub observations: Vec<OwnedWindowObservation>,
    pub main_display_id: u32,
    pub displays: Vec<WindowDisplayGeometry>,
    pub occlusion_rows: Option<Vec<RawWindowOcclusionRow>>,
}

pub fn window_identity_and_layers() -> WindowCaptureResult<(i32, WindowCatalogLayers)> {
    admit(|| {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            super::macos::window::identity_and_layers()
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            Err(WindowCaptureError::Unavailable)
        }
    })
}

/// Worker-only entry. NSScreen is accessed inside an owned main-queue context;
/// the worker remains physically counted until that context has drained, even
/// after cancellation/deadline. Never call from foreground publication/apply.
pub fn observe_windows(
    request: WindowObservationRequest,
) -> WindowCaptureResult<WindowCatalogSnapshot> {
    admit(|| {
        request.check()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            super::macos::window::observe(request)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            Err(WindowCaptureError::Unavailable)
        }
    })
}

pub(super) fn admit_capture<T>(
    work: impl FnOnce() -> WindowCaptureResult<T>,
) -> WindowCaptureResult<T> {
    admit(work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};
    #[test]
    fn closed_platform_gate_has_zero_native_actions() {
        let actions = Cell::new(0);
        let result = admit(|| {
            actions.set(actions.get() + 1);
            Ok(())
        });
        assert_eq!(result, Err(WindowCaptureError::Unavailable));
        assert_eq!(actions.get(), 0);
        assert!(check_window_available().is_err());
        assert!(window_identity_and_layers().is_err());
        assert!(observe_windows(WindowObservationRequest::new(
            CaptureCancellation::default(),
            Instant::now() + Duration::from_secs(1)
        ))
        .is_err());
    }
    #[test]
    fn observation_uses_absolute_deadline_and_owned_cancellation() {
        let flag = CaptureCancellation::default();
        let request =
            WindowObservationRequest::new(flag.clone(), Instant::now() + Duration::from_secs(1));
        assert!(request.check().is_ok());
        flag.cancel();
        assert_eq!(request.check(), Err(CaptureError::Cancelled.into()));
        let expired = WindowObservationRequest::new(CaptureCancellation::default(), Instant::now());
        assert_eq!(expired.check(), Err(CaptureError::TimedOut.into()));
    }
}
