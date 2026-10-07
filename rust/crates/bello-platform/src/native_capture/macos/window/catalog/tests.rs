//! Constructed CF rows only. Never enumerate NSScreen, SCK or live CG windows.
use super::*;
use crate::native_capture::macos::window::tests::{array, dictionary, integer, string};
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGRectCreateDictionaryRepresentation(rect: CaptureRect) -> Id;
}
unsafe fn floating(value: f64) -> Id {
    send!(class(b"NSNumber\0").unwrap(), b"numberWithDouble:\0", (f64 => value) -> Id)
}

#[test]
fn raw_catalog_preserves_missing_fields_order_none_and_empty() {
    unsafe {
        let _pool = pool().unwrap();
        let bounds = OwnedCf::new(CGRectCreateDictionaryRepresentation(CaptureRect::new(
            10.000_000_1,
            20.,
            80.25,
            60.5,
        )))
        .unwrap();
        let keys = [
            kCGWindowNumber,
            kCGWindowOwnerPID,
            kCGWindowLayer,
            kCGWindowAlpha,
            kCGWindowBounds,
        ];
        let values = [
            integer(42),
            integer(75),
            integer(3),
            floating(0.010_000_01),
            bounds.0,
        ];
        let target_only = dictionary(&keys[..1], &values[..1]);
        let complete = dictionary(&keys, &values);
        let rows = raw_rows(
            array(&[target_only, dictionary(&[], &[]), complete]),
            || Ok(()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].window_id, Some(42));
        assert_eq!(rows[0].owner_process_id, None);
        assert_eq!(rows[1], RawWindowOcclusionRow::default());
        assert_eq!(rows[2].frame.unwrap().origin.x, 10.000_000_1);
        assert_eq!(rows[2].alpha, Some(0.010_000_01));
        assert!(observations(array(&[target_only])).is_err());
        assert!(raw_rows(ptr::null_mut(), || Ok(())).unwrap().is_none());
        assert_eq!(raw_rows(array(&[]), || Ok(())).unwrap(), Some(vec![]));
    }
}

#[test]
fn raw_wrong_types_numeric_overflow_and_limits_never_become_valid_identity() {
    unsafe {
        let _pool = pool().unwrap();
        let wrong = string("42");
        let keys = [
            kCGWindowNumber,
            kCGWindowOwnerPID,
            kCGWindowLayer,
            kCGWindowAlpha,
            kCGWindowBounds,
        ];
        let values = [
            wrong.ptr,
            integer(i64::MAX),
            wrong.ptr,
            wrong.ptr,
            integer(7),
        ];
        let invalid = dictionary(&keys, &values);
        let rows = raw_rows(array(&[invalid]), || Ok(())).unwrap().unwrap();
        assert_eq!(rows, [RawWindowOcclusionRow::default()]);
        let overflow = dictionary(&keys[..1], &[integer(i64::MAX)]);
        assert_eq!(
            raw_rows(array(&[overflow]), || Ok(())).unwrap().unwrap()[0].window_id,
            None
        );
        assert!(raw_rows(integer(1), || Ok(())).is_err());
        assert!(raw_rows(array(&[integer(1)]), || Ok(())).is_err());
        assert_eq!(
            raw_rows(array(&vec![invalid; MAX_WINDOW_CANDIDATES]), || Ok(()))
                .unwrap()
                .unwrap()
                .len(),
            MAX_WINDOW_CANDIDATES
        );
        assert!(raw_rows(array(&vec![invalid; MAX_WINDOW_CANDIDATES + 1]), || Ok(())).is_err());
    }
}

#[test]
fn strict_and_raw_owned_metadata_survive_cf_teardown() {
    let (strict, raw) = unsafe {
        let _pool = pool().unwrap();
        let bounds = OwnedCf::new(CGRectCreateDictionaryRepresentation(CaptureRect::new(
            10.000_000_1,
            20.,
            80.25,
            60.5,
        )))
        .unwrap();
        let keys = [
            kCGWindowNumber,
            kCGWindowOwnerPID,
            kCGWindowLayer,
            kCGWindowAlpha,
            kCGWindowBounds,
        ];
        let values = [
            integer(42),
            integer(75),
            integer(0),
            floating(0.375),
            bounds.0,
        ];
        let row = dictionary(&keys, &values);
        let native = array(&[row]);
        assert!(observations(array(&[row, row])).is_err());
        (
            observations(native).unwrap(),
            raw_rows(native, || Ok(())).unwrap().unwrap(),
        )
    };
    assert_eq!(strict[0].identity.owner_bundle_id, None);
    assert_eq!(strict[0].frame.0.origin.x, 10.000_000_1);
    assert_eq!(raw[0].frame, Some(strict[0].frame.0));
    assert_eq!(strict[0].alpha, 0.375);
}

#[test]
fn cancellation_is_checked_between_raw_and_strict_rows_and_after_last_row() {
    use std::cell::Cell;
    unsafe {
        let _pool = pool().unwrap();
        let row = dictionary(&[kCGWindowNumber], &[integer(42)]);
        let calls = Cell::new(0);
        let result = raw_rows(array(&[row; 4]), || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(CaptureError::Cancelled.into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(CaptureError::Cancelled.into()));
        assert_eq!(calls.get(), 3);
        let calls = Cell::new(0);
        let result = raw_rows(array(&[]), || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err(CaptureError::TimedOut.into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(CaptureError::TimedOut.into()));
        assert_eq!(calls.get(), 2);
    }
}

#[test]
fn queued_context_retains_admission_after_logical_timeout_until_fake_dispatch_drains() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let lease = InflightGuard::acquire(&SLOT).unwrap();
    let drain = lease.drain();
    let request =
        WindowObservationRequest::new(CaptureCancellation::default(), std::time::Instant::now());
    let job = CatalogJob::with_deadline(request.cancellation(), request.deadline());
    // The pointer follows exactly enqueue_context's ownership conversion, but
    // fake dispatch invokes on_main only after the logical waiter has timed out.
    let context = Box::into_raw(Box::new(MainContext::Catalog {
        request,
        job: job.clone(),
        _lease: lease,
    }))
    .cast();
    assert!(matches!(
        job.wait(),
        Err(WindowCaptureError::Native(CaptureError::TimedOut))
    ));
    assert!(SLOT.load(Ordering::Acquire));
    assert!(InflightGuard::acquire(&SLOT).is_err());
    unsafe {
        on_main(context);
    } // deadline rejects before any native read
    drain.wait();
    assert!(!SLOT.load(Ordering::Acquire));
}

#[test]
fn successful_fake_dispatch_completes_job_without_releasing_a_live_callback_lease() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let lease = InflightGuard::acquire(&SLOT).unwrap();
    let callback = lease.clone();
    let drain = lease.drain();
    let request = WindowObservationRequest::new(
        CaptureCancellation::default(),
        std::time::Instant::now() + Duration::from_secs(1),
    );
    let job = CatalogJob::with_deadline(request.cancellation(), request.deadline());
    let snapshot = WindowCatalogSnapshot {
        observations: vec![],
        main_display_id: 1,
        displays: vec![],
        occlusion_rows: Some(vec![]),
    };
    let context = Box::into_raw(Box::new(MainContext::SuppliedCatalog {
        request,
        snapshot: Some(snapshot),
        job: job.clone(),
        _lease: lease,
    }))
    .cast();
    unsafe {
        on_main(context);
    }
    assert!(job.wait().unwrap().occlusion_rows.unwrap().is_empty());
    assert!(
        SLOT.load(Ordering::Acquire),
        "Job.complete is not physical drain"
    );
    assert!(InflightGuard::acquire(&SLOT).is_err());
    drop(callback);
    drain.wait();
    assert!(!SLOT.load(Ordering::Acquire));
}
#[test]
fn cancelled_fake_dispatch_drops_owned_context_without_reading_native_catalog() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let lease = InflightGuard::acquire(&SLOT).unwrap();
    let drain = lease.drain();
    let cancellation = CaptureCancellation::default();
    let request = WindowObservationRequest::new(
        cancellation.clone(),
        std::time::Instant::now() + Duration::from_secs(1),
    );
    let job = CatalogJob::with_deadline(cancellation.clone(), request.deadline());
    let context = Box::into_raw(Box::new(MainContext::Catalog {
        request,
        job: job.clone(),
        _lease: lease,
    }))
    .cast();
    cancellation.cancel();
    assert!(matches!(
        job.wait(),
        Err(WindowCaptureError::Native(CaptureError::Cancelled))
    ));
    assert!(SLOT.load(Ordering::Acquire));
    unsafe {
        on_main(context);
    }
    drain.wait();
    assert!(!SLOT.load(Ordering::Acquire));
}

#[test]
fn strict_cf_catalog_preserves_fractional_geometry_and_enforces_exact_512_bound() {
    unsafe {
        let _pool = pool().unwrap();
        let bounds = OwnedCf::new(CGRectCreateDictionaryRepresentation(CaptureRect::new(
            0.000_000_1,
            20.,
            80.25,
            60.5,
        )))
        .unwrap();
        let keys = [
            kCGWindowNumber,
            kCGWindowOwnerPID,
            kCGWindowLayer,
            kCGWindowAlpha,
            kCGWindowBounds,
        ];
        let rows: Vec<_> = (1..=MAX_WINDOW_CANDIDATES)
            .map(|id| {
                dictionary(
                    &keys,
                    &[
                        integer(id as i64),
                        integer(75),
                        integer(0),
                        floating(0.125),
                        bounds.0,
                    ],
                )
            })
            .collect();
        let owned = observations(array(&rows)).unwrap();
        assert_eq!(owned.len(), MAX_WINDOW_CANDIDATES);
        assert_eq!(owned[511].identity.window_id, 512);
        assert_eq!(owned[0].frame.0.origin.x, 0.000_000_1);
        let fractional_id = dictionary(
            &keys,
            &[
                floating(42.25),
                integer(75),
                integer(0),
                floating(0.125),
                bounds.0,
            ],
        );
        assert!(observations(array(&[fractional_id])).is_err());
        assert_eq!(
            raw_rows(array(&[fractional_id]), || Ok(()))
                .unwrap()
                .unwrap()[0]
                .window_id,
            None
        );
        let mut too_many = rows;
        too_many.push(too_many[0]);
        assert!(observations(array(&too_many)).is_err());
    }
}
