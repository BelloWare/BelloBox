# Portable scrolling-session foundation evidence

Date: 2026-10-06 UTC. Base: `eb54763eadccc7c61c12ea59a44a38990ed9bde8`.
Worktree: `BelloBox-scroll-session`.

## Scope

This is a portable session/stitch foundation and a debug-only, app-owned synthetic
scrolling fixture. It is **not completed native scrolling capture**. No real screen,
clipboard, provider, OS synthetic scrolling, permission prompt or user computer was
used for these checks. Production Area/Window/Scrolling acquisition remains unchanged.

The implementation follows Swift `ScrollCaptureEngine`, `ScrollSampleAnalyzer`,
`ImageStitcher`, and the synthetic HUD demo in `SelectionOverlayController`.

Implemented core boundaries include serial sample jobs; session/generation/operation
and frame-count identities; cooperative cancellation inside pixel loops; two final
samples on Finish; final-sample recovery notes; sticky bands and seam accounting;
height-limited trailing-frame omission; preserved capture notes; and a checked,
conservative 512 MiB working-set budget. Frames omitted from an output remain counted
while the session still retains them.

## Actual validation

The following command was run from this worktree's `rust` directory, using the
parent-granted focused core slot:

```sh
CARGO_HOME=/workspace/scratch/5ddc1675ed73/.cargo \
RUSTUP_HOME=/workspace/scratch/5ddc1675ed73/.rustup \
CARGO_TARGET_DIR=/workspace/scratch/5ddc1675ed73/target-clock-ui-final \
CARGO_BUILD_JOBS=2 \
/workspace/scratch/5ddc1675ed73/.cargo/bin/cargo test \
  --locked --offline -p bellobox-core --lib screenshot::scroll -- --test-threads=2
```

Initial checkpoint result:

```text
Finished `test` profile [unoptimized] target(s) in 3.80s
running 32 tests
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured;
189 filtered out; finished in 1.72s
```

The first attempted invocation ran zero matching tests because the preliminary
module-insertion command used the wrong relative path. That run provides no feature
evidence. After the approved `pub mod scroll;` declaration was inserted, compilation
caught one incorrect Option pattern; it was fixed before the successful 32-test run.

All owned Rust files passed direct-toolchain rustfmt. `git diff --cached --check`
passed. No app build, GPUI link, binary copy, clippy, workspace matrix, native UI
interaction or export-to-filesystem test has run for this feature yet.

The 32 tests cover:
- Exact two/three-frame overlap composition and original pixels; upward ordering
- Sticky header, 40/44/120-row footer, disappearing footer, repeated middle content,
  blank seams, and full-resolution refinement after 2x downsampling
- Unchanged, continuously changing, fast-scrolling and reverse-moving samples
- No initial frame; frame, dimension, input, memory and output-height bounds
- Two final samples, final-sample failure recovery and warning order
- Session/operation isolation, stale analysis/Finish, repeated actions, cancellation,
  cancellation after computation, closing via session Drop, and restart
- Preview row accounting, bounded thumbnail PNG, and synthetic frame PNG round trip
- Completed session into the existing annotation editor model, opaque mask, crop,
  complete PNG render/decode and tiled-preview path

## App integration contract (not yet compiled)

New child module: `bellobox-app/src/screenshot_ui/scroll_capture.rs`.

Parent integration should:
1. Declare `#[cfg(debug_assertions)] mod scroll_capture;` in `screenshot_ui.rs`.
2. Route an explicitly supplied `BELLOBOX_SCROLL_FIXTURE` debug fixture to
   `scroll_capture::open_fixture(cx)` before opening the ordinary chooser.
3. Supply `open_scroll_result(result: ScrollResult, cx: &mut App)`, initializing
   the existing screenshot editor, its usual local font, Fit Width, and capture notes.
4. Keep the production scrolling chooser unavailable.

The fixture exposes Start, a synthetic page that responds to wheel/arrow scrolling,
manual-mode HUD, bounded preview strip, Finish, Cancel and Escape. Its worker closures
use the same core session jobs. The close callback cancels session/preview work.
The entrypoint and synthetic frame generator must remain gated out of release builds.

Required next verification: compile the parent integration, exercise the real GPUI
fixture on an isolated cloud display, confirm notes/Fit Width, export PNG through the
existing save workflow, decode/inspect dimensions and masked pixels, test save-dialog
cancellation/non-overwrite, and verify Cancel/close/reopen and interrupted Finish.

## Explicit deviations and remaining gates

- Grayscale downsampling is deterministic nearest-neighbor, whereas Swift uses
  CoreGraphics low interpolation. Full-resolution seam refinement and raw pixel
  composition are preserved; the checked 2x off-grid case passes. This does not prove
  interpolation equivalence for every scale/content combination.
- Standalone arbitrary-width imported-image normalization is excluded. Sessions
  require identical capture dimensions; no new public PNG import workflow was added.
- Manual fixture only: native auto-scroll, Accessibility-controlled input, region
  acquisition, native display/window resolution and capture permission behavior
  remain unimplemented/unverified by this slice.
- The aggregate memory guard is an intentional Rust safety boundary absent from the
  corresponding Swift engine. It may reject captures earlier than the Swift source.
- Linux core tests do not validate ScreenCaptureKit, macOS display topology/Retina,
  frozen selection overlays, protected content, capture trust/fallback policy,
  recording/GIF, signing, packaging or publication.

This checkpoint calibrates portable pixel processing and asynchronous lifecycle work.
It cannot establish a whole-handoff ETA for the outstanding native media bridges.


## Independent-review fixes and revalidation

Independent review found that the original synthetic page's full-width, unsmoothed
row noise and repeating 37-row stripes could mislead the source-shaped four-row
coarse search. A real fixture step from 0 to 90 pixels could append an unmatched
640-pixel result instead of the correct 410 pixels; another step could omit rows.
The original random-content tests did not cover this actual fixture pattern.

The debug fixture now renders spatially smooth, globally distinct color anchors,
linearly interpolated over 16 document rows, with narrow 8-row markers based on the
Swift fixture. This is a portable synthetic visual approximation, not a screenshot
of the Swift NSView. Production overlap scoring, refinement and warnings were not
changed or weakened. New regressions use `synthetic_page_frame` itself and compare
every stitched pixel with the same absolute-page generator:
- Every 90-pixel pair and accumulated session from offset 0 through 900
- Wheel offsets 17, 53, 119, 207, 293, 381, 461, 501, 674 and 801
- Correct overlap, output dimensions, retained row accounting and seam notes

The app also now uses an explicit `FinalSampleSchedule` deadline. Once the first
final sample is accepted, the next final sample cannot start until
`max(50 ms, sampleInterval)` has elapsed from that completion. A periodic timer tick
cannot bypass the deadline. Controlled-clock core regressions establish that a first
completion at 139 ms blocks a tick at 140 ms and allows the second sample at 279 ms;
the 50 ms minimum and reset are covered too. Another controlled session verifies
that the delayed second sample retains the final scroll and exact 410-pixel output.

The same full focused command above was rerun after both changes:

```text
Finished `test` profile [unoptimized] target(s) in 4.73s
running 36 tests
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured;
189 filtered out; finished in 11.00s
```

A preliminary actual-fixture-only run passed both new pixel regressions in 8.51 s.
No app build/link, native UI validation or file publication was performed during
this review-fix pass. The app's deadline wiring remains subject to parent compilation
and independent review; the core scheduling helper and exact-pixel regressions are
validated by the 36-test run.

## Final integration on the current Rust branch

The implementation is integrated additively on GIF foundation
`e4e508fd2aff1b12aa26359776d44623ec715238`, which includes the Number Base checkpoint.
The DEBUG route and child are gated with `debug_assertions`; the ordinary chooser
still does not enable native Area/Window/Scrolling capture. Accepted stitch results
prepare the editor and optional local font on a background worker. A window-owned
generation token rejects the result after Start, Cancel or native close. The editor
opens at Fit Width, keeps frame count in the subtitle, and shows ordered source notes
with two collapsed entries, a 132px bounded expanded list, and Dismiss. Notes remain
separate from OCR content.

Independent final review found no blocking defect in this scoped debug flow after
correcting the always-visible frame count. The actual combined tree passed:
- locked/offline strict Clippy for bellobox-app (including the affected core library)
- 36 focused scroll tests, 0 failed, 226 filtered, 8.63s runtime

Two Clippy-only changes use equivalent let-chain and is_multiple_of expressions.
Existing proc-macro-error2 emits an upstream future-compatibility advisory, not a
new feature warning. Real GPUI interaction and export validation are pending the
immutable candidate run; no native capture or performance/FPS claim follows from
these checks.

## Scoped real GPUI fixture QA

Immutable Linux candidate SHA-256:
`9e7606b06b84ee624fe91f30c9998fb0655e31e0409d41bd2d5e99bc417d0e4a`.
The actual cloud desktop fixture run passed:
- Start, two settled 90px Down steps: three frames, 420×500 output, Fit Width editor
- Large manual wheel movement: an unmatched-seam warning reached the editor;
  Dismiss hid it while preserving the three-frame subtitle
- Minimum 640×440 editor remained readable
- Finish then Escape, and Finish then Cancel: no late editor appeared
- Fresh reopen reset to one frame / 320px; app was closed after the run

Save PNG displayed `Could not open the save dialog.` This is the previously observed
Linux portal limitation also seen in existing tools; no PNG was saved through the
GUI and no save-dialog cancellation/non-overwrite result is claimed. The core
stitch→annotation→PNG render/decode tests are separate byte-level evidence.
The expanded list with more than two notes and native window-close during Finish
were not independently exercised live. Native-close generation cancellation was
reviewed in code; lifecycle core regressions passed. No actual screen/window/region
capture, macOS runtime, IME, memory profiling or FPS result is claimed.
