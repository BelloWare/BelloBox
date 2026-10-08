# Reviewed Clock Copilot r2 Rust counts

Source freeze: manifest SHA256
`27b87dcd0e288ffe68eda9116e8a545a954b15c1980ff8383f0d560ba7e541e3`.
All 12 frozen paths match, including nine Rust files.

Against `dec241107f359146d0e63fb051dfafe94e1425e1`, Clock adds
**1,346 production / 1,498 test-support / 0 benchmark** nonblank physical lines.
The Clock-only checkout totals 55,255 / 36,880 / 105 = 92,240 lines in 184 files.

Integrating the exact disjoint Clock patch over published
`e9cdb40c6475a9ba006cd245050fc650a82780ba` retains its provider Quit repair and
produces **55,276 production / 37,190 test-support / 105 benchmark**, totaling
**92,571 lines across 185 Rust files**. Both baseline Git trees and the complete
physical inventories are independently verified. This is source-union
accounting, not a claim that the Clock worktree already contains the Quit repair.

Comments count. Complete positive test-only cfg spans and the two external
test modules are support; adjacent production comments keep their category.
Ordinary production/feature cfg alternatives remain production. Existing
baseline categories are inherited. Shared workbench is counted once in BelloBox,
not again through BelloAgent's Git dependency. Snapshots, documentation, lockfiles,
manifest files, recovery patches and dependencies are excluded from Rust LOC.

`loc-ledger.json` and `loc-integrated-ledger.json` bind SHA256s, exact reviewed support
ranges, per-file counts and cumulative physical sums. The full local audit bundle (not included here) runs offline and passed portable/live verification plus four tamper controls. Source review and full tests were pending at audit time; final integrated test results are separately recorded in README.md. The final f30 base differs from e9 only by documentation, with identical Rust source. LOC implies neither parity nor an ETA.
