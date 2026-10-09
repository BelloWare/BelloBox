# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T14:57:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 1 mixed windows; 1 with endpoints, 9m 24.0s union; scopes overlap resources and do not measure Review alone |
| Builds | Unavailable separately |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 3m 25.9s measured command resource time |
| Interactive GUI validation | 4m 42.5s observed process lifetime; overlaps workflow windows, limited acceptance only |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | 35.0s across 1 failed process/API receipts; total rework effort unavailable |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 4 | 206.252 | 4 | 206.252 | 206.252 |
| catchup_gui_process | 1 | 282.466 | 1 | 282.466 | 282.466 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 205.897 |
| catchup_command: dependency/environment recovery | 0.355 |
| catchup_gui_process: interactive GUI validation | 282.466 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T14:47:00Z–2026-10-09T14:57:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Regex aggregate gates, option GUI recheck, coordination and waiting | 2026-10-09T14:46:40Z | 2026-10-09T14:56:04Z | 564 | completed |
| Regex minimal and strict gate sequence | 2026-10-09T14:55:38Z | unknown | unknown | Sequence incomplete; initial minimal attempt failed from full overlay; recovery recorded and retry begins after cutoff |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| aggregate-app | build + test/check (combined) | 2026-10-09T14:46:26.968423+00:00 | 2026-10-09T14:47:51.856526+00:00 | 84.88810613600072 | 0 |
| aggregate-recording-app | build + test/check (combined) | 2026-10-09T14:47:51.880364+00:00 | 2026-10-09T14:49:17.870954+00:00 | 85.99059414799558 | 0 |
| minimal-core-app | build + test/check (combined) | 2026-10-09T14:55:41.841104+00:00 | 2026-10-09T14:56:16.859513+00:00 | 35.01841460200376 | Failed linker Bus error with overlay full; no tests executed |
| recover-box-clean | dependency/environment recovery | 2026-10-09T14:56:56.675035+00:00 | 2026-10-09T14:56:57.030137+00:00 | 0.3551059999881545 | Reproducible Box package outputs cleaned; retry remains separate |
| Regex option recheck GUI process lifetime | interactive GUI validation | 2026-10-09T14:49:57.189244833+00:00 | 2026-10-09T14:54:39.655462616+00:00 | 282.466218 | Normal exit0; pointer Multiline off/on observed, keyboard-toggle attempt remains inconclusive |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,572 items; 590 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 189 | 3252.612 | 158 | 2508.986 | 2506.932 |
| catchup_gui_process | 6 | 1737.635 | 6 | 1737.635 | 1737.635 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
