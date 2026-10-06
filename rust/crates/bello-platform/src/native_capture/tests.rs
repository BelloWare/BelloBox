use super::{
    completion::{InflightGuard, Job, Stage},
    *,
};
use std::sync::atomic::AtomicUsize;
fn display(id: u32, x: f64, y: f64) -> CaptureDisplay {
    CaptureDisplay {
        id,
        bounds: CaptureRect::new(x, y, 1440., 900.),
        pixels: CapturePixelSize {
            width: 2880,
            height: 1800,
        },
    }
}
#[test]
fn full_display_defaults_preserve_native_size_no_cursor() {
    let request = CaptureRequest::full_display(display(1, -1440., 900.));
    assert!(request.validate().is_ok());
    assert!(!request.include_cursor);
    assert!(request.region.is_none());
    assert_eq!(request.output_size, request.display.pixels);
}
#[test]
fn region_is_local_top_left_points_and_must_not_escape_display() {
    let mut request = CaptureRequest::full_display(display(5, -1440., -900.));
    request.region = Some(CaptureRect::new(10., 20., 300., 200.));
    request.output_size = CapturePixelSize {
        width: 600,
        height: 400,
    };
    assert!(request.validate().is_ok());
    for region in [
        CaptureRect::new(-1., 20., 300., 200.),
        CaptureRect::new(1400., 0., 80., 100.),
        CaptureRect::new(0., 0., f64::NAN, 4.),
        CaptureRect::new(0., 0., 0., 4.),
    ] {
        request.region = Some(region);
        assert_eq!(request.validate(), Err(CaptureError::InvalidRequest));
    }
}
#[test]
fn invalid_request_fails_without_any_platform_action() {
    let mut request = CaptureRequest::full_display(display(1, 0., 0.));
    request.timeout = Duration::ZERO;
    assert!(matches!(
        capture(request, CaptureCancellation::default()),
        Err(CaptureError::InvalidRequest)
    ));
    let mut request = CaptureRequest::full_display(display(1, 0., 0.));
    request.output_size.width = 1;
    assert_eq!(request.validate(), Err(CaptureError::InvalidRequest));
    let cancellation = CaptureCancellation::default();
    cancellation.cancel();
    assert!(matches!(
        capture(
            CaptureRequest::full_display(display(1, 0., 0.)),
            cancellation
        ),
        Err(CaptureError::Cancelled)
    ));
}
#[test]
fn dimension_and_pixel_budget_checks_cannot_overflow() {
    for size in [
        CapturePixelSize {
            width: 0,
            height: 1,
        },
        CapturePixelSize {
            width: u32::MAX,
            height: u32::MAX,
        },
        CapturePixelSize {
            width: 10_000,
            height: 10_000,
        },
    ] {
        assert_eq!(size.validate(), Err(CaptureError::OutputTooLarge));
    }
    assert!(CapturePixelSize {
        width: 8000,
        height: 8000
    }
    .validate()
    .is_ok());
}
#[test]
fn resolver_matches_swift_precedence_without_a_main_display_fallback() {
    let desired = display(7, -1440., 0.);
    let request = CaptureRequest::full_display(desired);
    assert_eq!(
        resolve_display(&request, &[desired], None).unwrap().path,
        DisplayResolutionPath::InitialId
    );
    assert_eq!(
        resolve_display(&request, &[], Some(&[desired]))
            .unwrap()
            .path,
        DisplayResolutionPath::RefreshedId
    );
    let replacement = CaptureDisplay { id: 8, ..desired };
    assert_eq!(
        resolve_display(&request, &[], Some(&[replacement]))
            .unwrap()
            .path,
        DisplayResolutionPath::RefreshedBounds
    );
    assert_eq!(
        resolve_display(&request, &[replacement], Some(&[]))
            .unwrap()
            .path,
        DisplayResolutionPath::InitialBounds
    );
    assert_eq!(
        resolve_display(&request, &[display(1, 0., 0.)], None),
        Err(CaptureError::DisplayNotFound)
    );
}
#[test]
fn stale_resolution_or_geometry_fails_closed() {
    let desired = display(7, -1440., 0.);
    let request = CaptureRequest::full_display(desired);
    let moved = display(7, 0., 0.);
    assert_eq!(
        resolve_display(&request, &[moved], None),
        Err(CaptureError::DisplayChanged)
    );
    let changed = CaptureDisplay {
        pixels: CapturePixelSize {
            width: 1440,
            height: 900,
        },
        ..desired
    };
    assert_eq!(
        resolve_display(&request, &[changed], None),
        Err(CaptureError::DisplayChanged)
    );
    assert_eq!(
        resolve_display(&request, &vec![desired; 65], None),
        Err(CaptureError::InvalidRequest)
    );
}
#[test]
fn refreshed_identity_precedes_initial_bounds_match() {
    let desired = display(7, 0., 0.);
    let request = CaptureRequest::full_display(desired);
    let initial = CaptureDisplay { id: 3, ..desired };
    let resolution = resolve_display(&request, &[initial], Some(&[desired])).unwrap();
    assert_eq!(resolution.path, DisplayResolutionPath::RefreshedId);
    assert_eq!(resolution.display.id, 7);
}
#[test]
fn two_point_bounds_tolerance_does_not_allow_unrelated_display() {
    let request = CaptureRequest::full_display(display(7, 0., 0.));
    assert!(resolve_display(&request, &[display(8, 2., -2.)], None).is_ok());
    assert_eq!(
        resolve_display(&request, &[display(8, 3., 0.)], None),
        Err(CaptureError::DisplayNotFound)
    );
}
#[derive(Debug)]
struct Payload(Arc<AtomicUsize>);
impl Drop for Payload {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}
#[test]
fn completion_is_once_and_duplicate_payloads_are_released() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let job = Job::new(CaptureCancellation::default(), Duration::from_secs(1));
    assert!(job.complete(Ok(Payload(dropped.clone()))));
    assert!(!job.complete(Ok(Payload(dropped.clone()))));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    drop(job.wait().unwrap());
    assert_eq!(dropped.load(Ordering::Relaxed), 2);
}
#[test]
fn cancellation_wins_even_after_native_success_before_publication() {
    let cancellation = CaptureCancellation::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    let job = Job::new(cancellation.clone(), Duration::from_secs(1));
    assert!(job.complete(Ok(Payload(dropped.clone()))));
    cancellation.cancel();
    assert!(matches!(job.wait(), Err(CaptureError::Cancelled)));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(!job.complete(Ok(Payload(dropped.clone()))));
    assert_eq!(dropped.load(Ordering::Relaxed), 2);
}
#[test]
fn timeout_abandons_job_without_dangling_callback_state() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let job = Job::new(CaptureCancellation::default(), Duration::ZERO);
    let callback = job.clone();
    assert!(matches!(job.wait(), Err(CaptureError::TimedOut)));
    drop(job);
    assert!(!callback.complete(Ok(Payload(dropped.clone()))));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
}
#[test]
fn callback_stage_claims_reject_duplicate_and_out_of_order_callbacks() {
    let job: Arc<Job<()>> = Job::new(CaptureCancellation::default(), Duration::from_secs(1));
    assert!(!job.transition(Stage::Image, Stage::Encoding));
    assert!(job.transition(Stage::InitialContent, Stage::ResolvingInitial));
    assert!(!job.transition(Stage::InitialContent, Stage::ResolvingInitial));
    assert!(job.transition(Stage::ResolvingInitial, Stage::RefreshedContent));
    assert!(job.transition(Stage::RefreshedContent, Stage::ResolvingRefreshed));
    assert!(job.transition(Stage::ResolvingRefreshed, Stage::Image));
    assert!(job.transition(Stage::Image, Stage::EncodingQueued));
    assert!(!job.transition(Stage::Image, Stage::EncodingQueued));
    assert!(job.transition(Stage::EncodingQueued, Stage::Encoding));
    assert!(!job.transition(Stage::EncodingQueued, Stage::Encoding));
    assert!(job.complete(Ok(())));
    assert!(!job.active());
}
#[test]
fn separate_jobs_never_share_cancellation_or_results() {
    let a_cancel = CaptureCancellation::default();
    let a: Arc<Job<u32>> = Job::new(a_cancel.clone(), Duration::from_secs(1));
    let b = Job::new(CaptureCancellation::default(), Duration::from_secs(1));
    a_cancel.cancel();
    assert!(!a.complete(Ok(1)));
    assert!(b.complete(Ok(2)));
    assert_eq!(a.wait(), Err(CaptureError::Cancelled));
    assert_eq!(b.wait(), Ok(2));
}
#[test]
fn result_debug_and_errors_never_include_image_or_native_text() {
    let snapshot = NativeCaptureSnapshot {
        png: b"SECRET IMAGE PAYLOAD".to_vec(),
        diagnostics: CaptureDiagnostics {
            requested_display_id: 1,
            resolved_display_id: 1,
            resolution_path: DisplayResolutionPath::InitialId,
            output_size: CapturePixelSize {
                width: 100,
                height: 100,
            },
            region: None,
            includes_cursor: false,
            backend: "test",
        },
    };
    assert!(!format!("{snapshot:?}").contains("SECRET"));
    assert!(!CaptureError::NativeFailure.to_string().contains("NSError"));
}
#[test]
#[cfg(not(target_os = "macos"))]
fn unsupported_platform_has_no_implicit_capture_route() {
    assert!(!is_available());
    assert_eq!(main_display(), Err(CaptureError::Unsupported));
    assert!(matches!(
        capture(
            CaptureRequest::full_display(display(1, 0., 0.)),
            CaptureCancellation::default()
        ),
        Err(CaptureError::Unsupported)
    ));
}

#[test]
fn region_sampling_never_includes_cursor_but_display_choice_is_explicit() {
    let mut request = CaptureRequest::full_display(display(1, 0., 0.));
    request.include_cursor = true;
    assert!(request.validate().is_ok());
    request.region = Some(CaptureRect::new(10., 20., 300., 200.));
    request.output_size = CapturePixelSize {
        width: 600,
        height: 400,
    };
    assert_eq!(request.validate(), Err(CaptureError::InvalidRequest));
    request.include_cursor = false;
    assert!(request.validate().is_ok());
}

// A portable ownership model uses the production Job/InflightGuard. It does not
// purport to execute libdispatch or prove CGImage retain/release on Linux.
struct QueuedImageDrop {
    slot: &'static AtomicBool,
    dropped: Arc<AtomicUsize>,
}
impl Drop for QueuedImageDrop {
    fn drop(&mut self) {
        assert!(
            self.slot.load(Ordering::Acquire),
            "image must drop before lease"
        );
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}
struct QueuedEncodeModel {
    _image: QueuedImageDrop,
    job: Arc<Job<Payload>>,
    _lease: Arc<InflightGuard>,
}
fn queued_model(
    slot: &'static AtomicBool,
    cancellation: CaptureCancellation,
) -> (Arc<InflightGuard>, Box<QueuedEncodeModel>, Arc<AtomicUsize>) {
    let lease = InflightGuard::acquire(slot).unwrap();
    let job = Job::new(cancellation, Duration::from_secs(5));
    assert!(job.transition(Stage::InitialContent, Stage::Image));
    assert!(job.transition(Stage::Image, Stage::EncodingQueued));
    let dropped = Arc::new(AtomicUsize::new(0));
    let queued = Box::new(QueuedEncodeModel {
        _image: QueuedImageDrop {
            slot,
            dropped: dropped.clone(),
        },
        job,
        _lease: lease.clone(),
    });
    (lease, queued, dropped)
}
#[test]
fn queued_cancellation_keeps_slot_until_context_drops_after_callback() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let cancellation = CaptureCancellation::default();
    let (callback, queued, dropped) = queued_model(&SLOT, cancellation.clone());
    let waiter = queued.job.clone();
    cancellation.cancel();
    assert!(matches!(waiter.wait(), Err(CaptureError::Cancelled)));
    drop(waiter);
    drop(callback);
    assert!(matches!(
        InflightGuard::acquire(&SLOT),
        Err(CaptureError::Busy)
    ));
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
    assert!(!queued
        .job
        .transition(Stage::EncodingQueued, Stage::Encoding));
    drop(queued);
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn queued_timeout_does_not_reclaim_context_or_let_another_operation_start() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let lease = InflightGuard::acquire(&SLOT).unwrap();
    let job = Job::new(CaptureCancellation::default(), Duration::from_millis(10));
    assert!(job.transition(Stage::InitialContent, Stage::Image));
    assert!(job.transition(Stage::Image, Stage::EncodingQueued));
    let dropped = Arc::new(AtomicUsize::new(0));
    let queued = Box::new(QueuedEncodeModel {
        _image: QueuedImageDrop {
            slot: &SLOT,
            dropped: dropped.clone(),
        },
        job: job.clone(),
        _lease: lease.clone(),
    });
    // Model the exclusively queue-owned Box while the waiter knows only Job.
    let context = Box::into_raw(queued);
    assert!(matches!(job.wait(), Err(CaptureError::TimedOut)));
    drop(job);
    drop(lease);
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
    assert!(matches!(
        InflightGuard::acquire(&SLOT),
        Err(CaptureError::Busy)
    ));
    // Exactly once, by the simulated worker, never by the timed-out waiter.
    let queued = unsafe { Box::from_raw(context) };
    assert!(!queued
        .job
        .transition(Stage::EncodingQueued, Stage::Encoding));
    drop(queued);
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn callback_lease_keeps_slot_after_worker_success_and_image_drop() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let (callback, queued, dropped) = queued_model(&SLOT, CaptureCancellation::default());
    assert!(queued
        .job
        .transition(Stage::EncodingQueued, Stage::Encoding));
    let payload_drops = Arc::new(AtomicUsize::new(0));
    assert!(queued.job.complete(Ok(Payload(payload_drops.clone()))));
    drop(queued.job.wait().unwrap());
    drop(queued);
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert_eq!(payload_drops.load(Ordering::Relaxed), 1);
    assert!(matches!(
        InflightGuard::acquire(&SLOT),
        Err(CaptureError::Busy)
    ));
    drop(callback);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn queued_duplicate_callback_cannot_create_a_second_image_owner() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let (callback, queued, dropped) = queued_model(&SLOT, CaptureCancellation::default());
    assert!(!queued.job.transition(Stage::Image, Stage::EncodingQueued));
    assert_eq!(Arc::strong_count(&callback), 2);
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
    drop(callback);
    drop(queued);
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn encode_panic_releases_image_before_last_lease() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let (callback, queued, dropped) = queued_model(&SLOT, CaptureCancellation::default());
    drop(callback);
    let result = std::panic::catch_unwind(move || {
        let _queued = queued;
        panic!("synthetic encode panic");
    });
    assert!(result.is_err());
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn pre_submission_failure_drops_local_image_and_preserves_callback_lease() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let (callback, queued, dropped) = queued_model(&SLOT, CaptureCancellation::default());
    assert!(queued.job.complete(Err(CaptureError::NativeFailure)));
    assert!(matches!(
        queued.job.wait(),
        Err(CaptureError::NativeFailure)
    ));
    drop(queued);
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(matches!(
        InflightGuard::acquire(&SLOT),
        Err(CaptureError::Busy)
    ));
    drop(callback);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn cancelled_image_callback_cannot_claim_queue_phase() {
    let cancellation = CaptureCancellation::default();
    let job: Arc<Job<()>> = Job::new(cancellation.clone(), Duration::from_secs(1));
    assert!(job.transition(Stage::InitialContent, Stage::Image));
    cancellation.cancel();
    assert!(!job.transition(Stage::Image, Stage::EncodingQueued));
}
