# Immutable image OCR consent host

This implementation is an image-specific ScreenshotEditor flow. It does not
turn on ordinary screenshot uploads, native capture, credentials, paid provider
access or any Area/Window/movie gate. Ordinary AI OCR stops at the production
admission function before settings, environment keys, client construction or DNS.
The independent text-only AI transport remains separate.

## Source contract and deliberate limits

Behavioral anchors are `ScreenshotPopupView.swift` 569–615 (prepare/confirm),
767–777 (image OCR options), 1088–1127 (preview and disclosure), and
`OCRPanelView.swift` 87–130 (literal Text/Markdown and explicit Copy/Save).
`ScreenshotPopupViewModelOCRTaskTests.swift` 229–261 explicitly preserves the
approved provider/config when settings change before confirmation. This host
preserves that snapshot as well: ordinary setting edits never retarget an
existing approval. Explicit authority revocation rejects dispatch and publication.

The Rust confirmation improves the byte contract rather than copying Swift's
later re-encoding: preparation retains the final crop/mask-aware PNG, and the
preview is decoded from precisely those bytes on a worker. The immutable PNG,
options, digest and provider lease are carried into dispatch unchanged.
Decorations are excluded. Masks and their intentional eraser holes are applied
before hidden transparent RGB is flattened onto the disclosed opaque white
background and before any downsampling. Core parser/render behavior is documented
in the companion transport implementation and its tests.

The first slice does not provide local-text hints, hybrid recognition or structured
OCR regions. These are visibly unavailable. Text and Markdown use the same plain,
read-only editor; neither HTML, links nor image syntax is rendered or fetched.
Results do not automatically copy or save. Save writes a mode-0600 sibling on Unix,
then publishes without overwriting an existing destination; cancellation and
write failure remove the staging file. This is a bounded result file, not a
persistent OCR history or a provider-content log.

The existing environment-key configuration has no live disconnect/revocation
feed. The opaque provider authority supports actual explicit revocation, exercised
synthetically; it does not invent a live credential revocation signal. Production
activation still requires that integration and independent runtime approval.

## Consent and stale-work fences

The common export preparation boundary finishes inline selection drafts, label
drag and pending text, and refuses active crop/mask/eraser/annotation gestures.
Approval binds the unique job session/generation, edit-session/base token,
revision, monotonic interaction epoch, retained payload and frozen provider lease.
Privacy-relevant pointer-down invalidates old approval and result before a
revision changes. Undo, Redo, refresh/base changes, cancellation, owner close and
revocation cannot revive an old approval.

Confirmation is single-use: it is removed before dispatch validation. Its modal
covers the canvas and toolbar, intercepts keys even when the reader had focus,
and contains Tab/Shift-Tab, Escape, Space and Enter. Initial Enter selects Cancel;
Tab selects the explicit Upload and Improve action. Inline Copy & Finish cannot
receive that Enter. Canceling a retry may retain a previous result only while its
image/session/interaction and authority remain valid. Invalidation resets the
reader's entire text engine, including selection, native copy data and Undo.

## Physical ownership

One process-wide physical lease spans preparation, held confirmation bytes,
request/client/body, bounded response parsing and final off-thread disposal.
Logical cancellation never frees admission early. There is no unbounded retry
queue. Each inline job also owns an additive RAII coordinator count: its count
retires only after its heavy owners are dropped, even if the editor entity or
window no longer exists. UI callbacks only reevaluate coordinator release.
Matching active transactions remain countable when cancellation races the earlier
logical admission check. A missing or mismatched inline owner refuses preparation
and Save; it never silently starts uncounted work. Actual coordinator regressions
hold the prepared image and final disposal across that precise cancellation gap.

The current-window GPUI atlas matters. Confirmation cancellation defers the whole
owned preview until the borrowed window returns to App, then evicts its GPU image
before worker disposal. Approval passes the current window explicitly to image
retirement and keeps its CPU owner inside the same physical lease. A window-closed
observer handles direct window destruction even when a caller retains the editor
entity. The app-quit handler can drop retained modal owners on a background worker
without creating a forbidden new foreground task during shutdown. GPUI's bounded
shutdown grace is not a promise of forced cancellation of an arbitrary provider
or a whole-process hard deadline.

## Supplied-image route and verification status

The finite DEBUG routes all use the same owned numeric-loopback fixture and
shared editor/controller/transport:

- `BELLOBOX_AI_OCR_FIXTURE=1`: popup over the generated 960×540 document.
- `BELLOBOX_AI_OCR_FIXTURE=area`: actual frozen Area pointer selection, retaining
  the generated base owner through crop selection and the owned inline host.
- `BELLOBOX_AI_OCR_FIXTURE=window`: an already-generated fixed Window frame,
  automatically mounted in the real owned selector/editor. Crop letterboxes in
  that fixed frame and there are no Area resize handles. This route does not
  exercise native Window catalog/capture/refresh or physical Window extraction.

There is no arbitrary image, path, endpoint, key or response constructor. The
permit binds both the generated base-image owner and exact authority owner;
the editor additionally binds the original base-generation token so restoring an
old image through Undo cannot restore upload authority after source replacement.
Ordinary imported/clipboard/user images remain blocked in this build. Actual
window closure drops the fixture owner even if a caller retains the editor entity;
an active worker keeps its own permit only until physical completion/disposal.
The reader's finite response control exercises success, literal Markdown,
HTTP failures, redirects, size limits, timeout and disconnect behavior through
the real image transport/parser. The redirect listener must remain at zero.

Candidate 1 local checks (immutable GUI binary `4a4cf2f4…c78e53`):

- The final ordinary full app suite passed all 285 tests. The exact app-only
  debug-assertions-disabled executable passed all 277 tests; this is not a
  whole-workspace optimized release run.
- Combined app/platform all-target strict Clippy with `movie-fixtures` and the
  app minimal-feature check passed. Core screenshot tests passed 187 and core
  test-target strict Clippy passed.
- The 16 popup/controller and 12 actual-inline regressions include modal input
  routing, single-use approval, frozen configuration, pointer-start invalidation,
  literal Copy/Save, retained entities after window destruction, shutdown,
  immutable bytes, real numeric-loopback HTTP and canceled-between-check/count
  physical ownership. Transport's 11 tests include all three provider formats,
  IPv4/IPv6, redirect/proxy traps, size bounds, read/total deadlines and explicit
  cancellation/revocation during an actual pending body read.

Deterministic inline holds retain actual prepared PNG/preview bytes, actual
completed HTTP/parser results and actual rejected-result disposal owners. The
HTTP completion hold is after parsing; it is not evidence of UI interaction while
a socket remains blocked. The dropped-Save-awaiter regression holds the same
physical writer after a real private staging write and sync, drops its result
receiver, closes the editor, then releases the worker and verifies cancellation,
cleanup and RAII count retirement. No response or successful write is fabricated.

The source-replacement regression genuinely restores the original generated
image Arc through Undo while its monotonic base token still refuses upload.

Actual candidate-1 CUA used the supplied generated document and same-process
owned numeric-loopback listener. It verified popup crop/mask sanitization,
transparent-background disclosure, decorative exclusion, modal cancellation at
zero requests, single explicit dispatch, literal Markdown, explicit private file
save, HTTP error/oversize/redirect recovery and late-response suppression after
Crop plus Undo. Actual inline Area selection/resize confirmed the effective crop;
Escape closed its editor during an eight-second response delay while the same
process's chooser remained busy until physical drain. Fixed-generated Window
confirmed its crop within the unchanged selected frame, zero-dispatch default
Enter and one successful request. The source and original screenshot manifests
are under `validation/ai-ocr-2026-10-07/`.

That CUA also found three UI defects: OCR Save displayed the screenshot-export
caption, warnings collapsed in a constrained inline reader, and a full-frame
Window status banner overlapped its toolbar. Candidate 2 contains the reviewed narrow repairs and passed all 287 ordinary
app tests, all 279 exact app-only debug-assertions-disabled tests, combined strict
Clippy, minimal-feature check and the immutable normal build. Its affected actual
CUA passed on immutable binary `136076adc9…346af5`: both Save captions and
Cancel/reopen, literal private file output, visible constrained warnings, status
clear of the toolbar, actual delayed HTTP close/drain, and minimum-size modal
keyboard/scroll containment. See the [final local report](validation/ai-ocr-2026-10-07/QA-report.md).
Candidate-1 evidence remains separately attributed. Exact published Linux/macOS
OCR CI is recorded below, with the macOS app-test execution gap kept explicit.
No native macOS capture/provider runtime or complete screenshot parity is claimed.


## Final local acceptance checkpoint

Candidate2 passed287 ordinary app tests and279 app-only no-debug-assertions tests,
strict app/platform Clippy, minimal-feature check, and actual popup/Area/fixed-frame
Window GUI acceptance. Three desktop findings were repaired and rechecked: the
OCR Save caption, collapsed provider warning viewport, and overlapping Window
status/toolbar. Actual delayed HTTP plus edit/Undo or owner close cannot republish
stale text; the same-process chooser remains occupied until physical drain.
Literal Markdown Save was43 bytes, mode0600, with exact retained content.

A later fixture-only accepted-socket correction normalizes Darwin's inherited
O_NONBLOCK before existing bounded timeouts. It changes two DEBUG/test support
files, not production transport or UI. The pre-correction GUI source snapshots
and candidate2 manifest remain intact; a separate publication manifest records
new hashes. All12 transport regressions, final strict app Clippy and format pass.
See `validation/ai-ocr-2026-10-07/QA-report.md` for the immutable local acceptance.

## Exact published CI and focused macOS execution

Commit `91f9a8484a09801845b83505fcfbd32183bc6e6d`, tree
`b775300cda6eb34d15d3dc8d8fdfe4d3b44c4065`, passed
[Linux 37645491894](https://github.com/BelloWare/BelloBox/actions/runs/37645491894)
and [macOS 37645491886](https://github.com/BelloWare/BelloBox/actions/runs/37645491886).
Linux executed all 288 app tests, including 12 transport, 18 controller and
12 inline OCR tests. macOS compiled those app tests but did not run these modules;
its successful existing fixture/build/package steps are not OCR app execution.

The focused CI continuation runs these three filters serially on macOS:

```sh
cargo test --locked -p bellobox-app transport::image_ocr::tests -- --test-threads=1 --show-output
cargo test --locked -p bellobox-app screenshot_ui::ai_ocr::tests -- --test-threads=1 --show-output
cargo test --locked -p bellobox-app screenshot_ui::main_area::selector::ai_fixture::tests -- --test-threads=1 --show-output
```

The two controller modules use `TestAppContext` and GPUI's test platform. Their
synthetic windows, clipboard and chooser tests do not launch the native app or
establish real macOS GUI behavior. Transport uses only the generated-image sealed
numeric-loopback route. Check the new commit's exact run before claiming macOS
execution; no Rust source, production admission gate or dependency changes here.
The public-repository/free-runner guard stays intact. See the source-name/hash and
CI record in [`published-ci.json`](validation/ai-ocr-2026-10-07/published-ci.json).
