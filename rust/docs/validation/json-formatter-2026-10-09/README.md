# JSON formatter source-preserving integration

Base: `0c80343a6d540aba3123825790f194d5817664fa`, tree `b47174d97ce4bb2cfd1ebf7b3b11f083a196d0ca`, parent `bc77dac089457c96696e7c9a98b0dbe085a23395`.

This successor routes only `developer::execute("json", ...)` through a bounded formatter-local parser. It retains raw number spellings, recursively compares object keys using NFC while emitting original decoded key spelling, and rejects canonical-equivalent duplicate keys. Arrays retain order. Pretty uses two-space indentation, compact empty containers, and no trailing newline. Successful Validate stays exactly `Valid JSON.`.

No Cargo manifest/lock changes, new dependencies, App session ownership changes, editor changes, provider requests, fixture features, or global JSON pointer semantics. Two expected strings in an existing App handoff regression change from insertion order to sorted order; its unsorted input and lifecycle assertions stay intact. Existing `jsonPointer` still independently resolves composed/decomposed names.

## Bounds

Combined input and untrimmed option: 500,000 UTF-8 bytes. Output: 4,000,000 bytes, checked before every append; string escaping streams through the same capped writer. Values: 20,000 including root. Depth: root 0, reject depth 64. Raw number slices borrow the already bounded input. Strings decode through serde_json; normalized comparison keys are separate from emitted spelling. Error diagnostics and Unicode-runtime equivalence are not broadly claimed.

## Native oracle scope

Pinned Swift `e43b1c4595c42383e087fe70f38c28d07c31dda0` DeveloperJSON source was compared through a Foundation-only CLI on macOS 14.8 arm64, Swift 6.0.2, Foundation 6.9/build 2602.0.902 (loaded 2602.0). All 51 Pretty/Minify/Validate acceptance outcomes across 17 original synthetic vectors agree; all 16 accepted Pretty/Minify outputs are byte-identical. The 8 accepted inputs include nested ordering, raw numeric lexemes, normalization ordering, scalar-vs-UTF16 ordering, escaped/non-ASCII strings, scalar root and empty collections. The 9 rejected inputs include duplicate aliases, canonical duplicate aliases, unpaired surrogates and malformed syntax.

This receipt does not prove deployed-runtime parity, every Unicode version/edge, error message equivalence, Swift Workbench validation counts, or a global pointer policy. Fixture metadata retains its original source-analysis provenance; the separate native-comparison receipt records the later observed evidence.

Sources: [Swift parser](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/DeveloperTools/DeveloperJSON.swift), [native receipt](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6077551503).

## Validation

See `checks.json`, `command-timings.json`, `mutant-controls.json`, and logs. The initial App regression failure is preserved: its expected unsorted output was updated in two assertions, then the checks were rerun. Real source mutations independently reintroduced serde numeric spelling conversion and byte-exact duplicate keys; the corresponding tests failed as expected (exit 101), and the exact original source hash was restored before final checks and ordinary build. These are distinct from the prototype's characterization contrasts against the previous implementation.

The ordinary binary is package-clean, default-feature, non-test, and has no fixture features. Focused actual cloud Linux GUI passed; see gui-acceptance.json. Sorted outputs, exact numeric spelling, Validate, Copy, Use as Input, rejection/recovery, palette transfer, and independent windows were observed; process exited normally with code 0. Screenshots remain local only. The original build seal’s pending GUI field records its earlier creation time. No exact-commit CI or publication result is implied by local validation.

## Timing

Command receipts use monotonic wall measurements with UTC endpoints and log hashes. Inference time is unavailable. Task windows include source work, review, communication, and waits; do not sum them with enclosed commands or concurrent native work.


## LOC ledger

Against `0c80343`, nonblank physical Rust lines (including comments) change by
+296 production, +232 test/support, and 0 benchmark. Cumulative Box counts are
59,595 production, 43,168 test/support, and 105 benchmark. Shared editor code is
counted once under Box. The complete baseline inventory is verified against Git;
exact positive cfg(test) spans and child test files are support. Five deliberately
corrupted ledgers (count, classification, omitted path, unchanged hash and duplicate
shared path) were rejected. See [LOC verification](loc/verification.json) and
[ledger](loc/ledger.json). Counts are source accounting, not feature-parity claims.
