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
