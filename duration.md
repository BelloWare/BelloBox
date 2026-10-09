# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T17:10:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains in the complete ledger and SHA-pinned historical view linked below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; no isolated implementation timer |
| Review | Active effort unavailable; only review-focused observations: 1 mixed windows; 1 with endpoints, 1m 32.4s union |
| Mixed implementation/review/validation windows | 0 mixed windows; 0 with endpoints, unavailable union; scopes overlap resources and do not measure Review alone |
| Builds | 18.4s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 5m 53.3s measured command resource time |
| Interactive GUI validation | No new completed GUI process receipt |
| CI | No new completed job duration in this cohort; prior terminal jobs remain in the ledger and linked history |
| Dependency/environment setup | 154.292ms measured command resource time |
| Retries/rework | Unavailable separately; retained successful checks do not establish zero rework |
| Publication | No isolated API total; 0 mixed windows; 0 with endpoints, unavailable union |
| Waiting | 1 mixed windows; 1 with endpoints, 8m 10.7s union shared-build-lane constraint; overlaps useful work, not proven idle |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 13 | 424.889 | 12 | 423.874 | 423.874 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 353.279 |
| catchup_command: automated lint/check | 52.050 |
| catchup_command: dependency/environment or verification | 0.154 |
| catchup_command: build | 18.390 |
| catchup_command: automated accounting validation | 1.015 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

This cohort adds no CI job execution intervals; prior verified terminal runs remain in earlier accounting. Coverage of 2026-10-09T16:57:00Z–2026-10-09T17:10:00Z is partial and does not establish an idle-time or inference budget.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Box shared Cargo lane lower-bound wait | 2026-10-09T16:55:37.242920+00:00 | 2026-10-09T17:03:47.975975+00:00 | 490.733055 | completed |
| Independent HTTP deadline source/receipt review | 2026-10-09T17:00:42Z | 2026-10-09T17:02:14.399153+00:00 | 92.399153 | completed |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in preserved earlier accounting. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| HTTP deadline fixture focused-restored | build + test/check (combined) | 2026-10-09T16:56:31.915315+00:00 | 2026-10-09T16:57:10.762071+00:00 | 38.846757487001014 | 0 |
| HTTP deadline fixture strict-recording | automated lint/check | 2026-10-09T16:57:10.846747+00:00 | 2026-10-09T16:57:23.422561+00:00 | 12.575815147007233 | 0 |
| HTTP deadline fixture format-all | automated lint/check | 2026-10-09T16:57:23.510495+00:00 | 2026-10-09T16:57:25.682302+00:00 | 2.1718070190108847 | 0 |
| Same-session transfer combined-package-clean | dependency/environment or verification | 2026-10-09T17:03:47.975975+00:00 | 2026-10-09T17:03:48.130265+00:00 | 0.1542917680053506 | 0 |
| Same-session transfer combined-focused | build + test/check (combined) | 2026-10-09T17:03:48.170920+00:00 | 2026-10-09T17:04:32.447949+00:00 | 44.277030999001 | 0 |
| Same-session transfer combined-default | build + test/check (combined) | 2026-10-09T17:04:32.533240+00:00 | 2026-10-09T17:06:14.732989+00:00 | 102.1997510150104 | 0 |
| Same-session transfer combined-strict-default | automated lint/check | 2026-10-09T17:06:14.773604+00:00 | 2026-10-09T17:06:26.364271+00:00 | 11.59067013800086 | 0 |
| Same-session transfer combined-recording | build + test/check (combined) | 2026-10-09T17:06:26.432969+00:00 | 2026-10-09T17:09:14.388450+00:00 | 167.95548211400455 | 0 |
| Same-session transfer combined-strict-recording | automated lint/check | 2026-10-09T17:09:14.431901+00:00 | 2026-10-09T17:09:28.060054+00:00 | 13.628155106998747 | 0 |
| Same-session transfer combined-strict-minimal | automated lint/check | 2026-10-09T17:09:28.100757+00:00 | 2026-10-09T17:09:38.123730+00:00 | 10.022975394997047 | 0 |
| Same-session transfer combined-format | automated lint/check | 2026-10-09T17:09:38.165264+00:00 | 2026-10-09T17:09:40.226119+00:00 | 2.060858198994538 | 0 |
| Same-session transfer combined-ordinary-build | build | 2026-10-09T17:09:40.266264+00:00 | 2026-10-09T17:09:58.656581+00:00 | 18.39032031300303 | 0 |
| Independent combined HTTP LOC reproduction | automated accounting validation | unknown | unknown | 1.015225046 | all baseline blobs/tree objects and five negative controls passed; 67666 production /51013 support /105 benchmark |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.


## All recorded resource groups

The ledger retains 3,815 items; 821 are flagged for their own resource-group totals. These are all recorded observations, not a complete migration budget. Groups retain their existing definitions and checkpoint-era names; they must not be added into one elapsed or effort total. Mixed work windows and nested phases remain excluded. Missing endpoints make some interval unions unavailable.

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
| catchup_command | 274 | 5654.414 | 242 | 4909.773 | 4906.764 |
| catchup_gui_process | 10 | 3228.786 | 10 | 3228.786 | 3228.786 |
| catchup_api | 138 | 1602.990 | 138 | 1602.990 | 485.928 |

## Complete detail and immutable history

- [Complete per-item timing ledger, sources and checkpoint summaries](duration-data.json).
- [Full historical report through 13:37 UTC](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration.md).
- [Ledger paired with that immutable report](https://github.com/BelloWare/BelloBox/blob/60a3367c3e8ae97fffb77a5cd1eb42610844164e/duration-data.json).

The current page is a compact view. The ledger retains every historical item unchanged; earlier rendered reports remain available at their original Git commits. Inference timing is unavailable. Resource sums, overlap-safe endpoint subsets and mixed workflow windows answer different questions; none is a complete time budget.
