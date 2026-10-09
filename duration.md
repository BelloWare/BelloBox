# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T16:07:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 0 mixed windows; 0 with endpoints, unavailable union; scopes overlap resources and do not measure Review alone |
| Builds | 18.8s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 4m 20.7s measured command resource time |
| Build + negative-control tests | 1m 3.7s measured command resource time; expected caught failures are not implementation failures |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | Unavailable |
| Retries/rework | 1m 41.8s across 2 failed process/API receipts; total rework effort unavailable |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 18 | 375.381 | 18 | 375.381 | 375.381 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: automated lint/check | 32.109 |
| catchup_command: build + test/check (combined) | 260.710 |
| catchup_command: build + negative-control test (combined) | 63.738 |
| catchup_command: build | 18.825 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T15:47:00Z–2026-10-09T16:07:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| HTTP documentation and portable LOC/accounting reproduction | unknown | unknown | unknown | Verification completed; duration unavailable because no timing interval was recorded |
| HTTP App test source verification | unknown | unknown | unknown | Source verification recorded; no measured elapsed receipt |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| clippy-final-r2 | automated lint/check | 2026-10-09T15:49:00.669163+00:00 | 2026-10-09T15:49:07.224466+00:00 | 6.555302619934082 | 0 |
| focused-final-r2 | build + test/check (combined) | 2026-10-09T15:48:46.589848+00:00 | 2026-10-09T15:48:55.318656+00:00 | 8.728807926177979 | 0 |
| focused-final | build + test/check (combined) | 2026-10-09T15:48:25.754048+00:00 | 2026-10-09T15:48:26.044685+00:00 | 0.2906363010406494 | 101 |
| mutant-changed_decimal_cap | build + negative-control test (combined) | 2026-10-09T15:48:09.868448+00:00 | 2026-10-09T15:48:18.369221+00:00 | 8.500773191452026 | Expected negative-control failure; source restored |
| mutant-exact_cap_false_truncation | build + negative-control test (combined) | 2026-10-09T15:47:42.998567+00:00 | 2026-10-09T15:47:52.427241+00:00 | 9.42867374420166 | Expected negative-control failure; source restored |
| mutant-file_body_admission | build + negative-control test (combined) | 2026-10-09T15:47:24.801909+00:00 | 2026-10-09T15:47:33.980085+00:00 | 9.178175926208496 | Expected negative-control failure; source restored |
| mutant-framing_replay | build + negative-control test (combined) | 2026-10-09T15:47:14.626571+00:00 | 2026-10-09T15:47:24.801286+00:00 | 10.174714803695679 | Expected negative-control failure; source restored |
| mutant-json_fallback_discards_original | build + negative-control test (combined) | 2026-10-09T15:48:01.494468+00:00 | 2026-10-09T15:48:09.867666+00:00 | 8.37319827079773 | Expected negative-control failure; source restored |
| mutant-json_join_as_form | build + negative-control test (combined) | 2026-10-09T15:47:33.980698+00:00 | 2026-10-09T15:47:42.997978+00:00 | 9.017280101776123 | Expected negative-control failure; source restored |
| mutant-lossy_utf8_preview | build + negative-control test (combined) | 2026-10-09T15:47:52.428811+00:00 | 2026-10-09T15:48:01.493927+00:00 | 9.065116167068481 | Expected negative-control failure; source restored |
| oracle-portable-final-r2 | build + test/check (combined) | 2026-10-09T15:48:55.319016+00:00 | 2026-10-09T15:49:00.668831+00:00 | 5.3498148918151855 | 0 |
| owned-format-final-r2 | automated lint/check | 2026-10-09T15:48:46.556110+00:00 | 2026-10-09T15:48:46.589447+00:00 | 0.03333711624145508 | 0 |
| owned-format-final | automated lint/check | 2026-10-09T15:48:25.693290+00:00 | 2026-10-09T15:48:25.753648+00:00 | 0.06035780906677246 | 0 |
| focused-initial | build + test/check (combined) | 2026-10-09T15:49:50.307476+00:00 | 2026-10-09T15:51:31.866749+00:00 | 101.55927547400643 | 101 |
| focused-r2 | build + test/check (combined) | 2026-10-09T15:52:07.917593+00:00 | 2026-10-09T15:53:53.646299+00:00 | 105.72870890000195 | 0 |
| focused-final | build + test/check (combined) | 2026-10-09T15:58:43.276453+00:00 | 2026-10-09T15:59:22.328842+00:00 | 39.05239071599499 | 0 |
| strict-final | automated lint/check | 2026-10-09T15:59:22.408356+00:00 | 2026-10-09T15:59:47.867880+00:00 | 25.459525564001524 | 0 |
| ordinary-build | build | 2026-10-09T15:59:47.914916+00:00 | 2026-10-09T16:00:06.740224+00:00 | 18.825310714004445 | 0 |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,680 items; 691 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 233 | 3892.925 | 202 | 3149.299 | 3147.245 |
| catchup_gui_process | 6 | 1737.635 | 6 | 1737.635 | 1737.635 |
| catchup_api | 55 | 968.019 | 55 | 968.019 | 216.248 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
