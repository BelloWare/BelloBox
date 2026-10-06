# Native macOS one-shot capture adapter checkpoint

Date: 2026-10-06 UTC. Base: `e4e508fd2aff1b12aa26359776d44623ec715238`.
Worktree: `BelloBox-capture-native`. Rust-only implementation; no macOS execution.

The image-callback encoding/ownership behavior below is superseded by
[native-capture-encoding-evidence.md](native-capture-encoding-evidence.md): image
completion now retains and dispatches a specialized context, while dimension/
display validation and PNG encoding execute on an explicitly checked worker.
The original runtime/source-parity gates still apply.

## Scope and platform boundary

`bello_platform::native_capture` adds an explicit, in-memory macOS 14+
SCScreenshotManager one-shot adapter and portable region/display policy types.
The app/preview bundle still declares macOS 13.0 (`validate-bellobox-macos.py`),
so this adapter is not complete deployment-target coverage. It returns Unsupported
on macOS 13; the Swift source's SCStream one-frame fallback remains missing.
There is no helper/legacy fallback in the request API. A region request cannot
be broadened to a display or moved to the main display on an error.

This checkpoint does not claim actual capture, macOS visual/permission parity,
SDK link validation, or production readiness. The parent owns GUI integration.

## Public API

- `is_available() -> bool`: class/selector availability only; no capture/TCC request
- `main_display() -> CaptureResult<CaptureDisplay>`: read-only CoreGraphics metadata
- `capture_main_display_snapshot(CaptureCancellation) -> CaptureResult<NativeCaptureSnapshot>`
- `capture(CaptureRequest, CaptureCancellation) -> CaptureResult<NativeCaptureSnapshot>`
- `CaptureCancellation::from_flag(Arc<AtomicBool>)` interoperates with window jobs

The result owns `png: Vec<u8>` and `diagnostics`, including requested/resolved
IDs, resolution path, output dimensions, backend, region and cursor choice.
Debug omits image bytes. Error strings contain no native NSError text or paths.

The host must hide all BelloBox windows on the UI thread before capture, wait the
source's 150 ms visibility-settle interval, then restore windows on all outcomes.
GPUI App::hide is the available all-window seam; minimizing only the chooser is
insufficient. Stale completions must never activate a window or open an editor.
The parent is separately implementing guarded hide/restore behavior.

## Source policy and bounds

- Source order: initial display ID, refreshed ID, refreshed bounds, initial bounds
- Bounds matching uses the source's two-point tolerance; pixel-size/geometry
  changes for a selected ID fail closed rather than silently reframing
- Native initial-bounds fallback is usable only if that ID exists in the refreshed
  catalog. Unlike Swift, this narrower adapter will not revive a disappeared
  SCDisplay object or invoke legacy capture
- Local top-left display-point region, entirely inside the requested display
- Explicit full-display cursor option, false by default; region + cursor=true is
  rejected because source captureRegionImage always disables the cursor
- Audio always disabled; no permission prompt, file write, clipboard or network
- 64 million pixels, 32,768 pixels/axis, 64 million PNG bytes, ten-second deadline
- PNG uses a bounded CGDataConsumer; partial/oversized output cannot publish
- A single native-operation lease remains held until the framework drops its
  callbacks and the queued/running encoder releases its context, preventing
  repeated timeouts from accumulating native operations

SCScreenshotManager exposes no per-operation cancel API. Cancellation/deadline
bounds the Rust waiter and rejects late results; it cannot prove the underlying
framework stopped. A hung native operation can therefore leave the adapter Busy
until the framework releases its callback or the app exits. No retry loop hides it.

## Callback ownership and FFI review

Callbacks chain native work on their callback thread and publish only bounded,
owned Rust bytes. Both closure capture sets must satisfy Rust Send + Sync before
being passed through block2 0.6.2; no raw native pointers travel through channels and
no unsafe Send/Sync wrapper exists. Phase claims reject duplicate/out-of-order
callbacks, and cancel/timeout wins before publication. Rust panics are caught at
content, image, and data-consumer callback bodies.

Borrowed content/display objects are used within the content callback. The display
is explicitly retained while constructing and submitting the request. Filter,
configuration and display ownership remains local through the SCScreenshotManager
call and is released on that same callback thread; none is captured in the image
completion block. The current image completion retains the borrowed CGImage and
enqueues a specialized encoding context. See the linked hardening checkpoint for the
independent worker lease, dispatch ownership and off-main execution evidence.

This relies on Cocoa's normal callee-retains-stored-arguments convention for the
documented asynchronous SCScreenshotManager API. The API page does not explicitly
repeat a retain guarantee. Framework retention/lifetime behavior remains an
explicit SDK/native review gate; a cross-target type check cannot prove it.

No Objective-C struct-return selector is used. CGDisplayBounds is a regular C
struct-return function; setSourceRect: passes a repr(C) four-double CGRect and
returns void. Concrete message signatures, pointer ownership, C versus Objective-C
BOOL, and CGRect ABI need SDK/runtime review. ARM64 Rust type checking is the only
Apple architecture check executed; x86_64 Apple and runtime support are unverified.

Primary references:
- Swift ScreenCaptureService.swift:157–257,429–704 and DisplayCaptureResolver.swift
- https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager/captureimage(contentfilter:configuration:completionhandler:)
- https://developer.apple.com/documentation/screencapturekit/scshareablecontent/getexcludingdesktopwindows(_:onscreenwindowsonly:completionhandler:)
- https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/mmRules.html
- https://developer.apple.com/videos/play/wwdc2023/10136/

## Dependency provenance and gate

Final direct addition: target-macOS `block2 = "=0.6.2"`, already in Cargo.lock.
Its existing locked closure is objc2 0.6.4 and objc2-encode 4.1.0. The temporary
legacy block0.1.6 direct edge was removed after review and explicit approval of the
already-locked block2 replacement. No package versions, registry source files,
Objective-C/Swift sources or build scripts were changed.

Each archive was fetched only from its official static.crates.io URL, checked
against its existing lockfile SHA-256, then placed in Cargo's registry cache:

- https://static.crates.io/crates/block2/block2-0.6.2.crate
  - 34,505 bytes
  - cdeb9d870516001442e364c5220d3574d2da8dc765554b4a617230d33fa58ef5
- https://static.crates.io/crates/objc2/objc2-0.6.4.crate
  - 275,200 bytes
  - 3a12a8ed07aefc768292f076dc3ac8c48f3781c8f2d5851dd3d98950e8c5a89f
- https://static.crates.io/crates/objc2-encode/objc2-encode-4.1.0.crate
  - 21,004 bytes
  - ef25abbcd74fb2609453eb695bd2f860d389e457f67dc17cafc8b8cbc89d0c33

The first offline workspace fetch stopped on unrelated uncached
security-framework3.7.0 without a network request. Only the approved archives
were subsequently fetched; there was no alternate source or broad native fetch.

The initially approved block0.1.6 archive was also officially retrieved and
verified (4,077 bytes; SHA-256
0d8c1fef690941d3e7788d328517591fecc684c084084702d6ff1641e993699a).
Its uninhabited `_NSConcreteStackBlock: Class` static triggered a Rust future-
incompatibility warning, which motivated the approved replacement. The adapter
no longer uses that package and its final native check has no such warning.
Legacy block remains in the workspace lock for unrelated existing dependencies;
this change makes no claim about the whole app's upstream dependency graph.

The replacement uses RcBlock::new. block2 0.6.2 documents that its block type
cannot express thread-safe blocks, so the explicit Send + Sync closure-capture
check remains required at our unsafe FFI boundary. Every copied SCK block captures
only owned Rust Send + Sync values. Native request objects stay on the content
callback thread and are released after native submission. The separately documented
immutable image owner now crosses only the specialized libdispatch FFI boundary.

## Executed checks

Environment for each command:

```sh
export CARGO_HOME=/workspace/scratch/5ddc1675ed73/.cargo
export RUSTUP_HOME=/workspace/scratch/5ddc1675ed73/.rustup
export CARGO_TARGET_DIR=/workspace/scratch/5ddc1675ed73/target-clock-ui-final
export CARGO_BUILD_JOBS=2
```

From the worktree root, using `/workspace/scratch/5ddc1675ed73/.cargo/bin/cargo`:

```sh
cargo test --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --lib native_capture -- --test-threads=2
```

Final result after block2 migration: 16 passed, 0 failed, 25 filtered out;
0.00 s test time, 0.12 s test-profile preparation (cached).
Tests cover request bounds/cursor policy, resolver precedence, changed displays,
cancellation/deadline, single publication, duplicate/late payload destruction,
phase claims, separate jobs and redacted diagnostics. No test captures a display.

```sh
cargo check --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --tests --target aarch64-apple-darwin
cargo clippy --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --tests --target aarch64-apple-darwin -- -D warnings
cargo clippy --locked --offline --manifest-path rust/Cargo.toml \
  -p bello-platform --tests -- -D warnings
```

Apple type check passed after block2 migration (1.56 s). An initial duplicate C
preflight declaration warning was fixed to match the existing bool declaration;
two C-string style findings were fixed before strict Clippy passed. Final strict
Clippy passed for aarch64 Apple (0.52 s) and Linux (0.10 s), without the former
block0.1.6 future-incompatibility warning in this platform package. Owned files
were formatted with the pinned direct rustfmt executable.

## Explicit remaining source-parity gates

- Swift DisplayCaptureEnginePolicy's Auto/ScreenCaptureKit/Legacy settings and
  topology-keyed DisplayCaptureTrustCache are not implemented in this adapter.
- Source display-image trust verification, mismatch detection and verified legacy
  fallback are not implemented. Correct IDs/configuration alone do not prove that
  the received image shows the requested display; native visual verification is
  still necessary. No automatic helper fallback is available to hide that gap.
- Native framework errors are generic. The source's named protected-content error
  distinction, content-specific failure handling, and any protected-surface
  behavior are not established. Empty/dimension-invalid images are rejected;
  there is no black-frame heuristic or promise to capture DRM/protected content.
- macOS 13 one-frame SCStream fallback, window catalog/independent-window capture,
  frozen selection overlay, live scrolling source and auto-scroll remain outside
  this checkpoint. The separately labeled legacy CLI is not this API's fallback.
- No Apple SDK link, native callback-lifetime/ABI verification, actual TCC grant,
  real capture/image export, GUI runtime, signing or publication has run here.
- No broad workspace or app-link check was performed by this feature worker.

These gates must remain visible in the final app/ledger. This is an implemented,
Rust-type-checked macOS 14+ adapter candidate, not full source capture parity.


## Parent integration and independent review

Integrated additively on scrolling foundation
`a90a99d82f7856291e64c0b25cdccdd1a6d21446`. The explicit macOS GUI Screen action
now selects only this 14+ native adapter. There is no automatic helper fallback on
unsupported/error outcomes. The existing legacy CLI and Linux capture route are
unchanged; the application deployment target remains 13, with this feature’s 14+
requirement reported explicitly.

The host hides the application, waits 150 ms, and passes the window-owned cancellation
flag into the worker. A concurrent GUI capture is rejected while one is finishing.
Native-close invalidates the job; a Rust worker panic becomes an error so restoration
still runs. Restoration uses the independent app context even if the requesting
window/entity has disappeared. The UI-thread AppKit helper calls
unhideWithoutActivation, never activation. Editor focus requires BOTH
NSApplication.isActive and requester isKeyWindow; other valid results open unfocused.
Cancelled/stale/closed requests never open an editor.

Independent integration review caught and fixed a focus issue: GPUI's macOS
active_window reports mainWindow, which can survive deactivation. It is not used as
the foreground predicate. The new explicit activation/key-state gate and restore
path were re-reviewed with no remaining blocker in this scope. Parent independently
reviewed platform ownership/cancellation and corrected the region cursor policy,
non-Send native object captures and ImageIO's exact C bool finalize signature.

Final integrated focused checks:
- 16 portable native_capture tests passed (25 filtered), with no real capture calls
- 2 app host-policy tests passed (100 filtered): all focus predicate combinations,
 ordinary worker result/error propagation and panic containment
- strict aarch64 platform test-target Clippy passed, including the AppKit helper
- strict Linux app test-target Clippy passed; the existing proc-macro-error2 upstream
 future-compatibility advisory remains unrelated to the new block2 edge

These are not native GUI/runtime checks. macOS app SDK compile/link is deferred to
exact-checkpoint CI; actual SCK image/TCC/focus/visibility/cancellation and framework
argument lifetime validation remain Mac-runtime gates. No cloud screen was captured,
no permission was granted and no FPS or whole-feature-parity result is claimed.
