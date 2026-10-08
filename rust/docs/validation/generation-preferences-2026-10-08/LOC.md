# Generation preferences r1: reviewed Rust LOC

Exact baseline: `3bbef6854b5b1323c8573409f7f7b42ea71022d6`.
Baseline: 56,247 production / 38,624 test-support / 105 benchmark.

The 17 changed/new Rust files add **498 production / 533 test-support /
0 benchmark** nonblank physical lines. Candidate cumulative totals are
**56,745 production / 39,157 test-support / 105 benchmark**, totaling
**96,007 lines across 192 Rust files**. The baseline's 189 files independently
sum to 94,976; 175 files are unchanged and three new Rust files are added.

The shared core generation module is counted once: 211 production / 215 support
lines. Complete before/after accounting subtracts 47 production lines from the
image request implementation as duplicate options/logic are replaced. This is
net physical accounting, not an assertion that all new module lines were moved.
Re-exports and call sites count only their actual physical lines.

Comments count. Complete positive test-only cfg spans and external test modules
are support. The sealed image-OCR fixture retains its published whole-module
support classification; that does not reclassify ordinary production/feature
cfg alternatives. Adjacent non-test comments remain production. Unchanged
baseline categories are inherited and bound to the published source ledger.

Manifest SHA256: `54fd3706d632b760dc99cc8168e73b74d17df08826a0538065c179bd7589f673`.
All 18 source/document entries match hashes and byte lengths. loc-ledger.json binds
exact Git commit/tree/blob identities, source SHA256s, reviewed ranges and
physical reconciliation. Portable/live verification and four tamper controls pass.
Shared workbench counts once in BelloBox; BelloAgent's Git dependency, snapshots,
docs, patches and build output are excluded. Full clean gates were pending at
audit time. No builds or source edits were performed; LOC implies neither parity nor ETA.

Repository scope: this summary, loc-ledger.json and loc-verification.json are published. Full local snapshots/verifier and tamper controls are retained separately. Final tests and GUI observations have independent receipts.
