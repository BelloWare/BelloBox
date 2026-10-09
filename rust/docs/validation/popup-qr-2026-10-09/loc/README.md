# Popup QR final successor Rust accounting

Exact candidate:80e6d41a68fc0e2bf186272fb788a301f4f9e744; parent4589e0280844df4a070533c7babb9d01cf430905; accounting baseline15197a71d81c620bf60da18f0d066505e4c0dc99.

57,905 production +41,607 test/support +105 benchmark =99,617 active product nonblank Rust lines across199 files. Two unchanged standalone validation probes total115 lines and are excluded. Shared workbench and shared QR helpers count once in Box. Delta:+124 production,+538 support,0 benchmark. Comments count as nonblank physical lines; reviewed support ranges are explicit rather than a universal cfg parser. These counts do not measure parity or completion time.

ledger.json binds every product file, reviewed changed-file ranges, baseline categories, exact candidate tree and excluded probes. Historical verifier receipts are retained unchanged. Publication deliberately omits duplicated Rust snapshots and binary Git objects.

To independently reproduce with all three immutable commits available in a local Git clone:

    python3 reproduce-from-git.py --repo /path/to/BelloBox

The wrapper reads the immutable commits using Git, constructs the original verifier inputs in a temporary directory, invokes unchanged verify.py, and removes temporary snapshots on exit. It does not fetch, modify Git, or use working-tree source. reproduction-result.json records a successful run of this wrapper. Its live_repository_verified=false means it verified immutable Git source, not the checkout's potentially newer working files.

The final fixture repair adds76 support lines and no production lines. loc-v3/ preserves the earlier4589 ledger and its original reproduction. precursor-ledger.json and fixture-production-equivalence.json bind the successor accounting and ordinary binary equivalence.
