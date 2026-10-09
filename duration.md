# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T14:37:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 0 mixed windows; 0 with endpoints, unavailable union; scopes overlap resources and do not measure Review alone |
| Builds | 25.6s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 1m 32.8s measured command resource time |
| Build + negative-control tests | 24.2s measured command resource time; expected caught failures are not implementation failures |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | 0.1s measured command resource time |
| Retries/rework | 12.1s across 1 failed command receipts; total rework effort unavailable |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 12 | 172.628 | 12 | 172.628 | 172.628 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: automated source/binary verification | 0.533 |
| catchup_command: build + test/check (combined) | 92.792 |
| catchup_command: build + negative-control test (combined) | 24.187 |
| catchup_command: dependency/environment or verification | 0.143 |
| catchup_command: automated lint/check | 29.418 |
| catchup_command: build | 25.555 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T14:27:00Z–2026-10-09T14:37:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Independent Regex source and sealed binary hash verification | automated source/binary verification | 2026-10-09T14:32:57.288493+00:00 | 2026-10-09T14:32:57.821323+00:00 | 0.532835828998941 | passed;254 source entries and binary match |
| focused-app-r2 | build + test/check (combined) | 2026-10-09T14:26:39.641718+00:00 | 2026-10-09T14:27:26.288341+00:00 | 46.64662687499367 | 0 |
| focused-core-r3 | build + test/check (combined) | 2026-10-09T14:27:26.320273+00:00 | 2026-10-09T14:27:41.879118+00:00 | 15.55884838399652 | 0 |
| mutant-utf16_as_bytes | build + negative-control test (combined) | 2026-10-09T14:28:02.478856+00:00 | 2026-10-09T14:28:10.839977+00:00 | 8.361125171999447 | Expected negative-control assertion failure; source restored |
| mutant-suppress_adjacent_empty | build + negative-control test (combined) | 2026-10-09T14:28:10.862989+00:00 | 2026-10-09T14:28:18.455827+00:00 | 7.592841737001436 | Expected negative-control assertion failure; source restored |
| mutant-expand_past_output_bound | build + negative-control test (combined) | 2026-10-09T14:28:18.482951+00:00 | 2026-10-09T14:28:26.716271+00:00 | 8.233324703993276 | Expected negative-control assertion failure; source restored |
| restore-core-clean | dependency/environment or verification | 2026-10-09T14:28:26.754856+00:00 | 2026-10-09T14:28:26.898270+00:00 | 0.1434181499935221 | 0 |
| final-format | automated lint/check | 2026-10-09T14:28:26.925871+00:00 | 2026-10-09T14:28:28.981355+00:00 | 2.0554890720086405 | 0 |
| final-core | build + test/check (combined) | 2026-10-09T14:28:29.004365+00:00 | 2026-10-09T14:28:59.590775+00:00 | 30.586413594006444 | 0 |
| final-strict | automated lint/check | 2026-10-09T14:28:59.615855+00:00 | 2026-10-09T14:29:11.728859+00:00 | 12.113008695989265 | 101 |
| final-strict-r2 | automated lint/check | 2026-10-09T14:29:28.614814+00:00 | 2026-10-09T14:29:43.864533+00:00 | 15.24972291001177 | 0 |
| ordinary-build | build | 2026-10-09T14:29:43.898304+00:00 | 2026-10-09T14:30:09.453106+00:00 | 25.55480681300105 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,556 items; 579 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 181 | 3035.655 | 150 | 2292.029 | 2289.974 |
| catchup_gui_process | 3 | 1001.905 | 3 | 1001.905 | 1001.905 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
