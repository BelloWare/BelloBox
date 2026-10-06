//! Native-only synthetic tests. No SCK call, display capture, TCC request, GUI,
//! file, clipboard, or network is used. Rust tests do not establish main-thread
//! rejection: the harness generally runs tests on non-main threads.
use super::*;
use std::time::Instant;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> Id;
    fn CGColorSpaceRelease(space: *const c_void);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: *const c_void,
        bitmap_info: u32,
    ) -> Id;
    fn CGContextRelease(context: *const c_void);
    fn CGContextSetRGBFillColor(context: Id, red: f64, green: f64, blue: f64, alpha: f64);
    fn CGContextFillRect(context: Id, rect: CaptureRect);
    fn CGBitmapContextCreateImage(context: Id) -> Id;
}
struct SyntheticImage(Id);
impl Drop for SyntheticImage {
    fn drop(&mut self) {
        unsafe { CGImageRelease(self.0) };
    }
}
unsafe fn synthetic_image() -> SyntheticImage {
    let space = CGColorSpaceCreateDeviceRGB();
    assert!(!space.is_null());
    // kCGImageAlphaPremultipliedLast; 2x2 RGBA, no captured pixels.
    let bitmap = CGBitmapContextCreate(ptr::null_mut(), 2, 2, 8, 8, space, 1);
    CGColorSpaceRelease(space);
    assert!(!bitmap.is_null());
    CGContextSetRGBFillColor(bitmap, 0.25, 0.5, 0.75, 1.);
    CGContextFillRect(bitmap, CaptureRect::new(0., 0., 2., 2.));
    let image = CGBitmapContextCreateImage(bitmap);
    CGContextRelease(bitmap);
    assert!(!image.is_null());
    SyntheticImage(image)
}
fn no_display_request() -> (CaptureRequest, DisplayResolution) {
    // id=0 fails on the worker before any CoreGraphics display query. This tests
    // the real retained-image dispatch path without relying on a GUI/session.
    let display = CaptureDisplay {
        id: 0,
        bounds: CaptureRect::new(0., 0., 2., 2.),
        pixels: CapturePixelSize {
            width: 2,
            height: 2,
        },
    };
    (
        CaptureRequest::full_display(display),
        DisplayResolution {
            display,
            path: DisplayResolutionPath::InitialId,
        },
    )
}
fn await_slot_release(slot: &'static AtomicBool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(lease) = InflightGuard::acquire(slot) {
            drop(lease);
            return;
        }
        assert!(
            Instant::now() < deadline,
            "queued context did not release its lease"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn retained_synthetic_image_encodes_after_original_owner_drops_on_worker() {
    std::thread::spawn(|| unsafe {
        assert!(require_worker_thread().is_ok());
        let _pool = pool().unwrap();
        let image = synthetic_image();
        let retained = OwnedCaptureImage::retain(image.0).unwrap();
        drop(image);
        let job = Job::new(CaptureCancellation::default(), Duration::from_secs(5));
        let png = encode_png(retained.0, job).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(&png[16..20], &2u32.to_be_bytes());
        assert_eq!(&png[20..24], &2u32.to_be_bytes());
        assert!(png.len() <= MAX_CAPTURE_PNG_BYTES);
    })
    .join()
    .unwrap();
}
#[test]
fn real_dispatch_handoff_releases_context_and_rejects_duplicate_callback() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let callback_lease = InflightGuard::acquire(&SLOT).unwrap();
    let job = Job::new(CaptureCancellation::default(), Duration::from_secs(5));
    assert!(job.transition(Stage::InitialContent, Stage::Image));
    let (request, resolution) = no_display_request();
    unsafe {
        let _pool = pool().unwrap();
        let image = synthetic_image();
        enqueue_image(
            image.0,
            ptr::null_mut(),
            resolution,
            &request,
            &job,
            &callback_lease,
        )
        .unwrap();
        // Null would fail if duplicate invocation could pass the phase claim.
        enqueue_image(
            ptr::null_mut(),
            ptr::null_mut(),
            resolution,
            &request,
            &job,
            &callback_lease,
        )
        .unwrap();
        drop(image); // the dispatched context owns its independent native retain
    }
    // DisplayNotFound, rather than WorkerThreadRequired, establishes execution
    // off the real main thread without invoking any display/capture API.
    assert!(matches!(job.wait(), Err(CaptureError::DisplayNotFound)));
    assert!(matches!(
        InflightGuard::acquire(&SLOT),
        Err(CaptureError::Busy)
    ));
    drop(callback_lease);
    await_slot_release(&SLOT);
}
#[test]
fn cancelled_callback_skips_retain_and_dispatch_entirely() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let lease = InflightGuard::acquire(&SLOT).unwrap();
    let cancellation = CaptureCancellation::default();
    let job = Job::new(cancellation.clone(), Duration::from_secs(5));
    assert!(job.transition(Stage::InitialContent, Stage::Image));
    cancellation.cancel();
    let (request, resolution) = no_display_request();
    unsafe {
        enqueue_image(
            ptr::null_mut(),
            ptr::null_mut(),
            resolution,
            &request,
            &job,
            &lease,
        )
        .unwrap();
    }
    assert_eq!(Arc::strong_count(&lease), 1);
    assert!(matches!(job.wait(), Err(CaptureError::Cancelled)));
    drop(lease);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn callback_error_does_not_allocate_worker_context() {
    static SLOT: AtomicBool = AtomicBool::new(false);
    let lease = InflightGuard::acquire(&SLOT).unwrap();
    let job = Job::new(CaptureCancellation::default(), Duration::from_secs(5));
    assert!(job.transition(Stage::InitialContent, Stage::Image));
    let (request, resolution) = no_display_request();
    unsafe {
        assert_eq!(
            enqueue_image(
                ptr::null_mut(),
                ptr::null_mut(),
                resolution,
                &request,
                &job,
                &lease
            ),
            Err(CaptureError::NativeFailure)
        );
    }
    assert_eq!(Arc::strong_count(&lease), 1);
    drop(lease);
    assert!(InflightGuard::acquire(&SLOT).is_ok());
}
#[test]
fn png_sink_checks_byte_bound_and_cancellation_before_reading_bytes() {
    let cancellation = CaptureCancellation::default();
    let state = Arc::new(Mutex::new(PngState {
        bytes: Vec::new(),
        failed: false,
    }));
    let mut sink = PngSink {
        state: state.clone(),
        job: Job::new(cancellation.clone(), Duration::from_secs(5)),
    };
    let info = (&mut sink as *mut PngSink).cast();
    let byte = 0u8;
    unsafe {
        // Count intentionally exceeds the source buffer: cap check must precede
        // any slice construction/read. No large allocation is needed for this test.
        assert_eq!(
            put_bytes(info, (&byte as *const u8).cast(), MAX_CAPTURE_PNG_BYTES + 1),
            0
        );
    }
    assert!(state.lock().unwrap().failed);
    assert!(state.lock().unwrap().bytes.is_empty());
    state.lock().unwrap().failed = false;
    cancellation.cancel();
    unsafe {
        assert_eq!(put_bytes(info, (&byte as *const u8).cast(), 1), 0);
    }
    assert!(state.lock().unwrap().bytes.is_empty());
}
