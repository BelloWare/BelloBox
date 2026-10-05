# QR responsiveness slice

## Scope and evidence

Baseline: `84cada0646913ec32830f303d636fc9136618c95` (2026-10-05).
The separate release-profile CPU harness measured 2,000-byte PNG generation at
13.157 ms CPU p50, 15.835 ms p95, and 19.029 ms p99 (1,500 warm samples).
These are encoding CPU measurements, not UI frames, input-to-paint latency,
achieved FPS, or macOS performance. The patch removes this known synchronous
work from UI preview/edit and Save callbacks; it does not claim a measured
end-to-end speedup or a lower encoding cost.

## Narrow changes

- Desktop and launcher PNG generation run on GPUI's background executor.
- A new edit/search/focus generation clears the previous QR immediately. Copy
  therefore does nothing until a current image is accepted.
- Cheap empty/blank and byte-limit rejection happens before dispatch. A queued
  cancelled worker does no encoding. An already-running synchronous encode may
  finish; its stale success or error cannot publish. Launcher cancellation after
  PNG also skips terminal generation.
- Launcher image and accessible terminal preview come from the same input
  snapshot and publish together, behind one generation/window guard.
- Existing weak-entity updates and window-owned `SessionJobs` prevent closed or
  reopened windows from receiving an old completion. Launcher Escape and tool
  handoff explicitly cancel before closing.
- Save captures text at click time, then generates that snapshot and performs
  the existing no-overwrite write on the background executor after destination
  selection. Editing does not silently retarget or cancel a user-confirmed
  disk write. Separate save generations suppress obsolete status updates after
  edits or newer saves; a preview completion cannot replace same-edit Save
  status. Cancelling the dialog performs no encoding/write.
- No QR algorithm, limits, dimensions, styling, dependencies, or shared editor
  implementation changed. Native image decoding/presentation remains GPUI-owned.

## Automated checks

Passed on Linux with Rust 1.99.0, the existing matching-source
`target-clock-ui-final` cache, and `native-link/lib` supplied through
`LIBRARY_PATH`. No cache, recovery checkout, or baseline evidence was deleted.

- `cargo test --offline --locked -p bellobox-app qr_jobs`: 6/6 passed.
  Covers reordered success/error, empty/blank/oversize invalidation, 200 rapid
  queued generations (199 skip; latest encodes), cancel/drop/reopen, save snapshot
  and status gates, and exact PNG/terminal equality with unchanged core output.
- `bash rust/scripts/check.sh`: passed (format, workspace tests, all-target
  clippy with warnings denied, workspace build, four perf harness tests).
  Rust totals: 395 passed, 4 ignored, 0 failed. Python perf harness: 4 passed.
- `git diff --check`: passed.

The deterministic tests exercise real worker payloads and `SessionJobs`, but
model the publication/copy/save state rather than driving GPUI. They are not
clipboard, save-dialog, or native close runtime proof. Exact output equality is
not an independent scanner decode. Existing core pixel/quiet-zone tests remain
unchanged and passed in the aggregate gate.

## Independent review and Linux UI QA

Independent source review found no blocking issue in the narrow change and
reran the six focused tests. Actual Linux software-rendered UI QA used frozen
binary SHA-256
`ef9bcd46c91b1f388945ab84882fcfd2a3cf38e9837fbaae99741f9cb40cff5a`.

Passed: initial/current QR, rapid edit and pending-to-current transition,
exactly 2,000-byte generation, 2,001-byte rejection, Clear while generation was
pending (old result stayed absent), current Copy action confirmation, launcher
latest-preview and clear/focus-change flows, and clean close/reopen. All fixture
windows were closed after QA.

Save reported “The system save dialog could not be opened.” A bounded comparison
with the preserved pre-patch binary (SHA-256
`82d2e2ca29c3aa0e60d4106b2cdd5861e9725218e1dcff6c1853d44e8f3106f2`),
same renderer/display and same synthetic URL produced the identical failure.
The baseline and candidate use the same prompt API/path/name. No system setup
was changed. Thus this runtime cannot establish Save cancellation, click-time
saved PNG bytes, or completion status under a real dialog. Pending-Copy races
and clipboard PNG extraction also remain unvalidated at runtime; the current
Copy confirmation alone does not prove clipboard image bytes.

Local QA evidence: `box-qr-ef9b-{oversize,2000,clear,copy,launcher-latest,
launcher-clear,reopen}.png` and `box-qr-baseline-82d-save-failure.png` in the
external QA evidence directory. These are appearance/interaction evidence,
not display/frame measurements.

## Remaining acceptance boundary

Native macOS IME, clipboard/image paste, save dialog, display/frame pacing and
accessibility still require Mac validation. No UI callback or input-to-paint
timing comparison was instrumented in this patch. The tests and Linux checks
justify the scoped offload/lifecycle change, not full QR UI parity or a claim
that the application is now “buttery smooth.”
