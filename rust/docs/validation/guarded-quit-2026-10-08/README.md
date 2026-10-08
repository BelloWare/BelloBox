# Guarded Quit: recovered v6 source and scoped evidence

Checkpoint date: 2026-10-08 UTC. This record covers the recovered candidate on
base `a2389dac0cc425b1b179ab2b3e6c3483d4e201fa`. It is not a claim of complete
BelloBox parity, native GUI acceptance, release readiness, or current-commit CI.

## Source identity and recovery

The production repair and original tests were recovered from the immutable
[r5 review bundle](https://github.com/BelloWare/BelloBox/tree/ff47ef86c0ee67759f71a3fd49f44f76c7e021c3/rust/docs/review/guarded-quit-r5).
The final candidate adds only the peer's
[eight-line test correction](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6060265457).
No feature, dependency, platform gate, or shared-workbench edit is added by that
correction. `source-recovery.json` binds the 23 recovered source paths by SHA256
and Git blob identity.

Fresh recovery checks:

- The clean clone HEAD exactly matched the stated base.
- r5 manifest SHA256:
  `71af00ec111af7728548361a39394eefcce9b84994e18bc8bc4892a9ce39a3f3`.
- r5 full patch SHA256:
  `cf22badcd0421a9a50d5c5fb66a1ec8f0cb9fc7c310df8b335d7e53ccda1f5fd`.
- All 23 exact preimages and r5 afterimages matched; patch apply checks passed.
- The test-only patch SHA256 matched
  `da25c0d391133dbd75e2673c756282f458ca0c119b4376ec3a4302eee6b0f4fb`.
- Final `screenshot_ui/area_capture.rs` SHA256 is
  `dc5b0a5565f0ccccda6ff81f5330de4cecba51be275cd24111f6a5c312de9c2a`.
- All 189 frozen r5 inputs were checked. The sole expected difference is the
  corrected test file; the other 188 inputs remain exact.
- The complete reconstructed v6 patch, including all five new source files, has
  SHA256 `a092bfddd3fcee17dda50bfac559e9094db32bb5aa4a11cdd13f86822f1fa2a8`.
  Reverse-apply checks pass for both the test-only and complete v6 patches.

## Behavior and limits

App-controlled Quit evaluates the existing foreground owners and active work
before latching shutdown. Refusal preserves the current windows and content and
requires a later deliberate retry. Physical work retains ownership through
completion/disposal rather than being released by a timeout or logical cancel.
Ordinary refusal prompts own their dismissing mouse/key gestures and restore
GPUI's default prompt builder for unrelated prompts.

An active editor/selector pointer gesture refuses Quit using nonmodal owned
feedback, preserving the original gesture's natural release or Escape. The
standalone footer keeps ordinary status in layout and renders the notice as a
noninteractive absolute layer, so multiline/wrapped status does not change Fit
canvas geometry while a pointer is held. Earlier modal-release annotation
mutation and footer-reflow detecting failures remain historical rejected
candidates; they are not treated as accepted final behavior.

The v6 test-only Home companion represents the normal app context from which a
selector launches. On the peer's macOS GPUI test platform, the genuine AppKit
focus check rejected the subsequent natural-release handoff; selector closure
then triggered ordinary last-window shutdown in the one-window test. The
companion keeps Home alive while retaining the refusal assertions, expected
Area/Window rectangles, and production AppKit gate. It does not enable capture
or suppress the final no-shutdown assertion.

## Evidence separation

### Fresh checks in the recovery environment

- Exact source/patch/frozen-input checks above: passed.
- `git diff --check`: passed.
- `cargo fmt --all --manifest-path rust/Cargo.toml -- --check`: passed using
  Rust 1.99.0 / rustfmt 1.10.0-stable (`b940084d7e`, 2026-09-28).
- Performance harness self-tests: 4 passed (`performance-harness.txt`).
- Linux UI harness self-tests: 14 passed (`ui-harness.txt`). These test the
  harness itself; they do not launch or validate the BelloBox application.
- `cargo test --locked -p bellobox-core --lib`: 390 passed, 0 failed/ignored/filtered, including all eight portable B3 output-settings tests (`core-tests.txt`).
- Full original `bellobox-app` compilation with `recording-fixtures`: passed; no copied, truncated, or filtered source harness (`app-test-build.txt`).
- Complete App test binary executed serially: 359 passed, 0 failed/ignored/filtered in 191.25 seconds (`app-tests-full.txt`). This includes the guarded-Quit and held-pointer test-platform regressions.
- Exact-source LOC verifier: passed for all 23 recovered source paths.

- Fresh `cargo build --locked -p bellobox-app --features recording-fixtures`: passed in 37.68 seconds (`production-build.txt`). All 189 source/build inputs match before/after (`production-build-identity.json`). Binary SHA256: `23147cd33abf02794c0116b5fcf1f379a3703865f74622b7112aca7c0b3a8c95`.

- Strict Clippy: passed for default App all-targets and recording-fixture App + platform all-targets, both with `--no-deps -- -D warnings` (`clippy-default-app.txt`, `clippy-recording.txt`).

The dependency `proc-macro-error2` 2.0.1 emits a Cargo future-incompatibility notice; current builds and strict project Clippy pass. No dependency or lint gate was changed.
No native test execution was performed in this Linux recovery environment.
The limited subsequent cloud GUI observations are recorded below.

### Historical and peer-reported results

The immutable r5 bundle records earlier Linux checks: 121 executed focused App
tests (124 selected functions, three native cfg exclusions), eight portable B3
policy tests, formatting, strict checks and a fresh fixture production build.
Those results are historical source-bound reports, not fresh executions here.
The earlier local screenshots and raw GUI evidence package are unavailable after
the workspace reset. Historical GUI observations have not been freshly rerun;
no reconstructed historical screenshot or broad guarded-Quit visual acceptance
is claimed. New limited keyboard observations are separately recorded below.

The peer [first reported a reproducible r5 native synthetic test failure](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6060004739),
then [reported 135/135 native synthetic tests passing after the test-only correction](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6060265457)
on macOS 14.8/arm64, Xcode 16.1 and Rust 1.91.1. The latter report includes the
instrumented cause, both Area/Window iterations, final fixture test build,
formatting and immutable binary/log hashes. These are peer-reported results;
the raw native logs/binary were not recovered or re-executed here. Test-platform
events and native compile/link do not establish actual native GUI behavior.

Actual held-device overlap was not established by the earlier cloud CUA run:
its timed pointer holds serialized, so overlapping Quit could not be observed.
Native Dock/OS termination veto, forced quit, real capture, TCC/AX permissions,
Retina/Spaces/focus behavior and full workflow parity remain separate gates.
No user's device, credentials, real provider, signing, notarization or release
is involved in this recovery.

## Independent source-line recount

`loc-delta.json` contains complete before/after hashes and reviewed support ranges
for all 23 changed Rust files. Run `python3 verify-loc.py` from this directory (or
invoke that script by path from the repository root) to verify the exact delta.
Nonblank physical lines include comments. Positive `cfg(test)` and
`cfg(all(test, ...))` spans and test-only modules count as support;
`cfg(any(test, debug_assertions))` remains production. Unchanged files are not
recategorized; no cumulative project total is freshly certified by this audit.

The independently verified delta from `a2389da` is **+704 production,
+2414 test/support, 0 benchmark**, totaling +3118 nonblank Rust lines.
The earlier +801/+2317 categorization misclassified 97 test-only lines:

- `shutdown/quit.rs`: two-line test re-export, two-line test field, and 55-line
  inline test module (59 total).
- `shutdown/quit/prompt.rs`: test module declaration and test-only pending/dismiss
  helpers (38 nonblank lines).

This corrects categories, not the total size or runtime behavior. The count is
not a completion percentage or an estimate of delivered feature parity.

### Fresh, limited cloud desktop interaction

The immutable rebuilt fixture binary launched with isolated official lavapipe.
Actual keyboard navigation opened the local synthetic PNG editor; Ctrl+Alt+4
visibly selected Rectangle; Ctrl+Q from the clean editor closed both app windows
and the process exited0. These are actual interactive observations, limited to
those steps. See `gui/observations.json` and the two original screenshots.

Pointer clicks/drags did not reliably activate controls or create an annotation
in this attempt. Cause is not established. Dirty-decision, held-pointer Quit,
prompt shielding and active-work drain manual acceptance therefore remain open.
The fresh359 App tests exercise those paths on GPUI's test platform; they do not
turn this limited desktop attempt into a complete interactive matrix. The older
raw GUI evidence is still unavailable. A separately authorized synthetic peer
interaction check has been requested.

Applying the independently verified delta to the published baseline's audited
52,686 production /32,026 support /105 benchmark counts gives **53,390
production /34,440 support /105 benchmark**. The unchanged baseline categories
are inherited; this audit independently rechecks the changed source. Shared
workbench source remains counted only in BelloBox, not again in BelloAgent.
