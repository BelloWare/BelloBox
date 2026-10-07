//! Owned backend-injected Window evidence and independent acquisition.
//!
//! The selector uses normal-layer, non-own windows wholly on the main display.
//! Occlusion consumes a separate, raw front-to-back catalog, retaining our own
//! regular windows. Exact f64 evidence survives independently of the f32 UI.
//! Callback submission plans bind native filter evidence before acquisition; only
//! owned Rust metadata crosses queues. The concrete native catalog stays unavailable.
use bello_platform::{
    macos_capture_overlay::MainDisplayOverlayLayout,
    native_capture::{
        CaptureCancellation, CapturePixelSize, CaptureRect, CaptureRequest, WindowCaptureSession,
    },
    window_capture::{
        MAX_WINDOW_CANDIDATES, MAX_WINDOW_IDENTITY_BYTES, WindowCaptureOptions, WindowCapturePlan,
        WindowCaptureSelection, WindowDisplayGeometry, WindowFrame, WindowIdentity,
        WindowObservation, WindowPolicyError, WindowSelectionToken, WindowTopology,
    },
};
use bellobox_core::screenshot::{
    Rect, ScreenshotDocument,
    area::AreaDisplayGeometry,
    window::{FrozenWindowCandidate, FrozenWindowCommit},
    window_refresh::{
        MAX_OCCLUSION_ROWS, OcclusionLayers, OcclusionRow, WindowRefreshContext,
        WindowRefreshDecision, WindowRefreshSource, decide_window_refresh,
    },
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Complete owned CG-style evidence. Mutable titles are intentionally absent.
#[derive(Clone, Debug)]
pub struct Observation {
    pub window_id: u32,
    pub owner_process_id: i32,
    pub owner_bundle_id: Option<String>,
    pub frame: CaptureRect,
    pub layer: i64,
    pub alpha: f64,
    pub on_screen: bool,
}
impl Observation {
    fn borrowed(&self) -> WindowObservation<'_> {
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
    fn candidate(&self) -> FrozenWindowCandidate {
        FrozenWindowCandidate {
            window_id: self.window_id,
            owner_process_id: self.owner_process_id,
            frame_local_points: ui_rect(self.frame),
            layer: self.layer,
            alpha: self.alpha,
            on_screen: self.on_screen,
        }
    }
}

#[cfg(any(debug_assertions, test))]
/// Pointer-free evidence copied from the exact callback-local source before its
/// filter acquires pixels. Never substitute initial observations for this record.
#[derive(Clone, Debug)]
pub struct Submission {
    pub window_id: u32,
    pub owner_process_id: i32,
    pub owner_bundle_id: Option<String>,
    pub frame: CaptureRect,
    pub layer: i64,
    pub on_screen: bool,
}
#[cfg(any(debug_assertions, test))]
impl Submission {
    fn borrowed(&self) -> bello_platform::window_capture::SubmittedWindowSource<'_> {
        bello_platform::window_capture::SubmittedWindowSource {
            identity: WindowIdentity {
                window_id: self.window_id,
                owner_process_id: self.owner_process_id,
                owner_bundle_id: self.owner_bundle_id.as_deref(),
            },
            frame: WindowFrame(self.frame),
            layer: self.layer,
            on_screen: self.on_screen,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Topology {
    pub main_display_id: u32,
    pub displays: Vec<WindowDisplayGeometry>,
}
impl Topology {
    fn borrowed(&self) -> WindowTopology<'_> {
        WindowTopology {
            main_display_id: self.main_display_id,
            displays: &self.displays,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Evidence {
    pub observations: Vec<Observation>,
    pub topology: Topology,
    /// Raw parser rows, not the selector catalog. Missing fields retain the
    /// core/Swift skip behavior, and None models an unavailable catalog query.
    pub occlusion_rows: Option<Vec<OcclusionRow>>,
}

/// The catalog is independent of acquisition. Implementations return owned bounded
/// snapshots, preserving raw occlusion rows and exact f64 selector/topology values.
/// acquire MUST bind the actual callback-local source before acquiring pixels.
/// It must never construct submission evidence from initial CG selector rows.
pub trait Backend: Send + Sync {
    /// Side-effect-free capability check; never enumerates, requests permission or acquires pixels.
    fn check_available(&self) -> Result<(), String>;
    fn identity_and_layers(&self) -> Result<(i32, OcclusionLayers), String>;
    fn observe(&self) -> Result<Evidence, String>;
    fn acquire(&self, request: &AcquisitionRequest) -> Result<Acquired, String>;
}

/// The actual native catalog is deliberately unavailable until raw-row parsing,
/// native lifetime and UI-thread observation have been independently accepted.
/// The native image API also retains its own separate closed production gate.
pub struct NativeBackend;
impl Backend for NativeBackend {
    fn check_available(&self) -> Result<(), String> {
        Err("Native Window catalog is awaiting native review.".into())
    }
    fn identity_and_layers(&self) -> Result<(i32, OcclusionLayers), String> {
        Err("Native Window catalog is awaiting native review.".into())
    }
    fn observe(&self) -> Result<Evidence, String> {
        Err("Native Window catalog is awaiting native review.".into())
    }
    fn acquire(&self, request: &AcquisitionRequest) -> Result<Acquired, String> {
        let snapshot = bello_platform::native_capture::capture_window(
            request.selection.clone(),
            request.options,
            request.session.clone(),
            request.cancellation.clone(),
        )
        .map_err(|error| error.to_string())?;
        Acquired::from_native(snapshot, request)
    }
}

/// A request carries immutable selection and live cancellation/session ownership.
/// No guessed submission or image generator is part of the production request.
pub struct AcquisitionRequest {
    selection: WindowCaptureSelection,
    options: WindowCaptureOptions,
    session: Arc<WindowCaptureSession>,
    cancellation: CaptureCancellation,
    #[cfg(any(debug_assertions, test))]
    cancellation_flag: Arc<AtomicBool>,
    #[cfg(any(debug_assertions, test))]
    layout: MainDisplayOverlayLayout,
}
impl AcquisitionRequest {
    fn validate_completion_plan(&self, plan: &WindowCapturePlan) -> Result<(), String> {
        if !plan.matches_request(&self.selection, self.options) {
            return Err("Window completion does not belong to the acquisition request.".into());
        }
        Ok(())
    }
}

#[cfg(any(debug_assertions, test))]
impl AcquisitionRequest {
    /// Call inside the source-resolution callback, while the exact native window
    /// used by the independent filter is retained. No pixels may be acquired until
    /// this returns successfully. The plan owns all pointer-free source evidence.
    pub fn bind_submission(
        &self,
        submission: &Submission,
        current: &Evidence,
    ) -> Result<Arc<WindowCapturePlan>, String> {
        validate_evidence(current, self.layout)?;
        let fresh: Vec<_> = current
            .observations
            .iter()
            .map(Observation::borrowed)
            .collect();
        self.selection
            .plan(
                submission.borrowed(),
                &fresh,
                current.topology.borrowed(),
                self.session.current().map_err(|error| error.to_string())?,
                self.options,
                &self.cancellation,
            )
            .map(Arc::new)
            .map_err(|error| error.to_string())
    }
}

/// Owned result from the actual submission path, not a reconstructed identity from
/// a PNG diagnostic or the initial catalog. Opaque native owners never cross here.
pub struct Acquired {
    document: ScreenshotDocument,
    plan: Arc<WindowCapturePlan>,
}

impl Acquired {
    fn from_native(
        snapshot: bello_platform::native_capture::NativeWindowCaptureSnapshot,
        request: &AcquisitionRequest,
    ) -> Result<Self, String> {
        request.validate_completion_plan(&snapshot.completion)?;
        Ok(Self {
            document: ScreenshotDocument::from_png(&snapshot.png)?,
            plan: snapshot.completion,
        })
    }
}

pub struct Source {
    layout: MainDisplayOverlayLayout,
    own_pid: i32,
    selected: Vec<Observation>,
    topology: Topology,
    layers: OcclusionLayers,
    token: WindowSelectionToken,
    session: Arc<WindowCaptureSession>,
    backend: Arc<dyn Backend>,
    #[cfg(any(debug_assertions, test))]
    supplied: Option<Arc<fixtures::SuppliedBackend>>,
}
impl Source {
    pub fn from_backend(
        layout: MainDisplayOverlayLayout,
        backend: Arc<dyn Backend>,
    ) -> Result<Arc<Self>, String> {
        backend.check_available()?;
        let (own_pid, layers) = backend.identity_and_layers()?;
        let evidence = backend.observe()?;
        validate_evidence(&evidence, layout)?;
        if own_pid <= 0 || layers.normal != 0 {
            return Err("Invalid Window source.".into());
        }
        // Checked CAS preserves unique bounded session IDs at the Rust 1.88 MSRV.
        let mut session = NEXT_SESSION.load(Ordering::Relaxed);
        loop {
            let next = session
                .checked_add(1)
                .ok_or("Window source session limit reached.")?;
            match NEXT_SESSION.compare_exchange_weak(
                session,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => session = current,
            }
        }
        let token = WindowSelectionToken {
            session,
            generation: 1,
        };
        let cancellation = CaptureCancellation::default();
        let mut selected = Vec::new();
        for observation in &evidence.observations {
            match WindowCaptureSelection::new(
                observation.borrowed(),
                evidence.topology.borrowed(),
                own_pid,
                token,
                &cancellation,
            ) {
                Ok(_) => {
                    let candidate = observation.candidate();
                    let bounds = ui_rect(layout.cocoa_frame);
                    if candidate.frame_local_points.right() <= bounds.width
                        && candidate.frame_local_points.bottom() <= bounds.height
                    {
                        selected.push(observation.clone());
                    }
                }
                Err(
                    WindowPolicyError::IneligibleWindow | WindowPolicyError::UnsupportedGeometry,
                ) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(Arc::new(Self {
            layout,
            own_pid,
            selected,
            topology: evidence.topology,
            layers,
            token,
            session: Arc::new(WindowCaptureSession::new(token).map_err(|error| error.to_string())?),
            backend,
            #[cfg(any(debug_assertions, test))]
            supplied: None,
        }))
    }
    pub fn own_pid(&self) -> i32 {
        self.own_pid
    }
    pub fn candidates(&self) -> Result<Vec<FrozenWindowCandidate>, String> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(self.selected.len())
            .map_err(|_| "Window catalog allocation failed.")?;
        result.extend(self.selected.iter().map(Observation::candidate));
        Ok(result)
    }
    pub fn request(
        self: &Arc<Self>,
        commit: &FrozenWindowCommit,
        boundary: Arc<AtomicBool>,
    ) -> Result<(WindowRefreshContext, WindowRefreshDecision, Acquisition), String> {
        let candidate = commit.candidate();
        if commit.geometry() != geometry(self.layout) {
            return Err("The frozen Window display does not match its source.".into());
        }
        let selected = self
            .selected
            .iter()
            .find(|item| item.candidate() == candidate)
            .ok_or("The frozen Window selection does not match its source.")?;
        let cancellation = CaptureCancellation::from_flag(boundary);
        let selection = WindowCaptureSelection::new(
            selected.borrowed(),
            self.topology.borrowed(),
            self.own_pid,
            self.token,
            &cancellation,
        )
        .map_err(|error| error.to_string())?;
        let current = self.backend.observe()?;
        validate_evidence(&current, self.layout)?;
        let decision = decide_window_refresh(
            WindowRefreshSource {
                independent_window: true,
                scrolling_active: false,
                cut_from_frozen: true,
                window_id: candidate.window_id,
                frame: Some(candidate.frame_local_points),
            },
            current.occlusion_rows.as_deref(),
            self.own_pid,
            self.layers,
        )
        .map_err(|error| error.to_string())?;
        Ok((
            WindowRefreshContext {
                selection: commit.token(),
                window_id: candidate.window_id,
                display: commit.geometry(),
            },
            decision,
            Acquisition {
                source: self.clone(),
                selection,
                #[cfg(test)]
                generator: None,
            },
        ))
    }
}

pub struct Acquisition {
    source: Arc<Source>,
    selection: WindowCaptureSelection,
    #[cfg(test)]
    generator: Option<Arc<fixtures::Generator>>,
}
impl Acquisition {
    pub fn run(
        self,
        cancellation: Arc<AtomicBool>,
    ) -> Result<(ScreenshotDocument, Publication), String> {
        let started = Instant::now();
        let flag = CaptureCancellation::from_flag(cancellation.clone());
        if flag.is_cancelled() || self.selection.is_cancelled() {
            return Err("Window acquisition was cancelled.".into());
        }
        let request = AcquisitionRequest {
            selection: self.selection,
            options: WindowCaptureOptions::default(),
            session: self.source.session.clone(),
            cancellation: flag.clone(),
            #[cfg(any(debug_assertions, test))]
            layout: self.source.layout,
            #[cfg(any(debug_assertions, test))]
            cancellation_flag: cancellation,
        };
        #[cfg(test)]
        let acquired = if let Some(generator) = self.generator {
            self.source
                .supplied
                .as_ref()
                .ok_or("No supplied backend.")?
                .acquire_with(&request, &generator)?
        } else {
            self.source.backend.acquire(&request)?
        };
        #[cfg(not(test))]
        let acquired = self.source.backend.acquire(&request)?;
        let Acquired { document, plan } = acquired;
        request.validate_completion_plan(&plan)?;
        if !document.annotations().is_empty() || document.crop_rect().is_some() {
            return Err("The independent Window image must contain only base pixels.".into());
        }
        let (width, height) = document.dimensions();
        let publication = Publication {
            source: self.source,
            plan,
            image_size: CapturePixelSize { width, height },
            cancellation: flag,
            started,
        };
        publication.validate()?;
        Ok((document, publication))
    }
}

/// The editor also checks document/base epoch, coordinator generation and liveness
/// at publication. Fresh backend evidence is independent of acquisition evidence.
pub struct Publication {
    source: Arc<Source>,
    plan: Arc<WindowCapturePlan>,
    image_size: CapturePixelSize,
    cancellation: CaptureCancellation,
    started: Instant,
}
impl Publication {
    pub fn is_current(&self) -> bool {
        self.validate().is_ok()
    }
    fn validate(&self) -> Result<(), String> {
        if self.started.elapsed() > self.plan.options().timeout {
            return Err("Window image refresh timed out.".into());
        }
        let current = self.source.backend.observe()?;
        validate_evidence(&current, self.source.layout)?;
        let fresh: Vec<_> = current
            .observations
            .iter()
            .map(Observation::borrowed)
            .collect();
        self.plan
            .validate_completion(
                &fresh,
                current.topology.borrowed(),
                self.source
                    .session
                    .current()
                    .map_err(|error| error.to_string())?,
                self.image_size,
                &self.cancellation,
            )
            .map_err(|error| error.to_string())
    }
}

fn ui_rect(rect: CaptureRect) -> Rect {
    Rect::new(
        rect.origin.x as f32,
        rect.origin.y as f32,
        rect.size.width as f32,
        rect.size.height as f32,
    )
}
fn geometry(layout: MainDisplayOverlayLayout) -> AreaDisplayGeometry {
    AreaDisplayGeometry {
        display_id: layout.display.id,
        cocoa_frame: ui_rect(layout.cocoa_frame),
        pixel_size: (layout.display.pixels.width, layout.display.pixels.height),
        rotation_degrees: layout.rotation_degrees,
    }
}
fn validate_identity(identity: WindowIdentity<'_>) -> Result<(), String> {
    if identity.window_id == 0
        || identity.owner_process_id <= 0
        || identity.owner_bundle_id.is_some_and(|value| {
            value.is_empty()
                || value.len() > MAX_WINDOW_IDENTITY_BYTES
                || value.chars().any(char::is_control)
        })
    {
        return Err(WindowPolicyError::InvalidMetadata.to_string());
    }
    Ok(())
}
fn validate_frame(rect: CaptureRect) -> Result<(), String> {
    if [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ]
    .iter()
    .any(|value| !value.is_finite() || value.abs() > 1_000_000.)
        || rect.size.width <= 0.
        || rect.size.height <= 0.
    {
        return Err(WindowPolicyError::InvalidGeometry.to_string());
    }
    Ok(())
}
fn validate_evidence(evidence: &Evidence, layout: MainDisplayOverlayLayout) -> Result<(), String> {
    layout.validate().map_err(|error| error.to_string())?;
    let topology = &evidence.topology;
    if evidence.observations.len() > MAX_WINDOW_CANDIDATES
        || evidence
            .occlusion_rows
            .as_ref()
            .is_some_and(|rows| rows.len() > MAX_OCCLUSION_ROWS)
        || topology.displays.is_empty()
        || topology.displays.len() > 64
        || topology.main_display_id != layout.display.id
    {
        return Err("Invalid supplied Window catalog or topology.".into());
    }
    for (index, observation) in evidence.observations.iter().enumerate() {
        validate_identity(observation.borrowed().identity)?;
        validate_frame(observation.frame)?;
        if !observation.alpha.is_finite()
            || !(0.0..=1.0).contains(&observation.alpha)
            || evidence.observations[..index]
                .iter()
                .any(|old| old.window_id == observation.window_id)
        {
            return Err(WindowPolicyError::InvalidMetadata.to_string());
        }
    }
    for (index, item) in topology.displays.iter().enumerate() {
        CaptureRequest::full_display(item.display)
            .validate()
            .map_err(|error| error.to_string())?;
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
            .any(|value| !value.is_finite() || !(1.0..=1_000_000.).contains(value))
        {
            return Err(WindowPolicyError::InvalidGeometry.to_string());
        }
    }
    let main = topology
        .displays
        .iter()
        .find(|item| item.display.id == topology.main_display_id)
        .ok_or("Missing supplied main display.")?;
    if main.display != layout.display
        || main.appkit_size_points != layout.cocoa_frame.size
        || main.display.bounds.size != main.appkit_size_points
        || main.backing_scale != layout.backing_scale
        || main.rotation_degrees != f64::from(layout.rotation_degrees)
    {
        return Err("Supplied Window topology does not match the frozen overlay layout.".into());
    }
    Ok(())
}

#[cfg(any(debug_assertions, test))]
mod fixtures;
#[cfg(test)]
pub use fixtures::fixture;
#[cfg(any(debug_assertions, test))]
pub use fixtures::{Fixture, fixture_layout};
#[cfg(test)]
mod tests;

#[cfg(debug_assertions)]
pub use fixtures::fixture_for_ui;

#[cfg(test)]
pub use fixtures::ReturnedPlanFault;
