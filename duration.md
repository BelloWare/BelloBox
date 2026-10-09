# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T17:43:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 0 mixed windows; 0 with endpoints, unavailable union; scopes overlap resources and do not measure Review alone |
| Builds | 16.2s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 5m 46.2s measured command resource time |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | 5.0s across 1 failed process/API receipts; total rework effort unavailable |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 10 | 414.612 | 10 | 414.612 | 414.611 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 346.179 |
| catchup_command: automated lint/check | 52.246 |
| catchup_command: build | 16.187 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T17:33:00Z–2026-10-09T17:43:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Regex palette default-app-r9 | build + test/check (combined) | 2026-10-09T17:34:03.860856+00:00 | 2026-10-09T17:36:26.827040+00:00 | 142.96626280999044 | 0 |
| Regex palette focused-r10 | build + test/check (combined) | 2026-10-09T17:40:19.647710+00:00 | 2026-10-09T17:41:03.439817+00:00 | 43.792205395991914 | 0 |
| Regex palette format-r9 | automated lint/check | 2026-10-09T17:39:27.677324+00:00 | 2026-10-09T17:39:29.607458+00:00 | 1.9302140030049486 | 0 |
| Regex palette ordinary-build-r9 | build | 2026-10-09T17:39:29.637896+00:00 | 2026-10-09T17:39:45.824797+00:00 | 16.186980092999875 | 0 |
| Regex palette recording-app-r9 | build + test/check (combined) | 2026-10-09T17:36:26.861088+00:00 | 2026-10-09T17:39:06.281348+00:00 | 159.42034018300183 | 0 |
| Regex palette strict-default-r10 | automated lint/check | 2026-10-09T17:41:29.879620+00:00 | 2026-10-09T17:41:42.852004+00:00 | 12.972467034007423 | 0 |
| Regex palette strict-default-r8 | automated lint/check | 2026-10-09T17:32:56.506381+00:00 | 2026-10-09T17:33:01.515854+00:00 | 5.009549725000397 | 101 |
| Regex palette strict-default-r9 | automated lint/check | 2026-10-09T17:33:52.775052+00:00 | 2026-10-09T17:34:03.819739+00:00 | 11.044762369987438 | 0 |
| Regex palette strict-minimal-r9 | automated lint/check | 2026-10-09T17:39:18.170113+00:00 | 2026-10-09T17:39:27.645231+00:00 | 9.47519594100595 | 0 |
| Regex palette strict-recording-r9 | automated lint/check | 2026-10-09T17:39:06.318996+00:00 | 2026-10-09T17:39:18.133219+00:00 | 11.814299122997909 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,868 items; 872 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 293 | 6304.079 | 261 | 5559.437 | 5556.427 |
| catchup_gui_process | 10 | 3228.786 | 10 | 3228.786 | 3228.786 |
| catchup_api | 170 | 1691.583 | 170 | 1691.583 | 556.568 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
