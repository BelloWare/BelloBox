# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T16:22:00Z

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
| Build + test/check (combined) | 8m 5.0s measured command resource time |
| Interactive GUI validation | 14m 26.2s observed process lifetime; overlaps workflow windows, limited acceptance only |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 9 | 523.836 | 9 | 523.836 | 522.881 |
| catchup_gui_process | 3 | 866.244 | 3 | 866.244 | 866.244 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 484.996 |
| catchup_command: automated lint/check | 37.430 |
| catchup_command: automated accounting validation | 0.955 |
| catchup_command: packaging/source validation | 0.456 |
| catchup_gui_process: interactive GUI validation | 866.244 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T16:07:00Z–2026-10-09T16:22:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| aggregate-core | build + test/check (combined) | 2026-10-09T16:10:22.381342+00:00 | 2026-10-09T16:10:41.337726+00:00 | 18.95638676499948 | 0 |
| aggregate-app | build + test/check (combined) | 2026-10-09T16:10:41.410809+00:00 | 2026-10-09T16:12:35.072478+00:00 | 113.66166974201042 | 0 |
| aggregate-recording-app | build + test/check (combined) | 2026-10-09T16:12:35.116258+00:00 | 2026-10-09T16:15:28.158264+00:00 | 173.04200750299788 | 0 |
| strict-recording | automated lint/check | 2026-10-09T16:15:28.202518+00:00 | 2026-10-09T16:15:47.871224+00:00 | 19.668708325989428 | 0 |
| aggregate-minimal | build + test/check (combined) | 2026-10-09T16:15:47.921469+00:00 | 2026-10-09T16:18:47.256967+00:00 | 179.3354997520073 | 0 |
| strict-minimal | automated lint/check | 2026-10-09T16:18:47.334826+00:00 | 2026-10-09T16:19:03.030092+00:00 | 15.695266601003823 | 0 |
| format-all | automated lint/check | 2026-10-09T16:19:03.077954+00:00 | 2026-10-09T16:19:05.143955+00:00 | 2.066003020008793 | 0 |
| HTTP interactive GUI 20261009T160053-light | interactive GUI validation | 2026-10-09T16:00:53.374808187+00:00 | 2026-10-09T16:08:11.656642558+00:00 | 438.281834 | exit=0; scoped local fixture interactions completed |
| HTTP interactive GUI 20261009T160836-dark | interactive GUI validation | 2026-10-09T16:08:36.550889828+00:00 | 2026-10-09T16:11:32.098727953+00:00 | 175.547838 | exit=0; scoped local fixture interactions completed |
| HTTP interactive GUI 20261009T161330-light | interactive GUI validation | 2026-10-09T16:13:30.613081454+00:00 | 2026-10-09T16:17:43.027655590+00:00 | 252.414574 | exit=0; scoped local fixture interactions completed |
| Independent root HTTP LOC reproduction | automated accounting validation | 2026-10-09T16:14:44.516455+00:00 | 2026-10-09T16:14:45.471284+00:00 | 0.9548299419984687 | 0 |
| Root HTTP publication package verification | packaging/source validation | 2026-10-09T16:21:53.149831+00:00 | 2026-10-09T16:21:53.605522+00:00 | 0.45570019200386014 | passed;41 paths and all hashes verified |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,692 items; 703 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_api | 55 | 968.019 | 55 | 968.019 | 216.248 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
