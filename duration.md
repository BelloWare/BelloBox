# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T13:57:00Z

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
| CI | 32m 11.0s runner time across 2 completed jobs (0 failed); 19m 16.0s wall union |
| Dependency/environment setup | 46.0s nested CI phase time (already inside CI jobs); command setup shown separately below |
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_ci_job | 2 | 1931.000 | 2 | 1931.000 | 1156.000 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_ci_job: CI runner | 1931.000 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|
| CI orchestration | 12.0s |
| dependency/environment setup | 46.0s |
| build/test/check (combined) | 22m 45.0s |
| build + automated lint/check | 6m 9.0s |
| build | 1m 31.0s |
| packaging/validation | 45.0s |
| CI reporting | 0.0s |

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

Within 2026-10-09T13:47:00Z–2026-10-09T13:57:00Z, newly recorded CI jobs cover 582.000 overlap-safe seconds. Remaining time is unclassified, not proven idle or inference.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Rust macOS native checks / apple-silicon | CI runner | 2026-10-09T13:37:30Z | 2026-10-09T13:56:42Z | 1152.0 | success |
| Rust Linux checks / linux | CI runner | 2026-10-09T13:37:26Z | 2026-10-09T13:50:25Z | 779.0 | success |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,536 items; 563 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 165 | 2796.460 | 134 | 2052.834 | 2050.780 |
| catchup_gui_process | 3 | 1001.905 | 3 | 1001.905 | 1001.905 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
