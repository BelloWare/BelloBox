//! macOS 14+ one-shot bridge. No native object/pointer is sent through a channel.
//! Content callbacks chain capture; image callbacks encode into bounded owned
//! Rust bytes before publishing. Copied blocks own only Rust Send+Sync state.
//! Native request objects remain on their creation callback thread, alive through
//! the SCScreenshotManager call. No unsafe Send/Sync implementation is used.
use super::{
    completion::{Job, Stage},
    *,
};
use block2::{Block, RcBlock};
use std::{
    ffi::{c_char, c_void},
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
    sync::Mutex,
};

type Id = *mut c_void;
type Sel = *mut c_void;
type ObjcBool = i8;
fn assert_callback_send_sync<F: Send + Sync>(_: &F) {}
const BACKEND: &str = "ScreenCaptureKit one-shot (macOS 14+)";
// A timed-out SCScreenshotManager request cannot be forcibly cancelled. Hold this
// slot until the framework releases every callback, preventing repeated timeouts
// from accumulating native work/retained contexts without a bound.
static CAPTURE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
struct InflightGuard;
impl Drop for InflightGuard {
    fn drop(&mut self) {
        CAPTURE_IN_FLIGHT.store(false, Ordering::Release);
    }
}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
}
#[link(name = "Foundation", kind = "framework")]
extern "C" {}
#[link(name = "ScreenCaptureKit", kind = "framework")]
extern "C" {}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(id: u32) -> CaptureRect;
    fn CGDisplayPixelsWide(id: u32) -> usize;
    fn CGDisplayPixelsHigh(id: u32) -> usize;
    fn CGDisplayIsOnline(id: u32) -> u32;
    fn CGImageGetWidth(image: *const c_void) -> usize;
    fn CGImageGetHeight(image: *const c_void) -> usize;
    fn CGDataConsumerCreate(info: *mut c_void, callbacks: *const ConsumerCallbacks) -> Id;
    fn CGDataConsumerRelease(consumer: *const c_void);
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: *const c_void);
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        text: *const c_char,
        encoding: u32,
    ) -> Id;
}
#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageDestinationCreateWithDataConsumer(
        consumer: *const c_void,
        image_type: *const c_void,
        count: usize,
        options: *const c_void,
    ) -> Id;
    fn CGImageDestinationAddImage(
        destination: *const c_void,
        image: *const c_void,
        properties: *const c_void,
    );
    fn CGImageDestinationFinalize(destination: *const c_void) -> bool;
}
// No Objective-C struct-return selectors are used. CGDisplayBounds is a regular
// C function (Rust chooses that target's C struct-return ABI). setSourceRect:
// passes the 4-double CGRect by value and returns void; review both Apple ABIs
// before claiming x86_64 runtime support. BOOL uses an explicit byte, not bool.
macro_rules! send {
    ($object:expr, $selector:literal, ($($ty:ty => $arg:expr),* $(,)?) -> $ret:ty) => {{
        let function: unsafe extern "C" fn(Id, Sel, $($ty),*) -> $ret = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function($object, sel_registerName($selector.as_ptr().cast()), $($arg),*)
    }};
}
struct OwnedObject {
    ptr: Id,
    _not_send: PhantomData<Rc<()>>,
}
impl OwnedObject {
    unsafe fn from_owned(ptr: Id) -> CaptureResult<Self> {
        if ptr.is_null() {
            Err(CaptureError::NativeFailure)
        } else {
            Ok(Self {
                ptr,
                _not_send: PhantomData,
            })
        }
    }
    unsafe fn retained(ptr: Id) -> CaptureResult<Self> {
        if ptr.is_null() {
            return Err(CaptureError::NativeFailure);
        }
        Self::from_owned(send!(ptr, b"retain\0", () -> Id))
    }
}
impl Drop for OwnedObject {
    fn drop(&mut self) {
        unsafe {
            send!(self.ptr, b"release\0", () -> ());
        }
    }
}
struct OwnedCf(Id);
impl OwnedCf {
    fn new(ptr: Id) -> CaptureResult<Self> {
        if ptr.is_null() {
            Err(CaptureError::NativeFailure)
        } else {
            Ok(Self(ptr))
        }
    }
}
impl Drop for OwnedCf {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}
struct Consumer(Id);
impl Drop for Consumer {
    fn drop(&mut self) {
        unsafe {
            CGDataConsumerRelease(self.0);
        }
    }
}
unsafe fn class(name: &'static [u8]) -> CaptureResult<Id> {
    let value = objc_getClass(name.as_ptr().cast());
    if value.is_null() {
        Err(CaptureError::Unsupported)
    } else {
        Ok(value)
    }
}
unsafe fn pool() -> CaptureResult<OwnedObject> {
    OwnedObject::from_owned(send!(class(b"NSAutoreleasePool\0")?, b"new\0", () -> Id))
}

pub(super) fn is_available() -> bool {
    unsafe {
        let Ok(manager) = class(b"SCScreenshotManager\0") else {
            return false;
        };
        let selector =
            sel_registerName(c"captureImageWithFilter:configuration:completionHandler:".as_ptr());
        send!(manager, b"respondsToSelector:\0", (Sel => selector) -> ObjcBool) != 0
    }
}
fn display_metadata(id: u32) -> CaptureResult<CaptureDisplay> {
    unsafe {
        if id == 0 || CGDisplayIsOnline(id) == 0 {
            return Err(CaptureError::DisplayNotFound);
        }
        let display = CaptureDisplay {
            id,
            bounds: CGDisplayBounds(id),
            pixels: CapturePixelSize {
                width: u32::try_from(CGDisplayPixelsWide(id))
                    .map_err(|_| CaptureError::OutputTooLarge)?,
                height: u32::try_from(CGDisplayPixelsHigh(id))
                    .map_err(|_| CaptureError::OutputTooLarge)?,
            },
        };
        display.validate()?;
        Ok(display)
    }
}
pub(super) fn main_display() -> CaptureResult<CaptureDisplay> {
    display_metadata(unsafe { CGMainDisplayID() })
}
pub(super) fn capture(
    request: CaptureRequest,
    cancellation: CaptureCancellation,
) -> CaptureResult<NativeCaptureSnapshot> {
    if !is_available() {
        return Err(CaptureError::Unsupported);
    }
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err(CaptureError::PermissionNotGranted);
    }
    // Reject changes before asking the framework for content; the selected display
    // may have disappeared, so absence is left for the source-shaped resolver.
    if let Ok(now) = display_metadata(request.display.id) {
        if now.bounds != request.display.bounds || now.pixels != request.display.pixels {
            return Err(CaptureError::DisplayChanged);
        }
    }
    if CAPTURE_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(CaptureError::Busy);
    }
    let lease = Arc::new(InflightGuard);
    let job = Job::new(cancellation, request.timeout);
    unsafe {
        enumerate(request, job.clone(), None, lease)?;
    }
    job.wait()
}
unsafe fn enumerate(
    request: CaptureRequest,
    job: Arc<Job<NativeCaptureSnapshot>>,
    initial: Option<Vec<CaptureDisplay>>,
    lease: Arc<InflightGuard>,
) -> CaptureResult<()> {
    if !job.active() {
        return Ok(());
    }
    let _pool = pool()?;
    let content_class = class(b"SCShareableContent\0")?;
    let refreshed = initial.is_some();
    let callback_job = job.clone();
    let callback = move |content: Id, error: Id| {
        let expected = if refreshed {
            Stage::RefreshedContent
        } else {
            Stage::InitialContent
        };
        let resolving = if refreshed {
            Stage::ResolvingRefreshed
        } else {
            Stage::ResolvingInitial
        };
        if !callback_job.transition(expected, resolving) {
            return;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| unsafe {
            let _pool = pool()?;
            if !error.is_null() || content.is_null() {
                return Err(CaptureError::NativeFailure);
            }
            let displays: Id = send!(content, b"displays\0", () -> Id);
            if displays.is_null() {
                return Err(CaptureError::DisplayNotFound);
            }
            let count: usize = send!(displays, b"count\0", () -> usize);
            if count > MAX_DISPLAYS {
                return Err(CaptureError::InvalidRequest);
            }
            let mut candidates = Vec::with_capacity(count);
            for index in 0..count {
                if !callback_job.active() {
                    return Ok(());
                }
                let display: Id = send!(displays, b"objectAtIndex:\0", (usize => index) -> Id);
                if display.is_null() {
                    continue;
                }
                let id: u32 = send!(display, b"displayID\0", () -> u32);
                if let Ok(metadata) = display_metadata(id) {
                    candidates.push(metadata);
                }
            }
            if !refreshed && !candidates.iter().any(|d| d.id == request.display.id) {
                if callback_job.transition(resolving, Stage::RefreshedContent) {
                    enumerate(
                        request.clone(),
                        callback_job.clone(),
                        Some(candidates),
                        lease.clone(),
                    )?;
                }
                return Ok(());
            }
            let resolution = resolve_display(
                &request,
                initial.as_deref().unwrap_or(&candidates),
                if refreshed { Some(&candidates) } else { None },
            )?;
            // Never revive a disappeared SCDisplay from the initial catalog. An
            // initial-bounds fallback is usable only if that ID is still present.
            let mut selected = ptr::null_mut();
            for index in 0..count {
                let display: Id = send!(displays, b"objectAtIndex:\0", (usize => index) -> Id);
                if !display.is_null()
                    && send!(display, b"displayID\0", () -> u32) == resolution.display.id
                {
                    selected = display;
                    break;
                }
            }
            if selected.is_null() {
                return Err(CaptureError::DisplayNotFound);
            }
            if callback_job.transition(resolving, Stage::Image) {
                capture_image(
                    selected,
                    resolution,
                    request.clone(),
                    callback_job.clone(),
                    lease.clone(),
                )?;
            }
            Ok(())
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                callback_job.complete(Err(error));
            }
            Err(_) => {
                callback_job.complete(Err(CaptureError::NativeFailure));
            }
        }
    };
    assert_callback_send_sync(&callback);
    let block = RcBlock::new(callback);
    // Public SDK BOOL/BOOL/block signature. false includes desktop windows,
    // matching ScreenCaptureService, true restricts enumeration to on-screen windows.
    send!(content_class, b"getShareableContentExcludingDesktopWindows:onScreenWindowsOnly:completionHandler:\0",
        (ObjcBool => 0, ObjcBool => 1, *const Block<dyn Fn(Id, Id)> => &*block) -> ());
    Ok(())
}
struct RequestObjects {
    _display: OwnedObject,
    filter: OwnedObject,
    configuration: OwnedObject,
}
unsafe fn capture_image(
    display: Id,
    resolution: DisplayResolution,
    request: CaptureRequest,
    job: Arc<Job<NativeCaptureSnapshot>>,
    lease: Arc<InflightGuard>,
) -> CaptureResult<()> {
    if !job.active() {
        return Ok(());
    }
    let display = OwnedObject::retained(display)?;
    let empty: Id = send!(class(b"NSArray\0")?, b"array\0", () -> Id);
    if empty.is_null() {
        return Err(CaptureError::NativeFailure);
    }
    let filter_alloc: Id = send!(class(b"SCContentFilter\0")?, b"alloc\0", () -> Id);
    let filter = OwnedObject::from_owned(
        send!(filter_alloc, b"initWithDisplay:excludingApplications:exceptingWindows:\0", (Id => display.ptr, Id => empty, Id => empty) -> Id),
    )?;
    let config_alloc: Id = send!(class(b"SCStreamConfiguration\0")?, b"alloc\0", () -> Id);
    let configuration = OwnedObject::from_owned(send!(config_alloc, b"init\0", () -> Id))?;
    send!(configuration.ptr, b"setWidth:\0", (usize => request.output_size.width as usize) -> ());
    send!(configuration.ptr, b"setHeight:\0", (usize => request.output_size.height as usize) -> ());
    send!(configuration.ptr, b"setShowsCursor:\0", (ObjcBool => if request.include_cursor { 1 } else { 0 }) -> ());
    send!(configuration.ptr, b"setCapturesAudio:\0", (ObjcBool => 0) -> ());
    if let Some(region) = request.region {
        send!(configuration.ptr, b"setSourceRect:\0", (CaptureRect => region) -> ());
    }
    let objects = RequestObjects {
        _display: display,
        filter,
        configuration,
    };
    let filter_ptr = objects.filter.ptr;
    let configuration_ptr = objects.configuration.ptr;
    let manager = class(b"SCScreenshotManager\0")?;
    let callback_job = job.clone();
    let callback = move |image: Id, error: Id| {
        // The borrowed CGImage is consumed only during this callback; it never
        // crosses a channel or survives its callback scope. No native request
        // objects are captured in this Send+Sync closure.
        let _hold_native_slot_until_callback_release = &lease;
        if !callback_job.transition(Stage::Image, Stage::Encoding) {
            return;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| unsafe {
            if !error.is_null() || image.is_null() {
                return Err(CaptureError::NativeFailure);
            }
            if display_metadata(resolution.display.id)? != resolution.display {
                return Err(CaptureError::DisplayChanged);
            }
            let size = CapturePixelSize {
                width: u32::try_from(CGImageGetWidth(image))
                    .map_err(|_| CaptureError::OutputTooLarge)?,
                height: u32::try_from(CGImageGetHeight(image))
                    .map_err(|_| CaptureError::OutputTooLarge)?,
            };
            size.validate()?;
            if size != request.output_size {
                return Err(CaptureError::InvalidImage);
            }
            let png = encode_png(image, callback_job.clone())?;
            if display_metadata(resolution.display.id)? != resolution.display {
                return Err(CaptureError::DisplayChanged);
            }
            Ok(NativeCaptureSnapshot {
                png,
                diagnostics: CaptureDiagnostics {
                    requested_display_id: request.display.id,
                    resolved_display_id: resolution.display.id,
                    resolution_path: resolution.path,
                    output_size: size,
                    region: request.region,
                    includes_cursor: request.include_cursor,
                    backend: BACKEND,
                },
            })
        }));
        callback_job.complete(match outcome {
            Ok(result) => result,
            Err(_) => Err(CaptureError::NativeFailure),
        });
    };
    assert_callback_send_sync(&callback);
    let block = RcBlock::new(callback);
    // Cocoa's ownership policy requires a receiver to own objects it needs after
    // a call. SCScreenshotManager is the documented asynchronous receiver here;
    // native filter/config/display owners stay local through this call, then
    // release on this same callback thread. No extra native retain is moved to
    // the potentially different completion queue. SDK/native lifetime validation
    // remains required; Rust type checking cannot establish framework retention.
    // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/mmRules.html
    // https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager/captureimage(contentfilter:configuration:completionhandler:)
    send!(manager, b"captureImageWithFilter:configuration:completionHandler:\0",
        (Id => filter_ptr, Id => configuration_ptr, *const Block<dyn Fn(Id, Id)> => &*block) -> ());
    drop(objects);
    Ok(())
}
#[repr(C)]
struct ConsumerCallbacks {
    put_bytes: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> usize,
    release_info: unsafe extern "C" fn(*mut c_void),
}
static CONSUMER_CALLBACKS: ConsumerCallbacks = ConsumerCallbacks {
    put_bytes,
    release_info: release_sink,
};
struct PngState {
    bytes: Vec<u8>,
    failed: bool,
}
struct PngSink {
    state: Arc<Mutex<PngState>>,
    job: Arc<Job<NativeCaptureSnapshot>>,
}
unsafe extern "C" fn put_bytes(info: *mut c_void, bytes: *const c_void, count: usize) -> usize {
    // ImageIO owns the consumer for the complete synchronous encode. All Rust
    // state is heap-owned; a mutex protects the writer if callbacks are concurrent.
    catch_unwind(AssertUnwindSafe(|| {
        if info.is_null() || (bytes.is_null() && count != 0) {
            return 0;
        }
        let sink = &*info.cast::<PngSink>();
        if !sink.job.active() {
            return 0;
        }
        let mut state = sink.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.failed
            || count > MAX_CAPTURE_PNG_BYTES.saturating_sub(state.bytes.len())
            || state.bytes.try_reserve(count).is_err()
        {
            state.failed = true;
            return 0;
        }
        if count != 0 {
            state
                .bytes
                .extend_from_slice(std::slice::from_raw_parts(bytes.cast::<u8>(), count));
        }
        count
    }))
    .unwrap_or(0)
}
unsafe extern "C" fn release_sink(info: *mut c_void) {
    // Called once by CGDataConsumerRelease; no caller frees a successful consumer's info.
    if !info.is_null() {
        drop(Box::from_raw(info.cast::<PngSink>()));
    }
}
unsafe fn encode_png(image: Id, job: Arc<Job<NativeCaptureSnapshot>>) -> CaptureResult<Vec<u8>> {
    if !job.active() {
        return Err(CaptureError::Cancelled);
    }
    let state = Arc::new(Mutex::new(PngState {
        bytes: Vec::new(),
        failed: false,
    }));
    let info = Box::into_raw(Box::new(PngSink {
        state: state.clone(),
        job,
    }))
    .cast();
    let consumer = CGDataConsumerCreate(info, &CONSUMER_CALLBACKS);
    if consumer.is_null() {
        release_sink(info);
        return Err(CaptureError::NativeFailure);
    }
    let consumer = Consumer(consumer);
    let image_type = OwnedCf::new(CFStringCreateWithCString(
        ptr::null(),
        c"public.png".as_ptr(),
        0x0800_0100,
    ))?;
    let destination = OwnedCf::new(CGImageDestinationCreateWithDataConsumer(
        consumer.0,
        image_type.0,
        1,
        ptr::null(),
    ))?;
    CGImageDestinationAddImage(destination.0, image, ptr::null());
    let valid = CGImageDestinationFinalize(destination.0);
    drop(destination);
    drop(consumer);
    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
    if state.failed {
        return Err(CaptureError::OutputTooLarge);
    }
    if !valid || state.bytes.len() < 45 || !state.bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(CaptureError::InvalidImage);
    }
    Ok(std::mem::take(&mut state.bytes))
}
