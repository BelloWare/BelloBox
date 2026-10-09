# Shared editor dependency: independent composed-source audit

Summary: LOC.md. Full accounting: ledger.json. All work is isolated under this
audit directory. Neither owned source checkout was edited and no refs were
created or changed. composition-source/ materializes an offline source tree;
before/after snapshots use.txt extensions. No Cargo command was run.

## Exact composition

Start with popup80e6 source, including its final two-file fixture repair.
Overlay exactly five postimages from editor2e72: Cargo.toml, editor_view.rs,
editor_presentation_tests.rs, text_presentation.rs and lib.rs. All five hashes
match the frozen candidate proof. Full Git path maps against common15197 show
no overlap between popup changes and editor changes. The composed tree equals
popup80e6 everywhere else, including both standalone Rust probes.

The deterministic offline tree is edf8c85509d3885567b2bf09c51ea67766a02c2f.
No commit is claimed for this historical offline tree. The actual documentation
parent0e13 and source childe6a are now verified separately below; their exact
source binding supersedes the initial provisional status while preserving this
original ledger and composition identity.

## Independent delta and classification

- editor_view.rs: +394 production / +3 support
- editor_presentation_tests.rs: 0 production / +512 support
- text_presentation.rs: +77 production / +59 support
- lib.rs: +5 production / 0 support

Total +476 production / +574 support / 0 benchmark. Cargo.toml is hash-bound but
excluded from Rust LOC. The input candidate's narrow totals were independently
recounted, not copied as authority for the final cumulative figure.

editor_view.rs support is before1113–1496; after1508–1891 and1893–1895. The
latter three-line cfg/path/module declaration is support. The new external
editor_presentation_tests.rs is wholly support at1–524 (512 nonblank lines).
text_presentation.rs has a complete inline test module81–139 (59 lines); its
preceding production helpers/comments remain production. lib.rs is entirely
production, including physical re-exports. Nonblank comments count. Ordinary
production/feature cfg alternatives are not test-only merely by mentioning test.

Baseline categories come from the exact popup audit ledger SHA
88c07cd566003f7b039c7831981d4889655afca9bc841be13fc90d2d4cf594b2,
preserved as popup-baseline-ledger.json; every baseline product source SHA matches
that ledger and exact80e6 Git blobs. The product inventory grows199→201 files,
with197 unchanged and two new Rust files. All-source sums reconcile99,617→100,667.
Shared editor/workbench code is counted once in BelloBox and not duplicated for
BelloAgent's Git consumer. The two existing docs probes remain19+96=115 lines,
separately excluded from product categories and fully Git/hash-bound. Their
union with product source exactly matches203 tracked .rs paths and100,782 lines.

## Reproduce

    python verify.py
    python negative-controls.py

Offline verification uses Python's standard library, no Git executable or
network required. Original commit/tree/blob objects bind popup, editor and
common-base inputs. The verifier checks the entire composed tree, disjoint
change sets, postimage SHA/byte lengths, all product/evidence source, explicit
support ranges and arithmetic. Verification is of the offline composition, not
a live integration checkout or future publication. Optional --repo can verify
a real composed Git checkout later if its source scope exactly matches.

Six controls reject source drift, altered ranges, missing product inventory,
modified baseline tree, missing probe and modified composed tree. These checks
are exact reviewed source accounting, not a universal Rust parser, native test
result, performance proof or source-acceptance decision.

## Text-only publication package

If before/after snapshots, evidence copies and binary Git tree objects are omitted
from publication, verify.py alone is not standalone. Use the supplied wrapper:

    python verify_from_git.py --repo /path/to/BelloBox

The repository must contain exact80e6,2e72 and15197 commit objects. The wrapper
uses read-only Git to reconstruct required source/evidence/object files in a
temporary directory, runs the same verifier, then deletes temporary files. It
never fetches, changes refs/index/worktrees, runs Cargo or modifies source.
Required published text inputs: verify.py, verify_from_git.py, ledger.json,
source-manifest.json, final-manifest.json, editor-manifest.json and
popup-baseline-ledger.json. git-rehydrated-verification.json records a successful
run using only those text inputs plus a supplied repository. Original negative
control results remain receipts; negative-controls.py needs the full offline
audit directory and is not independently runnable from a stripped text package.
Do not package construction utilities build-ledger.py or bind-tree.py.

## Final publication-source binding (supersedes provisional status)

The historical ledger and offline composition remain unchanged for attribution.
publication-binding.json binds their counts to exact source child e6a39466,
parent0e13ee6. The source trees differ from the earlier offline tree only because
the actual parent includes documentation; every Rust blob matches the audited
composition, with the exact five postimages and no source overlap.

    python verify_publication.py --repo /path/to/BelloBox
    python publication-negative-controls.py --repo /path/to/BelloBox

For text-only packaging, add verify_publication.py and publication-binding.json
to the previously listed required inputs. publication-verification.json records
success; publication-negative-controls.py/.json supply three additional failing
controls for final tree, parent and category-total tampering. The repository
must also contain exact0e13 ande6a objects. No fetching/ref/index/worktree edits
or builds occur. A checked-out branch need not point to the child: verification
reads immutable commit objects rather than assuming HEAD is the candidate.
