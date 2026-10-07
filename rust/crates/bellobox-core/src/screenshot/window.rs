//! Pure Window-only selection of immutable frozen display pixels.
//!
//! Source: CaptureWindowCatalog, CaptureSelectionResolver and the active frozen
//! CaptureOverlayController. This is not an independent live Window capture: a
//! selected crop still contains any occluding windows visible in the snapshot.
//! Candidate frames/pointer coordinates are LOCAL TOP-LEFT DISPLAY POINTS. A
//! future native host must convert CG/Cocoa coordinates explicitly and preserve
//! its original native identity evidence separately from these UI f32 values.
//! This bounded slice accepts normal windows wholly on an unrotated main display.
//! No enumeration, permissions, image acquisition, file or clipboard I/O occurs.
use super::{
    Point, Rect, ScreenshotDocument, ScreenshotEditSession,
    area::{
        AreaDisplayGeometry, AreaError, AreaPhase, FreezeToken, FrozenAreaSession,
        MINIMUM_AREA_POINTS,
    },
};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_FROZEN_WINDOW_CANDIDATES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrozenWindowCandidate {
    pub window_id: u32,
    pub owner_process_id: i32,
    pub frame_local_points: Rect,
    pub layer: i64,
    pub alpha: f64,
    pub on_screen: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenWindowError {
    InvalidCatalog,
    InvalidGeometry,
    InvalidPoint,
    InvalidSnapshot,
    InvalidState,
    TopologyChanged,
    Cancelled,
    OutputTooLarge,
}
impl fmt::Display for FrozenWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidCatalog => "The frozen Window catalog is invalid or exceeds its limit.",
            Self::InvalidGeometry => "Frozen Window selection requires an unrotated main display.",
            Self::InvalidPoint => "The pointer position is invalid.",
            Self::InvalidSnapshot => "The frozen image does not match its display.",
            Self::InvalidState => "Frozen Window selection is not ready for this action.",
            Self::TopologyChanged => "The display changed. Start a new Window selection.",
            Self::Cancelled => "Frozen Window selection was cancelled.",
            Self::OutputTooLarge => "The frozen Window image exceeds the bounded output size.",
        })
    }
}
impl std::error::Error for FrozenWindowError {}
impl From<AreaError> for FrozenWindowError {
    fn from(value: AreaError) -> Self {
        match value {
            AreaError::InvalidGeometry => Self::InvalidGeometry,
            AreaError::InvalidPoint => Self::InvalidPoint,
            AreaError::InvalidSnapshot => Self::InvalidSnapshot,
            AreaError::InvalidState => Self::InvalidState,
            AreaError::TopologyChanged => Self::TopologyChanged,
            AreaError::Cancelled => Self::Cancelled,
        }
    }
}
pub type FrozenWindowResult<T> = Result<T, FrozenWindowError>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenWindowPhase {
    Freezing,
    Selecting,
    Pressed,
    Committed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenWindowCommitToken(FreezeToken);

/// Reuses the existing frozen Area image/generation/topology/cancellation owner.
/// The Window-only pointer resolver and physically cropped output are distinct.
pub struct FrozenWindowSession {
    frozen: FrozenAreaSession,
    catalog: Vec<FrozenWindowCandidate>,
    freeze: FreezeToken,
    hovered: Option<usize>,
    pressed: Option<Point>,
    committed: bool,
}
impl fmt::Debug for FrozenWindowSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrozenWindowSession")
            .field("phase", &self.phase())
            .field("candidate_count", &self.catalog.len())
            .finish_non_exhaustive()
    }
}
impl FrozenWindowSession {
    pub fn new(
        geometry: AreaDisplayGeometry,
        candidates: &[FrozenWindowCandidate],
        own_pid: i32,
    ) -> FrozenWindowResult<Self> {
        let catalog = validated_catalog(geometry, candidates, own_pid)?;
        let frozen = FrozenAreaSession::new(geometry)?;
        let freeze = frozen
            .freeze_token()
            .ok_or(FrozenWindowError::InvalidState)?;
        Ok(Self {
            frozen,
            catalog,
            freeze,
            hovered: None,
            pressed: None,
            committed: false,
        })
    }
    pub fn phase(&self) -> FrozenWindowPhase {
        if self.frozen.cancellation().load(Ordering::Acquire) {
            return FrozenWindowPhase::Cancelled;
        }
        match self.frozen.phase() {
            AreaPhase::Cancelled => FrozenWindowPhase::Cancelled,
            AreaPhase::Freezing => FrozenWindowPhase::Freezing,
            _ if self.committed => FrozenWindowPhase::Committed,
            _ if self.pressed.is_some() => FrozenWindowPhase::Pressed,
            _ => FrozenWindowPhase::Selecting,
        }
    }
    pub fn display(&self) -> AreaDisplayGeometry {
        self.frozen.display()
    }
    pub fn freeze_token(&self) -> Option<FreezeToken> {
        self.frozen.freeze_token()
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.frozen.cancellation()
    }
    pub fn frozen_document(&self) -> Option<&ScreenshotDocument> {
        self.frozen.frozen_document()
    }
    pub fn accept_frozen(
        &mut self,
        token: FreezeToken,
        document: ScreenshotDocument,
        observed: AreaDisplayGeometry,
    ) -> FrozenWindowResult<bool> {
        Ok(self.frozen.accept_frozen(token, document, observed)?)
    }
    pub fn restart(
        &mut self,
        geometry: AreaDisplayGeometry,
        candidates: &[FrozenWindowCandidate],
        own_pid: i32,
    ) -> FrozenWindowResult<FreezeToken> {
        self.cancel();
        let catalog = validated_catalog(geometry, candidates, own_pid)?;
        self.freeze = self.frozen.restart(geometry)?;
        self.catalog = catalog;
        self.committed = false;
        Ok(self.freeze)
    }
    pub fn cancel(&mut self) {
        self.frozen.cancel();
        self.hovered = None;
        self.pressed = None;
        self.committed = false;
    }
    /// An unlocked selector loses focus; a pending editor handoff is separately
    /// cancelled by its host, just as in the existing frozen Area fixture.
    pub fn focus_lost(&mut self) {
        if !self.committed {
            self.cancel();
        }
    }
    fn ready(&mut self, observed: AreaDisplayGeometry) -> FrozenWindowResult<()> {
        if let Err(error) = self.frozen.validate_topology(observed) {
            self.hovered = None;
            self.pressed = None;
            return Err(error.into());
        }
        if self.committed || !self.frozen.can_select() {
            return Err(FrozenWindowError::InvalidState);
        }
        Ok(())
    }
    pub fn hovered(&self) -> Option<FrozenWindowCandidate> {
        if self.frozen.cancellation().load(Ordering::Acquire) || self.committed {
            return None;
        }
        self.hovered.map(|index| self.catalog[index])
    }
    pub fn preview_rect(&self) -> Option<Rect> {
        self.hovered().map(|window| window.frame_local_points)
    }
    fn hit(&self, point: Point) -> Option<usize> {
        // The view's local bounds are half-open in local coordinates. Only
        // candidate frames use Cocoa's vertically inverted edge convention.
        let bounds = self.display().local_bounds();
        if point.x < 0. || point.y < 0. || point.x >= bounds.width || point.y >= bounds.height {
            return None;
        }
        // Never sort by size, owner or title. CG catalog order is front-to-back.
        self.catalog
            .iter()
            .position(|window| contains(window.frame_local_points, point))
    }
    pub fn hover(&mut self, point: Point, observed: AreaDisplayGeometry) -> FrozenWindowResult<()> {
        self.ready(observed)?;
        validate_point(point)?;
        // Source freezes hover after mouseDown until mouseUp resolves selection.
        if self.pressed.is_none() {
            self.hovered = self.hit(point);
        }
        Ok(())
    }
    pub fn begin_press(
        &mut self,
        point: Point,
        observed: AreaDisplayGeometry,
    ) -> FrozenWindowResult<()> {
        self.ready(observed)?;
        validate_point(point)?;
        if self.pressed.is_some() {
            return Err(FrozenWindowError::InvalidState);
        }
        self.hovered = self.hit(point);
        self.pressed = Some(clamp(point, self.display()));
        Ok(())
    }
    /// WindowOnly: both final clamped drag axes must be strictly below 8 points.
    /// No Area/display fallback. A rejected gesture resets hover at its endpoint.
    /// Returns a cheap immutable crop job; pixel allocation/copy belongs on a worker.
    pub fn end_press(
        &mut self,
        point: Point,
        observed: AreaDisplayGeometry,
    ) -> FrozenWindowResult<Option<FrozenWindowCommit>> {
        self.ready(observed)?;
        validate_point(point)?;
        let Some(start) = self.pressed.take() else {
            return Ok(None);
        };
        let end = clamp(point, self.display());
        let is_click = (end.x - start.x).abs() < MINIMUM_AREA_POINTS
            && (end.y - start.y).abs() < MINIMUM_AREA_POINTS;
        let chosen = is_click.then(|| self.hovered()).flatten();
        let Some(candidate) = chosen else {
            self.hovered = self.hit(point);
            return Ok(None);
        };
        let geometry = self.display();
        let crop = geometry
            .pixel_crop(candidate.frame_local_points)?
            .unwrap_or(Rect::new(
                0.,
                0.,
                geometry.pixel_size.0 as f32,
                geometry.pixel_size.1 as f32,
            ));
        let frozen = self
            .frozen
            .frozen_document()
            .ok_or(FrozenWindowError::InvalidState)?
            .clone();
        self.committed = true;
        self.hovered = None;
        Ok(Some(FrozenWindowCommit {
            token: FrozenWindowCommitToken(self.freeze),
            candidate,
            geometry,
            pixel_crop: crop,
            frozen,
            cancellation: self.frozen.cancellation(),
        }))
    }
    /// Call again at the actual editor-publication boundary. Another session's
    /// token or an older restart cannot revive this selection.
    pub fn accepts_commit(
        &mut self,
        token: FrozenWindowCommitToken,
        observed: AreaDisplayGeometry,
    ) -> FrozenWindowResult<bool> {
        if token != FrozenWindowCommitToken(self.freeze) || !self.committed {
            return Ok(false);
        }
        self.frozen.validate_topology(observed)?;
        Ok(true)
    }
}
fn validate_point(point: Point) -> FrozenWindowResult<()> {
    if point.valid() {
        Ok(())
    } else {
        Err(FrozenWindowError::InvalidPoint)
    }
}
fn clamp(point: Point, geometry: AreaDisplayGeometry) -> Point {
    Point::new(
        point.x.clamp(0., geometry.cocoa_frame.width),
        point.y.clamp(0., geometry.cocoa_frame.height),
    )
}
fn contains(rect: Rect, point: Point) -> bool {
    // Source hit-tests the converted point in Cocoa CGRect. Its minimum Y is
    // the local bottom edge, so inversion makes local top exclusive/bottom inclusive.
    point.x >= rect.x && point.y > rect.y && point.x < rect.right() && point.y <= rect.bottom()
}
fn validated_catalog(
    geometry: AreaDisplayGeometry,
    candidates: &[FrozenWindowCandidate],
    own_pid: i32,
) -> FrozenWindowResult<Vec<FrozenWindowCandidate>> {
    geometry.validate()?;
    if geometry.rotation_degrees != 0
        || geometry.cocoa_frame.x != 0.
        || geometry.cocoa_frame.y != 0.
    {
        return Err(FrozenWindowError::InvalidGeometry);
    }
    if own_pid <= 0 || candidates.len() > MAX_FROZEN_WINDOW_CANDIDATES {
        return Err(FrozenWindowError::InvalidCatalog);
    }
    let mut catalog = Vec::new();
    catalog
        .try_reserve_exact(candidates.len())
        .map_err(|_| FrozenWindowError::InvalidCatalog)?;
    for (index, window) in candidates.iter().enumerate() {
        let frame = window.frame_local_points;
        if window.window_id == 0
            || window.owner_process_id <= 0
            || !frame.valid()
            || frame.width <= 0.
            || frame.height <= 0.
            || !window.alpha.is_finite()
            || !(0.0..=1.0).contains(&window.alpha)
            || candidates[..index]
                .iter()
                .any(|other| other.window_id == window.window_id)
        {
            return Err(FrozenWindowError::InvalidCatalog);
        }
        let width = frame.width.floor();
        let height = frame.height.floor();
        if window.owner_process_id != own_pid
            && window.layer == 0
            && window.on_screen
            && window.alpha > 0.01
            && width >= 8.
            && height >= 8.
            && width * height >= 96.
            && frame.x >= 0.
            && frame.y >= 0.
            && frame.right() <= geometry.cocoa_frame.width
            && frame.bottom() <= geometry.cocoa_frame.height
        {
            catalog.push(*window);
        }
    }
    Ok(catalog)
}

pub struct FrozenWindowCommit {
    token: FrozenWindowCommitToken,
    candidate: FrozenWindowCandidate,
    geometry: AreaDisplayGeometry,
    pixel_crop: Rect,
    frozen: ScreenshotDocument,
    cancellation: Arc<AtomicBool>,
}
impl fmt::Debug for FrozenWindowCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrozenWindowCommit")
            .field("candidate", &self.candidate)
            .field("pixel_crop", &self.pixel_crop)
            .finish_non_exhaustive()
    }
}
pub struct FrozenWindowSelection {
    pub token: FrozenWindowCommitToken,
    pub candidate: FrozenWindowCandidate,
    pub selection_local_points: Rect,
    pub selection_cocoa_points: Rect,
    pub editor: ScreenshotEditSession,
}
impl FrozenWindowCommit {
    pub fn token(&self) -> FrozenWindowCommitToken {
        self.token
    }
    pub fn candidate(&self) -> FrozenWindowCandidate {
        self.candidate
    }
    pub fn geometry(&self) -> AreaDisplayGeometry {
        self.geometry
    }
    pub fn pixel_crop(&self) -> Rect {
        self.pixel_crop
    }
    /// Copies only the chosen pixels; the resulting document cannot reveal the
    /// rest of the display by clearing a crop or undoing an initial selection.
    /// The external flag belongs to the host handoff; both flags are preserved.
    pub fn materialize(
        self,
        external_cancellation: &AtomicBool,
    ) -> FrozenWindowResult<FrozenWindowSelection> {
        let active = || {
            if self.cancellation.load(Ordering::Acquire)
                || external_cancellation.load(Ordering::Acquire)
            {
                Err(FrozenWindowError::Cancelled)
            } else {
                Ok(())
            }
        };
        active()?;
        let (x, y, width, height) = (
            self.pixel_crop.x as u32,
            self.pixel_crop.y as u32,
            self.pixel_crop.width as u32,
            self.pixel_crop.height as u32,
        );
        super::validate_image_size(width, height).map_err(|_| FrozenWindowError::OutputTooLarge)?;
        let source = &self.frozen.base_image;
        if x.checked_add(width).is_none_or(|end| end > source.width())
            || y.checked_add(height)
                .is_none_or(|end| end > source.height())
        {
            return Err(FrozenWindowError::InvalidGeometry);
        }
        let bytes = usize::try_from(u64::from(width) * u64::from(height) * 4)
            .map_err(|_| FrozenWindowError::OutputTooLarge)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(bytes)
            .map_err(|_| FrozenWindowError::OutputTooLarge)?;
        for row in y..y + height {
            active()?;
            let start = (row as usize * source.width() as usize + x as usize) * 4;
            rgba.extend_from_slice(&source.as_raw()[start..start + width as usize * 4]);
        }
        active()?;
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or(FrozenWindowError::InvalidSnapshot)?;
        let document =
            ScreenshotDocument::from_rgba(image).map_err(|_| FrozenWindowError::InvalidSnapshot)?;
        let selection_cocoa_points = self
            .geometry
            .local_rect_to_cocoa(self.candidate.frame_local_points)?;
        Ok(FrozenWindowSelection {
            token: self.token,
            candidate: self.candidate,
            selection_local_points: self.candidate.frame_local_points,
            selection_cocoa_points,
            editor: ScreenshotEditSession::new(document),
        })
    }
}

/// Two overlapping app-owned surfaces, drawn before any fixture window opens.
/// This helper is absent from production release builds and never samples an OS.
#[cfg(any(debug_assertions, test))]
pub fn synthetic_window_fixture() -> FrozenWindowResult<(
    AreaDisplayGeometry,
    Vec<FrozenWindowCandidate>,
    ScreenshotDocument,
)> {
    let geometry = AreaDisplayGeometry {
        display_id: 1,
        cocoa_frame: Rect::new(0., 0., 800., 500.),
        pixel_size: (1600, 1000),
        rotation_degrees: 0,
    };
    let window = |window_id, frame_local_points| FrozenWindowCandidate {
        window_id,
        owner_process_id: 100 + window_id as i32,
        frame_local_points,
        layer: 0,
        alpha: 1.,
        on_screen: true,
    };
    let catalog = vec![
        window(1, Rect::new(300., 80., 340., 270.)),
        window(2, Rect::new(100., 170., 420., 230.)),
    ];
    let image = image::RgbaImage::from_fn(1600, 1000, |x, y| {
        let point = Point::new(x as f32 / 2., y as f32 / 2.);
        let Some(window) = catalog.iter().find(|window| {
            let r = window.frame_local_points;
            point.x >= r.x && point.x < r.right() && point.y >= r.y && point.y < r.bottom()
        }) else {
            return image::Rgba([232, 236, 241, 255]);
        };
        let header = point.y < window.frame_local_points.y + 32.;
        let color = match (window.window_id, header) {
            (1, true) => [166, 75, 30, 255],
            (1, false) => [255, 224, 186, 255],
            (_, true) => [40, 79, 128, 255],
            _ => [185, 215, 241, 255],
        };
        image::Rgba(color)
    });
    Ok((
        geometry,
        catalog,
        ScreenshotDocument::from_rgba(image).map_err(|_| FrozenWindowError::InvalidSnapshot)?,
    ))
}

#[cfg(test)]
mod tests;
