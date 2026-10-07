# Synthetic Window refresh through the real editor

The DEBUG fixture now has an opt-in Window refresh vertical. Launch with
`BELLOBOX_WINDOW_FIXTURE=1 BELLOBOX_WINDOW_REFRESH_FIXTURE=1` and open Screenshot
(or use the existing `BELLOBOX_TOOL=screenshot` route). Without the refresh flag,
the original frozen-only fixture is unchanged. These flags generate app-owned
pixels; they do not enumerate or capture native windows or request permissions.

## Source behavior and visible result

The source is `CaptureOverlayController.refreshWindowScreenshotIfNeeded`,
`CaptureWindowCatalog.isOccluded`, `ImageAlphaMask`, and
`ScreenshotPopupViewModel.refreshBaseCapture`.

The orange front window remains its frozen orange pixels but borrows rounded,
transparent corners from its synthetic independent shape. The independent input
is deliberately purple, making accidental full replacement observable. The blue
back window is occluded by orange in the frozen display, so its independent blue
image replaces the complete frozen window crop and removes the orange overlap.
Dimensions stay 680×540 for front, 840×460 for back. The editor subtitle identifies
synthetic alpha-on-frozen versus independent pixels after successful acceptance.

The supplied synthetic occluder catalog is independent of selector eligibility.
Its levels are fixture values, not native CGWindowLevelForKey results. Native
catalog/topology/current-window acquisition remains unwired. No screenshot
Auto Copy behavior or new permission path is introduced.

## Ownership and failure behavior

A refresh request carries the immutable selection/window/display context plus the
original navigation cancellation flag. Its independent generated pixels and mask
preparation run off the foreground executor. Each editor host owns a separate
SessionJobs generation; superseding a host or closing drops/cancels the old work.
The result is accepted at most once. Current context, base epoch and edit-session
identity are checked at publication. Current annotations/crop reject replacement;
editing then Undo to clean remains eligible on the same base.

Uncommitted gestures/text/label drag, export, and discard confirmation also refuse
replacement. Failures, cancellation, stale context or rejected edits keep the
frozen image and existing UI state usable, without interrupting the user with an
error. A successful refresh invokes the editor's real changed() path: pending OCR
is cancelled, OCR display/copy data invalidated, preview revision advanced and
old tiles discarded/rebuilt. Refresh does not add an Undo entry. Existing
image-aware history still keeps its associated image and edit geometry together.

Close, Escape/Cancel and discard/export-finish close paths explicitly retire the
refresh host. Window-owned weak async delivery also cannot resurrect a dead view.
The selector retains its original launch/selection guards through editor handoff.
The separate editor remains different from Swift's inline overlay editor.

## Validation scope

Seven GPUI editor tests execute the real entities, worker publication and preview
rebuild, covering both decisions and pixel outputs, OCR invalidation, duplicate
results, failure retention, edits versus Undo to clean, cancellation, newer job,
changed display, active interactions and close with a retained entity owner.
Two additional core fixture tests verify shape/dimensions/independent colors and
unknown identity/cancellation rejection. The 43 focused core Window/refresh tests
and all seven GPUI tests pass locally. The complete affected suite passes: 163 core
screenshot tests and 42 app screenshot tests. Strict app all-target Clippy and the
application build pass. A negative control deliberately omitted changed(); the
preview/OCR regression failed, then passed after exact source restoration.
Actual Linux desktop interaction and a rebuilt-binary scoped recheck passed;
see [the retained QA report](validation/window-refresh-2026-10-07/QA-report.md).
It distinguishes the earlier broad gesture/Undo/close pass from the final binary's
front/back/frozen-output and close/cancel recheck. Source input and binary hashes
are retained alongside representative app-only screenshots.

The new GPUI test-support feature is dev-only. Its registry-locked transitive
dependencies are the existing GPUI family's test helpers (including backtrace and
libgit2); no existing locked version was upgraded and no production service was
added. The new core synthetic image helper is debug/test gated.

Native alpha oracle testing independently found a real ±1-pixel sampling mismatch;
see [native-alpha-oracle.md](native-alpha-oracle.md). This fixture uses equal-size
images and does not erase that blocker. No claim is made of native capture,
CoreGraphics RGB/ICC/HDR equivalence, TCC/Spaces/focus acceptance or full UI parity.
All production Area/Window/movie gates remain disabled.


## Reviewed source accounting

Compared with implementation `0b8f44d`, this checkpoint adds 998 nonblank Rust
lines: six production cancellation scaffolding lines and 992 test/support lines.
Counts are 41,895 production, 19,161 test/support and 105 benchmark lines. Comments
count; DEBUG-only host/fixture code and native test modules are support. See the
per-file hashes and classification in
[loc-delta.json](validation/window-refresh-2026-10-07/loc-delta.json).
These source counts measure neither feature completion nor performance.


## Native mask seam continuation

The macOS host now uses the exact supplied-image CoreGraphics adapter through the
common core publication guards, while Linux retains the deterministic portable
sampler. The earlier native mismatch is preserved as explicit portable
characterization; see [native-alpha-oracle.md](native-alpha-oracle.md). This is
in-memory image processing, not capture enablement. The retained GUI screenshots
above remain evidence for their exact host checkpoint; new native mask execution
is separately gated by exact macOS CI and does not establish native GUI behavior.
