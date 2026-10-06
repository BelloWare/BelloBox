//! Native macOS adapters. This module is compiled only on macOS.
//!
//! Sparkle must be owned by the application's main-thread state, never a worker.
//! Its constructor does not start the updater or contact the appcast. Vision is
//! an explicit, local operation suitable for a worker; it never accepts a URL.
//! The small Objective-C bridge below uses audited, concrete message signatures
//! rather than a variadic call to objc_msgSend. No private key is read here.

use crate::{ErrorKind, PlatformError, Result, MAX_OCR_IMAGE_BYTES, MAX_OCR_TEXT_BYTES};
use std::ffi::{c_char, c_void};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::ptr::{self, NonNull};
use std::rc::Rc;

type Id = *mut c_void;
type Sel = *mut c_void;
// Both supported macOS ABIs pass Objective-C BOOL in a byte-sized integer slot.
// We pass only 0/1 and compare returned values to zero (never construct Rust bool).
type ObjcBool = i8;
const UTF8: u32 = 0x0800_0100;
const UPDATE: &str = "Check for updates";
const OCR: &str = "Local OCR";
const SPARKLE_VERSION: &str = "2.8.1";
const MAX_IMAGE_PIXELS: u64 = 40_000_000;
const MAX_IMAGE_DIMENSION: i64 = 16_384;

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
}

// Force Foundation/Vision to be linked so their Objective-C classes are loaded.
#[link(name = "Foundation", kind = "framework")]
extern "C" {}
#[link(name = "Vision", kind = "framework")]
extern "C" {}
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[repr(C)]
#[derive(Clone, Copy)]
struct CfRange {
    location: isize,
    length: isize,
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: *const c_void);
    fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(value: *const c_void) -> u8;
    fn CFGetTypeID(value: *const c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetLength(value: *const c_void) -> isize;
    fn CFStringGetBytes(
        value: *const c_void,
        range: CfRange,
        encoding: u32,
        loss_byte: u8,
        external: u8,
        buffer: *mut u8,
        max: isize,
        used: *mut isize,
    ) -> isize;
    fn CFDataCreate(allocator: *const c_void, bytes: *const u8, length: isize) -> Id;
    fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> Id;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(number: *const c_void, kind: isize, value: *mut c_void) -> u8;
}

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageSourceCreateWithData(data: *const c_void, options: *const c_void) -> Id;
    fn CGImageSourceCopyPropertiesAtIndex(
        source: *const c_void,
        index: usize,
        options: *const c_void,
    ) -> Id;
    fn CGImageSourceCreateImageAtIndex(
        source: *const c_void,
        index: usize,
        options: *const c_void,
    ) -> Id;
    static kCGImagePropertyPixelWidth: *const c_void;
    static kCGImagePropertyPixelHeight: *const c_void;
    static kCGImagePropertyOrientation: *const c_void;
}

// SAFETY: Every invocation supplies the exact SDK method signature. All calls
// are confined to this module and made with an object of the documented class.
// These methods return only integers/pointers/void, never ABI-sensitive structs.
macro_rules! send {
    ($object:expr, $selector:literal, ($($ty:ty => $arg:expr),* $(,)?) -> $return:ty) => {{
        let function: unsafe extern "C" fn(Id, Sel, $($ty),*) -> $return =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function($object, sel_registerName($selector.as_ptr().cast()), $($arg),*)
    }};
}

/// Restore the host application after an explicit capture without stealing focus.
/// Must run on the AppKit/UI thread. No windows are created and no capture occurs.
/// Returns whether AppKit reports the application active after nonactivating restore.
pub fn unhide_application_without_activation() -> Result<bool> {
    const OP: &str = "Restore screenshot windows";
    unsafe {
        if send!(class(b"NSThread\0", OP)?, b"isMainThread\0", () -> ObjcBool) == 0 {
            return Err(error(
                OP,
                "Application visibility must be restored on the UI thread.",
            ));
        }
        let application = send!(class(b"NSApplication\0", OP)?, b"sharedApplication\0", () -> Id);
        if application.is_null() {
            return Err(error(OP, "The application is unavailable."));
        }
        send!(application, b"unhideWithoutActivation\0", () -> ());
        Ok(send!(application, b"isActive\0", () -> ObjcBool) != 0)
    }
}

struct OwnedObject {
    ptr: NonNull<c_void>,
    _thread_bound: PhantomData<Rc<()>>,
}

impl OwnedObject {
    unsafe fn new(ptr: Id, operation: &'static str) -> Result<Self> {
        NonNull::new(ptr)
            .map(|ptr| Self {
                ptr,
                _thread_bound: PhantomData,
            })
            .ok_or_else(|| {
                error(
                    operation,
                    "The native framework could not create the requested object.",
                )
            })
    }
    fn as_ptr(&self) -> Id {
        self.ptr.as_ptr()
    }
}

impl Drop for OwnedObject {
    fn drop(&mut self) {
        // SAFETY: new() is called only for +1 alloc/init/new objects.
        unsafe {
            send!(self.as_ptr(), b"release\0", () -> ());
        }
    }
}

struct OwnedCf(NonNull<c_void>);
impl OwnedCf {
    unsafe fn new(ptr: Id, operation: &'static str) -> Result<Self> {
        NonNull::new(ptr).map(Self).ok_or_else(|| {
            error(
                operation,
                "The native image decoder could not read the image.",
            )
        })
    }
    fn as_ptr(&self) -> Id {
        self.0.as_ptr()
    }
}
impl Drop for OwnedCf {
    fn drop(&mut self) {
        // SAFETY: OwnedCf holds only a +1 Core Foundation Create/Copy result.
        unsafe {
            CFRelease(self.as_ptr());
        }
    }
}

struct AutoreleasePool(OwnedObject);
impl AutoreleasePool {
    unsafe fn new(operation: &'static str) -> Result<Self> {
        Ok(Self(OwnedObject::new(
            send!(class(b"NSAutoreleasePool\0", operation)?, b"new\0", () -> Id),
            operation,
        )?))
    }
}
// Releasing NSAutoreleasePool drains it. Keeping this wrapper in scope also
// makes all borrowed Foundation values valid until the public call returns.
impl Drop for AutoreleasePool {
    fn drop(&mut self) {
        let _ = self.0.as_ptr();
    }
}

fn error(operation: &'static str, message: &'static str) -> PlatformError {
    PlatformError::new(ErrorKind::BackendFailed, operation, message)
}
fn unavailable(operation: &'static str, message: &'static str) -> PlatformError {
    PlatformError::new(ErrorKind::Unavailable, operation, message)
}

unsafe fn class(name: &'static [u8], operation: &'static str) -> Result<Id> {
    let class = objc_getClass(name.as_ptr().cast());
    if class.is_null() {
        Err(unavailable(
            operation,
            "The required macOS framework class is unavailable.",
        ))
    } else {
        Ok(class)
    }
}

unsafe fn ns_string(value: &str, operation: &'static str) -> Result<OwnedObject> {
    let allocated = send!(class(b"NSString\0", operation)?, b"alloc\0", () -> Id);
    OwnedObject::new(
        send!(allocated, b"initWithBytes:length:encoding:\0",
        (*const u8 => value.as_ptr(), usize => value.len(), usize => 4) -> Id),
        operation,
    )
}

unsafe fn string(value: Id, max: usize, operation: &'static str) -> Result<String> {
    if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
        return Err(error(
            operation,
            "The native framework returned an invalid string.",
        ));
    }
    let length = CFStringGetLength(value);
    if length < 0 || length as usize > max {
        return Err(PlatformError::new(
            ErrorKind::OutputTooLarge,
            operation,
            "Native text exceeds the output limit.",
        ));
    }
    let mut bytes = vec![0u8; (length as usize).saturating_mul(4).min(max)];
    let mut used = 0;
    let converted = CFStringGetBytes(
        value,
        CfRange {
            location: 0,
            length,
        },
        UTF8,
        0,
        0,
        bytes.as_mut_ptr(),
        bytes.len() as isize,
        &mut used,
    );
    if converted != length || used < 0 || used as usize > bytes.len() {
        return Err(PlatformError::new(
            ErrorKind::OutputTooLarge,
            operation,
            "Native text cannot be represented within the UTF-8 output limit.",
        ));
    }
    bytes.truncate(used as usize);
    String::from_utf8(bytes).map_err(|_| {
        error(
            operation,
            "The native framework returned invalid UTF-8 text.",
        )
    })
}

unsafe fn require_main_thread() -> Result<()> {
    if send!(class(b"NSThread\0", UPDATE)?, b"isMainThread\0", () -> ObjcBool) == 0 {
        Err(unavailable(
            UPDATE,
            "Sparkle must be used on the application's main thread.",
        ))
    } else {
        Ok(())
    }
}

unsafe fn bundle_string(bundle: Id, key: &str) -> Result<String> {
    let key = ns_string(key, UPDATE)?;
    let value = send!(bundle, b"objectForInfoDictionaryKey:\0", (Id => key.as_ptr()) -> Id);
    if value.is_null() {
        return Ok(String::new());
    }
    string(value, 4096, UPDATE)
}

struct SparkleBundle {
    bundle: Id,
}

unsafe fn validated_sparkle_bundle() -> Result<SparkleBundle> {
    let main = send!(class(b"NSBundle\0", UPDATE)?, b"mainBundle\0", () -> Id);
    let path = string(send!(main, b"bundlePath\0", () -> Id), 16 * 1024, UPDATE)?;
    let app = PathBuf::from(path);
    if app.extension().and_then(|value| value.to_str()) != Some("app") {
        return Err(unavailable(
            UPDATE,
            "Updates require the packaged Rust .app; a cargo-run executable has no Sparkle bundle.",
        ));
    }
    let feed = bundle_string(main, "SUFeedURL")?;
    if feed.is_empty() {
        return Err(unavailable(
            UPDATE,
            "This Rust bundle has no SUFeedURL. Package it with its own HTTPS Rust appcast URL.",
        ));
    }
    validate_feed(&feed)?;
    let key = bundle_string(main, "SUPublicEDKey")?;
    if !valid_public_key(&key) {
        return Err(unavailable(
            UPDATE,
            "This bundle needs a valid 32-byte base64 Sparkle SUPublicEDKey.",
        ));
    }
    validate_preview_identity(&bundle_string(main, "CFBundleIdentifier")?)?;
    if bundle_string(main, "CFBundleVersion")?.is_empty() {
        return Err(unavailable(
            UPDATE,
            "The Rust app bundle is missing its identifier or build version.",
        ));
    }
    let app = app
        .canonicalize()
        .map_err(|_| unavailable(UPDATE, "The Rust application bundle cannot be inspected."))?;
    let framework = app
        .join("Contents/Frameworks/Sparkle.framework")
        .canonicalize()
        .map_err(|_| {
            unavailable(
                UPDATE,
                "Bundled Sparkle.framework 2.8.1 was not found in Contents/Frameworks.",
            )
        })?;
    if !framework.starts_with(&app) {
        return Err(unavailable(
            UPDATE,
            "Sparkle.framework must be inside this app bundle, not a link to external code.",
        ));
    }
    let path = framework
        .to_str()
        .ok_or_else(|| unavailable(UPDATE, "The framework path is not valid UTF-8."))?;
    let path = ns_string(path, UPDATE)?;
    let bundle =
        send!(class(b"NSBundle\0", UPDATE)?, b"bundleWithPath:\0", (Id => path.as_ptr()) -> Id);
    if bundle.is_null() || bundle_string(bundle, "CFBundleShortVersionString")? != SPARKLE_VERSION {
        return Err(unavailable(
            UPDATE,
            "This Rust port requires the bundled official Sparkle.framework version 2.8.1.",
        ));
    }
    Ok(SparkleBundle { bundle })
}

fn valid_public_key(key: &str) -> bool {
    // A 32-byte key has exactly 43 base64 characters plus '='. The final sextet
    // has two padding bits, so its base64 alphabet index must be divisible by 4.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = key.as_bytes();
    bytes.len() == 44
        && bytes[43] == b'='
        && bytes[..43].iter().all(|byte| ALPHABET.contains(byte))
        && ALPHABET
            .iter()
            .position(|byte| *byte == bytes[42])
            .is_some_and(|index| index % 4 == 0)
}

fn validate_preview_identity(identifier: &str) -> Result<()> {
    if identifier != "com.ainoob.BelloBox.rust.preview" {
        return Err(unavailable(
            UPDATE,
            "Updates require the isolated Rust preview bundle identifier.",
        ));
    }
    Ok(())
}

fn validate_feed(feed: &str) -> Result<()> {
    let Some(authority_and_path) = feed.strip_prefix("https://") else {
        return Err(unavailable(
            UPDATE,
            "The Rust appcast SUFeedURL must be HTTPS.",
        ));
    };
    let host = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    // Keep this deliberately narrow grammar aligned with preview packaging.
    // No URL normalization, DNS lookups, or production-host aliases are needed.
    if !feed.is_ascii()
        || feed.chars().any(|c| c.is_whitespace() || c.is_control())
        || feed.contains(['@', '#', '%', '\\'])
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label.as_bytes()[0].is_ascii_alphanumeric()
                || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(unavailable(
            UPDATE,
            "The preview feed requires unambiguous ASCII HTTPS and a DNS hostname without ports, credentials, fragments or escapes.",
        ));
    }
    let host = host.to_ascii_lowercase();
    if host == "belloware.com" || host.ends_with(".belloware.com") {
        return Err(unavailable(UPDATE, "Production Belloware hosts cannot serve Rust preview updates. Configure a separate preview host."));
    }
    Ok(())
}

/// Read-only packaging preflight. Does not load executable framework code,
/// instantiate an updater, start a network request, or alter Sparkle preferences.
/// A successful result is not a code-signature or update-feed verification.
pub fn sparkle_availability() -> Result<()> {
    // SAFETY: NSBundle inspection is allowed off the main thread. All borrowed
    // objects remain within this call's autorelease pool.
    unsafe {
        let _pool = AutoreleasePool::new(UPDATE)?;
        validated_sparkle_bundle().map(|_| ())
    }
}

pub fn sparkle_available() -> bool {
    sparkle_availability().is_ok()
}

/// Retained Sparkle 2.8.1 standard updater. Keep one instance alive in the app's
/// main-thread state through any update UI/session. It is deliberately !Send and
/// !Sync, so it cannot be created on the UI thread then moved to a worker.
///
/// This does not poll or start Sparkle automatically. The first explicit
/// `check_for_updates` starts Sparkle and respects its existing user preferences.
/// Package SUEnableAutomaticChecks=false and SUAutomaticallyUpdate=false for a
/// new Rust bundle; do not point it at the Swift application's production feed.
pub struct SparkleUpdater {
    controller: OwnedObject,
    started: bool,
}

impl SparkleUpdater {
    /// Loads only this application's bundled Sparkle.framework and creates a
    /// stopped controller. Fails if called off the main thread or mispackaged.
    pub fn new() -> Result<Self> {
        unsafe {
            require_main_thread()?;
            let _pool = AutoreleasePool::new(UPDATE)?;
            let bundle = validated_sparkle_bundle()?;
            let mut native_error: Id = ptr::null_mut();
            if send!(bundle.bundle, b"loadAndReturnError:\0", (*mut Id => &mut native_error) -> ObjcBool)
                == 0
            {
                return Err(unavailable(UPDATE, "The bundled Sparkle framework could not load; verify its signing and embedded helper bundles."));
            }
            let allocated =
                send!(class(b"SPUStandardUpdaterController\0", UPDATE)?, b"alloc\0", () -> Id);
            let controller = OwnedObject::new(
                send!(allocated,
                b"initWithStartingUpdater:updaterDelegate:userDriverDelegate:\0",
                (ObjcBool => 0, Id => ptr::null_mut(), Id => ptr::null_mut()) -> Id),
                UPDATE,
            )?;
            Ok(Self {
                controller,
                started: false,
            })
        }
    }

    /// Explicit menu/button action. Success means Sparkle accepted the request,
    /// not that a download, verification, installation or update has completed.
    /// Sparkle presents its standard progress, error and installation UI.
    pub fn check_for_updates(&mut self) -> Result<()> {
        unsafe {
            require_main_thread()?;
            let _pool = AutoreleasePool::new(UPDATE)?;
            let updater = send!(self.controller.as_ptr(), b"updater\0", () -> Id);
            if updater.is_null() {
                return Err(error(UPDATE, "Sparkle did not provide an updater."));
            }
            if !self.started {
                let mut native_error: Id = ptr::null_mut();
                if send!(updater, b"startUpdater:\0", (*mut Id => &mut native_error) -> ObjcBool)
                    == 0
                {
                    return Err(error(UPDATE, "Sparkle rejected this bundle's update configuration. Check its appcast, public key, signing and embedded helpers."));
                }
                self.started = true;
            }
            if send!(updater, b"canCheckForUpdates\0", () -> ObjcBool) == 0 {
                return Err(error(
                    UPDATE,
                    "Sparkle is busy with an update session; try again when it finishes.",
                ));
            }
            send!(self.controller.as_ptr(), b"checkForUpdates:\0", (Id => ptr::null_mut()) -> ());
            Ok(())
        }
    }
}

/// Recognizes a bounded in-memory image with Apple's on-device Vision engine.
/// Call from a worker after an explicit OCR action. At most the first image in a
/// container is processed. Encoded input is limited to 32 MiB, decoded images to
/// 40 megapixels/16384 pixels per dimension, and UTF-8 text to 1 MiB/4096 regions.
/// No clipboard, file writes, subprocess, provider, or network access occurs.
///
/// Language hints are `+`-separated BCP-47 tags (for example `en-US+fr-FR`). The
/// common Tesseract hints eng/fra/deu/spa/ita/por/jpn/kor/chi_sim/chi_tra are mapped
/// for the portable API; other three-letter hints fail instead of being ignored.
/// `None` requests Vision's language detection. macOS 13 or newer is required.
/// Vision's synchronous perform call has no hard wall-clock timeout; callers
/// must not block the UI and should discard results for closed/replaced jobs.
pub fn recognize_text(image: &[u8], languages: Option<&str>) -> Result<String> {
    if image.is_empty() {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            OCR,
            "Choose a nonempty local image.",
        ));
    }
    if image.len() as u64 > MAX_OCR_IMAGE_BYTES {
        return Err(PlatformError::new(
            ErrorKind::InputTooLarge,
            OCR,
            "Image exceeds the 32 MiB OCR input limit.",
        ));
    }
    let languages = vision_languages(languages)?;
    unsafe {
        let _pool = AutoreleasePool::new(OCR)?;
        let data = OwnedCf::new(
            CFDataCreate(ptr::null(), image.as_ptr(), image.len() as isize),
            OCR,
        )?;
        let source = OwnedCf::new(CGImageSourceCreateWithData(data.as_ptr(), ptr::null()), OCR)?;
        let properties = OwnedCf::new(
            CGImageSourceCopyPropertiesAtIndex(source.as_ptr(), 0, ptr::null()),
            OCR,
        )?;
        let width = image_dimension(properties.as_ptr(), kCGImagePropertyPixelWidth)?;
        let height = image_dimension(properties.as_ptr(), kCGImagePropertyPixelHeight)?;
        validate_dimensions(width, height)?;
        let orientation = image_orientation(properties.as_ptr())?;
        let image = OwnedCf::new(
            CGImageSourceCreateImageAtIndex(source.as_ptr(), 0, ptr::null()),
            OCR,
        )?;
        let request = OwnedObject::new(
            send!(class(b"VNRecognizeTextRequest\0", OCR)?, b"new\0", () -> Id),
            OCR,
        )?;
        send!(request.as_ptr(), b"setRecognitionLevel:\0", (usize => 0) -> ()); // VNRequestTextRecognitionLevelAccurate
        send!(request.as_ptr(), b"setRevision:\0", (usize => 3) -> ()); // macOS 13+
        send!(request.as_ptr(), b"setUsesLanguageCorrection:\0", (ObjcBool => 1) -> ());
        if languages.is_empty() {
            send!(request.as_ptr(), b"setAutomaticallyDetectsLanguage:\0", (ObjcBool => 1) -> ());
        } else {
            let array = send!(class(b"NSMutableArray\0", OCR)?, b"array\0", () -> Id);
            for language in languages {
                let value = ns_string(&language, OCR)?;
                send!(array, b"addObject:\0", (Id => value.as_ptr()) -> ());
            }
            send!(request.as_ptr(), b"setRecognitionLanguages:\0", (Id => array) -> ());
        }
        let options = send!(class(b"NSDictionary\0", OCR)?, b"dictionary\0", () -> Id);
        let allocated = send!(class(b"VNImageRequestHandler\0", OCR)?, b"alloc\0", () -> Id);
        let handler = OwnedObject::new(
            send!(allocated, b"initWithCGImage:orientation:options:\0",
            (Id => image.as_ptr(), u32 => orientation, Id => options) -> Id),
            OCR,
        )?;
        let requests =
            send!(class(b"NSArray\0", OCR)?, b"arrayWithObject:\0", (Id => request.as_ptr()) -> Id);
        let mut native_error: Id = ptr::null_mut();
        if send!(handler.as_ptr(), b"performRequests:error:\0", (Id => requests, *mut Id => &mut native_error) -> ObjcBool)
            == 0
        {
            return Err(error(
                OCR,
                "Apple Vision could not recognize this image or language configuration.",
            ));
        }
        let observations = send!(request.as_ptr(), b"results\0", () -> Id);
        let count = send!(observations, b"count\0", () -> usize);
        if count > 4096 {
            return Err(PlatformError::new(
                ErrorKind::OutputTooLarge,
                OCR,
                "OCR exceeds the 4096-region limit.",
            ));
        }
        let mut output = String::new();
        for index in 0..count {
            let observation = send!(observations, b"objectAtIndex:\0", (usize => index) -> Id);
            let candidates = send!(observation, b"topCandidates:\0", (usize => 1) -> Id);
            if send!(candidates, b"count\0", () -> usize) == 0 {
                continue;
            }
            let candidate = send!(candidates, b"objectAtIndex:\0", (usize => 0) -> Id);
            let text = string(
                send!(candidate, b"string\0", () -> Id),
                MAX_OCR_TEXT_BYTES,
                OCR,
            )?;
            if text.is_empty() {
                continue;
            }
            let separator = usize::from(!output.is_empty());
            if text.len() + separator > MAX_OCR_TEXT_BYTES - output.len() {
                return Err(PlatformError::new(
                    ErrorKind::OutputTooLarge,
                    OCR,
                    "OCR exceeds the 1 MiB text limit.",
                ));
            }
            if separator != 0 {
                output.push('\n');
            }
            output.push_str(&text);
        }
        Ok(output)
    }
}

unsafe fn image_orientation(dictionary: Id) -> Result<u32> {
    let value = CFDictionaryGetValue(dictionary, kCGImagePropertyOrientation);
    if value.is_null() {
        return Ok(1);
    } // kCGImagePropertyOrientationUp
    let mut orientation: i64 = 0;
    if CFGetTypeID(value) != CFNumberGetTypeID()
        || CFNumberGetValue(value, 4, (&mut orientation as *mut i64).cast()) == 0
        || !(1..=8).contains(&orientation)
    {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            OCR,
            "The image has invalid orientation metadata.",
        ));
    }
    Ok(orientation as u32)
}

unsafe fn image_dimension(dictionary: Id, key: *const c_void) -> Result<i64> {
    let value = CFDictionaryGetValue(dictionary, key);
    let mut number: i64 = 0;
    // kCFNumberSInt64Type = 4. Never assume malformed properties are CFNumber.
    if value.is_null()
        || CFGetTypeID(value) != CFNumberGetTypeID()
        || CFNumberGetValue(value, 4, (&mut number as *mut i64).cast()) == 0
    {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            OCR,
            "The image has no valid pixel dimensions.",
        ));
    }
    Ok(number)
}

fn validate_dimensions(width: i64, height: i64) -> Result<()> {
    if width <= 0 || height <= 0 {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            OCR,
            "The image has no valid pixel dimensions.",
        ));
    }
    if width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || (width as u64).saturating_mul(height as u64) > MAX_IMAGE_PIXELS
    {
        return Err(PlatformError::new(
            ErrorKind::InputTooLarge,
            OCR,
            "Image exceeds the 40-megapixel or 16384-pixel dimension limit.",
        ));
    }
    Ok(())
}

fn vision_languages(languages: Option<&str>) -> Result<Vec<String>> {
    let Some(languages) = languages else {
        return Ok(Vec::new());
    };
    if languages.is_empty() || languages.len() > 128 {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            OCR,
            "Use up to 128 bytes of BCP-47 language hints.",
        ));
    }
    languages.split('+').map(|hint| {
        let mapped = match hint {
            "eng" => "en-US", "fra" => "fr-FR", "deu" => "de-DE", "spa" => "es-ES",
            "ita" => "it-IT", "por" => "pt-BR", "jpn" => "ja-JP", "kor" => "ko-KR",
            "chi_sim" => "zh-Hans", "chi_tra" => "zh-Hant", _ => hint,
        };
        if mapped.len() < 2 || mapped.len() > 35
            || !mapped.split('-').all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
            || (mapped.len() == 3 && !mapped.contains('-')) {
            return Err(PlatformError::new(ErrorKind::InvalidInput, OCR, "Use Vision BCP-47 hints such as en-US+fr-FR or supported portable language codes."));
        }
        Ok(mapped.to_owned())
    }).collect()
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: i32) -> Id;
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementGetPid(element: Id, pid: *mut i32) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Id, seconds: f32) -> i32;
    fn AXUIElementCopyAttributeValue(element: Id, attribute: Id, value: *mut Id) -> i32;
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetType(value: Id) -> u32;
    fn AXValueGetValue(value: Id, kind: u32, output: *mut c_void) -> u8;
}

const SELECTION: &str = "Read selection";

/// Text and the application from which an explicit AX read was captured. There
/// is intentionally no Debug implementation that could log the selected text.
/// A returned source PID is context, not authorization to replace text later.
pub struct NativeSelection {
    pub text: String,
    pub source_pid: i32,
    pub source_bundle_identifier: Option<String>,
}

/// Reads only AXSelectedText from the currently focused external control.
/// Requires already-granted Accessibility permission and never requests it.
///
/// Before reading text, validates the focused window, source PID and up to 16
/// ancestors; secure/protected or uninspectable ancestry fails closed. After
/// reading it revalidates frontmost app, focused window and focused control.
/// Calls are capped at 40 ms each within a 320 ms overall validation budget.
///
/// This deliberately does not match the Swift implementation's text-marker or
/// selected-range fallback. It never reads AXValue/the whole field/document,
/// searches other controls, synthesizes Copy/Paste or accesses the clipboard.
/// Call before activating the Rust application's UI or it will reject its own
/// focus. Run from an explicit action/shortcut, never from a preview/status read.
pub fn read_selected_text() -> Result<NativeSelection> {
    unsafe {
        if AXIsProcessTrusted() == 0 {
            return Err(unavailable(SELECTION, "Accessibility permission is not granted. Enable it in System Settings before reading a selection."));
        }
        let _pool = AutoreleasePool::new(SELECTION)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(320);
        let workspace = send!(class(b"NSWorkspace\0", SELECTION)?, b"sharedWorkspace\0", () -> Id);
        let frontmost = send!(workspace, b"frontmostApplication\0", () -> Id);
        if frontmost.is_null() {
            return Err(unavailable(
                SELECTION,
                "No frontmost application is available.",
            ));
        }
        let pid = send!(frontmost, b"processIdentifier\0", () -> i32);
        if pid <= 0 || pid as u32 == std::process::id() {
            return Err(unavailable(SELECTION, "Read selection before activating BelloBox, while the source application still has focus."));
        }
        let bundle = send!(frontmost, b"bundleIdentifier\0", () -> Id);
        let bundle = if bundle.is_null() {
            None
        } else {
            Some(string(bundle, 1024, SELECTION)?)
        };
        let app = OwnedCf::new(AXUIElementCreateApplication(pid), SELECTION)?;
        let window = ax_required_element(app.as_ptr(), "AXFocusedWindow", deadline)?;
        let focused = ax_required_element(app.as_ptr(), "AXFocusedUIElement", deadline)?;
        let owner_window = ax_required_element(focused.as_ptr(), "AXWindow", deadline)?;
        if CFEqual(window.as_ptr(), owner_window.as_ptr()) == 0 {
            return Err(unavailable(
                SELECTION,
                "The focused control does not belong to the captured source window.",
            ));
        }
        validate_selection_ancestry(focused.as_ptr(), window.as_ptr(), pid, deadline)?;
        // A collapsed or malformed exposed range must never become a selection.
        if let Some(range) = ax_attribute(focused.as_ptr(), "AXSelectedTextRange", deadline)? {
            let mut value = CfRange {
                location: 0,
                length: 0,
            };
            if CFGetTypeID(range.as_ptr()) != AXValueGetTypeID()
                || AXValueGetType(range.as_ptr()) != 4
                || AXValueGetValue(range.as_ptr(), 4, (&mut value as *mut CfRange).cast()) == 0
                || value.location < 0
                || value.length <= 0
            {
                return Err(PlatformError::new(
                    ErrorKind::InvalidInput,
                    SELECTION,
                    "The focused control has no nonempty text selection.",
                ));
            }
        }
        let selected = ax_attribute(focused.as_ptr(), "AXSelectedText", deadline)?
            .ok_or_else(|| unavailable(SELECTION, "The focused control does not expose AX selected text. Use explicit clipboard import instead."))?;
        let text = string(selected.as_ptr(), crate::MAX_CLIPBOARD_BYTES, SELECTION)?;
        if text.is_empty() {
            return Err(PlatformError::new(
                ErrorKind::InvalidInput,
                SELECTION,
                "The focused control has no nonempty text selection.",
            ));
        }
        let current_window = ax_required_element(app.as_ptr(), "AXFocusedWindow", deadline)?;
        let current_focus = ax_required_element(app.as_ptr(), "AXFocusedUIElement", deadline)?;
        let current_app = send!(workspace, b"frontmostApplication\0", () -> Id);
        if send!(current_app, b"processIdentifier\0", () -> i32) != pid
            || CFEqual(window.as_ptr(), current_window.as_ptr()) == 0
            || CFEqual(focused.as_ptr(), current_focus.as_ptr()) == 0
        {
            return Err(unavailable(SELECTION, "The source application, window or focused control changed during the selection read."));
        }
        check_ax_deadline(deadline)?;
        Ok(NativeSelection {
            text,
            source_pid: pid,
            source_bundle_identifier: bundle,
        })
    }
}

fn check_ax_deadline(deadline: std::time::Instant) -> Result<()> {
    if std::time::Instant::now() >= deadline {
        Err(PlatformError::new(
            ErrorKind::TimedOut,
            SELECTION,
            "The selection read exceeded its validation time budget.",
        ))
    } else {
        Ok(())
    }
}

unsafe fn ax_attribute(
    element: Id,
    name: &str,
    deadline: std::time::Instant,
) -> Result<Option<OwnedCf>> {
    check_ax_deadline(deadline)?;
    let remaining = deadline
        .saturating_duration_since(std::time::Instant::now())
        .as_secs_f32();
    if AXUIElementSetMessagingTimeout(element, remaining.clamp(0.001, 0.040)) != 0 {
        return Err(unavailable(
            SELECTION,
            "The source accessibility element cannot be inspected safely.",
        ));
    }
    let attribute = ns_string(name, SELECTION)?;
    let mut value: Id = ptr::null_mut();
    let result = AXUIElementCopyAttributeValue(element, attribute.as_ptr(), &mut value);
    // Own any returned value immediately, even if the response is an error or
    // arrived after the deadline, so early returns cannot leak native objects.
    let value = NonNull::new(value).map(OwnedCf);
    check_ax_deadline(deadline)?;
    match result {
        0 => Ok(value),
        -25205 | -25212 => Ok(None), // attributeUnsupported / noValue
        _ => Err(unavailable(
            SELECTION,
            "The source application could not provide a bounded accessibility response.",
        )),
    }
}

unsafe fn ax_required_element(
    element: Id,
    name: &str,
    deadline: std::time::Instant,
) -> Result<OwnedCf> {
    let value = ax_attribute(element, name, deadline)?.ok_or_else(|| {
        unavailable(
            SELECTION,
            "The source application's focused accessibility context is unavailable.",
        )
    })?;
    if CFGetTypeID(value.as_ptr()) != AXUIElementGetTypeID() {
        return Err(unavailable(
            SELECTION,
            "The source application returned an invalid accessibility element.",
        ));
    }
    Ok(value)
}

unsafe fn validate_selection_ancestry(
    focused: Id,
    window: Id,
    pid: i32,
    deadline: std::time::Instant,
) -> Result<()> {
    // Retain every copied ancestor until validation ends, enabling reliable
    // cycle detection without dangling pointers. The focused/window owners are
    // kept alive by read_selected_text throughout this helper.
    let mut ancestors: Vec<OwnedCf> = Vec::new();
    let mut current = focused;
    for _ in 0..16 {
        check_ax_deadline(deadline)?;
        let mut owner_pid = 0;
        if AXUIElementGetPid(current, &mut owner_pid) != 0 || owner_pid != pid {
            return Err(unavailable(
                SELECTION,
                "The selected control's source application cannot be verified.",
            ));
        }
        let role = ax_attribute(current, "AXRole", deadline)?.ok_or_else(|| {
            unavailable(
                SELECTION,
                "The selected control's ancestry cannot be inspected safely.",
            )
        })?;
        let role = string(role.as_ptr(), 256, SELECTION)?;
        if role == "AXSecureTextField" {
            return Err(unavailable(
                SELECTION,
                "Reading selected text from protected controls is not allowed.",
            ));
        }
        if let Some(subrole) = ax_attribute(current, "AXSubrole", deadline)? {
            if string(subrole.as_ptr(), 256, SELECTION)? == "AXSecureTextField" {
                return Err(unavailable(
                    SELECTION,
                    "Reading selected text from protected controls is not allowed.",
                ));
            }
        }
        if let Some(protected) = ax_attribute(current, "AXProtectedContent", deadline)? {
            if CFGetTypeID(protected.as_ptr()) != CFBooleanGetTypeID()
                || CFBooleanGetValue(protected.as_ptr()) != 0
            {
                return Err(unavailable(
                    SELECTION,
                    "Reading selected text from protected controls is not allowed.",
                ));
            }
        }
        if CFEqual(current, window) != 0 {
            return Ok(());
        }
        let parent = ax_required_element(current, "AXParent", deadline)?;
        if CFEqual(parent.as_ptr(), focused) != 0
            || ancestors
                .iter()
                .any(|item| CFEqual(item.as_ptr(), parent.as_ptr()) != 0)
        {
            return Err(unavailable(
                SELECTION,
                "The selected control's accessibility ancestry contains a cycle.",
            ));
        }
        current = parent.as_ptr();
        ancestors.push(parent);
    }
    Err(unavailable(
        SELECTION,
        "The source window could not be validated within the 16-ancestor limit.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_configuration_is_strict() {
        assert!(validate_preview_identity("com.ainoob.BelloBox.rust.preview").is_ok());
        for identifier in ["", "com.ainoob.BelloBox", "com.belloware.PiApp"] {
            assert!(validate_preview_identity(identifier).is_err());
        }
        assert!(validate_feed("https://example.com/rust.xml").is_ok());
        for feed in [
            "",
            "http://example.com/rust.xml",
            "https://",
            "https://user@example.com/x",
            "https://example.com/a b",
            "https://belloware.com/assets/bello_box.appcast.xml",
            "https://belloware.com/assets/bello_box.appcast.xml?rust=1",
            "https://belloware.com./assets/bello_box.appcast.xml",
            "https://belloware.com:443/assets/bello_box.appcast.xml",
            "https://belloware.com/assets/./bello_box.appcast.xml",
            "https://belloware.com/assets/%62ello_box.appcast.xml",
            "https://updates.belloware.com/preview.xml",
            "https://example.com:443/preview.xml",
            "https://example.com/preview.xml#fragment",
            "https://@example.com/preview.xml",
            "https://example..com/preview.xml",
            "https://éxample.com/preview.xml",
        ] {
            assert!(validate_feed(feed).is_err());
        }
        assert!(valid_public_key(
            "slSJ7z2j8RDa266+E/7To5AOOloc2YtiMUZUVEIhwNA="
        ));
        assert!(!valid_public_key("not-a-key"));
        assert!(!valid_public_key(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB="
        ));
    }

    #[test]
    fn vision_input_and_output_policy_helpers_are_bounded() {
        assert!(validate_dimensions(4000, 4000).is_ok());
        for (width, height) in [
            (0, 100),
            (-1, 100),
            (10000, 10000),
            (16385, 1),
            (i64::MAX, i64::MAX),
        ] {
            assert!(validate_dimensions(width, height).is_err());
        }
        assert_eq!(
            vision_languages(Some("eng+fra")).unwrap(),
            ["en-US", "fr-FR"]
        );
        assert_eq!(vision_languages(Some("zh-Hans")).unwrap(), ["zh-Hans"]);
        assert!(vision_languages(None).unwrap().is_empty());
        for value in ["", "en++fr", "../en", "xxx", "en_US"] {
            assert!(vision_languages(Some(value)).is_err());
        }
    }
    #[test]
    fn expired_ax_budget_is_rejected_without_native_calls() {
        let deadline = std::time::Instant::now() - std::time::Duration::from_millis(1);
        assert_eq!(
            check_ax_deadline(deadline).unwrap_err().kind,
            ErrorKind::TimedOut
        );
        assert!(
            check_ax_deadline(std::time::Instant::now() + std::time::Duration::from_secs(1))
                .is_ok()
        );
    }
}
