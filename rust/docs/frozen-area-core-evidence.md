# Frozen Area core and DEBUG selector checkpoints

Date: 2026-10-06 UTC. Base: `b2e27d6cd730afbfff69a834568da9c4e4334459`.
Worktree: `BelloBox-area-frozen`.

## Scope

New `bellobox_core::screenshot::area` state/geometry module and deterministic tests.
The pure module contains no screen acquisition, window creation, native helper,
clipboard access, permission request, or file/network I/O. No dependencies were
added. A later DEBUG-only app-owned selector checkpoint is described below;
production Area and the native overlay host remain separate. Production integration is intended to remain main-display-only
until broader display behavior is implemented and verified.

Source references:
- `CaptureOverlayController.swift`: freeze before overlay creation (200–260),
  selection/focus-loss behavior (427–494), mouse down/drag/up (1327–1356)
- `CaptureSelectionResolver.swift`: Area-only click/drag rules and eight-point minimum
- `RegionCaptureGeometry.swift`: six-point visible-drag threshold, eight-point minimum
- `ScreenCaptureService.swift:288–305`: full immutable display plus initial crop
- Existing Rust `screenshot/selection.rs`: dim bands, crop conversion and adjustment

## API and coordinate contract

`AreaDisplayGeometry` contains display ID, global Cocoa bottom-left frame, native
pixel dimensions and rotation. All pointer events are display-local top-left points.
CoreGraphics display frames must be explicitly converted; they are not interchangeable
with Cocoa frames. `core_graphics_frame_to_cocoa` accepts the primary display's Cocoa
top edge explicitly rather than incorrectly using the target display's height.

`FrozenAreaSession` starts Freezing and cannot accept selection input until a
matching `FreezeToken`, full clean document and exact display metadata are accepted.
The token has private owner/generation fields. Duplicate, cancelled, older-session
and other-owner freeze completions cannot replace the current frozen image.

Begin/update/end drag calls validate current metadata. Display ID, global frame,
pixel size or rotation changes invalidate the capture; there is no implicit target
replacement or same-size fallback. Cancel, restart and Drop signal the prior freeze
worker's cancellation flag. Restart creates a fresh flag. Focus loss cancels only an
unlocked post-freeze selection, so the host's initial hide does not cancel freezing.

The six-point rubber-band threshold is distinct from the commit rule: both selection
dimensions must reach eight points. Clicks/tiny/skinny drags reset for another attempt.
Pointer-down and mouse-up are clamped to the original display; reverse drags work.
Mouse-up includes its final point and produces at most one committed result.

`AreaSelection.editor` is a clean-baseline `ScreenshotEditSession`: the crop is set
on a clone of the frozen document before session construction. It does not fabricate
an initial undo step or dirty state. Full original pixels remain shared immutably,
allowing later crop adjustment without reacquisition. Full-display selection leaves
the crop unset. Pixel mapping uses actual independent x/y ratios and outward-rounded
edges; origin-free conversion prevents large/negative desktop origins from degrading
the local pixel crop.

This is still-image Area behavior. It must not be relabeled as a live region sampler
or used to replace region-only acquisition for scrolling capture.

## Executed validation

From the worktree root:

```sh
CARGO_HOME=/workspace/scratch/5ddc1675ed73/.cargo \
RUSTUP_HOME=/workspace/scratch/5ddc1675ed73/.rustup \
CARGO_TARGET_DIR=/workspace/scratch/5ddc1675ed73/target-clock-ui-final \
CARGO_BUILD_JOBS=2 \
/workspace/scratch/5ddc1675ed73/.cargo/bin/cargo test \
  --locked --offline --manifest-path rust/Cargo.toml \
  -p bellobox-core --lib screenshot::area -- --test-threads=2
```

Result: 24 passed, 0 failed, 262 filtered out; 0.07 s tests after 4.31 s compile.

Coverage:
- No selection before frozen pixels; immutable pixels after synthetic live content changes
- Mouse-up point, forward/reverse/clamped drags, click/tiny/skinny/exact-minimum cases
- 1x, 2x and nonuniform x/y pixel ratios; outward pixel rounding
- Negative/above/below display origins and explicit CoreGraphics/Cocoa conversion
- Full-display unset crop; initial clean baseline; one-step adjustment undo
- Single publication, cancellation, late/duplicate/other-owner replies and restart
- Invalid newer request cancels old selection; Drop signals in-flight worker
- Focus loss before/after freeze and after selection locks
- Exact topology and snapshot-dimension/prior-crop checks; invalid geometry/points
- Four nonoverlapping dim bands cover only the unselected pixels

The exact-pixel regression changes the synthetic live page to solid red after the
only supplied freeze. It compares the exported selected PNG against the original
frozen crop and verifies that the editor shares the original immutable image Arc.
No capture function exists in the pure module, so committing cannot recapture.

Focused strict Clippy also passed:

```sh
# Same CARGO_HOME, RUSTUP_HOME, CARGO_TARGET_DIR and CARGO_BUILD_JOBS=2
cargo clippy --locked --offline --manifest-path rust/Cargo.toml \
  -p bellobox-core --lib --tests -- -D warnings
```

Elapsed: 2.10 s. Owned Rust files were formatted with the pinned direct rustfmt.
No broad workspace matrix, app build, binary copy or native execution was performed.

## Remaining integration/native gates

A host must still create the overlay only after accepted freeze completion, provide
fresh topology metadata, map its actual logical viewport, draw the frozen pixels,
route cancellation/focus changes, close only owned windows, and transfer the editor
without changing the baseline crop. This core checkpoint cannot establish those UI
or native behaviors.

Native panel levels/Spaces/nonactivation, menu-bar/Dock coverage, capture visibility
timing, genuine hover preservation, Retina/scaled/rotated display mapping and display
reconfiguration remain separate macOS gates. Pure support for explicit secondary-
display coordinates is not a claim that multi-display production UI is enabled.


## DEBUG app-owned frozen selector checkpoint

Additional owned app file: `screenshot_ui/area_capture.rs`. Parent-module changes
are limited to its debug-gated declaration/route and extracting `prepare_session`
so the existing `prepare_document` delegates without resetting a supplied session's
clean crop baseline. Normal capture paths and the disabled production Area button
remain unchanged.

Launch expectation for a debug build:

```sh
BELLOBOX_AREA_FIXTURE=1 BELLOBOX_TOOL=screenshot <debug-bellobox-binary>
```

The window is titled `Area · Frozen Synthetic Fixture — Bello Box`, with requested
window dimensions 840 x 680 points and an 800 x 500 point logical canvas. All pixels
are app-owned synthetic data at 1600 x 1000 pixels. A worker generates and accepts
the freeze, decodes the document, and prepares <=1024-pixel image tiles before the
selector window is created. A preparation error opens a separate plain error view,
not a selector over live screen content. Repeated opens supersede preparation. Newer tool or launcher navigation
invalidates the generation before a selector or error window can open.

The pointer handlers use the measured canvas bounds, clamp through the core Area
model, draw four dim bands, a crosshair, an orange rubber band and a pixel-size label.
During dragging they update only geometry over cached images; they never reencode
or regenerate the frozen pixels. The exact example local drag (100,80) -> (400,280)
maps to a 600 x 400 pixel crop.

Escape, native close and post-activation focus loss cancel work and close the
selector. Commit keeps its locked rectangle visible while preparing the existing
editor session on a worker. SessionJobs rejects stale/closed/cancelled handoff;
the requesting window must still be active to open the editor. On macOS the
read-only UI-thread `macos_native::application_is_active()` getter must also
confirm `NSApplication.isActive`, with errors treated as inactive; it never
unhides or activates the application. Linux keeps its existing window-active check. A transfer flag
prevents the fixture's own focus-loss observer from cancelling a successful handoff.
The only file I/O in this path is the existing optional local-font preparation;
there is no image-file import, clipboard read, native capture/overlay call, provider
request or automatic export.

Four new app test functions cover tiled preparation before selection, the exact
600 x 400 clean-baseline crop, cancelled preparation, the four-state combined
application/key-window focus policy, and source chrome rules. They have been compiled
by metadata checking but have **not executed** because app linking is reserved for
parent coordination. The original 24 executed core tests remain the current
runtime-independent geometry/state evidence.

Executed metadata command, with the same Cargo environment above:

```sh
cargo check --locked --offline --manifest-path rust/Cargo.toml   -p bellobox-app --tests
```

Final result: passed in 1.75 s. The first attempt found an ambiguous floating border
width; it was explicitly typed f32 before the passing run. Cargo reports the existing
proc-macro-error2 2.0.1 future-incompatibility warning; no dependency was changed.
Owned app code was rustfmt-checked and the diff has no whitespace errors.

Not run for this app checkpoint: app test execution/link, binary copy, native UI
interaction, screenshot/export verification, focus/close/reopen runtime cases, or
publication. Parent-owned isolated cloud GUI QA must establish actual pointer
mapping, layout, frozen pixels, handoff/export, and interrupted flows before claiming
the DEBUG selector vertical slice complete. This is an ordinary fixture window;
it is not the source's borderless native capture overlay.


### Review corrections before app linking

`CaptureOverlayController.swift:1394-1406` filters empty selection rectangles,
hides the locked selection badge and uses a 2.5-point locked border (2 points
while dragging). The fixture now follows those rules. Its size badge preserves
independent x/y ratios and truncation, with no per-drag image work.

The only additional platform change is the narrow read-only UI-thread
`macos_native::application_is_active()` getter. It checks `NSThread.isMainThread`,
reads the existing application's `isActive` BOOL, and reports errors without
changing application visibility or focus. The macOS Area handoff combines it
with requester key state using the existing tested `native_capture_should_focus`
predicate immediately before opening the editor. A pure four-state regression
covers this shared predicate, including inactive app plus stale key-window state.

Final corrected-source metadata checks (same environment, jobs=2):

```sh
cargo check --locked --offline --manifest-path rust/Cargo.toml -p bellobox-app --tests
cargo check --locked --offline --manifest-path rust/Cargo.toml -p bello-platform --target aarch64-apple-darwin
```

Both passed, respectively 1.78 s and 0.32 s. The four new app tests are compiled,
not executed. No app link or runtime/native QA was performed in this worker.

## Native cloud fixture observations and exact-coordinate resolution

The Linux GPUI fixture candidate `363f02c8fbd1a284c9442dc505cc256cdab89b8a1a8ce370a8b0508ec01c6391`
passed small-drag rejection, forward/reversed identical crops, annotation then Undo
back to the clean initial crop, close without a discard prompt, reopen, Escape,
Cancel and focus-loss dismissal. The precise pending-handoff race was not observed.
This was app-owned synthetic content, never a real screen capture.

The requested desktop drag produced 601 × 401 pixels, rather than the 600 × 400
integer-local unit fixture. A separate scalar-only diagnostic candidate
`64d675f722e33e6b90bc087164c1aa69f431b055db753940d167d7eb8f513a71`
resolved the difference without changing crop behavior:

- Canvas origin `(20,84)`, size `(800,500)`, scale `1`: exactly integral
- Actual down event `(120.001831055,164.002502441)`, local `(100.001838684,80.002502441)`
- Actual up event `(420.006408691,364.005554199)`, local `(400.006408691,280.005554199)`
- Source integral coverage: x from floor(200.003677...) to ceil(800.012817...),
  y from floor(160.005004...) to ceil(560.011108...), yielding `(200,160,601,401)`

The discrepancy is fractional injected input-event coverage, not fractional canvas
layout or a crop regression. The source intentionally rounds edges outward. A new
focused regression preserves both the exact integer 600 × 400 case and the traced
fractional 601 × 401 case in both directions. Diagnostic instrumentation was removed;
its immutable binary, raw scalar trace and original QA evidence were retained.
Save dialog behavior remains the known cloud portal limitation; no GUI-saved PNG,
macOS Area capture, native borderless overlay or performance result is claimed.


## Final parent checkpoint checks

Integrated on background-encoding checkpoint
`4c575d228ac75bd79ef5af1760982cc07d61e3cf`. The reviewed app source used for the
validated v1 fixture is unchanged after removing the separate trace instrumentation.
Final parent checks supersede the earlier worker-only metadata limits:
- 25 focused core Area tests passed, including integer/fractional/reversed coverage
- 5 focused app fixture tests passed, including actual pending-generation acceptance
- strict Linux app/test Clippy and the native application build passed
- The added read-only AppKit activation getter passed Apple-target platform checking

Tool and launcher navigation invalidate pre-window preparation. A stale completion
cannot clear a newer pending state. On macOS, an inactive/error application state
suppresses both selector and error-window creation; later editor handoff additionally
requires the requester key-window state. Repeated opens supersede earlier work.

The separately developed native owned-overlay/visibility helper is not part of this
foundation publication. Production Area remains disabled; this checkpoint enables
only the documented DEBUG synthetic route. No new dependency is added here.
