# Popup QR v3: exact Rust LOC accounting

Baseline: `15197a71d81c620bf60da18f0d066505e4c0dc99`, tree
`1ca9979fab6bff4f20d3abee773b411104b029ad`.
Candidate: `4589e0280844df4a070533c7babb9d01cf430905`, tree
`294c99a7847757d2c41217834b8dc87eab9e4765`, directly parented by the baseline.

Seven changed/new Rust paths add **124 production / 462 test-support /
0 benchmark** nonblank physical lines. Cumulative product totals:
**57,905 production / 41,531 support / 105 benchmark**,
**99,541 lines across 199 product Rust files**.

The existing two standalone docs probes remain separately **115 evidence-only
lines**. All tracked .rs reconcile to **99,656 lines across 201 files**.
Four existing product files change, three are added, and192 remain unchanged.
All eight final source/document postimages match the frozen v3 snapshot.

Positive cfg(test) items/modules are support, including complete bodies and
attributes. External tests are wholly support. Adjacent non-test comments and
ordinary platform/backend branches remain production. Comments count. Shared
QR clipboard and physical-key helpers are counted once at their new paths;
removed old implementation/test override lines are subtracted. Shared workbench
counts once in BelloBox, not again through BelloAgent's Git dependency.

Ledger SHA256:
`9ffc53591b02fee917ae4567ba91743ee4a309d553d9f2556ce5634b28b92c11`.
Exact commit/tree/blob identities, current published baseline categories,
source hashes, explicit ranges and complete inventories verify offline and live.
Six tamper controls pass. No edits, Cargo, builds, desktop or remote writes were
performed. Execution/native/GUI acceptance remains separate; LOC implies neither
parity nor ETA.
