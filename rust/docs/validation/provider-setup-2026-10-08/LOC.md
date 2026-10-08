# Reviewed Rust source counts

Exact baseline7a3ecdc:53,390 production /34,440 test-support /105 benchmark.
The seven changed/new Rust files add519 production and942 support lines.
Candidate totals: **53,909 production /35,382 test-support /105 benchmark**,
89,396 nonblank physical lines across177 Rust files. Comments count.

`loc-audit.json` contains before/after SHA256s, complete reviewed support ranges,
per-file deltas and all-source physical inventory. Positive cfg(test) spans and
test-only modules are support; adjacent production comments keep their category.
The unchanged baseline categories are inherited, with exact baseline Git
commit/tree/blob identities and physical sums independently checked. All170
unchanged Rust files match; allseven changed postimages match frozenr3.

Shared workbench source is counted once in BelloBox; BelloAgent's Git dependency
is excluded there. Recovery/audit source copies and logs are not active Rust LOC.
These counts do not establish parity, performance or a completion estimate.
