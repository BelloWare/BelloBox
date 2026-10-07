# BelloBox recording checkpoint: independent Rust LOC audit

## Result

The frozen recording/shutdown checkpoint is **52,593 production / 31,183 test-support / 105 benchmark nonblank physical Rust lines**, for **83,881 total**.

Against published BelloBox commit `1f5874ce741b8de21fa09c747a5bada7488e2982`, tree `f797c0e9c7956ef842a4bb6a4dc769bb932ee897`, this is **+2,540 production / +2,795 support / 0 benchmark**.

| Category | Published baseline | Added | Removed | Reclassified in | Reclassified out | Frozen checkpoint |
|---|---:|---:|---:|---:|---:|---:|
| Production | 50,053 | 2,696 | 152 | 0 | 4 | 52,593 |
| Test/support | 28,388 | 2,841 | 50 | 4 | 0 | 31,183 |
| Benchmark | 105 | 0 | 0 | 0 | 0 | 105 |
| All Rust | 78,546 | 5,537 | 202 | — | — | 83,881 |

The baseline has 148 Rust source files. The checkpoint has 163: 20 modified, 15 added, 128 byte-identical. Every one of the 35 changed files has complete before/after classifications, including unchanged support bodies; 12,484 production and 3,431 support nonblank lines remain explicitly classified as unchanged. Four literally preserved lines in `gif_converter/model.rs` move from production to an explicit recording-fixtures branch. This textual correspondence is recorded, not interpreted as semantic code movement.

## Scope and classification

- Count nonblank physical Rust lines, including comments. A long/minified physical line counts once.
- Positive test, DEBUG and fixture-only spans start at their attributes. Preceding comments retain their containing classification. Whole inherited test/fixture modules include their imports and comments as support.
- Ordinary platform code, `cfg(any(target_os = ..., test))`, negative test/DEBUG alternatives, normal-build no-op gates, unguarded shared writer/validation/provenance/save machinery and mixed production/support physical lines remain production.
- Every changed file's complete complement of reviewed support ranges is production. Frozen source hashes bind these explicitly reviewed ranges; the verifier is not a generic Rust cfg parser.
- Unchanged files retain the published baseline categorization. The baseline's **whole physical total** is independently recounted and reconciles exactly. This audit is a reviewed checkpoint delta, not a fresh semantic recategorization of every unchanged baseline file.
- Shared `bello-workbench` and `bello-workbench-ui` appear exactly once in the inventory: 9 / 6 Rust files, 5,782 / 3,208 physical nonblank lines, all byte-identical to the published baseline. They must not be added again to an aggregate BelloAgent+BelloBox total.
- Benchmark/example scope remains the published 105 physical lines, with unchanged bytes.
- `.rs.txt` archives, documentation, logs, Cargo manifests/lockfile, generated/build output, dependencies, and external duplicated focused-harness source are excluded. No excluded recovery/fault-mutated copy is substituted for a production source file.

## Exact source and publication binding

All 163 current Rust files match the exact 176-file repository source archive from `20261007T185316Z-531957`; the original manifest contains those 176 repository inputs plus 3 external build-support inputs, **179 total**. The archived initial helper supplies the original bytes for the one external helper that changed later. Before and after manifests are byte-identical.

The frozen executable was independently hashed as:

`bdfc718a20ad84be9f59dc52b459ef0f02c275fa0525c2f6b82dd60ad1d28a54`

The final publication manifest has 216 add/modify paths and 35 Rust paths. Every Rust before/after payload matches this audit exactly. Its SHA-256 is:

`a7645f5509f9e9524184b37e3918e45ffd008895e47d525ffe6643a9ac8a4d85`

The source-set hashes (canonical sorted JSON map of path to content SHA-256) are:

- Before: `db50055a3884672da9d81305aa80f7a9a452866fe5cb738be4b8c032671ad70a`
- After: `614a8976f006e341dae7cf51872abf0d707077c39961da227780a8dd72592562`

The original source-input manifest SHA-256 is `56d04a62fdbc7414f975f4a7db1929969decc64a4d58f1e9131399fd2b75467f`. Its matching before/after snapshot-manifest SHA-256 is `27434f2c0e0479e67f5c35cac672a87f7da9c0998cbe34c42fe92e5f6ce76a2d`.

### Disclosed support-generator drift

The initial helper's archived hash is `a826bfccac70e2a36e4045f0f026ebfc7544c3eb46855aae5f8005bd4fde1831`; the current helper is `97856c86f9f5f277b021de2ee5aed7430811ad243780c2661f0b6d3431f47950` because it later added literal include discovery. No Rust input changed. Two extra non-Rust include inputs, `project.yml` and the app icon, were audited afterward, with their complete byte sequences independently confirmed inside the unchanged executable. This is **not a retroactive 181-input before/after build claim**. The original 179-input records remain unmodified.

## Reproduce without building

Python 3 standard library only. From the unpacked audit directory:

```sh
python3 verify-recording-loc.py
python3 test-detecting-controls.py
```

To additionally compare the checkout's full Rust source inventory and published Git preimages, and verify the actual frozen executable and later include bytes:

```sh
python3 verify-recording-loc.py \
  --repo /workspace/scratch/8b6fda578834/BelloBox-recording-staging \
  --binary /workspace/scratch/8b6fda578834/build-environment/recording-shutdown-qa/bellobox-bdfc718a20ad84be
```

`verification.json` records the full live-source/binary run. `detecting-controls.json` records **24/24 passing controls**: 21 intended rejections and 3 intended exclusions. They cover production/support/unchanged source edits, missing/new/duplicated Rust paths, baseline changes, totals/ranges/unchanged-classification tampering, comment/blank-byte drift, original manifest/archive corruption and prior-ledger rewrites. Archived `.rs.txt`, external focused copies and target artifacts correctly leave counts unchanged. Controls modify temporary audit copies only; the original snapshot is reverified afterward.

## Deliverables and preservation

- `loc-recording-checkpoint.json`: full per-file hashes, category counts, complete classifications and per-span changes **including unchanged lines**.
- `per-file-delta.csv`: compact review index.
- `reviewed-ranges.json`: immutable reviewed positive support intervals, sealed by the verifier.
- `rust-inventory.json`: all before/after Rust paths and content hashes.
- `source-binding.json`, `publication-manifest.json`, `audit-inputs.json`: exact publication and build provenance.
- `prior-ledgers/`: unchanged copies of the published prior LOC ledgers and verifier, including the OCR baseline. No earlier ledger was overwritten.
- `before/`, `after/`: immutable audit snapshots, kept outside source ownership; `build-provenance/`: original archive, exact manifests, preserved initial helper and later include audit.

For publication, preserve the audit package as an archive or publish its JSON/report/verifier evidence. **Do not copy the raw `before/` or `after/` Rust snapshot directories into the application's source tree.**

This audit performed no builds, source edits, desktop operations, commits or publication. LOC classification does not establish native feature availability, runtime acceptance or completion of blocked full-App test suites; those remain separate validation evidence.
