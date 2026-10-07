# Recording workspace formatting correction

[Linux CI on f61d0e4](https://github.com/BelloWare/BelloBox/actions/runs/37691415417) failed `cargo fmt --all -- --check` before any test step. Rust 1.99.0 workspace formatting corrected five source files: import/module ordering, tuple-field spacing and assertion wrapping only. No behavior or production gate changed.

`cargo fmt --all`, `cargo fmt --all -- --check`, `git diff --check` and an independent 163-file source/classification verifier passed locally. New exact-commit Linux/macOS CI is still required; this record does not claim tests ran on the correction.

[source-and-loc.json](source-and-loc.json) binds all five before/after hashes and classifications, with 158 other Rust files unchanged. Nonblank physical Rust LOC: **52,593 production / 31,180 test-support / 105 benchmark**, total 83,878. Only three support lines were removed by wrapping; shared workbench code remains counted once in BelloBox.

The previous recording GUI binary and original evidence archives remain immutable evidence for their original source bytes. They were not rebuilt for this formatting correction. Actual native capture, TCC, general macOS termination handling and release acceptance remain separate gates.
