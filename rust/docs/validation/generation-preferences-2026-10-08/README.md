# Generation preferences validation

Candidate integrated on exact Box Rust `3bbef6854b5b1323c8573409f7f7b42ea71022d6`.
All 18 source/document postimages match `source-manifest.json`; independent source
review passed manifest SHA256 `54fd3706d632b760dc99cc8168e73b74d17df08826a0538065c179bd7589f673`.

Final gates on the integrated source, after cleaning both affected packages:

- `cargo test --locked -p bellobox-core --lib`: 431 passed, zero failures/ignored.
- `cargo test --locked -p bellobox-app --features recording-fixtures`: 423 passed,
  zero failures, two child-only ignored markers. Both readiness and ambient-proxy
  child cases were executed by their passing isolated parent tests.
- Strict core Clippy with tests; App default Clippy; App recording-fixtures Clippy
  with tests: all finished successfully, with `-D warnings`.
- `cargo fmt --all -- --check`: passed.

Focused Settings coverage passed 22 tests: explicit mutation, no-network browsing,
model switching, reopen, Reset, failed-save cache/file preservation, thinking and
output control behavior, and real numeric-loopback Load → select → Test → Ask AI
request captures carrying the saved temperature/reasoning fields. Effective-route
overrides and immutable snapshots have additional transport-level tests. Core
cross-caller tests compare every generation wire field across text, Clock Copilot
and the confirmed-image request builder for all three API formats. Malformed
optional stored fields default independently without damaging valid siblings or
routing; structural/routing corruption still fails closed.

Earlier attempts are retained: `app-settings.log` used the wrong feature spelling
and did not compile; `app-settings-r1.log`/`r2.log` show a test-module glob import
causing GPUI attribute recursion. Explicit test imports fixed it; r3 passed all 22.
These were command/test-harness corrections, not hidden successful test runs.

This is synthetic Linux GPUI and local transport evidence. No new interactive GUI
capture, native macOS/TCC/AX/Keychain/signing/performance acceptance, real provider
request, real credential, or production image/capture enablement is claimed.
