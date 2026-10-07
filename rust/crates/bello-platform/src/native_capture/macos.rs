//! macOS 14+ one-shot bridge. Copied blocks capture only Rust Send+Sync state.
//! The image callback retains one immutable CGImage and transfers a specialized
//! boxed context to libdispatch; image/display validation and PNG encoding run
//! off the main thread.
//! Native request objects remain local through the SCScreenshotManager call.
//! No unsafe Send/Sync implementation or generic non-Send dispatcher is used.
use super::{
    completion::{InflightGuard, Job, Stage},
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
// slot until both the framework's callbacks and the queued/running encode have
// released their leases. A timed-out waiter never reclaims a submitted context.
static CAPTURE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[path = "alpha_mask_macos.rs"]
pub(super) mod alpha_mask;

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
}
#[link(name = "System")]
extern "C" {
    // dispatch/queue.h: intptr_t identifier, uintptr_t flags; dispatch_function_t
    // is void (*)(void *). The global queue is borrowed and must not be released.
    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> Id;
    fn dispatch_async_f(queue: Id, context: *mut c_void, work: unsafe extern "C" fn(*mut c_void));
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
    fn CGImageRetain(image: *const c_void) -> Id;
    fn CGImageRelease(image: *const c_void);
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

/// Test the actual Cocoa main thread, not the caller's thread name or queue.
pub(super) fn require_worker_thread() -> CaptureResult<()> {
    unsafe {
        if send!(class(b"NSThread\0")?, b"isMainThread\0", () -> ObjcBool) != 0 {
            return Err(CaptureError::WorkerThreadRequired);
        }
    }
    Ok(())
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
    capture_retiring(request, cancellation, false)
}
#[cfg(target_arch = "aarch64")]
pub(super) fn capture_window_freeze(
    request: CaptureRequest,
    cancellation: CaptureCancellation,
) -> CaptureResult<NativeCaptureSnapshot> {
    capture_retiring(request, cancellation, true)
}
fn capture_retiring(
    request: CaptureRequest,
    cancellation: CaptureCancellation,
    wait_for_drain: bool,
) -> CaptureResult<NativeCaptureSnapshot> {
    require_worker_thread()?;
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
    let lease = InflightGuard::acquire(&CAPTURE_IN_FLIGHT)?;
    let drain = wait_for_drain.then(|| lease.drain());
    let job = Job::new(cancellation, request.timeout);
    unsafe {
        enumerate(request, job.clone(), None, lease)?;
    }
    match drain {
        Some(drain) => job.wait_drained(drain),
        None => job.wait(),
    }
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
        // SCK does not document this callback's queue. Keep it to phase/ownership
        // work even if delivered on the main thread. This original lease remains
        // captured until SCK releases the block, independently of the worker lease.
        let _hold_native_slot_until_callback_release = &lease;
        let outcome = catch_unwind(AssertUnwindSafe(|| unsafe {
            enqueue_image(image, error, resolution, &request, &callback_job, &lease)
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                callback_job.complete(Err(error));
            }
            Err(payload) => {
                // Even a custom panic payload's destructor must not unwind into C.
                std::mem::forget(payload);
                callback_job.complete(Err(CaptureError::NativeFailure));
            }
        }
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
/// Owns exactly one CGImageRetain; intentionally neither Clone nor Send/Sync.
/// Only immutable SCK CGImages enter this private owner. Apple's CGImage is
/// Sendable; immutable Core Foundation references may be retained/read/released
/// across threads. This does not generalize to arbitrary CoreGraphics objects.
/// https://developer.apple.com/documentation/coregraphics/cgimage?changes=latest__1
/// https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html
struct OwnedCaptureImage(Id);
impl OwnedCaptureImage {
    unsafe fn retain(image: Id) -> CaptureResult<Self> {
        if image.is_null() {
            return Err(CaptureError::InvalidImage);
        }
        let image = CGImageRetain(image);
        if image.is_null() {
            Err(CaptureError::NativeFailure)
        } else {
            Ok(Self(image))
        }
    }
}
impl Drop for OwnedCaptureImage {
    fn drop(&mut self) {
        unsafe { CGImageRelease(self.0) };
    }
}
// Both actual source paths use this image-only transport. A target contains
// owned Rust metadata, never SCWindow/SCDisplay/filter/configuration pointers.
trait EncodeTarget: Clone + Send + Sync + 'static {
    type Output: Send + 'static;
    type Error: From<CaptureError> + Send + 'static;
    fn validate_source(&self) -> Result<(), Self::Error>;
    fn validate_image(&self, size: CapturePixelSize) -> Result<(), Self::Error>;
    fn finish(
        &self,
        png: Vec<u8>,
        size: CapturePixelSize,
        job: &Arc<Job<Self::Output, Self::Error>>,
        lease: &Arc<InflightGuard>,
    ) -> Result<(), Self::Error>;
}
#[derive(Clone)]
struct DisplayEncodeTarget {
    resolution: DisplayResolution,
    request: CaptureRequest,
}
impl EncodeTarget for DisplayEncodeTarget {
    type Output = NativeCaptureSnapshot;
    type Error = CaptureError;
    fn validate_source(&self) -> CaptureResult<()> {
        if display_metadata(self.resolution.display.id)? != self.resolution.display {
            return Err(CaptureError::DisplayChanged);
        }
        Ok(())
    }
    fn validate_image(&self, size: CapturePixelSize) -> CaptureResult<()> {
        if size != self.request.output_size {
            return Err(CaptureError::InvalidImage);
        }
        Ok(())
    }
    fn finish(
        &self,
        png: Vec<u8>,
        size: CapturePixelSize,
        job: &Arc<Job<NativeCaptureSnapshot>>,
        _lease: &Arc<InflightGuard>,
    ) -> CaptureResult<()> {
        // Preserve the original post-encoding display check.
        if display_metadata(self.resolution.display.id)? != self.resolution.display {
            return Err(CaptureError::DisplayChanged);
        }
        job.complete(Ok(NativeCaptureSnapshot {
            png,
            diagnostics: CaptureDiagnostics {
                requested_display_id: self.request.display.id,
                resolved_display_id: self.resolution.display.id,
                resolution_path: self.resolution.path,
                output_size: size,
                region: self.request.region,
                includes_cursor: self.request.include_cursor,
                backend: BACKEND,
            },
        }));
        Ok(())
    }
}
struct EncodeContext<T: EncodeTarget> {
    // Release image before the last lease, including during unwind.
    image: OwnedCaptureImage,
    target: T,
    job: Arc<Job<T::Output, T::Error>>,
    _lease: Arc<InflightGuard>,
}
unsafe fn enqueue_image(
    image: Id,
    error: Id,
    resolution: DisplayResolution,
    request: &CaptureRequest,
    job: &Arc<Job<NativeCaptureSnapshot>>,
    lease: &Arc<InflightGuard>,
) -> CaptureResult<()> {
    queue_image(
        image,
        error,
        DisplayEncodeTarget {
            resolution,
            request: request.clone(),
        },
        job,
        lease,
    )
}
unsafe fn queue_image<T: EncodeTarget>(
    image: Id,
    error: Id,
    target: T,
    job: &Arc<Job<T::Output, T::Error>>,
    lease: &Arc<InflightGuard>,
) -> Result<(), T::Error> {
    if !job.transition(Stage::Image, Stage::EncodingQueued) {
        return Ok(());
    }
    if !error.is_null() || image.is_null() {
        return Err(CaptureError::NativeFailure.into());
    }
    let queue = dispatch_get_global_queue(0, 0);
    if queue.is_null() {
        return Err(CaptureError::NativeFailure.into());
    }
    let context = Box::new(EncodeContext {
        image: OwnedCaptureImage::retain(image)?,
        target,
        job: job.clone(),
        _lease: lease.clone(),
    });
    // Only the independently retained immutable CGImage and owned Send Rust state
    // cross this specialized FFI boundary. No request object or borrowed address
    // is moved. libdispatch invokes this exact monomorphized destructor once;
    // the waiter cannot reclaim this context after timeout/cancellation.
    let context = Box::into_raw(context).cast();
    dispatch_async_f(queue, context, encode_on_worker::<T>);
    Ok(())
}
unsafe extern "C" fn encode_on_worker<T: EncodeTarget>(context: *mut c_void) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let context = Box::from_raw(context.cast::<EncodeContext<T>>());
        let result = catch_unwind(AssertUnwindSafe(|| encode_context(&context)));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                context.job.complete(Err(error));
            }
            Err(payload) => {
                std::mem::forget(payload);
                context
                    .job
                    .complete(Err(CaptureError::NativeFailure.into()));
            }
        }
        drop(context);
    }));
    if let Err(payload) = outcome {
        std::mem::forget(payload);
    }
}
unsafe fn encode_context<T: EncodeTarget>(context: &EncodeContext<T>) -> Result<(), T::Error> {
    require_worker_thread()?;
    if !context
        .job
        .transition(Stage::EncodingQueued, Stage::Encoding)
    {
        return Err(CaptureError::Cancelled.into());
    }
    let _pool = pool()?;
    context.target.validate_source()?;
    let size = CapturePixelSize {
        width: u32::try_from(CGImageGetWidth(context.image.0))
            .map_err(|_| CaptureError::OutputTooLarge)?,
        height: u32::try_from(CGImageGetHeight(context.image.0))
            .map_err(|_| CaptureError::OutputTooLarge)?,
    };
    size.validate()?;
    context.target.validate_image(size)?;
    let png = encode_png(context.image.0, context.job.clone())?;
    context
        .target
        .finish(png, size, &context.job, &context._lease)
}

#[repr(C)]
struct ConsumerCallbacks {
    put_bytes: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> usize,
    release_info: unsafe extern "C" fn(*mut c_void),
}
struct PngState {
    bytes: Vec<u8>,
    failed: bool,
}
struct PngSink<T = NativeCaptureSnapshot, E = CaptureError> {
    state: Arc<Mutex<PngState>>,
    job: Arc<Job<T, E>>,
}
unsafe extern "C" fn put_bytes<T, E: From<CaptureError>>(
    info: *mut c_void,
    bytes: *const c_void,
    count: usize,
) -> usize {
    // ImageIO owns the consumer for the complete synchronous encode. All Rust
    // state is heap-owned; a mutex protects the writer if callbacks are concurrent.
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if info.is_null() || (bytes.is_null() && count != 0) {
            return 0;
        }
        let sink = &*info.cast::<PngSink<T, E>>();
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
    }));
    match outcome {
        Ok(count) => count,
        Err(payload) => {
            std::mem::forget(payload);
            0
        }
    }
}
unsafe extern "C" fn release_sink<T, E>(info: *mut c_void) {
    // Called once by CGDataConsumerRelease; no caller frees a successful consumer's info.
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if !info.is_null() {
            drop(Box::from_raw(info.cast::<PngSink<T, E>>()));
        }
    }));
    if let Err(payload) = outcome {
        std::mem::forget(payload);
    }
}
unsafe fn encode_png<T: Send, E: From<CaptureError> + Send>(
    image: Id,
    job: Arc<Job<T, E>>,
) -> CaptureResult<Vec<u8>> {
    require_worker_thread()?;
    if !job.active() {
        return Err(CaptureError::Cancelled);
    }
    let state = Arc::new(Mutex::new(PngState {
        bytes: Vec::new(),
        failed: false,
    }));
    let info = Box::into_raw(Box::new(PngSink {
        state: state.clone(),
        job: job.clone(),
    }))
    .cast();
    let callbacks = ConsumerCallbacks {
        put_bytes: put_bytes::<T, E>,
        release_info: release_sink::<T, E>,
    };
    let consumer = CGDataConsumerCreate(info, &callbacks);
    if consumer.is_null() {
        release_sink::<T, E>(info);
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
    if !job.active() {
        return Err(CaptureError::Cancelled);
    }
    CGImageDestinationAddImage(destination.0, image, ptr::null());
    if !job.active() {
        return Err(CaptureError::Cancelled);
    }
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

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "alpha_mask_tests.rs"]
mod alpha_mask_tests;

#[cfg(target_arch = "aarch64")]
pub(super) mod window;
