# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T15:17:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 1 mixed windows; 1 with endpoints, 10m 24.0s union; scopes overlap resources and do not measure Review alone |
| Builds | Unavailable separately |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 3m 13.8s measured command resource time |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | 331.455ms measured command resource time |
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | 16m 0.4s measured API/client time; 2 mixed windows; 2 with endpoints, 9m 36.6s union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 9 | 230.526 | 9 | 230.526 | 230.526 |
| catchup_api | 55 | 968.019 | 55 | 968.019 | 216.248 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 193.842 |
| catchup_command: dependency/environment recovery | 0.331 |
| catchup_command: automated accounting validation | 1.991 |
| catchup_command: automated lint/check | 34.362 |
| catchup_api: publication | 960.444 |
| catchup_api: publication verification | 7.575 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T14:57:00Z–2026-10-09T15:17:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Regex disk wait, recovery gates, coordination and packet preparation | 2026-10-09T14:56:04Z | 2026-10-09T15:06:28Z | 624 | completed |
| Regex immutable candidate preparation, upload, review wait, retry and verification | 2026-10-09T15:07:15Z | 2026-10-09T15:16:51Z | 576.0 | completed |
| Initial metadata blob operation denied | unknown | unknown | unknown | Exact authorized retry later succeeded; initial elapsed unavailable |
| Combined immutable candidate verification read batch | 2026-10-09T15:16:50.401Z | 2026-10-09T15:16:51.618Z | 1.217 | completed |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| minimal-core-app-r2 | build + test/check (combined) | 2026-10-09T14:57:25.073340+00:00 | 2026-10-09T14:59:07.982661+00:00 | 102.90933551899798 | 0 |
| recover-minimal-app-clean | dependency/environment recovery | 2026-10-09T14:59:08.008506+00:00 | 2026-10-09T14:59:08.339956+00:00 | 0.331454961997224 | 0 |
| loc-keyboard-update | automated accounting validation | 2026-10-09T14:59:22.370261+00:00 | 2026-10-09T14:59:23.372707+00:00 | 1.0024502930027666 | 0 |
| aggregate-app-final | build + test/check (combined) | 2026-10-09T15:02:46.293567+00:00 | 2026-10-09T15:04:08.235116+00:00 | 81.94155260699335 | 0 |
| final-feature-strict | automated lint/check | 2026-10-09T15:04:08.266384+00:00 | 2026-10-09T15:04:27.286075+00:00 | 19.01969456099323 | 0 |
| minimal-strict | automated lint/check | 2026-10-09T15:04:27.310597+00:00 | 2026-10-09T15:04:40.681043+00:00 | 13.370450591988629 | 0 |
| final-format-r2 | automated lint/check | 2026-10-09T15:04:40.705950+00:00 | 2026-10-09T15:04:42.677604+00:00 | 1.9716592869954184 | 0 |
| publication-remote | build + test/check (combined) | 2026-10-09T15:07:29.822680+00:00 | 2026-10-09T15:07:38.813324+00:00 | 8.990649151994148 | 0 |
| Independent final Regex LOC/source reproduction | automated accounting validation | 2026-10-09T15:07:46.026267+00:00 | 2026-10-09T15:07:47.014861+00:00 | 0.9885964120039716 | passed; full ledger/source reproduction match |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/binary.sha256 | publication | 2026-10-09T15:10:21.631Z | 2026-10-09T15:10:30.692Z | 9.061 | created |
| Create immutable Regex blob: .github/workflows/rust-macos.yml | publication | 2026-10-09T15:10:49.246Z | 2026-10-09T15:10:58.182Z | 8.936 | created |
| Create immutable Regex blob: rust/crates/bellobox-app/src/main.rs | publication | 2026-10-09T15:10:49.246Z | 2026-10-09T15:11:07.046Z | 17.8 | created |
| Create immutable Regex blob: rust/crates/bellobox-app/src/regex_session.rs | publication | 2026-10-09T15:10:49.246Z | 2026-10-09T15:11:15.643Z | 26.397 | created |
| Create immutable Regex blob: rust/crates/bellobox-app/src/regex_ui.rs | publication | 2026-10-09T15:10:49.246Z | 2026-10-09T15:11:24.194Z | 34.948 | created |
| Create immutable Regex blob: rust/crates/bellobox-app/src/desktop.rs | publication | 2026-10-09T15:10:49.246Z | 2026-10-09T15:11:30.585Z | 41.339 | created |
| Create immutable Regex blob: rust/crates/bellobox-core/src/developer/regex_inspection_tests.rs | publication | 2026-10-09T15:11:24.194Z | 2026-10-09T15:11:31.808Z | 7.614 | created |
| Create immutable Regex blob: rust/crates/bellobox-app/src/regex_session/tests.rs | publication | 2026-10-09T15:10:49.246Z | 2026-10-09T15:11:35.996Z | 46.75 | created |
| Create immutable Regex blob: rust/crates/bellobox-core/tests/fixtures/regex-swift-characterization.json | publication | 2026-10-09T15:11:30.585Z | 2026-10-09T15:11:36.721Z | 6.136 | created |
| Create immutable Regex blob: rust/crates/bellobox-core/tests/fixtures/regex-swift-oracle.json | publication | 2026-10-09T15:11:31.809Z | 2026-10-09T15:11:43.895Z | 12.086 | created |
| Create immutable Regex blob: rust/crates/bellobox-app/src/regex_ui/tests.rs | publication | 2026-10-09T15:10:58.182Z | 2026-10-09T15:11:44.359Z | 46.177 | created |
| Create immutable Regex blob: rust/docs/bellobox-parity.md | publication | 2026-10-09T15:11:36.721Z | 2026-10-09T15:11:44.739Z | 8.018 | created |
| Create immutable Regex blob: rust/docs/comparison-workflow.md | publication | 2026-10-09T15:11:43.895Z | 2026-10-09T15:11:51.159Z | 7.264 | created |
| Create immutable Regex blob: rust/crates/bellobox-core/tests/regex_swift_oracle.rs | publication | 2026-10-09T15:11:35.996Z | 2026-10-09T15:11:51.575Z | 15.579 | created |
| Create immutable Regex blob: rust/crates/bellobox-core/src/developer.rs | publication | 2026-10-09T15:11:07.046Z | 2026-10-09T15:11:52.503Z | 45.457 | created |
| Create immutable Regex blob: rust/docs/regex-workflow.md | publication | 2026-10-09T15:11:44.359Z | 2026-10-09T15:11:57.863Z | 13.504 | created |
| Create immutable Regex blob: rust/crates/bellobox-core/src/developer/regex_inspection.rs | publication | 2026-10-09T15:11:15.643Z | 2026-10-09T15:11:59.824Z | 44.181 | created |
| Create immutable Regex blob: rust/docs/url-workflow.md | publication | 2026-10-09T15:11:44.739Z | 2026-10-09T15:12:05.936Z | 21.197 | created |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/supplemental-build-inputs.json | publication | 2026-10-09T15:13:08.366Z | 2026-10-09T15:13:17.081Z | 8.715 | created on exact authorized retry |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/source-inventory.json | publication | 2026-10-09T15:14:06.461Z | 2026-10-09T15:14:17.032Z | 10.571 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/local-validation.json | publication | 2026-10-09T15:14:06.461Z | 2026-10-09T15:14:24.926Z | 18.465 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/post-gui-source-delta.json | publication | 2026-10-09T15:14:06.461Z | 2026-10-09T15:14:34.582Z | 28.121 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/final-source-manifest.json | publication | 2026-10-09T15:14:06.461Z | 2026-10-09T15:14:54.621Z | 48.16 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/negative-controls.json | publication | 2026-10-09T15:14:54.621Z | 2026-10-09T15:15:02.862Z | 8.241 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/commands.jsonl | publication | 2026-10-09T15:14:06.461Z | 2026-10-09T15:15:03.440Z | 56.979 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/output-bound-mutant-excerpt.json | publication | 2026-10-09T15:15:02.862Z | 2026-10-09T15:15:11.229Z | 8.367 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/gui-source-manifest.json | publication | 2026-10-09T15:14:06.461Z | 2026-10-09T15:15:12.545Z | 66.084 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/sealed-cli-regression.log | publication | 2026-10-09T15:15:03.441Z | 2026-10-09T15:15:18.002Z | 14.561 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/gui-receipt.json | publication | 2026-10-09T15:14:17.032Z | 2026-10-09T15:15:19.438Z | 62.406 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/independent-loc-review.json | publication | 2026-10-09T15:15:12.545Z | 2026-10-09T15:15:21.904Z | 9.359 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/refs-before-candidate.txt | publication | 2026-10-09T15:15:11.229Z | 2026-10-09T15:15:24.863Z | 13.634 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/loc/ledger.json | publication | 2026-10-09T15:14:24.926Z | 2026-10-09T15:15:25.858Z | 60.932 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/independent-final-review.json | publication | 2026-10-09T15:15:18.002Z | 2026-10-09T15:15:29.810Z | 11.808 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/loc/verification.json | publication | 2026-10-09T15:14:34.582Z | 2026-10-09T15:15:31.968Z | 57.386 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/main-verification.json | publication | 2026-10-09T15:15:19.438Z | 2026-10-09T15:15:37.901Z | 18.463 | created after verified absent |
| Create immutable Regex blob: rust/docs/validation/regex-workflow-2026-10-09/README.md | publication | 2026-10-09T15:15:21.904Z | 2026-10-09T15:15:44.924Z | 23.02 | created after verified absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/source-inventory.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.310Z | 0.496 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/final-source-manifest.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.194Z | 0.38 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/gui-source-manifest.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.217Z | 0.403 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/post-gui-source-delta.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.172Z | 0.358 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/local-validation.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.173Z | 0.359 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/commands.jsonl | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.332Z | 0.518 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/gui-receipt.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.292Z | 0.478 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/loc/ledger.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.310Z | 0.496 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/loc/verification.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.187Z | 0.373 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/negative-controls.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.266Z | 0.452 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/output-bound-mutant-excerpt.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.167Z | 0.353 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/sealed-cli-regression.log | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.283Z | 0.469 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/refs-before-candidate.txt | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.217Z | 0.403 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/independent-loc-review.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.405Z | 0.591 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/independent-final-review.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.304Z | 0.49 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/main-verification.json | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.263Z | 0.449 | verified-absent |
| Reconcile uncertain blob existence: rust/docs/validation/regex-workflow-2026-10-09/README.md | publication verification | 2026-10-09T15:13:53.814Z | 2026-10-09T15:13:54.321Z | 0.507 | verified-absent |
| Create immutable Regex tree | publication | 2026-10-09T15:16:09.637Z | 2026-10-09T15:16:20.486Z | 10.849 | Object created; no ref movement |
| Create immutable Regex commit | publication | 2026-10-09T15:16:20.486Z | 2026-10-09T15:16:32.365Z | 11.879 | Object created; no ref movement |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,640 items; 654 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

| Existing resource group | Counted timed items | Resource seconds | Items with endpoints | Endpoint-subset seconds | Subset interval union seconds |
|---|---:|---:|---:|---:|---:|
| ci_job | 168 | 73147.000 | 168 | 73147.000 | 45253.000 |
| local_receipt | 166 | 1389.032 | 156 | 1256.178 | 1256.241 |
| incremental_command | 6 | 249.417 | 6 | 249.417 | 249.417 |
| incremental_ci_job | 2 | 1441.000 | 2 | 1441.000 | 823.000 |
| new_local_command | 25 | 673.614 | 25 | 673.614 | 673.614 |
| new_reported_phase | 3 | 0.890 | 0 | 0.000 | unavailable |
| catchup_ci_job | 14 | 12089.000 | 14 | 12089.000 | 7178.000 |
| catchup_native_command | 11 | 4.661 | 11 | 4.661 | 4.661 |
| catchup_command | 198 | 3483.138 | 167 | 2739.512 | 2737.458 |
| catchup_gui_process | 6 | 1737.635 | 6 | 1737.635 | 1737.635 |
| catchup_api | 55 | 968.019 | 55 | 968.019 | 216.248 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
