# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T16:37:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 0 mixed windows; 0 with endpoints, unavailable union; scopes overlap resources and do not measure Review alone |
| Builds | Unavailable separately |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | Unavailable |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | 31.5s across 1 failed process/API receipts; total rework effort unavailable |
| Publication | 10m 35.0s measured API/client time; 1 mixed windows; 1 with endpoints, 52.1s union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_api | 83 | 634.971 | 83 | 634.971 | 269.680 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_api: publication | 634.971 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T16:22:00Z–2026-10-09T16:37:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Root HTTP source verification and normal publication | 2026-10-09T16:29:41.540Z | 2026-10-09T16:30:33.669Z | 52.129 | completed |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Create immutable HTTP blob: .github/workflows/rust-macos.yml | publication | 2026-10-09T16:22:24.894Z | 2026-10-09T16:22:34.257Z | 9.363 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/http_session.rs | publication | 2026-10-09T16:22:55.553Z | 2026-10-09T16:23:01.299Z | 5.746 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/http_session/tests.rs | publication | 2026-10-09T16:22:55.553Z | 2026-10-09T16:23:05.174Z | 9.621 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/http_ui.rs | publication | 2026-10-09T16:22:55.552Z | 2026-10-09T16:23:09.665Z | 14.113 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/desktop.rs | publication | 2026-10-09T16:22:55.553Z | 2026-10-09T16:23:15.283Z | 19.73 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/shutdown.rs | publication | 2026-10-09T16:23:15.828Z | 2026-10-09T16:23:21.182Z | 5.354 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/http_ui/tests.rs | publication | 2026-10-09T16:23:15.828Z | 2026-10-09T16:23:25.588Z | 9.76 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/main.rs | publication | 2026-10-09T16:23:15.828Z | 2026-10-09T16:23:29.780Z | 13.952 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/transport.rs | publication | 2026-10-09T16:23:15.836Z | 2026-10-09T16:23:36.065Z | 20.229 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/transport/http_request.rs | publication | 2026-10-09T16:23:37.003Z | 2026-10-09T16:23:42.986Z | 5.983 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-core/src/developer/http_request.rs | publication | 2026-10-09T16:23:37.003Z | 2026-10-09T16:23:48.898Z | 11.895 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-app/src/transport/http_request/tests.rs | publication | 2026-10-09T16:23:37.003Z | 2026-10-09T16:23:56.482Z | 19.479 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-core/src/developer.rs | publication | 2026-10-09T16:23:37.005Z | 2026-10-09T16:24:15.154Z | 38.149 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-core/tests/http_swift_oracle.rs | publication | 2026-10-09T16:24:15.834Z | 2026-10-09T16:24:20.181Z | 4.347 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-core/tests/fixtures/http-swift-characterization.json | publication | 2026-10-09T16:24:15.751Z | 2026-10-09T16:24:22.238Z | 6.487 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-core/tests/fixtures/http-swift-oracle.json | publication | 2026-10-09T16:24:15.752Z | 2026-10-09T16:24:28.164Z | 12.412 | SHA/content verified |
| Create immutable HTTP blob: rust/crates/bellobox-core/src/developer/http_request_tests.rs | publication | 2026-10-09T16:24:15.833Z | 2026-10-09T16:24:32.954Z | 17.121 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/README.md | publication | 2026-10-09T16:24:33.715Z | 2026-10-09T16:24:39.389Z | 5.674 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/http-workflow.md | publication | 2026-10-09T16:24:33.716Z | 2026-10-09T16:24:45.768Z | 12.053 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/app-commands.json | publication | 2026-10-09T16:24:33.716Z | 2026-10-09T16:24:50.806Z | 17.09 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/bellobox-parity.md | publication | 2026-10-09T16:24:33.717Z | 2026-10-09T16:24:56.786Z | 23.069 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/app-handoff.md | publication | 2026-10-09T16:24:57.510Z | 2026-10-09T16:25:03.170Z | 5.66 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/core-checks.json | publication | 2026-10-09T16:24:57.510Z | 2026-10-09T16:25:17.264Z | 19.754 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/changed-source.json | publication | 2026-10-09T16:24:57.511Z | 2026-10-09T16:25:22.836Z | 25.325 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/compiled-source-manifest.json | publication | 2026-10-09T16:24:57.511Z | 2026-10-09T16:25:28.369Z | 30.858 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-fixture-20261009T160836-dark.json | publication | 2026-10-09T16:25:28.959Z | 2026-10-09T16:25:35.101Z | 6.142 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-fixture-20261009T160053-light.json | publication | 2026-10-09T16:25:28.960Z | 2026-10-09T16:25:40.400Z | 11.44 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/failure-notes.json | publication | 2026-10-09T16:25:28.959Z | 2026-10-09T16:25:47.498Z | 18.539 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-fixture-20261009T161330-light.json | publication | 2026-10-09T16:25:28.963Z | 2026-10-09T16:25:52.169Z | 23.206 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-receipt.json | publication | 2026-10-09T16:25:52.854Z | 2026-10-09T16:25:58.112Z | 5.258 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui.md | publication | 2026-10-09T16:25:52.858Z | 2026-10-09T16:26:03.340Z | 10.482 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-seal.json | publication | 2026-10-09T16:25:52.858Z | 2026-10-09T16:26:10.621Z | 17.763 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-source-manifest.json | publication | 2026-10-09T16:25:52.859Z | 2026-10-09T16:26:24.362Z | 31.503 | Failed denied initial call; authorized exact retry recorded separately |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-source-manifest.json | publication | 2026-10-09T16:27:19.156Z | 2026-10-09T16:27:23.849Z | 4.693 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/source-unchanged-final.json | publication | 2026-10-09T16:27:35.896Z | 2026-10-09T16:27:39.881Z | 3.985 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/loc-controls.json | publication | 2026-10-09T16:27:35.904Z | 2026-10-09T16:27:43.733Z | 7.829 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/independent-review.json | publication | 2026-10-09T16:27:35.904Z | 2026-10-09T16:27:47.489Z | 11.585 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/loc.json | publication | 2026-10-09T16:27:35.912Z | 2026-10-09T16:27:51.489Z | 15.577 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/swift-source-verification.json | publication | 2026-10-09T16:27:52.158Z | 2026-10-09T16:28:05.845Z | 13.687 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/verify-http-loc.py | publication | 2026-10-09T16:27:52.158Z | 2026-10-09T16:28:10.506Z | 18.348 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/documentation-evidence-supplement.json | publication | 2026-10-09T16:27:52.179Z | 2026-10-09T16:28:16.087Z | 23.908 | SHA/content verified |
| Create immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/verification.json | publication | 2026-10-09T16:27:52.179Z | 2026-10-09T16:28:21.037Z | 28.858 | SHA/content verified |
| Read back immutable HTTP blob: .github/workflows/rust-macos.yml | publication | 2026-10-09T16:26:50.558Z | 2026-10-09T16:26:51.253Z | 0.695 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/http_session/tests.rs | publication | 2026-10-09T16:28:43.556Z | 2026-10-09T16:28:43.891Z | 0.335 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/http_session.rs | publication | 2026-10-09T16:28:43.556Z | 2026-10-09T16:28:44.054Z | 0.498 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/desktop.rs | publication | 2026-10-09T16:28:43.556Z | 2026-10-09T16:28:43.978Z | 0.422 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/main.rs | publication | 2026-10-09T16:28:45.716Z | 2026-10-09T16:28:46.134Z | 0.418 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/http_ui/tests.rs | publication | 2026-10-09T16:28:45.716Z | 2026-10-09T16:28:46.190Z | 0.474 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/shutdown.rs | publication | 2026-10-09T16:28:45.716Z | 2026-10-09T16:28:46.217Z | 0.501 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/http_ui.rs | publication | 2026-10-09T16:28:45.716Z | 2026-10-09T16:28:46.432Z | 0.716 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/transport/http_request.rs | publication | 2026-10-09T16:28:47.505Z | 2026-10-09T16:28:47.852Z | 0.347 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/transport/http_request/tests.rs | publication | 2026-10-09T16:28:47.505Z | 2026-10-09T16:28:47.971Z | 0.466 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-app/src/transport.rs | publication | 2026-10-09T16:28:47.505Z | 2026-10-09T16:28:47.978Z | 0.473 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-core/src/developer.rs | publication | 2026-10-09T16:28:47.505Z | 2026-10-09T16:28:48.081Z | 0.576 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-core/src/developer/http_request_tests.rs | publication | 2026-10-09T16:28:50.196Z | 2026-10-09T16:28:50.546Z | 0.35 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-core/tests/fixtures/http-swift-characterization.json | publication | 2026-10-09T16:28:50.196Z | 2026-10-09T16:28:50.554Z | 0.358 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-core/src/developer/http_request.rs | publication | 2026-10-09T16:28:50.196Z | 2026-10-09T16:28:50.560Z | 0.364 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-core/tests/fixtures/http-swift-oracle.json | publication | 2026-10-09T16:28:50.196Z | 2026-10-09T16:28:50.706Z | 0.51 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/README.md | publication | 2026-10-09T16:28:51.670Z | 2026-10-09T16:28:52.009Z | 0.339 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/http-workflow.md | publication | 2026-10-09T16:28:51.670Z | 2026-10-09T16:28:52.039Z | 0.369 | SHA/content verified |
| Read back immutable HTTP blob: rust/crates/bellobox-core/tests/http_swift_oracle.rs | publication | 2026-10-09T16:28:51.670Z | 2026-10-09T16:28:52.071Z | 0.401 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/bellobox-parity.md | publication | 2026-10-09T16:28:51.670Z | 2026-10-09T16:28:52.530Z | 0.86 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/app-handoff.md | publication | 2026-10-09T16:28:54.066Z | 2026-10-09T16:28:54.406Z | 0.34 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/app-commands.json | publication | 2026-10-09T16:28:54.066Z | 2026-10-09T16:28:54.423Z | 0.357 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/compiled-source-manifest.json | publication | 2026-10-09T16:28:54.066Z | 2026-10-09T16:28:54.489Z | 0.423 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/changed-source.json | publication | 2026-10-09T16:28:54.066Z | 2026-10-09T16:28:54.531Z | 0.465 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/failure-notes.json | publication | 2026-10-09T16:28:55.392Z | 2026-10-09T16:28:55.712Z | 0.32 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-fixture-20261009T160053-light.json | publication | 2026-10-09T16:28:55.392Z | 2026-10-09T16:28:55.724Z | 0.332 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-fixture-20261009T160836-dark.json | publication | 2026-10-09T16:28:55.392Z | 2026-10-09T16:28:55.852Z | 0.46 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/core-checks.json | publication | 2026-10-09T16:28:55.392Z | 2026-10-09T16:28:55.889Z | 0.497 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-seal.json | publication | 2026-10-09T16:28:56.804Z | 2026-10-09T16:28:57.207Z | 0.403 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-fixture-20261009T161330-light.json | publication | 2026-10-09T16:28:56.804Z | 2026-10-09T16:28:57.246Z | 0.442 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-receipt.json | publication | 2026-10-09T16:28:56.804Z | 2026-10-09T16:28:57.259Z | 0.455 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui-source-manifest.json | publication | 2026-10-09T16:28:56.804Z | 2026-10-09T16:28:57.270Z | 0.466 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/loc-controls.json | publication | 2026-10-09T16:28:59.044Z | 2026-10-09T16:28:59.516Z | 0.472 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/independent-review.json | publication | 2026-10-09T16:28:59.044Z | 2026-10-09T16:28:59.538Z | 0.494 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/gui.md | publication | 2026-10-09T16:28:59.044Z | 2026-10-09T16:29:00.143Z | 1.099 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/loc.json | publication | 2026-10-09T16:28:59.044Z | 2026-10-09T16:28:59.604Z | 0.56 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/swift-source-verification.json | publication | 2026-10-09T16:29:01.043Z | 2026-10-09T16:29:01.389Z | 0.346 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/verify-http-loc.py | publication | 2026-10-09T16:29:01.043Z | 2026-10-09T16:29:01.398Z | 0.355 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/verification.json | publication | 2026-10-09T16:29:01.043Z | 2026-10-09T16:29:01.429Z | 0.386 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/source-unchanged-final.json | publication | 2026-10-09T16:29:01.043Z | 2026-10-09T16:29:01.531Z | 0.488 | SHA/content verified |
| Read back immutable HTTP blob: rust/docs/validation/http-workflow-2026-10-09/documentation-evidence-supplement.json | publication | 2026-10-09T16:29:02.340Z | 2026-10-09T16:29:02.652Z | 0.312 | SHA/content verified |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,776 items; 786 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

| Existing resource group | Counted timed items | Resource seconds | Items with endpoints | Endpoint-subset seconds | Subset interval union seconds |
|---|---:|---:|---:|---:|---:|
| ci_job | 168 | 73147.000 | 168 | 73147.000 | 45253.000 |
| local_receipt | 166 | 1389.032 | 156 | 1256.178 | 1256.241 |
| incremental_command | 6 | 249.417 | 6 | 249.417 | 249.417 |
| incremental_ci_job | 2 | 1441.000 | 2 | 1441.000 | 823.000 |
| new_local_command | 25 | 673.614 | 25 | 673.614 | 673.614 |
| new_reported_phase | 3 | 0.890 | 0 | 0.000 | unavailable |
| catchup_ci_job | 16 | 13969.000 | 16 | 13969.000 | 8292.000 |
| catchup_native_command | 11 | 4.661 | 11 | 4.661 | 4.661 |
| catchup_command | 242 | 4416.761 | 211 | 3673.135 | 3670.126 |
| catchup_gui_process | 9 | 2603.879 | 9 | 2603.879 | 2603.879 |
| catchup_api | 138 | 1602.990 | 138 | 1602.990 | 485.928 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
