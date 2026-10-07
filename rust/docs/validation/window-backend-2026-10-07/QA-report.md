# Production-compiled Window backend: scoped acceptance

Date: 2026-10-07 UTC. Baseline: f403966c4a55dc8af25fb7d725035e7f0cc72faf.
The modified source is not yet a published native acceptance checkpoint. Production
Area, Window and native movie activation gates remain false/unavailable.

## Exact candidate and checks

All retained interactive observations below used immutable Linux binary SHA256
7681a1008962862464e2342bbf4cc9eacbf9e74bf6df49f6c6fa362fa6344056. Source hashes
are in validation.json and the explicit per-file LOC ledger. The earlier
f6b31a50 build was not used for this acceptance: a source review found a returned
plan binding defect before GUI testing began, and the candidate was rebuilt.

Passed after that correction (the full suites precede only the narrow test-only
strengthening noted below):

- 96 screenshot application regressions, including the actual shared Window
  coordinator, callback source mismatch, wrong returned plan, physical disposal,
  clean-state/history, editor publication, and existing Area regressions.
- 88 screenshot regressions in the exact app-only debug-assertions-disabled test
  executable. The Window workflow and its lifecycle tests remain compiled here;
  only separate legacy DEBUG fixture hosts are omitted. Dependencies retained
  their ordinary debug build settings; this is not an all-workspace release test.
- 130 portable platform tests, with 3 existing ignored tests.
- Strict app and platform all-target Clippy; minimal-feature app check; normal
  app build; warning-free app-only debug-assertions-disabled metadata compile;
  formatting and diff whitespace checks.

Reproduction commands, from rust/, with the documented pinned toolchain:

    cargo test --locked -p bellobox-app screenshot_ui:: -- --test-threads=2
    cargo test --locked -p bello-platform
    cargo clippy --locked -p bellobox-app --all-targets -- -D warnings
    cargo clippy --locked -p bello-platform --all-targets -- -D warnings
    cargo check --locked -p bellobox-app --no-default-features
    cargo rustc --locked -p bellobox-app --bin bellobox -- -C debug-assertions=no --emit=metadata
    cargo rustc --locked -p bellobox-app --bin bellobox --tests --profile test -- -C debug-assertions=no --emit=link
    cargo build --locked -p bellobox-app

Run the executable emitted by the last test-harness command with screenshot_ui::
and --test-threads=2. Do not substitute the ordinary test binary or count the
metadata-only command as test execution. Exact new-commit Apple compile, synthetic
native completion ownership, native mask oracle and host-worker checks remain CI
gates after publication. No local macOS execution or screen capture was performed.

After GUI acceptance, one cfg(test)-only helper was strengthened to create its
refresh through window_workflow::Source::request and Request::supplied rather than
the direct Request::fixture shortcut. The paused-disposal/coordinator-retirement
regressions therefore also exercise the production backend acquisition and
publication seam. All 11 coordinator Window tests and strict app all-target Clippy
passed again. Normal GUI executable inputs and the retained binary were unchanged;
validation.json records both GUI and final test-source hashes. This adds seven
support lines only. The final no-debug-assertion coordinator rerun is recorded in
validation.json.

## Review finding and negative control

The first injected backend contract accepted any otherwise valid returned
WindowCapturePlan in the same source/session. That was insufficient: a different
valid catalog window can have equal dimensions. Review required binding the
completion to the exact WindowCaptureSelection Arc owner and exact capture options.
The new matches_request predicate enforces this. Generic acquisition checks it
before masking/refresh; the native result adapter checks it before PNG decoding.
The actual callback still validates its retained SCWindow before requesting pixels.

The same-coordinator regression injects a valid other-window/same-session/
equal-dimension plan, equivalent metadata under a different selection owner, and
changed cursor/deadline options. Each retains the original frozen pixels.
Temporarily removing only the generic request-binding check caused the regression
to fail with “Window · independent pixels” where it required “Window · frozen
pixels.” The exact source was restored, and the corrected focused and full suites
passed. The invalid-native-PNG test also verifies request binding precedes decode.

## Actual supplied-pixel Linux interaction

Launch BELLOBOX_TOOL=screenshot with BELLOBOX_INLINE_WINDOW_FIXTURE=1 and an
isolated configuration directory. The fixture uses the same production-compiled
preparation, coordinator, selector, inline editor and backend-driven refresh.
Only the source evidence and pixels are generated. Real desktop pointer/keyboard
input was used; window-scoped injected X11 input did not reliably target this
notification-style overlay and was not used for accepted gesture claims.

1. Front Window selection retained frozen orange RGB and borrowed rounded alpha
   from independent pixels. The selected frame stayed fixed. 01-front-window.jpg.
2. Blank clicks and long drags remained in Window selection without Area fallback.
   A later exposed-back click selected the back window correctly.
3. Cropped the front image to its body. The dark header disappeared, the image
   uniformly fit inside the original frame, and the top/bottom well bands stayed
   clean. Undo restored the full selected image; Redo restored the cropped fit.
   02-fixed-crop-redo.jpg. No Window resize handles were present.
4. Clicked the text control near the fitted image edge and entered “Edge.” The
   control remained hit-testable; committed the text and removed it with Undo.
   03-edge-text.jpg.
5. Copy & Finish removed the owned overlay and cleared the chooser's busy/progress
   state. Paste Image reimported the actual clipboard image: 401×161 pixels, only
   the orange body, with no excluded header, display pixels or letterbox bands.
   04-clipboard-reimport.jpg. Fractional pointer/layout coordinates were not traced;
   the observed size is recorded, not rounded to an assumed 400×160.
6. Reopened and selected the exposed blue back Window. Independent blue pixels
   replaced its frozen orange occluder. 05-back-window.jpg.
7. With BELLOBOX_INLINE_WINDOW_REFRESH=delayed, the frozen back editor became
   usable during the eight-second acquisition delay. A rectangle drawn before
   completion remained, together with the original frozen orange overlap, after
   completion. Undo to clean did not republish the already rejected result.
   06-delayed-edit-preserved.jpg. An earlier timing attempt drew only after refresh
   had settled; it was repeated with both actions in one input sequence to verify
   editing while the supplied worker was genuinely pending.
8. With BELLOBOX_INLINE_WINDOW_REFRESH=failure, drew a rectangle while the worker
   was pending. After its controlled failure, the frozen image, overlap and edit
   remained usable. 07-failed-refresh-edit.jpg.
9. Reopened another delayed/failing worker, entered its frozen editor, and pressed
   Escape before completion. The overlay closed, the chooser returned to idle,
   and no late editor reappeared after the full observation interval. Final
   completion/disposal Busy retention is additionally tested with a paused actual
   background-disposal future, not inferred from this quick cooperative cancel.
10. Area regression on the same binary: selected a region, resized with its corner
    handle, undid that resize in one step, moved the region with Select, and
    completed Copy & Finish. The eight handles and direct-ratio image view stayed
    distinct from Window's fixed frame. 08-area-regression.jpg. The exact 2.5×
    independent-axis mapping remains covered by the existing GPUI regression.

All eight retained images are original app-only JPEG tool outputs, with no
transcoding, cropping, overlays or image editing. No raw desktop/terminal
screenshots or private user inputs are included. The shared desktop was left
with the test application closed after the successful final cleanup.

## Limits

This is supplied-pixel Linux interaction, not native SCWindow/filter acquisition,
TCC/AX permission, original image color/ICC/HDR, AppKit/Spaces/Retina/hotplug or
native focus/accessibility acceptance. System Save dialogs are not part of this
Window matrix; separate cloud-dialog evidence records that environment's results.
Initial/changed previews transiently showed the empty well while their asynchronous
tiles loaded, then settled correctly. No frame-latency or responsiveness benchmark
claim is made.

The normal-build evidence validation still clones bounded owned snapshots and
performs an O(n²) duplicate check over at most 512 complete observations; raw
occlusion rows have their separate bound. The concrete native backend is
Unavailable. Native observation-thread placement and measured UI latency remain
activation gates, rather than being inferred from this synthetic implementation.
