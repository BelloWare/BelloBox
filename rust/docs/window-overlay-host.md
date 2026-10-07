# Supplied-pixel Window overlay, catalog and independent refresh

The DEBUG `BELLOBOX_INLINE_WINDOW_FIXTURE=1` route enters the existing owned
main-display coordinator, freezes app-supplied pixels, selects a window, mounts
`ScreenshotEditor` inside the same overlay, and performs a guarded independent
image refresh. Use the existing `BELLOBOX_TOOL=screenshot` route and an isolated
`BELLOBOX_CONFIG_DIR`. Do not use the ordinary Screen action for this fixture.
Production Area stays disabled and the public Window acquisition function still
returns Unavailable before enumeration, permission checks or native capture.

## Source behavior and deliberate scope

References are `CaptureOverlayController.swift:221–264,470–598,839–849,1544–1665`,
`CaptureWindowCatalog.swift`, `CaptureSelectionResolver.swift`,
`AnnotationCanvasView.swift` and `AnnotationModel.swift:308–357`.

The shared selector supports a Window policy alongside its accepted Area policy.
A mouse-down updates and latches the first eligible front-to-back window; both
final clamped drag axes must remain below eight points. Blank clicks and longer
drags never fall through to Area or Screen. Window materialization physically
copies only the selected frozen rectangle on the counted worker. Its editor base
contains no full-display image, initial crop, annotations or Undo state; Crop
Reset and history cannot expose another window or the rest of the display.
The overlay separately retains frozen background tiles until retirement.

The Window frame, border and toolbar stay fixed after editor Crop and Undo.
There are no Area adjustment handles or Select-move. The visible image is centered
with a uniform aspect-fit scale in that fixed frame, matching `ImageViewport`.
Preview pixel tiles are clipped to the fitted image; letterboxes use the well
color, never excluded crop pixels. The fixed frame owns pointer gestures, including
letterbox-to-image clamping. Text fields and label drag controls retain their
existing propagation and may extend into padding within the fixed frame.
Area keeps direct independent pixel-ratio multiplication and its existing crop,
move, handles and toolbar behavior.

This remains the deliberately narrower normal-layer, wholly-contained,
unrotated-main-display policy. Native catalog acquisition, visible-frame system
surfaces, spanning-window composition and fallback are not implemented here.

## Supplied evidence and refresh boundary

`window_workflow` separates bounded complete selector observations from raw
occlusion rows. Exact f64 selected frame/identity and full display topology are
retained separately from f32 UI rectangles. Selector candidates preserve supplied
front-to-back order and exclude the owning process. Malformed or duplicate complete
identities fail closed. The independent raw occlusion catalog preserves missing
fields and the Swift/core skip behavior, retains own regular windows and source
levels, and excludes own screen-saver overlays. It is not reconstructed from the
narrower selector list. Missing frame and unavailable catalog remain distinct
source decisions.

The supplied acquisition binds an immutable submission record to the exact
supplied generator argument, calls the existing `WindowCaptureSelection::plan`
before generating pixels, validates fresh complete evidence and output dimensions
afterward, and repeats completion validation at foreground publication. No native
objects cross queues. Source evidence is never locked across image generation.
Original coordinator and operation cancellation, source-session generation,
identity/frame/full-topology changes, malformed output and deadline expiry reject
publication. These synthetic snapshots do not prove an atomic native window
incarnation, original color space or any ScreenCaptureKit callback behavior.

Unoccluded front Window retains frozen orange RGB and borrows rounded alpha from
a deliberately purple independent input. Occluded back Window replaces its frozen
orange overlap with independent blue pixels. The existing normalized SDR native
CoreGraphics mask adapter runs on macOS; the deterministic portable sampler remains
explicitly approximate for +/-1-pixel resampling. See
[native alpha scope](native-alpha-oracle.md) for the unchanged exact oracle and
its capture/color limits.

The existing clean-state, edit-session and monotonic base-epoch rules still govern
replacement. Current annotations/crop, active gestures/text/label movement,
export/discard, newer jobs and cancelled/stale owners reject replacement. Failure
leaves the frozen editor usable. Success uses `changed()` to cancel OCR, invalidate
copy data and rebuild preview, without adding an Undo entry. No Auto Copy or
provider action is introduced.

## Counted lifetime and retirement

Acquisition, masking and completion disposal hold one coordinator worker admission
until actual physical completion, independent of editor entity or native window
liveness. A cancelled owner cannot start uncounted new work. Prepared refresh now
has a shallow Clone seam so the completion future can retain immutable pixel
ownership while foreground `apply()` consumes a copy. The old base is retained
similarly. Both retained owners are disposed on a counted background task before
Busy can clear; stale, rejected or dead-entity deliveries use the same drain.
Cloning is not retry permission: the first successful apply advances the base
epoch and every cloned later apply is rejected, including after Undo to clean.
Opaque GPU/framework allocations retain their previously documented limits.

## Verification

The final-source affected suites pass 86 screenshot application tests and 168 core
screenshot tests. The new tests include 16 supplied-evidence cases and eight
same-coordinator Window cases, plus an Area anisotropic pointer regression and
core retained-clone/base-epoch/disposal regression. They cover both refresh
choices, exact physical crop/export pixels, fixed-frame crop/letterbox input,
front-order/click policy, failed and changed source retention, coordinator topology
and navigation retirement, and cancelled Busy ownership through a deterministically
paused actual background-disposal future. Existing Area and Window refresh tests,
including the worker-only native mask test path, remain present.

Strict application all-target Clippy, core test-target Clippy, normal build,
minimal-feature check and formatting pass. The exact app-only test harness with
debug assertions disabled ran 46 screenshot tests successfully. Actual supplied-
pixel Linux GUI acceptance passed for front/back refresh, fixed-frame Crop and
Undo/Redo, clean letterboxes, edge text input, Copy & Finish, body-only clipboard
reimport and clean close. The observed reimport dimensions were 401×161; pointer/
layout fractions were not traced in that manual run. Full scope, five original
JPEGs and exact source/binary hashes are in the
[acceptance report](validation/window-overlay-2026-10-07/QA-report.md).
Exact new-commit native CI remains a separate publication gate.

The source-reviewed delta is 178 production and 2,040 test/support nonblank Rust
lines, including comments: totals 44,451 / 23,705 / 105 benchmark. DEBUG-only
supplied evidence and its tests are support. The explicit spans and hashes are in
[loc-delta.json](validation/window-overlay-2026-10-07/loc-delta.json); these counts
are neither a parity percentage nor performance evidence. The bounded supplied
catalog retains a quadratic duplicate check for at most 512 rows; no unmeasured
responsiveness claim is made from that bound.
No native permissions, real screen pixels, Mac access, release signing, provider
requests, spending or production gate enablement are part of this checkpoint.
