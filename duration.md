# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T17:23:00Z

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
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | 1m 28.6s measured API/client time; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 1 | 0.423 | 1 | 0.423 | 0.423 |
| catchup_api | 32 | 88.593 | 32 | 88.593 | 70.640 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: packaging/source validation | 0.423 |
| catchup_api: publication | 88.593 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T17:10:00Z–2026-10-09T17:23:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Remaining HTTP handoff source upload awaiting mandatory approval | unknown | unknown | unknown | Source package upload awaiting attached mandatory approval; completed operations are recorded separately; pending call duration/outcome unavailable |
| New local Regex palette handoff implementation and focused run | unknown | unknown | unknown | First source-bound focused Cargo run active at cutoff; no completed reliable command timer yet. Source-edit duration and initial formatter UTC start unavailable. |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Independent46-path HTTP package review | packaging/source validation | 2026-10-09T17:11:57.356819+00:00 | 2026-10-09T17:11:57.780281+00:00 | 0.42347022400645074 | all planned paths/hashes verified |
| Create completed HTTP handoff blob: rust/crates/bellobox-app/src/http_session.rs | publication | 2026-10-09T17:12:34.381Z | 2026-10-09T17:12:39.167Z | 4.786 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/crates/bellobox-app/src/http_session.rs | publication | 2026-10-09T17:12:42.612Z | 2026-10-09T17:12:42.964Z | 0.352 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/crates/bellobox-app/src/http_ui/tests.rs | publication | 2026-10-09T17:13:47.712Z | 2026-10-09T17:13:52.659Z | 4.947 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/crates/bellobox-app/src/http_ui/tests.rs | publication | 2026-10-09T17:13:52.659Z | 2026-10-09T17:13:53.071Z | 0.412 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/crates/bellobox-app/src/http_ui.rs | publication | 2026-10-09T17:13:47.978Z | 2026-10-09T17:13:56.774Z | 8.796 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/crates/bellobox-app/src/http_ui.rs | publication | 2026-10-09T17:13:56.775Z | 2026-10-09T17:13:57.200Z | 0.425 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/crates/bellobox-app/src/launcher_ui/copilot_lifecycle_tests.rs | publication | 2026-10-09T17:13:53.596Z | 2026-10-09T17:13:57.751Z | 4.155 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/crates/bellobox-app/src/launcher_ui/copilot_lifecycle_tests.rs | publication | 2026-10-09T17:13:57.751Z | 2026-10-09T17:13:58.210Z | 0.459 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/crates/bellobox-app/src/launcher_ui.rs | publication | 2026-10-09T17:13:58.646Z | 2026-10-09T17:14:02.917Z | 4.271 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/crates/bellobox-app/src/launcher_ui.rs | publication | 2026-10-09T17:14:02.917Z | 2026-10-09T17:14:03.490Z | 0.573 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/crates/bellobox-app/src/transport/http_request/tests.rs | publication | 2026-10-09T17:13:58.794Z | 2026-10-09T17:14:06.645Z | 7.851 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/crates/bellobox-app/src/transport/http_request/tests.rs | publication | 2026-10-09T17:14:06.645Z | 2026-10-09T17:14:09.052Z | 2.407 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/bellobox-parity.md | publication | 2026-10-09T17:14:04.846Z | 2026-10-09T17:14:08.568Z | 3.722 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/bellobox-parity.md | publication | 2026-10-09T17:14:08.568Z | 2026-10-09T17:14:09.775Z | 1.207 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/http-workflow.md | publication | 2026-10-09T17:14:09.601Z | 2026-10-09T17:14:14.673Z | 5.072 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/http-workflow.md | publication | 2026-10-09T17:14:14.673Z | 2026-10-09T17:14:15.098Z | 0.425 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/README.md | publication | 2026-10-09T17:14:15.194Z | 2026-10-09T17:14:19.695Z | 4.501 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/README.md | publication | 2026-10-09T17:14:19.695Z | 2026-10-09T17:14:20.155Z | 0.46 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/baseline-commit.json | publication | 2026-10-09T17:14:19.974Z | 2026-10-09T17:14:24.870Z | 4.896 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/baseline-commit.json | publication | 2026-10-09T17:14:24.870Z | 2026-10-09T17:14:25.240Z | 0.37 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/combined-source-manifest.json | publication | 2026-10-09T17:14:26.084Z | 2026-10-09T17:14:31.095Z | 5.011 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/combined-source-manifest.json | publication | 2026-10-09T17:14:31.096Z | 2026-10-09T17:14:31.475Z | 0.379 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/combined-verification.json | publication | 2026-10-09T17:14:31.984Z | 2026-10-09T17:14:35.925Z | 3.941 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/combined-verification.json | publication | 2026-10-09T17:14:35.926Z | 2026-10-09T17:14:36.440Z | 0.514 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/commands.json | publication | 2026-10-09T17:14:36.984Z | 2026-10-09T17:14:41.752Z | 4.768 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/commands.json | publication | 2026-10-09T17:14:41.753Z | 2026-10-09T17:14:42.255Z | 0.502 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/deadline-correction/base-verification.json | publication | 2026-10-09T17:14:42.786Z | 2026-10-09T17:14:47.148Z | 4.362 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/deadline-correction/base-verification.json | publication | 2026-10-09T17:14:47.149Z | 2026-10-09T17:14:47.595Z | 0.446 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/deadline-correction/candidate-source-manifest.json | publication | 2026-10-09T17:14:48.154Z | 2026-10-09T17:14:51.937Z | 3.783 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/deadline-correction/candidate-source-manifest.json | publication | 2026-10-09T17:14:51.937Z | 2026-10-09T17:14:52.420Z | 0.483 | Blob SHA/full-byte readback verified |
| Create completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/deadline-correction/commands.json | publication | 2026-10-09T17:14:53.030Z | 2026-10-09T17:14:57.021Z | 3.991 | Blob SHA/full-byte readback verified |
| Read back completed HTTP handoff blob: rust/docs/validation/http-palette-transfer-2026-10-09/deadline-correction/commands.json | publication | 2026-10-09T17:14:57.021Z | 2026-10-09T17:14:57.347Z | 0.326 | Blob SHA/full-byte readback verified |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,850 items; 854 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

| Existing resource group | Counted timed items | Resource seconds | Items with endpoints | Endpoint-subset seconds | Subset interval union seconds |
|---|---:|---:|---:|---:|---:|
| ci_job | 168 | 73147.000 | 168 | 73147.000 | 45253.000 |
| local_receipt | 166 | 1389.032 | 156 | 1256.178 | 1256.241 |
| incremental_command | 6 | 249.417 | 6 | 249.417 | 249.417 |
| incremental_ci_job | 2 | 1441.000 | 2 | 1441.000 | 823.000 |
| new_local_command | 25 | 673.614 | 25 | 673.614 | 673.614 |
| new_reported_phase | 3 | 0.890 | 0 | 0.000 | unavailable |
| catchup_ci_job | 18 | 15363.000 | 18 | 15363.000 | 9042.000 |
| catchup_native_command | 11 | 4.661 | 11 | 4.661 | 4.661 |
| catchup_command | 275 | 5654.838 | 243 | 4910.196 | 4907.187 |
| catchup_gui_process | 10 | 3228.786 | 10 | 3228.786 | 3228.786 |
| catchup_api | 170 | 1691.583 | 170 | 1691.583 | 556.568 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
