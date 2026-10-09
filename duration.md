# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T14:47:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 2 mixed windows; 2 with endpoints, 20m 0.0s union; scopes overlap resources and do not measure Review alone |
| Builds | Unavailable separately |
| Tests | 0.0s measured execution of existing binaries; no compilation included |
| Build + test/check (combined) | Unavailable |
| Interactive GUI validation | 7m 33.3s observed process lifetime; overlaps workflow windows, limited acceptance only |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_gui_process | 2 | 453.263 | 2 | 453.263 | 453.263 |
| catchup_command | 4 | 10.705 | 4 | 10.705 | 10.705 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_gui_process: interactive GUI validation | 453.263 |
| catchup_command: automated accounting validation | 2.138 |
| catchup_command: remote repository verification | 8.531 |
| catchup_command: test execution (existing binary) | 0.035 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T14:37:00Z–2026-10-09T14:47:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Regex commands, GUI, review, coordination and waits | 2026-10-09T14:26:40Z | 2026-10-09T14:40:42Z | 842 | completed |
| Regex docs, support tests, packaging preparation, review and lane waiting | 2026-10-09T14:40:42Z | 2026-10-09T14:46:40Z | 358 | completed |
| Regex aggregate App gate | 2026-10-09T14:46:27Z | unknown | unknown | Open at cutoff; final duration/result unavailable |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Regex light GUI process lifetime | interactive GUI validation | 2026-10-09T14:31:32.339079327+00:00 | 2026-10-09T14:37:27.230341271+00:00 | 354.891262 | Normal exit0; scoped cloud GUI checks observed, Multiline off-state and real IME unverified |
| Regex dark GUI process lifetime | interactive GUI validation | 2026-10-09T14:37:37.575550665+00:00 | 2026-10-09T14:39:15.947468346+00:00 | 98.371918 | Normal exit0; scoped cloud GUI checks observed, Multiline off-state and real IME unverified |
| Independent Regex LOC reproduction | automated accounting validation | 2026-10-09T14:44:37.594138+00:00 | 2026-10-09T14:44:38.696822+00:00 | 1.1026853259972995 | passed; full ledger match |
| remote-final | remote repository verification | 2026-10-09T14:42:15.636798+00:00 | 2026-10-09T14:42:24.168247+00:00 | 8.531452898998396 | 0 |
| loc-audit-final | automated accounting validation | 2026-10-09T14:42:24.195952+00:00 | 2026-10-09T14:42:25.231466+00:00 | 1.035517135009286 | 0 |
| sealed-cli-regression | test execution (existing binary) | 2026-10-09T14:45:27.147325+00:00 | 2026-10-09T14:45:27.182745+00:00 | 0.035423687993898056 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,565 items; 585 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 185 | 3046.360 | 154 | 2302.734 | 2300.680 |
| catchup_gui_process | 5 | 1455.169 | 5 | 1455.169 | 1455.169 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
