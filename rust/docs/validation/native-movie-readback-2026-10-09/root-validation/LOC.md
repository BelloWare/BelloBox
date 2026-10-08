# GIF completion: peer source plus root Clippy adjustment

Exact published baseline: `abd7247c25c1736f7a4fd6a34032e7af9d7a1f5d`.
Peer commit: `7b9a99ae7c19b8251836cc82b6618101e8f8a55e`, tree
`de876ac8da9860ce584e815aff50be5cd46f4186`.

The frozen candidate is that peer source plus the one-line gif_decode.rs change
at line 121 recorded in working-tree-addendum.json. Its final file SHA256 is
`643a162698bf3f113cfed4b6c1ec63605121ac5febc0d2d3cc47fd3efc1ca22c`.
This Clippy adjustment changes no physical line count or category. No later
commit identity is claimed by this pre-publication audit.

Product delta: **+121 production / +587 test-support / 0 benchmark**.
Candidate totals: **56,866 production / 39,744 support / 105 benchmark**,
or **96,715 nonblank physical Rust lines across 194 product files**.

Separately inventoried standalone docs probes contribute 19 and 96 nonblank
lines: **115 evidence-only lines in two files**, excluded from product categories.
All tracked .rs therefore reconcile to **96,830 nonblank lines in 196 files**.
Their bytes, hashes and Git identities are retained, not silently discarded.

The shared decoder is counted once; removed setup in callers is subtracted.
Complete test modules and cfg(test) declarations are support. Comments count;
adjacent production comments retain their category. Inherited baseline source
categories are Git-bound. Shared workbench counts once in BelloBox, not again
through BelloAgent's Git dependency.

ledger.json preserves the exact peer audit. working-tree-addendum.json and
working-tree-source-manifest.json bind the final narrow change. Run
verify_candidate.py for the frozen root candidate. Offline/live inventory
verification passes, as do six peer tamper controls and two addendum controls.
No builds, source edits or remote writes were performed by this audit.
Behavioral review and final gates are separate; LOC implies neither parity nor ETA.
