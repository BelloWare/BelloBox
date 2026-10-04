# bello-platform

Dependency-free, explicit platform adapters for the incremental BelloBox port.
`Platform::new()` has no side effects. `status()` only inspects environment/tool
availability and read-only macOS permission preflights. Actions are synchronous:
run them off the GPUI thread, only from explicit user gestures.

| Action | Linux Wayland | Linux X11 | macOS |
| --- | --- | --- | --- |
| Read/write clipboard text | wl-paste / wl-copy | xclip | pbpaste / pbcopy |
| Full-screen PNG capture | grim (screencopy-capable compositor) | ImageMagick import | screencapture (main display) |
| Local image OCR | Tesseract | Tesseract | Tesseract |
| Read-only permission status | unavailable | unavailable | CoreGraphics / Accessibility preflight |

Presence of a helper is reported as an implemented capability; permission and
display-server support are **not** assumed. Missing helpers have actionable
unavailable reasons. On Wayland there is no implicit Xwayland fallback. All
helpers are searched only in absolute PATH directories; macOS system helpers use
their fixed system paths. The application does not install any helper.

Clipboard reads preserve trailing newlines and reject non-UTF-8 output rather
than silently changing text. Clipboard writes are separate from paste. Linux
clipboard tools can retain a background owner process after a successful write.

Capture requires a fresh `.png` path in an existing writable directory. It uses
a private sibling staging directory, bounds and checks the PNG header/ending,
sets owner-only file permissions, and publishes atomically with a hard link.
Existing files and dangling symlinks are never replaced. The destination
filesystem must support hard links. Areas, windows, scrolling, and editing are
not implemented by this crate.

OCR accepts one existing PNG, JPEG, TIFF, BMP, GIF, or WebP file with a matching
signature. It rejects text image-list files so Tesseract cannot implicitly read
additional paths. Validated image bytes are supplied over stdin without reopening
the path. Language strings are validated identifiers (`eng`, `eng+fra`,
`chi_sim`), never shell commands/config arguments. Installed Tesseract language
data is required. Apple Vision and provider/LLM OCR are not implemented.

Bounds: clipboard 500 KiB; OCR input 32 MiB and text output 1 MiB; screenshot
100 MiB, at most 32768 pixels per axis and 200 megapixels; helper stderr 64 KiB.
Timeouts: clipboard/settings 3 seconds, capture 15 seconds, OCR 30 seconds.
Subprocesses use literal argument arrays, nonblocking pipes and isolated process
groups. Failure/timeout kills that helper group. Process output is not copied into
errors or logs. This crate has no networking, credentials, or text persistence.

macOS permission checks never ask for authorization. An explicit settings action
can open the relevant Privacy & Security pane, but cannot grant permission.
Permission state does not imply feature implementation. Selection capture and
replacement, global shortcut/monitoring, recording/GIF, Keychain/Secret Service,
and Sparkle/updating are explicitly unavailable.

Default unit tests are pure or reject invalid input before any platform action.
Three opt-in ignored subprocess tests use cat, sleep, and yes to check large
stdin, timeouts, and output bounds. None read/write the system clipboard, capture
the screen, request permissions, or run OCR. Runtime desktop integration still
needs manual verification on the relevant compositor and macOS.

## Native macOS bridge checkpoint

`macos_native.rs` now provides:

- `SparkleUpdater::new/check_for_updates`: a retained main-thread-only Sparkle
  2.8.1 controller. Construction remains stopped; explicit checking starts its
  standard UI. Only the framework inside this application's bundle can load.
  The Rust appcast is validated and the known Swift production feed is rejected.
- `sparkle_availability`: read-only bundle/framework/feed/key preflight.
- `recognize_text`: local native Vision OCR (now used by `Platform` on macOS).
  Validates encoded bytes and dimensions, caps 40 MP/16,384 pixels per axis and
  1 MiB/4,096 text regions. Runs on a worker and has no hard wall-clock timeout.
- `read_selected_text`: conservative explicit AXSelectedText reading with
  focused source identity/window/range/ancestor safety checks and a 160 ms budget.
  Rejects own-app focus, secure or uninspectable paths, carets and stale focus.
  No whole-document, marker-range, clipboard or synthesized-input fallback.

`cargo check --tests --target aarch64-apple-darwin` and cross-target Clippy with
`-D warnings` pass. These are Rust compile/type checks only: no Apple SDK linking,
macOS runtime, permissions, VoiceOver, Vision recognition, updater signing,
appcast download or installation has been tested. Three native-only pure test
functions compile but have not executed. Linux platform tests still pass.

This checkpoint supersedes the earlier native-Vision/AX/Sparkle-unavailable
statements above; complete native interaction parity is still not implemented.
