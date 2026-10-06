//! Explicit, in-memory macOS 14+ ScreenCaptureKit one-shot capture.
//!
//! No helper fallback, permission request, clipboard, filesystem, network, or
//! automatic capture. Call only after an explicit user gesture, on a worker.
//! For Display/Region the host hides its capture UI/all BelloBox windows first
//! and restores them on completion/cancellation. This hide transaction does not
//! apply to the disabled future frozen-overlay Window refresh. Region coordinates are LOCAL
//! TOP-LEFT DISPLAY POINTS, never desktop pixels or Cocoa bottom-left points.
//! macOS 13's source SCStream fallback is not implemented by this API.
#[cfg(any(target_os = "macos", test))]
mod completion;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(test)]
mod tests;

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub const MAX_CAPTURE_PIXELS: u64 = 64_000_000;
pub const MAX_CAPTURE_DIMENSION: u32 = 32_768;
pub const MAX_CAPTURE_PNG_BYTES: usize = 64_000_000;
pub const MAX_CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_DISPLAYS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    Cancelled,
    TimedOut,
    Unsupported,
    Busy,
    WorkerThreadRequired,
    PermissionNotGranted,
    InvalidRequest,
    DisplayNotFound,
    DisplayChanged,
    NativeFailure,
    InvalidImage,
    OutputTooLarge,
}
impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Screen capture was cancelled.",
            Self::TimedOut => "Screen capture timed out. No screenshot was published.",
            Self::Unsupported => "Native one-shot ScreenCaptureKit capture requires macOS 14 or later.",
            Self::WorkerThreadRequired => "Native capture and PNG encoding must run off the main thread.",
            Self::Busy => "A previous native screen capture is still finishing. Try again when it has stopped.",
            Self::PermissionNotGranted => "Screen Recording permission is not granted. Open System Settings to enable it.",
            Self::InvalidRequest => "The requested display, region, dimensions, or deadline are invalid.",
            Self::DisplayNotFound => "The selected display is no longer available. Choose the display again.",
            Self::DisplayChanged => "The display configuration changed. Choose the capture area again.",
            Self::NativeFailure => "ScreenCaptureKit could not capture the selected display or region.",
            Self::InvalidImage => "ScreenCaptureKit returned an empty or unexpected image.",
            Self::OutputTooLarge => "The screenshot exceeds the bounded image or PNG size limit.",
        })
    }
}
impl std::error::Error for CaptureError {}
pub type CaptureResult<T> = Result<T, CaptureError>;

/// A fresh window/job-owned flag. Cancellation rejects native late completions;
/// SCScreenshotManager itself does not expose an operation-cancel API.
#[derive(Clone, Debug, Default)]
pub struct CaptureCancellation(Arc<AtomicBool>);
impl CaptureCancellation {
    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self(flag)
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapturePoint {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureSize {
    pub width: f64,
    pub height: f64,
}
/// Global CoreGraphics top-left bounds for a display, or local top-left bounds
/// for a request region. The field owning this value documents which applies.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureRect {
    pub origin: CapturePoint,
    pub size: CaptureSize,
}
impl CaptureRect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: CapturePoint { x, y },
            size: CaptureSize { width, height },
        }
    }
    fn valid(self) -> bool {
        [
            self.origin.x,
            self.origin.y,
            self.size.width,
            self.size.height,
        ]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
            && self.size.width >= 1.
            && self.size.height >= 1.
            && (self.origin.x + self.size.width).is_finite()
            && (self.origin.y + self.size.height).is_finite()
    }
    fn nearly_equals(self, other: Self) -> bool {
        self.valid()
            && other.valid()
            && [
                (self.origin.x, other.origin.x),
                (self.origin.y, other.origin.y),
                (self.size.width, other.size.width),
                (self.size.height, other.size.height),
            ]
            .iter()
            .all(|(a, b)| (a - b).abs() <= 2.)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapturePixelSize {
    pub width: u32,
    pub height: u32,
}
impl CapturePixelSize {
    pub fn validate(self) -> CaptureResult<()> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_CAPTURE_DIMENSION
            || self.height > MAX_CAPTURE_DIMENSION
            || u64::from(self.width) * u64::from(self.height) > MAX_CAPTURE_PIXELS
        {
            Err(CaptureError::OutputTooLarge)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureDisplay {
    pub id: u32,
    /// Global top-left display bounds, in points, from CoreGraphics.
    pub bounds: CaptureRect,
    pub pixels: CapturePixelSize,
}
impl CaptureDisplay {
    fn validate(self) -> CaptureResult<()> {
        if self.id == 0 || !self.bounds.valid() {
            return Err(CaptureError::InvalidRequest);
        }
        self.pixels.validate()
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureRequest {
    pub display: CaptureDisplay,
    /// None captures the explicit display. Some captures ONLY this local region.
    pub region: Option<CaptureRect>,
    pub output_size: CapturePixelSize,
    pub include_cursor: bool,
    pub timeout: Duration,
}
impl CaptureRequest {
    pub fn full_display(display: CaptureDisplay) -> Self {
        Self {
            display,
            region: None,
            output_size: display.pixels,
            include_cursor: false,
            timeout: MAX_CAPTURE_TIMEOUT,
        }
    }
    pub fn validate(&self) -> CaptureResult<()> {
        self.display.validate()?;
        self.output_size.validate()?;
        if self.timeout.is_zero() || self.timeout > MAX_CAPTURE_TIMEOUT {
            return Err(CaptureError::InvalidRequest);
        }
        if let Some(region) = self.region {
            if self.include_cursor
                || !region.valid()
                || region.origin.x < 0.
                || region.origin.y < 0.
                || region.origin.x + region.size.width > self.display.bounds.size.width
                || region.origin.y + region.size.height > self.display.bounds.size.height
            {
                return Err(CaptureError::InvalidRequest);
            }
        } else if self.output_size != self.display.pixels {
            return Err(CaptureError::InvalidRequest);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayResolutionPath {
    InitialId,
    RefreshedId,
    RefreshedBounds,
    InitialBounds,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayResolution {
    pub display: CaptureDisplay,
    pub path: DisplayResolutionPath,
}
/// Swift DisplayCaptureResolver order, without its legacy fallback. A changed
/// target cannot fall through to the main display or a whole-screen helper.
pub fn resolve_display(
    request: &CaptureRequest,
    initial: &[CaptureDisplay],
    refreshed: Option<&[CaptureDisplay]>,
) -> CaptureResult<DisplayResolution> {
    request.validate()?;
    if initial.len() > MAX_DISPLAYS || refreshed.is_some_and(|v| v.len() > MAX_DISPLAYS) {
        return Err(CaptureError::InvalidRequest);
    }
    for candidate in initial.iter().chain(refreshed.unwrap_or_default()) {
        candidate.validate()?;
    }
    let find_id =
        |items: &[CaptureDisplay]| items.iter().find(|d| d.id == request.display.id).copied();
    let find_bounds = |items: &[CaptureDisplay]| {
        items
            .iter()
            .find(|d| d.bounds.nearly_equals(request.display.bounds))
            .copied()
    };
    let selected = find_id(initial)
        .map(|display| DisplayResolution {
            display,
            path: DisplayResolutionPath::InitialId,
        })
        .or_else(|| {
            refreshed
                .and_then(find_id)
                .map(|display| DisplayResolution {
                    display,
                    path: DisplayResolutionPath::RefreshedId,
                })
        })
        .or_else(|| {
            refreshed
                .and_then(find_bounds)
                .map(|display| DisplayResolution {
                    display,
                    path: DisplayResolutionPath::RefreshedBounds,
                })
        })
        .or_else(|| {
            find_bounds(initial).map(|display| DisplayResolution {
                display,
                path: DisplayResolutionPath::InitialBounds,
            })
        })
        .ok_or(CaptureError::DisplayNotFound)?;
    // A native ID that survives a resolution/geometry change is still stale for
    // the previously selected region. Fail instead of silently stretching/reframing.
    if !selected
        .display
        .bounds
        .nearly_equals(request.display.bounds)
        || selected.display.pixels != request.display.pixels
    {
        return Err(CaptureError::DisplayChanged);
    }
    Ok(selected)
}
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureDiagnostics {
    pub requested_display_id: u32,
    pub resolved_display_id: u32,
    pub resolution_path: DisplayResolutionPath,
    pub output_size: CapturePixelSize,
    pub region: Option<CaptureRect>,
    pub includes_cursor: bool,
    pub backend: &'static str,
}
/// Owned image bytes only. Debug deliberately omits PNG data and native errors.
pub struct NativeCaptureSnapshot {
    pub png: Vec<u8>,
    pub diagnostics: CaptureDiagnostics,
}
impl fmt::Debug for NativeCaptureSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeCaptureSnapshot")
            .field("png_byte_count", &self.png.len())
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

/// Read-only availability; never enumerates content, captures, or requests TCC.
pub fn is_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::is_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
/// Read-only display metadata, not an image capture or permission request.
pub fn main_display() -> CaptureResult<CaptureDisplay> {
    #[cfg(target_os = "macos")]
    {
        macos::main_display()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(CaptureError::Unsupported)
    }
}
pub fn capture_main_display_snapshot(
    cancellation: CaptureCancellation,
) -> CaptureResult<NativeCaptureSnapshot> {
    if cancellation.is_cancelled() {
        return Err(CaptureError::Cancelled);
    }
    #[cfg(target_os = "macos")]
    macos::require_worker_thread()?;
    capture(CaptureRequest::full_display(main_display()?), cancellation)
}
/// Synchronous worker action; macOS main-thread calls are rejected.
/// The waiter/output are bounded, not the native framework's runtime or memory.
/// The host must hide its own windows first.
/// No legacy/helper fallback can broaden a region or change a display target.
pub fn capture(
    request: CaptureRequest,
    cancellation: CaptureCancellation,
) -> CaptureResult<NativeCaptureSnapshot> {
    request.validate()?;
    if cancellation.is_cancelled() {
        return Err(CaptureError::Cancelled);
    }
    #[cfg(target_os = "macos")]
    {
        macos::capture(request, cancellation)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        Err(CaptureError::Unsupported)
    }
}

/// Window capture remains unavailable in production. Native unit-test builds
/// compile the candidate without adding an app route or requesting permission.
pub const NATIVE_WINDOW_CAPTURE_IMPLEMENTED: bool = false;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCaptureError {
    Unavailable,
    Native(CaptureError),
    Policy(crate::window_capture::WindowPolicyError),
}
impl From<CaptureError> for WindowCaptureError {
    fn from(value: CaptureError) -> Self {
        Self::Native(value)
    }
}
impl From<crate::window_capture::WindowPolicyError> for WindowCaptureError {
    fn from(value: crate::window_capture::WindowPolicyError) -> Self {
        Self::Policy(value)
    }
}
impl fmt::Display for WindowCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("Independent window capture is not enabled."),
            Self::Native(error) => error.fmt(f),
            Self::Policy(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for WindowCaptureError {}
pub type WindowCaptureResult<T> = Result<T, WindowCaptureError>;

/// Pointer-free, caller-owned live session/generation. The host advances this on
/// reselection/reopen and cancels its original cancellation flag on dismissal.
/// No native object, permission or atomic window-incarnation guarantee is stored.
/// Updating this token, or cancelling a separate selection flag, cannot interrupt
/// ImageIO already encoding. The PNG sink observes the operation Job's flag and
/// deadline; generation/selection checks at later boundaries reject stale output.
/// The host must recheck its document/session/cancellation before publication.
#[derive(Debug)]
pub struct WindowCaptureSession(std::sync::Mutex<crate::window_capture::WindowSelectionToken>);
impl WindowCaptureSession {
    pub fn new(token: crate::window_capture::WindowSelectionToken) -> WindowCaptureResult<Self> {
        if token.session == 0 {
            return Err(crate::window_capture::WindowPolicyError::InvalidMetadata.into());
        }
        Ok(Self(std::sync::Mutex::new(token)))
    }
    pub fn update(
        &self,
        token: crate::window_capture::WindowSelectionToken,
    ) -> WindowCaptureResult<()> {
        if token.session == 0 {
            return Err(crate::window_capture::WindowPolicyError::InvalidMetadata.into());
        }
        *self.0.lock().map_err(|_| CaptureError::NativeFailure)? = token;
        Ok(())
    }
    pub fn current(&self) -> WindowCaptureResult<crate::window_capture::WindowSelectionToken> {
        Ok(*self.0.lock().map_err(|_| CaptureError::NativeFailure)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowCaptureDiagnostics {
    pub window_id: u32,
    pub owner_process_id: i32,
    pub output_size: CapturePixelSize,
    pub includes_cursor: bool,
    pub backend: &'static str,
}
pub struct NativeWindowCaptureSnapshot {
    pub png: Vec<u8>,
    pub diagnostics: WindowCaptureDiagnostics,
}
impl fmt::Debug for NativeWindowCaptureSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeWindowCaptureSnapshot")
            .field("png_byte_count", &self.png.len())
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

/// Deliberately disabled before any enumeration, native call, permission check,
/// image acquisition or app side effect. The future frozen-overlay host must
/// perform its own final document/session/cancellation publication check.
pub fn capture_window(
    _selection: crate::window_capture::WindowCaptureSelection,
    _options: crate::window_capture::WindowCaptureOptions,
    _session: Arc<WindowCaptureSession>,
    _cancellation: CaptureCancellation,
) -> WindowCaptureResult<NativeWindowCaptureSnapshot> {
    Err(WindowCaptureError::Unavailable)
}

#[cfg(test)]
fn validate_external_window_owner(owner_process_id: i32) -> WindowCaptureResult<()> {
    if owner_process_id <= 0 {
        return Err(crate::window_capture::WindowPolicyError::InvalidMetadata.into());
    }
    // Native trust boundary: never rely solely on the pure policy's injected
    // own PID. A mistakenly constructed selection cannot authorize self-capture.
    if u32::try_from(owner_process_id).ok() == Some(std::process::id()) {
        return Err(crate::window_capture::WindowPolicyError::IneligibleWindow.into());
    }
    Ok(())
}
