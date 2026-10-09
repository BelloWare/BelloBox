# HTTP & cURL validation checkpoint

This bounded text package records local validation of the ordinary independent
HTTP window. It does not establish native Swift/Foundation execution, exact
published-commit CI success, full Swift UI parity, or performance. No screenshots,
compiled binaries, real credentials, or private workspace paths are included.

- [Workflow and limits](../../http-workflow.md)
- [Verification summary](verification.json)
- [GUI observations](gui.md) and [machine-readable receipt](gui-receipt.json)
- [Core checks and seven mutations](core-checks.json)
- [App handoff](app-handoff.md) and [command outcomes](app-commands.json)
- [Independent source review](independent-review.json)
- [Earlier Core failure/correction history](failure-notes.json)
- [Immutable Swift source verification](swift-source-verification.json)
- [Changed source/fixture/CI inventory](changed-source.json)
- [LOC ledger](loc.json), [controls](loc-controls.json), and
  [parameterized verifier](verify-http-loc.py)
- [Documentation/evidence supplement](documentation-evidence-supplement.json)

The original [GUI seal](gui-seal.json) and [GUI source manifest](gui-source-manifest.json)
are preserved separately from later documentation/evidence. The
[compiled source manifest](compiled-source-manifest.json) binds the tested Rust
and Cargo files. Do not treat the GUI manifest as a final publication tree or
silently reseal it after adding this package.

LOC reproduces the prior Regex accounting method: baseline Git objects and ledger,
nonblank physical Rust lines including comments, explicit positive test-only spans
and external test children, benchmarks separate, shared code once under Box.
Delta is +1,891 production / +1,966 test-support / +0 benchmark; prospective totals
are 67,188 / 50,357 / 105. These are not completion percentages. The verifier must
run against an unmodified candidate source tree and a Git checkout/object store
containing the immutable predecessor; output goes to a separate directory:

```sh
python verify-http-loc.py --candidate /path/to/candidate --git-root /path/to/git-checkout --output /path/to/separate-audit-output
```

The ledger matches the byte-identical independently reproduced result. Controls
reject an incorrect total, changed test classification, omitted changed path,
corrupted unchanged hash, and duplicated shared path. The classifier is scoped
to the inspected changed sources, not a universal Rust parser. Future compound
cfg changes require explicit review.

App's initial link failed for missing native library search paths; the corrected
build environment allowed a passing rerun. The measurement harness also had an
initial path error before Cargo started. Source review corrected its own earlier
Latin-1 wording: final opaque response headers use tagged ASCII byte escapes.
Earlier failures are not counted as passes. Logs are represented by original
hashes, command receipts and bounded summaries rather than bulky machine paths.
