# Palette Copilot presentation validation — 2026-10-08

Base and final exact two Rust source hashes are in `source-manifest.json`.
The earlier presentation candidate is separately recorded in `source-manifest-r1.json`.
This repair changes presentation and derived suggestion status only.

## Results

- Focused presentation: 6 passed, no failures; 1 isolated-child marker ignored by
  the outer harness but explicitly run and checked by its parent test.
- Full App, recording-fixtures: 419 passed, no failures, 2 child-only ignored
  markers. Both isolated children are explicitly executed by parent tests.
- Strict App Clippy: default and recording-fixtures passed with `-D warnings`.
- Workspace format check and ordinary App build passed.
- Synthetic GPUI at 320/480/680-point widths measured the actual rendered input
  (30 points), editor (20 points), transcript (98 points), 6-point gap, and
  261/365-point preview bounds. Clicking the empty hint focuses the actual editor.
- Missing-model, configured-model, unsupported-Codex and invalid credential-bearing
  endpoint cases exercise the existing readiness cache and save invalidation in a
  separate process with temporary settings. A loopback listener verifies zero
  requests. Readiness never bypasses actual Send's independent validation.

`prior-behavior-mutant.patch` restores the old 18-point input reservation and
unconditional location-parts deferral in the new testable renderer. Four relevant
regressions fail (`red-prior-behavior.log`); this is a targeted mutation test, not
an assertion that the complete old branch was rebuilt. After restoring green
source, `cargo clean -p bellobox-app` removed 26 files/394.5 MiB before the complete
App suite and final gates. The ordinary binary's hash is recorded in the manifest.

Early implementation failures are preserved: invalid singular feature spelling,
a glob-import test macro collision, and unavailable direct chrono imports in
App tests. Those invocations established no passing coverage and were corrected
without changing dependencies or weakening assertions. `focused-r3.log` and
`full-app.log` contain the final green results.

## Commands

Run from `rust/`, using the existing official Rust1.99 and verified Linux GPUI
prerequisites:

    cargo test --locked -p bellobox-app --features recording-fixtures launcher_clock_ui::copilot::presentation_tests -- --test-threads=1
    cargo clean -p bellobox-app
    cargo test --locked -p bellobox-app --features recording-fixtures -- --test-threads=2
    cargo fmt --all -- --check
    cargo clippy --locked -p bellobox-app --tests -- -D warnings
    cargo clippy --locked -p bellobox-app --features recording-fixtures --tests -- -D warnings
    cargo build --locked -p bellobox-app

The tests are synthetic; no actual interactive GUI, native macOS/TCC/AX/Keychain,
signing, performance or release acceptance is claimed by this package. Existing
capture/tool/vault/native production gates remain unchanged.

## Interactive clipping follow-up

Actual interactive review of r1 found typed text clipped despite a visible hint.
The original frame hash is retained in the final manifest. The new real-editor
measurement regression failed on r1 with `typed editor content 28 exceeds field
20px` (`typed-content-red.log`). Local host appearance now removes inherited
4-point top/bottom padding, retaining the same outer input/preview bounds and
shared editor defaults. `focused-final.log` verifies the complete typed content
fits at all three widths; `build-final.log` records the rebuilt ordinary binary.
Final full/strict results are recorded separately with `-final.log` suffixes;
earlier logs remain as historical candidate evidence.

The rebuilt ordinary binary also passed an actual cloud-Linux typed-text recheck:
"plan tokyo gjpqy" and the caret remained fully visible at the same 680×447
empty palette size. Placeholder-click focus worked and typing sent no request.
The successful and original clipped frame hashes are in the manifest; the GUI
validation package retains their original screenshots. This scoped interactive
check is separate from the synthetic tests and does not establish native macOS
acceptance or complete Swift presentation parity.
