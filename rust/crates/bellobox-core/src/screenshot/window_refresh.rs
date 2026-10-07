//! Pure Window refresh decisions and preparation of an already supplied image.
//! Source: CaptureWindowCatalog.isOccluded, ImageAlphaMask and
//! ScreenshotPopupViewModel.refreshBaseCapture. No capture, catalog query, UI,
//! clipboard or OCR operation occurs. Native acquisition/host publication remain
//! separate. The existing screenshot model/renderer owns all image/edit state.
use super::{
    BaseCaptureToken, Rect, ScreenshotDocument, ScreenshotEditSession, area::AreaDisplayGeometry,
    window::FrozenWindowCommitToken,
};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_OCCLUSION_ROWS: usize = 512;

/// Raw front-to-back evidence from an on-screen catalog excluding desktop
/// elements, not the selector's eligible-window catalog.
/// Missing fields model rows that the Swift CG dictionary parser skips. All
/// frames must be in the same POINT coordinate space as the target.
#[derive(Clone, Copy, Debug)]
pub struct OcclusionRow {
    pub window_id: Option<u32>,
    pub owner_process_id: Option<i32>,
    pub layer: Option<i64>,
    pub alpha: Option<f64>,
    pub frame: Option<Rect>,
}
/// A future native host supplies actual CGWindowLevelForKey values. The pure
/// model never hardcodes platform window levels or queries them itself.
#[derive(Clone, Copy, Debug)]
pub struct OcclusionLayers {
    pub normal: i64,
    pub floating: i64,
    pub modal_panel: i64,
    pub main_menu: i64,
    pub status: i64,
    pub popup_menu: i64,
    pub screen_saver: i64,
}
impl OcclusionLayers {
    fn selectable(self, layer: i64) -> bool {
        [
            self.normal,
            self.floating,
            self.modal_panel,
            self.main_menu,
            self.status,
            self.popup_menu,
        ]
        .contains(&layer)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowRefreshDecision {
    KeepFrozen,
    ReplaceWithIndependent,
    MaskFrozenAlpha,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowRefreshError {
    Cancelled,
    InvalidContext,
    InvalidCatalog,
    StaleContext,
    IncompatibleImages,
    InvalidMaskOutput,
    MaskFailed,
    OutputTooLarge,
}
impl fmt::Display for WindowRefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Window image refresh was cancelled.",
            Self::InvalidContext => "Window image refresh context is invalid.",
            Self::InvalidCatalog => "Window occlusion evidence exceeds its bounded limit.",
            Self::StaleContext => "Window selection or display changed before image refresh.",
            Self::IncompatibleImages => "Window image sizes cannot be alpha-masked together.",
            Self::InvalidMaskOutput => "Window alpha mask returned an invalid replacement image.",
            Self::MaskFailed => "Window alpha mask could not be prepared.",
            Self::OutputTooLarge => "Window image refresh exceeds the bounded image size.",
        })
    }
}
impl std::error::Error for WindowRefreshError {}
pub type WindowRefreshResult<T> = Result<T, WindowRefreshError>;

/// Exact source iteration: the first matching target ID ends the scan before
/// checking that row's remaining fields. Own regular windows may occlude; only
/// own overlays at/above screenSaver are excluded. Rows behind target do not count.
pub fn is_occluded(
    target_id: u32,
    frame: Rect,
    rows: Option<&[OcclusionRow]>,
    own_pid: i32,
    layers: OcclusionLayers,
) -> WindowRefreshResult<bool> {
    if target_id == 0 || own_pid <= 0 {
        return Err(WindowRefreshError::InvalidContext);
    }
    let Some(rows) = rows else {
        return Ok(false);
    }; // Swift CG query failure
    if rows.len() > MAX_OCCLUSION_ROWS {
        return Err(WindowRefreshError::InvalidCatalog);
    }
    let target = frame.standardized();
    if !target.valid() || target.width <= 0. || target.height <= 0. {
        return Ok(false);
    }
    for row in rows {
        let Some(id) = row.window_id else {
            continue;
        };
        if id == target_id {
            return Ok(false);
        }
        let (Some(pid), Some(layer), Some(alpha), Some(bounds)) =
            (row.owner_process_id, row.layer, row.alpha, row.frame)
        else {
            continue;
        };
        if (pid == own_pid && layer >= layers.screen_saver)
            || !layers.selectable(layer)
            || !alpha.is_finite()
            || alpha <= 0.01
            || !bounds.valid()
        {
            continue;
        }
        let width = bounds.width.floor();
        let height = bounds.height.floor();
        if width < 8. || height < 8. || width * height < 96. {
            continue;
        }
        if let Some(overlap) = bounds.intersection(target)
            && overlap.width >= 2.
            && overlap.height >= 2.
        {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Visible-frame surfaces and active scrolling keep their frozen pixels. A live
/// independent image replaces an occluded/non-frozen capture; an unobscured frozen
/// crop borrows only live alpha. Missing frame and unavailable catalog follow the
/// source's distinct defaults (occluded and not-occluded respectively).
#[derive(Clone, Copy, Debug)]
pub struct WindowRefreshSource {
    pub independent_window: bool,
    pub scrolling_active: bool,
    pub cut_from_frozen: bool,
    pub window_id: u32,
    pub frame: Option<Rect>,
}
pub fn decide_window_refresh(
    source: WindowRefreshSource,
    rows: Option<&[OcclusionRow]>,
    own_pid: i32,
    layers: OcclusionLayers,
) -> WindowRefreshResult<WindowRefreshDecision> {
    if !source.independent_window || source.scrolling_active {
        return Ok(WindowRefreshDecision::KeepFrozen);
    }
    if !source.cut_from_frozen {
        return Ok(WindowRefreshDecision::ReplaceWithIndependent);
    }
    let occluded = match source.frame {
        Some(frame) => is_occluded(source.window_id, frame, rows, own_pid, layers)?,
        None => true,
    };
    Ok(if occluded {
        WindowRefreshDecision::ReplaceWithIndependent
    } else {
        WindowRefreshDecision::MaskFrozenAlpha
    })
}

/// Current host-owned selection token and main-display geometry. The native
/// adapter must independently validate its full topology and actual source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRefreshContext {
    pub selection: FrozenWindowCommitToken,
    pub window_id: u32,
    pub display: AreaDisplayGeometry,
}
impl WindowRefreshContext {
    fn validate(self) -> WindowRefreshResult<()> {
        if self.window_id == 0 || self.display.validate().is_err() {
            Err(WindowRefreshError::InvalidContext)
        } else {
            Ok(())
        }
    }
}
/// Borrowed, tightly packed straight RGBA BASE pixels supplied to a mask backend.
/// No rendered annotations, crop or font state is exposed, so edit/Undo races
/// cannot bake transient edits into a replacement. The borrow ends with the
/// synchronous callback; a native backend must retain/copy its own image inputs.
#[derive(Clone, Copy)]
pub struct WindowAlphaPixels<'a> {
    dimensions: (u32, u32),
    rgba: &'a [u8],
}
impl<'a> WindowAlphaPixels<'a> {
    fn from_document(document: &'a ScreenshotDocument) -> Self {
        let dimensions = document.dimensions();
        // RgbaImage permits a backing Vec longer than its raster. Expose only
        // actual pixels, never trailing storage unrelated to the selected image.
        let length = dimensions.0 as usize * dimensions.1 as usize * 4;
        Self {
            dimensions,
            rgba: &document.base_image.as_raw()[..length],
        }
    }
    pub fn dimensions(self) -> (u32, u32) {
        self.dimensions
    }
    pub fn rgba(self) -> &'a [u8] {
        self.rgba
    }
}

/// An immutable request tied to one edit session/base epoch and frozen selection.
/// Font preparation or an edit followed by Undo on the same base is permitted;
/// the CURRENT document must still be annotation-free and uncropped at acceptance.
pub struct WindowRefreshPlan {
    context: WindowRefreshContext,
    base: BaseCaptureToken,
    frozen: ScreenshotDocument,
    decision: WindowRefreshDecision,
    cancellation: Arc<AtomicBool>,
}
impl WindowRefreshPlan {
    pub fn new(
        session: &ScreenshotEditSession,
        context: WindowRefreshContext,
        decision: WindowRefreshDecision,
        cancellation: Arc<AtomicBool>,
    ) -> WindowRefreshResult<Self> {
        if cancellation.load(Ordering::Acquire) {
            return Err(WindowRefreshError::Cancelled);
        }
        context.validate()?;
        Ok(Self {
            context,
            base: session.base_capture_token(),
            frozen: session.render_snapshot(),
            decision,
            cancellation,
        })
    }
    pub fn decision(&self) -> WindowRefreshDecision {
        self.decision
    }
    /// Portable deterministic preparation. Its ±1-pixel nearest sampler is an
    /// approximation, not a CoreGraphics tie/color-equivalence claim. Native
    /// hosts use prepare_with_alpha_mask with the source-exact platform backend.
    pub fn prepare(
        self,
        independent: ScreenshotDocument,
    ) -> WindowRefreshResult<Option<PreparedWindowRefresh>> {
        self.prepare_with_alpha_mask(independent, mask_frozen_alpha)
    }
    /// Common worker preparation and publication guards for a supplied mask
    /// backend. The callback runs only for MaskFrozenAlpha, on immutable BASE
    /// pixels, after size/cancellation validation. It never acquires images.
    /// A result must have the exact frozen size and no annotations or crop.
    pub fn prepare_with_alpha_mask(
        self,
        independent: ScreenshotDocument,
        mask: impl FnOnce(
            WindowAlphaPixels<'_>,
            WindowAlphaPixels<'_>,
            &AtomicBool,
        ) -> WindowRefreshResult<ScreenshotDocument>,
    ) -> WindowRefreshResult<Option<PreparedWindowRefresh>> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(WindowRefreshError::Cancelled);
        }
        let replacement = match self.decision {
            WindowRefreshDecision::KeepFrozen => return Ok(None),
            WindowRefreshDecision::ReplaceWithIndependent => independent,
            WindowRefreshDecision::MaskFrozenAlpha => {
                let frozen = WindowAlphaPixels::from_document(&self.frozen);
                let shape = WindowAlphaPixels::from_document(&independent);
                if frozen.dimensions.0.abs_diff(shape.dimensions.0) > 1
                    || frozen.dimensions.1.abs_diff(shape.dimensions.1) > 1
                {
                    return Err(WindowRefreshError::IncompatibleImages);
                }
                let output = mask(frozen, shape, &self.cancellation)?;
                if output.dimensions() != frozen.dimensions
                    || !output.annotations().is_empty()
                    || output.crop_rect().is_some()
                {
                    return Err(WindowRefreshError::InvalidMaskOutput);
                }
                output
            }
        };
        if self.cancellation.load(Ordering::Acquire) {
            return Err(WindowRefreshError::Cancelled);
        }
        Ok(Some(PreparedWindowRefresh {
            context: self.context,
            base: self.base,
            replacement,
            cancellation: self.cancellation,
        }))
    }
}

/// Cloning retains immutable pixel ownership for counted off-thread disposal.
/// It does not permit retry: the original base epoch can publish only once.
#[derive(Clone)]
pub struct PreparedWindowRefresh {
    context: WindowRefreshContext,
    base: BaseCaptureToken,
    replacement: ScreenshotDocument,
    cancellation: Arc<AtomicBool>,
}
impl PreparedWindowRefresh {
    /// Does not publish to UI, cancel OCR, rebuild preview tiles or copy/export.
    /// A true result advances session.revision(); the host must use its existing
    /// changed() invalidation path and final liveness/selection/cancellation gate.
    pub fn apply(
        self,
        session: &mut ScreenshotEditSession,
        current: WindowRefreshContext,
        boundary_cancellation: &AtomicBool,
    ) -> WindowRefreshResult<bool> {
        if self.cancellation.load(Ordering::Acquire)
            || boundary_cancellation.load(Ordering::Acquire)
        {
            return Err(WindowRefreshError::Cancelled);
        }
        current.validate()?;
        if current != self.context {
            return Err(WindowRefreshError::StaleContext);
        }
        Ok(session.replace_base_capture(self.base, self.replacement))
    }
}

/// Bounded straight-RGBA equivalent of the source's destination-in mask. The
/// frozen RGB remains unchanged for nonzero output alpha; fully transparent
/// output is normalized to transparent black instead of retaining hidden RGB.
/// Source allows +/-1 pixel per axis and stretches with interpolation disabled.
/// Sampling below is explicit integer nearest-centre. Native synthetic tests
/// demonstrated scale-dependent CoreGraphics center ties; this portable fallback
/// deliberately stays deterministic rather than guessing an undocumented rule.
/// Native hosts use the source-exact supplied-image platform mask backend.
fn mask_frozen_alpha(
    frozen: WindowAlphaPixels<'_>,
    shape: WindowAlphaPixels<'_>,
    cancellation: &AtomicBool,
) -> WindowRefreshResult<ScreenshotDocument> {
    let (width, height) = frozen.dimensions();
    let (sw, sh) = shape.dimensions();
    if width.abs_diff(sw) > 1 || height.abs_diff(sh) > 1 {
        return Err(WindowRefreshError::IncompatibleImages);
    }
    super::validate_image_size(width, height).map_err(|_| WindowRefreshError::OutputTooLarge)?;
    let count = usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| WindowRefreshError::OutputTooLarge)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| WindowRefreshError::OutputTooLarge)?;
    for y in 0..height {
        if cancellation.load(Ordering::Acquire) {
            return Err(WindowRefreshError::Cancelled);
        }
        let sy = (((2 * u64::from(y) + 1) * u64::from(sh)) / (2 * u64::from(height)))
            .min(u64::from(sh - 1)) as u32;
        for x in 0..width {
            let sx = (((2 * u64::from(x) + 1) * u64::from(sw)) / (2 * u64::from(width)))
                .min(u64::from(sw - 1)) as u32;
            let offset = (y as usize * width as usize + x as usize) * 4;
            let old = &frozen.rgba[offset..offset + 4];
            let shape_alpha = shape.rgba[(sy as usize * sw as usize + sx as usize) * 4 + 3];
            let alpha = ((u16::from(old[3]) * u16::from(shape_alpha) + 127) / 255) as u8;
            let output = if alpha == 0 {
                [0, 0, 0, 0]
            } else {
                [old[0], old[1], old[2], alpha]
            };
            pixels.extend_from_slice(&output);
        }
    }
    let image = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or(WindowRefreshError::OutputTooLarge)?;
    ScreenshotDocument::from_rgba(image).map_err(|_| WindowRefreshError::OutputTooLarge)
}

#[cfg(test)]
mod tests;
