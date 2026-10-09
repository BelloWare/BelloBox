Queued request only; root will post after the popup fixture repair is finished.

Please validate this isolated shared editor dependency candidate on the authorized native CLI/TestPlatform host:
https://github.com/BelloWare/BelloBox/commit/2e72ec51bd010a96624ca7326f9a9afcb1057686
Commit: 2e72ec51bd010a96624ca7326f9a9afcb1057686
Tree: c3af9d1030f8d65df98cd9ef1b41d7759ca5ea5f
Parent: 15197a71d81c620bf60da18f0d066505e4c0dc99

This candidate contains exactly five shared-workbench paths, with no popup source, Cargo.lock, Agent pin, screenshots, credentials or native permission changes. It adds exact-text-bound transient decoration, nonfocusing range reveal, one-shot measured scene-paint geometry and explicit select_all. It is dependency infrastructure until its Agent consumer is wired. Root has independently fetched and byte-verified all five postimages. Keep rust unchanged; root remains the only integrator.

Please use a fresh exact checkout, report HEAD/tree and verify these SHA-256 postimages before testing:
85551129fcb904d1cd6b36fa5e08a117747037d45d9941bdd5df3872d8609470  rust/crates/bello-workbench-ui/Cargo.toml
384588661e1e70c66730a7e0318b1a574116d789d6f153a6e8b5139d5621da11  rust/crates/bello-workbench-ui/src/editor_view.rs
b024ad91b362f8b98e23095b6f2d70ede8a889708d953529913a5d522d266736  rust/crates/bello-workbench-ui/src/editor_presentation_tests.rs
ee9ceeb559880cac8f23220d1c13a624fc2c4db88147d4424865ace434549fac  rust/crates/bello-workbench-ui/src/text_presentation.rs
23e57b6010a45aa4ca2be0960c396907069a566dd1fa7ea8f535a09c0d050a3e  rust/crates/bello-workbench-ui/src/lib.rs

From rust/, use the pinned Rust toolchain and existing approved prerequisites:
1. cargo fmt --all -- --check
2. cargo test --locked -p bello-workbench-ui --lib -- --test-threads=2
3. cargo clippy --locked -p bello-workbench-ui --all-targets -- -D warnings
4. cargo clean -p bello-workbench-ui
5. cargo build --locked -p bello-workbench-ui --lib

The focused Linux suite and independent rerun each passed 37 tests, zero ignored/failed. Preserve any native failures and compare warnings with the exact parent if needed; an existing proc-macro-error2 future-incompatibility notice is known. Explicit package cleaning before the final ordinary library build avoids inheriting test-support artifacts. This is a library build, not a whole-app GUI binary seal. Report commands, exit codes, counts, toolchain/host identity, source hashes before/after and hashed text logs. No screenshots or binary uploads.

In particular preserve exact original UTF-8/CRLF ranges, immutable edit history/selection/focus, IME/drag Busy, cached-child set/clear invalidation, same-length revision-reset replacement, clipped/wrapped geometry, cancellation and stale postpaint deferred receipts. GPUI TestPlatform scene paint is synthetic evidence only; do not claim actual AppKit/AX/IME/visibility acceptance. The consumer must still fence outer navigation/lifetime/geometry, bound no-receipt retries and validate a complete Find workflow.

Do not modify production source or relax assertions to obtain a pass. Return a scoped repair patch for a demonstrated native issue if necessary, with original failure evidence; root will review and create a new candidate. Do not move rust, modify main, change permissions or access real credentials/user devices.
