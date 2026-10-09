# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T15:47:00Z

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
| Build + test/check (combined) | 26.4s measured command resource time |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | 16.5s across 4 failed process/API receipts; total rework effort unavailable |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 17 | 34.406 | 17 | 34.406 | 34.406 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: automated source edit | 0.021 |
| catchup_command: automated lint/check | 8.035 |
| catchup_command: build + test/check (combined) | 26.350 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T15:37:00Z–2026-10-09T15:47:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| HTTP core-wiring bounded source-edit operation | automated source edit | 2026-10-09T15:31:34.184531+00:00 | 2026-10-09T15:31:34.187578+00:00 | 0.0030475120001938194 | edit completed; execution validation separate |
| HTTP app-wiring bounded source-edit operation | automated source edit | 2026-10-09T15:39:54.860161+00:00 | 2026-10-09T15:39:54.861651+00:00 | 0.001489962000050582 | edit completed; execution validation separate |
| HTTP native-ci-wiring bounded source-edit operation | automated source edit | 2026-10-09T15:40:49.464626+00:00 | 2026-10-09T15:40:49.481051+00:00 | 0.016426991001935676 | edit completed; execution validation separate |
| clippy-initial | automated lint/check | 2026-10-09T15:34:54.653222+00:00 | 2026-10-09T15:35:01.826814+00:00 | 7.1735920906066895 | 0 |
| clippy-oracle-r2 | automated lint/check | 2026-10-09T15:39:51.303504+00:00 | 2026-10-09T15:39:51.599151+00:00 | 0.29564642906188965 | 0 |
| clippy-oracle | automated lint/check | 2026-10-09T15:39:38.090665+00:00 | 2026-10-09T15:39:38.481509+00:00 | 0.39084410667419434 | 101 |
| focused-initial | build + test/check (combined) | 2026-10-09T15:33:51.829231+00:00 | 2026-10-09T15:34:01.823865+00:00 | 9.99463415145874 | 101 |
| focused-r2 | build + test/check (combined) | 2026-10-09T15:34:45.236883+00:00 | 2026-10-09T15:34:54.652840+00:00 | 9.415956735610962 | 0 |
| format-initial | automated lint/check | 2026-10-09T15:33:51.769794+00:00 | 2026-10-09T15:33:51.828864+00:00 | 0.05906987190246582 | 0 |
| format-r2 | automated lint/check | 2026-10-09T15:34:45.204268+00:00 | 2026-10-09T15:34:45.236538+00:00 | 0.03226971626281738 | 0 |
| oracle-format-initial | automated lint/check | 2026-10-09T15:38:56.460004+00:00 | 2026-10-09T15:38:56.488106+00:00 | 0.028101205825805664 | 0 |
| oracle-format-r2 | automated lint/check | 2026-10-09T15:39:22.468536+00:00 | 2026-10-09T15:39:22.493051+00:00 | 0.02451467514038086 | 0 |
| oracle-format-r3 | automated lint/check | 2026-10-09T15:39:37.282360+00:00 | 2026-10-09T15:39:37.298280+00:00 | 0.01591968536376953 | 0 |
| oracle-format-r4 | automated lint/check | 2026-10-09T15:39:51.288356+00:00 | 2026-10-09T15:39:51.303105+00:00 | 0.014749765396118164 | 0 |
| oracle-portable-initial | build + test/check (combined) | 2026-10-09T15:38:56.489140+00:00 | 2026-10-09T15:39:02.222146+00:00 | 5.7330052852630615 | 101 |
| oracle-portable-r2 | build + test/check (combined) | 2026-10-09T15:39:22.493665+00:00 | 2026-10-09T15:39:22.908743+00:00 | 0.41507768630981445 | 101 |
| oracle-portable-r3 | build + test/check (combined) | 2026-10-09T15:39:37.298889+00:00 | 2026-10-09T15:39:38.090348+00:00 | 0.7914586067199707 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,660 items; 673 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 215 | 3517.543 | 184 | 2773.917 | 2771.863 |
| catchup_gui_process | 6 | 1737.635 | 6 | 1737.635 | 1737.635 |
| catchup_api | 55 | 968.019 | 55 | 968.019 | 216.248 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
