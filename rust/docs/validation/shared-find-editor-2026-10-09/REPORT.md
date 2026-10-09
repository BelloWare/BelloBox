# Shared editor native preflight receipt verification

## Result and provenance

The native peer reported a successful **library-only** preflight for BelloBox candidate
`2e72ec51bd010a96624ca7326f9a9afcb1057686`, tree
`c3af9d1030f8d65df98cd9ef1b41d7759ca5ea5f`, parent
`15197a71d81c620bf60da18f0d066505e4c0dc99`.

Source: https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074894362
Published by GitHub account BelloWare on 2026-10-09 at 05:26:46 UTC.
The comment calls its producer migration-peer. This verifier fetched the full comment
through the GitHub connector and inspected its lossless text evidence independently.
No native command was rerun here; no native host or binary was directly observed.

Verified independently in this packaging pass:

- Base64/xz transport digest and decoded JSON digest exactly match the comment.
- Every one of 24 embedded UTF-8 record hashes matches, including unabridged logs,
  command records, host output, preparation/validation records and runner/finalizer scripts.
  The bundled peer scripts were inspected as text, never executed here.
- Exact candidate commit, tree, parent and five changed-path set match local Git objects.
  All five postimage SHA-256 values, byte lengths, Git blobs and modes match candidate
  objects and the frozen local source bytes. Cargo.lock is absent from the candidate diff.
- All 1,925 tracked candidate metadata records were reconstructed from Git objects using
  the peer's JSON record format. The reconstructed manifest digest exactly matches the
  native receipt: `2c23f2d4d99abaf65153245f1095ac994227a1b6d34ec0e63d1a04067d6cb067`.
  This verifies the candidate baseline metadata, not observation of the peer filesystem
  before/after execution. The native before/after/clean-checkout statements remain
  peer-attested through the hash-verified receipt and inspected finalizer implementation.
- Native raw logs contain exactly 37 distinct listed tests and exactly those 37 names
  each executed once successfully: 37 passed, 0 failed/ignored/measured/filtered out.
  The same name set appears in Linux owner r9 and independent rerun logs.
- Six native result records consistently match requested command arrays and raw log
  digests, all exit 0. The extra initial package clean is also recorded exit 0.
  Requested test execution used `--test-threads=2`.
- The final package-clean completion precedes the ordinary library build. Its log shows
  a fresh compilation of bello-workbench-ui in the exact isolated checkout.
- Native logs contain no compiler warning/error or Cargo future-incompatibility notice.
  The Linux owner Clippy/build logs retain the proc-macro-error2 future-compatibility
  notice; that Linux finding must not be called a native warning or erased.

## Native commands and environment (peer-recorded)

All package commands ran from the isolated exact checkout's rust directory.
Cargo uses offline locked dependencies; format is `cargo fmt --all -- --check`.

1. `cargo fmt --all -- --check`: exit 0.
2. `cargo test --offline --locked -p bello-workbench-ui --lib -- --list`: exit 0; 37 names.
3. `cargo test --offline --locked -p bello-workbench-ui --lib -- --test-threads=2`: exit 0; 37 passed.
4. `cargo clippy --offline --locked -p bello-workbench-ui --all-targets -- -D warnings`: exit 0.
5. `cargo clean --offline --locked -p bello-workbench-ui`: exit 0.
6. `cargo build --offline --locked -p bello-workbench-ui --lib`: exit 0.

Additional pre-test `cargo clean --offline --locked -p bello-workbench-ui`: exit 0.
Recorded host: macOS 14.8 (23J21), arm64/aarch64-apple-darwin; Rust/Cargo 1.91.1,
Swift 6.0.2, SDK 15.1, deployment target 14.0; two Cargo jobs and isolated generated
configuration. Host records report no matching environment override names removed;
the runner filters provider/fixture override names. Do not infer removal of actual
configured credentials or real-provider testing from that filtering policy.

Inventory and execution record the same Mach-O arm64 test-binary SHA-256:
`a4c0337745824f53e26e6e623025a76db42f0bd58f967854ead3e40a365e8dc6`.
Final ordinary `libbello_workbench_ui.rlib` peer-recorded SHA-256:
`08edcdde97332e9ab51d0757b0262f59d3578ab5886dd9afe4b766d1a8610f80`.
These hashes are internally consistent receipt fields, not rehashed native artifact
bytes here. The artifacts were deliberately not transferred.

## Exact source postimages

- rust/crates/bello-workbench-ui/Cargo.toml: 349 bytes;
  `85551129fcb904d1cd6b36fa5e08a117747037d45d9941bdd5df3872d8609470`.
- rust/crates/bello-workbench-ui/src/editor_view.rs: 71,200 bytes;
  `384588661e1e70c66730a7e0318b1a574116d789d6f153a6e8b5139d5621da11`.
- rust/crates/bello-workbench-ui/src/editor_presentation_tests.rs: 20,557 bytes;
  `b024ad91b362f8b98e23095b6f2d70ede8a889708d953529913a5d522d266736`.
- rust/crates/bello-workbench-ui/src/text_presentation.rs: 4,762 bytes;
  `ee9ceeb559880cac8f23220d1c13a624fc2c4db88147d4424865ace434549fac`.
- rust/crates/bello-workbench-ui/src/lib.rs: 715 bytes;
  `23e57b6010a45aa4ca2be0960c396907069a566dd1fa7ea8f535a09c0d050a3e`.

## Linux evidence and independent source review

`linux/` preserves all original hashed development logs, including failed iterations,
final r9 37-test pass, strict Clippy, formatting and ordinary library build. This pass
verified every original log-manifest digest and length. No Cargo/build lane was used.
`independent/` preserves the prior review and independent execution log. That reviewer
found no remaining blocking issue in the frozen shared-editor scope and independently
executed the owner-built Linux test binary: 37 passed. It was an independent test
execution, not an independent rebuild. This packaging pass only verified its text.

## Scope limitations and integration duties

These are synthetic GPUI TestPlatform/editor scene checks and native library preflight.
They do not establish whole-app native binary validity, AppKit/AX/real IME/OS visibility,
interactive acceptance, the Agent consumer's complete Find workflow, performance gains,
release/signing readiness, pinned CI, or cross-package integration validity.

The consumer must preserve exact original UTF-8/CRLF text and range identity; fence outer
navigation/controller/chat/window/lifetime generation; invalidate/rearm after outer
geometry changes; reject stale receipts inside callbacks; avoid reusing receipt geometry
across later scrolling/layout; and bound retries/timeouts where no visible fragment can
produce a receipt. Report honest row-level/unavailable destinations rather than inventing
an exact landing. Shared code is counted once in BelloBox.

This package binds the five source postimages to the standalone candidate above.
The exact integrated source child is now `e6a39466f9bf1722557cf6a350e53bb9f970bc89`,
tree `30709481594d4b63fae8ccfbba3d121c389c4da3`, parent
`0e13ee6def214425865759591c37539429530d71`. Independent read-only Git verification
confirmed exactly the five expected changes, byte/blob/mode equivalence to the native-tested
standalone candidate and preservation of every other parent path. Popup and editor
changes do not overlap. This is shared-source equivalence, not native execution of
the integrated whole tree. The native receipt continues to identify standalone `2e72ec5`.
The complete shared UI crate tree, its sole local bello-workbench dependency tree,
workspace manifest/lock, and all seven repository Cargo configuration/manifests/lock
paths also match exactly; integration/compilation-input-equivalence.json records
this additional source-closure proof, independently checked here. Registry source
bytes were not reverified; their dependency resolution remains pinned by unchanged
Cargo.lock. Popup app source is outside this library closure.
No branch-ref publication of the shared child is claimed by this package. `candidate/source-count-delta.json` records the narrow shared delta
(+476 production, +574 support nonblank Rust lines); its inherited provisional totals
are not a cumulative popup-composition audit. The independently audited composition
of popup `80e6d41a68fc0e2bf186272fb788a301f4f9e744` plus exactly these five
postimages has 58,381 production / 42,181 support / 105 benchmark nonblank physical
Rust lines (100,667 product lines across 201 files), with 115 standalone evidence
probe lines separately excluded. The complete tracked Rust total is 100,782 across
203 files. Shared code is counted once. There is zero popup/editor changed-path
overlap. Its offline synthesized tree is `edf8c85509d3885567b2bf09c51ea67766a02c2f`;
this is not a published commit. Ledger SHA-256:
`afc8d365f0c41abab978a6d34499ac952aebf7dc32dc1463de6e74c5f1d534ba`.

`loc/` includes the cumulative ledger, exact baseline ledger, manifests, six negative
control receipts and Git-backed text-only verifier. This packaging pass independently
ran `PYTHONDONTWRITEBYTECODE=1 python loc/verify_from_git.py --repo /path/to/repo`
and reproduced the complete composition/count proof. It transiently reconstructs
required Git objects and source snapshots in a temporary directory and deletes them;
they are not distributed. `loc/verify.py` alone needs omitted snapshot/object inputs;
use the wrapper with a repository containing all three exact input commits.
The final shared-child identity and exact preservation proof are included under integration/.
The ledger's original offline-composition identity remains explicitly distinct from this
docs-parent integration; the additional LOC binding proof establishes equal Rust scope.
`loc/publication-binding.json` SHA-256 is
`4d0e026e7a84088e6724156050af6a4a2dccfe11f28344a40a81aa719302ede3`.
This packaging pass also independently ran `loc/verify_publication.py --repo` against
the integrated checkout: all 203 Rust paths, totals, exact five postimages and parent
preservation passed. That binding supersedes the original ledger's provisional
publication status; it still makes no branch-ref publication or native runtime claim.
No commits were created, source edited, or refs moved by this packaging pass.

## Package inventory and verification

- native-comment.json: full fetched API comment, including original transport.
- native-evidence.json: exact decoded lossless JSON.
- native/: all 24 decoded records, unchanged.
- candidate/: prior candidate proof, narrow source-count delta/request, and independently
  reconstructed tracked metadata manifest. No duplicate Rust source is included.
- linux/: source manifest, owner handoff and all hashed Linux logs. One internal review-routing
  identifier is editorially omitted from copied handoff prose, disclosed there. Raw
  logs, source hashes and validation claims are unchanged.
- independent/: prior review, rerun log and test-binary checksum text.
- integration/: exact immutable child identity and five-path preservation proof.
- loc/: cumulative composition accounting and its verified Git-backed reproduction.
- verification.json: this verifier's machine-readable result.
- verify-package.py: read-only Git/object/log verifier; no Cargo or peer-script execution.
- SHA256SUMS: integrity inventory for all other package files.

This package is UTF-8 text only: no screenshots, native binaries, archives, source patches,
or duplicate Rust source copies. The encoded native payload contains only text records.
To check delivered package integrity, run `sha256sum -c SHA256SUMS` from this directory.
To repeat semantic verification, run `python verify-package.py /path/to/frozen/checkout`;
that refreshes verification.json's timestamp and reconstructed manifest, so the original
package checksum inventory should be checked first. The checkout must have candidate
Git objects and the five frozen source postimages. Neither command alters repository refs.
