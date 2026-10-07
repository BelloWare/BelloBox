//! Supplied generators and mutable evidence are excluded from ordinary builds.
use super::*;
use bello_platform::native_capture::CaptureDisplay;
use bellobox_core::screenshot::window::{synthetic_independent_window, synthetic_window_fixture};
use std::sync::Mutex;

pub(super) type Generator = dyn Fn(&Submission, CapturePixelSize, &AtomicBool) -> Result<ScreenshotDocument, String>
    + Send
    + Sync;
pub(super) type Observer = dyn Fn(&WindowObservationRequest) -> Result<(), String> + Send + Sync;
pub(super) struct SuppliedBackend {
    pub revision: AtomicU64,
    observer: Option<Arc<Observer>>,
    own_pid: i32,
    layers: OcclusionLayers,
    pub current: Mutex<Evidence>,
    pub submissions: Vec<Submission>,
    generator: Arc<Generator>,
}
impl Backend for SuppliedBackend {
    fn check_available(&self) -> Result<(), String> {
        Ok(())
    }
    fn identity_and_layers(&self) -> Result<(i32, OcclusionLayers), String> {
        Ok((self.own_pid, self.layers))
    }
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }
    fn observe(&self, request: &WindowObservationRequest) -> Result<Evidence, String> {
        request.check().map_err(|e| e.to_string())?;
        if let Some(observer) = &self.observer {
            observer(request)?;
        }
        request.check().map_err(|e| e.to_string())?;
        self.current
            .lock()
            .map(|evidence| evidence.clone())
            .map_err(|_| "Window evidence is unavailable.".into())
    }
    fn acquire(&self, request: &AcquisitionRequest) -> Result<Acquired, String> {
        self.acquire_with(request, &self.generator)
    }
}
impl SuppliedBackend {
    pub(super) fn acquire_with(
        &self,
        request: &AcquisitionRequest,
        generator: &Arc<Generator>,
    ) -> Result<Acquired, String> {
        let submission = self
            .submissions
            .iter()
            .find(|item| item.window_id == request.selection.selected_identity().window_id)
            .ok_or("The independent Window image is unavailable.")?;
        if self
            .submissions
            .iter()
            .filter(|item| item.window_id == submission.window_id)
            .count()
            != 1
        {
            return Err("Ambiguous callback Window submission.".into());
        }
        // This is a supplied callback source, distinct from the initial CG-style
        // catalog. Validate BEFORE any generator runs, exactly like native SCK.
        let (current, _) = observe_backend(self, &request.observation)?;
        let plan = request.bind_submission(submission, &current)?;
        let document = generator(submission, plan.output_size(), &request.cancellation_flag)?;
        Ok(Acquired { document, plan })
    }
}
impl Source {
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
        if submissions.len() > MAX_WINDOW_CANDIDATES {
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
        let supplied = Arc::new(SuppliedBackend {
            revision: AtomicU64::new(0),
            observer: None,
            own_pid,
            layers,
            current: Mutex::new(evidence),
            submissions,
            generator: Arc::new(generator),
        });
        let mut source = Self::from_backend(layout, supplied.clone())?;
        Arc::get_mut(&mut source).unwrap().supplied = Some(supplied);
        Ok(source)
    }
    #[cfg(test)]
    pub fn with_observation_gate(
        mut self: Arc<Self>,
        gate: super::super::window_refresh::DisposalGate,
    ) -> Arc<Self> {
        Arc::get_mut(&mut self).unwrap().observation_gate = Some(gate);
        self
    }
    #[cfg(test)]
    pub fn with_refresh_timeout(mut self: Arc<Self>, timeout: std::time::Duration) -> Arc<Self> {
        assert!(!timeout.is_zero() && timeout <= WindowCaptureOptions::default().timeout);
        Arc::get_mut(&mut self).unwrap().refresh_timeout = timeout;
        self
    }
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
        let this = Arc::get_mut(&mut self)
            .expect("configure supplied generator before sharing its source");
        let old = this.supplied.as_ref().unwrap();
        let supplied = Arc::new(SuppliedBackend {
            revision: AtomicU64::new(old.revision()),
            observer: old.observer.clone(),
            own_pid: old.own_pid,
            layers: old.layers,
            current: Mutex::new(old.current.lock().unwrap().clone()),
            submissions: old.submissions.clone(),
            generator: Arc::new(generator),
        });
        this.backend = supplied.clone();
        this.supplied = Some(supplied);
        self
    }
    #[cfg(test)]
    pub fn with_submissions(
        mut self: Arc<Self>,
        update: impl FnOnce(&mut Vec<Submission>),
    ) -> Arc<Self> {
        let this = Arc::get_mut(&mut self).expect("configure callback evidence before sharing");
        let old = this.supplied.as_ref().unwrap();
        let mut submissions = old.submissions.clone();
        update(&mut submissions);
        let supplied = Arc::new(SuppliedBackend {
            revision: AtomicU64::new(old.revision()),
            observer: old.observer.clone(),
            own_pid: old.own_pid,
            layers: old.layers,
            current: Mutex::new(old.current.lock().unwrap().clone()),
            submissions,
            generator: old.generator.clone(),
        });
        this.backend = supplied.clone();
        this.supplied = Some(supplied);
        self
    }
    #[cfg(test)]
    pub fn update_current(&self, update: impl FnOnce(&mut Evidence)) {
        let supplied = self.supplied.as_ref().unwrap();
        let mut current = supplied.current.lock().expect("supplied evidence mutex");
        let before = current.clone();
        update(&mut current);
        // Topology vector order is not identity; preserve that existing negative
        // control while fencing every meaningful explicit fixture mutation.
        if !equivalent_evidence(&before, &current) {
            supplied.revision.fetch_add(1, Ordering::Release);
        }
    }
    #[cfg(test)]
    pub fn advance_generation(&self) {
        let mut token = self.session.current().expect("supplied session");
        token.generation += 1;
        self.session.update(token).expect("supplied generation");
    }
}
#[cfg(test)]
fn equivalent_evidence(a: &Evidence, b: &Evidence) -> bool {
    a.observations == b.observations
        && a.topology.main_display_id == b.topology.main_display_id
        && a.topology.displays.len() == b.topology.displays.len()
        && a.topology
            .displays
            .iter()
            .all(|item| b.topology.displays.contains(item))
        && match (&a.occlusion_rows, &b.occlusion_rows) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(a, b)| {
                        a.window_id == b.window_id
                            && a.owner_process_id == b.owner_process_id
                            && a.layer == b.layer
                            && a.alpha == b.alpha
                            && a.frame == b.frame
                    })
            }
            _ => false,
        }
}
impl Source {
    pub fn with_observer(
        mut self: Arc<Self>,
        observer: impl Fn(&WindowObservationRequest) -> Result<(), String> + Send + Sync + 'static,
    ) -> Arc<Self> {
        let this = Arc::get_mut(&mut self).expect("configure observer before sharing source");
        let old = this.supplied.as_ref().unwrap();
        let supplied = Arc::new(SuppliedBackend {
            revision: AtomicU64::new(old.revision()),
            observer: Some(Arc::new(observer)),
            own_pid: old.own_pid,
            layers: old.layers,
            current: Mutex::new(old.current.lock().unwrap().clone()),
            submissions: old.submissions.clone(),
            generator: old.generator.clone(),
        });
        this.backend = supplied.clone();
        this.supplied = Some(supplied);
        self
    }
}

#[cfg(test)]
impl Acquisition {
    pub fn observation_gate(&self) -> Option<super::super::window_refresh::DisposalGate> {
        self.source.observation_gate.clone()
    }
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
        self.generator = Some(Arc::new(generator));
        self
    }
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

/// Explicit DEBUG-only GUI controls. These delay/fail the same injected acquisition
/// used by normal Window composition; there is no second editor or refresh host.
#[cfg(debug_assertions)]
pub fn fixture_for_ui() -> Result<Fixture, String> {
    let fixture = fixture()?;
    let mode = std::env::var("BELLOBOX_INLINE_WINDOW_REFRESH").unwrap_or_default();
    if mode == "observation-delayed" {
        let first = AtomicBool::new(true);
        let source = fixture.source.with_observer(move |request| {
            if first.swap(false, Ordering::AcqRel) {
                for _ in 0..80 {
                    request.check().map_err(|e| e.to_string())?;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
            Ok(())
        });
        return Ok(Fixture { source, ..fixture });
    }
    if !matches!(mode.as_str(), "delayed" | "failure") {
        return Ok(fixture);
    }
    let source = fixture
        .source
        .with_generator(move |submission, _, cancellation| {
            for _ in 0..80 {
                if cancellation.load(Ordering::Acquire) {
                    return Err("Supplied refresh cancelled.".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if mode == "failure" {
                return Err("Controlled supplied Window failure.".into());
            }
            synthetic_independent_window(submission.window_id, cancellation)
                .map_err(|error| error.to_string())
        });
    Ok(Fixture { source, ..fixture })
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub enum ReturnedPlanFault {
    OtherWindow,
    EquivalentSelection,
    Cursor,
    Deadline,
}
#[cfg(test)]
struct WrongCompletionBackend {
    supplied: Arc<SuppliedBackend>,
    fault: ReturnedPlanFault,
}
#[cfg(test)]
impl Backend for WrongCompletionBackend {
    fn check_available(&self) -> Result<(), String> {
        Ok(())
    }
    fn identity_and_layers(&self) -> Result<(i32, OcclusionLayers), String> {
        self.supplied.identity_and_layers()
    }
    fn observe(&self, request: &WindowObservationRequest) -> Result<Evidence, String> {
        self.supplied.observe(request)
    }
    fn revision(&self) -> u64 {
        self.supplied.revision()
    }
    fn acquire(&self, request: &AcquisitionRequest) -> Result<Acquired, String> {
        let mut acquired = self.supplied.acquire(request)?;
        let mut current = self.observe(&request.observation)?;
        let mut observed = current
            .observations
            .iter()
            .find(|item| item.window_id == request.selection.selected_identity().window_id)
            .unwrap()
            .clone();
        if matches!(self.fault, ReturnedPlanFault::OtherWindow) {
            observed.window_id = 77;
            observed.owner_process_id += 7;
            // Equal frame/dimensions but a different valid catalog identity in
            // the SAME live source/session. Completion validation alone accepts.
            current.observations.push(observed.clone());
            self.supplied
                .current
                .lock()
                .unwrap()
                .observations
                .push(observed.clone());
        }
        let selection = if matches!(
            self.fault,
            ReturnedPlanFault::OtherWindow | ReturnedPlanFault::EquivalentSelection
        ) {
            WindowCaptureSelection::new(
                observed.borrowed(),
                current.topology.borrowed(),
                self.supplied.own_pid,
                request.session.current().map_err(|e| e.to_string())?,
                &request.cancellation,
            )
            .map_err(|e| e.to_string())?
        } else {
            request.selection.clone()
        };
        let mut options = request.options;
        match self.fault {
            ReturnedPlanFault::Cursor => options.include_cursor = !options.include_cursor,
            ReturnedPlanFault::Deadline => options.timeout -= std::time::Duration::from_millis(1),
            _ => {}
        }
        let submission = bello_platform::window_capture::SubmittedWindowSource {
            identity: observed.borrowed().identity,
            frame: WindowFrame(observed.frame),
            layer: observed.layer,
            on_screen: observed.on_screen,
        };
        let fresh: Vec<_> = current
            .observations
            .iter()
            .map(Observation::borrowed)
            .collect();
        let plan = selection
            .plan(
                submission,
                &fresh,
                current.topology.borrowed(),
                request.session.current().unwrap(),
                options,
                &request.cancellation,
            )
            .map_err(|e| e.to_string())?;
        assert_eq!(plan.output_size(), acquired.plan.output_size());
        assert!(!plan.matches_request(&request.selection, request.options));
        plan.validate_completion(
            &fresh,
            current.topology.borrowed(),
            request.session.current().unwrap(),
            plan.output_size(),
            &request.cancellation,
        )
        .unwrap();
        acquired.plan = Arc::new(plan);
        Ok(acquired)
    }
}
#[cfg(test)]
impl Source {
    pub fn with_returned_plan_fault(mut self: Arc<Self>, fault: ReturnedPlanFault) -> Arc<Self> {
        let this = Arc::get_mut(&mut self).expect("configure returned plan before sharing");
        this.backend = Arc::new(WrongCompletionBackend {
            supplied: this.supplied.as_ref().unwrap().clone(),
            fault,
        });
        self
    }
}
