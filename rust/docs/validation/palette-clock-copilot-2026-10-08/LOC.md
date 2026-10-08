# Reviewed palette Copilot handoff Rust LOC

Exact baseline: `2a5f95b35d27f33b19e4530e696a06da496fc9b8`.
Its Rust tree is identical to `c3ced91eeb7678305f770cfc144352a9643d4327`.
Baseline counts: 55,276 production / 37,190 test-support / 105 benchmark.

The frozen 14-path change adds **828 production / 1,240 test-support /
0 benchmark** nonblank physical Rust lines. Candidate cumulative totals are
**56,104 production / 38,430 test-support / 105 benchmark**,
or **94,639 lines across 188 active Rust files**.

The moved worker is counted once: remove its old 146 production lines and add
169 production / 2 support at the shared path, for a net +23 / +2. The other
source changes, including complete new external test modules and explicit
inline/test-only helper spans, are classified in loc-ledger.json. Comments count;
adjacent production comments keep their category. Feature/platform code stays
production. Unchanged baseline categories are inherited from the published
ledger and bound to exact source hashes.

Manifest SHA256: `298d7414e08c901f6c7840c6fd4524de237ab0a05f14dbcd268c391bf55b1dbf`.
Patch SHA256: `32c5b6017439fe8601ffe7f5af843f5bbfdb738926a85ef4345ff43e4d481169`.
All 13 present postimages and the old-worker deletion match. Portable/live
verification and five tamper controls pass. Shared workbench is counted once
in BelloBox; BelloAgent's Git dependency, snapshots, docs and patches are excluded.

Full clean tests and final review were pending at accounting time. No source
was edited or build run by this audit. These counts imply neither parity nor ETA.

Repository scope: this summary, loc-ledger.json and loc-verification.json are published. The full local snapshot/verifier bundle is not uploaded here. Final tests and review are recorded separately in README.md.
