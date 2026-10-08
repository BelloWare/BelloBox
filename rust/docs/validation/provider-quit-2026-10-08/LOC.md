# Provider Quit r2 Rust LOC

Baseline: `dec241107f359146d0e63fb051dfafe94e1425e1`.

Delta: **+21 production / +310 test-support / 0 benchmark**.
Cumulative: **53,930 production / 35,692 test-support / 105 benchmark**.
Total: 89,727 nonblank physical Rust lines across 178 files.

Count includes comments. Whole external test modules and positive cfg(test) spans are support; production feature paths stay production. Unchanged baseline classifications are inherited. All 177 baseline Rust files reconcile to 89,396 lines; 174 files remain unchanged, three modified and one test module added. The four source postimages and original 20-path repair manifest were verified.

`loc-ledger.json` binds before/after hashes, explicit support ranges, source inventory and arithmetic to the exact baseline commit/tree. `loc-verification.json` records the independent verification. The full local audit bundle also passed four tamper controls (source bytes, support ranges, inventory, Git tree), but that bundle and its standalone verifier are not published here.

Shared workbench is counted once in BelloBox, excluded from BelloAgent's Git dependency. Benchmarks are separate. Documentation, patch snapshots, logs, generated output and dependencies are excluded. This is accounting, not workflow completion or an ETA.
