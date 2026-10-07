# GIF publication/cancellation source repair

Validated 2026-10-07 UTC with `rustc 1.99.0 (b940084d7 2026-09-28)`.
This is a later core-only source repair, separate from the unchanged immutable
recording GUI candidate and its 174-input `source-inputs.json`. It is not evidence
that the GUI candidate contains this repair. App rebuild and affected GUI checks
belong to the subsequent shutdown checkpoint. No native execution is claimed here.

## Change and exact inputs

`ExportControl` now uses an atomic single-use state instead of holding a mutex
across final filesystem checks and publication. Cancellation accepted before the
publication claim prevents publication. Cancellation after that claim returns
`PublicationClaimed`, which does not mean a file was saved. The worker may still
fail. Error or unwind after claiming records `Failed`; successful publication
records `Published`. None of these logical outcomes establishes physical drain.

The target's filesystem checks, no-overwrite hard link, explicit-replace rename,
source protection and staged-file ownership/cleanup logic are unchanged.

SHA-256 of the tested inputs before the later MSRV spelling correction:

- `rust/crates/bellobox-core/src/recording/gif/gif_file.rs`:
  `bae0e3fb27dc1fa1f384ef02e6060b71c51151189134ee55e4e94dca9c9eb26b`
- `rust/crates/bellobox-core/src/recording/gif/gif_tests.rs`:
  `6cd9197ae29508b66bf97e4784ef4cd5420306ffa9ad294e01ae8adf622536da`

The current MSRV-compatible `gif_file.rs` SHA-256 is
`68af625034babb43ff6777de804ed7b1efb24c5b1ffd843bdf1ec92d28010971`.
It uses `fetch_update` with a statement-scoped deprecation allowance and rationale;
the test file is unchanged. The focused exact-source rerun passed all 35 GIF tests
and strict core test Clippy at 18:38 UTC. The isolated three-test and mutant checks
below retain their earlier semantically equivalent `try_update` spelling; those
isolated checks were not rebuilt for this source-only API compatibility change.

## Positive checks

All compilation was sequential with `CARGO_BUILD_JOBS=1`, the cached workspace
dependencies, and the shared BelloBox target. No dependency caches were removed.

- `cargo test --offline --locked -p bellobox-core --lib recording::gif:: -- --test-threads=1`:
  **35 passed, 0 failed, 347 filtered out** on the exact final inputs.
- `cargo clippy --offline --locked -p bellobox-core --tests -- -D warnings`:
  **passed** on the exact final inputs.
- `rustfmt` on both changed files and `git diff --check`: **passed**.
- Isolated `rustc --test -D warnings` harness: **3 passed**. It copies the exact
  production control prefix and three unmodified focused tests, with a minimal
  error-enum stub. It excludes the filesystem target and does not replace the
  complete core GIF suite above.

The focused tests cover cancellation/status while publication is held, 128
competing claim/cancel races, cancellation before/after start, truthful claimed
outcomes, terminal error/panic handling, refusal of a raced destination, preserved
source/destination bytes and removal of the owned stage. Existing end-to-end
encoding, readback, source-alias and replacement tests also passed in the 35-test
suite.

The first 35-test run passed with a Rust 1.99 deprecation warning for
`AtomicU8::fetch_update`. The sole follow-up source edit used its renamed
`try_update`; the exact suite and strict Clippy then passed without that warning.
A subsequent compatibility audit found that the workspace declares Rust 1.88,
while `try_update` was stabilized in 1.95. The call was returned to the equivalent
`fetch_update` (stable since 1.45) with a narrowly scoped allowance. No MSRV or
broader lint policy changed. See the [official Rust API documentation](https://doc.rust-lang.org/std/sync/atomic/struct.Atomic.html).

## Detecting negative control

An isolated mutant adds a wait in `cancel()` while the state is
`PublicationClaimed`, reproducing the inherited publication-waiting defect. The
same held-publication test failed as intended with exit **101** and
`cancellation waited for publication I/O: Timeout`. Its publisher was released
and both threads joined before the assertion, so this negative check did not
strand a worker. This is a synthetic mutation of the new control, not execution
of an old application binary.

The exact and mutant harness sources, patch, original files and logs are retained
in the local validation bundle `/tmp/recording-gif-control-review/`.

Current exact-source log SHA-256:

- `core-gif-tests-msrv.log`: `e3d888a6b6fa9ffe619a67ae261dc802bbcc2cc4a1d26aec6389629a1de4edc2`
- `core-clippy-msrv.log`: `0843355319744c6edf2848d08013f30d254a1772cf4aafcd4bf7d42ae00a3641`

Retained earlier-run log SHA-256:

- `core-gif-tests-final.log`: `161fb4f560d8f6e0180d54da8e96f8c823a42920bf2ee728134c79e5878870fd`
- `core-clippy-final.log`: `7dad877dd11eba31ee471c3f6b7a99ee79713052e4cde58dc3466cab417a6063`
- `control-exact-tests.log`: `6c5c11e1ec5cae8ecea80cf853d65ef1fea9beabceed5fa58ce056c8b8830aaf`
- `control-blocking-mutant-tests.log`: `ecee511430534e5874087669477fc4e79f77d4b7ff3db2aa4e746ff9ad3bed2b`

App-controlled process shutdown and physical-retirement coordination remain a
separate repair; its current acceptance target is Linux last-window close.
Native platform termination, including ordinary macOS menu/Cmd-Q unless it is
separately intercepted, can bypass that gate. It remains an unfinished native
shutdown/recovery boundary, not a guarantee limited only by forced termination.
