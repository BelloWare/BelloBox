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
    let mut prepared =
        prepare_area_with(display, Arc::new(AtomicBool::new(false)), |request, _| {
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
    assert!(prepared.area.freeze_token().is_none());
    prepared
        .area
        .begin_drag(Point::new(10., 20.), geometry(display))
        .unwrap();
    prepared
        .area
        .update_drag(Point::new(130., 140.), geometry(display))
        .unwrap();
    let selected = prepared
        .area
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
fn pending_editor_handoff_is_observed_until_accepted_terminal_transfer() {
    let (mut run, drops) = observed_run();
    run.transaction.start_worker();
    assert!(run.accepts_deactivation(17, 5, 5, host::ApplicationDeactivationEvent::ResignedActive));
    assert_eq!(drops.get(), 0);
    drop(run.finish());
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
    assert!(run.transaction.can_release());
}
#[test]
fn terminal_owner_releases_guard_without_requester_entity_access() {
    let (run, drops) = observed_run();
    drop(run);
    assert_eq!(drops.get(), 1);
}
