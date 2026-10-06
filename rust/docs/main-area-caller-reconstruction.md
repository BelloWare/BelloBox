# Disabled main-display Area caller: fresh reconstruction

This is newly written source based on published commit
`38ea32b727aa6c5c257647057999a9c064ec540f`, the original Swift implementation, the
published immutable Area model/DEBUG fixture, and the reviewed ownership
contracts. A workspace reset erased earlier unpublished caller/observer patches.
This source is **not** an identical recovered artifact. Historical test results
and historical unpublished hashes do not validate this reconstruction.

`PRODUCTION_AREA_ENABLED` remains **false** on every platform, with no fixture or
environment bypass. The explicit chooser label is `Area · main display only`.
Apple Silicon/macOS 14+ and an unrotated main display are the bounded target;
Linux and unsupported paths stay disabled. The published DEBUG fixture is intact.

## Source authority and boundaries

- `BelloBox/UI/CaptureOverlayController.swift`: 60 ms snapshot delay, immutable
  display snapshots before overlay creation, Escape, dim bands/crosshair/orange
  border, source application-deactivation observation and unlocked selection.
- `BelloBox/Screenshot/CaptureOverlayPanelConfiguration.swift`: borderless
  nonactivating panel, screen-saver level, exact source collection behavior.
- `BelloBox/Screenshot/ScreenCaptureService.swift`: hiding/restoring owned windows
  around frozen acquisition.
- Published `screenshot::area`: six-point drag preview, eight-point commitment,
  topology validation, independent pixel ratios and a clean initial crop.
- Pinned GPUI 0.2.2 ownership contracts: raw borrows stay inside live logical
  window updates; native close/activation and key callbacks may be queued.

The separately reconstructed platform helper supplies guarded synchronous popup
presentation and a UI-owned application-deactivation subscription. Its files are
published in `4a5dbc9fcd680b7fbde613ab7502bbdb1bc69ac7` and inherited here.
No core capture/model or native helper implementation is changed by the app patch.
The app adds only the already-locked macOS `raw-window-handle = 0.6.2` edge.

## Transaction and image flow

1. Read and validate immutable main-display metadata on the UI thread. Preflight
   every live owned GPUI logical handle before returning a mutation plan. Invalid,
   parented/sheet and fullscreen targets fail closed. Previously hidden/minimized
   targets never enter the restore set.
2. Record the full prior-visible set before any orderOut. The click callback's
   requester is already borrowed, so it is inspected/hidden/restored directly;
   other owned windows are accessed through their logical handles. This covers a
   native mutation that succeeds before its postcondition returns an error.
3. One background worker waits 60 ms and captures the explicit display exactly
   once. PNG decode, dimension verification and bounded tile preparation stay off
   the UI thread. No selector exists yet. No mouse-up capture path exists.
4. Before restoration, check requester liveness, navigation generation, actual
   application/key context and topology. Restore only still-live prior-visible
   logical handles through the approved nonactivating helper.
5. Subscribe to exact application deactivation after freeze/restoration and before
   selector creation. Recheck continuity after subscription. Registration failure
   disposes the prepared pixels without creating a picker.
6. Create a hidden/unfocused PopUp on the explicitly matching GPUI display ID,
   without titlebar or tabbing. Configure that exact borrowed window only. Before
   presentation require actual cached GPUI viewport, display ID and backing scale
   to match. A synchronous resize callback may be rejected while borrowed;
   deferral is not a guarantee of replay. Mismatch closes the hidden popup.
7. Present synchronously while nested logical updates hold both distinct live
   popup/requester owners, passing only short-lived raw borrows and the shared
   cancellation flag. Never call the queued GPUI activate_window route.
8. Pointer transitions recheck live native key/application state and topology.
   Dragging changes only dim bands and border/badge geometry. Immutable image tiles
   and full pixels remain unchanged; a valid mouse-up retains the full document
   and initializes a clean crop with no undo entry. Font preparation is off-thread.

**Documented UI deviation:** a successful selection enters the existing separate
screenshot editor, rather than Swift's inline overlay editor. The pending handoff
remains guarded against focus loss, close, cancellation, navigation and topology
changes. Existing-editor creation errors are reported.

## Cancellation, observer ownership and error semantics

New tool/launcher navigation, requester close/Escape, selector close/Escape,
window-key loss or exact app-deactivation notification invalidate the transaction.
Cleanup uses an independent App context, not requester/entity liveness.

The native observer guard is !Send/!Sync and lives only in the UI coordinator.
Only its pointer-free one-shot signal, transaction id and generation enter the
App-owned waiter. `ResignedActive` is accepted for that exact live subscription
without requiring `!cancelled`, because the callback sets cancellation before
waking it. `ObserverRemoved` and old ids/generations never cancel a successor.
The guard is taken and dropped outside the mutable global borrow on cancellation,
terminal error, accepted handoff or final owner cleanup. No timer polling is used.

Prior windows restore promptly on cancellation even while a slow worker drains.
Busy remains held until worker/disposal work and selector retirement finish.
A retiring picker synchronously orders out its exact owned window before logical
removal, since GPUI queues native close. Full pixels/tile disposal runs on a worker.

Restoration is one safe attempt. A refused restoration never retries with forced
front ordering, activation, level changes or old-window recreation. Its diagnostic
is shown in a live chooser or retained for the next chooser after requester closure.
A failed selector hide is also reported and close requested; immediate native
invisibility is not claimed after an error. These error paths require native QA.

## Fresh verification status

- Owned Rust source formatting and `git diff --check`: passed.
- Eighteen newly written focused tests cover transaction preflight/partial hide,
  closed/prior-hidden/minimized/new handles, stale navigation/requester, cancellation,
  focus/topology, busy until worker and selector cleanup, one freeze, tile bounds,
  clean full-pixel crop, target mismatch, viewport/scale/display identity, explicit
  gate/delay, and observer event/removal/terminal lifetime.
- Eight real pure transaction-module tests passed through an isolated test crate
  importing the unchanged application module and actual platform dependency.
- Fresh real-GPUI Linux cfg(test) check: blocked locally by missing system build
  prerequisites; two setup launches were cancelled before installation.
- Fresh focused app tests and strict Clippy: pending shared prerequisites/lane.
- Independent fresh static source review: passed, including chooser/navigation/
  editor-open glue and all 18 newly written tests. The eight transaction tests have since passed; the ten GPUI-dependent tests
  remain unexecuted locally.

The cfg(test) build uses the real GPUI caller/selector/async flow; only the five
app-local native borrowed-handle edges return Unsupported outside macOS. There is
no fake GPUI or fake SDK. No historical pass is carried forward.

Not performed: broad workspace build, desktop/native capture, permission grant,
provider upload, SDK installation, full native app compilation or runtime QA.
Actual native capture pixels/Retina alignment, AppKit delivery and callback
teardown, Spaces/Stage Manager, geometry refresh and error-path visibility remain
gates. The production constant must not be enabled on the basis of portable checks.

This production-disabled source checkpoint is published for existing Linux and
macOS CI to compile and test the actual GPUI application. Full app validation is
pending at publication; no local full-app or native-runtime success is claimed.
