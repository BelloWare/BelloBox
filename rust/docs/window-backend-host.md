# Backend-injected production Window composition

This report records the backend-composition checkpoint `505e8f1`. Its historical
foreground re-observation and test-only native acquisition descriptions below are
superseded by [the owned native catalog bridge](native-window-catalog.md): native
code now compiles in ordinary Apple Silicon builds behind closed activation gates,
and final observation runs on the counted refresh worker after masking. UI delivery
uses owned deadline/session/generation/base/liveness/clean-state checks; it does not
perform native observation or establish atomic live-window freshness.

The main-display Window workflow is now compiled in ordinary application builds,
including builds with application debug assertions disabled. It shares Area's
owned coordinator, freeze worker, selector window, real inline ScreenshotEditor,
preview/OCR/export jobs and physical disposal accounting. This is an integration
checkpoint under closed native activation gates, not usable native capture parity.

## Source and scope

The behavioral reference remains CaptureOverlayController.swift:221–264 and
470–598, CaptureWindowCatalog.swift:4–95, and ScreenCaptureService.swift:626–653.
Window selection remains deliberately restricted to normal-layer, non-own windows
wholly contained in an unrotated main display. There is no spanning-window,
visible-frame system-surface or display-crop fallback for independent acquisition.

Area and Window have separate false production constants. Both are tested at the
entry boundary, before backend construction, display/catalog reads, permission
checks, capture, clipboard or presentation. A second side-effect-free capability
preflight rejects the unavailable native backend before hide/freeze, even inside
intent admission. Window's production entry is wired to
the capture chooser and the same hide/restore transaction used by Area; a closed
gate never reaches that transaction. Generated images, mutable supplied evidence,
fixture options and optional disposal suspension remain DEBUG/test-only.

## Owned backend boundary

window_workflow::Backend supplies independent bounded evidence snapshots and an
independent acquisition implementation. Evidence retains exact f64 window identity,
frame and full topology separately from f32 selector chrome. Own PID and actual
occlusion levels are supplied by the backend. The selector observations and raw
front-to-back occlusion rows remain separate. Missing fields, unavailable catalogs,
own regular windows and own screen-saver overlays preserve the existing source
semantics; the strict native candidate's CG parser is not substituted for raw rows.

An acquisition receives the immutable selected-window policy, bounded options,
live session ownership and cancellation. The source callback must validate its
actual retained window with WindowCaptureSelection::plan before requesting pixels.
The supplied backend does the same before invoking its pixel generator. Callback
identity/frame mismatch, absent or duplicate identity therefore causes zero pixel
generation. A successful result carries the immutable plan through decoding,
completion, masking and fresh foreground publication checks. The returned plan
must match the exact selection Arc owner and exact requested options, not merely
a matching source/session token or dimensions. Native completion is checked before
PNG decoding; generic backend completion is checked before refresh/masking.

NativeWindowCaptureSnapshot now carries the exact Arc<WindowCapturePlan> made
inside the existing shareable-content callback from its retained SCWindow. The
same local SCWindow is used to create the independent filter. Completion clones
that plan; it never reconstructs submission metadata from initial CG rows, PNG
bytes or diagnostics. The plan owns only Rust metadata and cancellation flags.
The native candidate remains test-compiled, and its public capture entry remains
Unavailable before enumeration, permission checks or acquisition. Native catalog
setup also remains explicitly Unavailable, pending source-exact raw-row parsing
and native thread/lifetime acceptance. No native object gains a Send implementation.

Completion checks exact identity/frame/full topology, current session, both
selection and operation cancellation, dimensions and deadline; the editor repeats
fresh evidence validation at actual publication. This is point-in-time evidence,
not an atomic native window-incarnation guarantee. Opaque native work is not
forcibly interrupted by cancellation or timeout.

## Preserved editor and retirement behavior

Window commit physically copies only its selected frozen rectangle. No full-display
base is hidden behind a crop. The chosen frame and toolbar remain fixed; cropped
pixels aspect-fit in clean letterboxes with pixel-only clipping, while text-control
hit behavior remains unchanged. There are no Window resize handles or Select-move.
Area keeps its independent-axis pixel-ratio mapping and adjustable selection.

Failure leaves frozen pixels and current edits usable. Replacement still requires
a matching document/base epoch and current clean annotation/crop/interaction state.
Undo to clean is eligible, but old base epochs never become current again.
Acceptance uses the actual changed() path to invalidate OCR/copy data and rebuild
preview, without adding a synthetic Undo step. No automatic clipboard copy or
provider request was added.

The coordinator remains Busy through physical completion and off-thread disposal,
including rejection after cancellation, navigation, editor close or entity release.
The retained prepared image and old base cannot become their final heavy drop on
the UI executor. Existing tests pause the actual disposal future and verify that a
new capture cannot be admitted until ownership drains.

## Reproduce supplied interaction

Use BELLOBOX_INLINE_WINDOW_FIXTURE=1 with BELLOBOX_TOOL=screenshot and an isolated
BELLOBOX_CONFIG_DIR. It enters the same normal-build preparation, selector,
editor and backend-driven refresh composition. Its generated surface is 800×500
points / 1600×1000 pixels. Orange front selection is 680×540; blue back selection
is 840×460. Front refresh borrows independent alpha; occluded back refresh replaces
the frozen orange overlap with independent blue pixels.

DEBUG-only BELLOBOX_INLINE_WINDOW_REFRESH=delayed waits eight seconds before
producing the independent image. The value failure waits the same interval and
then returns a controlled failure. During either wait, the frozen editor must
remain editable and close/cancel must retire work safely. These controls are absent
from ordinary builds and do not access a screen, permission or native catalog.

## Verification status

The final hardened Linux candidate passed 96 screenshot app tests, 88 screenshot
tests in the exact app-only debug-assertions-disabled harness, 130 platform tests
(3 existing ignored), strict app/platform all-target Clippy, minimal-feature check,
normal build and app-only non-debug metadata compilation. Actual supplied-pixel
Linux interaction passed the front/back, fixed Crop/Undo/letterbox/text,
Copy & Finish/reimport, delayed/failing refresh, cancellation and Area matrix.
Eight original JPEGs, exact binary/source hashes, the independently reviewed
returned-plan correction and its detecting negative control are recorded in
[the acceptance report](validation/window-backend-2026-10-07/QA-report.md).

The scoped reviewed delta is 901 production and 108 support nonblank Rust lines,
with no benchmark change: cumulative totals 45,352 / 23,813 / 105. The ledger
separately identifies 651 preserved support lines reclassified to production and
3 in the reverse direction; new/reworked/removed lines are separate. This is a
conservative literal-line decomposition, not a feature-completion measure. See
[the explicit ranges and hashes](validation/window-backend-2026-10-07/loc-delta.json).

Exact published backend source `505e8f193de858163474e2af269d2e3ca51e475a`, tree
`8477ad4e3b0a7da76db42de83530faccc5cb6f6a`, passed Linux run `37591717754`
and macOS run `37591717776`. All steps passed, including native compile/link,
constructed completion ownership, strict normalized mask/host-worker tests, app
build and offline development packaging. These results apply to that checkpoint;
the subsequent native catalog source requires its own exact CI. No actual screen
capture, permission or Mac access was performed. Native Area, Window and movie
activation remains closed.

Existing normalized-SDR mask oracle scope is unchanged. Linux's portable sampling
is still approximate for ±1-pixel sizes. No original-capture ICC/HDR, AppKit/Spaces,
Retina/hotplug, native callback responsiveness or whole-project parity is claimed.
Source reclassification from DEBUG support to production is reported separately
from newly added lines; neither is a feature-completion percentage.

The production-compiled evidence layer still clones bounded owned snapshots and
uses an O(n²) duplicate scan over at most 512 complete observations. Native catalog
acquisition remains unavailable; observation-thread placement and measured UI
latency are still gates. No native responsiveness claim follows from the bound.
