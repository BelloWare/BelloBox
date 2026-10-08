# Palette Clock Copilot validation — 2026-10-08 UTC

## Exact source

Integrated parent: `2a5f95b35d27f33b19e4530e696a06da496fc9b8`.
The candidate modifies 14 Rust paths: 13 present paths and deletion of the old
window-private worker. `source-r1-manifest.json` gives every exact source hash,
size and deletion. Its SHA-256 is
`298d7414e08c901f6c7840c6fd4524de237ab0a05f14dbcd268c391bf55b1dbf`.
The independent final source review checked the manifest and the guard-dedup
follow-on before validation was accepted. Every source byte was checked again
after the final tests and lints.

The candidate patch SHA-256 is
`32c5b6017439fe8601ffe7f5af843f5bbfdb738926a85ef4345ff43e4d481169`.
The patch is a local integration artifact, not included here as duplicate source.
The original c3 dedicated Clock GUI evidence is preserved unchanged in the
neighboring `clock-copilot-gui-2026-10-08` folder; it does not establish interactive
palette acceptance.

## Final gates

Before testing this clean integrated worktree, Cargo removed both `bellobox-core`
and `bellobox-app` package outputs. An explicit assertion found no surviving App
test executables. This avoids reusing a binary built from another worktree.

Commands from `rust/`, using the pinned official toolchain and isolated official
Linux prerequisites:

- `cargo clean -p bellobox-core -p bellobox-app`
- `cargo test --locked -p bellobox-core --lib`: **424 passed**.
- `cargo test --locked -p bellobox-app --features recording-fixtures -- --test-threads=2`:
  **413 passed, zero failed, one ignored child-only test**. The parent proxy
  isolation test explicitly runs that child in its separate environment.
- `cargo clippy --locked -p bellobox-core --tests -- -D warnings`: passed.
- `cargo clippy --locked -p bellobox-app --tests -- -D warnings`: passed.
- `cargo clippy --locked -p bellobox-app --features recording-fixtures --tests -- -D warnings`:
  passed.
- `cargo fmt --all -- --check`: passed.

The final App test executable SHA-256 is
`8c3c038ae730294e7597a2d8751287b9f78ca6e9efd1aef9de238b16107bf91b`.
`verification.json` ties that executable to the final source manifest and gate
counts. No executable is uploaded in this evidence package.

## Workflow and negative coverage

New coverage includes explicit palette Send with numeric-loopback HTTP, accepted
reply, time-only Apply, preserved unapplied location suggestion and draft,
subsequent dedicated location Apply, source preferences unchanged on handoff,
stale Apply/Copy/Complete rejection, and destination-monotonic restore IDs.

Held real loopback transport exercises discarded/retained preview entities,
selection replacement, non-clock selection, actual GPUI native-close callback,
platform activation/deactivation, failed target adoption preserving the source,
successful source launch with an older retained guard, and adoption into a
physically busy destination. Cancellation cannot release the single physical
lane, app Quit blocker or local close refusal. Repeated same-request transfers
retain only one observer; a fresh close after physical retirement succeeds.

Keyboard regressions cover secondary-editor composition, Escape and held Escape,
Tab/Shift-Tab, Return, Ctrl/Cmd+K and stable per-message Apply focus across live
revision changes. Exact 8,192-byte ASCII and multibyte draft transfers preserve
bytes; an 8,193-byte draft fails before source/target mutation or activation.

Earlier development attempts exposed compilation errors while a new test module
was still being written and a missing trait import, then strict lint correctly
reported unused fixture helpers before their tests were added. Those attempts
were not counted as successful validation. The final clean gate records above
supersede them; no assertion was removed or gate relaxed to obtain these passes.

## Scope limits

These are synthetic GPUI/action tests and loopback transport tests. No interactive
palette screenshot, native macOS/TCC/AX/Keychain/signing/performance acceptance,
real provider request, real credentials, capture enablement or release claim is
made. Remote CI belongs to the eventual published commit and must be checked on
that exact SHA separately.
