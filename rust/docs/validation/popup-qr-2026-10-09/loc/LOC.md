# Popup QR final source plus fixture repair: exact Rust LOC

Published baseline: `15197a71d81c620bf60da18f0d066505e4c0dc99`.
Final source: `80e6d41a68fc0e2bf186272fb788a301f4f9e744`, tree
`3a4d1f63a5857a0cf60d37001991f4e59fa61df8`, parent
`4589e0280844df4a070533c7babb9d01cf430905`.

Original4589 popup delta: +124 production / +462 support / 0 benchmark.
The exact two-test-file successor adds **0 production / 76 support / 0 benchmark**,
38 nonblank support lines per file. Every other Git path, including the original
eight source/document postimages, remains unchanged.

Combined delta: **124 production / 538 test-support / 0 benchmark**.
Cumulative product totals: **57,905 production / 41,607 support / 105 benchmark**,
**99,617 lines across199 product Rust files**. Existing standalone docs probes
remain **115 separate evidence lines across2 files**. All tracked .rs reconcile
to **99,732 nonblank lines across201 files**.

The two successor files are entire cfg(test) external modules:
launcher_clock_ui/copilot/tests.rs (380→419 physical lines) and
launcher_ui/copilot_lifecycle_tests.rs (382→421). Nonblank counts rise376→414
and375→413. Public fixture helpers inside them remain support. Shared QR helper
and workbench code is counted once; Agent's Git dependency does not duplicate it.
Comments count; snapshots/docs/build output do not enter product Rust totals.

Ledger SHA256:
`88c07cd566003f7b039c7831981d4889655afca9bc841be13fc90d2d4cf594b2`.
Exact final/precursor/base Git bindings, source hashes/ranges and full inventories
verify offline/live. Six tamper controls pass. Prior4589 attribution is preserved
in precursor-ledger.json and the separate v3 artifact directory. Production/binary
equivalence receipt is preserved separately; this LOC audit does not rerun builds
or claim native/GUI acceptance. No source/Cargo/desktop/remote changes performed.
