# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T16:57:00Z

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
| Build + test/check (combined) | 6m 22.0s measured command resource time |
| Build + negative-control tests | 1m 46.6s measured command resource time; expected caught failures are not implementation failures |
| Interactive GUI validation | 10m 24.9s observed process lifetime; overlaps workflow windows, limited acceptance only |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | 377.112ms measured command resource time |
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | 1 mixed windows; 0 with endpoints, unavailable union shared-build-lane constraint; overlaps useful work, not proven idle |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 9 | 515.439 | 9 | 515.439 | 515.439 |
| catchup_gui_process | 1 | 624.907 | 1 | 624.907 | 624.907 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 382.028 |
| catchup_command: automated lint/check | 26.462 |
| catchup_command: build + negative-control test (combined) | 106.572 |
| catchup_command: dependency/environment or verification | 0.377 |
| catchup_gui_process: interactive GUI validation | 624.907 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T16:47:00Z–2026-10-09T16:57:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Observed shared Cargo lane queue: combined compact HTTP transfer plus deadline test repair | 2026-10-09T16:55:37.242920+00:00 | unknown | unknown | Open lower-bound queue observation; earlier entry time unknown |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Same-session transfer aggregate-recording | build + test/check (combined) | 2026-10-09T16:46:02.530635+00:00 | 2026-10-09T16:49:03.558769+00:00 | 181.02813662600238 | 0 |
| Same-session transfer strict-recording | automated lint/check | 2026-10-09T16:49:03.638613+00:00 | 2026-10-09T16:49:16.635999+00:00 | 12.997388689007494 | 0 |
| Same-session transfer aggregate-minimal | build + test/check (combined) | 2026-10-09T16:49:16.722797+00:00 | 2026-10-09T16:51:39.996691+00:00 | 143.27389436100202 | 0 |
| Same-session transfer strict-minimal | automated lint/check | 2026-10-09T16:51:40.045785+00:00 | 2026-10-09T16:51:51.297433+00:00 | 11.251651161001064 | 0 |
| Same-session transfer format-all | automated lint/check | 2026-10-09T16:51:51.379685+00:00 | 2026-10-09T16:51:53.592225+00:00 | 2.2125404160033213 | 0 |
| Same Session HTTP handoff interactive GUI process | interactive GUI validation | 2026-10-09T16:44:26.295882+00:00 | 2026-10-09T16:54:51.203139+00:00 | 624.907257 | Normal exit0; scoped cloud GUI process completed |
| HTTP deadline fixture focused-initial | build + test/check (combined) | 2026-10-09T16:52:33.515599+00:00 | 2026-10-09T16:53:31.241335+00:00 | 57.72573737199127 | 0 |
| HTTP deadline fixture mutant-total | build + negative-control test (combined) | 2026-10-09T16:53:46.700821+00:00 | 2026-10-09T16:54:54.165085+00:00 | 67.46426531099132 | Expected negative-control failure; source restored |
| HTTP deadline fixture mutant-read | build + negative-control test (combined) | 2026-10-09T16:55:22.119149+00:00 | 2026-10-09T16:56:01.227233+00:00 | 39.1080853940075 | Expected negative-control failure; source restored |
| HTTP deadline fixture package-clean-restored | dependency/environment or verification | 2026-10-09T16:56:31.489725+00:00 | 2026-10-09T16:56:31.866836+00:00 | 0.37711248599225655 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,800 items; 808 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 261 | 5229.525 | 230 | 4485.899 | 4482.890 |
| catchup_gui_process | 10 | 3228.786 | 10 | 3228.786 | 3228.786 |
| catchup_api | 138 | 1602.990 | 138 | 1602.990 | 485.928 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
