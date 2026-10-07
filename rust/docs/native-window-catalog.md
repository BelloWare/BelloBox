# Owned asynchronous Window catalog: admission remains closed

This advances the normal-build Window backend composition. The Apple Silicon
Window catalog and independent-acquisition implementation now compile in ordinary
macOS builds. Separate application Area/Window gates and the platform Window gate
remain false. Public Window catalog, freeze and acquisition calls fail before
native enumeration, permissions, image acquisition or presentation. This is an
owned integration checkpoint, not usable native capture or macOS UI parity.

## Source and owned transport

The reference remains CaptureWindowCatalog.swift:44–99 and
CaptureOverlayController.swift:221–264,470–598. Scope is normal-layer, non-own,
wholly contained windows on an unrotated, zero-origin main display. Full secondary
display topology participates in invalidation. There is no system-surface,
spanning-window or display-crop fallback for independent acquisition.

Platform snapshots contain only owned Rust data: exact f64 identity/frame/alpha,
full display geometry, and a separate optional front-to-back raw occlusion catalog.
The raw catalog retains row count/order and missing fields. A target-ID-only row
still ends the occlusion scan before any other field is read. Unavailable None is
distinct from an empty array. Own normal windows can occlude; own screen-saver
level overlays cannot. Actual CGWindowLevelForKey values are transported, not
replaced with fixture constants. The application alone converts raw frames to the
existing core f32 decision model in its approved main-display-local coordinate
space; the platform gains no production bellobox-core dependency.

Strict selector/completion metadata and raw occlusion parsing remain independent.
The strict parser rejects malformed identities, invalid fields, duplicate IDs and
more than 512 rows; it intentionally fails closed rather than adopting the raw
parser's skips. Raw partial dictionaries keep source skip/early-stop semantics.
This stricter identity policy is not a claim of complete Swift catalog parity.
CG observations have no bundle identity. Only the callback-local retained SCWindow
supplies bundle evidence. The exact callback-created Arc<WindowCapturePlan> still
binds the exact retained SCWindow passed to the independent filter. No callback
submission is reconstructed from CG rows. Returned plans must match the original
selection Arc and requested options before PNG decode or masking.

## Scheduling and freshness contract

The initial full-display freeze runs before any selector appears. Window's freeze
uses the existing Display implementation with physical-drain retirement before
subsequent catalog admission. Ordinary Display/Area retain their prior logical
return timing. This prevents a successful Display Job from racing its outstanding
callback/encoder lease into a spurious Busy at Window catalog entry.

The platform observation API is worker-only. It rejects a main-thread caller
before dispatch-and-wait, and sends an owned MainContext through dispatch_async_f.
Catalog CF/NSScreen ownership remains local to that main invocation. Cancellation
and an absolute deadline are checked before dispatch, on main entry, between
parsing/topology stages, after opaque calls and after physical drain. No new native
Send/Sync assertion or unchecked cross-queue request-object transfer is added. The
pre-existing explicitly retained immutable CGImage encoder transport remains
separately scoped; callback-local SCWindow/filter/configuration ownership is unchanged.

The application prepares and mounts the usable frozen Window editor before
optional occlusion observation. A coordinator-counted worker resolves the decision,
acquires independent pixels, validates acquisition evidence, masks when required,
and then observes again for final delivery evidence. The consuming with_decision
operation preserves the original document/base/context/cancellation snapshot.

Delivery evidence means fresh at the completed final observation. It does not
mean live-native or atomic window-incarnation proof at UI apply. UI delivery checks
only owned deadline, cancellation, selection/session generation, coordinator,
document/base epoch and clean interaction guards; it does not call observe. Supplied
fixture mutations advance an atomic revision, preserving stale-delivery tests.
Native revision zero explicitly denotes the absence of an OS revision feed. A
native change after observation, or a change-and-revert between snapshots, cannot
be detected by this contract. Native activation remains closed.

## Logical completion versus physical retirement

Job completion, cancellation and timeout do not release admission while queued
main work, a native callback or an encoder still owns its original lease. A
separate PhysicalDrain signal does not itself retain or reacquire that lease.
The coordinator's worker count lasts through physical drain and rejected-result
pixel disposal. Successful values held during drain are checked again afterward;
late cancellation/deadline cannot escape as successful platform output. Original
selection cancellation and live session are also rechecked for Window capture.

Opaque framework work cannot be forcibly interrupted. Logical timeout suppresses
publication, but physical worker/admission retirement can remain pending until the
framework releases its owned callbacks. This is deliberately not a bounded native
wall-clock completion claim.

Catalog collection still performs two CG queries and four topology passes on the
main queue, plus bounded CF parsing. Async ownership and off-main waiting do not
prove main-thread responsiveness. Native queue delivery, latency, callback lifetime,
TCC/AppKit/Spaces, Retina/hotplug, protected content, original-capture ICC/HDR and
actual independent pixels remain separately authorized runtime gates.

## Validation

Portable regressions exercise the same production composition with transport-
injected records/pixels, including deferred editor mount, delayed observation
editing, close/navigation/reopen, one-shot Undo, wrong returned plans, exact
subpixel/full-topology rejection, raw partial target rows, actual level transport,
own overlays, overlap thresholds and post-call/post-drain expiration. Core tests
verify with_decision retains original Arc/base/context guards. Native tests use
only constructed CF dictionaries, scalar topology and fake dispatch contexts;
they never enumerate windows/displays or acquire screenshots.

The macOS workflow installs Clippy and lints normal plus synthetic platform targets
with --all-targets --no-deps and -D warnings. The no-deps scope excludes unrelated
pre-existing macOS core settings lint; it does not suppress platform warnings.
Local Apple-target checks are metadata/type checks, not SDK linking or execution.
Exact new-commit macOS/Linux CI remains pending publication. Final local command,
GUI and source-hash evidence is recorded in the
[scoped acceptance report](validation/native-window-catalog-2026-10-07/QA-report.md).

DEBUG BELLOBOX_INLINE_WINDOW_REFRESH=observation-delayed pauses the first optional
observation for eight seconds; delayed and failure retain their existing acquisition
controls. These generated-source modes enter the same host and do not enable any
native gate. Linux supplied-pixel interaction is not native capture acceptance.

The immutable Linux candidate passed 103 screenshot app tests, 95 screenshot tests
in the exact app-only no-debug-assertion harness, 135 platform tests (3 existing
ignored), 169 core screenshot tests and the recorded strict lint/build checks.
Actual supplied interaction passed front/back refresh, delayed-observation editing,
one-shot Undo rejection, controlled failure, early close, fixed Crop/Undo/Redo,
actual Save cancel/reopen, 401×161 body-only PNG and clipboard reimport, and shared
Area resize/move/Copy & Finish. Twelve original app/dialog-only JPEGs, the actual
PNG, exact source/binary hashes and limits are retained with the acceptance report.
