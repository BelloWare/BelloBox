# Palette QR r5 portable LOC audit

See LOC.md for exact source identities and current totals. ledger.json is the
full accounting; source-manifest.json binds all eight postimages and
sealed-source-manifest.json preserves the owner's exact local source freeze.
The remote source-only tree and local source tree are separately Git-bound,
with every one of the eight source/document postimages checked for equality.
Live verification confirms
all final product and excluded-evidence Rust bytes still match.

## Per-file delta (production / support)

- rust/crates/bellobox-app/src/desktop.rs: +0 / +0
- rust/crates/bellobox-app/src/desktop/qr_jobs.rs: -10 / -11
- rust/crates/bellobox-app/src/launcher_qr_ui.rs: +746 / +12
- rust/crates/bellobox-app/src/launcher_qr_ui/tests.rs: +0 / +513
- rust/crates/bellobox-app/src/launcher_ui.rs: +115 / +445
- rust/crates/bellobox-app/src/main.rs: +1 / +0
- rust/crates/bellobox-core/src/qr.rs: +63 / +101

## Explicit support ranges

- rust/crates/bellobox-app/src/desktop.rs: before [[1995, 1996]]; after [[1995, 1996]]
- rust/crates/bellobox-app/src/desktop/qr_jobs.rs: before [[46, 186]]; after [[36, 165]]
- rust/crates/bellobox-app/src/launcher_qr_ui.rs: before []; after [[108, 113], [162, 165], [761, 762]]
- rust/crates/bellobox-app/src/launcher_qr_ui/tests.rs: before []; after [[1, 516]]
- rust/crates/bellobox-app/src/launcher_ui.rs: before [[892, 907], [909, 910]]; after [[1007, 1022], [1024, 1025], [1027, 1471]]
- rust/crates/bellobox-app/src/main.rs: before []; after []
- rust/crates/bellobox-core/src/qr.rs: before [[60, 87]]; after [[129, 264]]

Ranges are inclusive, one-based physical lines. Positive cfg(test) spans include
whole definitions and attributes. The synthetic clipboard local override is
support; adjacent explanatory comments remain production. Ordinary unsupported
clipboard guards, keyboard Save lifecycle helpers, platform/backend branches
and core QR helpers remain production. The new external QR tests are wholly
support; platform-specific shortcuts inside them retain that classification.
Nonblank comments count. Unchanged baseline categories are inherited from the
published PR6 ledger, matching every baseline product source hash. Shared code
is counted once; deleted/replaced job code is subtracted through exact deltas.

Product inventory:194 baseline files,196 candidate files,189 unchanged paths.
Two unchanged standalone docs probes remain19+96=115 nonblank lines, outside
product categories. Product/probe union equals every tracked .rs path:198 files.
No standalone evidence source is silently dropped. Snapshot files end in.txt
and are not active Rust. Shared workbench is counted once in BelloBox and never
duplicated through BelloAgent's Git dependency.

## Reproduce

    python verify.py
    python verify.py --repo /path/to/checkout
    python negative-controls.py

Offline checks use Python standard library only; live inventory checking also
uses Git. Original commit/tree/blob objects bind baseline, remote candidate and
local source. The sealed manifest SHA and its eight postimage hashes are checked.
Verification covers source hashes, reviewed spans, all category/physical sums,
unchanged probes and unexpected live Rust paths. Six controls reject source
drift, altered ranges, omitted product inventory, changed baseline tree,
omitted probe and changed candidate tree. See verification.json and
portable-verification.json. The earlier audits are preserved in sibling r1/r2
folders. No Cargo/build/source/remote action was performed by this auditor.
Native/GUI and final execution acceptance remain separate and unclaimed.

The r4 whole-window physical-key tracker and routing changes are production.
Three new GPUI tests stay inside the complete test module. Static review receipt
is preserved and hash-bound; no native/GUI acceptance is inferred from LOC.

The r5 change is exclusively inside the final cfg(test) QR lifecycle module.
The complete prefix through line1026 is unchanged from r4, and the support-only
change adds28 nonblank lines. Source snapshots and this comparison are verified.
No native retry outcome or binary equivalence is asserted by the LOC verifier.
