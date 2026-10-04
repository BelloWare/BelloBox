//! Explicit platform actions for BelloBox. Constructing [`Platform`] does nothing.
//!
//! Call capture/OCR/clipboard actions only from explicit user gestures on a
//! background worker. Portable helpers are bounded to 30 seconds; native Vision
//! has no hard wall-clock timeout. Merely opening a preview must never read the
//! clipboard, take a screenshot, invoke OCR, or start an updater.
//! Text/image adapters stay local and never persist clipboard/OCR text. The
//! cfg-gated Sparkle controller uses network only after an explicit update action.
//! Conservative native AX selection, Vision OCR and Sparkle APIs are implemented
//! for macOS but not runtime-tested here. Selection replacement, global hotkeys,
//! recording and Keychain remain unimplemented.

mod backend;
mod capture;
#[cfg(target_os = "macos")]
pub mod macos_native;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod process;

use std::fmt;
use std::path::Path;

pub use capture::CapturedScreenshot;

/// A captured PNG kept in memory after a private, automatically removed staging file.
/// No user-visible image file is saved until the user explicitly exports it.
pub struct ScreenshotSnapshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub backend: &'static str,
}

/// Maximum clipboard text accepted or returned, in UTF-8 bytes (500 KiB).
pub const MAX_CLIPBOARD_BYTES: usize = 500 * 1024;
/// Maximum image file accepted for local OCR (32 MiB).
pub const MAX_OCR_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum UTF-8 OCR output (1 MiB).
pub const MAX_OCR_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    ClipboardRead,
    ClipboardWrite,
    Screenshot,
    LocalOcr,
    PermissionStatus,
    SelectionRead,
    SelectionReplace,
    GlobalShortcut,
    ScreenRecording,
    CredentialStore,
    AutomaticUpdates,
}

impl Capability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClipboardRead => "Read clipboard",
            Self::ClipboardWrite => "Write clipboard",
            Self::Screenshot => "Full-screen PNG screenshot",
            Self::LocalOcr => "Local OCR",
            Self::PermissionStatus => "Permission status",
            Self::SelectionRead => "Read selection",
            Self::SelectionReplace => "Replace selection",
            Self::GlobalShortcut => "Global shortcut",
            Self::ScreenRecording => "Screen recording",
            Self::CredentialStore => "Secure credential storage",
            Self::AutomaticUpdates => "Automatic updates",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    /// The adapter is implemented and its prerequisites were found. Permission
    /// and display-server support still have to be established by the action.
    Implemented,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityStatus {
    pub capability: Capability,
    pub state: CapabilityState,
    pub backend: Option<String>,
    pub reason: String,
}

impl CapabilityStatus {
    pub const fn is_available(&self) -> bool {
        matches!(self.state, CapabilityState::Implemented)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySession {
    MacOS,
    Wayland,
    X11,
    Headless,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ScreenCapture,
    Accessibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    /// Not granted according to a read-only macOS preflight. This intentionally
    /// does not distinguish not-yet-asked, denied, and restricted.
    NotGranted,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionStatus {
    pub permission: Permission,
    pub state: PermissionState,
    pub reason: String,
    /// A system settings destination, not a claim that authorization was granted.
    pub settings_url: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentStatus {
    pub operating_system: &'static str,
    pub display_session: DisplaySession,
    pub capabilities: Vec<CapabilityStatus>,
    pub permissions: Vec<PermissionStatus>,
}

impl EnvironmentStatus {
    pub fn capability(&self, capability: Capability) -> Option<&CapabilityStatus> {
        self.capabilities
            .iter()
            .find(|status| status.capability == capability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Unavailable,
    InvalidInput,
    InputTooLarge,
    OutputTooLarge,
    TimedOut,
    BackendFailed,
    Io,
    InvalidOutput,
    AlreadyExists,
}

/// Diagnostics intentionally omit process output, clipboard text, image paths,
/// and OCR text. UI error reporting must not accidentally expose those values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub kind: ErrorKind,
    pub operation: &'static str,
    pub message: String,
    pub exit_code: Option<i32>,
}

impl PlatformError {
    pub(crate) fn new(
        kind: ErrorKind,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            operation,
            message: message.into(),
            exit_code: None,
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.message)?;
        if let Some(code) = self.exit_code {
            write!(f, " (exit {code})")?;
        }
        Ok(())
    }
}

impl std::error::Error for PlatformError {}

pub type Result<T> = std::result::Result<T, PlatformError>;

/// Stateless facade. Construction has no I/O and never asks for permission.
#[derive(Debug, Default, Clone, Copy)]
pub struct Platform;

impl Platform {
    pub const fn new() -> Self {
        Self
    }

    /// Reads environment/tool availability and, on macOS, non-prompting permission
    /// preflights. Never reads selection/clipboard, captures, or starts a process.
    pub fn status(&self) -> EnvironmentStatus {
        backend::status()
    }

    /// Explicit clipboard import. Does not synthesize Copy or inspect selection.
    pub fn read_clipboard_text(&self) -> Result<String> {
        backend::read_clipboard()
    }

    /// Explicit clipboard export. Does not paste into any other application.
    pub fn write_clipboard_text(&self, text: &str) -> Result<()> {
        if text.len() > MAX_CLIPBOARD_BYTES {
            return Err(PlatformError::new(
                ErrorKind::InputTooLarge,
                "Write clipboard",
                "Text exceeds the 500 KiB clipboard limit.",
            ));
        }
        backend::write_clipboard(text)
    }

    /// Captures the full virtual screen (the main display on macOS) as a PNG.
    /// Destination must end in `.png` and must not exist. Uses a private staging
    /// directory and publishes without replacing an existing file. No clipboard
    /// changes, area/window selection, or scrolling capture are performed.
    pub fn capture_screenshot(&self, destination: &Path) -> Result<CapturedScreenshot> {
        capture::capture(destination)
    }

    /// Explicit full-screen capture for the annotation editor. The private
    /// staging file is removed before this function returns, including on error.
    pub fn capture_screenshot_snapshot(&self) -> Result<ScreenshotSnapshot> {
        capture::snapshot()
    }

    /// Publish an explicitly exported PNG atomically, without overwriting any
    /// existing file or leaving a partial destination if writing fails.
    pub fn save_png_bytes(&self, destination: &Path, png: &[u8]) -> Result<()> {
        capture::save_png(destination, png)
    }

    /// Explicit local OCR: Apple Vision on macOS, Tesseract on Linux.
    /// No image is uploaded. Native Vision must run on a worker and has no hard timeout.
    pub fn recognize_text(&self, image: &Path, languages: Option<&str>) -> Result<String> {
        let image = validate_ocr_input(image)?;
        self.recognize_image_bytes(&image, languages)
    }

    /// Recognize an already rendered, crop/redaction-aware local image. This
    /// avoids writing an OCR copy to disk or reopening the original screenshot.
    /// No provider request is made by this API on any platform.
    pub fn recognize_image_bytes(&self, image: &[u8], languages: Option<&str>) -> Result<String> {
        if image.is_empty()
            || image.len() as u64 > MAX_OCR_IMAGE_BYTES
            || !is_supported_image(image)
        {
            return Err(PlatformError::new(
                ErrorKind::InvalidInput,
                "Local OCR",
                "Provide a supported image of at most 32 MiB; image-list files are not accepted.",
            ));
        }
        #[cfg(target_os = "macos")]
        {
            macos_native::recognize_text(image, languages)
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(languages) = languages {
                validate_languages(languages)?;
            }
            backend::recognize_text(image, languages)
        }
    }

    /// Explicitly opens macOS Privacy & Security settings. It does not grant a
    /// permission, prompt for access, or implement Accessibility selection APIs.
    pub fn open_permission_settings(&self, permission: Permission) -> Result<()> {
        backend::open_permission_settings(permission)
    }
}

#[cfg(any(not(target_os = "macos"), test))]
fn validate_languages(languages: &str) -> Result<()> {
    if languages.is_empty()
        || languages.len() > 128
        || !languages.split('+').all(|part| {
            !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
    {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            "Local OCR",
            "Use language identifiers such as eng or eng+fra (128 bytes maximum).",
        ));
    }
    Ok(())
}

fn validate_ocr_input(image: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(image).map_err(|_| {
        PlatformError::new(
            ErrorKind::Io,
            "Local OCR",
            "The selected image could not be opened.",
        )
    })?;
    let metadata = file.metadata().map_err(|_| {
        PlatformError::new(
            ErrorKind::Io,
            "Local OCR",
            "The selected image could not be inspected.",
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            "Local OCR",
            "Choose a nonempty local image file.",
        ));
    }
    if metadata.len() > MAX_OCR_IMAGE_BYTES {
        return Err(PlatformError::new(
            ErrorKind::InputTooLarge,
            "Local OCR",
            "Image exceeds the 32 MiB OCR input limit.",
        ));
    }
    // Snapshot from the same open file, with a byte limit even if it grows.
    // Feeding validated bytes over stdin avoids a path-replacement race and
    // prevents Tesseract's image-list feature from reading unrequested paths.
    let mut bytes = Vec::new();
    file.take(MAX_OCR_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            PlatformError::new(
                ErrorKind::Io,
                "Local OCR",
                "The selected image could not be read.",
            )
        })?;
    if bytes.len() as u64 > MAX_OCR_IMAGE_BYTES {
        return Err(PlatformError::new(
            ErrorKind::InputTooLarge,
            "Local OCR",
            "Image exceeds the 32 MiB OCR input limit.",
        ));
    }
    if !is_supported_image(&bytes) {
        return Err(PlatformError::new(
            ErrorKind::InvalidInput,
            "Local OCR",
            "Choose a PNG, JPEG, TIFF, BMP, GIF, or WebP image; image-list files are not accepted.",
        ));
    }
    Ok(bytes)
}

fn is_supported_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_ocr_rejects_nonimages_before_starting_a_backend() {
        let platform = Platform::new();
        assert_eq!(
            platform
                .recognize_image_bytes(b"/tmp/private-file.png\n", None)
                .unwrap_err()
                .kind,
            ErrorKind::InvalidInput
        );
        assert_eq!(
            platform.recognize_image_bytes(&[], None).unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn language_identifiers_are_literal_and_bounded() {
        for valid in ["eng", "eng+fra", "chi_sim", "srp_latn+deu"] {
            assert!(validate_languages(valid).is_ok());
        }
        for invalid in [
            "",
            "+eng",
            "eng+",
            "eng++fra",
            "../eng",
            "--psm",
            "eng fra",
            "eng;touch",
            "eng\nfra",
            "日本語",
        ] {
            assert!(validate_languages(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_languages(&"a".repeat(128)).is_ok());
        assert!(validate_languages(&"a".repeat(129)).is_err());
    }

    #[test]
    fn reject_image_lists_and_unknown_formats() {
        assert!(!is_supported_image(b"/private/image.png\n"));
        assert!(!is_supported_image(b"%PDF-1.7"));
        assert!(!is_supported_image(b""));
        for signature in [
            &b"\x89PNG\r\n\x1a\n"[..],
            b"\xff\xd8\xff",
            b"II*\0",
            b"MM\0*",
            b"BM",
            b"GIF89a",
            b"RIFF1234WEBP",
        ] {
            assert!(is_supported_image(signature));
        }
    }

    #[test]
    fn oversized_clipboard_write_rejected_before_any_backend() {
        let error = Platform::new()
            .write_clipboard_text(&"x".repeat(MAX_CLIPBOARD_BYTES + 1))
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InputTooLarge);
    }

    #[test]
    fn capability_lookup_is_typed() {
        let status = EnvironmentStatus {
            operating_system: "test",
            display_session: DisplaySession::Headless,
            permissions: vec![],
            capabilities: vec![CapabilityStatus {
                capability: Capability::ScreenRecording,
                state: CapabilityState::Unavailable,
                backend: None,
                reason: "Not implemented.".into(),
            }],
        };
        assert!(!status
            .capability(Capability::ScreenRecording)
            .unwrap()
            .is_available());
        assert!(status.capability(Capability::ClipboardRead).is_none());
    }
}
