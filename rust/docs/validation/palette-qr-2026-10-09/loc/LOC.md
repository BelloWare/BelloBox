# Palette QR r5: exact source accounting

Published baseline: `25e78766d2463c186e7dbdf4d85029401290d062`.
Remote source candidate: `45d5e841e89d8062816dc9df24789cf603be22c9`, tree
`d5649f40822d55a6494b9f47bd8ca62cc7c94c1f`.
All eight final postimages match local source
`3cd1404b20afa0f47ffd392abe83a8cd88d9364e`, tree
`c37b8b64a7b00d7ece522561a2b30427439df5c2`. The trees differ in documentation;
this audit binds their exact source postimages without claiming identical trees.

Delta: **915 production / 1,060 test-support / 0 benchmark** nonblank Rust lines.
Cumulative: **57,781 production / 41,069 support / 105 benchmark**,
**98,955 lines across 196 product Rust files**. Relative to r4, this adds
0 production and 28 support lines. The two existing standalone docs probes
remain separately **115 evidence-only lines across two files**. All tracked
Rust reconciles to **99,070 lines across 198 files**.

Complete positive cfg(test) items/modules are support. The new synthetic
clipboard-only local binding is support at launcher_qr_ui.rs 108–113; its preceding
three explanatory comments remain production. The test accessor is162–165 and
test module declaration761–762. The whole external test file is support1–516.
launcher_ui.rs QR lifecycle support extends through 1471. Other existing ranges
remain explicit in ledger.json. Ordinary platform/backend and UI/core helpers
remain production. Comments count; shared implementation is counted once.

Current ledger SHA256:
`3ee3628d4cd78655a32477f060a07ad17ad7cad8d6f22892f529cfb14a66e650`.
Both source identities, the sealed manifest, published baseline, all source
hashes/ranges and complete physical inventories verify offline and live.
Six tamper controls pass. Earlier audits remain preserved as r1/r2/r3/r4.

No source edits, Cargo or remote writes were performed. This audit establishes
accounting only, not pending native/GUI acceptance or behavioral equivalence.
Shared workbench counts once in BelloBox; BelloAgent's Git dependency is excluded.
LOC implies neither parity nor ETA.

The r4 whole-window physical-key tracker and routing changes are production.
Three new GPUI tests stay inside the complete test module. Static review receipt
is preserved and hash-bound; no native/GUI acceptance is inferred from LOC.

The r5 change is exclusively inside the final cfg(test) QR lifecycle module.
The complete prefix through line1026 is unchanged from r4, and the support-only
change adds28 nonblank lines. Source snapshots and this comparison are verified.
No native retry outcome or binary equivalence is asserted by the LOC verifier.
