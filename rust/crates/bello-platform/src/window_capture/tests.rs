use super::*;

const TOKEN: WindowSelectionToken = WindowSelectionToken {
    session: 17,
    generation: 3,
};
fn observed() -> WindowObservation<'static> {
    WindowObservation {
        identity: WindowIdentity {
            window_id: 41,
            owner_process_id: 200,
            owner_bundle_id: None,
        },
        frame: WindowFrame(CaptureRect::new(100., 120., 500., 320.)),
        layer: 0,
        alpha: 1.,
        on_screen: true,
    }
}
fn display() -> WindowDisplayGeometry {
    WindowDisplayGeometry {
        display: CaptureDisplay {
            id: 1,
            bounds: CaptureRect::new(0., 0., 1440., 900.),
            pixels: CapturePixelSize {
                width: 1440,
                height: 1800,
            },
        },
        appkit_size_points: CaptureSize {
            width: 1440.,
            height: 900.,
        },
        backing_scale: 2.,
        rotation_degrees: 0.,
    }
}
fn topology(displays: &[WindowDisplayGeometry]) -> WindowTopology<'_> {
    WindowTopology {
        main_display_id: 1,
        displays,
    }
}
fn source(value: WindowObservation<'_>) -> SubmittedWindowSource<'_> {
    SubmittedWindowSource {
        identity: value.identity,
        frame: value.frame,
        layer: value.layer,
        on_screen: value.on_screen,
    }
}
fn select(
    value: WindowObservation<'_>,
    displays: &[WindowDisplayGeometry],
) -> WindowPolicyResult<WindowCaptureSelection> {
    WindowCaptureSelection::new(
        value,
        topology(displays),
        100,
        TOKEN,
        &CaptureCancellation::default(),
    )
}

struct Fixture {
    selection: WindowCaptureSelection,
    submission: SubmittedWindowSource<'static>,
    fresh: Vec<WindowObservation<'static>>,
    displays: Vec<WindowDisplayGeometry>,
    main_id: u32,
    token: WindowSelectionToken,
    options: WindowCaptureOptions,
    selection_cancel: CaptureCancellation,
    job_cancel: CaptureCancellation,
}
impl Fixture {
    fn new() -> Self {
        let displays = vec![display()];
        let selection_cancel = CaptureCancellation::default();
        let selection = WindowCaptureSelection::new(
            observed(),
            topology(&displays),
            100,
            TOKEN,
            &selection_cancel,
        )
        .unwrap();
        Self {
            selection,
            submission: source(observed()),
            fresh: vec![observed()],
            displays,
            main_id: 1,
            token: TOKEN,
            options: WindowCaptureOptions::default(),
            selection_cancel,
            job_cancel: CaptureCancellation::default(),
        }
    }
    fn topology(&self) -> WindowTopology<'_> {
        WindowTopology {
            main_display_id: self.main_id,
            displays: &self.displays,
        }
    }
    fn plan(&self) -> WindowPolicyResult<WindowCapturePlan> {
        self.selection.plan(
            self.submission,
            &self.fresh,
            self.topology(),
            self.token,
            self.options,
            &self.job_cancel,
        )
    }
    fn complete(&self, plan: &WindowCapturePlan) -> WindowPolicyResult<()> {
        self.complete_size(plan, plan.output_size())
    }
    fn complete_size(
        &self,
        plan: &WindowCapturePlan,
        size: CapturePixelSize,
    ) -> WindowPolicyResult<()> {
        // Deliberately a NEW boundary flag: stored selection/job flags must still
        // prevent a cancelled object from being revived by this caller.
        plan.validate_completion(
            &self.fresh,
            self.topology(),
            self.token,
            size,
            &CaptureCancellation::default(),
        )
    }
}

#[test]
fn independent_window_defaults_and_explicit_cursor_are_preserved() {
    let mut f = Fixture::new();
    let plan = f.plan().unwrap();
    assert!(!plan.options().include_cursor);
    assert_eq!(plan.options().timeout, MAX_CAPTURE_TIMEOUT);
    assert!(f.complete(&plan).is_ok());
    f.options.include_cursor = true;
    assert!(f.plan().unwrap().options().include_cursor);
}

#[test]
fn sizing_uses_maximum_cg_or_backing_ratio_independently_per_axis() {
    let mut d = display();
    d.display.pixels.height = 2700;
    let selected = select(observed(), &[d]).unwrap();
    let plan = selected
        .plan(
            source(observed()),
            &[observed()],
            topology(&[d]),
            TOKEN,
            WindowCaptureOptions::default(),
            &CaptureCancellation::default(),
        )
        .unwrap();
    assert_eq!(
        plan.output_size(),
        CapturePixelSize {
            width: 1000,
            height: 960
        }
    );
}

#[test]
fn sizing_rounds_only_after_scaling_the_window_points() {
    let mut value = observed();
    value.frame.0.size = CaptureSize {
        width: 100.25,
        height: 40.75,
    };
    let selected = select(value, &[display()]).unwrap();
    let plan = selected
        .plan(
            source(value),
            &[value],
            topology(&[display()]),
            TOKEN,
            WindowCaptureOptions::default(),
            &CaptureCancellation::default(),
        )
        .unwrap();
    assert_eq!(
        plan.output_size(),
        CapturePixelSize {
            width: 201,
            height: 82
        }
    );
}

#[test]
fn active_catalog_floored_dimensions_and_area_replace_legacy_list_threshold() {
    for (width, height, allowed) in [
        (8., 12., true),
        (12., 8., true),
        (8.9, 11.99, false),
        (7.99, 100., false),
        (8., 8., false),
        (20., 20., true),
    ] {
        let mut value = observed();
        value.frame.0.size = CaptureSize { width, height };
        assert_eq!(
            select(value, &[display()]).is_ok(),
            allowed,
            "{width} x {height}"
        );
    }
}

#[test]
fn alpha_threshold_is_strict_with_nonfinite_and_out_of_range_values_rejected() {
    let mut value = observed();
    for alpha in [0., 0.01] {
        value.alpha = alpha;
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::IneligibleWindow)
        ));
    }
    value.alpha = 0.010001;
    assert!(select(value, &[display()]).is_ok());
    for alpha in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        value.alpha = alpha;
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::InvalidMetadata)
        ));
    }
}

#[test]
fn own_offscreen_and_non_normal_windows_are_ineligible() {
    let mut own = observed();
    own.identity.owner_process_id = 100;
    let mut hidden = observed();
    hidden.on_screen = false;
    for value in [own, hidden] {
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::IneligibleWindow)
        ));
    }
    for layer in [-1, 3, 8, 24, 25, 101, 1000] {
        let mut value = observed();
        value.layer = layer;
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::IneligibleWindow)
        ));
    }
}

#[test]
fn invalid_identity_session_and_own_pid_are_rejected() {
    for (id, pid) in [(0, 200), (41, 0), (41, -1)] {
        let mut value = observed();
        value.identity.window_id = id;
        value.identity.owner_process_id = pid;
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::InvalidMetadata)
        ));
    }
    for own_pid in [0, -1] {
        assert!(matches!(
            WindowCaptureSelection::new(
                observed(),
                topology(&[display()]),
                own_pid,
                TOKEN,
                &CaptureCancellation::default()
            ),
            Err(WindowPolicyError::InvalidMetadata)
        ));
    }
    assert!(matches!(
        WindowCaptureSelection::new(
            observed(),
            topology(&[display()]),
            100,
            WindowSelectionToken {
                session: 0,
                ..TOKEN
            },
            &CaptureCancellation::default()
        ),
        Err(WindowPolicyError::InvalidMetadata)
    ));
}

#[test]
fn borrowed_bundle_limits_use_utf8_bytes_before_owned_copy() {
    let excessive = "x".repeat(MAX_WINDOW_IDENTITY_BYTES + 1);
    let multibyte = "é".repeat(MAX_WINDOW_IDENTITY_BYTES);
    for bundle in [
        "",
        "bad\0bundle",
        "bad\nbundle",
        excessive.as_str(),
        multibyte.as_str(),
    ] {
        let mut value = observed();
        value.identity.owner_bundle_id = Some(bundle);
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::InvalidMetadata)
        ));
    }
    let bounded = "a".repeat(MAX_WINDOW_IDENTITY_BYTES);
    let mut value = observed();
    value.identity.owner_bundle_id = Some(&bounded);
    assert!(select(value, &[display()]).is_ok());
}

#[test]
fn debug_never_discloses_native_bundle_text() {
    let mut value = observed();
    value.identity.owner_bundle_id = Some("private.identity.example");
    let selected = select(value, &[display()]).unwrap();
    let plan = selected
        .plan(
            source(value),
            &[value],
            topology(&[display()]),
            TOKEN,
            WindowCaptureOptions::default(),
            &CaptureCancellation::default(),
        )
        .unwrap();
    assert!(!format!("{value:?} {selected:?} {plan:?}").contains("private.identity.example"));
}

#[test]
fn raw_cg_bundle_none_and_submission_some_are_compatible_and_bound() {
    let mut f = Fixture::new();
    f.submission.identity.owner_bundle_id = Some("submission.bundle");
    let plan = f.plan().unwrap();
    assert_eq!(
        plan.submitted_identity().owner_bundle_id,
        Some("submission.bundle")
    );
    assert_eq!(f.complete(&plan), Ok(()));
    f.fresh[0].identity.owner_bundle_id = Some("different.bundle");
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowChanged));
    f.fresh[0].identity.owner_bundle_id = None;
    assert_eq!(f.complete(&plan), Ok(()));
    assert_eq!(
        plan.submitted_identity().owner_bundle_id,
        Some("submission.bundle")
    );
}

#[test]
fn known_selected_bundle_must_match_callback_local_submission() {
    let mut value = observed();
    value.identity.owner_bundle_id = Some("selected.bundle");
    let selected = select(value, &[display()]).unwrap();
    for bundle in [None, Some("another.bundle")] {
        let mut submission = source(value);
        submission.identity.owner_bundle_id = bundle;
        assert!(matches!(
            selected.plan(
                submission,
                &[observed()],
                topology(&[display()]),
                TOKEN,
                WindowCaptureOptions::default(),
                &CaptureCancellation::default()
            ),
            Err(WindowPolicyError::WindowChanged)
        ));
    }
}

#[test]
fn fresh_bundle_must_match_submission_evidence_even_when_selection_bundle_is_absent() {
    let mut f = Fixture::new();
    f.fresh[0].identity.owner_bundle_id = Some("fresh.bundle");
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
    f.submission.identity.owner_bundle_id = Some("other.bundle");
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
    f.submission.identity.owner_bundle_id = Some("fresh.bundle");
    let plan = f.plan().unwrap();
    f.fresh[0].identity.owner_bundle_id = Some("new.bundle");
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowChanged));
}

#[test]
fn completion_rejects_fresh_bundle_not_evidenced_at_submission() {
    let mut f = Fixture::new();
    let plan = f.plan().unwrap();
    f.fresh[0].identity.owner_bundle_id = Some("late.bundle");
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowChanged));
}

#[test]
fn completion_uses_owned_submission_evidence_after_callback_local_data_is_gone() {
    let f = Fixture::new();
    let plan = {
        let bundle = String::from("callback.local.bundle");
        let mut submission = source(observed());
        submission.identity.owner_bundle_id = Some(&bundle);
        f.selection
            .plan(
                submission,
                &f.fresh,
                f.topology(),
                f.token,
                f.options,
                &f.job_cancel,
            )
            .unwrap()
        // The callback-local borrowed metadata is gone after this block. There
        // is no native-source argument to fake rereading at completion.
    };
    assert_eq!(
        plan.submitted_identity().owner_bundle_id,
        Some("callback.local.bundle")
    );
    assert_eq!(f.complete(&plan), Ok(()));
}

#[test]
fn mutable_caller_source_copy_does_not_replace_immutable_submission_evidence() {
    let mut f = Fixture::new();
    f.submission.identity.owner_bundle_id = Some("bound.bundle");
    let plan = f.plan().unwrap();
    // An unrelated mutable copy is not a native-object reread. Completion only
    // checks supplied fresh CG observations against the stored submission.
    f.submission.identity.owner_bundle_id = Some("other.bundle");
    f.submission.identity.owner_process_id += 1;
    f.submission.frame.0.size.width += 1.;
    f.submission.on_screen = false;
    assert_eq!(
        plan.submitted_identity().owner_bundle_id,
        Some("bound.bundle")
    );
    assert_eq!(f.complete(&plan), Ok(()));
    f.fresh[0].identity.owner_process_id += 1;
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowChanged));
}

#[test]
fn missing_window_and_same_bounds_other_id_never_choose_a_substitute() {
    let mut f = Fixture::new();
    let plan = f.plan().unwrap();
    f.fresh[0].identity.window_id += 1;
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowNotFound)));
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowNotFound));
    f.fresh.clear();
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowNotFound)));
}

#[test]
fn reused_id_different_pid_and_replaced_submission_identity_fail_closed() {
    let mut f = Fixture::new();
    let plan = f.plan().unwrap();
    f.fresh[0].identity.owner_process_id += 1;
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowChanged));
    f.fresh[0] = observed();
    f.submission.identity.owner_process_id += 1;
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
    f.submission = source(observed());
    f.submission.identity.window_id += 1;
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
}

#[test]
fn duplicate_selected_id_and_oversized_catalog_fail_closed() {
    let mut f = Fixture::new();
    f.fresh.push(observed());
    assert!(matches!(f.plan(), Err(WindowPolicyError::AmbiguousWindow)));
    f.fresh = vec![observed(); MAX_WINDOW_CANDIDATES + 1];
    assert!(matches!(f.plan(), Err(WindowPolicyError::InvalidMetadata)));
}

#[test]
fn move_resize_and_subpoint_changes_require_reselection() {
    for resize in [false, true] {
        let mut f = Fixture::new();
        let plan = f.plan().unwrap();
        if resize {
            f.fresh[0].frame.0.size.width += 0.001;
        } else {
            f.fresh[0].frame.0.origin.x += 0.001;
        }
        assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
        assert_eq!(f.complete(&plan), Err(WindowPolicyError::WindowChanged));
    }
    let mut f = Fixture::new();
    f.submission.frame.0.size.height += 1.;
    assert!(matches!(f.plan(), Err(WindowPolicyError::WindowChanged)));
}

#[test]
fn fresh_cg_eligibility_is_rechecked_after_submission() {
    for field in 0..3 {
        let mut f = Fixture::new();
        let plan = f.plan().unwrap();
        match field {
            0 => f.fresh[0].alpha = 0.01,
            1 => f.fresh[0].layer = 3,
            _ => f.fresh[0].on_screen = false,
        }
        assert!(matches!(f.plan(), Err(WindowPolicyError::IneligibleWindow)));
        assert_eq!(f.complete(&plan), Err(WindowPolicyError::IneligibleWindow));
    }
}

#[test]
fn submission_layer_and_visibility_are_checked_before_filter_binding() {
    for hidden in [false, true] {
        let mut f = Fixture::new();
        if hidden {
            f.submission.on_screen = false;
        } else {
            f.submission.layer = 3;
        }
        assert!(matches!(f.plan(), Err(WindowPolicyError::IneligibleWindow)));
    }
}

#[test]
fn selected_identity_and_cancellation_accessors_only_expose_immutable_policy_state() {
    let f = Fixture::new();
    assert_eq!(f.selection.selected_identity(), observed().identity);
    assert!(!f.selection.is_cancelled());
    f.selection_cancel.cancel();
    assert!(f.selection.is_cancelled());
    assert_eq!(f.selection.selected_identity(), observed().identity);
}

#[test]
fn invalid_window_geometry_is_rejected_before_size_conversion() {
    for number in [f64::NAN, f64::INFINITY, -1., 0., 1_000_001.] {
        let mut value = observed();
        value.frame.0.size.width = number;
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::InvalidGeometry)
        ));
    }
    for number in [f64::NAN, f64::NEG_INFINITY, -1_000_001., 1_000_001.] {
        let mut value = observed();
        value.frame.0.origin.y = number;
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::InvalidGeometry)
        ));
    }
}

#[test]
fn initial_scope_requires_whole_window_on_unrotated_main_display() {
    for frame in [
        CaptureRect::new(-1., 20., 100., 100.),
        CaptureRect::new(1400., 20., 100., 100.),
        CaptureRect::new(20., 850., 100., 100.),
    ] {
        let mut value = observed();
        value.frame = WindowFrame(frame);
        assert!(matches!(
            select(value, &[display()]),
            Err(WindowPolicyError::UnsupportedGeometry)
        ));
    }
    let mut d = display();
    d.rotation_degrees = 90.;
    assert!(matches!(
        select(observed(), &[d]),
        Err(WindowPolicyError::UnsupportedGeometry)
    ));
    d = display();
    d.display.bounds.origin.x = -1440.;
    assert!(matches!(
        select(observed(), &[d]),
        Err(WindowPolicyError::UnsupportedGeometry)
    ));
    d = display();
    d.appkit_size_points.width += 1.;
    assert!(matches!(
        select(observed(), &[d]),
        Err(WindowPolicyError::UnsupportedGeometry)
    ));
}

#[test]
fn topology_values_counts_and_ids_are_validated_before_copy() {
    assert!(select(observed(), &[]).is_err());
    assert!(select(observed(), &[display(), display()]).is_err());
    assert!(select(observed(), &vec![display(); MAX_TOPOLOGY_DISPLAYS + 1]).is_err());
    for field in 0..6 {
        let mut d = display();
        match field {
            0 => d.backing_scale = f64::NAN,
            1 => d.backing_scale = 17.,
            2 => d.rotation_degrees = f64::INFINITY,
            3 => d.appkit_size_points.width = 0.,
            4 => d.display.id = 0,
            _ => d.display.pixels.width = u32::MAX,
        }
        assert!(matches!(
            select(observed(), &[d]),
            Err(WindowPolicyError::InvalidGeometry)
        ));
    }
    assert!(matches!(
        WindowCaptureSelection::new(
            observed(),
            WindowTopology {
                main_display_id: 9,
                displays: &[display()]
            },
            100,
            TOKEN,
            &CaptureCancellation::default()
        ),
        Err(WindowPolicyError::InvalidGeometry)
    ));
}

#[test]
fn all_topology_changes_including_secondary_or_backing_scale_are_stale() {
    for field in 0..4 {
        let mut f = Fixture::new();
        let plan = f.plan().unwrap();
        match field {
            0 => f.displays[0].backing_scale = 3.,
            1 => f.displays[0].display.pixels.width += 1,
            2 => {
                let mut added = display();
                added.display.id = 2;
                added.display.bounds.origin.x = 1440.;
                f.displays.push(added);
            }
            _ => {
                f.displays[0].display.id = 2;
                f.main_id = 2;
            }
        }
        assert!(matches!(f.plan(), Err(WindowPolicyError::TopologyChanged)));
        assert_eq!(f.complete(&plan), Err(WindowPolicyError::TopologyChanged));
    }
}

#[test]
fn topology_order_is_not_identity_and_negative_secondary_origins_are_preserved() {
    let mut secondary = display();
    secondary.display.id = 2;
    secondary.display.bounds.origin.x = -1440.;
    let displays = [display(), secondary];
    let selected = select(observed(), &displays).unwrap();
    assert!(selected
        .plan(
            source(observed()),
            &[observed()],
            topology(&[secondary, display()]),
            TOKEN,
            WindowCaptureOptions::default(),
            &CaptureCancellation::default()
        )
        .is_ok());
}

#[test]
fn stale_generation_and_reopened_session_reject_plan_and_completion() {
    for token in [
        WindowSelectionToken {
            session: 18,
            ..TOKEN
        },
        WindowSelectionToken {
            generation: 4,
            ..TOKEN
        },
    ] {
        let mut f = Fixture::new();
        let plan = f.plan().unwrap();
        f.token = token;
        assert!(matches!(f.plan(), Err(WindowPolicyError::StaleSelection)));
        assert_eq!(f.complete(&plan), Err(WindowPolicyError::StaleSelection));
    }
}

#[test]
fn cancellation_wins_at_selection_and_boundary_before_other_errors() {
    let flag = CaptureCancellation::default();
    flag.cancel();
    assert!(matches!(
        WindowCaptureSelection::new(observed(), topology(&[]), 0, TOKEN, &flag),
        Err(WindowPolicyError::Cancelled)
    ));
    let f = Fixture::new();
    assert!(matches!(
        f.selection
            .plan(f.submission, &[], f.topology(), TOKEN, f.options, &flag),
        Err(WindowPolicyError::Cancelled)
    ));
    let plan = f.plan().unwrap();
    assert_eq!(
        plan.validate_completion(&[], f.topology(), TOKEN, plan.output_size(), &flag),
        Err(WindowPolicyError::Cancelled)
    );
}

#[test]
fn canceled_selection_cannot_be_revived_with_a_fresh_job_flag() {
    let mut f = Fixture::new();
    let plan = f.plan().unwrap();
    f.selection_cancel.cancel();
    assert!(!f.job_cancel.is_cancelled());
    f.job_cancel = CaptureCancellation::default();
    assert!(matches!(f.plan(), Err(WindowPolicyError::Cancelled)));
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::Cancelled));
}

#[test]
fn canceled_plan_keeps_its_own_flag_when_new_requests_have_new_flags() {
    let mut f = Fixture::new();
    let plan = f.plan().unwrap();
    f.job_cancel.cancel();
    assert!(!f.selection_cancel.is_cancelled());
    f.job_cancel = CaptureCancellation::default();
    assert_eq!(f.complete(&plan), Err(WindowPolicyError::Cancelled));
    let new_plan = f.plan().unwrap();
    assert_eq!(f.complete(&new_plan), Ok(()));
}

#[test]
fn timeout_option_is_bounded_without_claiming_a_native_timer() {
    let mut f = Fixture::new();
    for timeout in [
        Duration::ZERO,
        MAX_CAPTURE_TIMEOUT + Duration::from_nanos(1),
    ] {
        f.options.timeout = timeout;
        assert!(matches!(f.plan(), Err(WindowPolicyError::InvalidOptions)));
    }
}

#[test]
fn scaled_output_and_actual_dimensions_use_existing_capture_budgets() {
    let mut d = display();
    d.backing_scale = 16.;
    let mut value = observed();
    value.frame = WindowFrame(CaptureRect::new(0., 0., 1440., 900.));
    assert!(matches!(
        select(value, &[d]),
        Err(WindowPolicyError::OutputTooLarge)
    ));
    let f = Fixture::new();
    let plan = f.plan().unwrap();
    assert_eq!(
        f.complete_size(
            &plan,
            CapturePixelSize {
                width: 999,
                height: 640
            }
        ),
        Err(WindowPolicyError::UnexpectedImageSize)
    );
    for size in [
        CapturePixelSize {
            width: 0,
            height: 640,
        },
        CapturePixelSize {
            width: u32::MAX,
            height: 640,
        },
        CapturePixelSize {
            width: 8001,
            height: 8001,
        },
    ] {
        assert_eq!(
            f.complete_size(&plan, size),
            Err(WindowPolicyError::OutputTooLarge)
        );
    }
}
