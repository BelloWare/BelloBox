//! Test-compiled macOS 14+/Apple Silicon independent-window candidate.
//! SCWindow/filter/configuration stay local to the shareable-content callback.
//! Only submission evidence, jobs and leases cross queues, apart from the shared
//! specialized immutable CGImage transport. No native-object Send assertion.
use super::*;
use crate::window_capture::{
    SubmittedWindowSource, WindowCaptureOptions, WindowCapturePlan, WindowCaptureSelection,
    WindowDisplayGeometry, WindowFrame, WindowIdentity, WindowObservation, WindowPolicyError,
    WindowSelectionToken, WindowTopology, MAX_WINDOW_CANDIDATES, MAX_WINDOW_IDENTITY_BYTES,
};

type WindowJob = Job<NativeWindowCaptureSnapshot, WindowCaptureError>;
type Method = *mut c_void;
const WINDOW_BACKEND: &str = "ScreenCaptureKit independent window (macOS 14+, candidate)";

#[link(name = "AppKit", kind = "framework")]
extern "C" {}
#[link(name = "System")]
// dispatch_get_main_queue is inline in Apple queue.h, not an exported symbol.
// Use the address of its exported opaque global object; never read its contents.
extern "C" {
    static _dispatch_main_q: c_void;
}
#[link(name = "objc")]
extern "C" {
    fn object_getClass(object: Id) -> Id;
    fn class_getInstanceMethod(class: Id, selector: Sel) -> Method;
    fn method_getImplementation(method: Method) -> Option<unsafe extern "C" fn()>;
    fn method_getNumberOfArguments(method: Method) -> u32;
    fn method_getReturnType(method: Method, destination: *mut c_char, length: usize);
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(options: u32, relative_to: u32) -> Id;
    fn CGRectMakeWithDictionaryRepresentation(dictionary: Id, rect: *mut CaptureRect) -> bool;
    fn CGDisplayRotation(display: u32) -> f64;
    static kCGWindowNumber: Id;
    static kCGWindowOwnerPID: Id;
    static kCGWindowLayer: Id;
    static kCGWindowAlpha: Id;
    static kCGWindowBounds: Id;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFGetTypeID(value: Id) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetCount(array: Id) -> isize;
    fn CFArrayGetValueAtIndex(array: Id, index: isize) -> Id;
    fn CFDictionaryGetValue(dictionary: Id, key: Id) -> Id;
    fn CFNumberGetValue(number: Id, kind: isize, value: *mut c_void) -> u8;
    fn CFStringGetLength(string: Id) -> isize;
    fn CFStringGetBytes(
        string: Id,
        range: CfRange,
        encoding: u32,
        loss_byte: u8,
        external: u8,
        buffer: *mut u8,
        maximum: isize,
        used: *mut isize,
    ) -> isize;
}

#[repr(C)]
struct CfRange {
    location: isize,
    length: isize,
}

#[derive(Clone)]
struct TopologySnapshot {
    main_display_id: u32,
    displays: Vec<WindowDisplayGeometry>,
}
impl TopologySnapshot {
    fn borrowed(&self) -> WindowTopology<'_> {
        WindowTopology {
            main_display_id: self.main_display_id,
            displays: &self.displays,
        }
    }
}
fn require_main_thread() -> WindowCaptureResult<()> {
    unsafe {
        if send!(class(b"NSThread\0")?, b"isMainThread\0", () -> ObjcBool) == 0 {
            return Err(CaptureError::NativeFailure.into());
        }
    }
    Ok(())
}
// Matches the existing overlay's deliberately Apple-Silicon-only checked IMP
// bridge. Rust selects the C CGRect return ABI, rather than pretending that
// objc_msgSend has a portable structure-return signature. No dynamic selector.
// https://github.com/apple-oss-distributions/objc4/blob/main/runtime/runtime.h
unsafe fn read_frame(object: Id) -> WindowCaptureResult<CaptureRect> {
    if object.is_null() {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let selector = sel_registerName(c"frame".as_ptr());
    let method = class_getInstanceMethod(object_getClass(object), selector);
    if method.is_null() || method_getNumberOfArguments(method) != 2 {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let mut encoding = [0u8; 128];
    method_getReturnType(method, encoding.as_mut_ptr().cast(), encoding.len());
    let end = encoding
        .iter()
        .position(|b| *b == 0)
        .ok_or(WindowPolicyError::InvalidMetadata)?;
    if ![
        &b"{CGRect={CGPoint=dd}{CGSize=dd}}"[..],
        &b"{_NSRect={_NSPoint=dd}{_NSSize=dd}}"[..],
    ]
    .contains(&&encoding[..end])
    {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let implementation =
        method_getImplementation(method).ok_or(WindowPolicyError::InvalidMetadata)?;
    let getter: unsafe extern "C" fn(Id, Sel) -> CaptureRect = std::mem::transmute(implementation);
    let rect = getter(object, selector);
    if !rect.valid() {
        return Err(WindowPolicyError::InvalidGeometry.into());
    }
    Ok(rect)
}
unsafe fn read_bundle(value: Id) -> WindowCaptureResult<String> {
    if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let length = CFStringGetLength(value);
    if length <= 0 || length as usize > MAX_WINDOW_IDENTITY_BYTES {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    // Convert the complete UTF-16 range, preserving embedded NULs so policy
    // rejects them rather than silently truncating the identity.
    let mut bytes = [0u8; MAX_WINDOW_IDENTITY_BYTES];
    let mut used = 0isize;
    let converted = CFStringGetBytes(
        value,
        CfRange {
            location: 0,
            length,
        },
        0x0800_0100,
        0,
        0,
        bytes.as_mut_ptr(),
        bytes.len() as isize,
        &mut used,
    );
    if converted != length || used <= 0 || used as usize > bytes.len() {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let text = std::str::from_utf8(&bytes[..used as usize])
        .map_err(|_| WindowPolicyError::InvalidMetadata)?;
    if text.chars().any(char::is_control) {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    Ok(text.to_owned())
}
unsafe fn number(dictionary: Id, key: Id) -> WindowCaptureResult<Id> {
    let value = CFDictionaryGetValue(dictionary, key);
    if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    Ok(value)
}
unsafe fn integer(dictionary: Id, key: Id) -> WindowCaptureResult<i64> {
    let mut result = 0i64;
    // SDK kCFNumberSInt64Type = 4; Boolean is an unsigned byte, not Rust bool.
    if CFNumberGetValue(
        number(dictionary, key)?,
        4,
        (&mut result as *mut i64).cast(),
    ) == 0
    {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    Ok(result)
}
unsafe fn observations(array: Id) -> WindowCaptureResult<Vec<WindowObservation<'static>>> {
    if array.is_null() || CFGetTypeID(array) != CFArrayGetTypeID() {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let count = CFArrayGetCount(array);
    if count < 0 || count as usize > MAX_WINDOW_CANDIDATES {
        return Err(WindowPolicyError::InvalidMetadata.into());
    }
    let mut result = Vec::with_capacity(count as usize);
    for index in 0..count {
        let row = CFArrayGetValueAtIndex(array, index);
        if row.is_null() || CFGetTypeID(row) != CFDictionaryGetTypeID() {
            return Err(WindowPolicyError::InvalidMetadata.into());
        }
        let window_id = u32::try_from(integer(row, kCGWindowNumber)?)
            .map_err(|_| WindowPolicyError::InvalidMetadata)?;
        let owner_process_id = i32::try_from(integer(row, kCGWindowOwnerPID)?)
            .map_err(|_| WindowPolicyError::InvalidMetadata)?;
        let layer = integer(row, kCGWindowLayer)?;
        let mut alpha = 0f64;
        if CFNumberGetValue(
            number(row, kCGWindowAlpha)?,
            6,
            (&mut alpha as *mut f64).cast(),
        ) == 0
        {
            return Err(WindowPolicyError::InvalidMetadata.into());
        }
        let bounds = CFDictionaryGetValue(row, kCGWindowBounds);
        if bounds.is_null() || CFGetTypeID(bounds) != CFDictionaryGetTypeID() {
            return Err(WindowPolicyError::InvalidMetadata.into());
        }
        let mut frame = CaptureRect::new(0., 0., 0., 0.);
        if !CGRectMakeWithDictionaryRepresentation(bounds, &mut frame) {
            return Err(WindowPolicyError::InvalidGeometry.into());
        }
        // This parser is used only with optionOnScreenOnly. Membership is the
        // CG visibility observation; CG bundle identifiers are deliberately absent.
        result.push(WindowObservation {
            identity: WindowIdentity {
                window_id,
                owner_process_id,
                owner_bundle_id: None,
            },
            frame: WindowFrame(frame),
            layer,
            alpha,
            on_screen: true,
        });
    }
    Ok(result)
}
unsafe fn current_observations() -> WindowCaptureResult<Vec<WindowObservation<'static>>> {
    // SDK optionOnScreenOnly (1) | excludeDesktopElements (16), null window ID.
    let catalog = OwnedCf::new(CGWindowListCopyWindowInfo(1 | 16, 0))?;
    observations(catalog.0)
}
unsafe fn topology_once() -> WindowCaptureResult<TopologySnapshot> {
    require_main_thread()?;
    let main_display_id = CGMainDisplayID();
    let screens = send!(class(b"NSScreen\0")?, b"screens\0", () -> Id);
    if screens.is_null() {
        return Err(WindowPolicyError::InvalidGeometry.into());
    }
    let count = send!(screens, b"count\0", () -> usize);
    if count == 0 || count > 64 {
        return Err(WindowPolicyError::InvalidGeometry.into());
    }
    let key = send!(class(b"NSString\0")?, b"stringWithUTF8String:\0", (*const c_char => c"NSScreenNumber".as_ptr()) -> Id);
    if key.is_null() {
        return Err(CaptureError::NativeFailure.into());
    }
    let mut displays = Vec::with_capacity(count);
    for index in 0..count {
        let screen = send!(screens, b"objectAtIndex:\0", (usize => index) -> Id);
        if screen.is_null() {
            return Err(WindowPolicyError::InvalidGeometry.into());
        }
        let description = send!(screen, b"deviceDescription\0", () -> Id);
        let number = send!(description, b"objectForKey:\0", (Id => key) -> Id);
        if number.is_null()
            || send!(number, b"isKindOfClass:\0", (Id => class(b"NSNumber\0")?) -> ObjcBool) == 0
        {
            return Err(WindowPolicyError::InvalidGeometry.into());
        }
        let id = send!(number, b"unsignedIntValue\0", () -> u32);
        displays.push(WindowDisplayGeometry {
            display: display_metadata(id)?,
            appkit_size_points: read_frame(screen)?.size,
            backing_scale: send!(screen, b"backingScaleFactor\0", () -> f64),
            rotation_degrees: CGDisplayRotation(id),
        });
    }
    Ok(TopologySnapshot {
        main_display_id,
        displays,
    })
}
unsafe fn current_topology() -> WindowCaptureResult<TopologySnapshot> {
    let first = topology_once()?;
    let second = topology_once()?;
    if first.main_display_id != second.main_display_id
        || first.displays.len() != second.displays.len()
        || first.displays.iter().any(|d| !second.displays.contains(d))
    {
        return Err(WindowPolicyError::TopologyChanged.into());
    }
    Ok(second)
}

#[derive(Clone)]
struct WindowRequest {
    selection: WindowCaptureSelection,
    options: WindowCaptureOptions,
    session: Arc<WindowCaptureSession>,
    cancellation: CaptureCancellation,
    expected_token: WindowSelectionToken,
}
impl WindowRequest {
    fn check(&self, job: &WindowJob) -> WindowCaptureResult<()> {
        if self.selection.is_cancelled() || self.cancellation.is_cancelled() || !job.active() {
            return Err(CaptureError::Cancelled.into());
        }
        if self.session.current()? != self.expected_token {
            return Err(WindowPolicyError::StaleSelection.into());
        }
        Ok(())
    }
}
// This private entry is deliberately not called by automated tests. It compiles
// the real action path; synthetic tests never enumerate SCK or acquire pixels.
fn capture(
    selection: WindowCaptureSelection,
    options: WindowCaptureOptions,
    session: Arc<WindowCaptureSession>,
    cancellation: CaptureCancellation,
) -> WindowCaptureResult<NativeWindowCaptureSnapshot> {
    require_worker_thread()?;
    if selection.is_cancelled() || cancellation.is_cancelled() {
        return Err(CaptureError::Cancelled.into());
    }
    validate_external_window_owner(selection.selected_identity().owner_process_id)?;
    if options.timeout.is_zero() || options.timeout > MAX_CAPTURE_TIMEOUT {
        return Err(WindowPolicyError::InvalidOptions.into());
    }
    if !is_available() {
        return Err(CaptureError::Unsupported.into());
    }
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err(CaptureError::PermissionNotGranted.into());
    }
    let lease = InflightGuard::acquire(&CAPTURE_IN_FLIGHT)?;
    let job = WindowJob::new(cancellation.clone(), options.timeout);
    let expected_token = session.current()?;
    let request = WindowRequest {
        selection,
        options,
        session,
        cancellation,
        expected_token,
    };
    unsafe {
        enqueue_main(MainAction::Begin(request), job.clone(), lease)?;
    }
    job.wait()
}

enum MainAction {
    Begin(WindowRequest),
    Complete {
        target: WindowEncodeTarget,
        png: Vec<u8>,
        size: CapturePixelSize,
    },
}
struct MainContext {
    action: MainAction,
    job: Arc<WindowJob>,
    _lease: Arc<InflightGuard>,
}
unsafe fn enqueue_main(
    action: MainAction,
    job: Arc<WindowJob>,
    lease: Arc<InflightGuard>,
) -> WindowCaptureResult<()> {
    let queue = ptr::addr_of!(_dispatch_main_q).cast_mut();
    if queue.is_null() {
        return Err(CaptureError::NativeFailure.into());
    }
    // Only owned Rust data enters this context. Native objects are queried and
    // released inside the main invocation, never sent from another thread.
    fn assert_send<T: Send>() {}
    assert_send::<MainContext>();
    let context = Box::into_raw(Box::new(MainContext {
        action,
        job,
        _lease: lease,
    }))
    .cast();
    dispatch_async_f(queue, context, on_main);
    Ok(())
}
unsafe extern "C" fn on_main(context: *mut c_void) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut context = Box::from_raw(context.cast::<MainContext>());
        let result = catch_unwind(AssertUnwindSafe(|| run_main(&mut context)));
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
unsafe fn run_main(context: &mut MainContext) -> WindowCaptureResult<()> {
    require_main_thread()?;
    if !context.job.active() {
        return Ok(());
    }
    let _pool = pool()?;
    match &mut context.action {
        MainAction::Begin(request) => {
            if !context
                .job
                .transition(Stage::InitialContent, Stage::ResolvingInitial)
            {
                return Ok(());
            }
            request.check(&context.job)?;
            let topology = current_topology()?;
            if context
                .job
                .transition(Stage::ResolvingInitial, Stage::RefreshedContent)
            {
                enumerate_window(
                    request.clone(),
                    topology,
                    context.job.clone(),
                    context._lease.clone(),
                )?;
            }
        }
        MainAction::Complete { target, png, size } => {
            if !context.job.transition(Stage::Encoding, Stage::Validating) {
                return Ok(());
            }
            target.request.check(&context.job)?;
            let topology = current_topology()?;
            let fresh = current_observations()?;
            complete_validated(
                target,
                png,
                *size,
                &fresh,
                topology.borrowed(),
                &context.job,
            )?;
        }
    }
    Ok(())
}
fn complete_validated(
    target: &WindowEncodeTarget,
    png: &mut Vec<u8>,
    size: CapturePixelSize,
    fresh: &[WindowObservation<'_>],
    topology: WindowTopology<'_>,
    job: &Arc<WindowJob>,
) -> WindowCaptureResult<()> {
    target.request.check(job)?;
    // Hold the live-token lock through the final claim; a new generation cannot
    // slip between validation and completion. Host publication remains separate.
    let token = target
        .request
        .session
        .0
        .lock()
        .map_err(|_| CaptureError::NativeFailure)?;
    target
        .plan
        .validate_completion(fresh, topology, *token, size, &target.request.cancellation)?;
    let identity = target.plan.submitted_identity();
    job.complete(Ok(NativeWindowCaptureSnapshot {
        png: std::mem::take(png),
        diagnostics: WindowCaptureDiagnostics {
            window_id: identity.window_id,
            owner_process_id: identity.owner_process_id,
            output_size: size,
            includes_cursor: target.plan.options().include_cursor,
            backend: WINDOW_BACKEND,
        },
    }));
    Ok(())
}
unsafe fn enumerate_window(
    request: WindowRequest,
    topology: TopologySnapshot,
    job: Arc<WindowJob>,
    lease: Arc<InflightGuard>,
) -> WindowCaptureResult<()> {
    request.check(&job)?;
    let callback_job = job.clone();
    let callback = move |content: Id, error: Id| {
        let _callback_lease = &lease;
        if !callback_job.transition(Stage::RefreshedContent, Stage::ResolvingRefreshed) {
            return;
        }
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            let _pool = pool()?;
            request.check(&callback_job)?;
            if content.is_null() || !error.is_null() {
                return Err(CaptureError::NativeFailure.into());
            }
            let windows = send!(content, b"windows\0", () -> Id);
            if windows.is_null() {
                return Err(WindowPolicyError::WindowNotFound.into());
            }
            let count = send!(windows, b"count\0", () -> usize);
            if count > MAX_WINDOW_CANDIDATES {
                return Err(WindowPolicyError::InvalidMetadata.into());
            }
            let mut selected: Id = ptr::null_mut();
            for index in 0..count {
                request.check(&callback_job)?;
                let window = send!(windows, b"objectAtIndex:\0", (usize => index) -> Id);
                if window.is_null() {
                    return Err(WindowPolicyError::InvalidMetadata.into());
                }
                if send!(window, b"windowID\0", () -> u32)
                    == request.selection.selected_identity().window_id
                {
                    if !selected.is_null() {
                        return Err(WindowPolicyError::AmbiguousWindow.into());
                    }
                    selected = window;
                }
            }
            if selected.is_null() {
                return Err(WindowPolicyError::WindowNotFound.into());
            }
            let selected = OwnedObject::retained(selected)?;
            let app = send!(selected.ptr, b"owningApplication\0", () -> Id);
            if app.is_null() {
                return Err(WindowPolicyError::InvalidMetadata.into());
            }
            let bundle = read_bundle(send!(app, b"bundleIdentifier\0", () -> Id))?;
            let source = SubmittedWindowSource {
                identity: WindowIdentity {
                    window_id: send!(selected.ptr, b"windowID\0", () -> u32),
                    owner_process_id: send!(app, b"processID\0", () -> i32),
                    owner_bundle_id: Some(&bundle),
                },
                frame: WindowFrame(read_frame(selected.ptr)?),
                layer: send!(selected.ptr, b"windowLayer\0", () -> isize) as i64,
                on_screen: send!(selected.ptr, b"isOnScreen\0", () -> ObjcBool) != 0,
            };
            validate_external_window_owner(source.identity.owner_process_id)?;
            let fresh = current_observations()?;
            let plan = request.selection.plan(
                source,
                &fresh,
                topology.borrowed(),
                request.session.current()?,
                request.options,
                &request.cancellation,
            )?;
            request.check(&callback_job)?;
            if callback_job.transition(Stage::ResolvingRefreshed, Stage::Image) {
                submit_window(
                    selected,
                    WindowEncodeTarget {
                        request: request.clone(),
                        plan: Arc::new(plan),
                    },
                    callback_job.clone(),
                    lease.clone(),
                )?;
            }
            Ok(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                callback_job.complete(Err(error));
            }
            Err(payload) => {
                std::mem::forget(payload);
                callback_job.complete(Err(CaptureError::NativeFailure.into()));
            }
        }
    };
    assert_callback_send_sync(&callback);
    let block = RcBlock::new(callback);
    send!(class(b"SCShareableContent\0")?, b"getShareableContentExcludingDesktopWindows:onScreenWindowsOnly:completionHandler:\0",
        (ObjcBool => 0, ObjcBool => 1, *const Block<dyn Fn(Id, Id)> => &*block) -> ());
    Ok(())
}
#[derive(Clone)]
struct WindowEncodeTarget {
    request: WindowRequest,
    plan: Arc<WindowCapturePlan>,
}
impl EncodeTarget for WindowEncodeTarget {
    type Output = NativeWindowCaptureSnapshot;
    type Error = WindowCaptureError;
    fn validate_source(&self) -> WindowCaptureResult<()> {
        if self.request.session.current()? != self.request.expected_token {
            return Err(WindowPolicyError::StaleSelection.into());
        }
        Ok(())
    }
    fn validate_image(&self, size: CapturePixelSize) -> WindowCaptureResult<()> {
        if size != self.plan.output_size() {
            return Err(WindowPolicyError::UnexpectedImageSize.into());
        }
        if self.request.selection.is_cancelled() || self.request.cancellation.is_cancelled() {
            return Err(CaptureError::Cancelled.into());
        }
        Ok(())
    }
    fn finish(
        &self,
        png: Vec<u8>,
        size: CapturePixelSize,
        job: &Arc<WindowJob>,
        lease: &Arc<InflightGuard>,
    ) -> WindowCaptureResult<()> {
        self.request.check(job)?;
        unsafe {
            enqueue_main(
                MainAction::Complete {
                    target: self.clone(),
                    png,
                    size,
                },
                job.clone(),
                lease.clone(),
            )
        }
    }
}
unsafe fn submit_window(
    selected: OwnedObject,
    target: WindowEncodeTarget,
    job: Arc<WindowJob>,
    lease: Arc<InflightGuard>,
) -> WindowCaptureResult<()> {
    target.request.check(&job)?;
    validate_external_window_owner(target.plan.submitted_identity().owner_process_id)?;
    let filter_alloc = send!(class(b"SCContentFilter\0")?, b"alloc\0", () -> Id);
    let filter = OwnedObject::from_owned(
        send!(filter_alloc, b"initWithDesktopIndependentWindow:\0", (Id => selected.ptr) -> Id),
    )?;
    let configuration =
        OwnedObject::from_owned(send!(class(b"SCStreamConfiguration\0")?, b"new\0", () -> Id))?;
    let size = target.plan.output_size();
    send!(configuration.ptr, b"setWidth:\0", (usize => size.width as usize) -> ());
    send!(configuration.ptr, b"setHeight:\0", (usize => size.height as usize) -> ());
    send!(configuration.ptr, b"setShowsCursor:\0", (ObjcBool => if target.plan.options().include_cursor { 1 } else { 0 }) -> ());
    send!(configuration.ptr, b"setCapturesAudio:\0", (ObjcBool => 0) -> ());
    let callback_job = job.clone();
    let callback = move |image: Id, error: Id| {
        let _callback_lease = &lease;
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            target.request.check(&callback_job)?;
            queue_image(image, error, target.clone(), &callback_job, &lease)
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                callback_job.complete(Err(error));
            }
            Err(payload) => {
                std::mem::forget(payload);
                callback_job.complete(Err(CaptureError::NativeFailure.into()));
            }
        }
    };
    assert_callback_send_sync(&callback);
    let block = RcBlock::new(callback);
    send!(class(b"SCScreenshotManager\0")?, b"captureImageWithFilter:configuration:completionHandler:\0",
        (Id => filter.ptr, Id => configuration.ptr, *const Block<dyn Fn(Id, Id)> => &*block) -> ());
    // Same Cocoa asynchronous-argument ownership basis as Display. SCK must keep
    // the state it needs; we do not claim it retains this exact SCWindow instance.
    // All our native request owners release here on the content callback thread.
    drop(configuration);
    drop(filter);
    drop(selected);
    Ok(())
}

#[cfg(test)]
mod tests;
