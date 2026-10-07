use super::*;
use bellobox_core::screenshot::window::synthetic_independent_window;
use bellobox_core::screenshot::{
    AnnotationKind, AnnotationStyle, Point, ScreenshotEditSession, window::FrozenWindowSession,
};
use std::sync::Mutex;
use std::{
    sync::{atomic::AtomicUsize, mpsc},
    time::Duration,
};

fn flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
fn select(fixture: &Fixture, id: u32) -> (FrozenWindowSession, FrozenWindowCommit) {
    let display = geometry(fixture.layout);
    let mut selector = FrozenWindowSession::new(
        display,
        &fixture.source.candidates().unwrap(),
        fixture.source.own_pid(),
    )
    .unwrap();
    selector
        .accept_frozen(
            selector.freeze_token().unwrap(),
            fixture.document.clone(),
            display,
        )
        .unwrap();
    let point = if id == 1 {
        Point::new(400., 120.)
    } else {
        Point::new(150., 200.)
    };
    selector.begin_press(point, display).unwrap();
    let commit = selector.end_press(point, display).unwrap().unwrap();
    (selector, commit)
}
fn evidence(source: &Source) -> Evidence {
    source
        .supplied
        .as_ref()
        .unwrap()
        .current
        .lock()
        .unwrap()
        .clone()
}
fn rebuild(
    fixture: &Fixture,
    evidence: Evidence,
    submissions: Vec<Submission>,
) -> Result<Arc<Source>, String> {
    Source::new(
        fixture.layout,
        fixture.source.own_pid(),
        evidence,
        submissions,
        fixture.source.layers,
        |submission, _, cancellation| {
            synthetic_independent_window(submission.window_id, cancellation)
                .map_err(|error| error.to_string())
        },
    )
}
fn added_window(id: u32, pid: i32, layer: i64) -> Observation {
    Observation {
        window_id: id,
        owner_process_id: pid,
        owner_bundle_id: None,
        frame: CaptureRect::new(300., 80., 100., 100.),
        layer,
        alpha: 1.,
        on_screen: true,
    }
}
fn row(observation: &Observation) -> OcclusionRow {
    OcclusionRow {
        window_id: Some(observation.window_id),
        owner_process_id: Some(observation.owner_process_id),
        layer: Some(observation.layer),
        alpha: Some(observation.alpha),
        frame: Some(ui_rect(observation.frame)),
    }
}

#[test]
fn fixture_connects_exact_policy_to_both_independent_image_decisions() {
    fn require_send<T: Send>() {}
    require_send::<Acquisition>();
    require_send::<Publication>();
    let fixture = fixture().unwrap();
    assert_eq!(fixture.layout, fixture_layout());
    assert_eq!(fixture.document.dimensions(), (1600, 1000));
    assert_eq!(
        fixture
            .source
            .candidates()
            .unwrap()
            .iter()
            .map(|item| item.window_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    for id in [1, 2] {
        let (_selector, commit) = select(&fixture, id);
        let (context, decision, acquisition) = fixture.source.request(&commit, flag()).unwrap();
        assert_eq!(context.selection, commit.token());
        assert_eq!(context.window_id, id);
        assert_eq!(
            decision,
            if id == 1 {
                WindowRefreshDecision::MaskFrozenAlpha
            } else {
                WindowRefreshDecision::ReplaceWithIndependent
            }
        );
        let (image, publication) = acquisition.run(flag()).unwrap();
        assert_eq!(
            image.dimensions(),
            if id == 1 { (680, 540) } else { (840, 460) }
        );
        assert!(publication.is_current());
        let pixel = image.render_rgba().unwrap().get_pixel(100, 100).0;
        // Independent front pixels deliberately differ from the frozen orange.
        if id == 1 {
            assert_ne!(pixel, [255, 224, 186, 255]);
        } else {
            assert_eq!(pixel, [185, 215, 241, 255]);
        }
    }
}

#[test]
fn selector_filters_exact_scope_but_raw_occlusion_retains_own_regular_windows() {
    let mut fixture = fixture().unwrap();
    let mut supplied = evidence(&fixture.source);
    let own = added_window(3, fixture.source.own_pid(), 0);
    supplied.observations.insert(0, own.clone());
    supplied.observations.push(added_window(4, 104, 3));
    let mut outside = added_window(5, 105, 0);
    outside.frame.origin.x = -0.000_001;
    supplied.observations.push(outside);
    let mut hidden = added_window(6, 106, 0);
    hidden.on_screen = false;
    supplied.observations.push(hidden);
    let mut transparent = added_window(7, 107, 0);
    transparent.alpha = 0.01;
    supplied.observations.push(transparent);
    let mut tiny = added_window(8, 108, 0);
    tiny.frame.size.width = 7.999_999;
    assert_eq!(tiny.frame.size.width as f32, 7.999_999_f32);
    supplied.observations.push(tiny);
    supplied
        .occlusion_rows
        .as_mut()
        .unwrap()
        .insert(0, row(&own));
    fixture.source = rebuild(
        &fixture,
        supplied,
        fixture
            .source
            .supplied
            .as_ref()
            .unwrap()
            .submissions
            .clone(),
    )
    .unwrap();
    assert_eq!(
        fixture
            .source
            .candidates()
            .unwrap()
            .iter()
            .map(|item| item.window_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let (_selector, commit) = select(&fixture, 1);
    let (_, decision, _) = fixture.source.request(&commit, flag()).unwrap();
    assert_eq!(decision, WindowRefreshDecision::ReplaceWithIndependent);
    // Move the same own row behind the target: no sorting or selector filtering.
    fixture.source.update_current(|current| {
        let rows = current.occlusion_rows.as_mut().unwrap();
        let own = rows.remove(0);
        rows.push(own);
    });
    let (_, decision, _) = fixture.source.request(&commit, flag()).unwrap();
    assert_eq!(decision, WindowRefreshDecision::MaskFrozenAlpha);
}

#[test]
fn raw_rows_preserve_missing_fields_visible_layers_and_overlay_exclusion() {
    let fixture = fixture().unwrap();
    let (_selector, commit) = select(&fixture, 1);
    for (layer, pid, expected) in [
        (3, 300, WindowRefreshDecision::ReplaceWithIndependent),
        (1000, 999, WindowRefreshDecision::MaskFrozenAlpha),
        (0, 999, WindowRefreshDecision::ReplaceWithIndependent),
    ] {
        fixture.source.update_current(|current| {
            current.occlusion_rows = Some(vec![
                OcclusionRow {
                    window_id: None,
                    owner_process_id: None,
                    layer: None,
                    alpha: None,
                    frame: None,
                },
                row(&added_window(55, pid, layer)),
                // Target stops the source scan even if the other fields are absent.
                OcclusionRow {
                    window_id: Some(1),
                    owner_process_id: None,
                    layer: None,
                    alpha: None,
                    frame: None,
                },
                row(&added_window(56, 400, 0)),
            ]);
        });
        assert_eq!(fixture.source.request(&commit, flag()).unwrap().1, expected);
    }
    fixture
        .source
        .update_current(|current| current.occlusion_rows = None);
    assert_eq!(
        fixture.source.request(&commit, flag()).unwrap().1,
        WindowRefreshDecision::MaskFrozenAlpha
    );
}

#[test]
fn selection_catalog_keeps_supplied_front_to_back_order() {
    let mut fixture = fixture().unwrap();
    let mut supplied = evidence(&fixture.source);
    supplied.observations.reverse();
    fixture.source = rebuild(
        &fixture,
        supplied,
        fixture
            .source
            .supplied
            .as_ref()
            .unwrap()
            .submissions
            .clone(),
    )
    .unwrap();
    assert_eq!(
        fixture
            .source
            .candidates()
            .unwrap()
            .iter()
            .map(|item| item.window_id)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    let display = geometry(fixture.layout);
    let mut selector =
        FrozenWindowSession::new(display, &fixture.source.candidates().unwrap(), 999).unwrap();
    selector
        .accept_frozen(selector.freeze_token().unwrap(), fixture.document, display)
        .unwrap();
    selector.hover(Point::new(400., 200.), display).unwrap();
    assert_eq!(selector.hovered().unwrap().window_id, 2);
}

#[test]
fn malformed_catalog_identity_and_topology_fail_before_selection() {
    let fixture = fixture().unwrap();
    for case in 0..18 {
        let mut supplied = evidence(&fixture.source);
        match case {
            0 => supplied.observations[0].window_id = 0,
            1 => supplied.observations[0].owner_process_id = 0,
            2 => supplied.observations[0].owner_bundle_id = Some(String::new()),
            3 => supplied.observations[0].owner_bundle_id = Some("bad\0bundle".into()),
            4 => {
                supplied.observations[0].owner_bundle_id =
                    Some("x".repeat(MAX_WINDOW_IDENTITY_BYTES + 1))
            }
            5 => supplied.observations[0].frame.origin.x = f64::NAN,
            6 => supplied.observations[0].frame.size.width = 0.,
            7 => supplied.observations[0].frame.origin.x = 1_000_001.,
            8 => supplied.observations[0].alpha = f64::NAN,
            9 => supplied.observations[0].alpha = 1.1,
            10 => supplied.observations.push(supplied.observations[0].clone()),
            11 => {
                supplied.observations =
                    vec![supplied.observations[0].clone(); MAX_WINDOW_CANDIDATES + 1]
            }
            12 => {
                supplied.occlusion_rows =
                    Some(vec![row(&supplied.observations[0]); MAX_OCCLUSION_ROWS + 1])
            }
            13 => supplied
                .topology
                .displays
                .push(supplied.topology.displays[0]),
            14 => supplied.topology.main_display_id = 999,
            15 => supplied.topology.displays[0].rotation_degrees = 90.,
            16 => supplied.topology.displays[0].backing_scale = f64::NAN,
            _ => supplied.topology.displays[0].appkit_size_points.width += 0.000_001,
        }
        assert!(
            rebuild(
                &fixture,
                supplied,
                fixture
                    .source
                    .supplied
                    .as_ref()
                    .unwrap()
                    .submissions
                    .clone()
            )
            .is_err(),
            "case {case}"
        );
    }
}

#[test]
fn unsupported_and_oversized_supplied_sources_fail_closed() {
    let fixture = fixture().unwrap();
    let mut supplied = evidence(&fixture.source);
    let mut layout = fixture.layout;
    layout.backing_scale = 16.;
    supplied.topology.displays[0].backing_scale = 16.;
    supplied.observations[0].frame = layout.display.bounds;
    assert!(
        Source::new(
            layout,
            999,
            supplied,
            fixture
                .source
                .supplied
                .as_ref()
                .unwrap()
                .submissions
                .clone(),
            fixture.source.layers,
            |_, _, _| unreachable!("invalid source cannot acquire")
        )
        .is_err()
    );
    let mut supplied = evidence(&fixture.source);
    supplied.observations[0].frame.origin.x = 799.;
    let source = rebuild(
        &fixture,
        supplied,
        fixture
            .source
            .supplied
            .as_ref()
            .unwrap()
            .submissions
            .clone(),
    )
    .unwrap();
    assert_eq!(
        source
            .candidates()
            .unwrap()
            .iter()
            .map(|item| item.window_id)
            .collect::<Vec<_>>(),
        [2]
    );
    let mut submissions = fixture
        .source
        .supplied
        .as_ref()
        .unwrap()
        .submissions
        .clone();
    submissions.push(submissions[0].clone());
    assert!(rebuild(&fixture, evidence(&fixture.source), submissions).is_err());
}

#[test]
fn exact_subpixel_change_is_not_hidden_by_selector_f32_conversion() {
    let mut fixture = fixture().unwrap();
    let mut supplied = evidence(&fixture.source);
    supplied.observations[0].frame.origin.x += 0.000_001;
    let mut submissions = fixture
        .source
        .supplied
        .as_ref()
        .unwrap()
        .submissions
        .clone();
    submissions[0].frame = supplied.observations[0].frame;
    fixture.source = rebuild(&fixture, supplied, submissions).unwrap();
    let (_selector, commit) = select(&fixture, 1);
    assert_eq!(commit.candidate().frame_local_points.x, 300.);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    let (_, publication) = acquisition
        .with_generator(|submission, size, cancellation| {
            assert_eq!(submission.frame.origin.x, 300.000_001);
            assert_eq!(
                size,
                CapturePixelSize {
                    width: 680,
                    height: 540
                }
            );
            synthetic_independent_window(1, cancellation).map_err(|error| error.to_string())
        })
        .run(flag())
        .unwrap();
    fixture
        .source
        .update_current(|current| current.observations[0].frame.origin.x += 0.000_001);
    assert_eq!(
        fixture
            .source
            .supplied
            .as_ref()
            .unwrap()
            .current
            .lock()
            .unwrap()
            .observations[0]
            .candidate(),
        commit.candidate()
    );
    assert!(!publication.is_current());
}

#[test]
fn changed_or_missing_fresh_source_never_reaches_the_generator() {
    for case in 0..8 {
        let fixture = fixture().unwrap();
        let (_selector, commit) = select(&fixture, 1);
        let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
        fixture.source.update_current(|current| match case {
            0 => current.observations[0].frame.origin.x += 0.000_001,
            1 => current.observations[0].owner_process_id = 201,
            2 => {
                current.observations.remove(0);
            }
            3 => current.observations.push(current.observations[0].clone()),
            4 => current.observations[0].layer = 3,
            5 => current.observations[0].on_screen = false,
            6 => current.observations[0].alpha = 0.01,
            _ => current.observations[0].owner_bundle_id = Some("conflicting.bundle".into()),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let result = acquisition
            .with_generator(move |_, _, _| {
                counter.fetch_add(1, Ordering::Relaxed);
                Err("must not run".into())
            })
            .run(flag());
        assert!(result.is_err(), "case {case}");
        assert_eq!(calls.load(Ordering::Relaxed), 0, "case {case}");
    }
}

#[test]
fn exact_submitted_source_is_checked_before_generator() {
    let original = fixture().unwrap();
    for case in 0..5 {
        let mut submissions = original
            .source
            .supplied
            .as_ref()
            .unwrap()
            .submissions
            .clone();
        match case {
            0 => submissions[0].owner_process_id = 201,
            1 => submissions[0].frame.origin.x += 0.000_001,
            2 => submissions[0].layer = 3,
            3 => submissions[0].on_screen = false,
            _ => submissions[0].owner_bundle_id = Some("conflicting.bundle".into()),
        }
        let mut supplied = evidence(&original.source);
        supplied.observations[0].owner_bundle_id = Some("example.synthetic.window1".into());
        let fixture = Fixture {
            layout: original.layout,
            document: original.document.clone(),
            source: rebuild(&original, supplied, submissions).unwrap(),
        };
        let (_selector, commit) = select(&fixture, 1);
        let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
        assert!(
            acquisition
                .with_generator(|_, _, _| panic!("invalid submission reached generator"))
                .run(flag())
                .is_err()
        );
    }
}

#[test]
fn fresh_completion_rejects_changes_while_generator_is_blocked() {
    for case in 0..5 {
        let fixture = fixture().unwrap();
        let (_selector, commit) = select(&fixture, 1);
        let boundary = flag();
        let job = flag();
        let (_, _, acquisition) = fixture.source.request(&commit, boundary.clone()).unwrap();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = mpsc::sync_channel(1);
        let resume_rx = Mutex::new(resume_rx);
        let acquisition = acquisition.with_generator(move |submission, _, cancellation| {
            started_tx.send(()).map_err(|error| error.to_string())?;
            resume_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .map_err(|error| error.to_string())?;
            synthetic_independent_window(submission.window_id, cancellation)
                .map_err(|error| error.to_string())
        });
        let worker_job = job.clone();
        let worker = std::thread::spawn(move || acquisition.run(worker_job));
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        match case {
            0 => fixture
                .source
                .update_current(|current| current.observations[0].owner_process_id = 201),
            1 => fixture
                .source
                .update_current(|current| current.topology.displays[0].backing_scale = 2.1),
            2 => fixture.source.advance_generation(),
            3 => boundary.store(true, Ordering::Release),
            _ => job.store(true, Ordering::Release),
        }
        resume_tx.send(()).unwrap();
        assert!(worker.join().unwrap().is_err(), "case {case}");
    }
}

#[test]
fn publication_rechecks_fresh_evidence_session_and_both_original_flags() {
    for case in 0..6 {
        let fixture = fixture().unwrap();
        let (_selector, commit) = select(&fixture, 1);
        let boundary = flag();
        let job = flag();
        let (_, _, acquisition) = fixture.source.request(&commit, boundary.clone()).unwrap();
        let (_, publication) = acquisition.run(job.clone()).unwrap();
        assert!(publication.is_current());
        match case {
            0 => fixture
                .source
                .update_current(|current| current.observations[0].frame.size.width += 0.000_001),
            1 => fixture.source.update_current(|current| {
                current.observations[0].owner_bundle_id = Some("other.bundle".into())
            }),
            2 => fixture.source.advance_generation(),
            3 => boundary.store(true, Ordering::Release),
            4 => job.store(true, Ordering::Release),
            _ => fixture
                .source
                .update_current(|current| current.topology.displays[0].display.pixels.width += 1),
        }
        assert!(!publication.is_current(), "case {case}");
    }
}

#[test]
fn all_displays_participate_and_topology_order_is_not_identity() {
    let mut fixture = fixture().unwrap();
    let mut supplied = evidence(&fixture.source);
    let mut secondary = supplied.topology.displays[0];
    secondary.display.id = 2;
    secondary.display.bounds.origin.x = -800.;
    supplied.topology.displays.push(secondary);
    fixture.source = rebuild(
        &fixture,
        supplied,
        fixture
            .source
            .supplied
            .as_ref()
            .unwrap()
            .submissions
            .clone(),
    )
    .unwrap();
    let (_selector, commit) = select(&fixture, 1);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    let (_, publication) = acquisition.run(flag()).unwrap();
    fixture
        .source
        .update_current(|current| current.topology.displays.reverse());
    assert!(publication.is_current());
    fixture.source.update_current(|current| {
        current.topology.displays[0].display.bounds.origin.x -= 0.000_001
    });
    assert!(!publication.is_current());
}

#[test]
fn cancellation_cannot_be_reset_with_a_fresh_operation_flag() {
    let fixture = fixture().unwrap();
    let (_selector, commit) = select(&fixture, 1);
    let boundary = flag();
    let (_, _, acquisition) = fixture.source.request(&commit, boundary.clone()).unwrap();
    boundary.store(true, Ordering::Release);
    assert!(
        acquisition
            .with_generator(|_, _, _| panic!("cancelled source reached generator"))
            .run(flag())
            .is_err()
    );
    assert!(fixture.source.request(&commit, boundary).is_err());
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    let job = Arc::new(AtomicBool::new(true));
    assert!(
        acquisition
            .with_generator(|_, _, _| panic!("cancelled operation reached generator"))
            .run(job)
            .is_err()
    );
}

#[test]
fn generator_failure_wrong_size_and_nonbase_document_are_rejected() {
    let fixture = fixture().unwrap();
    let (_selector, commit) = select(&fixture, 1);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    assert!(
        acquisition
            .with_generator(|_, _, _| Err("supplied acquisition failure".into()))
            .run(flag())
            .is_err()
    );
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    assert!(
        acquisition
            .with_generator(
                |_, _, cancellation| synthetic_independent_window(2, cancellation)
                    .map_err(|error| error.to_string())
            )
            .run(flag())
            .is_err()
    );
    for crop in [false, true] {
        let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
        assert!(
            acquisition
                .with_generator(move |_, _, cancellation| {
                    let image = synthetic_independent_window(1, cancellation)
                        .map_err(|error| error.to_string())?;
                    let mut session = ScreenshotEditSession::new(image);
                    if crop {
                        session.set_crop(Some(Rect::new(1., 1., 20., 20.)))?;
                    } else {
                        session.add_annotation(
                            AnnotationKind::Rectangle(Rect::new(1., 1., 20., 20.)),
                            AnnotationStyle::default(),
                        )?;
                    }
                    Ok(session.render_snapshot())
                })
                .run(flag())
                .is_err()
        );
    }
}

#[test]
fn fixture_source_generator_can_be_configured_before_coordinator_handoff() {
    let mut fixture = fixture().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    fixture.source = fixture.source.with_generator(move |_, _, _| {
        counter.fetch_add(1, Ordering::Relaxed);
        Err("controlled fixture failure".into())
    });
    let (_selector, commit) = select(&fixture, 1);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    assert!(acquisition.run(flag()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn request_rejects_foreign_commit_metadata_and_display_geometry() {
    let fixture = fixture().unwrap();
    for change_display in [false, true] {
        let mut display = geometry(fixture.layout);
        let mut candidates = fixture.source.candidates().unwrap();
        if change_display {
            display.cocoa_frame.width += 1.;
        } else {
            candidates[0].owner_process_id = 777;
        }
        let mut selector = FrozenWindowSession::new(display, &candidates, 999).unwrap();
        selector
            .accept_frozen(
                selector.freeze_token().unwrap(),
                fixture.document.clone(),
                display,
            )
            .unwrap();
        let point = Point::new(400., 120.);
        selector.begin_press(point, display).unwrap();
        let commit = selector.end_press(point, display).unwrap().unwrap();
        assert!(fixture.source.request(&commit, flag()).is_err());
    }
    let other = rebuild(
        &fixture,
        evidence(&fixture.source),
        fixture
            .source
            .supplied
            .as_ref()
            .unwrap()
            .submissions
            .clone(),
    )
    .unwrap();
    assert_ne!(fixture.source.token.session, other.token.session);
}

#[test]
fn native_catalog_boundary_is_explicitly_unavailable() {
    assert!(Source::from_backend(fixture_layout(), Arc::new(NativeBackend)).is_err());
}

#[test]
fn expired_completion_never_republishes_even_with_unchanged_evidence() {
    let fixture = fixture().unwrap();
    let (_selector, commit) = select(&fixture, 1);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    let (_, mut publication) = acquisition.run(flag()).unwrap();
    assert!(publication.is_current());
    publication.started =
        Instant::now() - publication.plan.options().timeout - Duration::from_millis(1);
    assert!(!publication.is_current());
}

#[test]
fn production_backend_reads_acquisition_and_publication_evidence_independently() {
    struct CountedBackend {
        supplied: Arc<fixtures::SuppliedBackend>,
        observations: Arc<AtomicUsize>,
        acquisitions: Arc<AtomicUsize>,
    }
    impl Backend for CountedBackend {
        fn check_available(&self) -> Result<(), String> {
            self.supplied.check_available()
        }
        fn identity_and_layers(&self) -> Result<(i32, OcclusionLayers), String> {
            self.supplied.identity_and_layers()
        }
        fn revision(&self) -> u64 {
            self.supplied.revision()
        }
        fn observe(&self, request: &WindowObservationRequest) -> Result<Evidence, String> {
            self.observations.fetch_add(1, Ordering::Relaxed);
            self.supplied.observe(request)
        }
        fn acquire(&self, request: &AcquisitionRequest) -> Result<Acquired, String> {
            self.acquisitions.fetch_add(1, Ordering::Relaxed);
            self.supplied.acquire(request)
        }
    }
    let mut fixture = fixture().unwrap();
    let supplied = fixture.source.supplied.as_ref().unwrap().clone();
    let observations = Arc::new(AtomicUsize::new(0));
    let acquisitions = Arc::new(AtomicUsize::new(0));
    fixture.source = Source::from_backend(
        fixture.layout,
        Arc::new(CountedBackend {
            supplied: supplied.clone(),
            observations: observations.clone(),
            acquisitions: acquisitions.clone(),
        }),
    )
    .unwrap();
    assert_eq!(observations.load(Ordering::Relaxed), 1);
    let (_selector, commit) = select(&fixture, 1);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    assert_eq!(observations.load(Ordering::Relaxed), 2);
    let (_, publication) = acquisition.run(flag()).unwrap();
    assert_eq!(acquisitions.load(Ordering::Relaxed), 1);
    assert_eq!(observations.load(Ordering::Relaxed), 3);
    assert!(publication.is_current());
    assert_eq!(observations.load(Ordering::Relaxed), 3);
    supplied.current.lock().unwrap().observations[0]
        .frame
        .origin
        .x += 0.000_001;
    supplied.revision.fetch_add(1, Ordering::Release);
    assert!(!publication.is_current());
    assert_eq!(observations.load(Ordering::Relaxed), 3);
}

#[test]
fn host_decodes_the_native_completion_contract_without_reconstructing_submission() {
    use bello_platform::native_capture::{NativeWindowCaptureSnapshot, WindowCaptureDiagnostics};
    let fixture = fixture().unwrap();
    let (_selector, commit) = select(&fixture, 1);
    let (_, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    let selected = acquisition.selection.clone();
    let mut request = AcquisitionRequest {
        selection: selected,
        options: WindowCaptureOptions::default(),
        session: fixture.source.session.clone(),
        cancellation: CaptureCancellation::default(),
        observation: observation_request(CaptureCancellation::default(), Instant::now()),
        cancellation_flag: flag(),
        layout: fixture.layout,
    };
    let (document, publication) = acquisition.run(flag()).unwrap();
    let plan = publication.plan.clone();
    let completed = Acquired::from_native(
        NativeWindowCaptureSnapshot {
            png: document.render_png().unwrap(),
            completion: plan.clone(),
            diagnostics: WindowCaptureDiagnostics {
                window_id: 1,
                owner_process_id: 101,
                output_size: plan.output_size(),
                includes_cursor: false,
                backend: "synthetic callback contract",
            },
        },
        &request,
    )
    .unwrap();
    assert!(Arc::ptr_eq(&completed.plan, &plan));
    assert_eq!(completed.document.dimensions(), (680, 540));
    assert_eq!(
        completed.plan.submitted_identity().owner_bundle_id,
        Some("example.synthetic.window1")
    );
    request.options.include_cursor = true;
    let rejected = Acquired::from_native(
        NativeWindowCaptureSnapshot {
            png: vec![0], // invalid PNG: binding must fail before the decoder is called.
            completion: plan.clone(),
            diagnostics: WindowCaptureDiagnostics {
                window_id: 1,
                owner_process_id: 101,
                output_size: plan.output_size(),
                includes_cursor: false,
                backend: "synthetic callback contract",
            },
        },
        &request,
    );
    assert_eq!(
        rejected.err().unwrap(),
        "Window completion does not belong to the acquisition request."
    );
    fixture
        .source
        .update_current(|current| current.observations[0].frame.origin.x += 0.000_001);
    assert!(!publication.is_current());
}

#[test]
fn pending_request_does_not_observe_or_delay_frozen_materialization() {
    let mut fixture = fixture().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = calls.clone();
    fixture.source = fixture.source.with_observer(move |_| {
        worker_calls.fetch_add(1, Ordering::Relaxed);
        Err("controlled observation failure".into())
    });
    let (_selector, commit) = select(&fixture, 1);
    let (_, acquisition) = fixture.source.pending_request(&commit, flag()).unwrap();
    let frozen = commit.materialize(&AtomicBool::new(false)).unwrap();
    assert_eq!(frozen.editor.document().dimensions(), (680, 540));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(acquisition.decision(flag()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn late_success_and_cancelled_native_like_observation_never_create_evidence() {
    struct LateBackend {
        evidence: Evidence,
        calls: AtomicUsize,
        cancel: bool,
    }
    impl Backend for LateBackend {
        fn check_available(&self) -> Result<(), String> {
            Ok(())
        }
        fn identity_and_layers(&self) -> Result<(i32, OcclusionLayers), String> {
            unreachable!()
        }
        fn revision(&self) -> u64 {
            0
        }
        fn observe(&self, request: &WindowObservationRequest) -> Result<Evidence, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.cancel {
                request.cancellation().cancel();
            } else {
                std::thread::sleep(
                    request.deadline().saturating_duration_since(Instant::now())
                        + Duration::from_millis(2),
                );
            }
            // Simulate an opaque native API that returns success after logical
            // timeout/cancellation. The common host boundary MUST reject this.
            Ok(self.evidence.clone())
        }
        fn acquire(&self, _: &AcquisitionRequest) -> Result<Acquired, String> {
            unreachable!()
        }
    }
    for cancel in [false, true] {
        let fixture = fixture().unwrap();
        let backend = LateBackend {
            evidence: evidence(&fixture.source),
            calls: AtomicUsize::new(0),
            cancel,
        };
        let request = WindowObservationRequest::new(
            CaptureCancellation::default(),
            Instant::now() + Duration::from_millis(20),
        );
        assert!(observe_backend(&backend, &request).is_err());
        assert_eq!(backend.calls.load(Ordering::Relaxed), 1);
        assert!(observe_backend(&backend, &request).is_err());
        assert_eq!(
            backend.calls.load(Ordering::Relaxed),
            1,
            "expired/cancelled precheck invokes zero additional native operations"
        );
    }
}

#[test]
fn final_worker_observation_after_mask_rejects_changed_exact_metadata() {
    let fixture = fixture().unwrap();
    let (_selector, commit) = select(&fixture, 1);
    let (context, _, acquisition) = fixture.source.request(&commit, flag()).unwrap();
    let session = commit.materialize(&AtomicBool::new(false)).unwrap().editor;
    let plan = bellobox_core::screenshot::window_refresh::WindowRefreshPlan::new(
        &session,
        context,
        WindowRefreshDecision::KeepFrozen,
        flag(),
    )
    .unwrap()
    .with_decision(WindowRefreshDecision::MaskFrozenAlpha);
    let (image, publication) = acquisition.run(flag()).unwrap();
    let prepared = plan.prepare(image).unwrap().unwrap();
    fixture
        .source
        .update_current(|e| e.observations[0].frame.origin.x += 0.000_001);
    assert!(publication.observe_after_mask().is_err());
    drop(prepared);
    assert_eq!(session.revision(), 0);
    assert_eq!(
        session.document().render_rgba().unwrap().get_pixel(0, 0).0[3],
        255
    );
}

#[test]
fn platform_raw_transport_keeps_partial_target_order_and_exact_identity_separate() {
    use bello_platform::native_capture::{OwnedWindowObservation, RawWindowOcclusionRow};
    let base = fixture().unwrap();
    let strict = evidence(&base.source);
    let make = |rows| {
        evidence_from_native(WindowCatalogSnapshot {
            observations: strict
                .observations
                .iter()
                .map(|row| OwnedWindowObservation {
                    window_id: row.window_id,
                    owner_process_id: row.owner_process_id,
                    owner_bundle_id: None,
                    frame: row.frame,
                    layer: row.layer,
                    alpha: row.alpha,
                    on_screen: row.on_screen,
                })
                .collect(),
            main_display_id: strict.topology.main_display_id,
            displays: strict.topology.displays.clone(),
            occlusion_rows: rows,
        })
    };
    let own = RawWindowOcclusionRow {
        window_id: Some(77),
        owner_process_id: Some(999),
        layer: Some(0),
        alpha: Some(1.),
        frame: Some(CaptureRect::new(300., 80., 10., 10.)),
    };
    let target = RawWindowOcclusionRow {
        window_id: Some(1),
        ..Default::default()
    };
    for (rows, expected) in [
        (None, WindowRefreshDecision::MaskFrozenAlpha),
        (Some(vec![]), WindowRefreshDecision::MaskFrozenAlpha),
        (
            Some(vec![target, own]),
            WindowRefreshDecision::MaskFrozenAlpha,
        ),
        (
            Some(vec![RawWindowOcclusionRow::default(), own, target]),
            WindowRefreshDecision::ReplaceWithIndependent,
        ),
        (
            Some(vec![
                RawWindowOcclusionRow {
                    layer: Some(1000),
                    ..own
                },
                target,
            ]),
            WindowRefreshDecision::MaskFrozenAlpha,
        ),
        (
            Some(vec![
                RawWindowOcclusionRow {
                    frame: Some(CaptureRect::new(638.01, 80., 10., 10.)),
                    ..own
                },
                target,
            ]),
            WindowRefreshDecision::MaskFrozenAlpha,
        ),
        (
            Some(vec![
                RawWindowOcclusionRow {
                    frame: Some(CaptureRect::new(638., 80., 10., 10.)),
                    ..own
                },
                target,
            ]),
            WindowRefreshDecision::ReplaceWithIndependent,
        ),
    ] {
        let supplied = make(rows.clone());
        assert_eq!(
            supplied.occlusion_rows.as_ref().map(Vec::len),
            rows.as_ref().map(Vec::len)
        );
        let source = rebuild(
            &base,
            supplied,
            base.source.supplied.as_ref().unwrap().submissions.clone(),
        )
        .unwrap();
        let fixture = Fixture {
            source,
            layout: base.layout,
            document: base.document.clone(),
        };
        let (_selector, commit) = select(&fixture, 1);
        let (_, acquisition) = fixture.source.pending_request(&commit, flag()).unwrap();
        assert_eq!(acquisition.decision(flag()).unwrap(), expected);
    }
}

#[test]
fn supplied_platform_level_values_survive_app_adaptation_without_constants() {
    let levels = bello_platform::native_capture::WindowCatalogLayers {
        normal: 0,
        floating: 37,
        modal_panel: 38,
        main_menu: 39,
        status: 40,
        popup_menu: 41,
        screen_saver: 42,
    };
    let adapted = layers_from_native(levels);
    assert_eq!(
        [
            adapted.normal,
            adapted.floating,
            adapted.modal_panel,
            adapted.main_menu,
            adapted.status,
            adapted.popup_menu,
            adapted.screen_saver
        ],
        [0, 37, 38, 39, 40, 41, 42]
    );
    for (layer, pid, expected) in [
        (37, 300, true),
        (3, 300, false),
        (0, 999, true),
        (42, 999, false),
    ] {
        let occluded = bellobox_core::screenshot::window_refresh::is_occluded(
            1,
            Rect::new(10., 10., 80., 60.),
            Some(&[OcclusionRow {
                window_id: Some(2),
                owner_process_id: Some(pid),
                layer: Some(layer),
                alpha: Some(1.),
                frame: Some(Rect::new(10., 10., 80., 60.)),
            }]),
            999,
            adapted,
        )
        .unwrap();
        assert_eq!(occluded, expected);
    }
}
