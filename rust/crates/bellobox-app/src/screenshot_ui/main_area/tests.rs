use super::*;
use capture::{
    CaptureDiagnostics, CaptureDisplay, CapturePixelSize, CaptureRect, CaptureRequest, CaptureSize,
    DisplayResolutionPath, NativeCaptureSnapshot,
};
use std::{cell::Cell, rc::Rc};

fn layout() -> host::MainDisplayOverlayLayout {
    host::MainDisplayOverlayLayout {
        display: CaptureDisplay {
            id: 7,
            bounds: CaptureRect::new(0., 0., 650., 400.),
            pixels: CapturePixelSize {
                width: 1300,
                height: 800,
            },
        },
        cocoa_frame: CaptureRect::new(0., 0., 650., 400.),
        rotation_degrees: 0,
        backing_scale: 2.,
    }
}
fn snapshot(request: CaptureRequest) -> NativeCaptureSnapshot {
    use bellobox_core::screenshot::scroll::synthetic_page_frame;
    NativeCaptureSnapshot {
        png: synthetic_page_frame(
            0,
            request.output_size.width,
            request.output_size.height,
            1500,
        )
        .unwrap()
        .png()
        .unwrap(),
        diagnostics: CaptureDiagnostics {
            requested_display_id: request.display.id,
            resolved_display_id: request.display.id,
            resolution_path: DisplayResolutionPath::InitialId,
            output_size: request.output_size,
            region: request.region,
            includes_cursor: false,
            backend: "in-memory test fixture",
        },
    }
}
#[test]
fn single_freeze_prepares_tiles_then_mouse_up_reuses_clean_full_pixels() {
    use bellobox_core::screenshot::Point;
    let count = Cell::new(0);
    let display = layout();
    let prepared = prepare_area_with(display, Arc::new(AtomicBool::new(false)), |request, _| {
        count.set(count.get() + 1);
        assert_eq!(request.display, display.display);
        assert!(request.region.is_none());
        Ok(snapshot(request))
    })
    .unwrap();
    assert_eq!(count.get(), 1);
    assert!(prepared.tiles.len() >= 2);
    assert!(
        prepared
            .tiles
            .iter()
            .all(|t| t.width <= 1024 && t.height <= 1024)
    );
    let mut area = match prepared.selection {
        selector::Selection::Area(area) => area,
        _ => panic!("Area preparation"),
    };
    assert!(area.freeze_token().is_none());
    area.begin_drag(Point::new(10., 20.), geometry(display))
        .unwrap();
    area.update_drag(Point::new(130., 140.), geometry(display))
        .unwrap();
    let selected = area
        .end_drag(Point::new(110., 120.), geometry(display))
        .unwrap()
        .unwrap();
    assert_eq!(selected.editor.document().dimensions(), (1300, 800));
    assert_eq!(selected.editor.document().visible_dimensions(), (200, 200));
    assert!(!selected.editor.has_edits());
    assert!(!selected.editor.can_undo());
    assert_eq!(count.get(), 1);
}
#[test]
fn cancellation_before_worker_capture_produces_no_selector_pixels() {
    assert!(
        prepare_area_with(layout(), Arc::new(AtomicBool::new(true)), |_, _| {
            panic!("cancelled job must not call capture")
        })
        .is_err()
    );
}
#[test]
fn changed_native_target_never_recaptures_or_silently_switches_displays() {
    let count = Cell::new(0);
    let result = prepare_area_with(layout(), Arc::new(AtomicBool::new(false)), |request, _| {
        count.set(count.get() + 1);
        let mut result = snapshot(request);
        result.diagnostics.resolved_display_id += 1;
        Ok(result)
    });
    assert!(result.is_err());
    assert_eq!(count.get(), 1);
}
#[test]
fn viewport_actual_scale_and_exact_primary_display_are_all_required() {
    let display = layout();
    let viewport = CaptureSize {
        width: 650.,
        height: 400.,
    };
    assert!(validate_viewport(display, viewport, 2., Some(7)).is_ok());
    assert!(
        validate_viewport(
            display,
            CaptureSize {
                width: 649.,
                ..viewport
            },
            2.,
            Some(7)
        )
        .is_err()
    );
    assert!(validate_viewport(display, viewport, 1., Some(7)).is_err());
    assert!(validate_viewport(display, viewport, f64::NAN, Some(7)).is_err());
    assert!(validate_viewport(display, viewport, 2., Some(8)).is_err());
    assert!(validate_viewport(display, viewport, 2., None).is_err());
}
#[test]
fn source_area_delay_and_production_gate_are_explicit() {
    assert_eq!(SNAPSHOT_DELAY, std::time::Duration::from_millis(60));
    const { assert!(!PRODUCTION_AREA_ENABLED) };
}

struct DropProbe(Rc<Cell<usize>>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn observed_run() -> (ActiveCapture<u64, DropProbe>, Rc<Cell<usize>>) {
    let drops = Rc::new(Cell::new(0));
    (
        ActiveCapture {
            source: HostSource::Native,
            selection_locked: false,
            transaction: AreaTransaction::new(17, 5, 10, vec![10]),
            layout: layout(),
            deactivation: Some(DropProbe(drops.clone())),
        },
        drops,
    )
}
#[test]
fn deactivation_is_accepted_after_callback_already_cancelled_the_flag() {
    let (mut run, drops) = observed_run();
    run.transaction.cancellation.store(true, Ordering::Release);
    assert!(run.accepts_deactivation(17, 5, 5, host::ApplicationDeactivationEvent::ResignedActive));
    drop(run.cancel());
    assert_eq!(drops.get(), 1);
    assert!(run.transaction.cancelled());
    assert!(!run.accepts_deactivation(
        17,
        5,
        5,
        host::ApplicationDeactivationEvent::ResignedActive
    ));
}
#[test]
fn removed_and_old_observer_signals_cannot_cancel_a_successor() {
    let (run, drops) = observed_run();
    assert!(!run.accepts_deactivation(
        17,
        5,
        5,
        host::ApplicationDeactivationEvent::ObserverRemoved
    ));
    assert!(!run.accepts_deactivation(
        16,
        5,
        5,
        host::ApplicationDeactivationEvent::ResignedActive
    ));
    assert!(!run.accepts_deactivation(
        17,
        4,
        5,
        host::ApplicationDeactivationEvent::ResignedActive
    ));
    assert!(!run.accepts_deactivation(
        17,
        5,
        6,
        host::ApplicationDeactivationEvent::ResignedActive
    ));
    assert!(!run.transaction.cancelled());
    assert_eq!(drops.get(), 0);
}
#[test]
fn cancel_unregisters_once_while_slow_worker_still_holds_busy() {
    let (mut run, drops) = observed_run();
    run.transaction.start_worker();
    run.transaction.selector = Some(99);
    let guard = run.cancel();
    assert_eq!(drops.get(), 0);
    drop(guard); // Mirrors outside-global-borrow native destruction.
    assert_eq!(drops.get(), 1);
    run.transaction.restore.clear();
    run.transaction.selector = None;
    assert!(!run.transaction.can_release());
    drop(run.cancel());
    assert_eq!(drops.get(), 1);
    run.transaction.finish_worker();
    assert!(run.transaction.can_release());
}
#[test]
fn selection_lock_unregisters_without_releasing_inline_ownership() {
    let (mut run, drops) = observed_run();
    run.transaction.start_worker();
    assert!(run.accepts_deactivation(17, 5, 5, host::ApplicationDeactivationEvent::ResignedActive));
    assert_eq!(drops.get(), 0);
    run.transaction.selector = Some(99);
    drop(run.lock_selection());
    assert_eq!(drops.get(), 1);
    assert!(!run.transaction.cancelled());
    assert!(!run.accepts_deactivation(
        17,
        5,
        5,
        host::ApplicationDeactivationEvent::ResignedActive
    ));
    run.transaction.restore.clear();
    assert!(!run.transaction.can_release());
    run.transaction.finish_worker();
    assert!(!run.transaction.can_release());
    run.transaction.cancel();
    run.transaction.selector = None;
    assert!(run.transaction.can_release());
}
#[test]
fn terminal_owner_releases_guard_without_requester_entity_access() {
    let (run, drops) = observed_run();
    drop(run);
    assert_eq!(drops.get(), 1);
}

#[test]
fn separate_production_gates_precede_every_admission_side_effect() {
    // All read/acquire/presentation work is inside the continuation used by the
    // real entry points. A false gate must not invoke any of those operations.
    for intent in [CaptureIntent::Area, CaptureIntent::Window] {
        let backend = Cell::new(0);
        let catalog = Cell::new(0);
        let permission = Cell::new(0);
        let capture = Cell::new(0);
        let clipboard = Cell::new(0);
        let presentation = Cell::new(0);
        assert!(
            intent
                .admitted(|| {
                    backend.set(backend.get() + 1);
                    catalog.set(catalog.get() + 1);
                    permission.set(permission.get() + 1);
                    capture.set(capture.get() + 1);
                    clipboard.set(clipboard.get() + 1);
                    presentation.set(presentation.get() + 1);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(
            [
                backend.get(),
                catalog.get(),
                permission.get(),
                capture.get(),
                clipboard.get(),
                presentation.get()
            ],
            [0; 6]
        );
    }
}

#[gpui::test]
fn production_window_entry_does_not_construct_backend_or_mutate_coordinator(
    cx: &mut gpui::TestAppContext,
) {
    let requester = cx.add_window(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus);
        CaptureChooser {
            busy: false,
            status: String::new(),
            jobs: Default::default(),
            focus,
        }
    });
    let before = cx.read(|cx| cx.windows());
    requester
        .update(cx, |_, window, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                "unchanged gate sentinel".into(),
            ));
            assert_eq!(
                begin_window_with(window, cx, || panic!("closed gate constructed backend")),
                Err("Window · main display only is awaiting native review.".into())
            );
            assert_eq!(
                begin(window, cx),
                Err("Area · main display only is awaiting native review.".into())
            );
            assert!(cx.try_global::<Coordinator>().is_none());
            assert!(cx.try_global::<NativeCaptureVisibility>().is_none());
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("unchanged gate sentinel".into())
            );
        })
        .unwrap();
    cx.read(|cx| assert!(cx.windows() == before));
}

#[test]
fn unavailable_native_capability_precedes_hide_and_freeze_even_inside_admission() {
    let hide = Cell::new(0);
    let freeze = Cell::new(0);
    let capture = Cell::new(0);
    let result = with_window_capability(
        || Arc::new(super::super::window_workflow::NativeBackend),
        |_| {
            hide.set(hide.get() + 1);
            freeze.set(freeze.get() + 1);
            capture.set(capture.get() + 1);
            Ok(())
        },
    );
    assert_eq!(
        result,
        Err("Independent window capture is not enabled.".into())
    );
    assert_eq!([hide.get(), freeze.get(), capture.get()], [0, 0, 0]);
}

#[gpui::test]
fn pending_supplied_preparation_refuses_quit_and_failure_clears_its_blocker(
    cx: &mut gpui::TestAppContext,
) {
    struct Root;
    impl gpui::Render for Root {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }
    cx.update(crate::shutdown::init);
    let requester = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        Root
    });
    cx.update(|cx| {
        begin_supplied_prepared(requester.into(), layout(), cx, |_| {
            Err("injected preparation failure".into())
        })
        .unwrap();
        assert!(cx.global::<Coordinator>().quit_blocker.is_some());
        cx.dispatch_action(&crate::shutdown::Quit);
    });
    assert!(!cx.read(crate::shutdown::requested));
    assert!(crate::shutdown::pending_refusal(cx).is_some());
    cx.run_until_parked();
    assert!(cx.read(|cx| cx.global::<Coordinator>().active.is_none()));
    assert!(cx.read(|cx| cx.global::<Coordinator>().quit_blocker.is_none()));
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    assert!(!cx.read(crate::shutdown::requested));
    cx.dispatch_action(requester.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn pending_supplied_cancel_keeps_quit_refused_until_preparation_retires(
    cx: &mut gpui::TestAppContext,
) {
    struct Root;
    impl gpui::Render for Root {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }
    cx.update(crate::shutdown::init);
    let requester = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        Root
    });
    cx.update(|cx| {
        begin_supplied_prepared(requester.into(), layout(), cx, |cancelled| {
            assert!(cancelled.load(Ordering::Acquire));
            Err("cancelled preparation".into())
        })
        .unwrap();
        let id = cx
            .global::<Coordinator>()
            .active
            .as_ref()
            .unwrap()
            .transaction
            .id;
        cancel(id, cx);
        assert!(cx.global::<Coordinator>().quit_blocker.is_some());
        cx.dispatch_action(&crate::shutdown::Quit);
    });
    assert!(!cx.read(crate::shutdown::requested));
    cx.run_until_parked();
    assert!(cx.read(|cx| cx.global::<Coordinator>().active.is_none()));
    assert!(cx.read(|cx| cx.global::<Coordinator>().quit_blocker.is_none()));
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    cx.dispatch_action(requester.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}
