//! DEBUG-only supplied Window evidence, not native enumeration or capture.
//!
//! The selector uses normal-layer, non-own windows wholly on the main display.
//! Occlusion consumes a separate, raw front-to-back catalog, retaining our own
//! regular windows. Exact f64 evidence survives independently of the f32 UI.
//! Source and submission records are synthetic owned values: they cannot prove
//! native object retention, atomic window incarnation, permissions or pixels.
use bello_platform::{
    macos_capture_overlay::MainDisplayOverlayLayout,
    native_capture::{
        CaptureCancellation, CaptureDisplay, CapturePixelSize, CaptureRect, CaptureRequest,
        WindowCaptureSession,
    },
    window_capture::{
        MAX_WINDOW_CANDIDATES, MAX_WINDOW_IDENTITY_BYTES, SubmittedWindowSource,
        WindowCaptureOptions, WindowCapturePlan, WindowCaptureSelection, WindowDisplayGeometry,
        WindowFrame, WindowIdentity, WindowObservation, WindowPolicyError, WindowSelectionToken,
        WindowTopology,
    },
};
use bellobox_core::screenshot::{
    Rect, ScreenshotDocument,
    area::AreaDisplayGeometry,
    window::{
        FrozenWindowCandidate, FrozenWindowCommit, synthetic_independent_window,
        synthetic_window_fixture,
    },
    window_refresh::{
        MAX_OCCLUSION_ROWS, OcclusionLayers, OcclusionRow, WindowRefreshContext,
        WindowRefreshDecision, WindowRefreshSource, decide_window_refresh,
    },
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Complete supplied CG-style evidence. Mutable titles are intentionally absent.
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

/// Immutable evidence associated with the exact supplied image generator input.
/// It is never used as a substitute for fresh observations after generation.
#[derive(Clone, Debug)]
pub struct Submission {
    pub window_id: u32,
    pub owner_process_id: i32,
    pub owner_bundle_id: Option<String>,
    pub frame: CaptureRect,
    pub layer: i64,
    pub on_screen: bool,
}
impl Submission {
    fn borrowed(&self) -> SubmittedWindowSource<'_> {
        SubmittedWindowSource {
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

type Generator = dyn Fn(&Submission, CapturePixelSize, &AtomicBool) -> Result<ScreenshotDocument, String>
    + Send
    + Sync;

pub struct Source {
    layout: MainDisplayOverlayLayout,
    own_pid: i32,
    selected: Vec<Observation>,
    topology: Topology,
    submissions: Vec<Submission>,
    layers: OcclusionLayers,
    token: WindowSelectionToken,
    session: WindowCaptureSession,
    current: Mutex<Evidence>,
    generator: Arc<Generator>,
}
impl Source {
    /// All inputs are already supplied in memory. The closure receives the same
    /// immutable submission that policy validates; it must return bare pixels.
    pub fn new(
        layout: MainDisplayOverlayLayout,
        own_pid: i32,
        evidence: Evidence,
        submissions: Vec<Submission>,
        layers: OcclusionLayers,
        generator: impl Fn(
            &Submission,
            CapturePixelSize,
            &AtomicBool,
        ) -> Result<ScreenshotDocument, String>
        + Send
        + Sync
        + 'static,
    ) -> Result<Arc<Self>, String> {
        validate_evidence(&evidence, layout)?;
        if own_pid <= 0 || layers.normal != 0 || submissions.len() > MAX_WINDOW_CANDIDATES {
            return Err("Invalid supplied Window source.".into());
        }
        for (index, submission) in submissions.iter().enumerate() {
            validate_identity(submission.borrowed().identity)?;
            validate_frame(submission.frame)?;
            if submissions[..index]
                .iter()
                .any(|old| old.window_id == submission.window_id)
            {
                return Err("Ambiguous supplied Window submission.".into());
            }
        }
        // Checked CAS keeps session IDs unique without wrapping and supports the
        // workspace's Rust 1.88 MSRV (newer atomics renamed fetch_update).
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
                    // The exact policy is authoritative; reject a UI conversion
                    // that no longer fits instead of expanding its eligible scope.
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
            topology: evidence.topology.clone(),
            submissions,
            layers,
            token,
            session: WindowCaptureSession::new(token).map_err(|error| error.to_string())?,
            current: Mutex::new(evidence),
            generator: Arc::new(generator),
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
            return Err("The frozen Window display does not match its supplied source.".into());
        }
        let selected = self
            .selected
            .iter()
            .find(|item| item.candidate() == candidate)
            .ok_or("The frozen Window selection does not match its supplied source.")?;
        let cancellation = CaptureCancellation::from_flag(boundary);
        let selection = WindowCaptureSelection::new(
            selected.borrowed(),
            self.topology.borrowed(),
            self.own_pid,
            self.token,
            &cancellation,
        )
        .map_err(|error| error.to_string())?;
        let submission = self
            .submissions
            .iter()
            .find(|item| item.window_id == selected.window_id)
            .ok_or("The supplied independent Window image is unavailable.")?
            .clone();
        let current = self
            .current
            .lock()
            .map_err(|_| "Window evidence is unavailable.")?;
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
                submission,
                generator: self.generator.clone(),
            },
        ))
    }
    /// Configure an unshared fixture before handing it to the coordinator.
    #[cfg(test)]
    pub fn with_generator(
        mut self: Arc<Self>,
        generator: impl Fn(
            &Submission,
            CapturePixelSize,
            &AtomicBool,
        ) -> Result<ScreenshotDocument, String>
        + Send
        + Sync
        + 'static,
    ) -> Arc<Self> {
        Arc::get_mut(&mut self)
            .expect("configure supplied generator before sharing its source")
            .generator = Arc::new(generator);
        self
    }
    /// Test mutation models fresh supplied observations without enumerating an OS.
    #[cfg(test)]
    pub fn update_current(&self, update: impl FnOnce(&mut Evidence)) {
        update(&mut self.current.lock().expect("supplied evidence mutex"));
    }
    #[cfg(test)]
    pub fn advance_generation(&self) {
        let mut token = self.session.current().expect("supplied session");
        token.generation += 1;
        self.session.update(token).expect("supplied generation");
    }
}

pub struct Acquisition {
    source: Arc<Source>,
    selection: WindowCaptureSelection,
    submission: Submission,
    generator: Arc<Generator>,
}
impl Acquisition {
    #[cfg(test)]
    pub fn with_generator(
        mut self,
        generator: impl Fn(
            &Submission,
            CapturePixelSize,
            &AtomicBool,
        ) -> Result<ScreenshotDocument, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.generator = Arc::new(generator);
        self
    }
    pub fn run(
        self,
        cancellation: Arc<AtomicBool>,
    ) -> Result<(ScreenshotDocument, Publication), String> {
        let started = Instant::now();
        let flag = CaptureCancellation::from_flag(cancellation.clone());
        let plan = {
            let current = self
                .source
                .current
                .lock()
                .map_err(|_| "Window evidence is unavailable.")?;
            validate_evidence(&current, self.source.layout)?;
            let fresh: Vec<_> = current
                .observations
                .iter()
                .map(Observation::borrowed)
                .collect();
            self.selection
                .plan(
                    self.submission.borrowed(),
                    &fresh,
                    current.topology.borrowed(),
                    self.source
                        .session
                        .current()
                        .map_err(|error| error.to_string())?,
                    WindowCaptureOptions::default(),
                    &flag,
                )
                .map_err(|error| error.to_string())?
        };
        // No evidence lock is held during supplied pixel generation. Tests may
        // deterministically change fresh evidence while their generator blocks.
        let document = (self.generator)(&self.submission, plan.output_size(), &cancellation)?;
        if !document.annotations().is_empty() || document.crop_rect().is_some() {
            return Err(
                "The supplied independent Window image must contain only base pixels.".into(),
            );
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

/// Pointer-free completion evidence. The editor must additionally check its
/// document, host generation and liveness at its actual publication boundary.
pub struct Publication {
    source: Arc<Source>,
    plan: WindowCapturePlan,
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
            return Err("Supplied Window image refresh timed out.".into());
        }
        let current = self
            .source
            .current
            .lock()
            .map_err(|_| "Window evidence is unavailable.")?;
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

pub struct Fixture {
    pub layout: MainDisplayOverlayLayout,
    pub source: Arc<Source>,
    pub document: ScreenshotDocument,
}
/// Cheap supplied metadata, with no document generation or native display read.
pub fn fixture_layout() -> MainDisplayOverlayLayout {
    MainDisplayOverlayLayout {
        display: CaptureDisplay {
            id: 1,
            bounds: CaptureRect::new(0., 0., 800., 500.),
            pixels: CapturePixelSize {
                width: 1600,
                height: 1000,
            },
        },
        cocoa_frame: CaptureRect::new(0., 0., 800., 500.),
        rotation_degrees: 0,
        backing_scale: 2.,
    }
}
pub fn fixture() -> Result<Fixture, String> {
    let layout = fixture_layout();
    let (display, candidates, document) =
        synthetic_window_fixture().map_err(|error| error.to_string())?;
    if display != geometry(layout) || document.dimensions() != (1600, 1000) {
        return Err("Synthetic Window fixture geometry changed.".into());
    }
    let observations: Vec<_> = candidates
        .iter()
        .map(|item| Observation {
            window_id: item.window_id,
            owner_process_id: item.owner_process_id,
            owner_bundle_id: None,
            frame: CaptureRect::new(
                f64::from(item.frame_local_points.x),
                f64::from(item.frame_local_points.y),
                f64::from(item.frame_local_points.width),
                f64::from(item.frame_local_points.height),
            ),
            layer: item.layer,
            alpha: item.alpha,
            on_screen: item.on_screen,
        })
        .collect();
    let submissions = observations
        .iter()
        .map(|item| Submission {
            window_id: item.window_id,
            owner_process_id: item.owner_process_id,
            owner_bundle_id: Some(format!("example.synthetic.window{}", item.window_id)),
            frame: item.frame,
            layer: item.layer,
            on_screen: item.on_screen,
        })
        .collect();
    let rows = observations
        .iter()
        .map(|item| OcclusionRow {
            window_id: Some(item.window_id),
            owner_process_id: Some(item.owner_process_id),
            layer: Some(item.layer),
            alpha: Some(item.alpha),
            frame: Some(ui_rect(item.frame)),
        })
        .collect();
    let source = Source::new(
        layout,
        999,
        Evidence {
            observations,
            topology: Topology {
                main_display_id: layout.display.id,
                displays: vec![WindowDisplayGeometry {
                    display: layout.display,
                    appkit_size_points: layout.cocoa_frame.size,
                    backing_scale: layout.backing_scale,
                    rotation_degrees: 0.,
                }],
            },
            occlusion_rows: Some(rows),
        },
        submissions,
        // Fixture values, never claimed to be native CGWindowLevelForKey results.
        OcclusionLayers {
            normal: 0,
            floating: 3,
            modal_panel: 8,
            main_menu: 24,
            status: 25,
            popup_menu: 101,
            screen_saver: 1000,
        },
        |submission, _, cancellation| {
            synthetic_independent_window(submission.window_id, cancellation)
                .map_err(|error| error.to_string())
        },
    )?;
    Ok(Fixture {
        layout,
        source,
        document,
    })
}

#[cfg(test)]
mod tests;
