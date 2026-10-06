//! Owned-window host for the frozen Area picker and capture visibility transaction.
//! No capture, permission prompt, window lookup or application activation occurs.
//! Only explicit restoration and guarded synchronous presentation order owned
//! windows. Configuration alone never presents a window.
//!
//! Native entry points are UI-thread-only and Apple Silicon/macOS 14+ only.
//! Configuration accepts only a borrowed handle to the exact hidden GPUI 0.2.2
//! PopUp just opened by the caller. Visibility/context calls accept currently
//! live owned GPUI windows. A configuration error must close that hidden popup,
//! never show a titled fallback. Read layout before freezing, then revalidate before presentation
//! and before accepting a crop. The caller still owns cancellation and teardown.
//!
//! Source authority: `CaptureOverlayPanelConfiguration.swift` and
//! `CaptureOverlayController.swift`. This first host supports an unrotated MAIN
//! display only. Secondary-display overlays and native rotated-display capture
//! remain parity gaps. Portable geometry handles negative origins and independent
//! pixel ratios. Cross-compilation/pure tests do NOT verify AppKit runtime,
//! Spaces/Stage Manager, input/focus, presentation or Retina capture alignment.

mod deactivation;
pub use deactivation::{ApplicationDeactivationEvent, ApplicationDeactivationSignal};

use crate::native_capture::{CaptureDisplay, CaptureRect, CaptureRequest, CaptureSize};
use std::fmt;

/// Deliberately stricter than a one-point/titlebar/one-pixel layout mismatch.
const POINT_EPSILON: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayError {
    Unsupported,
    WrongThread,
    InvalidGeometry,
    UnsupportedLayout,
    DisplayChanged,
    InvalidOwnedWindow,
    WindowAlreadyVisible,
    ViewportMismatch,
    NativeConfigurationFailed,
    NavigationChanged,
    FullscreenWindow,
    UnsafeRestoration,
}
impl fmt::Display for OverlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => {
                "The native Area overlay requires Apple Silicon and macOS 14 or later."
            }
            Self::WrongThread => "The native Area overlay must be prepared on the UI thread.",
            Self::InvalidGeometry => {
                "The display geometry is invalid or exceeds the Area capture limits."
            }
            Self::UnsupportedLayout => {
                "Area capture currently supports only an unrotated main display."
            }
            Self::DisplayChanged => "The display configuration changed. Start Area capture again.",
            Self::InvalidOwnedWindow => {
                "The window could not be verified as the live owned GPUI window."
            }
            Self::WindowAlreadyVisible => {
                "The Area popup must remain hidden until its overlay is configured."
            }
            Self::ViewportMismatch => {
                "The Area popup does not cover the complete display. Start Area capture again."
            }
            Self::NativeConfigurationFailed => {
                "The native owned window could not be configured safely."
            }
            Self::NavigationChanged => {
                "Application visibility or focus changed. Start capture again."
            }
            Self::FullscreenWindow => {
                "Capture cannot temporarily hide a fullscreen window. Leave fullscreen and try again."
            }
            Self::UnsafeRestoration => {
                "The prior window was left hidden to preserve your current navigation."
            }
        })
    }
}
impl std::error::Error for OverlayError {}
pub type OverlayResult<T> = Result<T, OverlayError>;

/// Immutable display topology, safe to carry to a worker together with the
/// capture request. `display.bounds` is CoreGraphics TOP-LEFT global points;
/// `cocoa_frame` is AppKit BOTTOM-LEFT global points. Never interchange them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MainDisplayOverlayLayout {
    pub display: CaptureDisplay,
    pub cocoa_frame: CaptureRect,
    pub rotation_degrees: u16,
    /// AppKit UI backing scale. This is NOT necessarily the captured PNG ratio.
    pub backing_scale: f64,
}
impl MainDisplayOverlayLayout {
    /// Production policy: the menu-bar/main display has origin (0, 0) in both
    /// coordinate systems. A secondary display must not become an implicit main
    /// display fallback. The native reader additionally checks CGMainDisplayID.
    pub fn validate(self) -> OverlayResult<()> {
        CaptureRequest::full_display(self.display)
            .validate()
            .map_err(|_| OverlayError::InvalidGeometry)?;
        validate_rect(self.cocoa_frame)?;
        if !self.backing_scale.is_finite() || !(0.25..=16.).contains(&self.backing_scale) {
            return Err(OverlayError::InvalidGeometry);
        }
        if self.rotation_degrees != 0
            || self.display.bounds.origin.x != 0.
            || self.display.bounds.origin.y != 0.
            || self.cocoa_frame.origin.x != 0.
            || self.cocoa_frame.origin.y != 0.
        {
            return Err(OverlayError::UnsupportedLayout);
        }
        if !same_size(self.display.bounds.size, self.cocoa_frame.size) {
            return Err(OverlayError::InvalidGeometry);
        }
        self.pixel_ratios()?;
        Ok(())
    }

    /// Independent image-pixels-per-point ratios, never a GPUI/backing-scale
    /// guess. The frozen image must separately be verified against display.pixels.
    pub fn pixel_ratios(self) -> OverlayResult<(f64, f64)> {
        validate_rect(self.cocoa_frame)?;
        self.display
            .pixels
            .validate()
            .map_err(|_| OverlayError::InvalidGeometry)?;
        let x = f64::from(self.display.pixels.width) / self.cocoa_frame.size.width;
        let y = f64::from(self.display.pixels.height) / self.cocoa_frame.size.height;
        if !(0.25..=16.).contains(&x) || !(0.25..=16.).contains(&y) {
            return Err(OverlayError::InvalidGeometry);
        }
        Ok((x, y))
    }

    /// Exact topology equality is intentional: even a subpoint/scale/rotation
    /// change invalidates the frozen image's coordinate contract.
    pub fn revalidate(self, observed: Self) -> OverlayResult<()> {
        self.validate()?;
        if self != observed {
            return Err(OverlayError::DisplayChanged);
        }
        observed.validate()
    }

    pub fn validate_viewport(self, viewport: CaptureSize) -> OverlayResult<()> {
        self.validate()?;
        if same_size(viewport, self.cocoa_frame.size) {
            Ok(())
        } else {
            Err(OverlayError::ViewportMismatch)
        }
    }
}

/// Reflect a global CoreGraphics top-left rect around the PRIMARY Cocoa top
/// edge. This is not the maximum Y of the combined display desktop. Involution:
/// the same operation also maps Cocoa rects back to CoreGraphics coordinates.
/// Negative origins/monitors above and left of the primary are preserved.
pub fn core_graphics_cocoa_rect(
    rect: CaptureRect,
    primary_cocoa_top: f64,
) -> OverlayResult<CaptureRect> {
    validate_rect(rect)?;
    if !primary_cocoa_top.is_finite() || primary_cocoa_top.abs() > 1_000_000. {
        return Err(OverlayError::InvalidGeometry);
    }
    let result = CaptureRect::new(
        rect.origin.x,
        primary_cocoa_top - rect.origin.y - rect.size.height,
        rect.size.width,
        rect.size.height,
    );
    validate_rect(result)?;
    Ok(result)
}

fn validate_rect(rect: CaptureRect) -> OverlayResult<()> {
    if [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ]
    .iter()
    .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        || rect.size.width < 1.
        || rect.size.height < 1.
    {
        Err(OverlayError::InvalidGeometry)
    } else {
        Ok(())
    }
}
fn near(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= POINT_EPSILON
}
fn same_size(a: CaptureSize, b: CaptureSize) -> bool {
    near(a.width, b.width) && near(a.height, b.height)
}
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
fn same_rect(a: CaptureRect, b: CaptureRect) -> bool {
    near(a.origin.x, b.origin.x) && near(a.origin.y, b.origin.y) && same_size(a.size, b.size)
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
fn validate_overlay_frames(
    expected: MainDisplayOverlayLayout,
    viewport: CaptureSize,
    window_frame: CaptureRect,
    content_bounds: CaptureRect,
    content_frame: CaptureRect,
    view_bounds: CaptureRect,
    view_frame: CaptureRect,
) -> OverlayResult<()> {
    expected.validate_viewport(viewport)?;
    let local = CaptureRect::new(0., 0., viewport.width, viewport.height);
    if same_rect(window_frame, expected.cocoa_frame)
        && [content_bounds, content_frame, view_bounds, view_frame]
            .iter()
            .all(|rect| same_rect(*rect, local))
    {
        Ok(())
    } else {
        Err(OverlayError::ViewportMismatch)
    }
}

/// Read-only main-display metadata on the UI thread. Does not request Screen
/// Recording/Accessibility access or touch any window. No mainScreen heuristic.
pub fn main_display_overlay_layout() -> OverlayResult<MainDisplayOverlayLayout> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        native::main_layout()
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        Err(OverlayError::Unsupported)
    }
}

/// Configure the exact newly created HIDDEN GPUI 0.2.2 PopUp, on the UI thread.
/// `handle` MUST come from that live GPUI window's HasWindowHandle implementation.
/// Create with show=false, focus=false, titlebar=None, no tabbing identifier and
/// the desired full-main-display size. This call never presents/activates it.
///
/// Borrowed raw-window-handle preserves pointer validity for this synchronous
/// call. No native pointers escape, are retained, cross a thread, or are looked
/// up by window title. GPUIView is GPUI's immediate contentView CHILD; its exact
/// parent/window links and the GPUIPanel class are checked before any mutation.
/// After configuring, native frame/content/view bounds and source flags are read
/// back. The caller must also verify GPUI's refreshed viewport before showing:
/// native resize can synchronously reenter GPUI's resize callback. On any error,
/// remove the still-hidden window. Never fall back to a normal titled window.
#[cfg(target_os = "macos")]
pub fn configure_hidden_owned_overlay(
    handle: raw_window_handle::WindowHandle<'_>,
    expected: MainDisplayOverlayLayout,
    expected_viewport: CaptureSize,
) -> OverlayResult<()> {
    #[cfg(target_arch = "aarch64")]
    {
        native::configure(handle, expected, expected_viewport)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = (handle, expected, expected_viewport);
        Err(OverlayError::Unsupported)
    }
}

/// A pointer-free classification of this application's key window. This does
/// not identify or inspect any other application's windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedKeyWindow {
    NoKeyWindow,
    Requester,
    OtherWindow,
}

/// Read immediately before restoring windows/accepting capture, never after a
/// restore has obscured a navigation change. No native objects or addresses are
/// kept in the snapshot. The host also owns its monotonic navigation generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedWindowContext {
    pub application_active: bool,
    pub application_hidden: bool,
    pub requester_visible: bool,
    pub key_window: OwnedKeyWindow,
}
impl OwnedWindowContext {
    /// Losing focus, hiding the app, opening another window, or any intervening
    /// navigation invalidates capture. Requester visibility alone cannot prove
    /// continuity (a window may have been closed/reopened or navigation reversed).
    pub fn allows_capture_continuation(
        self,
        expected_generation: u64,
        current_generation: u64,
    ) -> bool {
        expected_generation == current_generation
            && self.application_active
            && !self.application_hidden
            && matches!(
                self.key_window,
                OwnedKeyWindow::NoKeyWindow | OwnedKeyWindow::Requester
            )
    }
}

// NSFullScreenWindowMask = 1 << 14 in GPUI 0.2.2's cocoa NSWindowStyleMask.
// GPUI MacWindowState::is_fullscreen uses this exact bit, rather than geometry.
// https://github.com/servo/cocoa-rs/blob/master/src/appkit.rs
#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
fn validate_visibility_style(style: usize) -> OverlayResult<()> {
    if style & (1 << 14) != 0 {
        Err(OverlayError::FullscreenWindow)
    } else {
        Ok(())
    }
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestoreOrder {
    Unchanged,
    Back,
    BelowKey,
    Front,
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
fn restore_order(context: OwnedWindowContext) -> RestoreOrder {
    if context.requester_visible {
        RestoreOrder::Unchanged
    } else if !context.application_active || context.application_hidden {
        RestoreOrder::Back
    } else if context.key_window == OwnedKeyWindow::OtherWindow {
        RestoreOrder::BelowKey
    } else {
        RestoreOrder::Front
    }
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
fn validate_restore_level(
    order: RestoreOrder,
    target_level: isize,
    normal_level: isize,
    key_level: Option<isize>,
) -> OverlayResult<()> {
    // AppKit ordering is within a level. Never lower/raise a window's level just
    // to restore it, and never assume orderBack hides a screen-saver-level panel
    // behind another application's normal windows. Leave unsafe targets hidden.
    // https://developer.apple.com/documentation/appkit/nswindow/order(_:relativeto:)
    match order {
        RestoreOrder::Back if target_level != normal_level => Err(OverlayError::UnsafeRestoration),
        RestoreOrder::BelowKey if key_level != Some(target_level) => {
            Err(OverlayError::UnsafeRestoration)
        }
        _ => Ok(()),
    }
}

/// Read the application's activation/key state relative to one currently live
/// GPUI window. This accepts GPUIWindow or GPUIPanel through its borrowed view;
/// parented windows, attached sheets and native-fullscreen windows fail closed.
/// UI thread only; preflight every handle before ordering any window.
#[cfg(target_os = "macos")]
pub fn owned_window_context(
    handle: raw_window_handle::WindowHandle<'_>,
) -> OverlayResult<OwnedWindowContext> {
    #[cfg(target_arch = "aarch64")]
    {
        native::context(handle)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = handle;
        Err(OverlayError::Unsupported)
    }
}

/// Order out one currently live, visible, non-minimized owned GPUI window without
/// hiding/deactivating the application. Returns its prior visibility; false means
/// no change. Animation is disabled for the synchronous orderOut then restored.
///
/// The host must preflight all handles and record prior-visible logical GPUI
/// handles BEFORE ordering any window. An error can follow a successful orderOut
/// if postcondition checks fail; unwind that pre-recorded still-live set even on
/// error. Already-visible restoration is a no-op. Never retain native pointers
/// or infer restore eligibility from this call alone. Read navigation context
/// before any restoration.
#[cfg(target_os = "macos")]
pub fn order_out_owned(handle: raw_window_handle::WindowHandle<'_>) -> OverlayResult<bool> {
    #[cfg(target_arch = "aarch64")]
    {
        native::order_out(handle)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = handle;
        Err(OverlayError::Unsupported)
    }
}

/// Restore ONLY a previously-visible window recorded by the host's transaction,
/// after reacquiring a borrowed handle from its still-live logical GPUI handle.
/// Never call for a closed, replaced, newly created or previously-hidden window.
/// Already-visible windows are left alone. Otherwise inactive/hidden apps restore
/// with orderBack, and an active app with a competing key orders the target below
/// that key. Only an active app without a competing key uses orderFrontRegardless.
/// This deliberately refines the source popup's unconditional front ordering so
/// newer navigation is not covered by a delayed capture completion. Window levels
/// must permit that ordering; unsafe cross-level restores stay hidden with an error.
/// Minimized, parented/sheet and native-fullscreen targets also fail closed.
/// No unhide, deminiaturize, makeKey, level change or activation occurs. The key
/// reference is read only. Activation/key/hidden state is checked again afterward.
/// UI thread only; real Spaces/stacking behavior still needs native runtime QA.
#[cfg(target_os = "macos")]
pub fn restore_owned_without_activation(
    handle: raw_window_handle::WindowHandle<'_>,
) -> OverlayResult<()> {
    #[cfg(target_arch = "aarch64")]
    {
        native::restore(handle)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = handle;
        Err(OverlayError::Unsupported)
    }
}

/// Synchronously show the configured owned picker, then make only it key.
/// The caller must hold freshly borrowed popup AND requester handles, check its
/// live transaction/generation, and validate GPUI's refreshed viewport AND scale.
/// Pass the same cancellation flag used by that transaction; native callbacks can
/// synchronously reenter application code, so the flag is rechecked around each
/// presentation step. There is no queued activation or retained window pointer.
///
/// The app must stay active/unhidden with no key or the exact requester key until
/// picker key acquisition. Success requires the visible picker alone to be key.
/// Failure after showing orders out only the still-verified original picker.
/// Never invoke GPUI's queued activate_window as a fallback after this API.
#[cfg(target_os = "macos")]
pub fn present_configured_owned_overlay(
    popup: raw_window_handle::WindowHandle<'_>,
    requester: raw_window_handle::WindowHandle<'_>,
    expected: MainDisplayOverlayLayout,
    actual_viewport: CaptureSize,
    cancellation: &crate::native_capture::CaptureCancellation,
) -> OverlayResult<()> {
    #[cfg(target_arch = "aarch64")]
    {
        native::present(popup, requester, expected, actual_viewport, cancellation)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = (popup, requester, expected, actual_viewport, cancellation);
        Err(OverlayError::Unsupported)
    }
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PresentationStage {
    BeforeKey,
    AfterKey,
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), test))]
fn validate_presentation_context(
    requester: OwnedWindowContext,
    popup_is_key: bool,
    cancelled: bool,
    stage: PresentationStage,
) -> OverlayResult<()> {
    let expected_key = match stage {
        PresentationStage::BeforeKey => {
            !popup_is_key
                && matches!(
                    requester.key_window,
                    OwnedKeyWindow::NoKeyWindow | OwnedKeyWindow::Requester
                )
        }
        PresentationStage::AfterKey => {
            popup_is_key && requester.key_window == OwnedKeyWindow::OtherWindow
        }
    };
    if cancelled || !requester.application_active || requester.application_hidden || !expected_key {
        Err(OverlayError::NavigationChanged)
    } else {
        Ok(())
    }
}

/// UI-thread RAII subscription. The guard retains only a notification center and
/// observer token, never NSWindow/GPUI handles or an NSApp pointer. !Send/!Sync
/// keeps unregister/release on the checked construction thread in safe Rust.
///
/// Keep the guard in the live UI-owned transaction through selector/editor handoff.
/// Drop closes the one-shot before unregistering, releases native ownership, then
/// wakes an outstanding waiter. A queued late callback cannot cancel after close.
pub struct ApplicationDeactivationObserver {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    _native: native::DeactivationObserver,
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    _ui_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Observe this NSApp's didResignActive notification on NSOperationQueue.mainQueue,
/// matching CaptureOverlayController.installResignActiveObserver. The callback
/// only cancels this transaction's shared flag and wakes a bounded Rust one-shot;
/// it never accesses GPUI/windows, polls, or activates an application.
///
/// An App-owned task awaits the pointer-free signal, then checks transaction id
/// and generation before retiring/restoring live logical windows. ObserverRemoved
/// only terminates that waiter; it must never cancel a successor. Retain the UI
/// guard through any pending handoff that must be suppressed after focus loss.
pub fn observe_application_deactivation(
    cancellation: crate::native_capture::CaptureCancellation,
) -> OverlayResult<(
    ApplicationDeactivationObserver,
    ApplicationDeactivationSignal,
)> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let (observer, signal) = native::observe_deactivation(cancellation)?;
        Ok((
            ApplicationDeactivationObserver { _native: observer },
            signal,
        ))
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = cancellation;
        Err(OverlayError::Unsupported)
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod native {
    use super::*;
    use crate::native_capture::CapturePixelSize;
    use raw_window_handle::{RawWindowHandle, WindowHandle};
    use std::ffi::{c_char, c_void};

    type Id = *mut c_void;
    type Sel = *mut c_void;
    type Method = *mut c_void;
    // Objective-C BOOL is one byte. Do not construct a Rust bool from its bits.
    type ObjcBool = i8;
    // Apple SDK NSWindowStyleMaskNonactivatingPanel = 1 << 7; Borderless = 0.
    const NONACTIVATING_PANEL: usize = 1 << 7;
    // CaptureOverlayPanelConfiguration.collectionBehavior exactly: canJoinAllSpaces,
    // stationary, ignoresCycle, fullScreenAuxiliary, canJoinAllApplications (13+).
    const COLLECTION: usize = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8) | (1 << 18);
    // CGWindowLevelKey.kCGScreenSaverWindowLevelKey, not a fixed screen-saver level.
    const SCREEN_SAVER_LEVEL_KEY: i32 = 13;

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn object_getClass(object: Id) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn class_getInstanceMethod(class: Id, selector: Sel) -> Method;
        fn method_getImplementation(method: Method) -> Option<unsafe extern "C" fn()>;
        fn method_getNumberOfArguments(method: Method) -> u32;
        fn method_getReturnType(method: Method, destination: *mut c_char, length: usize);
    }
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSApplicationDidResignActiveNotification: Id;
    }
    #[link(name = "Foundation", kind = "framework")]
    extern "C" {}
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayIsOnline(id: u32) -> u32;
        fn CGDisplayBounds(id: u32) -> CaptureRect;
        fn CGDisplayPixelsWide(id: u32) -> usize;
        fn CGDisplayPixelsHigh(id: u32) -> usize;
        fn CGDisplayRotation(id: u32) -> f64;
        fn CGWindowLevelForKey(key: i32) -> i32;
    }

    // Only concrete pointer/integer/f64/void-return SDK selectors. CGRect RETURN
    // values intentionally use read_rect below, never this dispatch macro.
    macro_rules! send {
        ($object:expr, $selector:literal, ($($ty:ty => $arg:expr),* $(,)?) -> $ret:ty) => {{
            let function: unsafe extern "C" fn(Id, Sel, $($ty),*) -> $ret =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            function($object, sel_registerName($selector.as_ptr().cast()), $($arg),*)
        }};
    }

    unsafe fn class(name: &'static [u8]) -> OverlayResult<Id> {
        let value = objc_getClass(name.as_ptr().cast());
        if value.is_null() {
            Err(OverlayError::Unsupported)
        } else {
            Ok(value)
        }
    }
    fn ui_thread() -> OverlayResult<()> {
        unsafe {
            if send!(class(b"NSThread\0")?, b"isMainThread\0", () -> ObjcBool) == 0 {
                return Err(OverlayError::WrongThread);
            }
        }
        // Same non-prompting macOS 14+ SCScreenshotManager selector availability
        // check as the capture adapter. Loading a class does not capture anything.
        if !crate::native_capture::is_available() {
            return Err(OverlayError::Unsupported);
        }
        Ok(())
    }
    struct Pool(Id);
    impl Pool {
        unsafe fn new() -> OverlayResult<Self> {
            let value = send!(class(b"NSAutoreleasePool\0")?, b"new\0", () -> Id);
            if value.is_null() {
                Err(OverlayError::NativeConfigurationFailed)
            } else {
                Ok(Self(value))
            }
        }
    }
    impl Drop for Pool {
        fn drop(&mut self) {
            // Local pool created and dropped inside one checked UI-thread call.
            unsafe {
                send!(self.0, b"release\0", () -> ());
            }
        }
    }

    /// These three AppKit getters are declared `NSRect frame` (NSWindow/NSScreen/
    /// NSView) and `NSRect bounds` (NSView), with no explicit arguments. On 64-bit
    /// Apple targets NSRect/CGRect is { { double x,y }, { double width,height } }.
    /// CaptureRect and its two members are repr(C), with the same four f64 fields.
    ///
    /// Apple's runtime exposes the actual implementation through Method/IMP. We
    /// resolve the receiver's dynamic class, require a real method and non-null
    /// implementation, check its zero-explicit-argument/return encoding, then call
    /// that IMP as its exact C signature. The compiler handles the C struct-return
    /// ABI. This avoids pretending objc_msgSend has a portable CGRect-return ABI.
    /// This bridge is deliberately compiled ONLY for Apple Silicon; x86_64 is not
    /// claimed/audited. No dynamically supplied selectors are accepted here.
    ///
    /// Primary references:
    /// https://github.com/apple-oss-distributions/objc4/blob/main/runtime/runtime.h
    /// https://github.com/apple-oss-distributions/objc4/blob/main/runtime/message.h
    /// https://developer.apple.com/documentation/appkit/nswindow/frame
    /// https://developer.apple.com/documentation/appkit/nsview/bounds
    unsafe fn read_rect(object: Id, selector_name: &'static [u8]) -> OverlayResult<CaptureRect> {
        if object.is_null() || ![&b"frame\0"[..], &b"bounds\0"[..]].contains(&selector_name) {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        let selector = sel_registerName(selector_name.as_ptr().cast());
        let method = class_getInstanceMethod(object_getClass(object), selector);
        if method.is_null() || method_getNumberOfArguments(method) != 2 {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        let mut encoding = [0u8; 128];
        method_getReturnType(method, encoding.as_mut_ptr().cast(), encoding.len());
        let end = encoding
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(OverlayError::NativeConfigurationFailed)?;
        if ![
            &b"{CGRect={CGPoint=dd}{CGSize=dd}}"[..],
            &b"{_NSRect={_NSPoint=dd}{_NSSize=dd}}"[..],
        ]
        .contains(&&encoding[..end])
        {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        let implementation =
            method_getImplementation(method).ok_or(OverlayError::NativeConfigurationFailed)?;
        let getter: unsafe extern "C" fn(Id, Sel) -> CaptureRect =
            std::mem::transmute(implementation);
        let rect = getter(object, selector);
        validate_rect(rect)?;
        Ok(rect)
    }

    unsafe fn screen_id(screen: Id) -> OverlayResult<u32> {
        if screen.is_null() {
            return Err(OverlayError::DisplayChanged);
        }
        let key = send!(class(b"NSString\0")?, b"stringWithUTF8String:\0",
            (*const c_char => c"NSScreenNumber".as_ptr()) -> Id);
        if key.is_null() {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        let description = send!(screen, b"deviceDescription\0", () -> Id);
        let number = send!(description, b"objectForKey:\0", (Id => key) -> Id);
        if number.is_null()
            || send!(number, b"isKindOfClass:\0", (Id => class(b"NSNumber\0")?) -> ObjcBool) == 0
        {
            return Err(OverlayError::DisplayChanged);
        }
        Ok(send!(number, b"unsignedIntValue\0", () -> u32))
    }

    /// Resolve only the display identity supplied by CGMainDisplayID. NSScreen
    /// enumeration is bounded metadata lookup; no application/window enumeration.
    unsafe fn layout() -> OverlayResult<MainDisplayOverlayLayout> {
        let main_id = CGMainDisplayID();
        if main_id == 0 || CGDisplayIsOnline(main_id) == 0 {
            return Err(OverlayError::DisplayChanged);
        }
        let screens = send!(class(b"NSScreen\0")?, b"screens\0", () -> Id);
        if screens.is_null() {
            return Err(OverlayError::DisplayChanged);
        }
        let count = send!(screens, b"count\0", () -> usize);
        if count == 0 || count > 64 {
            return Err(OverlayError::UnsupportedLayout);
        }
        let mut matching = None;
        for index in 0..count {
            let screen = send!(screens, b"objectAtIndex:\0", (usize => index) -> Id);
            if screen.is_null() {
                return Err(OverlayError::DisplayChanged);
            }
            if screen_id(screen)? == main_id {
                if matching.is_some() {
                    return Err(OverlayError::DisplayChanged);
                }
                matching = Some(screen);
            }
        }
        let screen = matching.ok_or(OverlayError::DisplayChanged)?;
        let cocoa_frame = read_rect(screen, b"frame\0")?;
        let rotation = CGDisplayRotation(main_id);
        // Never guess orientation for CGDisplayPixelsWide/High under rotation.
        if !rotation.is_finite() || rotation != 0. {
            return Err(OverlayError::UnsupportedLayout);
        }
        let result = MainDisplayOverlayLayout {
            display: CaptureDisplay {
                id: main_id,
                bounds: CGDisplayBounds(main_id),
                pixels: CapturePixelSize {
                    width: u32::try_from(CGDisplayPixelsWide(main_id))
                        .map_err(|_| OverlayError::InvalidGeometry)?,
                    height: u32::try_from(CGDisplayPixelsHigh(main_id))
                        .map_err(|_| OverlayError::InvalidGeometry)?,
                },
            },
            cocoa_frame,
            rotation_degrees: 0,
            backing_scale: send!(screen, b"backingScaleFactor\0", () -> f64),
        };
        result.validate()?;
        if CGMainDisplayID() != main_id || CGDisplayIsOnline(main_id) == 0 {
            return Err(OverlayError::DisplayChanged);
        }
        Ok(result)
    }

    pub(super) fn main_layout() -> OverlayResult<MainDisplayOverlayLayout> {
        ui_thread()?;
        unsafe {
            let _pool = Pool::new()?;
            // Two equal reads reject a topology transition observed mid-query.
            let first = layout()?;
            first.revalidate(layout()?)?;
            Ok(first)
        }
    }

    // Only borrowed pointers during one synchronous call. Never store them in a
    // public receipt, on a worker, or for deferred restore of a closed window.
    struct OwnedView {
        view: Id,
        window: Id,
        content: Id,
    }
    unsafe fn owned_window(handle: WindowHandle<'_>) -> OverlayResult<OwnedView> {
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err(OverlayError::InvalidOwnedWindow);
        };
        let view = handle.ns_view.as_ptr();
        // Pinned GPUI ownership structure, not a search for a window by name.
        if object_getClass(view) != class(b"GPUIView\0")? {
            return Err(OverlayError::InvalidOwnedWindow);
        }
        let window = send!(view, b"window\0", () -> Id);
        if window.is_null() {
            return Err(OverlayError::InvalidOwnedWindow);
        }
        let window_class = object_getClass(window);
        if window_class != class(b"GPUIPanel\0")? && window_class != class(b"GPUIWindow\0")? {
            return Err(OverlayError::InvalidOwnedWindow);
        }
        validate_visibility_style(send!(window, b"styleMask\0", () -> usize))?;
        let content = send!(window, b"contentView\0", () -> Id);
        if content.is_null()
            || send!(view, b"superview\0", () -> Id) != content
            || send!(content, b"window\0", () -> Id) != window
            || !send!(window, b"parentWindow\0", () -> Id).is_null()
            || !send!(window, b"sheetParent\0", () -> Id).is_null()
            || !send!(window, b"attachedSheet\0", () -> Id).is_null()
        {
            return Err(OverlayError::InvalidOwnedWindow);
        }
        Ok(OwnedView {
            view,
            window,
            content,
        })
    }

    unsafe fn owned_hidden_panel(handle: WindowHandle<'_>) -> OverlayResult<OwnedView> {
        let owned = owned_window(handle)?;
        let window = owned.window;
        if object_getClass(window) != class(b"GPUIPanel\0")?
            || send!(window, b"styleMask\0", () -> usize) & NONACTIVATING_PANEL == 0
            || send!(window, b"isMiniaturized\0", () -> ObjcBool) != 0
        {
            return Err(OverlayError::InvalidOwnedWindow);
        }
        if send!(window, b"isVisible\0", () -> ObjcBool) != 0 {
            return Err(OverlayError::WindowAlreadyVisible);
        }
        Ok(owned)
    }

    unsafe fn application() -> OverlayResult<Id> {
        // A valid owned GPUI window establishes an existing running application.
        let application = send!(class(b"NSApplication\0")?, b"sharedApplication\0", () -> Id);
        if application.is_null() {
            Err(OverlayError::NativeConfigurationFailed)
        } else {
            Ok(application)
        }
    }

    unsafe fn window_context(window: Id, application: Id) -> OwnedWindowContext {
        let key = send!(application, b"keyWindow\0", () -> Id);
        OwnedWindowContext {
            application_active: send!(application, b"isActive\0", () -> ObjcBool) != 0,
            application_hidden: send!(application, b"isHidden\0", () -> ObjcBool) != 0,
            requester_visible: send!(window, b"isVisible\0", () -> ObjcBool) != 0
                && send!(window, b"isMiniaturized\0", () -> ObjcBool) == 0,
            key_window: if key.is_null() {
                OwnedKeyWindow::NoKeyWindow
            } else if key == window {
                OwnedKeyWindow::Requester
            } else {
                OwnedKeyWindow::OtherWindow
            },
        }
    }

    pub(super) fn context(handle: WindowHandle<'_>) -> OverlayResult<OwnedWindowContext> {
        ui_thread()?;
        unsafe {
            let _pool = Pool::new()?;
            let owned = owned_window(handle)?;
            Ok(window_context(owned.window, application()?))
        }
    }

    enum NativeOrder {
        Out,
        Back,
        Below(isize),
        Front,
    }

    unsafe fn ordering_without_animation(window: Id, order: NativeOrder) -> OverlayResult<()> {
        let animation = send!(window, b"animationBehavior\0", () -> isize);
        send!(window, b"setAnimationBehavior:\0", (isize => 2) -> ());
        if send!(window, b"animationBehavior\0", () -> isize) != 2 {
            send!(window, b"setAnimationBehavior:\0", (isize => animation) -> ());
            return Err(OverlayError::NativeConfigurationFailed);
        }
        // Source: beforeCapture/afterCapture uses orderOut/orderFrontRegardless.
        // Restore deliberately stays behind newer navigation. Exact SDK signatures:
        // -(void)orderOut:(id)sender; -(void)orderBack:(id)sender;
        // -(void)orderFrontRegardless;
        // -(void)orderWindow:(NSWindowOrderingMode)place relativeTo:(NSInteger)number;
        // NSWindowOrderingMode is NSInteger; NSWindowBelow = -1. No key mutation.
        // https://developer.apple.com/documentation/appkit/nswindow/order(_:relativeto:)
        match order {
            NativeOrder::Out => send!(window, b"orderOut:\0", (Id => std::ptr::null_mut()) -> ()),
            NativeOrder::Back => send!(window, b"orderBack:\0", (Id => std::ptr::null_mut()) -> ()),
            NativeOrder::Below(number) => {
                send!(window, b"orderWindow:relativeTo:\0", (isize => -1, isize => number) -> ())
            }
            NativeOrder::Front => send!(window, b"orderFrontRegardless\0", () -> ()),
        }
        send!(window, b"setAnimationBehavior:\0", (isize => animation) -> ());
        if send!(window, b"animationBehavior\0", () -> isize) != animation {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        Ok(())
    }

    pub(super) fn order_out(handle: WindowHandle<'_>) -> OverlayResult<bool> {
        ui_thread()?;
        unsafe {
            let _pool = Pool::new()?;
            let owned = owned_window(handle)?;
            if !window_context(owned.window, application()?).requester_visible {
                return Ok(false);
            }
            ordering_without_animation(owned.window, NativeOrder::Out)?;
            let after = owned_window(handle)?;
            if after.window != owned.window
                || after.content != owned.content
                || send!(after.window, b"isVisible\0", () -> ObjcBool) != 0
            {
                return Err(OverlayError::NativeConfigurationFailed);
            }
            Ok(true)
        }
    }

    pub(super) fn restore(handle: WindowHandle<'_>) -> OverlayResult<()> {
        ui_thread()?;
        unsafe {
            let _pool = Pool::new()?;
            let owned = owned_window(handle)?;
            let application = application()?;
            let before = window_context(owned.window, application);
            let order = restore_order(before);
            if order == RestoreOrder::Unchanged {
                return Ok(());
            }
            if send!(owned.window, b"isMiniaturized\0", () -> ObjcBool) != 0 {
                return Err(OverlayError::NavigationChanged);
            }
            // Ephemeral reference only, never retained or stored across calls.
            let key_before = send!(application, b"keyWindow\0", () -> Id);
            let target_level = send!(owned.window, b"level\0", () -> isize);
            let key_level = if order == RestoreOrder::BelowKey && !key_before.is_null() {
                Some(send!(key_before, b"level\0", () -> isize))
            } else {
                None
            };
            validate_restore_level(
                order,
                target_level,
                CGWindowLevelForKey(4) as isize,
                key_level,
            )?; // kCGNormalWindowLevelKey
            let native_order = match order {
                RestoreOrder::Unchanged => return Ok(()),
                RestoreOrder::Back => NativeOrder::Back,
                RestoreOrder::Front => NativeOrder::Front,
                RestoreOrder::BelowKey => {
                    if key_before.is_null()
                        || key_before == owned.window
                        || send!(key_before, b"isVisible\0", () -> ObjcBool) == 0
                        || send!(key_before, b"isMiniaturized\0", () -> ObjcBool) != 0
                        || validate_visibility_style(send!(key_before, b"styleMask\0", () -> usize))
                            .is_err()
                    {
                        return Err(OverlayError::UnsafeRestoration);
                    }
                    // AppKit keyWindow is an NSWindow owned by this application.
                    // Its stable positive windowNumber is only an ordering reference.
                    let number = send!(key_before, b"windowNumber\0", () -> isize);
                    if number <= 0 {
                        return Err(OverlayError::UnsafeRestoration);
                    }
                    NativeOrder::Below(number)
                }
            };
            ordering_without_animation(owned.window, native_order)?;
            let after_owned = owned_window(handle)?;
            let after = window_context(after_owned.window, application);
            if before.application_active != after.application_active
                || before.application_hidden != after.application_hidden
                || send!(application, b"keyWindow\0", () -> Id) != key_before
            {
                return Err(OverlayError::NavigationChanged);
            }
            if after_owned.window != owned.window
                || after_owned.content != owned.content
                || (!before.application_hidden && !after.requester_visible)
                || send!(after_owned.window, b"level\0", () -> isize) != target_level
            {
                return Err(OverlayError::NativeConfigurationFailed);
            }
            if order == RestoreOrder::BelowKey
                && send!(after_owned.window, b"orderedIndex\0", () -> isize)
                    <= send!(key_before, b"orderedIndex\0", () -> isize)
            {
                return Err(OverlayError::NativeConfigurationFailed);
            }
            Ok(())
        }
    }

    pub(super) struct DeactivationObserver {
        center: Id,
        token: Id,
        notifier: deactivation::Notifier,
        _ui_thread: std::marker::PhantomData<std::rc::Rc<()>>,
    }
    impl Drop for DeactivationObserver {
        fn drop(&mut self) {
            // Close first: queued callbacks can outlive native unregistration.
            // Main-thread construction plus !Send/!Sync owns native cleanup here.
            let wake = self.notifier.close();
            unsafe {
                send!(self.center, b"removeObserver:\0", (Id => self.token) -> ());
                send!(self.token, b"release\0", () -> ());
                send!(self.center, b"release\0", () -> ());
            }
            deactivation::wake_safely(wake);
        }
    }

    pub(super) fn observe_deactivation(
        cancellation: crate::native_capture::CaptureCancellation,
    ) -> OverlayResult<(DeactivationObserver, ApplicationDeactivationSignal)> {
        use block2::{Block, RcBlock};
        use std::panic::{catch_unwind, AssertUnwindSafe};
        ui_thread()?;
        unsafe {
            let _pool = Pool::new()?;
            let app = application()?;
            let center = send!(class(b"NSNotificationCenter\0")?, b"defaultCenter\0", () -> Id);
            let queue = send!(class(b"NSOperationQueue\0")?, b"mainQueue\0", () -> Id);
            let name = NSApplicationDidResignActiveNotification;
            if center.is_null() || queue.is_null() || name.is_null() {
                return Err(OverlayError::NativeConfigurationFailed);
            }
            let (notifier, signal) = deactivation::Notifier::pair();
            let callback_state = notifier.clone();
            let callback = move |_notification: Id| {
                // The source uses the main operation queue. Captured Rust state
                // is independently Send+Sync; no native object escapes or is read.
                let _ = catch_unwind(AssertUnwindSafe(|| callback_state.resigned(&cancellation)));
            };
            fn assert_send_sync<F: Send + Sync>(_: &F) {}
            assert_send_sync(&callback);
            let block = RcBlock::new(callback);
            // SDK: -(id<NSObject>)addObserverForName:(NSNotificationName)name
            // object:(id)object queue:(NSOperationQueue *)queue
            // usingBlock:(void (^)(NSNotification *))block;
            // Foundation strongly holds a copied block until unregistration.
            // Queued callbacks may still own a copy; the terminal claim makes
            // them inert after Drop. The block owns no guard/token/center cycle.
            // https://developer.apple.com/documentation/foundation/notificationcenter/addobserver(forname:object:queue:using:)
            // https://developer.apple.com/documentation/appkit/nsapplication/didresignactivenotification
            let token = send!(center, b"addObserverForName:object:queue:usingBlock:\0",
                (Id => name, Id => app, Id => queue, *const Block<dyn Fn(Id)> => &*block) -> Id);
            if token.is_null() {
                deactivation::wake_safely(notifier.close());
                return Err(OverlayError::NativeConfigurationFailed);
            }
            // Returned token is non-owned. Our +1 survives removeObserver until
            // the explicit release; the center is retained for that same lifetime.
            send!(token, b"retain\0", () -> Id);
            send!(center, b"retain\0", () -> Id);
            Ok((
                DeactivationObserver {
                    center,
                    token,
                    notifier,
                    _ui_thread: std::marker::PhantomData,
                },
                signal,
            ))
            // RcBlock drops locally; Foundation owns its copied registration.
        }
    }

    unsafe fn verify_configured(
        owned: &OwnedView,
        expected: MainDisplayOverlayLayout,
        expected_viewport: CaptureSize,
    ) -> OverlayResult<()> {
        let window = owned.window;
        let clear = send!(class(b"NSColor\0")?, b"clearColor\0", () -> Id);
        let level = CGWindowLevelForKey(SCREEN_SAVER_LEVEL_KEY) as isize;
        if clear.is_null() || level <= 0 {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        validate_overlay_frames(
            expected,
            expected_viewport,
            read_rect(window, b"frame\0")?,
            read_rect(owned.content, b"bounds\0")?,
            read_rect(owned.content, b"frame\0")?,
            read_rect(owned.view, b"bounds\0")?,
            read_rect(owned.view, b"frame\0")?,
        )?;
        if screen_id(send!(window, b"screen\0", () -> Id))? != expected.display.id {
            return Err(OverlayError::DisplayChanged);
        }
        let background = send!(window, b"backgroundColor\0", () -> Id);
        if send!(window, b"styleMask\0", () -> usize) != NONACTIVATING_PANEL
            || send!(window, b"level\0", () -> isize) != level
            || send!(window, b"collectionBehavior\0", () -> usize) != COLLECTION
            || send!(window, b"isFloatingPanel\0", () -> ObjcBool) == 0
            || send!(window, b"isOpaque\0", () -> ObjcBool) != 0
            || send!(background, b"isEqual:\0", (Id => clear) -> ObjcBool) == 0
            || send!(window, b"hasShadow\0", () -> ObjcBool) != 0
            || send!(window, b"hidesOnDeactivate\0", () -> ObjcBool) != 0
            || send!(window, b"isReleasedWhenClosed\0", () -> ObjcBool) != 0
            || send!(window, b"ignoresMouseEvents\0", () -> ObjcBool) != 0
            || send!(window, b"acceptsMouseMovedEvents\0", () -> ObjcBool) == 0
            || send!(window, b"becomesKeyOnlyIfNeeded\0", () -> ObjcBool) != 0
            || send!(window, b"canBecomeKeyWindow\0", () -> ObjcBool) == 0
            || send!(window, b"isMovable\0", () -> ObjcBool) != 0
            || send!(window, b"animationBehavior\0", () -> isize) != 2
        {
            return Err(OverlayError::NativeConfigurationFailed);
        }
        expected.revalidate(layout()?)?;
        Ok(())
    }

    unsafe fn presentation_context(
        requester: &OwnedView,
        popup: &OwnedView,
        app: Id,
        cancellation: &crate::native_capture::CaptureCancellation,
        stage: PresentationStage,
    ) -> OverlayResult<()> {
        validate_presentation_context(
            window_context(requester.window, app),
            send!(app, b"keyWindow\0", () -> Id) == popup.window,
            cancellation.is_cancelled(),
            stage,
        )
    }

    pub(super) fn present(
        popup_handle: WindowHandle<'_>,
        requester_handle: WindowHandle<'_>,
        expected: MainDisplayOverlayLayout,
        actual_viewport: CaptureSize,
        cancellation: &crate::native_capture::CaptureCancellation,
    ) -> OverlayResult<()> {
        ui_thread()?;
        expected.validate_viewport(actual_viewport)?;
        unsafe {
            let _pool = Pool::new()?;
            let popup = owned_hidden_panel(popup_handle)?;
            let requester = owned_window(requester_handle)?;
            if popup.window == requester.window {
                return Err(OverlayError::InvalidOwnedWindow);
            }
            let app = application()?;
            presentation_context(
                &requester,
                &popup,
                app,
                cancellation,
                PresentationStage::BeforeKey,
            )?;
            verify_configured(&popup, expected, actual_viewport)?;
            presentation_context(
                &requester,
                &popup,
                app,
                cancellation,
                PresentationStage::BeforeKey,
            )?;
            // Swift orderOverlayWindowsFront orders front before key acquisition.
            // Split makeKeyAndOrderFront into orderFrontRegardless/makeKeyWindow
            // to recheck cancellation after synchronous native ordering callbacks.
            // SDK: -(void)makeKeyWindow, no explicit arguments. No NSApp activation.
            // GPUI 0.2.2 MacWindow::activate instead queues a raw-pointer operation;
            // the two borrowed live owners here deliberately avoid that lifetime.
            // https://developer.apple.com/documentation/appkit/nswindow/makekey()
            let result = (|| -> OverlayResult<()> {
                ordering_without_animation(popup.window, NativeOrder::Front)?;
                let current_popup = owned_window(popup_handle)?;
                let current_requester = owned_window(requester_handle)?;
                if current_popup.window != popup.window
                    || current_popup.content != popup.content
                    || current_requester.window != requester.window
                    || current_requester.content != requester.content
                    || send!(current_popup.window, b"isVisible\0", () -> ObjcBool) == 0
                {
                    return Err(OverlayError::InvalidOwnedWindow);
                }
                presentation_context(
                    &current_requester,
                    &current_popup,
                    app,
                    cancellation,
                    PresentationStage::BeforeKey,
                )?;
                verify_configured(&current_popup, expected, actual_viewport)?;
                presentation_context(
                    &current_requester,
                    &current_popup,
                    app,
                    cancellation,
                    PresentationStage::BeforeKey,
                )?;
                send!(current_popup.window, b"makeKeyWindow\0", () -> ());
                let final_popup = owned_window(popup_handle)?;
                let final_requester = owned_window(requester_handle)?;
                if final_popup.window != popup.window
                    || final_popup.content != popup.content
                    || final_requester.window != requester.window
                    || final_requester.content != requester.content
                    || send!(final_popup.window, b"isVisible\0", () -> ObjcBool) == 0
                {
                    return Err(OverlayError::InvalidOwnedWindow);
                }
                verify_configured(&final_popup, expected, actual_viewport)?;
                presentation_context(
                    &final_requester,
                    &final_popup,
                    app,
                    cancellation,
                    PresentationStage::AfterKey,
                )
            })();
            if result.is_err() {
                // Restore the original hidden state only while exact ownership is
                // still verified. Never resurrect a closed/reparented view or alter
                // the requester/new key window to compensate for failed presentation.
                if let Ok(current) = owned_window(popup_handle) {
                    if current.window == popup.window && current.content == popup.content {
                        ordering_without_animation(current.window, NativeOrder::Out)?;
                        if send!(current.window, b"isVisible\0", () -> ObjcBool) != 0 {
                            return Err(OverlayError::NativeConfigurationFailed);
                        }
                    }
                }
            }
            result
        }
    }

    pub(super) fn configure(
        handle: WindowHandle<'_>,
        expected: MainDisplayOverlayLayout,
        expected_viewport: CaptureSize,
    ) -> OverlayResult<()> {
        ui_thread()?;
        expected.validate_viewport(expected_viewport)?;
        unsafe {
            let _pool = Pool::new()?;
            expected.revalidate(layout()?)?;
            let owned = owned_hidden_panel(handle)?;
            let window = owned.window;
            let clear = send!(class(b"NSColor\0")?, b"clearColor\0", () -> Id);
            if clear.is_null() {
                return Err(OverlayError::NativeConfigurationFailed);
            }
            let level = isize::try_from(CGWindowLevelForKey(SCREEN_SAVER_LEVEL_KEY))
                .map_err(|_| OverlayError::NativeConfigurationFailed)?;
            if level <= 0 {
                return Err(OverlayError::NativeConfigurationFailed);
            }

            // GPUI created this as an NSPanel with the nonactivating bit already
            // present. Keep that bit while removing all titled/window chrome.
            // Never toggle nonactivation onto an existing ordinary NSWindow.
            send!(window, b"setStyleMask:\0", (usize => NONACTIVATING_PANEL) -> ());
            send!(window, b"setFloatingPanel:\0", (ObjcBool => 1) -> ());
            send!(window, b"setLevel:\0", (isize => level) -> ());
            send!(window, b"setCollectionBehavior:\0", (usize => COLLECTION) -> ());
            send!(window, b"setOpaque:\0", (ObjcBool => 0) -> ());
            send!(window, b"setBackgroundColor:\0", (Id => clear) -> ());
            send!(window, b"setHasShadow:\0", (ObjcBool => 0) -> ());
            send!(window, b"setHidesOnDeactivate:\0", (ObjcBool => 0) -> ());
            send!(window, b"setReleasedWhenClosed:\0", (ObjcBool => 0) -> ());
            send!(window, b"setIgnoresMouseEvents:\0", (ObjcBool => 0) -> ());
            send!(window, b"setAcceptsMouseMovedEvents:\0", (ObjcBool => 1) -> ());
            send!(window, b"setBecomesKeyOnlyIfNeeded:\0", (ObjcBool => 0) -> ());
            send!(window, b"setMovable:\0", (ObjcBool => 0) -> ());
            // NSWindowAnimationBehaviorNone = 2. The source uses no show/hide
            // animation for the picker; the GPUI default is UtilityWindow.
            send!(window, b"setAnimationBehavior:\0", (isize => 2) -> ());
            // Exact SDK signature: -(void)setFrame:(NSRect)frame display:(BOOL).
            // This is a struct ARGUMENT with void result, not a struct-return call.
            send!(window, b"setFrame:display:\0", (CaptureRect => expected.cocoa_frame, ObjcBool => 0) -> ());

            let after = owned_hidden_panel(handle)?;
            if after.view != owned.view || after.window != window || after.content != owned.content
            {
                return Err(OverlayError::InvalidOwnedWindow);
            }
            verify_configured(&owned, expected, expected_viewport)?;
            // The caller exclusively controls ordering/presentation after Ok.
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_capture::CapturePixelSize;

    fn main_layout() -> MainDisplayOverlayLayout {
        MainDisplayOverlayLayout {
            display: CaptureDisplay {
                id: 17,
                bounds: CaptureRect::new(0., 0., 1440., 900.),
                pixels: CapturePixelSize {
                    width: 2880,
                    height: 1800,
                },
            },
            cocoa_frame: CaptureRect::new(0., 0., 1440., 900.),
            rotation_degrees: 0,
            backing_scale: 2.,
        }
    }

    #[test]
    fn cg_cocoa_conversion_uses_primary_top_not_desktop_maximum() {
        let left_above = CaptureRect::new(-1600., -1200., 1600., 1200.);
        let cocoa = core_graphics_cocoa_rect(left_above, 900.).unwrap();
        assert_eq!(cocoa, CaptureRect::new(-1600., 900., 1600., 1200.));
        assert_eq!(core_graphics_cocoa_rect(cocoa, 900.).unwrap(), left_above);
        let below = CaptureRect::new(100., 900., 1000., 700.);
        assert_eq!(
            core_graphics_cocoa_rect(below, 900.).unwrap(),
            CaptureRect::new(100., -700., 1000., 700.)
        );
    }

    #[test]
    fn native_pixel_ratios_do_not_assume_backing_scale_or_equal_axes() {
        let mut layout = main_layout();
        layout.display.pixels = CapturePixelSize {
            width: 2160,
            height: 1800,
        };
        assert_eq!(layout.pixel_ratios().unwrap(), (1.5, 2.));
        assert_eq!(layout.backing_scale, 2.);
        assert!(layout.validate().is_ok());
        // Pixel ratios are origin independent, including negative-origin fixtures.
        layout.cocoa_frame.origin.x = -1440.;
        layout.cocoa_frame.origin.y = -900.;
        assert_eq!(layout.pixel_ratios().unwrap(), (1.5, 2.));
    }

    #[test]
    fn production_rejects_secondary_and_rotated_layouts() {
        let baseline = main_layout();
        for rotation in [90, 180, 270, 360] {
            assert_eq!(
                MainDisplayOverlayLayout {
                    rotation_degrees: rotation,
                    ..baseline
                }
                .validate(),
                Err(OverlayError::UnsupportedLayout)
            );
        }
        let mut secondary = baseline;
        secondary.display.bounds.origin.x = -1440.;
        secondary.cocoa_frame.origin.x = -1440.;
        assert_eq!(secondary.validate(), Err(OverlayError::UnsupportedLayout));
        secondary = baseline;
        secondary.cocoa_frame.origin.y = 900.;
        assert_eq!(secondary.validate(), Err(OverlayError::UnsupportedLayout));
    }

    #[test]
    fn malformed_geometry_and_unbounded_ratios_fail_closed() {
        for bad in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.,
            -1.,
            1_000_001.,
        ] {
            assert!(core_graphics_cocoa_rect(CaptureRect::new(0., 0., bad, 20.), 900.).is_err());
        }
        assert!(core_graphics_cocoa_rect(CaptureRect::new(0., 0., 20., 20.), f64::NAN).is_err());
        for bad in [0., -1., f64::NAN, 17.] {
            assert_eq!(
                MainDisplayOverlayLayout {
                    backing_scale: bad,
                    ..main_layout()
                }
                .validate(),
                Err(OverlayError::InvalidGeometry)
            );
        }
        let mut zero_id = main_layout();
        zero_id.display.id = 0;
        assert!(zero_id.validate().is_err());
        let mut huge_ratio = main_layout();
        huge_ratio.cocoa_frame.size.width = 100.;
        assert!(huge_ratio.pixel_ratios().is_err());
    }

    #[test]
    fn every_frozen_topology_change_invalidates_the_layout() {
        let original = main_layout();
        assert!(original.revalidate(original).is_ok());
        let mut changes = [original; 7];
        changes[0].display.id += 1;
        changes[1].display.bounds.origin.x = 0.001;
        changes[2].display.bounds.size.width += 1.;
        changes[3].display.pixels.width += 1;
        changes[4].cocoa_frame.size.height += 1.;
        changes[5].backing_scale = 1.;
        changes[6].rotation_degrees = 90;
        for changed in changes {
            assert_eq!(
                original.revalidate(changed),
                Err(OverlayError::DisplayChanged)
            );
        }
    }

    #[test]
    fn native_window_content_and_renderer_must_all_fill_the_display() {
        let layout = main_layout();
        let local = CaptureRect::new(0., 0., 1440., 900.);
        assert!(
            validate_overlay_frames(layout, local.size, local, local, local, local, local).is_ok()
        );
        let wrong = [
            CaptureRect::new(0., 22., 1440., 900.), // ordinary titlebar frame
            CaptureRect::new(0., 0., 1440., 878.),  // work area/titlebar subtraction
            CaptureRect::new(1., 0., 1440., 900.),  // shifted content/renderer
            CaptureRect::new(0., 0., 2880., 1800.), // pixels confused with points
        ];
        for bad in wrong {
            for slot in 0..5 {
                let mut frames = [local; 5];
                frames[slot] = bad;
                assert_eq!(
                    validate_overlay_frames(
                        layout, local.size, frames[0], frames[1], frames[2], frames[3], frames[4]
                    ),
                    Err(OverlayError::ViewportMismatch)
                );
            }
        }
        assert_eq!(
            layout.validate_viewport(CaptureSize {
                width: 1439.,
                height: 900.
            }),
            Err(OverlayError::ViewportMismatch)
        );
        assert_eq!(
            layout.validate_viewport(CaptureSize {
                width: f64::NAN,
                height: 900.
            }),
            Err(OverlayError::ViewportMismatch)
        );
    }

    #[test]
    fn rect_layout_matches_the_audited_four_double_c_abi() {
        assert_eq!(std::mem::size_of::<CaptureRect>(), 32);
        assert_eq!(
            std::mem::align_of::<CaptureRect>(),
            std::mem::align_of::<f64>()
        );
        assert_eq!(std::mem::offset_of!(CaptureRect, origin), 0);
        assert_eq!(std::mem::offset_of!(CaptureRect, size), 16);
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    #[test]
    fn unsupported_hosts_never_attempt_native_metadata_or_window_actions() {
        assert_eq!(
            main_display_overlay_layout(),
            Err(OverlayError::Unsupported)
        );
    }

    fn capture_context(key_window: OwnedKeyWindow) -> OwnedWindowContext {
        OwnedWindowContext {
            application_active: true,
            application_hidden: false,
            requester_visible: false,
            key_window,
        }
    }

    #[test]
    fn capture_context_allows_only_current_active_unhidden_requester_or_no_key() {
        for key in [OwnedKeyWindow::NoKeyWindow, OwnedKeyWindow::Requester] {
            for requester_visible in [false, true] {
                let context = OwnedWindowContext {
                    requester_visible,
                    ..capture_context(key)
                };
                assert!(context.allows_capture_continuation(7, 7));
                assert!(!context.allows_capture_continuation(7, 8));
                assert!(!context.allows_capture_continuation(8, 7));
            }
        }
    }

    #[test]
    fn capture_context_rejects_inactive_hidden_and_foreign_key_independently() {
        for key in [
            OwnedKeyWindow::NoKeyWindow,
            OwnedKeyWindow::Requester,
            OwnedKeyWindow::OtherWindow,
        ] {
            assert!(!OwnedWindowContext {
                application_active: false,
                ..capture_context(key)
            }
            .allows_capture_continuation(7, 7));
            assert!(!OwnedWindowContext {
                application_hidden: true,
                ..capture_context(key)
            }
            .allows_capture_continuation(7, 7));
        }
        assert!(!capture_context(OwnedKeyWindow::OtherWindow).allows_capture_continuation(7, 7));
    }

    #[test]
    fn restored_visibility_or_focus_does_not_revalidate_stale_capture() {
        // Even a return to the same requester after newer navigation cannot make
        // an old capture current. Parent owns monotonic generation/liveness.
        let restored = OwnedWindowContext {
            requester_visible: true,
            ..capture_context(OwnedKeyWindow::Requester)
        };
        assert!(!restored.allows_capture_continuation(41, 42));
        assert!(!restored.allows_capture_continuation(u64::MAX, 0));
    }

    #[test]
    fn fullscreen_preflight_checks_the_native_style_bit_not_frame_geometry() {
        assert!(validate_visibility_style(0).is_ok());
        assert!(validate_visibility_style((1 << 0) | (1 << 7)).is_ok());
        assert_eq!(
            validate_visibility_style(1 << 14),
            Err(OverlayError::FullscreenWindow)
        );
        assert_eq!(
            validate_visibility_style((1 << 14) | (1 << 0)),
            Err(OverlayError::FullscreenWindow)
        );
    }

    #[test]
    fn restore_policy_keeps_visible_windows_unchanged_even_after_navigation() {
        for active in [false, true] {
            for hidden in [false, true] {
                for key_window in [
                    OwnedKeyWindow::NoKeyWindow,
                    OwnedKeyWindow::Requester,
                    OwnedKeyWindow::OtherWindow,
                ] {
                    let context = OwnedWindowContext {
                        application_active: active,
                        application_hidden: hidden,
                        requester_visible: true,
                        key_window,
                    };
                    assert_eq!(restore_order(context), RestoreOrder::Unchanged);
                }
            }
        }
    }

    #[test]
    fn restore_policy_uses_back_for_inactive_or_hidden_apps() {
        for key in [
            OwnedKeyWindow::NoKeyWindow,
            OwnedKeyWindow::Requester,
            OwnedKeyWindow::OtherWindow,
        ] {
            assert_eq!(
                restore_order(OwnedWindowContext {
                    application_active: false,
                    ..capture_context(key)
                }),
                RestoreOrder::Back
            );
            assert_eq!(
                restore_order(OwnedWindowContext {
                    application_hidden: true,
                    ..capture_context(key)
                }),
                RestoreOrder::Back
            );
        }
    }

    #[test]
    fn restore_policy_stays_below_competing_key_and_fronts_only_without_one() {
        assert_eq!(
            restore_order(capture_context(OwnedKeyWindow::OtherWindow)),
            RestoreOrder::BelowKey
        );
        for key in [OwnedKeyWindow::NoKeyWindow, OwnedKeyWindow::Requester] {
            assert_eq!(restore_order(capture_context(key)), RestoreOrder::Front);
        }
    }

    #[test]
    fn restore_policy_never_claims_cross_level_ordering_is_safe() {
        assert!(validate_restore_level(RestoreOrder::Back, 0, 0, None).is_ok());
        assert_eq!(
            validate_restore_level(RestoreOrder::Back, 101, 0, None),
            Err(OverlayError::UnsafeRestoration)
        );
        assert!(validate_restore_level(RestoreOrder::BelowKey, 0, 0, Some(0)).is_ok());
        for key_level in [None, Some(101), Some(-1)] {
            assert_eq!(
                validate_restore_level(RestoreOrder::BelowKey, 0, 0, key_level),
                Err(OverlayError::UnsafeRestoration)
            );
        }
        assert!(validate_restore_level(RestoreOrder::Front, 1000, 0, None).is_ok());
        assert!(validate_restore_level(RestoreOrder::Unchanged, 1000, 0, Some(0)).is_ok());
    }

    #[test]
    fn presentation_before_key_requires_active_unhidden_no_key_or_exact_requester() {
        for key in [OwnedKeyWindow::NoKeyWindow, OwnedKeyWindow::Requester] {
            assert!(validate_presentation_context(
                capture_context(key),
                false,
                false,
                PresentationStage::BeforeKey
            )
            .is_ok());
        }
        assert!(validate_presentation_context(
            capture_context(OwnedKeyWindow::OtherWindow),
            false,
            false,
            PresentationStage::BeforeKey
        )
        .is_err());
        for context in [
            OwnedWindowContext {
                application_active: false,
                ..capture_context(OwnedKeyWindow::Requester)
            },
            OwnedWindowContext {
                application_hidden: true,
                ..capture_context(OwnedKeyWindow::Requester)
            },
        ] {
            assert!(validate_presentation_context(
                context,
                false,
                false,
                PresentationStage::BeforeKey
            )
            .is_err());
        }
    }

    #[test]
    fn presentation_cancellation_wins_before_and_after_native_callbacks() {
        for stage in [PresentationStage::BeforeKey, PresentationStage::AfterKey] {
            for key in [
                OwnedKeyWindow::NoKeyWindow,
                OwnedKeyWindow::Requester,
                OwnedKeyWindow::OtherWindow,
            ] {
                for popup_is_key in [false, true] {
                    assert_eq!(
                        validate_presentation_context(
                            capture_context(key),
                            popup_is_key,
                            true,
                            stage
                        ),
                        Err(OverlayError::NavigationChanged)
                    );
                }
            }
        }
    }

    #[test]
    fn presentation_after_key_requires_only_the_visible_picker_to_be_key() {
        let context = capture_context(OwnedKeyWindow::OtherWindow);
        assert!(
            validate_presentation_context(context, true, false, PresentationStage::AfterKey)
                .is_ok()
        );
        assert!(
            validate_presentation_context(context, false, false, PresentationStage::AfterKey)
                .is_err()
        );
        assert!(
            validate_presentation_context(context, true, false, PresentationStage::BeforeKey)
                .is_err()
        );
        for key in [OwnedKeyWindow::NoKeyWindow, OwnedKeyWindow::Requester] {
            assert!(validate_presentation_context(
                capture_context(key),
                false,
                false,
                PresentationStage::AfterKey
            )
            .is_err());
        }
        assert!(validate_presentation_context(
            OwnedWindowContext {
                application_active: false,
                ..context
            },
            true,
            false,
            PresentationStage::AfterKey
        )
        .is_err());
        assert!(validate_presentation_context(
            OwnedWindowContext {
                application_hidden: true,
                ..context
            },
            true,
            false,
            PresentationStage::AfterKey
        )
        .is_err());
    }
}
