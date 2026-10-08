# Dedicated Clock Copilot validation — 2026-10-08 UTC

Final integrated base: `f30b81f0fd62e7bb29b8809a4f57471b46b01450`.
The feature candidate was developed on `dec241107f359146d0e63fb051dfafe94e1425e1`
and applied without conflicts onto the newer base, retaining the provider Quit
repair and provider GUI evidence already published there.

The exact 12 source/document postimages are listed in `source-manifest-r2.json`:
SHA-256 `27b87dcd0e288ffe68eda9116e8a545a954b15c1980ff8383f0d560ba7e541e3`.
All were checked after integration and after validation. The r2 source patch hash
is `2378955cf6db64932e105022a5f79a2ab44d684380d0be3d8cbc38360afd547c`.
`integration-verification.json` records that comparison and the final results.

## Final checks

Rust 1.99.0, Cargo 1.99.0, isolated Linux GPUI prerequisites. The two changed
packages were cleaned before rebuilding, preventing a prior worktree's test
binary from being mistaken for this candidate. Raw command logs are retained.

- `cargo test --locked -p bellobox-core --lib`: 421 passed, zero failed/ignored.
- `cargo test --locked -p bellobox-app --features recording-fixtures -- --test-threads=2`:
  397 passed, zero failed, one child-only proxy fixture marked ignored. Its parent
  separately executes the isolated subprocess test; this is not missing coverage.
- Strict core tests Clippy, ordinary App Clippy, recording-fixtures App tests
  Clippy, and workspace formatting check passed.
- Thirteen new synthetic GPUI tests passed, including real numeric-loopback HTTP
  and installed native-close callbacks. These are test-host execution, not manual
  GUI screenshots or macOS native acceptance.

The feature includes the dedicated conversation, explicit request admission,
Retry/Cancel/Clear/Hide, answer Copy, staged explicit Apply, preference-save error
reporting, and stale proposal protection. Pure protocol/session/apply regressions
cover request/parser bounds, provider shapes, grapheme limits and transactional
planner changes. See [feature scope and limits](../../clock-copilot.md).

## Important close-policy difference

Ordinary Clock Close while its HTTP request is physically active signals Cancel,
invalidates the answer and refuses closure, even when another window is open.
After retirement a fresh Close is required. The same local guard is installed for
native Close and preserves ordinary app Quit admission; it consults the existing
shared close gate only after physical retirement. App-controlled Quit is refused
through a worker-owned QuitBlocker. Forced removal with a retained entity is a
separate regression that proves stale-publication suppression, not a promise that
forced OS termination drains requests. No shared media shutdown policy changed.

## Preserved failure and correction

`initial-test-harness-failure.log` is the first full old-base candidate run:
379 passed, 11 failed, one ignored. The first failure was in the new Quit test:
it used a typed WorldClock window handle after successful Quit had correctly
replaced that root with Closing. Ten following tests failed because the shared
Clock test mutex was poisoned by that first assertion. The fixture was corrected
to retain the original Entity and invoke its late Send callback after Quit,
retaining the zero-network assertion. No production check was weakened. The
corrected focused twelve-test suite passed; an independent review then requested
that request failures and later explicit Apply feedback both remain visible.
That regression is the thirteenth test in the final integrated passing run.
Earlier development compilation errors were test imports/signatures; no recursion
limit increase or relaxed assertions was used to clear them.

## Acceptance limits

This uses synthetic fixtures and loopback providers only. No paid request,
credential persistence, user Mac access, TCC/AX/Keychain, native forced-termination,
performance, signing or release acceptance is claimed. Actual interactive GUI
validation, if later performed, has its own separately identified evidence.
Palette conversation preview/handoff and Codex app-server remain outside this
slice. The existing Settings opener is reused; AI Provider category navigation
remains an explicit user step rather than a new deep link.
