# PR6 sealed movie fixtures: reviewed Rust LOC

Exact candidate `59339af1fa422acea4067207c55344666c3fa4ab`, tree
`c6b021437434ce691c1a225469abeac3b92c80df`, against published baseline
`5934b9897854a4102ef71d6463d4b4f7a3005158`.

Four modified product Rust files add **0 production / 265 test-support /
0 benchmark** nonblank physical lines. Cumulative totals are
**56,866 production / 40,009 support / 105 benchmark**, totaling
**96,980 lines across 194 product Rust files**.

Support deltas: movie.rs +17; macos/fixtures.rs +10; macos/tests.rs +68;
App gif_converter/native_host_tests.rs +170. The complete ungated production
line sequence is unchanged. Public GeneratedTiming/generated_movie_case items
remain sealed fixture-gated support, not production merely because they are pub.
Entire fixture/test modules retain support classification.

Two existing standalone docs Rust probes remain unchanged: **115 evidence-only
lines in two files**, separately inventoried and excluded from product totals.
All tracked .rs reconcile to **97,095 lines across 196 files**. No new standalone
Rust or screenshot files were added. The two MOV fixtures are separately
hash-bound binary test assets with zero Rust LOC.

Comments count; complete positive test/fixture-only spans include attributes,
all bodies and closing braces. Adjacent production comments retain their class.
Ordinary platform/production cfg alternatives remain production. Shared workbench
is counted once in BelloBox, never again through BelloAgent's Git dependency.

ledger.json binds both commit/tree identities, source hashes, explicit ranges,
current published baseline ledger/addendum and complete inventories. Portable
and live verification plus six tamper controls pass. No source edits, Cargo,
builds or remote writes were performed. Review/platform gates remain separate;
LOC implies neither parity nor ETA.
