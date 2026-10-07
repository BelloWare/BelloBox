# BelloBox Rust + GPUI migration handoff

Snapshot: 2026-10-07 UTC. This document and the repository are sufficient to resume
without the previous computer or conversation. The current `rust` branch is the
source of truth. Read `AGENTS.md`, `rust/README.md`, and
[the detailed parity ledger](bellobox-parity.md) before editing.

## Goal, constraints and working agreement

Port the original BelloBox macOS application to Rust and GPUI with the original
Swift behavior and recognizable UI. The Swift tree remains the specification;
this is not permission to redesign tools or substitute a simpler workflow.
Apple Silicon/macOS is the principal native target. Linux is useful for portable
logic and GPUI validation, not proof of AppKit/TCC/Spaces behavior. Keep code Rust;
use audited Apple bindings rather than guessed ABI or a second Swift application.

Priorities: responsive, smooth interaction; bounded work and memory; minimal
new dependencies; no paid services or additional cloud expenditure. Use existing
public-repository CI and installed/cached tools. Do independent source work,
focused tests and review in parallel, coordinating shared build/cache and desktop
ownership. Do not wait idly for CI when useful independent approved work exists.
Do not claim frame-rate, native parity or performance completion from LOC or tests.

The user authorized coherent commits and ordinary pushes to BelloWare/BelloBox's
`rust` branch, including migration source, tests, project documentation and CI
repairs. Read back remote SHA/tree after a push. Never force-push, rewrite shared
history, merge to main, publish releases, access the user's credentials/device,
or grant TCC/AX/camera/microphone permissions as a side effect. Native validation
must use a separately authorized environment and permission flow. A previous
report-only schedule conflicted with implementation publication; the user removed
that restriction and expressly authorized GitHub writes. Honor any newer user
instructions and actual tool/security denials; do not change transport to bypass
a denial. No publishing or provisioning of paid services is implied.

## Durable baseline and verification

The last implementation checkpoint is
`0b8f44d2cdcd9f5cb6580431ca7ac5cdbd46027d`, tree
`7980c6f7da7c88d7797eaac82eaae7f64e1ec8e0`. It contains the reviewed Window refresh
policy and image-aware Undo history, including the corrected 64 MiB ledger.
Both exact CI runs succeeded:

- [Linux 37567749507](https://github.com/BelloWare/BelloBox/actions/runs/37567749507)
- [macOS 37567749475](https://github.com/BelloWare/BelloBox/actions/runs/37567749475)

The documentation/recovery checkpoint containing this file follows that baseline.
Its own CI may still be pending at handoff; inspect the exact current SHA rather
than transferring an older green status to it. No runnable code is added by this
handoff checkpoint. The incomplete alpha source is a `.patch` artifact only.

Recent useful checkpoints:

| Commit | Result |
| --- | --- |
| `4a5dbc9` | Reconstructed native owned-overlay presentation/deactivation helper |
| `bdd9168` | Reconstructed, production-disabled main-display Area caller |
| `bf023f8` | Test-gated native movie reader and synthetic MOV→frames→Rust GIF tests |
| `82c879f` | Canonical-path fixture and Linux activation-wait CI repairs |
| `57f2bbc` | Pure Window identity/topology/selection policy |
| `cef73a4` | Pure Clock copilot proposal plan; no provider/apply UI |
| `79c55f1` | Test-gated callback-local independent Window backend |
| `760561d` | Correct own-PID fixture expectation and CF const declarations; both CI green |
| `da9809a` | Synthetic frozen Window hover/click selector; both CI green |
| `0b8f44d` | Pure Window refresh and image-aware bounded history; both CI green |

One `da9809a` Linux attempt passed tests/lints/build but timed out before its default
window became visible under Xvfb. The unchanged exact failed-job rerun passed.
Do not call that an interactive Window selector failure or hide the first failure.
Native Window's first run had a real test expectation error: source PID 3 changed
to fixture own PID 4, correctly yielding IneligibleWindow, not WindowChanged.
The repair separately asserts foreign PID replacement, own PID exclusion and
stale generation. All six native Window fixtures then passed.

## Architecture and feature status

Rust workspace roles:

- `bellobox-core`: bounded portable engines, screenshot document/render/edit models,
  frozen Area/Window selection, scrolling stitch, GIF pipeline, Clock planning.
- `bello-platform`: filesystem/process/platform boundaries, native capture and
  movie adapters, topology/identity policy and owned AppKit overlay helpers.
- `bello-workbench` and UI crates: shared workbench/state/editor services.
- `bellobox-app`: GPUI shell, palette/tools and screenshot/editor hosts.

Read the parity ledger for per-feature source paths, evidence and intentional
limits. The following is a workflow-level map, not a completion percentage.

| State | Feature families |
| --- | --- |
| Usable implemented workflows | Home/palette navigation/search/favorites/recents; offline text/developer tools; QR; offline World Clock planning/locations/reference/copy; snippets; screenshot import/editor/OCR subset. Each retains documented platform/UI limits. |
| Source-checked narrow tools | List Set Operations stable order/blank/duplicate/match semantics; String Literal escaping/unquoting; canonical IPv4 subnet rows including /0,/31,/32; text-only chmod calculator; Number Base with separate GUI semantics preserving CLI compatibility. |
| Partial or backend/test-only | Native macOS14 Screen adapter; disabled Area; disabled Window backend; synthetic frozen Window selector; pure refresh/alpha policy; scroll stitch/debug HUD; portable Rust GIF encoding; test-only native movie reader; pure Clock copilot proposal plan. |
| Major missing source workflows | Complete live Area/Window/scroll host behavior; native recording/audio/privacy/countdown/pause/review; standalone movie→GIF converter UI; complete AX selection/global shortcuts/floating toolbar/replace-in-place; provider/Codex app-server/model-discovery workflows; consented AI OCR transport; copilot conversation/apply/handoff; native onboarding/menu bar/launch-at-login; vault/migration; accessibility/Spaces/material fidelity. |

Do not equate a catalog row, parser, CLI engine or pure plan with a complete UI
workflow. Audit less recently touched developer tools against Swift before calling
them exact. The broader tool/UI inventory and known gaps are in bellobox-parity.md.
No percentage or whole-project delivery date can be inferred from source counts.

### Screen capture, shared lease and callback ownership

`bello-platform/src/native_capture*` owns bounded capture requests. Display, Region
and Window share one in-flight admission lease. Callback contexts, encoder and
final validation retain independent lease owners; timeout/cancellation cannot
reopen Busy while outstanding native work still owns resources. Completion is
once-only, phase-checked and stale-result fenced.

The macOS14+ one-shot ScreenCaptureKit path is explicit. macOS13 is unsupported
for the new GUI adapter; the separately labeled existing CLI helper remains, with
no automatic fallback that could weaken region/privacy semantics. Native objects
must not receive unproven Send implementations or cross queues as unchecked raw
pointers. Retained immutable CGImage transfer to `dispatch_async_f` is the narrow
reviewed exception, with exact retain/release ownership and bounded PNG encoding
off the UI thread. Waiting/publication is bounded; opaque framework execution is
not forcibly interrupted, and callback queue behavior still needs native testing.

The Window backend resolves/retains the selected SCWindow locally inside the
shareable-content callback, validates it, builds the desktop-independent filter,
and submits while native objects remain callback-local. Owned Rust submission
metadata crosses queues. Fresh CG identity/frame/layer/alpha/on-screen metadata,
full topology and session token are checked after encoding. Actual own-process PID
is excluded independently. Bundle conversion is bounded, full UTF-8 and NUL-safe.

There is intentionally no second native bundle reread or atomic window-incarnation
claim. CG metadata lacks that bundle evidence; changes that occur and revert
between snapshots can escape comparison. No display-crop fallback is substituted
for independent Window capture. Source defaults for shadow/opacity/clipping remain.
Native Window code is Apple-Silicon/macOS test-compiled; public capture returns
Unavailable before enumeration/permissions/image acquisition. Real capture is off.

### Area and frozen overlay host

The Area caller in `screenshot_ui/main_area*` remains production-disabled. It
freezes a full display before selecting; live scroll-region privacy is a different
policy and must not be conflated with still Area. Pure geometry uses actual pixel
ratios, explicit CG/Cocoa conversion, negative display origins and outward pixel
rounding. A fractional input drag legitimately produced 601×401 where integer
coordinates produce 600×400; traced fractional CUA events explained it. Do not
force inward/integer rounding to make that screenshot match an assumed size.

The owned-window visibility transaction records prior visibility before orderOut,
uses exact live owned handles, never deactivates the whole app or manipulates
third-party windows, and restores without resurrecting closed windows or stealing
activation. Navigation/session generation, requester closure, app deactivation,
key state and topology fence late presentation. A UI-owned !Send deactivation
subscription exposes only pointer-free cancellation signaling. Hidden popup
presentation checks its exact native handle, content/renderer bounds and Retina
scale. Separate-editor handoff is still a documented difference from Swift's
inline capture-overlay editor. No Mac runtime alignment/focus claim is established.

### Frozen Window selection

`core::screenshot::window` reuses the frozen Area image owner and bounded topology/
cancellation tokens. Front-to-back eligible order wins; mouse-down refreshes and
latches hover. Both final clamped axes must be strictly below 8 points for a click.
Exactly 8 rejects; 7×7 is accepted. Blank clicks/long drags never fall through to
Area or Screen. Cocoa window half-open edges differ from the local viewport edges.

Commit evidence is private/immutable. A worker physically copies only the selected
window rectangle into a new window-sized base with no annotations/crop/Undo. It
must not keep a full-display base behind a crop, which could later expose pixels
outside the chosen window. Frozen occluding pixels remain visible until a separate
live refresh is implemented. Global launch identity persists through visible
selection and final handoff; newer navigation, close, focus loss and cancellation
retire old work.

DEBUG `BELLOBOX_WINDOW_FIXTURE=1`, then open Screenshot through the existing route.
It uses an 800×500 point /1600×1000 pixel synthetic surface: orange front window
(300,80,340,270), blue back window (100,170,420,230). Orange click should produce
680×540; exposed blue click 840×460 with orange overlap preserved. The core has20
focused tests; actual GPUI fixture tests compile/run in Linux CI. Live gesture QA
has not been performed on this final checkpoint. Area/scroll fixture routes remain
separate. See frozen-window-selector-evidence.md.

### Refresh, Undo and memory

`core::screenshot::window_refresh` is unwired pure preparation of already supplied
pixels. It uses a separate occluder catalog: own regular windows and several
source window levels can occlude, so the narrower selector catalog is unsuitable.
It preserves front-to-back source defaults and chooses KeepFrozen,
MaskFrozenAlpha or ReplaceWithIndependent. It checks selection/session/topology
and cancellation before acceptance. Native host wiring is still missing.

Replacement requires the same edit-session/base token plus CURRENT empty
annotations and absent crop. Editing then Undo to clean remains eligible; revision
zero and empty history are not the rule. Font preparation is preserved. Replacement
adds no Undo step, advances revision and base epoch, and invalidates pending OCR
by revision; eventual UI integration must also use its changed()/preview/OCR path.

History snapshots pair image Arc with annotation/crop state, so resize/Undo/Redo
cannot restore geometry onto the wrong pixels. The dirty baseline remains metadata
only and does not pin the original image. A monotonic session-owned base epoch
outside snapshots prevents restoring an old Arc from reviving old refresh tokens.
The combined history cap is64MiB for metadata plus unique older RGBA Vec capacities
across Undo/Redo, excluding the active base and deduplicating Arc allocations.
The64-step cap remains; oldest history is evicted beyond either bound. This is
explicitly more bounded than Swift's unrestricted whole-document history. Large
cross-base history may be evicted. The cap is not total process memory: current
image, detached renders and transient work have separate bounds.

All21 refresh regressions and161 affected screenshot tests pass, with strict core
Clippy and independent review. Portable alpha/resampling is deterministic but
CoreGraphics premultiplication/color/rounding equivalence is not yet established.

### Movie/GIF and recording

Portable Rust GIF code handles bounded options, centisecond timing (including a
partial final frame), browser repeat semantics, sequential encoding, cancellation,
staged sibling output and atomic final replacement. No production FFmpeg dependency
was added. Preserve source identity and never overwrite the source on failure.

The typed AVAssetReader native movie adapter is test-only. Production MovieAsset
open/reader/next_frame return Unavailable before I/O. Reader/output/asset stay on
one worker and are !Send; cancelReading does not race sample reads. Revalidate file
identity after each read, including nil/end markers. Decode source PTS and preferred
transform/orientation; copied CFData owns bytes beyond pixel-buffer unlock, avoiding
borrowed unlocked buffers. Metadata callbacks own Rust flags/leases, not stack or
unchecked native state. Visible buffer accounting is bounded, but opaque AV/CG
allocations and retained core frames are additional; do not claim whole-path96MiB.

Six native synthetic tests passed actual Apple CI, covering orientation, source
PTS, cancellation, sample limits, invalid metadata/status, and staged Rust GIF
integration. They do not prove all codecs, arbitrary corruption recovery, hardware
acceleration, native UI or recording. Approved pinned objc2 AVFoundation/CoreMedia/
CoreVideo/CoreGraphics0.3.2 family is in Cargo.lock with minimal explicit features.
Resolver-required weak transitive manifests are not permission to activate audio
or IOSurface functionality. No new direct package family is needed for alpha tests.

Recording is a major unimplemented vertical: source selection, ScreenCaptureKit/
AVFoundation stream ownership, optional system/microphone audio, cursor/click/key
privacy, countdown, pause/resume, stop/cancel, movie retention, conversion progress
and review. GIF mode records movie first and preserves it if conversion cancels.
Do not present movie decoding as recording or broadly call it GIF decoding.

## Incomplete alpha validation: preserve and resume deliberately

`handoff-recovery/INCOMPLETE-native-alpha-validation.patch` contains exactly322
added lines in two files on base0b8f44d: a test-only module declaration and
`native_capture/alpha_mask_tests.rs`. SHA256:
`4378cce0ea315889dab15745530b6ce6d087bcd844a87d0ed331eb88e1057c03`.
It was stopped immediately after its first source write. It is unformatted,
uncompiled, untested and not finally reviewed. It is not wired into this branch.
Review before applying; it is a recovery source artifact, not validated code.

Intended oracle: reproduce ImageAlphaMask.swift exactly using DeviceRGB,
PremultipliedLast, null bitmap data/automatic stride, interpolation None, draw
frozen image then DestinationIn shape, make an immutable image snapshot, encode
with the existing native PNG helper and decode through ScreenshotDocument.
Compare the public WindowRefreshPlan path, not a duplicate portable algorithm.
Use existing CoreGraphics bindings/core dev dependency; all CFRetained owners stay
on the test thread. No capture, permission or UI is required.

Pending validation: binding signatures/features, copied CFData/provider/image
lifetimes, explicit byte-order and alpha flags, independent input normalization,
asymmetric X/Y orientation, same-size and±1pixel sampling, all65,536 alpha pairs,
strict failure diagnostics and arbitrary low-alpha RGB reporting. Native8-bit
premultiplication can quantize straight RGB; do not invent tolerance or assert
universal byte equality. Strict alpha/sampling mismatches should drive investigation.
Run diagnostic output with `--show-output`. Passing a corpus proves its tested SDR
cases, not ICC/HDR or every CoreGraphics color path.

## Reproducible setup and checks

Use a fresh clone/checkout of the current `rust` branch. Do not restore old caches
or patches over it. Rust1.99.0 is pinned by both workflows. Commands from `rust/`:

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
rustup override set 1.99.0
cargo fmt --all -- --check
cargo test --locked -p bellobox-core --lib screenshot:: -- --test-threads=2
cargo clippy --locked -p bellobox-core --tests -- -D warnings
cargo test --locked -p bello-platform
cargo test --locked --workspace --no-run
cargo build --locked --workspace
```

Use focused tests appropriate to the edited component, then required CI; do not
repeat an entire matrix after every small edit. Existing macOS CI runs native
compile/link, core/workbench/platform tests, app build and offline packaging on
Apple Silicon with Xcode/SDK. It does not launch GPUI or grant native permissions.
Linux CI runs workspace tests/lints/build plus isolated Xvfb/Openbox/lavapipe startup
smoke. See `.github/workflows/rust.yml` and `rust-macos.yml` for exact commands.

Linux prerequisites are the official distribution packages listed in rust.yml:
build-essential, pkg-config, fontconfig, xkbcommon-x11, Wayland, X11-XCB, ALSA,
zstd, OpenSSL, Vulkan development packages, cmake/clang/lld and the isolated UI
smoke tools. The previous cloud environment lacked these. Its escalated setup
failed before launch because its sandbox could not create/inspect a synthetic
runtime mount target (bubblewrap reported `Not a directory`). No package fix was
applied by bypassing that boundary. Re-establish a supported environment; do not
modify protected runtime paths or search for credentials. Core tests worked with
cached crates. Local full Apple Cargo checking could also stop on missing optional
resolver-source manifests; actual remote Apple CI successfully resolved/compiled
those dependencies. Metadata-only local checks were never called SDK execution.

The final coordinated Linux prerequisite retry reached apt but exited100: it
could not write `/etc/apt/apt.conf.d/80-applied-apt-retries` and the
`/var/lib/apt/lists/partial` directory was missing. No packages were installed.
Do not bypass those access restrictions. There is no requirement to reconstruct
the deleted cloud machine; use a supported environment with its documented setup.

## Native and UI gates: do not erase these limitations

- Real TCC Screen Recording, AX selection/replace, AppKit focus/key restoration,
  Spaces/Stage Manager, multi-display/Retina alignment and protected content remain
  separate authorized runtime validation. No permission grant was performed here.
- Production Area/Window/movie gates stay disabled until their complete host and
  native lifetime/interaction contracts are validated. macOS13/x86 Window fallback
  is not silently available.
- Synthetic model/CI results do not prove live capture pixels, cursor/shadow/alpha,
  callback/main-loop behavior, close/resize/hotplug races or GUI pointer ergonomics.
- Historical Linux tool/scroll/Area screenshots from an earlier computer were lost
  during a prior workspace reset. Source evidence documents describe their scoped
  observations; do not claim the raw screenshots survive or treat them as current
  Mac validation. Recent Window/refresh work has no interactive GUI QA record.
- Linux Save PNG dialogs previously failed in the cloud environment, also on the
  QR baseline. Core PNG bytes were separately tested; no GUI-save success claim.
- No local paid provider request, secret lookup, Keychain access, release signing,
  notarization or Sparkle publishing occurred. Keep offline preview packaging
  separate from an actual signed update/release workflow.

## Prioritized successor tasks

1. Read this handoff and parity ledger; verify current remote SHA and exact CI.
   Preserve gates. Review/apply the alpha WIP in an isolated branch/worktree, run
   native synthetic comparison, fix proven differences without weakening assertions.
2. Complete and live-test the synthetic Window selector gestures/cancellation and
   refresh history on a supported GPUI environment. Then wire the guarded refresh
   path deliberately: occlusion replacement versus alpha-on-frozen pixels,
   document/token/topology fences, UI preview/OCR invalidation and failure recovery.
3. Finish one real capture vertical at a time: owned visibility/presentation,
   Area/Window host source behavior, inline overlay-editor gap, native catalog and
   current-topology acquisition, permission errors, cancellation/late callbacks,
   close/reopen and display changes. Native enablement requires evidence, not just
   toggling a constant. Scroll live acquisition/privacy is a distinct next vertical.
4. Connect the validated movie reader to a bounded standalone local movie→GIF UI,
   then implement recording as its own native lifecycle project with the source
   audio/privacy/countdown/pause/review semantics and cancellation-safe export.
5. Advance high-impact missing integrations: AX selection/global hotkey/floating
   toolbar, replace-in-place, provider/Codex app-server and model discovery,
   consented AI OCR, Clock copilot conversation/apply/handoff, native setup/menu
   bar/launch-at-login, secure vault/migration, accessibility/material fidelity.
6. Audit remaining developer-tool controls/engines against Swift rather than
   repeatedly polishing already completed narrow tools. Measure real interaction
   latency/memory/frame behavior only in a suitable environment, with scoped claims.

## Source survival and recovery inventory

Twelve Box worktrees were audited at freeze. Main `rust` is current; Clock's fork
is clean; alpha is incomplete. Other dirty forks are historical implementations
already merged and subsequently corrected. `handoff-recovery/source-survival.json`
records91 exact published source/index blob matches and five tiny preserved
historical variants (stale wording/evidence/module ordering/integration edges).
They are **DO NOT APPLY** artifacts, not pending feature merges. The alpha patch
is the only unintegrated implementation source and is deliberately preserved as WIP.
No worktree was reset or user source discarded.

Excluded from Git: compiled binaries/targets, downloaded crate caches, transient
acquisition stdout/stderr, duplicated old patch packaging and local audit scratch
files. These contain no unique migration implementation; project-level evidence,
source variants and resume instructions are here, and exact CI remains linked.
No private assistant notes, conversation exports, credentials or machine-specific
filesystem paths are exported in these recovery artifacts.

## LOC accounting

At implementation0b8f44d:41,889 nonblank production Rust lines,18,169 test/support
lines and105 benchmark lines. Comments count as nonblank physical lines. Test-only
external modules and positive `cfg(test)` / `cfg(all(test,...))` spans are tests;
`cfg(any(feature,test))` code remains production. Benchmarks/examples are separately
tracked. The count derives from verified previous Git-source totals plus exact
owned-file deltas; it is not a regenerated universal parser or completion metric.
The handoff adds documentation and a noncompiled patch, so active Rust counts stay
the same. Future counts must handle compound cfg correctly and exclude recovery
patch text from compiled source counts. Never turn LOC into parity percentages.


## Continuation checkpoint: synthetic host and native alpha diagnosis

This section supersedes the older pending fixture/UI statements above. The sole
incomplete alpha patch was reviewed and integrated (never apply it again). At
`adb877e`, exact Linux CI passed and native compile/link passed, but native tests
found a real portable ±1-pixel sampling difference. The expanded diagnostic at
`780ecd` retained strict assertions and demonstrated scale-dependent center-boundary
choices on both axes. No guessed tie tolerance or source-fidelity claim was added.
See [native-alpha-oracle.md](native-alpha-oracle.md) for exact failed-run evidence.

The opt-in DEBUG synthetic Window refresh now reaches the actual ScreenshotEditor:
front borrows alpha while keeping frozen orange; occluded back replaces with
independent blue. Success uses changed() to retire OCR and rebuild preview; one-shot
context/base/navigation/close/interaction guards preserve frozen usability on any
failure. Seven GPUI tests, 42 app screenshot tests, 163 core screenshot tests,
strict app all-target Clippy and app build pass. Actual synthetic Linux GUI
interaction and a final rebuilt-binary scoped check passed; evidence and exact
source/binary hashes are in [window-refresh-host.md](window-refresh-host.md).

Production capture/permissions/native host gates remain unchanged. The next native
step is a source-exact Rust/CoreGraphics supplied-image mask adapter with existing
bindings, retaining common core identity/publication gates. The portable nearest
sampler remains approximate at ±1 sizes. Do not label current macOS CI green, or
turn a future normalized-SDR native mask oracle into ICC/HDR/original-capture color
or macOS UI/Spaces/TCC acceptance. Current source counts are 41,895 production,
19,161 test/support and 105 benchmark nonblank Rust lines, with reviewed delta and
hashes retained beside the host evidence. They are not a parity percentage.

### Supplied-image native masking continuation

The next source checkpoint now contains the exact Rust/CoreGraphics local-image
adapter and its common core callback/publication seam. See
[native-alpha-oracle.md](native-alpha-oracle.md). It does not enable capture or
permissions. The portable nearest sampler is intentionally unchanged/approximate;
strict new tests compare the actual macOS adapter with an independent oracle on
identically normalized SDR inputs. The original native failures and portable
17-versus60 characterization remain recorded, rather than being called fixed
portable equivalence. 25 core refresh,129 platform and43 screenshot app tests plus
strict component Clippy pass locally; exact native CI is still pending at source
preparation. Updated scoped source counts are42,267 production,19,802 test/support,
105 benchmark lines, excluding concurrent converter work; reviewed hashes/delta
are stored with native-alpha evidence. Always inspect the exact published SHA's
native results before accepting the adapter or repeating these pending statements.


### Exact native mask checkpoint verified

`c41ce17a520a9bc39073e2302c5a061d29dd27df`, tree
`fd17a83c4ddf4d5b9e54158250d89ed1a7091f2e`, passed exact Linux37575287239 and
macOS37575287155. All9 alpha/oracle/lifecycle tests and the actual host worker
mask test passed; native platform153 passed,3 existing ignored, app build/offline
packaging passed. This supersedes the pending native statements for that code.
The normalized native RGBA path matches its oracle; portable resampling remains
17 versus60 in the retained case, and144-pixel low-alpha RGB characterization
reports89 straight mismatches/max129 and33 diagnostic premultiplied-projection
mismatches/max1. No tolerance or broader equivalence is inferred. Exact evidence
and remaining gates are in native-alpha-oracle.md and its linked validation JSON.
The subsequent documentation-only checkpoint does not change those tested inputs.


## Continuation checkpoint: standalone GIF host and bounded result preview

The `videoToGIF` route now opens a dedicated GPUI host. Metadata/export workers
are generation fenced; source changes reset trim/results but retain format
choices. Source-shaped paired sliders, exact numeric drafts, real encoding
progress, cancellation and result retention are wired to the existing bounded
Rust GIF exporter. `MovieAsset` remains production-disabled before source I/O;
this is a synthetic-export acceptance checkpoint, not usable native movie parity.

A DEBUG app-owned fixture supplies generated frames. The exported GIF preview is
streamed one frame ahead with validated timing, explicit Play/Pause, Once/Loop,
pinned-file rewind, owned-task cancellation and worker-prepared GPUI BGRA images.
Old GPU frames are explicitly retired. Preview starts paused; native movie seek,
Reduce Motion integration, source file clipboard and complete launcher handoff
remain absent. No source images/options/paths enter preferences.

Thirty-six converter regressions, thirty existing GIF regressions, strict app/core
all-target Clippy, app build and the exact debug-assertions-disabled test harness
pass. Actual Linux GUI testing found and fixed a skipped End-field Tab stop and
asset-loader blanking during playback, then verified the corrected candidate.
Seven unedited JPEG screenshots, binary/source hashes, scoped observations and
reviewed counts are preserved in [gif-converter-host.md](gif-converter-host.md)
and its validation manifest. The cloud system Open/Save dialogs fail; their
recoverable error handling is tested, while actual movie selection/conversion and
system-dialog save remain unverified. No native gate was enabled.

The reviewed converter delta is 1,178 production and 890 test/support nonblank
Rust lines: cumulative totals 43,445 / 20,692 / 105 benchmark. Concurrent inline
Area work is not part of this checkpoint or those totals. No completion percentage
or whole-project performance claim follows from these counts.

### Exact converter checkpoint verified

Published `c960d6e1a80ad915e7868133d8c1e95b538d076b`, tree
`e0f71c7c1a4293930afeacf28ac3648cd8373215`, passed Linux run
`37579346250` and macOS run `37579346240`. Native compile/link, pure
regressions, the strict normalized mask oracle and actual host mask worker,
application builds and offline development packaging all passed. These are the
exact converter checkpoint's CI results; its scoped interactive Linux evidence
and native movie/system-dialog limitations remain as recorded above.

## Continuation checkpoint: inline Area editor and owned retirement

The gated main-display Area coordinator now retains its existing overlay window
from selection through a real `ScreenshotEditor`, instead of opening a separate
popup. Shared annotation/history/export machinery runs over the immutable full
display with independent pixel ratios, eight resize handles, Select-move,
one-step crop Undo and a 44-point source-shaped toolbar. Window's existing
separate debug workflow and the popup's no-resize-handle policy are unchanged.

Selection lock ends deactivation cancellation before font preparation; later
focus loss preserves editing without requesting activation. Requester close,
navigation, topology and cancellation still fence publication. Preview/export/OCR
and actual session/tile disposal remain coordinator-counted through physical
worker completion after the overlay/editor is gone. Copy & Finish drains this
ownership before releasing capture busy. Known progress text clears on retirement
without erasing failure messages.

Visible resize drafts commit before Copy, Save or the common OCR snapshot;
active Crop/Mask/other annotation gestures refuse output until pointer release.
Tests cover the actual keyboard path, draft preservation and one Undo, preventing
stale wider or unredacted image export. Actual supplied-pixel Linux interaction
verified same-window selection/editing, annotation, resize/Undo, move, color-panel
hit shielding, focus-away persistence, Copy & Finish and clipboard image reimport.
See [inline-area-editor-host.md](inline-area-editor-host.md) and its linked
validation evidence for exact candidate hashes, tested scope and limitations.

`PRODUCTION_AREA_ENABLED` remains false. The DEBUG supplied-pixel route enters
the same coordinator without invoking native capture or permission APIs. Native
AppKit/TCC/Spaces, hotplug/multi-display, Retina alignment, system Save-dialog
acceptance and source live Scrolling Capture remain open. Xfwm skips the overlay
in Alt-Tab, so actual keyboard-only return is not claimed; logical focus has GPUI
regression coverage. These checks do not establish native usable capture parity.

The independently reviewed scoped delta is 828 production and 973 test/support
nonblank Rust lines, including comments; no benchmark lines were added. From the
published converter baseline this yields 44,273 production / 21,665 test-support /
105 benchmark lines. Exact per-file hashes, positive test/DEBUG-only spans and
counting rules are retained in `validation/inline-area-2026-10-07/loc-delta.json`.

All 61 affected screenshot app tests passed, including 18 new coordinator/inline
GPUI tests. The expanded error-retention fixture test, strict app all-target
Clippy, normal build, minimal-feature check, app-only debug-assertions-disabled
metadata check and formatting passed. The final GUI binary is
`6ca48fee5f603788d5968916ef7a6fc76320d7b7bad7e1347552fc010d544afb`;
its broader and final scoped interactions are distinguished in
[the QA report](validation/inline-area-2026-10-07/QA-report.md).
Exact Linux/macOS CI for this new source checkpoint is pending publication.

### Exact inline Area checkpoint verified

Published `0a73d42458b081e4d49c946acfeb1c0245dad011`, tree
`ebe64f7fc8734e90bbf51954df8c19ed7580c8cd`, passed Linux run
`37581683197` and macOS run `37581683028`. Every job step passed: native
compile/link, pure regressions, strict normalized alpha/lifecycle tests, supplied
host-mask worker, application build and offline packaging. This supersedes the
pending CI statement for the inline Area code. Native interactive capture,
permissions, AppKit/Spaces and the other recorded runtime gates remain open.

## Continuation checkpoint: shared Window overlay with supplied evidence

The Window host now uses the same owned capture coordinator and overlay as Area.
Its explicit DEBUG route is `BELLOBOX_INLINE_WINDOW_FIXTURE=1` with the existing
screenshot startup route. Ordered supplied records feed the existing Window-only
pointer policy and physically cropped `FrozenWindowCommit`; the editor never
owns the full display as its Window base. Ordinary Window capture remains
unavailable and the production Area gate is unchanged.

Exact f64 identity/frame/topology and submission evidence remain separate from
f32 selection chrome. Selectable normal-layer, non-own, wholly contained main-
display rows are distinct from the broader occlusion catalog, which can include
our own regular windows. Existing submission/completion policy checks and current
session evidence guard generated independent pixels. These are supplied records,
not native object retention or atomic incarnation guarantees. Front windows borrow
independent alpha; occluded windows replace frozen pixels through the same native
normalized-mask seam on macOS and documented portable approximation elsewhere.

Window's selected frame and toolbar remain fixed after Crop. Its visible image
uniformly aspect-fits with well-colored letterboxes, no Area resize handles or
Select-move. Pixel tiles alone clip at the fitted image, keeping cropped pixels
out of letterboxes while preserving text controls. The whole fixed frame owns
clamped annotation gestures. Area retains direct independent-axis multiplication,
including the regression that 330 points becomes exactly 825 pixels at 2.5×.

Acquisition, masking, publication and final rejected-result disposal retain one
coordinator worker count until physical completion, even after close. UI apply
rechecks the coordinator, session/evidence, document base, cancellation and clean
interaction state; accepted refresh calls the real OCR/preview invalidation path.
A narrow shared-backing `PreparedWindowRefresh` clone lets the completion retain
large immutable pixels for off-thread disposal. First acceptance advances the
base epoch, so clones cannot republish even after clean Undo. Controlled tests
hold disposal pending after cancellation, reject new admission and keep capture
busy until that disposal actually drains.


The exact supplied-pixel Linux GUI candidate is
`f002f201725dbd9a08bb2270948a073a7095b2642d7a35ce9a9df4c5f140c970`.
Scoped interaction passed front alpha preservation, blank/long-drag rejection,
fixed-frame Crop/Undo/Redo, contrasting clean letterboxes, edge-label editing,
Copy & Finish and 401×161 body-only clipboard reimport, independent blue back
Window with frozen orange overlap removed, and clean close. Five original
app-only JPEGs, binary/source hashes and exact limitations are retained in
[window-overlay-host.md](window-overlay-host.md) and its validation directory.

All 86 affected screenshot app tests and 168 core screenshot tests passed, as did
strict app all-target Clippy, core test-target Clippy, normal app build and
minimal-feature check.
The exact debug-assertions-disabled app test artifact also passed its 46 screenshot
tests. Native mask behavior remains covered by its existing worker-only exact CI
path; new exact-commit Linux/macOS results are pending publication for this source.

The reviewed scoped Rust delta is 178 production and 2,040 test/support nonblank
physical lines including comments, with no benchmark additions. New totals are
44,451 production / 23,705 support / 105 benchmark. DEBUG-only supplied evidence,
refresh host and fixture arms are support; shared editor/coordinator/core ownership
changes are production. Exact spans and hashes are in the scoped LOC manifest.

This remains normal-layer, wholly contained, unrotated-main-display supplied
acceptance. Native catalog ownership/acquisition, spanning/system-surface policy,
AppKit/Retina/Spaces/TCC and original-capture color/ICC/HDR remain open. Current
DEBUG evidence revalidation performs a bounded quadratic duplicate scan over at
most 512 observations under its mutex; pixel generation runs without that lock.
No measured latency/FPS or native responsiveness claim follows, and this cost
should be revisited before integrating a native catalog.

### Exact supplied Window overlay checkpoint verified

Published `f403966c4a55dc8af25fb7d725035e7f0cc72faf`, tree
`1a08993a0574eca80c6e48124cce5d19daf91bde`, passed Linux run
`37584275866` and macOS run `37584275876`. All steps passed, including native
compile/link, pure regressions, strict normalized alpha/lifecycle tests, supplied
host-mask worker, app build and offline development packaging. This supersedes
the pending exact-CI statements for that supplied Window source; the separate
native acquisition, catalog and interactive runtime gates remain open.

### Actual cloud system-dialog acceptance

The missing Linux FileChooser portal prerequisite is restored for a private
process-local session, with two verified official Debian packages extracted in the
workspace. No global service/settings or Rust source changed. On exact published
Window source `f403966` and immutable binary `f002f201…`, actual CUA Save
cancel/reopen wrote a verified 680×540 transparent-corner PNG. Converter Save
cancel/reopen wrote a verified 45-frame, 320×180, 3-second GIF. Open cancel retained
source/result; selecting a generated valid one-second H.264 MP4 reached the honest
native-movie-unavailable gate. Apps and private portal session closed cleanly.

See [cloud-dialog-acceptance.md](cloud-dialog-acceptance.md) for full binary/source
hashes, official package hashes, exact process-local launch recipe, six original
screenshots and verified output artifacts. This supersedes the earlier cloud
system-dialog failure statements only for this restored launch environment; it
is not native movie decoding or macOS dialog/capture acceptance. The ordinary
unwrapped desktop bus remains unchanged and still lacks its own portal service.

## Continuation checkpoint: production-compiled Window backend composition

Window-specific selection, refresh and editor publication now compile in ordinary
app builds alongside Area's shared coordinator. Generated pixels, supplied callback
records, evidence mutation and delayed/failure controls remain DEBUG/test-only.
Separate false Area/Window admission constants are checked before backend creation
or any display, permission, capture, clipboard or presentation operation. A second
side-effect-free native capability preflight rejects the unavailable catalog before
hide/freeze; real catalog enumeration would occur after the frozen display.

An owned backend now supplies exact observations/topology and acquisition through
the same host path. The native snapshot carries the exact Arc<WindowCapturePlan>
created from the callback-local retained SCWindow before filter submission. Native
acquisition remains test-compiled and public capture remains Unavailable; missing
raw occlusion-catalog/thread/runtime evidence is not fabricated from strict selector
rows. No native object is made Send and no activation gate is enabled.

Independent review found that checking a returned plan only against its own target
was insufficient. Completion now must match the exact original selection Arc owner
and requested options, before generic refresh and before native PNG decode. Same-
session/same-sized other-window plans, reconstructed equal-looking selections and
changed cursor/deadline options are rejected. A negative control removing this
check caused the real coordinator regression to publish wrong independent pixels;
restoring the exact check made the focused/full suites pass.

The hardened immutable Linux binary is
`7681a1008962862464e2342bbf4cc9eacbf9e74bf6df49f6c6fa362fa6344056`.
Actual CUA covers front/back, fixed-frame Crop/Undo/Redo and letterboxes, edge text,
Copy & Finish/reimport, delayed refresh while editing, one-shot rejection after
Undo to clean, controlled failure retaining usable pixels, close before completion,
and the shared Area resize/move regression. Eight original screenshots and exact
source/binary links are in [window-backend-host.md](window-backend-host.md) and its
validation directory. This remains supplied-pixel interaction, not native capture.

Final automated evidence includes 96 screenshot app tests, 88 in the exact app-only
debug-assertions-disabled test executable, 130 platform tests with 3 existing ignored,
strict app/platform all-target Clippy, minimal-feature check, normal app build and
warning-free app-only non-DEBUG metadata compile. Exact new-commit Linux/macOS CI,
native completion ownership and strict mask/worker tests remain pending publication.
Observation-thread placement and measured latency remain native activation gates;
owned snapshot cloning and the 512-row bounded quadratic duplicate check do not
establish a responsiveness claim.

The final cfg(test)-only paused-disposal helper was then strengthened to enter
`Source::request` and backend acquisition/publication. Its 11 coordinator tests
passed again in ordinary and app non-DEBUG configurations, with strict app Clippy.
No normal executable input changed after the hardened GUI pass; validation.json
retains both GUI and final test-source hashes.

The reviewed net Rust delta is +901 production / +108 support / +0 benchmark,
yielding 45,352 production / 23,813 support / 105 benchmark. The auditable literal-
line decomposition separately records 651 preserved support lines reclassified to
production and 3 in reverse, plus 266/13 production lines added/removed and
1,114/358 support lines added/removed. Fixtures remain support. This is scoped
nonblank physical-line accounting including comments, not feature completion or
engineering-effort measurement; see the before/after ranges and source hashes in
`validation/window-backend-2026-10-07/loc-delta.json`.

### Exact backend/dialog checkpoint verified

Published `505e8f193de858163474e2af269d2e3ca51e475a`, tree
`8477ad4e3b0a7da76db42de83530faccc5cb6f6a`, passed Linux run
`37591717754` and macOS run `37591717776`, with every step successful. Native
logs include the exact returned-plan ownership regression, 155 platform tests
with 3 existing ignored, 9 filtered alpha/lifecycle tests, the worker-only mask
integration test and 354 core tests. Native app build and offline development
packaging also passed. This supersedes the preceding pending-CI statement only
for that immutable source; new catalog changes need their own exact verification.
Actual SCK capture, permissions, callback runtime and native GUI remain unvalidated.

## Continuation checkpoint: owned asynchronous native Window bridge

The Apple Silicon Window catalog and independent acquisition now compile in normal
platform builds behind the still-false platform Window admission gate. App Area and
Window gates remain false. Exact f64 selector/topology data and source-shaped raw
partial occlusion rows are separate owned transports; actual platform window levels
are preserved. No native callback submission is fabricated from catalog rows. The
retained callback-local SCWindow still creates the exact returned selection-bound
plan before its independent filter requests pixels.

Optional decision observation now starts on the counted refresh worker after the
frozen editor mounts. Final observation follows masking; foreground publication
consumes one owned proof and checks deadline/cancellation/session/generation/base/
liveness/clean-state guards without native observation. This is freshness at the
completed observation, not an atomic live-window guarantee. Native revision zero
means no OS revision feed; supplied mutation tests use an explicit revision fence.

Independent lifetime review found two integration gaps and both were corrected:
Window's initial full-display freeze now waits for physical Display callback and
encoder drain before catalog admission, preventing a transient Busy handoff. The
ordinary Display/Area return timing is unchanged. All Window drained paths recheck
late cancellation/deadline, with selection/session checks for independent capture.
A regression handshake proves success was consumed before cancellation/expiration
while the final lease remained held. Logical timeout never releases physical
admission early; a framework callback that never releases can retain the worker.

Local final checks pass 103 screenshot app tests, 95 in the exact app-only
no-debug-assertions test artifact, 135 portable platform tests (3 existing ignored),
169 core screenshot tests, strict Linux app/platform all-target Clippy, minimal
check, normal build, app-only non-debug metadata check and Apple-target platform
all-target metadata Clippy. The macOS workflow now also runs strict platform Clippy.
Metadata checks do not establish SDK linking or native execution; exact new-commit
Linux/macOS CI remains pending publication. Native CI uses constructed CF rows,
scalar topology and fake dispatch contexts only, without live enumeration/capture.

The immutable Linux candidate binary SHA256 is
`d328600a3a4c11a0532ea16ab3c545a96290d61b1ec04cf14c826b02974a414d`.
Actual supplied-pixel CUA acceptance passed front/back refresh, fixed Crop/Undo/
Redo/letterboxes, system Save cancel/reopen, verified 401×161 cropped PNG and
Copy & Finish/reimport, delayed observation with editing and one-shot Undo,
controlled failure, close before observation return with no late resurrection,
and shared Area resize/Undo/Select move/Copy & Finish. Twelve original JPEGs and
the saved PNG are retained. The edit and close occurred during an explicit
eight-second supplied stall; their timestamps establish overlap only, not a native
latency measurement. See exact source/binary and interaction evidence in
`validation/native-window-catalog-2026-10-07/QA-report.md` and `validation.json`.
See [the native bridge contract](native-window-catalog.md) for source anchors,
transport/parser bounds and ownership details. Catalog collection still runs two
CG queries and four topology passes on main; asynchronous ownership does not prove
native responsiveness. TCC, AppKit/Spaces, callback runtime, original capture color,
ICC/HDR, Retina/hotplug and actual independent pixels remain native activation gates.

The independently checked Rust delta is +1,386 production / +387 support / +0
benchmark, yielding **46,738 production / 24,200 support / 105 benchmark** nonblank
physical lines including comments. The ledger explicitly separates 659 preserved
support lines promoted to production (647 native window, 11 owner guard, 1 phase)
and 22 in reverse, plus 813/64 production lines added/removed and 1,088/64 support
lines added/removed. Nineteen source hashes, positive support ranges and literal
reclassification pairs were independently verified. No feature-completion or
engineering-effort percentage follows from these counts.
