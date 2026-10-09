# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T16:47:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 0 mixed windows; 0 with endpoints, unavailable union; scopes overlap resources and do not measure Review alone |
| Builds | 17.0s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 4m 15.8s measured command resource time |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | 23m 14.0s runner time across 2 completed jobs (1 failed); 12m 30.0s wall union |
| Dependency/environment setup | 45.0s nested CI phase time (already inside CI jobs); command setup shown separately below |
| Retries/rework | 58.1s across 3 failed process/API receipts; total rework effort unavailable |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_ci_job | 2 | 1394.000 | 2 | 1394.000 | 750.000 |
| catchup_command | 10 | 297.325 | 10 | 297.325 | 297.325 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_ci_job: ci | 1394.000 |
| catchup_command: build + test/check (combined) | 255.807 |
| catchup_command: automated lint/check | 24.508 |
| catchup_command: build | 17.011 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|
| CI orchestration | 12.0s |
| dependency/environment setup | 45.0s |
| build + automated lint/check | 2m 55.0s |
| build/test/check (combined) | 18m 49.0s |
| build | 29.0s |
| CI reporting | 0.0s |
| packaging/validation | 0.0s |

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

Within 2026-10-09T16:37:00Z–2026-10-09T16:47:00Z, newly recorded CI jobs cover 368.000 overlap-safe seconds. Remaining time is unclassified, not proven idle or inference.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Same Session HTTP handoff GUI process still open | 2026-10-09T16:44:26Z | unknown | unknown | Active interactive session, not a completed interval yet |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Exact HTTP source Linux CI job | ci | 2026-10-09T16:30:38Z | 2026-10-09T16:41:27Z | 649.0 | success |
| Exact HTTP source macOS CI job | ci | 2026-10-09T16:30:43Z | 2026-10-09T16:43:08Z | 745.0 | failure |
| Same-session transfer focused-initial | build + test/check (combined) | 2026-10-09T16:37:53.870837+00:00 | 2026-10-09T16:37:58.898709+00:00 | 5.027873491999344 | 101 |
| Same-session transfer focused-r2 | build + test/check (combined) | 2026-10-09T16:38:47.200120+00:00 | 2026-10-09T16:39:31.698654+00:00 | 44.49853482299659 | 101 |
| Same-session transfer focused-r3 | build + test/check (combined) | 2026-10-09T16:39:59.304794+00:00 | 2026-10-09T16:40:42.502683+00:00 | 43.197891278992756 | 0 |
| Same-session transfer strict-r3 | automated lint/check | 2026-10-09T16:41:05.923696+00:00 | 2026-10-09T16:41:17.924159+00:00 | 12.000463593998575 | 0 |
| Same-session transfer minimal-check-r3 | build + test/check (combined) | 2026-10-09T16:41:17.961781+00:00 | 2026-10-09T16:41:23.306847+00:00 | 5.345070698007476 | 0 |
| Same-session transfer focused-r4 | build + test/check (combined) | 2026-10-09T16:42:10.662793+00:00 | 2026-10-09T16:42:19.232373+00:00 | 8.569581930991262 | 101 |
| Same-session transfer focused-r5 | build + test/check (combined) | 2026-10-09T16:42:50.035982+00:00 | 2026-10-09T16:43:30.818926+00:00 | 40.78294479500619 | 0 |
| Same-session transfer strict-r5 | automated lint/check | 2026-10-09T16:43:30.892655+00:00 | 2026-10-09T16:43:43.400089+00:00 | 12.507436194006004 | 0 |
| Same-session transfer ordinary-build-r5 | build | 2026-10-09T16:43:43.460348+00:00 | 2026-10-09T16:44:00.471041+00:00 | 17.010693751988583 | 0 |
| Same-session transfer aggregate-app | build + test/check (combined) | 2026-10-09T16:44:14.103808+00:00 | 2026-10-09T16:46:02.488533+00:00 | 108.384726416989 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,789 items; 798 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 252 | 4714.086 | 221 | 3970.460 | 3967.451 |
| catchup_gui_process | 9 | 2603.879 | 9 | 2603.879 | 2603.879 |
| catchup_api | 138 | 1602.990 | 138 | 1602.990 | 485.928 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
