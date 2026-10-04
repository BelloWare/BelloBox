use crate::*;
#[cfg(any(target_os = "linux", test))]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    WlPaste,
    WlCopy,
    Xclip,
    Grim,
    Import,
    PbPaste,
    PbCopy,
    ScreenCapture,
    Tesseract,
    Open,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::WlPaste => "wl-paste",
            Self::WlCopy => "wl-copy",
            Self::Xclip => "xclip",
            Self::Grim => "grim",
            Self::Import => "import",
            Self::PbPaste => "pbpaste",
            Self::PbCopy => "pbcopy",
            Self::ScreenCapture => "screencapture",
            Self::Tesseract => "tesseract",
            Self::Open => "open",
        }
    }
}

#[derive(Debug, Clone)]
struct Backend {
    tool: Tool,
    executable: PathBuf,
}

#[derive(Debug)]
struct Detection {
    session: DisplaySession,
    clipboard_read: Option<Backend>,
    clipboard_write: Option<Backend>,
    screenshot: Option<Backend>,
    ocr: Option<Backend>,
}

fn session() -> DisplaySession {
    #[cfg(target_os = "macos")]
    {
        DisplaySession::MacOS
    }
    #[cfg(target_os = "linux")]
    {
        classify_linux_session(
            std::env::var_os("WAYLAND_DISPLAY").as_deref(),
            std::env::var_os("DISPLAY").as_deref(),
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        DisplaySession::Unsupported
    }
}

#[cfg(any(target_os = "linux", test))]
fn classify_linux_session(wayland: Option<&OsStr>, x11: Option<&OsStr>) -> DisplaySession {
    if wayland.is_some_and(|value| !value.is_empty()) {
        DisplaySession::Wayland
    } else if x11.is_some_and(|value| !value.is_empty()) {
        DisplaySession::X11
    } else {
        DisplaySession::Headless
    }
}

fn candidates(session: DisplaySession) -> (Option<Tool>, Option<Tool>, Option<Tool>) {
    match session {
        DisplaySession::MacOS => (
            Some(Tool::PbPaste),
            Some(Tool::PbCopy),
            Some(Tool::ScreenCapture),
        ),
        DisplaySession::Wayland => (Some(Tool::WlPaste), Some(Tool::WlCopy), Some(Tool::Grim)),
        DisplaySession::X11 => (Some(Tool::Xclip), Some(Tool::Xclip), Some(Tool::Import)),
        _ => (None, None, None),
    }
}

fn detect() -> Detection {
    let session = session();
    let (read, write, screenshot) = candidates(session);
    Detection {
        session,
        clipboard_read: read.and_then(find_tool),
        clipboard_write: write.and_then(find_tool),
        screenshot: screenshot.and_then(find_tool),
        ocr: find_tool(Tool::Tesseract),
    }
}

fn find_tool(tool: Tool) -> Option<Backend> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return None;
    }
    let fixed = match tool {
        Tool::PbPaste => Some("/usr/bin/pbpaste"),
        Tool::PbCopy => Some("/usr/bin/pbcopy"),
        Tool::ScreenCapture => Some("/usr/sbin/screencapture"),
        Tool::Open => Some("/usr/bin/open"),
        _ => None,
    };
    let executable = if let Some(path) = fixed {
        let path = PathBuf::from(path);
        is_executable(&path).then_some(path)
    } else {
        let paths = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&paths)
            .filter(|p| p.is_absolute())
            .map(|p| p.join(tool.name()))
            .find(|path| is_executable(path))
    }?;
    Some(Backend { tool, executable })
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub(crate) fn status() -> EnvironmentStatus {
    let detection = detect();
    let mut capabilities = vec![
        capability(
            Capability::ClipboardRead,
            detection.clipboard_read.as_ref(),
            "Explicit text import only; no synthetic Copy. Display-server access is checked when used.",
            "Needs wl-paste in a Wayland session, xclip in an X11 session, or pbpaste on macOS.",
        ),
        capability(
            Capability::ClipboardWrite,
            detection.clipboard_write.as_ref(),
            "Explicit text export only; does not paste into another app. Clipboard helpers may retain a background owner process.",
            "Needs wl-copy in a Wayland session, xclip in an X11 session, or pbcopy on macOS.",
        ),
        capability(
            Capability::Screenshot,
            detection.screenshot.as_ref(),
            "Explicit full-screen PNG capture. macOS captures its main display; grim requires compositor screencopy support. Permissions are not implied by helper detection.",
            "Needs grim in a supported Wayland compositor, ImageMagick import in X11, or screencapture on macOS.",
        ),
        capability(
            Capability::LocalOcr,
            detection.ocr.as_ref(),
            "Explicit local-image Tesseract OCR; requires installed language data. No Apple Vision or provider OCR.",
            "Tesseract was not found on the executable search path. Install it and the desired language data.",
        ),
    ];
    capabilities.push(CapabilityStatus {
        capability: Capability::PermissionStatus,
        state: if cfg!(target_os = "macos") { CapabilityState::Implemented } else { CapabilityState::Unavailable },
        backend: if cfg!(target_os = "macos") { Some("macOS read-only preflight".into()) } else { None },
        reason: if cfg!(target_os = "macos") { "Read-only Screen Recording and Accessibility checks; permissions are never requested automatically." }
            else { "No portable Linux permission preflight is implemented; the compositor/display server controls access." }.into(),
    });
    for (kind, reason) in [
        (
            Capability::SelectionRead,
            "Native Accessibility selection capture has not been ported. Use explicit clipboard import.",
        ),
        (
            Capability::SelectionReplace,
            "Native selection replacement has not been ported. Use explicit Copy and paste manually.",
        ),
        (
            Capability::GlobalShortcut,
            "Global shortcut registration and selection monitoring have not been ported.",
        ),
        (
            Capability::ScreenRecording,
            "Screen/audio recording and GIF export have not been ported.",
        ),
        (
            Capability::CredentialStore,
            "Keychain/Secret Service integration has not been ported; this crate stores no credentials.",
        ),
        (
            Capability::AutomaticUpdates,
            "Sparkle/automatic update integration has not been ported.",
        ),
    ] {
        capabilities.push(CapabilityStatus {
            capability: kind,
            state: CapabilityState::Unavailable,
            backend: None,
            reason: reason.into(),
        });
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(ocr) = capabilities
            .iter_mut()
            .find(|c| c.capability == Capability::LocalOcr)
        {
            ocr.state = CapabilityState::Implemented;
            ocr.backend = Some("Apple Vision".into());
            ocr.reason = "Local native Vision OCR adapter. macOS runtime validation is pending; run on a worker. No network.".into();
        }
        if let Some(selection) = capabilities
            .iter_mut()
            .find(|c| c.capability == Capability::SelectionRead)
        {
            selection.state = CapabilityState::Implemented;
            selection.backend = Some("macOS Accessibility direct selected text".into());
            selection.reason = "Conservative explicit AX reader implemented, not runtime-tested. Requires external-app focus and Accessibility permission; no marker-range or document fallback, no global hotkey UI.".into();
        }
        if let Some(updates) = capabilities
            .iter_mut()
            .find(|c| c.capability == Capability::AutomaticUpdates)
        {
            match crate::macos_native::sparkle_availability() {
                Ok(()) => {
                    updates.state = CapabilityState::Implemented;
                    updates.backend = Some("Sparkle 2.8.1".into());
                    updates.reason = "Bundled Sparkle configuration detected. Updates begin only from an explicit check; macOS runtime validation pending.".into();
                }
                Err(error) => updates.reason = error.to_string(),
            }
        }
    }
    EnvironmentStatus {
        operating_system: std::env::consts::OS,
        display_session: detection.session,
        capabilities,
        permissions: permission_statuses(),
    }
}

fn capability(
    kind: Capability,
    backend: Option<&Backend>,
    ready: &str,
    missing: &str,
) -> CapabilityStatus {
    CapabilityStatus {
        capability: kind,
        state: if backend.is_some() {
            CapabilityState::Implemented
        } else {
            CapabilityState::Unavailable
        },
        backend: backend.map(|b| b.tool.name().into()),
        reason: if backend.is_some() { ready } else { missing }.into(),
    }
}

fn required(
    backend: Option<Backend>,
    operation: &'static str,
    message: &'static str,
) -> Result<Backend> {
    backend.ok_or_else(|| PlatformError::new(ErrorKind::Unavailable, operation, message))
}

pub(crate) fn read_clipboard() -> Result<String> {
    let backend = required(
        detect().clipboard_read,
        "Read clipboard",
        "No supported clipboard reader is available for this display session.",
    )?;
    let args: &[&str] = match backend.tool {
        Tool::WlPaste => &["--no-newline", "--type", "text"],
        Tool::Xclip => &["-selection", "clipboard", "-out", "-target", "UTF8_STRING"],
        Tool::PbPaste => &["-Prefer", "txt"],
        _ => unreachable!("clipboard backend"),
    };
    let bytes = execute(
        &backend,
        &os_args(args),
        None,
        MAX_CLIPBOARD_BYTES,
        Duration::from_secs(3),
        "Read clipboard",
    )?;
    decode_text(bytes, "Read clipboard")
}

pub(crate) fn write_clipboard(text: &str) -> Result<()> {
    let backend = required(
        detect().clipboard_write,
        "Write clipboard",
        "No supported clipboard writer is available for this display session.",
    )?;
    let args: &[&str] = match backend.tool {
        Tool::WlCopy => &["--type", "text/plain;charset=utf-8"],
        Tool::Xclip => &["-selection", "clipboard", "-in", "-target", "UTF8_STRING"],
        Tool::PbCopy => &[],
        _ => unreachable!("clipboard backend"),
    };
    execute(
        &backend,
        &os_args(args),
        Some(text.as_bytes()),
        1024,
        Duration::from_secs(3),
        "Write clipboard",
    )
    .map(|_| ())
}

pub(crate) fn capture_to(path: &Path) -> Result<&'static str> {
    let backend = required(
        detect().screenshot,
        "Capture screenshot",
        "No supported screenshot helper is available for this display session.",
    )?;
    let args = screenshot_args(backend.tool, path);
    execute(
        &backend,
        &args,
        None,
        1024,
        Duration::from_secs(15),
        "Capture screenshot",
    )?;
    Ok(backend.tool.name())
}

fn screenshot_args(tool: Tool, path: &Path) -> Vec<OsString> {
    match tool {
        Tool::Grim => {
            let mut args = os_args(&["-t", "png"]);
            args.push(path.as_os_str().to_owned());
            args
        }
        Tool::Import => {
            let mut args = os_args(&["-window", "root"]);
            let mut output = OsString::from("png:");
            output.push(path);
            args.push(output);
            args
        }
        Tool::ScreenCapture => {
            let mut args = os_args(&["-x", "-m", "-t", "png"]);
            args.push(path.as_os_str().to_owned());
            args
        }
        _ => unreachable!("screenshot backend"),
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn recognize_text(image: &[u8], languages: Option<&str>) -> Result<String> {
    let backend = required(
        find_tool(Tool::Tesseract),
        "Local OCR",
        "Tesseract was not found; install it and the desired language data.",
    )?;
    let mut args = os_args(&["stdin", "stdout"]);
    if let Some(languages) = languages {
        args.extend(os_args(&["-l", languages]));
    }
    let bytes = execute(
        &backend,
        &args,
        Some(image),
        MAX_OCR_TEXT_BYTES,
        Duration::from_secs(30),
        "Local OCR",
    )?;
    decode_text(bytes, "Local OCR")
}

fn decode_text(bytes: Vec<u8>, operation: &'static str) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| {
        PlatformError::new(
            ErrorKind::InvalidOutput,
            operation,
            "The helper returned non-UTF-8 text; no lossy conversion was applied.",
        )
    })
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn execute(
    backend: &Backend,
    args: &[OsString],
    input: Option<&[u8]>,
    max_output: usize,
    timeout: Duration,
    operation: &'static str,
) -> Result<Vec<u8>> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        crate::process::run(
            &backend.executable,
            args,
            input,
            max_output,
            timeout,
            operation,
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (backend, args, input, max_output, timeout);
        Err(PlatformError::new(
            ErrorKind::Unavailable,
            operation,
            "This operating system is not supported.",
        ))
    }
}

fn settings_url(permission: Permission) -> &'static str {
    match permission {
        Permission::ScreenCapture => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        Permission::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
    }
}

pub(crate) fn open_permission_settings(permission: Permission) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(PlatformError::new(
            ErrorKind::Unavailable,
            "Open permission settings",
            "System privacy settings links are implemented only on macOS.",
        ));
    }
    let backend = required(
        find_tool(Tool::Open),
        "Open permission settings",
        "The macOS open helper is unavailable.",
    )?;
    execute(
        &backend,
        &os_args(&[settings_url(permission)]),
        None,
        1024,
        Duration::from_secs(3),
        "Open permission settings",
    )
    .map(|_| ())
}

fn permission_statuses() -> Vec<PermissionStatus> {
    [Permission::ScreenCapture, Permission::Accessibility].into_iter().map(|permission| {
        #[cfg(target_os = "macos")]
        let granted = macos_permission_granted(permission);
        #[cfg(target_os = "macos")]
        { PermissionStatus { permission, state: if granted { PermissionState::Granted } else { PermissionState::NotGranted },
            reason: "Read-only preflight for this process; no permission was requested. Accessibility actions themselves are not implemented.".into(),
            settings_url: Some(settings_url(permission)) } }
        #[cfg(not(target_os = "macos"))]
        { PermissionStatus { permission, state: PermissionState::NotApplicable,
            reason: "No portable permission preflight is implemented on this operating system.".into(), settings_url: None } }
    }).collect()
}

#[cfg(target_os = "macos")]
fn macos_permission_granted(permission: Permission) -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
    }
    // SAFETY: These Apple APIs have no arguments, return a C Boolean/bool, and
    // perform read-only permission checks. Both are available on macOS 10.15+.
    unsafe {
        match permission {
            Permission::ScreenCapture => CGPreflightScreenCaptureAccess(),
            Permission::Accessibility => AXIsProcessTrusted() != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_detection_requires_a_nonempty_display_and_prefers_wayland() {
        assert_eq!(classify_linux_session(None, None), DisplaySession::Headless);
        assert_eq!(
            classify_linux_session(Some(OsStr::new("")), None),
            DisplaySession::Headless
        );
        assert_eq!(
            classify_linux_session(None, Some(OsStr::new(":0"))),
            DisplaySession::X11
        );
        assert_eq!(
            classify_linux_session(Some(OsStr::new("wayland-0")), Some(OsStr::new(":0"))),
            DisplaySession::Wayland
        );
    }

    #[test]
    fn backend_selection_never_uses_xwayland_clipboard_as_an_implicit_fallback() {
        assert_eq!(
            candidates(DisplaySession::Wayland),
            (Some(Tool::WlPaste), Some(Tool::WlCopy), Some(Tool::Grim))
        );
        assert_eq!(candidates(DisplaySession::Headless), (None, None, None));
    }

    #[test]
    fn screenshot_destination_is_one_literal_argument() {
        let path = Path::new("/tmp/a folder/$(not-a-command).png");
        assert_eq!(
            screenshot_args(Tool::Grim, path),
            os_args(&["-t", "png", "/tmp/a folder/$(not-a-command).png"])
        );
        assert_eq!(
            screenshot_args(Tool::Import, path),
            os_args(&["-window", "root", "png:/tmp/a folder/$(not-a-command).png"])
        );
        assert_eq!(
            screenshot_args(Tool::ScreenCapture, path),
            os_args(&[
                "-x",
                "-m",
                "-t",
                "png",
                "/tmp/a folder/$(not-a-command).png"
            ])
        );
    }

    #[test]
    fn output_is_not_silently_repaired_or_trimmed() {
        assert_eq!(
            decode_text(b"line\n\n".to_vec(), "test").unwrap(),
            "line\n\n"
        );
        assert_eq!(
            decode_text(vec![0xff], "test").unwrap_err().kind,
            ErrorKind::InvalidOutput
        );
    }
}
