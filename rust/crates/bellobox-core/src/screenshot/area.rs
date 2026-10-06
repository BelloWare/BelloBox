//! Pure frozen-display Area selection, following CaptureOverlayController and
//! CaptureSelectionResolver. This module never captures, reads a clipboard/file,
//! creates a window, or asks for permission. A host must freeze BEFORE showing its
//! overlay, then supply the generation-bound immutable full-display document.
//!
//! Pointer coordinates are display-local TOP-LEFT POINTS. Display metadata uses
//! global COCOA BOTTOM-LEFT POINTS and must not be filled with CoreGraphics bounds
//! without explicit conversion. Native pixel ratios are independent on x and y.
//! Production UI remains outside this module and is scoped to the main display.
#[cfg(test)]
mod tests;

use super::{
    Point, Rect, ScreenshotDocument, ScreenshotEditSession, selection, validate_image_size,
};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

/// Source shows a rubber band at six points, but only commits areas with both
/// dimensions at least eight points. Thin/short drags reset for another attempt.
pub const MINIMUM_AREA_POINTS: f32 = 8.;
pub const PREVIEW_DRAG_POINTS: f32 = 6.;
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaError {
    InvalidGeometry,
    InvalidPoint,
    InvalidSnapshot,
    InvalidState,
    TopologyChanged,
    Cancelled,
}
impl fmt::Display for AreaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidGeometry => "The display geometry is invalid.",
            Self::InvalidPoint => "The selection point is outside supported coordinates.",
            Self::InvalidSnapshot => {
                "The frozen display image does not match the selected display."
            }
            Self::InvalidState => "The Area selection is not ready for this action.",
            Self::TopologyChanged => "The display configuration changed. Start a new Area capture.",
            Self::Cancelled => "Area capture was cancelled.",
        })
    }
}
impl std::error::Error for AreaError {}
pub type AreaResult<T> = Result<T, AreaError>;

/// Exact metadata captured with the frozen image, not a mutable live display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaDisplayGeometry {
    pub display_id: u32,
    pub cocoa_frame: Rect,
    pub pixel_size: (u32, u32),
    /// A topology fingerprint component; pixels must already be display-oriented.
    /// This module does not rotate or recapture an image.
    pub rotation_degrees: u16,
}
impl AreaDisplayGeometry {
    pub fn validate(self) -> AreaResult<()> {
        if self.display_id == 0
            || !self.cocoa_frame.valid()
            || self.cocoa_frame.width < 1.
            || self.cocoa_frame.height < 1.
            || self.rotation_degrees >= 360
        {
            return Err(AreaError::InvalidGeometry);
        }
        validate_image_size(self.pixel_size.0, self.pixel_size.1)
            .map_err(|_| AreaError::InvalidGeometry)
    }
    pub fn local_bounds(self) -> Rect {
        Rect::new(0., 0., self.cocoa_frame.width, self.cocoa_frame.height)
    }
    pub fn pixels_per_point(self) -> AreaResult<(f32, f32)> {
        self.validate()?;
        Ok((
            self.pixel_size.0 as f32 / self.cocoa_frame.width,
            self.pixel_size.1 as f32 / self.cocoa_frame.height,
        ))
    }
    pub fn local_rect_to_cocoa(self, rect: Rect) -> AreaResult<Rect> {
        self.validate()?;
        if !rect.valid() || rect.width < 0. || rect.height < 0. {
            return Err(AreaError::InvalidGeometry);
        }
        let result = Rect::new(
            self.cocoa_frame.x + rect.x,
            self.cocoa_frame.y + self.cocoa_frame.height - rect.bottom(),
            rect.width,
            rect.height,
        );
        if result.valid() {
            Ok(result)
        } else {
            Err(AreaError::InvalidGeometry)
        }
    }
    pub fn cocoa_rect_to_local(self, rect: Rect) -> AreaResult<Rect> {
        self.validate()?;
        if !rect.valid() || rect.width < 0. || rect.height < 0. {
            return Err(AreaError::InvalidGeometry);
        }
        let result = Rect::new(
            rect.x - self.cocoa_frame.x,
            self.cocoa_frame.bottom() - rect.bottom(),
            rect.width,
            rect.height,
        );
        if result.valid() {
            Ok(result)
        } else {
            Err(AreaError::InvalidGeometry)
        }
    }
    /// Matches the source conversion using actual image dimensions. Pixel mapping
    /// is origin-free so a large/negative desktop origin cannot lose local precision.
    pub fn pixel_crop(self, local_rect: Rect) -> AreaResult<Option<Rect>> {
        self.validate()?;
        if !local_rect.valid() || local_rect.width <= 0. || local_rect.height <= 0. {
            return Err(AreaError::InvalidGeometry);
        }
        let local = local_rect
            .intersection(self.local_bounds())
            .ok_or(AreaError::InvalidGeometry)?;
        let local_cocoa = Rect::new(
            local.x,
            self.cocoa_frame.height - local.bottom(),
            local.width,
            local.height,
        );
        selection::display_selection_crop(local_cocoa, self.local_bounds(), self.pixel_size)
            .map_err(|_| AreaError::InvalidGeometry)
    }
}

/// Explicit coordinate-space bridge. `primary_cocoa_top` is the primary display's
/// top edge in Cocoa points, not the target display's height or a backing scale.
pub fn core_graphics_frame_to_cocoa(frame: Rect, primary_cocoa_top: f32) -> AreaResult<Rect> {
    if !frame.valid()
        || frame.width < 1.
        || frame.height < 1.
        || !super::valid_coordinate(primary_cocoa_top)
    {
        return Err(AreaError::InvalidGeometry);
    }
    let result = Rect::new(
        frame.x,
        primary_cocoa_top - frame.bottom(),
        frame.width,
        frame.height,
    );
    if result.valid() {
        Ok(result)
    } else {
        Err(AreaError::InvalidGeometry)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreezeToken {
    owner: u64,
    generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaPhase {
    Freezing,
    Selecting,
    Dragging,
    Committed,
    Cancelled,
}

pub struct AreaSelection {
    pub editor: ScreenshotEditSession,
    pub display: AreaDisplayGeometry,
    pub selection_local_points: Rect,
    pub selection_cocoa_points: Rect,
    pub pixel_crop: Option<Rect>,
}
impl fmt::Debug for AreaSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AreaSelection")
            .field("display", &self.display)
            .field("selection_local_points", &self.selection_local_points)
            .field("pixel_crop", &self.pixel_crop)
            .finish_non_exhaustive()
    }
}

/// Window/session-owned model. The host checks current topology at each pointer
/// transition and again before publication. A different display is never adopted
/// silently, even when it has the same dimensions or occupies the same position.
pub struct FrozenAreaSession {
    owner: u64,
    generation: u64,
    geometry: AreaDisplayGeometry,
    phase: AreaPhase,
    frozen: Option<ScreenshotDocument>,
    start: Option<Point>,
    current: Option<Point>,
    cancellation: Arc<AtomicBool>,
}
impl fmt::Debug for FrozenAreaSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrozenAreaSession")
            .field("phase", &self.phase)
            .field("display", &self.geometry)
            .field("has_frozen_pixels", &self.frozen.is_some())
            .finish_non_exhaustive()
    }
}
impl FrozenAreaSession {
    pub fn new(geometry: AreaDisplayGeometry) -> AreaResult<Self> {
        geometry.validate()?;
        Ok(Self {
            owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            geometry,
            phase: AreaPhase::Freezing,
            frozen: None,
            start: None,
            current: None,
            cancellation: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn phase(&self) -> AreaPhase {
        self.phase
    }
    pub fn display(&self) -> AreaDisplayGeometry {
        self.geometry
    }
    pub fn freeze_token(&self) -> Option<FreezeToken> {
        (self.phase == AreaPhase::Freezing && !self.cancellation.load(Ordering::Acquire)).then_some(
            FreezeToken {
                owner: self.owner,
                generation: self.generation,
            },
        )
    }
    /// A fresh per-generation flag for an external freeze worker. Never reuse it
    /// for a restarted capture; restart sets the old flag and creates a new one.
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancellation.clone()
    }
    pub fn frozen_document(&self) -> Option<&ScreenshotDocument> {
        self.frozen.as_ref()
    }
    pub fn can_select(&self) -> bool {
        matches!(self.phase, AreaPhase::Selecting | AreaPhase::Dragging)
            && !self.cancellation.load(Ordering::Acquire)
    }
    /// The only operation that makes a selector ready. False ignores a cancelled,
    /// duplicate or older worker result without changing the current session.
    pub fn accept_frozen(
        &mut self,
        token: FreezeToken,
        document: ScreenshotDocument,
        observed: AreaDisplayGeometry,
    ) -> AreaResult<bool> {
        if self.freeze_token() != Some(token) {
            return Ok(false);
        }
        self.validate_topology(observed)?;
        if document.dimensions() != self.geometry.pixel_size
            || !document.annotations.is_empty()
            || document.crop_rect.is_some()
        {
            self.cancel();
            return Err(AreaError::InvalidSnapshot);
        }
        self.frozen = Some(document);
        self.phase = AreaPhase::Selecting;
        Ok(true)
    }
    pub fn restart(&mut self, geometry: AreaDisplayGeometry) -> AreaResult<FreezeToken> {
        self.cancel();
        geometry.validate()?;
        self.geometry = geometry;
        self.cancellation = Arc::new(AtomicBool::new(false));
        self.phase = AreaPhase::Freezing;
        Ok(self.freeze_token().expect("new active freeze generation"))
    }
    pub fn cancel(&mut self) {
        self.cancellation.store(true, Ordering::Release);
        self.generation = self.generation.wrapping_add(1);
        self.phase = AreaPhase::Cancelled;
        self.start = None;
        self.current = None;
        self.frozen = None;
    }
    /// Source installs this observer after freezing and only dismisses an unlocked
    /// selection. Initial app hiding must not cancel the in-flight freeze itself.
    pub fn focus_lost(&mut self) -> bool {
        if matches!(self.phase, AreaPhase::Selecting | AreaPhase::Dragging) {
            self.cancel();
            true
        } else {
            false
        }
    }
    /// Invalidates even a still-freezing request. Comparison is exact: no display
    /// ID, bounds, pixel-size or rotation fallback can reuse stale frozen pixels.
    pub fn validate_topology(&mut self, observed: AreaDisplayGeometry) -> AreaResult<()> {
        if self.phase == AreaPhase::Cancelled || self.cancellation.load(Ordering::Acquire) {
            self.cancel();
            return Err(AreaError::Cancelled);
        }
        if observed.validate().is_err() || observed != self.geometry {
            self.cancel();
            return Err(AreaError::TopologyChanged);
        }
        Ok(())
    }
    fn selection_ready(&mut self, observed: AreaDisplayGeometry) -> AreaResult<()> {
        self.validate_topology(observed)?;
        if !self.can_select() || self.frozen.is_none() {
            return Err(AreaError::InvalidState);
        }
        Ok(())
    }
    fn clamp_point(&self, point: Point) -> AreaResult<Point> {
        if !point.valid() {
            return Err(AreaError::InvalidPoint);
        }
        Ok(Point::new(
            point.x.clamp(0., self.geometry.cocoa_frame.width),
            point.y.clamp(0., self.geometry.cocoa_frame.height),
        ))
    }
    pub fn begin_drag(&mut self, point: Point, observed: AreaDisplayGeometry) -> AreaResult<()> {
        self.selection_ready(observed)?;
        if self.phase != AreaPhase::Selecting {
            return Err(AreaError::InvalidState);
        }
        let point = self.clamp_point(point)?;
        self.start = Some(point);
        self.current = Some(point);
        self.phase = AreaPhase::Dragging;
        Ok(())
    }
    pub fn update_drag(&mut self, point: Point, observed: AreaDisplayGeometry) -> AreaResult<()> {
        self.selection_ready(observed)?;
        if self.phase != AreaPhase::Dragging {
            return Err(AreaError::InvalidState);
        }
        self.current = Some(self.clamp_point(point)?);
        Ok(())
    }
    pub fn drag_rect(&self) -> Option<Rect> {
        if !self.can_select() {
            return None;
        }
        Some(Rect::from_points(self.start?, self.current?))
    }
    pub fn preview_rect(&self) -> Option<Rect> {
        self.drag_rect()
            .filter(|r| r.width >= PREVIEW_DRAG_POINTS || r.height >= PREVIEW_DRAG_POINTS)
    }
    pub fn dim_bands(&self) -> AreaResult<[Rect; 4]> {
        selection::dim_bands(self.geometry.local_bounds(), self.preview_rect())
            .map_err(|_| AreaError::InvalidGeometry)
    }
    /// The mouse-up point is included. An invalid-size drag resets the selector;
    /// a valid one publishes exactly one clean-baseline session and locks selection.
    /// No I/O, capture, clipboard write, or later replacement image is possible here.
    pub fn end_drag(
        &mut self,
        point: Point,
        observed: AreaDisplayGeometry,
    ) -> AreaResult<Option<AreaSelection>> {
        self.selection_ready(observed)?;
        if self.phase == AreaPhase::Selecting {
            return Ok(None);
        }
        self.update_drag(point, observed)?;
        let local = self.drag_rect().ok_or(AreaError::InvalidState)?;
        if local.width < MINIMUM_AREA_POINTS || local.height < MINIMUM_AREA_POINTS {
            self.start = None;
            self.current = None;
            self.phase = AreaPhase::Selecting;
            return Ok(None);
        }
        let crop = self.geometry.pixel_crop(local)?;
        let cocoa = self.geometry.local_rect_to_cocoa(local)?;
        let mut document = self.frozen.as_ref().ok_or(AreaError::InvalidState)?.clone();
        // Capture's initial selection is a document property, not an undoable edit.
        document.crop_rect = crop;
        let editor = ScreenshotEditSession::new(document);
        self.phase = AreaPhase::Committed;
        self.start = None;
        self.current = None;
        Ok(Some(AreaSelection {
            editor,
            display: self.geometry,
            selection_local_points: local,
            selection_cocoa_points: cocoa,
            pixel_crop: crop,
        }))
    }
}
impl Drop for FrozenAreaSession {
    fn drop(&mut self) {
        self.cancellation.store(true, Ordering::Release);
    }
}
