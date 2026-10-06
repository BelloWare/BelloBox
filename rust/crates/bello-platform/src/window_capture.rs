//! Reconstructed pure policy for a future independent-window screenshot adapter.
//!
//! Source: CaptureWindowCatalog's active CG overlay catalog, not the legacy list
//! picker. No enumeration, permissions, native capture, native pointers, UI or I/O
//! occurs here. This narrower candidate accepts normal-layer windows wholly in
//! the unrotated main display. There is no visible-frame/display-crop fallback.
//!
//! Selected CG metadata, callback-local SCWindow submission evidence and fresh CG
//! observations are distinct. A plan binds metadata from the exact SCWindow used
//! to construct the filter in the content callback, then copies only Rust evidence.
//! Completion checks fresh CG/topology/session/cancellation/image-size inputs;
//! it does not reread that SCWindow or revalidate its native bundle. These checks
//! do not prove atomic window incarnation. A future adapter must disable audio
//! and preserve Swift's shadow/opacity/clip defaults.

use crate::native_capture::{
    CaptureCancellation, CaptureDisplay, CapturePixelSize, CaptureRect, CaptureRequest,
    CaptureSize, MAX_CAPTURE_TIMEOUT,
};
use std::{fmt, sync::Arc, time::Duration};

pub const MAX_WINDOW_CANDIDATES: usize = 512;
pub const MAX_WINDOW_IDENTITY_BYTES: usize = 1_024;
const MAX_TOPOLOGY_DISPLAYS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowPolicyError {
    Cancelled,
    InvalidMetadata,
    InvalidGeometry,
    InvalidOptions,
    UnsupportedGeometry,
    IneligibleWindow,
    WindowNotFound,
    WindowChanged,
    AmbiguousWindow,
    TopologyChanged,
    StaleSelection,
    OutputTooLarge,
    UnexpectedImageSize,
}
impl fmt::Display for WindowPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Window capture was cancelled.",
            Self::InvalidMetadata => "Window metadata is invalid or exceeds its limit.",
            Self::InvalidGeometry => "Window capture geometry is invalid.",
            Self::InvalidOptions => "Window capture options are invalid.",
            Self::UnsupportedGeometry => {
                "Window capture requires an unrotated main display containing the whole window."
            }
            Self::IneligibleWindow => {
                "The selected window is not eligible for independent capture."
            }
            Self::WindowNotFound => "The selected window is no longer available. Select it again.",
            Self::WindowChanged => "The selected window changed. Select it again.",
            Self::AmbiguousWindow => "The selected window cannot be resolved uniquely.",
            Self::TopologyChanged => "The display configuration changed. Select the window again.",
            Self::StaleSelection => "The window selection is no longer current.",
            Self::OutputTooLarge => "The window screenshot exceeds the bounded output size.",
            Self::UnexpectedImageSize => {
                "The captured window image does not match the planned dimensions."
            }
        })
    }
}
impl std::error::Error for WindowPolicyError {}
pub type WindowPolicyResult<T> = Result<T, WindowPolicyError>;

/// Global CoreGraphics top-left POINTS, never image pixels or Cocoa coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowFrame(pub CaptureRect);

/// Borrowed strings are validated before the policy makes bounded owned copies.
/// Titles and application display names are mutable labels, deliberately absent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WindowIdentity<'a> {
    pub window_id: u32,
    pub owner_process_id: i32,
    /// Active CG catalog data has None. Separately identity-matched native source
    /// metadata may enrich it at submission. Missing CG data never erases that
    /// evidence, but it does not independently revalidate the native bundle.
    pub owner_bundle_id: Option<&'a str>,
}
impl fmt::Debug for WindowIdentity<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowIdentity")
            .field("window_id", &self.window_id)
            .field("owner_process_id", &self.owner_process_id)
            .field("has_bundle_id", &self.owner_bundle_id.is_some())
            .finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowObservation<'a> {
    pub identity: WindowIdentity<'a>,
    pub frame: WindowFrame,
    pub layer: i64,
    pub alpha: f64,
    pub on_screen: bool,
}
/// Immutable submission evidence, read from the exact callback-local SCWindow
/// used to create the content filter. This is not a native object owner and is
/// never supplied as a purported fresh native-object observation at completion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubmittedWindowSource<'a> {
    pub identity: WindowIdentity<'a>,
    pub frame: WindowFrame,
    pub layer: i64,
    pub on_screen: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowDisplayGeometry {
    /// Global CG point bounds and CG pixel dimensions.
    pub display: CaptureDisplay,
    /// NSScreen frame SIZE in points, independently supplied by the native host.
    pub appkit_size_points: CaptureSize,
    pub backing_scale: f64,
    pub rotation_degrees: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct WindowTopology<'a> {
    pub main_display_id: u32,
    pub displays: &'a [WindowDisplayGeometry],
}
/// Caller-owned session/generation bookkeeping. Neither field is a native object
/// identity or a permission token. A reopened host must use a different session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSelectionToken {
    pub session: u64,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowCaptureOptions {
    pub include_cursor: bool,
    /// Policy validates the limit; a future native Job must enforce the deadline.
    pub timeout: Duration,
}
impl Default for WindowCaptureOptions {
    fn default() -> Self {
        Self {
            include_cursor: false,
            timeout: MAX_CAPTURE_TIMEOUT,
        }
    }
}

struct OwnedIdentity {
    window_id: u32,
    owner_process_id: i32,
    bundle: Option<String>,
}
impl fmt::Debug for OwnedIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.borrowed().fmt(f)
    }
}
impl OwnedIdentity {
    fn copy(value: WindowIdentity<'_>) -> WindowPolicyResult<Self> {
        validate_identity(value)?;
        let bundle = if let Some(text) = value.owner_bundle_id {
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| WindowPolicyError::InvalidMetadata)?;
            owned.push_str(text);
            Some(owned)
        } else {
            None
        };
        Ok(Self {
            window_id: value.window_id,
            owner_process_id: value.owner_process_id,
            bundle,
        })
    }
    fn borrowed(&self) -> WindowIdentity<'_> {
        WindowIdentity {
            window_id: self.window_id,
            owner_process_id: self.owner_process_id,
            owner_bundle_id: self.bundle.as_deref(),
        }
    }
}
#[derive(Debug)]
struct Selection {
    identity: OwnedIdentity,
    frame: WindowFrame,
    own_process_id: i32,
    token: WindowSelectionToken,
    main_display_id: u32,
    topology: Vec<WindowDisplayGeometry>,
    cancellation: CaptureCancellation,
}
#[derive(Clone, Debug)]
pub struct WindowCaptureSelection(Arc<Selection>);

/// A Window-only plan; no conversion to a display/region request or fallback.
#[derive(Debug)]
pub struct WindowCapturePlan {
    selection: WindowCaptureSelection,
    submitted_identity: OwnedIdentity,
    output_size: CapturePixelSize,
    options: WindowCaptureOptions,
    cancellation: CaptureCancellation,
}

impl WindowCaptureSelection {
    /// Identifies which SCWindow the content callback may submit. This accessor
    /// does not enumerate content or authorize a capture on its own.
    pub fn selected_identity(&self) -> WindowIdentity<'_> {
        self.0.identity.borrowed()
    }

    /// Allows the native entry/continuations to reject a cancelled selection
    /// before enumeration, even when their operation has a fresh job flag.
    pub fn is_cancelled(&self) -> bool {
        self.0.cancellation.is_cancelled()
    }

    pub fn new(
        selected: WindowObservation<'_>,
        topology: WindowTopology<'_>,
        own_process_id: i32,
        token: WindowSelectionToken,
        cancellation: &CaptureCancellation,
    ) -> WindowPolicyResult<Self> {
        check_cancel(cancellation)?;
        if own_process_id <= 0 || token.session == 0 {
            return Err(WindowPolicyError::InvalidMetadata);
        }
        validate_observation(selected)?;
        eligible(selected, own_process_id)?;
        let main = validate_topology(topology)?;
        output_size(selected.frame, main)?;
        // Only bounded validated identity/topology is copied into a selection.
        let identity = OwnedIdentity::copy(selected.identity)?;
        let mut displays = Vec::new();
        displays
            .try_reserve_exact(topology.displays.len())
            .map_err(|_| WindowPolicyError::InvalidMetadata)?;
        displays.extend_from_slice(topology.displays);
        check_cancel(cancellation)?;
        Ok(Self(Arc::new(Selection {
            identity,
            frame: selected.frame,
            own_process_id,
            token,
            main_display_id: topology.main_display_id,
            topology: displays,
            cancellation: cancellation.clone(),
        })))
    }

    /// Validates the callback-local native source before it is submitted and
    /// freezes its identity as evidence. The caller must construct the filter
    /// from that exact source object, not a later lookup. Cancellation cannot be
    /// reset by passing a fresh flag.
    pub fn plan(
        &self,
        submission: SubmittedWindowSource<'_>,
        fresh: &[WindowObservation<'_>],
        topology: WindowTopology<'_>,
        token: WindowSelectionToken,
        options: WindowCaptureOptions,
        cancellation: &CaptureCancellation,
    ) -> WindowPolicyResult<WindowCapturePlan> {
        self.check_cancellation(cancellation)?;
        if options.timeout.is_zero() || options.timeout > MAX_CAPTURE_TIMEOUT {
            return Err(WindowPolicyError::InvalidOptions);
        }
        if token != self.0.token {
            return Err(WindowPolicyError::StaleSelection);
        }
        self.validate_submission(submission)?;
        let observed = self.validate_current(fresh, topology, token, cancellation)?;
        if !matches_known(observed.identity, submission.identity) {
            return Err(WindowPolicyError::WindowChanged);
        }
        let output_size = output_size(self.0.frame, validate_topology(topology)?)?;
        let submitted_identity = OwnedIdentity::copy(submission.identity)?;
        self.check_cancellation(cancellation)?;
        Ok(WindowCapturePlan {
            selection: self.clone(),
            submitted_identity,
            output_size,
            options,
            cancellation: cancellation.clone(),
        })
    }

    fn check_cancellation(&self, boundary: &CaptureCancellation) -> WindowPolicyResult<()> {
        check_cancel(&self.0.cancellation)?;
        check_cancel(boundary)
    }

    fn validate_submission(&self, submission: SubmittedWindowSource<'_>) -> WindowPolicyResult<()> {
        validate_identity(submission.identity)?;
        validate_frame(submission.frame)?;
        if submission.layer != 0 || !submission.on_screen {
            return Err(WindowPolicyError::IneligibleWindow);
        }
        if !matches_known(self.0.identity.borrowed(), submission.identity)
            || submission.frame != self.0.frame
        {
            return Err(WindowPolicyError::WindowChanged);
        }
        Ok(())
    }

    fn validate_current<'a>(
        &self,
        fresh: &[WindowObservation<'a>],
        topology: WindowTopology<'_>,
        token: WindowSelectionToken,
        cancellation: &CaptureCancellation,
    ) -> WindowPolicyResult<WindowObservation<'a>> {
        self.check_cancellation(cancellation)?;
        if token != self.0.token {
            return Err(WindowPolicyError::StaleSelection);
        }
        validate_topology(topology)?;
        if topology.main_display_id != self.0.main_display_id
            || topology.displays.len() != self.0.topology.len()
            || self
                .0
                .topology
                .iter()
                .any(|expected| !topology.displays.contains(expected))
        {
            return Err(WindowPolicyError::TopologyChanged);
        }
        if fresh.len() > MAX_WINDOW_CANDIDATES {
            return Err(WindowPolicyError::InvalidMetadata);
        }
        let mut found = None;
        for observed in fresh {
            self.check_cancellation(cancellation)?;
            validate_observation(*observed)?;
            if observed.identity.window_id == self.0.identity.window_id
                && found.replace(*observed).is_some()
            {
                return Err(WindowPolicyError::AmbiguousWindow);
            }
        }
        let observed = found.ok_or(WindowPolicyError::WindowNotFound)?;
        eligible(observed, self.0.own_process_id)?;
        if observed.identity.owner_process_id != self.0.identity.owner_process_id
            || observed.frame != self.0.frame
        {
            return Err(WindowPolicyError::WindowChanged);
        }
        self.check_cancellation(cancellation)?;
        Ok(observed)
    }
}
impl WindowCapturePlan {
    pub fn output_size(&self) -> CapturePixelSize {
        self.output_size
    }
    pub fn options(&self) -> WindowCaptureOptions {
        self.options
    }
    /// What was bound into the filter at submission, not fresh native metadata.
    pub fn submitted_identity(&self) -> WindowIdentity<'_> {
        self.submitted_identity.borrowed()
    }

    fn check_cancellation(&self, boundary: &CaptureCancellation) -> WindowPolicyResult<()> {
        self.selection.check_cancellation(boundary)?;
        check_cancel(&self.cancellation)
    }

    /// Recheck fresh CG metadata after encoding; there is deliberately no native
    /// source argument. Missing CG bundle information makes no claim, while any
    /// supplied bundle must agree with immutable submission evidence. This does
    /// not reread the SCWindow/native bundle, prove incarnation or authenticate
    /// the image. The host still owns exactly-once Job acceptance and the final
    /// publication-time session/cancellation gate.
    pub fn validate_completion(
        &self,
        fresh: &[WindowObservation<'_>],
        topology: WindowTopology<'_>,
        token: WindowSelectionToken,
        image_size: CapturePixelSize,
        cancellation: &CaptureCancellation,
    ) -> WindowPolicyResult<()> {
        self.check_cancellation(cancellation)?;
        let observed = self
            .selection
            .validate_current(fresh, topology, token, cancellation)?;
        if !matches_known(observed.identity, self.submitted_identity.borrowed()) {
            return Err(WindowPolicyError::WindowChanged);
        }
        image_size
            .validate()
            .map_err(|_| WindowPolicyError::OutputTooLarge)?;
        if image_size != self.output_size {
            return Err(WindowPolicyError::UnexpectedImageSize);
        }
        self.check_cancellation(cancellation)
    }
}

fn check_cancel(flag: &CaptureCancellation) -> WindowPolicyResult<()> {
    if flag.is_cancelled() {
        Err(WindowPolicyError::Cancelled)
    } else {
        Ok(())
    }
}
fn validate_identity(value: WindowIdentity<'_>) -> WindowPolicyResult<()> {
    if value.window_id == 0
        || value.owner_process_id <= 0
        || value.owner_bundle_id.is_some_and(|s| {
            s.is_empty() || s.len() > MAX_WINDOW_IDENTITY_BYTES || s.chars().any(char::is_control)
        })
    {
        Err(WindowPolicyError::InvalidMetadata)
    } else {
        Ok(())
    }
}
fn matches_known(expected: WindowIdentity<'_>, actual: WindowIdentity<'_>) -> bool {
    expected.window_id == actual.window_id
        && expected.owner_process_id == actual.owner_process_id
        && expected
            .owner_bundle_id
            .map_or(true, |s| actual.owner_bundle_id == Some(s))
}
fn validate_frame(frame: WindowFrame) -> WindowPolicyResult<()> {
    let r = frame.0;
    if [r.origin.x, r.origin.y, r.size.width, r.size.height]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        || r.size.width <= 0.
        || r.size.height <= 0.
    {
        Err(WindowPolicyError::InvalidGeometry)
    } else {
        Ok(())
    }
}
fn validate_observation(value: WindowObservation<'_>) -> WindowPolicyResult<()> {
    validate_identity(value.identity)?;
    validate_frame(value.frame)?;
    if !value.alpha.is_finite() || !(0.0..=1.0).contains(&value.alpha) {
        return Err(WindowPolicyError::InvalidMetadata);
    }
    Ok(())
}
fn eligible(value: WindowObservation<'_>, own_pid: i32) -> WindowPolicyResult<()> {
    let width = value.frame.0.size.width.floor();
    let height = value.frame.0.size.height.floor();
    // CaptureWindowCatalog uses floored dimensions and alpha > .01. Normal layer
    // is independent-window; its other allowed layers are visible-frame surfaces.
    if value.identity.owner_process_id == own_pid
        || !value.on_screen
        || value.layer != 0
        || value.alpha <= 0.01
        || width < 8.
        || height < 8.
        || width * height < 96.
    {
        Err(WindowPolicyError::IneligibleWindow)
    } else {
        Ok(())
    }
}
fn validate_topology(topology: WindowTopology<'_>) -> WindowPolicyResult<WindowDisplayGeometry> {
    if topology.main_display_id == 0
        || topology.displays.is_empty()
        || topology.displays.len() > MAX_TOPOLOGY_DISPLAYS
    {
        return Err(WindowPolicyError::InvalidGeometry);
    }
    for (index, item) in topology.displays.iter().enumerate() {
        // Pure validation of display metadata; never calls capture or selects a
        // display as a substitute for the selected Window target.
        CaptureRequest::full_display(item.display)
            .validate()
            .map_err(|_| WindowPolicyError::InvalidGeometry)?;
        if topology.displays[..index]
            .iter()
            .any(|old| old.display.id == item.display.id)
            || !item.backing_scale.is_finite()
            || !(0.25..=16.).contains(&item.backing_scale)
            || !item.rotation_degrees.is_finite()
            || !(0.0..360.0).contains(&item.rotation_degrees)
            || [
                item.appkit_size_points.width,
                item.appkit_size_points.height,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 1. || *v > 1_000_000.)
        {
            return Err(WindowPolicyError::InvalidGeometry);
        }
    }
    let main = topology
        .displays
        .iter()
        .find(|d| d.display.id == topology.main_display_id)
        .copied()
        .ok_or(WindowPolicyError::InvalidGeometry)?;
    if main.rotation_degrees != 0.
        || main.display.bounds.origin.x != 0.
        || main.display.bounds.origin.y != 0.
        || main.display.bounds.size != main.appkit_size_points
    {
        return Err(WindowPolicyError::UnsupportedGeometry);
    }
    Ok(main)
}
fn output_size(
    frame: WindowFrame,
    display: WindowDisplayGeometry,
) -> WindowPolicyResult<CapturePixelSize> {
    let r = frame.0;
    let bounds = display.display.bounds;
    if r.origin.x < bounds.origin.x
        || r.origin.y < bounds.origin.y
        || r.origin.x + r.size.width > bounds.origin.x + bounds.size.width
        || r.origin.y + r.size.height > bounds.origin.y + bounds.size.height
    {
        return Err(WindowPolicyError::UnsupportedGeometry);
    }
    // Source: ScreenCaptureService.windowPixelSize + ScreenCoordinateSpace.
    // Do not use CG pixels alone or round the window points before scaling.
    let axis = |points: f64, screen_points: f64, cg_pixels: u32| -> WindowPolicyResult<u32> {
        let effective = f64::from(cg_pixels).max(screen_points * display.backing_scale);
        let pixels = (points * (effective / screen_points)).round();
        if !pixels.is_finite() || pixels < 1. || pixels > u32::MAX as f64 {
            return Err(WindowPolicyError::OutputTooLarge);
        }
        Ok(pixels as u32)
    };
    let size = CapturePixelSize {
        width: axis(
            r.size.width,
            display.appkit_size_points.width,
            display.display.pixels.width,
        )?,
        height: axis(
            r.size.height,
            display.appkit_size_points.height,
            display.display.pixels.height,
        )?,
    };
    size.validate()
        .map_err(|_| WindowPolicyError::OutputTooLarge)?;
    Ok(size)
}

#[cfg(test)]
mod tests;
