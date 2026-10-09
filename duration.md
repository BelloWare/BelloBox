# BelloBox Rust migration duration audit

## Current accounting checkpoint: 2026-10-09T12:40:00Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains preserved below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; mixed source/test windows recorded below |
| Review | Active effort unavailable; only review-focused observations: 0 mixed windows; 0 with endpoints, unavailable union |
| Mixed implementation/review/validation windows | 3 mixed windows; 3 with endpoints, 30m 52.0s union; scopes overlap resources and do not measure Review alone |
| Builds | 1m 2.9s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 8m 43.7s measured command resource time |
| CI | 22m 27.0s runner time across 2 completed jobs (1 failed); 13m 56.0s wall union |
| Dependency/environment setup | 1.7s measured resource time |
| Retries/rework | 25.0s across 4 failed command receipts; total rework effort unavailable |
| Publication | No isolated API total; 2 mixed windows; 2 with endpoints, 1m 0.0s union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_command | 46 | 657.362 | 43 | 649.398 | 648.355 |
| catchup_ci_job | 2 | 1347.000 | 2 | 1347.000 | 836.000 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_command: build + test/check (combined) | 523.689 |
| catchup_command: automated lint/check | 69.007 |
| catchup_command: build | 62.934 |
| catchup_command: dependency/environment or verification | 1.732 |
| catchup_ci_job: CI runner | 1347.000 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|
| CI orchestration | 13.0s |
| dependency/environment setup | 1m 10.0s |
| build/test/check (combined) | 16m 23.0s |
| build + automated lint/check | 3m 56.0s |
| build | 38.0s |
| packaging/validation | 0.0s |
| CI reporting | 0.0s |

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

Within 2026-10-09T12:20:16Z–2026-10-09T12:40:00Z, the selected CI jobs cover 558.000 overlap-safe wall seconds; 626.000 seconds are outside those jobs. This remainder includes implementation, tests, review, publication, waiting and unknown time; it is neither proven idle nor model inference.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| Publish Box catch-up timing checkpoint | 2026-10-09T12:32:38Z | 2026-10-09T12:33:06Z | 28 | completed |
| Comparison actual GUI acceptance | 2026-10-09T12:38:19.178644258+00:00 | unknown | unknown | ongoing |
| Source reading, app implementation and tests, review, delegation, overlapping Core work; not pure editing/inference | 2026-10-09T12:08:08+00:00 | 2026-10-09T12:22:08.401820+00:00 | 840.40182 | completed |
| App compile/test, Swift runtime alignment correction, review fixes, mutations, final feature matrix; commands nested, do not sum with wall interval | 2026-10-09T12:22:08.401820+00:00 | 2026-10-09T12:39:00+00:00 | 1011.59818 | completed |
| Comparison workflow implementation remains in progress | 2026-10-09T12:08:08+00:00 | unknown | unknown | ongoing, not a completion receipt |
| Prepare source-bound comparison oracle fixture and native driver | 2026-10-09T12:08:35Z | 2026-10-09T12:11:17Z | 162 | completed |
| Publish URL native fixture-ID support correction | 2026-10-09T12:30:55Z | 2026-10-09T12:31:27Z | 32 | completed |
| Rust macOS native checks / apple-silicon | 2026-10-09T12:31:35Z | unknown | unknown | in_progress |
| Rust Linux checks / linux | 2026-10-09T12:31:31Z | unknown | unknown | in_progress |

Open task and CI rows retain unknown final duration. Failed source attempts and the original failed native URL job remain in this cohort. Mixed windows overlap useful parallel work; they are not pure idle or active-review time.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| import-published-base | build + test/check (combined) | 2026-10-09T12:17:24.854407+00:00 | 2026-10-09T12:17:33.527596+00:00 | 8.673 | 0 |
| app-format | automated lint/check | 2026-10-09T12:18:00.268427+00:00 | 2026-10-09T12:18:00.419103+00:00 | 0.151 | 0 |
| app-first-compile | build | 2026-10-09T12:22:08.401820+00:00 | 2026-10-09T12:22:45.105512+00:00 | 36.704 | 0 |
| app-initial-session-tests | build + test/check (combined) | 2026-10-09T12:23:04.378611+00:00 | 2026-10-09T12:23:04.406349+00:00 | 0.028 | 0 |
| app-initial-ui-tests | build + test/check (combined) | 2026-10-09T12:23:04.429956+00:00 | 2026-10-09T12:23:04.826785+00:00 | 0.397 | 0 |
| app-focused-final-engine | build + test/check (combined) | 2026-10-09T12:27:17.924058+00:00 | 2026-10-09T12:27:53.658858+00:00 | 35.735 | 0 |
| app-strict-initial | automated lint/check | 2026-10-09T12:28:15.148966+00:00 | 2026-10-09T12:28:24.183872+00:00 | 9.035 | 101 |
| app-strict | automated lint/check | 2026-10-09T12:28:35.471122+00:00 | 2026-10-09T12:28:42.837841+00:00 | 7.367 | 0 |
| mutant-empty-second-admission | build + test/check (combined) | 2026-10-09T12:29:01.689069+00:00 | 2026-10-09T12:29:28.360118+00:00 | 26.671 | Expected negative-control failure; source restored |
| restore-clean-empty-second-admission | dependency/environment or verification | 2026-10-09T12:29:28.389575+00:00 | 2026-10-09T12:29:28.713977+00:00 | 0.324 | 0 |
| mutant-stale-copy-token | build + test/check (combined) | 2026-10-09T12:29:28.741398+00:00 | 2026-10-09T12:30:01.527570+00:00 | 32.786 | Expected negative-control failure; source restored |
| restore-clean-stale-copy-token | dependency/environment or verification | 2026-10-09T12:30:01.557975+00:00 | 2026-10-09T12:30:01.885274+00:00 | 0.327 | 0 |
| mutant-stale-generation-publication | build + test/check (combined) | 2026-10-09T12:30:01.915293+00:00 | 2026-10-09T12:30:33.638442+00:00 | 31.723 | Expected negative-control failure; source restored |
| restore-clean-stale-generation-publication | dependency/environment or verification | 2026-10-09T12:30:33.662743+00:00 | 2026-10-09T12:30:33.981840+00:00 | 0.319 | 0 |
| mutant-physical-worker-guard | build + test/check (combined) | 2026-10-09T12:30:34.007015+00:00 | 2026-10-09T12:31:04.243528+00:00 | 30.237 | Expected negative-control failure; source restored |
| restore-clean-physical-worker-guard | dependency/environment or verification | 2026-10-09T12:31:04.286057+00:00 | 2026-10-09T12:31:04.652270+00:00 | 0.366 | 0 |
| mutant-final-empty-line | build + test/check (combined) | 2026-10-09T12:31:04.679632+00:00 | 2026-10-09T12:31:12.226407+00:00 | 7.547 | Expected negative-control failure; source restored |
| restore-clean-final-empty-line | dependency/environment or verification | 2026-10-09T12:31:12.259277+00:00 | 2026-10-09T12:31:12.401159+00:00 | 0.142 | 0 |
| mutant-canonical-match | build + test/check (combined) | 2026-10-09T12:31:12.425639+00:00 | 2026-10-09T12:31:19.900472+00:00 | 7.475 | Expected negative-control failure; source restored |
| restore-clean-canonical-match | dependency/environment or verification | 2026-10-09T12:31:19.928817+00:00 | 2026-10-09T12:31:20.049864+00:00 | 0.121 | 0 |
| final-format | automated lint/check | 2026-10-09T12:31:53.117679+00:00 | 2026-10-09T12:31:57.101770+00:00 | 3.984 | 0 |
| loc-audit | build + test/check (combined) | 2026-10-09T12:32:12.833988+00:00 | 2026-10-09T12:32:13.875427+00:00 | 1.041 | 0 |
| core-default | build + test/check (combined) | 2026-10-09T12:31:57.130832+00:00 | 2026-10-09T12:32:18.827751+00:00 | 21.697 | 0 |
| core-minimal | build + test/check (combined) | 2026-10-09T12:32:18.855401+00:00 | 2026-10-09T12:32:37.507508+00:00 | 18.652 | 0 |
| app-default | build + test/check (combined) | 2026-10-09T12:32:37.532506+00:00 | 2026-10-09T12:33:50.249329+00:00 | 72.717 | 0 |
| app-minimal | build + test/check (combined) | 2026-10-09T12:33:50.275982+00:00 | 2026-10-09T12:35:01.188291+00:00 | 70.912 | 0 |
| app-recording | build + test/check (combined) | 2026-10-09T12:35:01.212920+00:00 | 2026-10-09T12:36:18.505825+00:00 | 77.293 | 0 |
| editor-core | build + test/check (combined) | 2026-10-09T12:36:18.538663+00:00 | 2026-10-09T12:36:20.554069+00:00 | 2.015 | 0 |
| editor-ui | build + test/check (combined) | 2026-10-09T12:36:20.586290+00:00 | 2026-10-09T12:36:28.591054+00:00 | 8.005 | 0 |
| strict-default | automated lint/check | 2026-10-09T12:36:28.617060+00:00 | 2026-10-09T12:36:41.453008+00:00 | 12.836 | 0 |
| strict-minimal | automated lint/check | 2026-10-09T12:36:41.478299+00:00 | 2026-10-09T12:36:51.969986+00:00 | 10.492 | 0 |
| strict-recording | automated lint/check | 2026-10-09T12:36:51.995474+00:00 | 2026-10-09T12:37:01.258045+00:00 | 9.263 | 0 |
| ordinary-clean | dependency/environment or verification | 2026-10-09T12:37:01.283185+00:00 | 2026-10-09T12:37:01.415936+00:00 | 0.133 | 0 |
| ordinary-build | build | 2026-10-09T12:37:01.439488+00:00 | 2026-10-09T12:37:15.669364+00:00 | 14.23 | 0 |
| focused initial compile | build | 2026-10-09T12:19:07Z | 2026-10-09T12:19:15Z | 8.0 | failed standalone formatter integration seam |
| focused Core tests | build + test/check (combined) | 2026-10-09T12:19:47Z | 2026-10-09T12:20:00Z | 13.0 | 12 passed |
| full Core compile | build | 2026-10-09T12:20:33Z | 2026-10-09T12:20:37Z | 4.0 | failed local edit script; source restored |
| full Core tests provisional runtime alignment | build + test/check (combined) | 2026-10-09T12:21:23Z | 2026-10-09T12:21:45Z | 22.0 | 528 passed,3 native ignored |
| Core Clippy provisional runtime alignment | automated lint/check | 2026-10-09T12:21:45Z | 2026-10-09T12:21:49Z | 4.0 | passed |
| corrected6.3.3 focused Core tests | build + test/check (combined) | 2026-10-09T12:25:07Z | 2026-10-09T12:25:17Z | 10.0 | 12 passed |
| corrected6.3.3 Core Clippy | automated lint/check | 2026-10-09T12:25:17Z | 2026-10-09T12:25:21Z | 4.0 | failed style lint; corrected |
| corrected6.3.3 full Core tests | build + test/check (combined) | 2026-10-09T12:25:48Z | 2026-10-09T12:26:09Z | 21.0 | 528 passed,3 native ignored |
| corrected6.3.3 final Core Clippy | automated lint/check | 2026-10-09T12:26:09Z | 2026-10-09T12:26:13Z | 4.0 | passed |
| test | build + test/check (combined) | unknown | unknown | 4.085 | 0 |
| strict | automated lint/check | unknown | unknown | 2.146 | 0 |
| fmt | automated lint/check | unknown | unknown | 1.733 | 0 |
| Rust macOS native checks / apple-silicon | CI runner | 2026-10-09T12:15:46Z | 2026-10-09T12:24:17Z | 511.0 | failure |
| Rust Linux checks / linux | CI runner | 2026-10-09T12:15:38Z | 2026-10-09T12:29:34Z | 836.0 | success |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.

### Earlier accounting (unchanged)

## Current accounting checkpoint: 2026-10-09T12:20:16Z

This catch-up incorporates selected verified receipts through the stated cutoff, including late-added earlier observations. Every earlier item and checkpoint remains preserved below. It is not a complete timesheet. Model inference duration remains unavailable, not zero. Shared coordination/publication appears once; local receipt hashes establish provenance without claiming independent public timing verification.

### At a glance

These top totals cover only this catch-up receipt cohort, including late-added earlier observations; they are not whole-migration cumulative totals. These are overlapping accounting views, not shares of one total. Mixed windows do not measure active labor.

| Where time went | What is actually measured |
|---|---|
| Implementation | Active effort unavailable; mixed source/test windows recorded below |
| Review | Active effort unavailable; only review-focused observations: 1 mixed windows; 1 with endpoints, 49.7s union |
| Mixed implementation/review/validation windows | 4 mixed windows; 4 with endpoints, 39m 29.5s union; scopes overlap resources and do not measure Review alone |
| Builds | 1m 59.9s measured command resource time |
| Tests | Unavailable separately from compilation in these command receipts |
| Build + test/check (combined) | 22m 58.4s measured command resource time |
| CI | 1h 55m 26.0s runner time across 8 jobs; 1h 8m 40.0s wall union |
| Dependency/environment setup | 5.3s measured resource time |
| Retries/rework | 3m 34.9s across 11 failed command receipts; total rework effort unavailable |
| Publication | No isolated API total; 6 mixed windows; 6 with endpoints, 16m 31.9s union |
| Waiting | Unavailable separately; waiting is mixed into recorded workflow windows |
| Model inference | Unavailable; no timing telemetry |

Later status, outside these totals (observed12:27UTC): [URL source fd434 macOS CI](https://github.com/BelloWare/BelloBox/actions/runs/37928860545) failed. Native URL acceptance was not established; a support-fixture repair was underway.

### Separate measured resource groups

| Group | Timed items | Resource/client seconds | Known-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| catchup_ci_job | 8 | 6926.000 | 8 | 6926.000 | 4120.000 |
| catchup_native_command | 11 | 4.661 | 11 | 4.661 | 4.661 |
| catchup_command | 89 | 1635.105 | 61 | 899.443 | 899.440 |

Groups overlap each other and mixed work windows; never add them into project elapsed or active-work time. Derived endpoints are excluded from unions. Monotonic timers and separately recorded UTC clocks can differ slightly. Whole-second 0s means below receipt resolution. CI steps and native subcommands are nested within job durations, not extra runner time.

| Resource group / category | Seconds |
|---|---:|
| catchup_ci_job: CI runner | 6926.000 |
| catchup_native_command: build + test/check (combined) | 0.075 |
| catchup_native_command: dependency/environment or verification | 4.587 |
| catchup_command: automated lint/check | 136.236 |
| catchup_command: build + test/check (combined) | 1378.319 |
| catchup_command: dependency/environment or verification | 0.695 |
| catchup_command: build | 119.856 |

### Nested CI phases (already included in CI jobs)

| Phase class | Runner step time |
|---|---:|
| CI orchestration | 50.0s |
| dependency/environment setup | 3m 28.0s |
| build + automated lint/check | 21m 31.0s |
| build/test/check (combined) | 1h 21m 34.0s |
| build | 5m 13.0s |
| CI reporting | 0.0s |
| packaging/validation | 2m 23.0s |

These conservative phase groups can include compilation and execution together; do not add them to the CI job totals.

Within 2026-10-09T08:43:00Z–2026-10-09T12:20:16Z, the selected CI jobs cover 4120.000 overlap-safe wall seconds; 8916.000 seconds are outside those jobs. This remainder includes implementation, tests, review, publication, waiting and unknown time; it is neither proven idle nor model inference.

### Mixed workflows and waits (excluded from resource totals)

| Activity | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---:|---|
| initial /usr/bin/time absent; switched to bash time | unknown | unknown | unknown | failure retained; elapsed unavailable |
| first cargo fmt invoked repository root without manifest; reran correct rust manifest | unknown | unknown | unknown | failure retained; elapsed unavailable |
| Compare native oracle receipts | 2026-10-09T08:46:08+00:00 | 2026-10-09T08:46:57.714628+00:00 | 49.714628 | completed |
| Formatter integration, tests, GUI and sealing | 2026-10-09T08:49:33+00:00 | 2026-10-09T09:00:47.696265+00:00 | 674.696265 | completed |
| Formatter application GUI lifetime | 2026-10-09T08:56:15+00:00 | 2026-10-09T08:59:21+00:00 | 186.0 | completed |
| Formatter source/doc packaging, upload/retry and immutable verification | 2026-10-09T09:02:08+00:00 | 2026-10-09T09:13:59.376384+00:00 | 711.376384 | completed |
| Text Tools portal export GUI retest | 2026-10-09T10:11:11,545051788+00:00 | 2026-10-09T10:13:01,093423163+00:00 | 109.548372 | completed |
| Text Tools source/docs upload and exact immutable readback | 2026-10-09T10:14:32.670Z | 2026-10-09T10:18:04.558Z | 211.888 | completed |
| TextStats/Unique oracle preparation, correction and handoff | 2026-10-09T10:21:58Z | 2026-10-09T10:48:10.827257+00:00 | 1572.827257 | completed |
| rustfmt absent from initial shell PATH; no command executed | unknown | unknown | unknown | setup corrected; no Cargo command executed at failure |
| sourcing gpui-env.sh without base env.sh left cargo unavailable; corrected by sourcing official existing env.sh | unknown | unknown | unknown | setup corrected; no Cargo command executed at failure |
| URL native oracle fixture/harness observed_window | 2026-10-09T11:03:47Z | 2026-10-09T11:04:42Z | 55 | completed |
| URL native oracle fixture/harness integration_window | 2026-10-09T11:10:29Z | 2026-10-09T11:11:36Z | 67 | completed |
| Python fixture-generator syntax typo | 2026-10-09T11:04:24Z | unknown | unknown | corrected and generator rerun successfully |
| URL final actual Linux GUI and source/binary recheck | 2026-10-09T11:49:28,348416177+00:00 | 2026-10-09T11:52:40.524951+00:00 | 192.176535 | completed |
| formatter source publication and verified branch readback | 2026-10-09T09:13:46Z | 2026-10-09T09:14:09Z | 23.0 | completed |
| text source publication and verified branch readback | 2026-10-09T10:18:20Z | 2026-10-09T10:18:40Z | 20.0 | completed |
| semantics source publication and verified branch readback | 2026-10-09T11:01:58Z | 2026-10-09T11:02:13Z | 15.0 | completed |
| url source publication and verified branch readback | 2026-10-09T12:15:11Z | 2026-10-09T12:15:35Z | 24.0 | completed |

The 12,254.9-second cancelled orchestration includes a preceding successful Markdown upload plus transport and the blocked ledger operation; it is not isolated approval time or idle time. Independent implementation and CI continued during this interval. Individual retry duration remains unknown.

### Measured items

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Rust Linux checks / linux | CI runner | 2026-10-09T08:49:40Z | 2026-10-09T09:00:12Z | 632.0 | success |
| Rust macOS native checks / apple-silicon | CI runner | 2026-10-09T08:49:50Z | 2026-10-09T09:07:20Z | 1050.0 | success |
| Rust Linux checks / linux | CI runner | 2026-10-09T09:14:13Z | 2026-10-09T09:26:50Z | 757.0 | success |
| Rust macOS native checks / apple-silicon | CI runner | 2026-10-09T09:14:19Z | 2026-10-09T09:31:12Z | 1013.0 | success |
| Rust Linux checks / linux | CI runner | 2026-10-09T10:18:45Z | 2026-10-09T10:31:16Z | 751.0 | success |
| Rust macOS native checks / apple-silicon | CI runner | 2026-10-09T10:18:52Z | 2026-10-09T10:37:07Z | 1095.0 | success |
| Rust Linux checks / linux | CI runner | 2026-10-09T11:02:17Z | 2026-10-09T11:13:52Z | 695.0 | success |
| Rust macOS native checks / apple-silicon | CI runner | 2026-10-09T11:02:23Z | 2026-10-09T11:17:56Z | 933.0 | success |
| architecture.command.json | build + test/check (combined) | 2026-10-09T08:40:00.631049+00:00 | 2026-10-09T08:40:00.633870+00:00 | 0.0028330000350251794 | 0 |
| binary-type.command.json | build + test/check (combined) | 2026-10-09T08:40:05.250831+00:00 | 2026-10-09T08:40:05.266293+00:00 | 0.015492584090679884 | 0 |
| compile.command.json | dependency/environment or verification | 2026-10-09T08:40:00.875741+00:00 | 2026-10-09T08:40:05.228815+00:00 | 4.35312220803462 | 0 |
| linked-libraries.command.json | build + test/check (combined) | 2026-10-09T08:40:05.229617+00:00 | 2026-10-09T08:40:05.250463+00:00 | 0.020874249981716275 | 0 |
| macos.command.json | build + test/check (combined) | 2026-10-09T08:40:00.620285+00:00 | 2026-10-09T08:40:00.630756+00:00 | 0.010483375051990151 | 0 |
| run-17-vectors.command.json | build + test/check (combined) | 2026-10-09T08:40:05.266591+00:00 | 2026-10-09T08:40:05.285204+00:00 | 0.018658041954040527 | 0 |
| sdk-path.command.json | dependency/environment or verification | 2026-10-09T08:40:00.857828+00:00 | 2026-10-09T08:40:00.867143+00:00 | 0.009355040965601802 | 0 |
| sdk-version.command.json | dependency/environment or verification | 2026-10-09T08:40:00.867496+00:00 | 2026-10-09T08:40:00.875215+00:00 | 0.0077527089742943645 | 0 |
| swift-path.command.json | build + test/check (combined) | 2026-10-09T08:40:00.634114+00:00 | 2026-10-09T08:40:00.640580+00:00 | 0.006496333051472902 | 0 |
| swift-version.command.json | dependency/environment or verification | 2026-10-09T08:40:00.640839+00:00 | 2026-10-09T08:40:00.789291+00:00 | 0.14849862502887845 | 0 |
| xcode-version.command.json | dependency/environment or verification | 2026-10-09T08:40:00.789651+00:00 | 2026-10-09T08:40:00.857487+00:00 | 0.06787279201671481 | 0 |
| format | automated lint/check | 2026-10-09T08:52:20.450204+00:00 | 2026-10-09T08:52:22.186792+00:00 | 1.7366 | 0 |
| core-json | build + test/check (combined) | 2026-10-09T08:52:22.187232+00:00 | 2026-10-09T08:52:22.919547+00:00 | 0.732334 | 0 |
| app-json | build + test/check (combined) | 2026-10-09T08:52:22.919957+00:00 | 2026-10-09T08:52:40.556242+00:00 | 17.636304 | 0 |
| core-full | build + test/check (combined) | 2026-10-09T08:52:40.556960+00:00 | 2026-10-09T08:53:03.373983+00:00 | 22.817046 | 0 |
| strict | automated lint/check | 2026-10-09T08:53:03.374464+00:00 | 2026-10-09T08:53:15.008558+00:00 | 11.634115 | 0 |
| minimal | build + test/check (combined) | 2026-10-09T08:53:15.008982+00:00 | 2026-10-09T08:53:19.012041+00:00 | 4.003079 | 0 |
| mutant-numeric-lexemes | automated lint/check | 2026-10-09T08:53:19.012734+00:00 | 2026-10-09T08:53:23.093650+00:00 | 4.080932 | Expected negative-control failure |
| mutant-canonical-duplicates | automated lint/check | 2026-10-09T08:53:23.094386+00:00 | 2026-10-09T08:53:27.134037+00:00 | 4.039671 | Expected negative-control failure |
| restored-core-json | build + test/check (combined) | 2026-10-09T08:53:27.135618+00:00 | 2026-10-09T08:53:34.051779+00:00 | 6.916178 | 0 |
| restored-strict | automated lint/check | 2026-10-09T08:53:34.052375+00:00 | 2026-10-09T08:53:42.593081+00:00 | 8.540728 | 0 |
| clean-app | dependency/environment or verification | 2026-10-09T08:53:42.593609+00:00 | 2026-10-09T08:53:42.732166+00:00 | 0.1386 | 0 |
| ordinary-build | build | 2026-10-09T08:53:42.732718+00:00 | 2026-10-09T08:53:55.224353+00:00 | 12.491656 | 0 |
| format | automated lint/check | 2026-10-09T08:51:07.091207+00:00 | 2026-10-09T08:51:08.666651+00:00 | 1.575458 | 0 |
| core-json | build + test/check (combined) | 2026-10-09T08:51:08.667024+00:00 | 2026-10-09T08:51:15.847381+00:00 | 7.180376 | 0 |
| app-json | build + test/check (combined) | 2026-10-09T08:51:15.847989+00:00 | 2026-10-09T08:51:37.173524+00:00 | 21.325551 | 101 |
| format | automated lint/check | 2026-10-09T09:41:01.429802+00:00 | 2026-10-09T09:41:03.055954+00:00 | 1.626 | 0 |
| strict | automated lint/check | 2026-10-09T09:41:03.056985+00:00 | 2026-10-09T09:41:09.650490+00:00 | 6.594 | 0 |
| default-app | build + test/check (combined) | 2026-10-09T09:41:09.652278+00:00 | 2026-10-09T09:43:07.168634+00:00 | 117.516 | 0 |
| recording-app | build + test/check (combined) | 2026-10-09T09:43:07.169569+00:00 | 2026-10-09T09:45:27.525812+00:00 | 140.356 | 0 |
| recording-strict | automated lint/check | 2026-10-09T09:45:27.528302+00:00 | 2026-10-09T09:45:35.216260+00:00 | 7.688 | 0 |
| minimal | build + test/check (combined) | 2026-10-09T09:45:35.217089+00:00 | 2026-10-09T09:45:39.094649+00:00 | 3.878 | 0 |
| clean-app | dependency/environment or verification | 2026-10-09T09:45:39.095548+00:00 | 2026-10-09T09:45:39.215613+00:00 | 0.12 | 0 |
| ordinary-build | build | 2026-10-09T09:45:39.216425+00:00 | 2026-10-09T09:45:49.846825+00:00 | 10.63 | 0 |
| format | automated lint/check | 2026-10-09T09:39:53.835004+00:00 | 2026-10-09T09:39:55.573388+00:00 | 1.738 | 0 |
| strict | automated lint/check | 2026-10-09T09:39:55.574120+00:00 | 2026-10-09T09:40:04.721306+00:00 | 9.147 | 101 |
| binding-default | build + test/check (combined) | 2026-10-09T10:46:11.906417+00:00 | 2026-10-09T10:46:12.469932+00:00 | 0.563538228001562 | 0 |
| binding-minimal | build + test/check (combined) | 2026-10-09T10:46:12.470254+00:00 | 2026-10-09T10:46:12.743519+00:00 | 0.2732876549998764 | 0 |
| strict | automated lint/check | 2026-10-09T10:46:12.743823+00:00 | 2026-10-09T10:46:13.078952+00:00 | 0.3351521410004352 | 0 |
| binding-final-default | build + test/check (combined) | 2026-10-09T10:46:38.398687+00:00 | 2026-10-09T10:46:38.862432+00:00 | 0.4637700349994702 | 101 |
| binding-fixed-default | build + test/check (combined) | 2026-10-09T10:46:53.449421+00:00 | 2026-10-09T10:46:53.916651+00:00 | 0.46725449000223307 | 0 |
| binding-fixed-minimal | build + test/check (combined) | 2026-10-09T10:46:53.916997+00:00 | 2026-10-09T10:46:54.363810+00:00 | 0.44683639999857405 | 0 |
| strict-fixed | automated lint/check | 2026-10-09T10:46:54.364119+00:00 | 2026-10-09T10:46:54.717012+00:00 | 0.3529163499988499 | 0 |
| fmt-fixed | automated lint/check | 2026-10-09T10:46:54.717371+00:00 | 2026-10-09T10:46:56.355038+00:00 | 1.637695072000497 | 0 |
| fmt | automated lint/check | 2026-10-09T11:42:17.430524+00:00 | unknown | 1.822 | 0 |
| core-default | build + test/check (combined) | 2026-10-09T11:42:19.252444+00:00 | unknown | 21.12 | 0 |
| core-minimal | build + test/check (combined) | 2026-10-09T11:42:40.372913+00:00 | unknown | 16.739 | 0 |
| app-default | build + test/check (combined) | 2026-10-09T11:42:57.112629+00:00 | unknown | 70.431 | 0 |
| app-minimal | build + test/check (combined) | 2026-10-09T11:44:07.544143+00:00 | unknown | 66.972 | 0 |
| app-recording | build + test/check (combined) | 2026-10-09T11:45:14.516465+00:00 | unknown | 72.684 | 0 |
| editor-core | build + test/check (combined) | 2026-10-09T11:46:27.200353+00:00 | unknown | 2.079 | 0 |
| editor-ui | build + test/check (combined) | 2026-10-09T11:46:29.279790+00:00 | unknown | 5.103 | 0 |
| strict | automated lint/check | 2026-10-09T11:46:34.383991+00:00 | unknown | 9.643 | 0 |
| strict-recording | automated lint/check | 2026-10-09T11:46:44.027584+00:00 | unknown | 9.238 | 0 |
| clean-app | dependency/environment or verification | 2026-10-09T11:46:53.266336+00:00 | unknown | 0.178 | 0 |
| ordinary-build | build | 2026-10-09T11:46:53.444906+00:00 | unknown | 13.647 | 0 |
| fmt | automated lint/check | 2026-10-09T11:49:54.715278+00:00 | unknown | 1.992 | 0 |
| app-default | build + test/check (combined) | 2026-10-09T11:49:56.707774+00:00 | unknown | 70.95 | 0 |
| app-minimal | build + test/check (combined) | 2026-10-09T11:51:07.657948+00:00 | unknown | 67.039 | 0 |
| app-recording | build + test/check (combined) | 2026-10-09T11:52:14.697496+00:00 | unknown | 70.323 | 0 |
| strict | automated lint/check | 2026-10-09T11:53:25.020709+00:00 | unknown | 7.746 | 0 |
| strict-recording | automated lint/check | 2026-10-09T11:53:32.766971+00:00 | unknown | 7.209 | 0 |
| clean-app | dependency/environment or verification | 2026-10-09T11:53:39.976577+00:00 | unknown | 0.142 | 0 |
| ordinary-build | build | 2026-10-09T11:53:40.119015+00:00 | unknown | 11.914 | 0 |
| core-default | build + test/check (combined) | 2026-10-09T10:27:27.635851+00:00 | 2026-10-09T10:27:40.248930+00:00 | 12.613 | 0 |
| core-minimal | build + test/check (combined) | 2026-10-09T10:27:40.249631+00:00 | 2026-10-09T10:27:52.458154+00:00 | 12.209 | 0 |
| core-strict | automated lint/check | 2026-10-09T10:27:52.458743+00:00 | 2026-10-09T10:27:55.975761+00:00 | 3.517 | 0 |
| format | automated lint/check | 2026-10-09T10:27:55.976370+00:00 | 2026-10-09T10:27:57.679912+00:00 | 1.704 | 0 |
| strict | automated lint/check | 2026-10-09T10:27:57.680506+00:00 | 2026-10-09T10:28:15.544209+00:00 | 17.864 | 0 |
| default-app | build + test/check (combined) | 2026-10-09T10:28:15.544889+00:00 | 2026-10-09T10:30:17.898919+00:00 | 122.354 | 0 |
| recording-app | build + test/check (combined) | 2026-10-09T10:30:17.899823+00:00 | 2026-10-09T10:32:38.178389+00:00 | 140.279 | 0 |
| recording-strict | automated lint/check | 2026-10-09T10:32:38.180537+00:00 | 2026-10-09T10:32:45.922918+00:00 | 7.742 | 0 |
| minimal | build + test/check (combined) | 2026-10-09T10:32:45.924121+00:00 | 2026-10-09T10:32:49.735005+00:00 | 3.811 | 0 |
| clean-app | dependency/environment or verification | 2026-10-09T10:32:49.735656+00:00 | 2026-10-09T10:32:49.851217+00:00 | 0.116 | 0 |
| ordinary-build | build | 2026-10-09T10:32:49.852057+00:00 | 2026-10-09T10:33:00.530665+00:00 | 10.679 | 0 |
| core-default | build + test/check (combined) | 2026-10-09T10:26:32.323493+00:00 | 2026-10-09T10:26:51.674534+00:00 | 19.351 | 0 |
| core-minimal | build + test/check (combined) | 2026-10-09T10:26:51.675052+00:00 | 2026-10-09T10:26:56.211904+00:00 | 4.537 | 101 |
| focused-core | build + test/check (combined) | 2026-10-09T10:24:19.478489+00:00 | 2026-10-09T10:24:23.091834+00:00 | 3.613 | 101 |
| focused-core | build + test/check (combined) | 2026-10-09T10:24:45.485819+00:00 | 2026-10-09T10:24:52.553303+00:00 | 7.067 | 0 |
| focused-app | build + test/check (combined) | 2026-10-09T10:24:52.553600+00:00 | 2026-10-09T10:25:19.057683+00:00 | 26.504 | 101 |
| focused-core | build + test/check (combined) | 2026-10-09T10:25:38.648210+00:00 | 2026-10-09T10:25:39.061013+00:00 | 0.413 | 0 |
| focused-app | build + test/check (combined) | 2026-10-09T10:25:39.061459+00:00 | 2026-10-09T10:25:58.947064+00:00 | 19.886 | 0 |
| oracle-combined-core-default | build + test/check (combined) | 2026-10-09T10:50:08.804638+00:00 | 2026-10-09T10:50:21.098560+00:00 | 12.294 | 0 |
| oracle-combined-core-minimal | build + test/check (combined) | 2026-10-09T10:50:21.098863+00:00 | 2026-10-09T10:50:33.254143+00:00 | 12.155 | 0 |
| oracle-combined-core-strict | automated lint/check | 2026-10-09T10:50:33.254500+00:00 | 2026-10-09T10:50:33.538084+00:00 | 0.284 | 0 |
| oracle-combined-core-minimal-strict | automated lint/check | 2026-10-09T10:50:33.538567+00:00 | 2026-10-09T10:50:33.838728+00:00 | 0.3 | 0 |
| oracle-combined-format | automated lint/check | 2026-10-09T10:50:33.839090+00:00 | 2026-10-09T10:50:35.636625+00:00 | 1.798 | 0 |
| repaired-core-default | build + test/check (combined) | 2026-10-09T10:33:00.648307+00:00 | 2026-10-09T10:33:13.555059+00:00 | 12.907 | 0 |
| repaired-core-minimal | build + test/check (combined) | 2026-10-09T10:33:13.555400+00:00 | 2026-10-09T10:33:25.668133+00:00 | 12.113 | 0 |
| repaired-core-strict | automated lint/check | 2026-10-09T10:33:25.668505+00:00 | 2026-10-09T10:33:26.107384+00:00 | 0.439 | 0 |
| repaired-core-minimal-strict | automated lint/check | 2026-10-09T10:33:26.107744+00:00 | 2026-10-09T10:33:28.579306+00:00 | 2.472 | 0 |
| repaired-format | automated lint/check | 2026-10-09T10:33:28.579699+00:00 | 2026-10-09T10:33:30.319609+00:00 | 1.74 | 0 |
| first-compile.log | build | unknown | unknown | 60.494 | failed linker: GPUI env script omitted |
| url-focused.log | build + test/check (combined) | unknown | unknown | 40.178 | failed compile: new event exhaustive downstream match; removed variant |
| url-focused-r2.log | build + test/check (combined) | unknown | unknown | 8.88 | failed compile: glob-imported gpui test macro recursion |
| url-focused-r3.log | build + test/check (combined) | unknown | unknown | 12.291 | failed compile: Focusable import plus unused local |
| url-focused-r4.log | build + test/check (combined) | unknown | unknown | 27.425 | 5 passed/1 failed: test direct row removal stale row render unwrap; guarded lookup |
| url-focused-r5.log | build + test/check (combined) | unknown | unknown | 26.186 | 6 passed |
| core-focused.log | build + test/check (combined) | unknown | unknown | 8.392 | passed; native oracle ignored on Linux |
| editor-boundary.log | build + test/check (combined) | unknown | unknown | 24.845 | 1 passed |

Full source hashes, source URLs, nested job steps and timing limitations are in duration-data.json. Native macOS full logs were unavailable for some Agent runs; verified job/step metadata is retained without a full-log claim. Later source CI may be running and is not silently promoted to success by this snapshot.

### Earlier accounting (unchanged)

## Checkpoint: 2026-10-09T08:43:00Z

This adds selected newly obtained receipts, including earlier intervals not present in the 08:08 snapshot. Historical records and previous checkpoint coverage remain unchanged. Unknown inference stays unavailable. Source/receipt hashes identify evidence; local observer timing is not independently verified merely by a source commit link.

| Group | Timed items | Resource/client seconds | Exact-endpoint items | Endpoint-subset seconds | Endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| new_local_command | 25 | 673.614 | 25 | 673.614 | 673.614 |
| new_reported_phase | 3 | 0.890 | 0 | 0.000 | unavailable |

Resource sums, interval unions, mixed windows and nested Cargo phase subtotals are separate accounting views. Do not add them into a total time budget. Parallel client/API durations include waiting, not server compute. Whole-second 0s means below receipt resolution, not zero effort. Absent endpoints exclude items from wall unions.

| Group / category | Seconds |
|---|---:|
| new_local_command: build + test/check (combined) | 375.921 |
| new_local_command: build + test/lint (combined) | 242.000 |
| new_local_command: automated lint/check | 36.868 |
| new_local_command: dependency/environment or source verification | 0.385 |
| new_local_command: build | 18.440 |
| new_reported_phase: test-target compilation | 0.570 |
| new_reported_phase: test execution | 0.020 |
| new_reported_phase: automated lint/check | 0.300 |

### Per-item observations

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| json-r1-gui | interactive validation (mixed) | 2026-10-09T08:20:32.534803+00:00 | 2026-10-09T08:30:33.200141+00:00 | 600.665338 | completed |
| json-r2-gui | interactive validation (mixed) | 2026-10-09T08:34:37.841199+00:00 | 2026-10-09T08:37:05.316487+00:00 | 147.475288 | completed |
| cargo test --locked -p bellobox-app json_session:: --no-run | build + test/check (combined) | 2026-10-09T07:51:53Z | 2026-10-09T07:52:01Z | 8.0 | compile failure: GPUI glob-import test macro recursion; fixed explicit import |
| cargo test --locked -p bellobox-app json_session:: | build + test/check (combined) | 2026-10-09T07:52:14Z | 2026-10-09T07:52:29Z | 15.0 | 2 tests passed; preliminary session source |
| cargo test --locked -p bellobox-app json_session:: | build + test/check (combined) | 2026-10-09T07:55:15Z | 2026-10-09T07:55:22Z | 7.0 | compile failure: two adapter method visibility errors; fixed |
| cargo test --locked -p bellobox-app json_session:: | build + test/check (combined) | 2026-10-09T07:55:39Z | 2026-10-09T07:55:54Z | 15.0 | 2 tests passed; integrated source |
| cargo test --locked -p bellobox-app json -- --test-threads=2 | build + test/check (combined) | 2026-10-09T08:00:35Z | 2026-10-09T08:00:50Z | 15.0 | 11 passed, 1 failed: pre-existing object key order differs from Swift; characterization retained |
| cargo test --locked -p bellobox-app json -- --test-threads=2 | build + test/check (combined) | 2026-10-09T08:02:50Z | 2026-10-09T08:03:06Z | 16.0 | 15 passed, 0 failed; no local code warnings |
| cargo fmt --all; cargo test --locked -p bellobox-app -- --test-threads=2 | build + test/lint (combined) | 2026-10-09T08:07:54Z | 2026-10-09T08:09:49Z | 115.0 | 467 passed, 2 existing ignored; historical pre-final ownership review |
| cargo fmt --all; cargo test --locked -p bellobox-app -- --test-threads=2 | build + test/lint (combined) | 2026-10-09T08:10:11Z | 2026-10-09T08:12:09Z | 118.0 | 468 passed, 2 existing ignored; historical before successful-open latch and pre-clone size guard |
| cargo fmt --all; cargo test --locked -p bellobox-app json -- --test-threads=2 | build + test/lint (combined) | 2026-10-09T08:13:45Z | 2026-10-09T08:13:54Z | 9.0 | compile failure: missing Focusable import in new test; corrected |
| cargo test --locked -p bellobox-app json -- --test-threads=2 | build + test/check (combined) | 2026-10-09T08:14:30Z | 2026-10-09T08:14:47Z | 17.0 | 19 passed, 0 failed |
| cargo fmt --all -- --check; cargo clippy --locked -p bellobox-app --all-targets -- -D warnings | automated lint/check | 2026-10-09T08:15:13Z | 2026-10-09T08:15:23Z | 10.0 | strict failure: unnecessary String conversion in debug selector; mechanically removed |
| Interleaved source implementation, investigation, review responses and focused command checks; overlaps command durations and is not additive. | implementation/review/validation (mixed) | 2026-10-09T07:48:45Z | 2026-10-09T08:03:18Z | 873.0 | completed |
| Interleaved source/ownership/key review, focused and broad verification, documentation, ordinary artifact sealing and GUI handoff. Overlaps commands; not additive. Inference duration unavailable. | implementation/review/validation (mixed) | 2026-10-09T08:03:18Z | 2026-10-09T08:19:59Z | 1001.0 | completed |
| Interleaved immutable artifact handoff, recording-feature checks, source/LOC verification, awaiting GUI acceptance and minimal Validate wording successor checks/seal. Overlaps commands and peer work; not additive. Inference timing unavailable. | implementation/review/validation (mixed) | 2026-10-09T08:19:59Z | 2026-10-09T08:34:25Z | 866.0 | completed |
| final: format | automated lint/check | 2026-10-09T08:16:28.367107+00:00 | 2026-10-09T08:16:29.966285+00:00 | 1.599 | passed |
| final: strict | automated lint/check | 2026-10-09T08:16:29.966662+00:00 | 2026-10-09T08:16:36.476497+00:00 | 6.51 | passed |
| final: full-app | build + test/check (combined) | 2026-10-09T08:16:36.476868+00:00 | 2026-10-09T08:18:31.878999+00:00 | 115.402 | passed |
| final: minimal | build + test/check (combined) | 2026-10-09T08:18:31.879449+00:00 | 2026-10-09T08:18:37.667035+00:00 | 5.788 | passed |
| final: clean-app | dependency/environment or source verification | 2026-10-09T08:18:37.667446+00:00 | 2026-10-09T08:18:37.783714+00:00 | 0.116 | passed |
| final: ordinary-build | build | 2026-10-09T08:18:37.784064+00:00 | 2026-10-09T08:18:47.376246+00:00 | 9.592 | passed |
| successor: format | automated lint/check | 2026-10-09T08:32:34.388445+00:00 | 2026-10-09T08:32:36.047403+00:00 | 1.659 | passed |
| successor: core-json | build + test/check (combined) | 2026-10-09T08:32:36.047722+00:00 | 2026-10-09T08:32:42.588437+00:00 | 6.541 | passed |
| successor: app-json | build + test/check (combined) | 2026-10-09T08:32:42.588795+00:00 | 2026-10-09T08:33:04.550588+00:00 | 21.962 | passed |
| successor: strict | automated lint/check | 2026-10-09T08:33:04.551118+00:00 | 2026-10-09T08:33:13.841845+00:00 | 9.291 | passed |
| successor: clean-app | dependency/environment or source verification | 2026-10-09T08:33:13.842233+00:00 | 2026-10-09T08:33:14.111263+00:00 | 0.269 | passed |
| successor: ordinary-build | build | 2026-10-09T08:33:14.111653+00:00 | 2026-10-09T08:33:22.960038+00:00 | 8.848 | passed |
| recording: recording-full-app | build + test/check (combined) | 2026-10-09T08:20:18.474436+00:00 | 2026-10-09T08:22:31.702798+00:00 | 133.228 | passed |
| recording: recording-strict | automated lint/check | 2026-10-09T08:22:31.703737+00:00 | 2026-10-09T08:22:39.512623+00:00 | 7.809 | passed |
| JSON formatter parity audit | review (mixed) | 2026-10-09T08:02:01+00:00 | 2026-10-09T08:05:08.455401+00:00 | 187.455 | completed |
| Initial JSON formatter prototype delivery | implementation/validation (mixed) | 2026-10-09T08:29:15+00:00 | 2026-10-09T08:36:14.889260+00:00 | 419.88926 | completed |
| test-target compilation | test-target compilation | unknown | unknown | 0.57 | completed |
| test execution | test execution | unknown | unknown | 0.02 | completed |
| automated lint/check | automated lint/check | unknown | unknown | 0.3 | completed |

### Mixed-window groups (not resource totals)

| Category | Windows | Known-endpoint union seconds |
|---|---:|---:|
| implementation/review/validation (mixed) | 3 | 2740.000 |
| implementation/validation (mixed) | 1 | 419.889 |
| interactive validation (mixed) | 2 | 748.141 |
| review (mixed) | 1 | 187.455 |

Mixed work windows overlap commands and each other; no active implementation, review or inference time is inferred. Native final-receipt aggregates duplicate individual results and are excluded. Original failed native ENOSPC attempts and exact retries remain separate; APFS copy-on-write recovery timing is measured where available, but metadata removal lacks a timer and stays unknown. Independently verified delivery is not a new native execution. CI rows here are status observations only. No full native suite or GUI claim is added.

### Earlier checkpoints (preserved)

## Incremental checkpoint: 2026-10-09T08:08:00Z

Historical audit below and its original CI cutoff remain unchanged. This update covers selected newly recorded work, not every intervening run or task. Unknown durations and inference remain unavailable. Local timing observations below are source records supported by retained receipt hashes; public commit links identify source or outcome, not independent timing verification.

### Separate resource totals

| Group | All timed items | All resource/client seconds | Known-endpoint items | Known-endpoint resource seconds | Known-endpoint union seconds |
|---|---:|---:|---:|---:|---:|
| incremental_command | 6 | 249.417 | 6 | 249.417 | 249.417 |
| incremental_ci_job | 2 | 1441.000 | 2 | 1441.000 | 823.000 |

| Resource group / category | Seconds |
|---|---:|
| incremental_command: automated lint/check | 9.365 |
| incremental_command: build + test/check (combined) | 231.242 |
| incremental_command: build | 8.810 |
| incremental_ci_job: CI | 1441.000 |

API elapsed sums include overlapping pending calls and client/service waiting; they are not server compute or active work. Command sums include build/test mixtures. Whole-second 0s observations mean below the receipt counter resolution, not zero effort. Missing or arithmetically derived end timestamps are excluded from known-endpoint unions. CI steps are nested and excluded from job sums. These groups must not be added to mixed task windows or treated as project elapsed time.

### Where newly observed task time went

7 mixed task windows; union of closed observed intervals: 2112.000 seconds. This includes overlap and waiting; it is not an active-work, CPU or inference total. Unfinished waits keep an unknown final duration.

| Activity | Category | Start UTC | End UTC | Seconds | Outcome |
|---|---|---|---|---:|---|
| Cumulative popup/palette actual GUI regression including wait for sealed binary | validation (mixed) | 2026-10-09T05:56:59Z | 2026-10-09T06:07:56Z | 657.0 | completed |
| WorldClock fixture repair and native full-suite validation | implementation/rework + validation (mixed) | 2026-10-09T06:11:04Z | 2026-10-09T06:21:15Z | 611.0 | completed |
| Final WorldClock successor LOC audit | review/documentation (mixed) | 2026-10-09T06:26:23Z | 2026-10-09T06:28:05Z | 102.0 | completed |
| Final WorldClock candidate fmt | automated lint/check | 2026-10-09T06:23:01.640321+00:00 | 2026-10-09T06:23:03.179157+00:00 | 1.5388964689991553 | completed |
| Final WorldClock candidate focused | build + test/check (combined) | 2026-10-09T06:23:03.179689+00:00 | 2026-10-09T06:23:25.546312+00:00 | 22.36665100400569 | completed |
| Final WorldClock candidate recording-full-app | build + test/check (combined) | 2026-10-09T06:23:25.546699+00:00 | 2026-10-09T06:26:54.264859+00:00 | 208.71817806200124 | completed |
| Final WorldClock candidate clippy | automated lint/check | 2026-10-09T06:26:54.265559+00:00 | 2026-10-09T06:27:02.091370+00:00 | 7.825826902997505 | completed |
| Final WorldClock candidate clean | build + test/check (combined) | 2026-10-09T06:27:02.091789+00:00 | 2026-10-09T06:27:02.249420+00:00 | 0.15764934199978597 | completed |
| Final WorldClock candidate ordinary-build | build | 2026-10-09T06:27:02.249845+00:00 | 2026-10-09T06:27:11.059920+00:00 | 8.810094290995039 | completed |
| Shared editor native evidence verification and packaging | review/documentation (mixed) | 2026-10-09T05:28:43Z | 2026-10-09T05:37:15Z | 512.0 | completed |
| Shared editor documentation preparation | review/documentation (mixed) | 2026-10-09T05:37:29Z | 2026-10-09T05:38:37Z | 68.0 | completed |
| Shared editor immutable documentation upload and verification | publication/preparation (mixed) | 2026-10-09T05:38:49Z | 2026-10-09T05:41:31Z | 162.0 | completed |
| Validation text artifact GitHub blob publication waiting for platform confirmation | waiting | 2026-10-09T06:35:27Z | unknown | unknown | in_progress |
| Final Box linux CI job | CI | 2026-10-09T07:00:32Z | 2026-10-09T07:10:56Z | 624.0 | completed |
| Final Box macos CI job | CI | 2026-10-09T07:00:38Z | 2026-10-09T07:14:15Z | 817.0 | completed |
| Final Box two-job elapsed coverage | mixed task window | 2026-10-09T07:00:32+00:00 | 2026-10-09T07:14:15+00:00 | 823.0 | completed |

### Mixed-window groups (not resource totals)

| Category | All windows | Closed windows with endpoints | Closed interval union (seconds) |
|---|---:|---:|---:|
| implementation/rework + validation (mixed) | 1 | 1 | 611.000 |
| publication/preparation (mixed) | 1 | 1 | 162.000 |
| review/documentation (mixed) | 3 | 3 | 682.000 |
| validation (mixed) | 1 | 1 | 657.000 |
| waiting | 1 | 0 | unknown / not yet closed |

Categories can overlap each other and include productive parallel work during waits. Do not sum category unions or interpret any category as active labor.

Per-command and API child records, nested CI steps, source hashes and uncertainty are retained in duration-data.json. Nested child resource totals are counted once in their separate group; their parent windows remain excluded. Shared editor work is assigned to BelloBox; shared initial timing-publication wait is represented once in BelloAgent.

## Historical audit (original coverage below)

Report generated 2026-10-09 06:05 UTC; CI evidence cutoff 2026-10-09 05:45:13 UTC. Local candidate receipts are identified separately. Source checkpoint: [`0e13ee6def21`](https://github.com/BelloWare/BelloBox/commit/0e13ee6def214425865759591c37539429530d71). Branch: `rust` (the requested “risk” interpreted as `rust`).

## What can actually be accounted for

- **168 CI runs, 168 job attempts:** 20h 19m 07.00s of measured runner occupancy. Parallel jobs overlap; their union covers **12h 34m 13.00s** of calendar time.
- **166 distinct timed local/native command receipts:** 0h 23m 09.03s of command occupancy. This is a separate partial subtotal, not total project effort.
- **Assistant inference duration: unavailable.** No per-request inference telemetry survives in these sources. It is neither zero nor the remainder between commits.
- Final CI run outcomes: 137 success, 20 failure, 11 cancelled. Failed and cancelled work is retained.
- Source implementation, interactive review/GUI operation, publication, lost-environment work and dependency waiting lack complete timers. Their durations remain unknown; commit timestamps prove milestones only.

## Accounting rules and coverage

The covered evidence spans October 4–9, 2026 UTC. This is a retrospective lower-coverage audit, not a timesheet. CI jobs use exact GitHub start/end timestamps (seconds precision). A resource subtotal sums occupied runner/command intervals even when independent resources execute in parallel. Only an interval union is overlap-safe calendar coverage. Neither is CPU time or active human/model labor. Local command receipts may contain compilation and execution together. CI step times are nested within jobs; Cargo-reported build and test phases are nested within commands. Do not add these subtotals together. No project elapsed-time remainder is assigned to inference, waiting, or implementation.

Machine-readable per-item records and source links are in [duration-data.json](duration-data.json). Each record supplies duration basis, available start/end, resource, outcome and uncertainty. Empty timestamps mean unavailable, not zero. No estimates are fabricated. The estimated-duration total is unavailable rather than a manufactured zero. The 168 runs include one rerun; its failed first attempt is listed separately, one cancelled run has no allocated job, leaving 168 allocated attempts across 168 runs. Content-identical logs and duplicate command receipts are collapsed; raw media sample durations are excluded.

Shared workbench engineering is assigned to BelloBox once; each consuming repository retains its own validation/CI time. This avoids double counting shared implementation across the two reports.

## Grouped measured totals

| Layer / group | Count | Resource duration | Interpretation |
|---|---:|---:|---|
| CI jobs: ci | 168 | 20h 19m 07.00s | Job occupancy |
| Local/native command receipts: automated lint/check | 53 | 0h 04m 10.82s | Partial command subtotal |
| Local/native command receipts: build | 20 | 0h 04m 47.66s | Partial command subtotal |
| Local/native command receipts: build + test (combined) | 83 | 0h 13m 56.92s | Partial command subtotal |
| Local/native command receipts: environment_wait | 2 | 0h 00m 01.50s | Partial command subtotal |
| Local/native command receipts: test | 8 | 0h 00m 12.14s | Partial command subtotal |
| Implementation / source review / interactive GUI / publication | Unknown | Unknown | Untimed; milestones only |
| Assistant inference | Unknown | Unavailable | No direct telemetry; never inferred from gaps |

### Where CI runner time went (nested step breakdown)

| Step category | Duration |
|---|---:|
| automated lint/check | 2h 42m 43.00s |
| build | 6h 22m 03.00s |
| build + test (combined) | 8h 06m 48.00s |
| build + test + lint (combined) | 0h 10m 17.00s |
| CI reporting | 0h 00m 06.00s |
| dependency/toolchain installation | 1h 31m 20.00s |
| environment setup/verification | 0h 20m 50.00s |
| packaging + validation | 0h 37m 05.00s |
| test | 0h 20m 13.00s |

The explicit step-name mapping was checked against the preserved Linux/macOS workflow commands. dependency_install is package/toolchain installation; environment_setup is checkout, runner setup/cleanup and Apple toolchain verification. Python test harnesses remain test-only; cargo test is combined build + test, including cache misses. The movie-host step also includes clippy and is explicitly combined build/test/lint. These categories do not measure all dependency/network waiting, because cargo can download/resolve or wait inside other steps. Mixed build/test steps and Cargo test commands include compilation; automated_lint_check means formatting/lint execution, not untimed reasoning or source review. Job-minus-step overhead is not inference.

### CI outcomes and daily resource use

| UTC job-start day | Attempts | Runner duration |
|---|---:|---:|
| 2026-10-04 | 3 | 0h 10m 47.00s |
| 2026-10-05 | 41 | 3h 01m 26.00s |
| 2026-10-06 | 41 | 3h 31m 17.00s |
| 2026-10-07 | 47 | 5h 27m 50.00s |
| 2026-10-08 | 26 | 5h 59m 59.00s |
| 2026-10-09 | 10 | 2h 07m 48.00s |

## Prior-days reconstruction and unmeasured work

The complete immutable [October 7 migration handoff](https://github.com/BelloWare/BelloBox/blob/f1c21ae4f22b14dc10c5a57651499b05cc37ec92/rust/docs/MIGRATION_HANDOFF.md) was reviewed, including the recovery inventory and documented failed prerequisites. Earlier source and validation records establish the following work; they do not establish active-work durations.

| Work / rework | Evidence and outcome | Duration |
|---|---|---|
| Initial Rust/GPUI migration; core tools, editor, QR, Clock and shared workbench | See dated milestones below and CI history | Unknown implementation/review |
| October 6–7 native Screen/Area/Window, movie reader, frozen selector, refresh/Undo | Handoff and checked-in evidence; native gates deliberately retained | Unknown implementation; CI measured separately |
| Environment loss/reconstruction and Linux prerequisites | Handoff records lost screenshots, bubblewrap mount failure, later apt exit 100; no packages installed by those failed attempts | Unknown blocked/wait/recovery time |
| Window own-PID fixture correction and transient Xvfb activation timeout | Handoff records real fixture error plus unchanged exact failed-job rerun success | CI attempts retained; diagnosis unknown |
| October 7 alpha oracle, overlay, catalog, converter/movie preview and OCR | Checked-in validation directories and milestone list | Partial command timers; source/GUI effort unknown |
| October 7–8 recording fixtures, shutdown/drain, Swift native oracle and GIF paths | Test-only/platform gates and independent native checks in source evidence | Partial command timers; rework unknown |
| October 8–9 provider setup/Quit, Clock Copilot and generation preferences | Multiple implementation, GUI and provenance checkpoints | Partial command timers; design/GUI/publication unknown |
| October 9 GIF completion, positive-composition and first-following fixtures | Failing controls, independent native investigation, lint failures and repaired tests retained | Timed local receipts and CI; untimed investigation unknown |
| October 9 palette/popup QR and clipboard, held-key, bounds and socket fixture repair | Repeated source/GUI/native iterations; current Linux CI succeeds, macOS fails on OCR accept-readiness fixture | Timed receipts and CI; interactive work unknown |
| Shared find/editor and OCR readiness repair after current published baseline | Candidate evidence includes 37 shared tests and cumulative 465 App passes/2 ignored; remaining validation/publication belongs to next checkpoint | Not represented as published completion |
| Assistant inference / planning / review | No direct model timing telemetry available | Unavailable, not zero |
| Publication uploads, provenance, source binding and verification | Verified commit milestones and publication records; no end-to-end timers | Unknown |

## Each CI job attempt

Final run outcomes are 137 successful, 20 failed and 11 cancelled. Allocated job-attempt outcomes are 137 successful, 21 failed and 10 cancelled. Run [37551464607](https://github.com/BelloWare/BelloBox/actions/runs/37551464607) failed on attempt 1 and succeeded on attempt 2; cancelled run [37545348312](https://github.com/BelloWare/BelloBox/actions/runs/37545348312) has no allocated job in the API. It is counted as a cancelled run with unknown/unallocated runner duration, not as a measured zero-time job.

| Run / job | UTC start | UTC end | Duration | Outcome |
|---|---|---|---:|---|
| [37177846628 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37177846628/job/111364124926) | 2026-10-04T04:44:03Z | 2026-10-04T04:46:47Z | 0h 02m 44.00s | success |
| [37180547846 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37180547846/job/111372077102) | 2026-10-04T05:40:04Z | 2026-10-04T05:44:07Z | 0h 04m 03.00s | success |
| [37181598496 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37181598496/job/111375112817) | 2026-10-04T06:01:56Z | 2026-10-04T06:05:56Z | 0h 04m 00.00s | success |
| [37260098623 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37260098623/job/111605325259) | 2026-10-05T03:36:22Z | 2026-10-05T03:38:24Z | 0h 02m 02.00s | cancelled |
| [37260219509 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37260219509/job/111605733920) | 2026-10-05T03:38:27Z | 2026-10-05T03:42:34Z | 0h 04m 07.00s | success |
| [37261861451 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37261861451/job/111610567936) | 2026-10-05T04:03:16Z | 2026-10-05T04:06:59Z | 0h 03m 43.00s | failure |
| [37262183461 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37262183461/job/111611527441) | 2026-10-05T04:08:03Z | 2026-10-05T04:10:06Z | 0h 02m 03.00s | cancelled |
| [37262306319 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37262306319/job/111611954445) | 2026-10-05T04:10:08Z | 2026-10-05T04:10:45Z | 0h 00m 37.00s | cancelled |
| [37262354202 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37262354202/job/111612096753) | 2026-10-05T04:10:47Z | 2026-10-05T04:14:49Z | 0h 04m 02.00s | success |
| [37262690018 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37262690018/job/111613040150) | 2026-10-05T04:15:18Z | 2026-10-05T04:17:51Z | 0h 02m 33.00s | cancelled |
| [37262865859 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37262865859/job/111613604228) | 2026-10-05T04:17:54Z | 2026-10-05T04:22:05Z | 0h 04m 11.00s | success |
| [37263948344 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37263948344/job/111616705235) | 2026-10-05T04:32:14Z | 2026-10-05T04:36:31Z | 0h 04m 17.00s | success |
| [37264348932 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37264348932/job/111617888292) | 2026-10-05T04:37:53Z | 2026-10-05T04:42:52Z | 0h 04m 59.00s | failure |
| [37264702367 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37264702367/job/111618927643) | 2026-10-05T04:42:43Z | 2026-10-05T04:47:14Z | 0h 04m 31.00s | success |
| [37264702355 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37264702355/job/111618964583) | 2026-10-05T04:42:58Z | 2026-10-05T04:47:05Z | 0h 04m 07.00s | failure |
| [37265316476 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37265316476/job/111620797492) | 2026-10-05T04:51:52Z | 2026-10-05T04:54:17Z | 0h 02m 25.00s | cancelled |
| [37265316546 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37265316546/job/111620797153) | 2026-10-05T04:51:55Z | 2026-10-05T04:55:06Z | 0h 03m 11.00s | failure |
| [37265458651 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37265458651/job/111621287216) | 2026-10-05T04:54:21Z | 2026-10-05T04:58:47Z | 0h 04m 26.00s | success |
| [37265458631 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37265458631/job/111621455112) | 2026-10-05T04:55:12Z | 2026-10-05T04:58:51Z | 0h 03m 39.00s | success |
| [37266099361 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37266099361/job/111623155109) | 2026-10-05T05:03:27Z | 2026-10-05T05:06:58Z | 0h 03m 31.00s | success |
| [37267080593 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37267080593/job/111626057217) | 2026-10-05T05:17:02Z | 2026-10-05T05:18:54Z | 0h 01m 52.00s | cancelled |
| [37267080650 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37267080650/job/111626057562) | 2026-10-05T05:17:09Z | 2026-10-05T05:22:53Z | 0h 05m 44.00s | success |
| [37267202831 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37267202831/job/111626477522) | 2026-10-05T05:18:59Z | 2026-10-05T05:23:24Z | 0h 04m 25.00s | success |
| [37267202895 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37267202895/job/111627370491) | 2026-10-05T05:23:02Z | 2026-10-05T05:28:38Z | 0h 05m 36.00s | success |
| [37273495584 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37273495584/job/111645345553) | 2026-10-05T06:39:08Z | 2026-10-05T06:43:28Z | 0h 04m 20.00s | success |
| [37273495574 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37273495574/job/111645345560) | 2026-10-05T06:39:14Z | 2026-10-05T06:45:54Z | 0h 06m 40.00s | success |
| [37275063052 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37275063052/job/111650164139) | 2026-10-05T06:57:24Z | 2026-10-05T07:01:10Z | 0h 03m 46.00s | success |
| [37275063054 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37275063054/job/111650163983) | 2026-10-05T06:57:28Z | 2026-10-05T07:01:18Z | 0h 03m 50.00s | success |
| [37277416231 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37277416231/job/111657433052) | 2026-10-05T07:22:56Z | 2026-10-05T07:27:12Z | 0h 04m 16.00s | success |
| [37277416153 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37277416153/job/111657433084) | 2026-10-05T07:23:02Z | 2026-10-05T07:28:36Z | 0h 05m 34.00s | success |
| [37283551894 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37283551894/job/111676992030) | 2026-10-05T08:25:38Z | 2026-10-05T08:30:09Z | 0h 04m 31.00s | success |
| [37283551895 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37283551895/job/111676991576) | 2026-10-05T08:25:46Z | 2026-10-05T08:33:28Z | 0h 07m 42.00s | success |
| [37287423173 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37287423173/job/111689532224) | 2026-10-05T09:02:30Z | 2026-10-05T09:06:58Z | 0h 04m 28.00s | success |
| [37287423208 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37287423208/job/111689532845) | 2026-10-05T09:02:34Z | 2026-10-05T09:07:16Z | 0h 04m 42.00s | success |
| [37288175014 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37288175014/job/111691994171) | 2026-10-05T09:09:27Z | 2026-10-05T09:14:12Z | 0h 04m 45.00s | failure |
| [37288171741 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37288171741/job/111691983207) | 2026-10-05T09:09:32Z | 2026-10-05T09:15:32Z | 0h 06m 00.00s | success |
| [37291058162 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37291058162/job/111701283611) | 2026-10-05T09:35:36Z | 2026-10-05T09:40:09Z | 0h 04m 33.00s | success |
| [37291058113 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37291058113/job/111701283614) | 2026-10-05T09:35:43Z | 2026-10-05T09:42:26Z | 0h 06m 43.00s | success |
| [37295640115 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37295640115/job/111716068446) | 2026-10-05T10:17:33Z | 2026-10-05T10:21:04Z | 0h 03m 31.00s | success |
| [37295640125 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37295640125/job/111716069175) | 2026-10-05T10:17:40Z | 2026-10-05T10:24:30Z | 0h 06m 50.00s | success |
| [37302005328 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37302005328/job/111736679648) | 2026-10-05T11:17:41Z | 2026-10-05T11:22:10Z | 0h 04m 29.00s | success |
| [37302005376 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37302005376/job/111736679657) | 2026-10-05T11:17:48Z | 2026-10-05T11:25:48Z | 0h 08m 00.00s | success |
| [37342336656 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37342336656/job/111872246723) | 2026-10-05T16:38:07Z | 2026-10-05T16:41:55Z | 0h 03m 48.00s | success |
| [37342336634 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37342336634/job/111872246940) | 2026-10-05T16:38:11Z | 2026-10-05T16:45:08Z | 0h 06m 57.00s | success |
| [37418026977 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37418026977/job/112120907132) | 2026-10-06T05:20:20Z | 2026-10-06T05:20:20Z | 0h 00m 00.00s | cancelled |
| [37418027941 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37418027941/job/112120912106) | 2026-10-06T05:20:23Z | 2026-10-06T05:23:34Z | 0h 03m 11.00s | success |
| [37418026951 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37418026951/job/112120907450) | 2026-10-06T05:20:30Z | 2026-10-06T05:27:07Z | 0h 06m 37.00s | success |
| [37418027962 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37418027962/job/112122665741) | 2026-10-06T05:27:16Z | 2026-10-06T05:34:11Z | 0h 06m 55.00s | success |
| [37418779184 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37418779184/job/112123242984) | 2026-10-06T05:29:25Z | 2026-10-06T05:33:04Z | 0h 03m 39.00s | success |
| [37418779183 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37418779183/job/112124486426) | 2026-10-06T05:34:16Z | 2026-10-06T05:37:53Z | 0h 03m 37.00s | success |
| [37420810196 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37420810196/job/112129513057) | 2026-10-06T05:53:34Z | 2026-10-06T05:57:14Z | 0h 03m 40.00s | success |
| [37420810184 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37420810184/job/112129513331) | 2026-10-06T05:53:41Z | 2026-10-06T05:58:50Z | 0h 05m 09.00s | success |
| [37422391807 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37422391807/job/112134420623) | 2026-10-06T06:11:37Z | 2026-10-06T06:16:24Z | 0h 04m 47.00s | success |
| [37422391840 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37422391840/job/112134420522) | 2026-10-06T06:11:45Z | 2026-10-06T06:19:11Z | 0h 07m 26.00s | success |
| [37423640462 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37423640462/job/112138311294) | 2026-10-06T06:25:27Z | 2026-10-06T06:30:09Z | 0h 04m 42.00s | success |
| [37423640458 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37423640458/job/112138311395) | 2026-10-06T06:25:34Z | 2026-10-06T06:32:21Z | 0h 06m 47.00s | success |
| [37426662276 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37426662276/job/112147748441) | 2026-10-06T06:56:55Z | 2026-10-06T07:00:03Z | 0h 03m 08.00s | success |
| [37426662186 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37426662186/job/112147748246) | 2026-10-06T06:56:59Z | 2026-10-06T07:02:42Z | 0h 05m 43.00s | success |
| [37430811813 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37430811813/job/112160975949) | 2026-10-06T07:37:23Z | 2026-10-06T07:40:55Z | 0h 03m 32.00s | success |
| [37430811739 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37430811739/job/112160975686) | 2026-10-06T07:37:31Z | 2026-10-06T07:43:54Z | 0h 06m 23.00s | success |
| [37433196310 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37433196310/job/112168659934) | 2026-10-06T08:00:03Z | 2026-10-06T08:04:43Z | 0h 04m 40.00s | success |
| [37433196098 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37433196098/job/112168659846) | 2026-10-06T08:00:09Z | 2026-10-06T08:06:39Z | 0h 06m 30.00s | success |
| [37434483040 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37434483040/job/112172828807) | 2026-10-06T08:11:44Z | 2026-10-06T08:16:21Z | 0h 04m 37.00s | success |
| [37434483149 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37434483149/job/112172829046) | 2026-10-06T08:11:49Z | 2026-10-06T08:18:19Z | 0h 06m 30.00s | success |
| [37437496880 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37437496880/job/112182809969) | 2026-10-06T08:38:50Z | 2026-10-06T08:41:59Z | 0h 03m 09.00s | success |
| [37437496733 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37437496733/job/112182809005) | 2026-10-06T08:38:56Z | 2026-10-06T08:46:15Z | 0h 07m 19.00s | success |
| [37442227773 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37442227773/job/112198491621) | 2026-10-06T09:20:10Z | 2026-10-06T09:25:27Z | 0h 05m 17.00s | success |
| [37442227696 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37442227696/job/112198490781) | 2026-10-06T09:20:16Z | 2026-10-06T09:26:57Z | 0h 06m 41.00s | success |
| [37446651077 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37446651077/job/112213001979) | 2026-10-06T09:58:42Z | 2026-10-06T10:02:57Z | 0h 04m 15.00s | success |
| [37446651145 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37446651145/job/112213002653) | 2026-10-06T09:58:46Z | 2026-10-06T10:05:51Z | 0h 07m 05.00s | success |
| [37449152012 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37449152012/job/112221178224) | 2026-10-06T10:20:44Z | 2026-10-06T10:24:16Z | 0h 03m 32.00s | success |
| [37449152089 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37449152089/job/112221177997) | 2026-10-06T10:20:50Z | 2026-10-06T10:28:36Z | 0h 07m 46.00s | success |
| [37526100531 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37526100531/job/112483378986) | 2026-10-06T20:23:43Z | 2026-10-06T20:26:49Z | 0h 03m 06.00s | success |
| [37526100468 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37526100468/job/112483379899) | 2026-10-06T20:23:48Z | 2026-10-06T20:30:35Z | 0h 06m 47.00s | success |
| [37527509291 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37527509291/job/112488152741) | 2026-10-06T20:34:49Z | 2026-10-06T20:39:35Z | 0h 04m 46.00s | success |
| [37527509327 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37527509327/job/112488153117) | 2026-10-06T20:34:55Z | 2026-10-06T20:43:56Z | 0h 09m 01.00s | success |
| [37530185712 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37530185712/job/112497249834) | 2026-10-06T20:56:07Z | 2026-10-06T21:00:34Z | 0h 04m 27.00s | failure |
| [37530185860 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37530185860/job/112497250821) | 2026-10-06T20:56:12Z | 2026-10-06T21:02:32Z | 0h 06m 20.00s | failure |
| [37545050717 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37545050717/job/112546939573) | 2026-10-06T23:10:28Z | 2026-10-06T23:13:35Z | 0h 03m 07.00s | cancelled |
| [37545050724 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37545050724/job/112546938811) | 2026-10-06T23:10:35Z | 2026-10-06T23:18:48Z | 0h 08m 13.00s | success |
| [37545348306 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37545348306/job/112547937578) | 2026-10-06T23:13:37Z | 2026-10-06T23:17:27Z | 0h 03m 50.00s | cancelled |
| [37545620862 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37545620862/job/112549155514) | 2026-10-06T23:17:30Z | 2026-10-06T23:20:43Z | 0h 03m 13.00s | success |
| [37545620865 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37545620865/job/112549563786) | 2026-10-06T23:18:56Z | 2026-10-06T23:25:57Z | 0h 07m 01.00s | success |
| [37549527902 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37549527902/job/112561450530) | 2026-10-06T23:59:25Z | 2026-10-07T00:02:21Z | 0h 02m 56.00s | success |
| [37549527909 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37549527909/job/112561449871) | 2026-10-06T23:59:31Z | 2026-10-07T00:05:24Z | 0h 05m 53.00s | failure |
| [37550440636 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37550440636/job/112564352888) | 2026-10-07T00:09:13Z | 2026-10-07T00:13:54Z | 0h 04m 41.00s | success |
| [37550440641 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37550440641/job/112564353163) | 2026-10-07T00:09:18Z | 2026-10-07T00:16:30Z | 0h 07m 12.00s | success |
| [37551464607 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37551464607/job/112567701934) | 2026-10-07T00:20:43Z | 2026-10-07T00:25:26Z | 0h 04m 43.00s | failure |
| [37551464599 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37551464599/job/112567702434) | 2026-10-07T00:20:51Z | 2026-10-07T00:29:04Z | 0h 08m 13.00s | success |
| [37551464607 / linux attempt 2](https://github.com/BelloWare/BelloBox/actions/runs/37551464607/job/112569387798) | 2026-10-07T00:26:43Z | 2026-10-07T00:30:26Z | 0h 03m 43.00s | success |
| [37567749507 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37567749507/job/112619250659) | 2026-10-07T03:39:21Z | 2026-10-07T03:44:18Z | 0h 04m 57.00s | success |
| [37567749475 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37567749475/job/112619250618) | 2026-10-07T03:39:27Z | 2026-10-07T03:46:41Z | 0h 07m 14.00s | success |
| [37569756305 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37569756305/job/112625481856) | 2026-10-07T04:05:10Z | 2026-10-07T04:09:55Z | 0h 04m 45.00s | success |
| [37569756320 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37569756320/job/112625481257) | 2026-10-07T04:05:15Z | 2026-10-07T04:12:10Z | 0h 06m 55.00s | success |
| [37571312929 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37571312929/job/112630355183) | 2026-10-07T04:24:38Z | 2026-10-07T04:28:40Z | 0h 04m 02.00s | success |
| [37571312948 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37571312948/job/112630355195) | 2026-10-07T04:24:42Z | 2026-10-07T04:28:29Z | 0h 03m 47.00s | failure |
| [37571908285 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37571908285/job/112632197947) | 2026-10-07T04:32:00Z | 2026-10-07T04:37:11Z | 0h 05m 11.00s | success |
| [37571908312 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37571908312/job/112632198142) | 2026-10-07T04:32:03Z | 2026-10-07T04:36:51Z | 0h 04m 48.00s | failure |
| [37573777383 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37573777383/job/112638011895) | 2026-10-07T04:55:22Z | 2026-10-07T04:59:08Z | 0h 03m 46.00s | success |
| [37573777479 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37573777479/job/112638012108) | 2026-10-07T04:55:27Z | 2026-10-07T05:00:27Z | 0h 05m 00.00s | failure |
| [37575287239 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37575287239/job/112642690061) | 2026-10-07T05:13:56Z | 2026-10-07T05:18:15Z | 0h 04m 19.00s | success |
| [37575287155 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37575287155/job/112642690044) | 2026-10-07T05:14:25Z | 2026-10-07T05:21:03Z | 0h 06m 38.00s | success |
| [37576314038 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37576314038/job/112645894950) | 2026-10-07T05:26:33Z | 2026-10-07T05:31:40Z | 0h 05m 07.00s | success |
| [37576314121 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37576314121/job/112645894839) | 2026-10-07T05:26:37Z | 2026-10-07T05:33:53Z | 0h 07m 16.00s | success |
| [37579346250 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37579346250/job/112655222210) | 2026-10-07T06:02:20Z | 2026-10-07T06:08:00Z | 0h 05m 40.00s | success |
| [37579346240 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37579346240/job/112655223825) | 2026-10-07T06:02:29Z | 2026-10-07T06:09:56Z | 0h 07m 27.00s | success |
| [37581683197 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37581683197/job/112662571963) | 2026-10-07T06:28:29Z | 2026-10-07T06:32:45Z | 0h 04m 16.00s | success |
| [37581683028 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37581683028/job/112662570607) | 2026-10-07T06:28:34Z | 2026-10-07T06:35:46Z | 0h 07m 12.00s | success |
| [37584275866 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37584275866/job/112670739284) | 2026-10-07T06:56:00Z | 2026-10-07T07:02:32Z | 0h 06m 32.00s | success |
| [37584275876 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37584275876/job/112670739532) | 2026-10-07T06:56:05Z | 2026-10-07T07:02:46Z | 0h 06m 41.00s | success |
| [37591717754 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37591717754/job/112694524883) | 2026-10-07T08:07:48Z | 2026-10-07T08:12:13Z | 0h 04m 25.00s | success |
| [37591717776 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37591717776/job/112694524224) | 2026-10-07T08:07:52Z | 2026-10-07T08:17:46Z | 0h 09m 54.00s | success |
| [37600789001 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37600789001/job/112724365897) | 2026-10-07T09:27:58Z | 2026-10-07T09:34:11Z | 0h 06m 13.00s | success |
| [37600789048 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37600789048/job/112724366359) | 2026-10-07T09:28:01Z | 2026-10-07T09:34:27Z | 0h 06m 26.00s | success |
| [37616462056 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37616462056/job/112775863497) | 2026-10-07T11:47:22Z | 2026-10-07T11:53:38Z | 0h 06m 16.00s | success |
| [37616462042 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37616462042/job/112775863468) | 2026-10-07T11:47:29Z | 2026-10-07T11:56:17Z | 0h 08m 48.00s | failure |
| [37620785940 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37620785940/job/112790232223) | 2026-10-07T12:24:44Z | 2026-10-07T12:30:59Z | 0h 06m 15.00s | success |
| [37620785861 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37620785861/job/112790231021) | 2026-10-07T12:24:49Z | 2026-10-07T12:32:28Z | 0h 07m 39.00s | failure |
| [37623218882 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37623218882/job/112798401781) | 2026-10-07T12:44:47Z | 2026-10-07T12:51:03Z | 0h 06m 16.00s | success |
| [37623218829 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37623218829/job/112798400955) | 2026-10-07T12:44:53Z | 2026-10-07T12:53:56Z | 0h 09m 03.00s | success |
| [37631715456 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37631715456/job/112827399089) | 2026-10-07T13:50:29Z | 2026-10-07T13:56:40Z | 0h 06m 11.00s | success |
| [37631714878 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37631714878/job/112827396039) | 2026-10-07T13:50:32Z | 2026-10-07T13:59:15Z | 0h 08m 43.00s | success |
| [37645491894 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37645491894/job/112875030620) | 2026-10-07T15:38:05Z | 2026-10-07T16:06:21Z | 0h 28m 16.00s | success |
| [37645491886 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37645491886/job/112875030297) | 2026-10-07T15:38:10Z | 2026-10-07T15:48:32Z | 0h 10m 22.00s | success |
| [37662350922 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37662350922/job/112932784763) | 2026-10-07T17:51:49Z | 2026-10-07T18:00:51Z | 0h 09m 02.00s | success |
| [37662351059 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37662351059/job/112932784837) | 2026-10-07T17:51:55Z | 2026-10-07T18:03:54Z | 0h 11m 59.00s | success |
| [37691415417 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37691415417/job/113032247043) | 2026-10-07T21:43:37Z | 2026-10-07T21:45:34Z | 0h 01m 57.00s | failure |
| [37691415517 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37691415517/job/113032248556) | 2026-10-07T21:43:44Z | 2026-10-07T21:51:12Z | 0h 07m 28.00s | failure |
| [37692338614 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37692338614/job/113035374584) | 2026-10-07T21:51:48Z | 2026-10-07T21:59:10Z | 0h 07m 22.00s | failure |
| [37692338775 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37692338775/job/113035375264) | 2026-10-07T21:51:56Z | 2026-10-07T21:59:36Z | 0h 07m 40.00s | failure |
| [37695162854 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37695162854/job/113044965263) | 2026-10-07T22:16:59Z | 2026-10-07T22:28:13Z | 0h 11m 14.00s | success |
| [37695162872 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37695162872/job/113044965213) | 2026-10-07T22:17:03Z | 2026-10-07T22:23:19Z | 0h 06m 16.00s | failure |
| [37719702228 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37719702228/job/113124289955) | 2026-10-08T02:48:51Z | 2026-10-08T03:00:31Z | 0h 11m 40.00s | success |
| [37719702221 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37719702221/job/113124290109) | 2026-10-08T02:48:55Z | 2026-10-08T02:58:48Z | 0h 09m 53.00s | failure |
| [37722023300 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37722023300/job/113131620475) | 2026-10-08T03:17:35Z | 2026-10-08T03:28:56Z | 0h 11m 21.00s | success |
| [37722023231 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37722023231/job/113131619991) | 2026-10-08T03:17:44Z | 2026-10-08T03:31:50Z | 0h 14m 06.00s | success |
| [37731947914 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37731947914/job/113162843486) | 2026-10-08T05:21:19Z | 2026-10-08T05:33:08Z | 0h 11m 49.00s | success |
| [37731947887 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37731947887/job/113162843244) | 2026-10-08T05:21:24Z | 2026-10-08T05:38:53Z | 0h 17m 29.00s | success |
| [37829023111 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37829023111/job/113489230046) | 2026-10-08T19:03:16Z | 2026-10-08T19:12:50Z | 0h 09m 34.00s | success |
| [37829023144 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37829023144/job/113489227527) | 2026-10-08T19:03:20Z | 2026-10-08T19:16:54Z | 0h 13m 34.00s | success |
| [37835689184 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37835689184/job/113511996904) | 2026-10-08T19:56:24Z | 2026-10-08T20:08:26Z | 0h 12m 02.00s | success |
| [37835689023 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37835689023/job/113511996895) | 2026-10-08T19:56:29Z | 2026-10-08T20:15:34Z | 0h 19m 05.00s | success |
| [37839126389 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37839126389/job/113523714800) | 2026-10-08T20:23:51Z | 2026-10-08T20:34:31Z | 0h 10m 40.00s | success |
| [37839126404 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37839126404/job/113523715070) | 2026-10-08T20:23:56Z | 2026-10-08T20:41:08Z | 0h 17m 12.00s | success |
| [37841835463 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37841835463/job/113532851619) | 2026-10-08T20:45:35Z | 2026-10-08T20:53:39Z | 0h 08m 04.00s | success |
| [37841835462 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37841835462/job/113532851124) | 2026-10-08T20:45:42Z | 2026-10-08T21:02:00Z | 0h 16m 18.00s | success |
| [37843283001 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37843283001/job/113537778436) | 2026-10-08T20:57:27Z | 2026-10-08T21:09:00Z | 0h 11m 33.00s | cancelled |
| [37843282942 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37843282942/job/113539688527) | 2026-10-08T21:02:10Z | 2026-10-08T21:20:05Z | 0h 17m 55.00s | success |
| [37844524335 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37844524335/job/113542559830) | 2026-10-08T21:09:03Z | 2026-10-08T21:22:04Z | 0h 13m 01.00s | success |
| [37844524272 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37844524272/job/113547000837) | 2026-10-08T21:20:13Z | 2026-10-08T21:36:07Z | 0h 15m 54.00s | success |
| [37846522075 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37846522075/job/113548651928) | 2026-10-08T21:24:17Z | 2026-10-08T21:36:43Z | 0h 12m 26.00s | success |
| [37846522007 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37846522007/job/113553354945) | 2026-10-08T21:36:18Z | 2026-10-08T21:54:47Z | 0h 18m 29.00s | success |
| [37850905973 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37850905973/job/113563271162) | 2026-10-08T22:02:39Z | 2026-10-08T22:15:25Z | 0h 12m 46.00s | success |
| [37850905866 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37850905866/job/113563270815) | 2026-10-08T22:02:47Z | 2026-10-08T22:18:11Z | 0h 15m 24.00s | success |
| [37854470671 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37854470671/job/113575218595) | 2026-10-08T22:35:55Z | 2026-10-08T22:48:09Z | 0h 12m 14.00s | success |
| [37854470833 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37854470833/job/113575219569) | 2026-10-08T22:36:02Z | 2026-10-08T22:50:16Z | 0h 14m 14.00s | success |
| [37860731377 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37860731377/job/113595554122) | 2026-10-08T23:40:31Z | 2026-10-08T23:52:48Z | 0h 12m 17.00s | success |
| [37860731492 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37860731492/job/113595554508) | 2026-10-08T23:40:37Z | 2026-10-09T00:01:36Z | 0h 20m 59.00s | success |
| [37868924100 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37868924100/job/113622193195) | 2026-10-09T01:15:52Z | 2026-10-09T01:28:29Z | 0h 12m 37.00s | success |
| [37868924095 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37868924095/job/113622193421) | 2026-10-09T01:15:58Z | 2026-10-09T01:32:42Z | 0h 16m 44.00s | success |
| [37872180831 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37872180831/job/113632552290) | 2026-10-09T01:56:39Z | 2026-10-09T02:05:28Z | 0h 08m 49.00s | success |
| [37872180832 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37872180832/job/113632552430) | 2026-10-09T01:56:46Z | 2026-10-09T02:10:14Z | 0h 13m 28.00s | success |
| [37876067479 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37876067479/job/113644839976) | 2026-10-09T02:45:26Z | 2026-10-09T02:57:42Z | 0h 12m 16.00s | success |
| [37876067513 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37876067513/job/113644840500) | 2026-10-09T02:45:32Z | 2026-10-09T02:58:24Z | 0h 12m 52.00s | success |
| [37881955421 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37881955421/job/113663387706) | 2026-10-09T04:01:38Z | 2026-10-09T04:12:03Z | 0h 10m 25.00s | success |
| [37881955423 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37881955423/job/113663387752) | 2026-10-09T04:01:44Z | 2026-10-09T04:16:56Z | 0h 15m 12.00s | success |
| [37889054525 / linux attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37889054525/job/113685587567) | 2026-10-09T05:32:27Z | 2026-10-09T05:45:12Z | 0h 12m 45.00s | success |
| [37889054529 / apple-silicon attempt 1](https://github.com/BelloWare/BelloBox/actions/runs/37889054529/job/113685587388) | 2026-10-09T05:32:33Z | 2026-10-09T05:45:13Z | 0h 12m 40.00s | failure |

## Each preserved timed local/native command

| Activity | UTC start / end | Duration | Outcome / source |
|---|---|---:|---|
| popup-qr-2026-10-09 / failure-diagnosis/native_probe/results/0 | unknown / unknown | 0h 00m 00.15s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/failure-diagnosis.json) |
| popup-qr-2026-10-09 / failure-diagnosis/native_probe/results/1 | unknown / unknown | 0h 00m 00.78s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/failure-diagnosis.json) |
| recording-shutdown-2026-10-07 / app-partitioned-resource | unknown / unknown | 0h 00m 33.54s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/recording-shutdown-2026-10-07/app-partitioned-resource.json) |
| recording-shutdown-2026-10-07 / bypass-physical-drain | unknown / unknown | 0h 00m 23.29s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/recording-shutdown-2026-10-07/detecting-controls/results.json) |
| recording-shutdown-2026-10-07 / bypass-final-window-veto | unknown / unknown | 0h 00m 20.09s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/recording-shutdown-2026-10-07/detecting-controls/results.json) |
| recording-shutdown-2026-10-07 / results/restored_positive | unknown / unknown | 0h 00m 51.39s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/recording-shutdown-2026-10-07/detecting-controls/results.json) |
| native-movie-readback-2026-10-09 / platform-movie | 2026-10-08T22:50:48.544636+00:00 / 2026-10-08T22:50:49.797075+00:00 | 0h 00m 01.25s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/baseline/assessment-validation.json) |
| native-movie-readback-2026-10-09 / app-native-movie | 2026-10-08T22:50:49.797711+00:00 / 2026-10-08T22:50:52.203355+00:00 | 0h 00m 02.41s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/baseline/app-native-movie-result.json) |
| native-movie-readback-2026-10-09 / core-regression-r1 | 2026-10-08T23:14:00.747692+00:00 / 2026-10-08T23:14:09.286788+00:00 | 0h 00m 08.54s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/core-regression-r1.json) |
| native-movie-readback-2026-10-09 / core-regression-r2 | 2026-10-08T23:20:39.323446+00:00 / 2026-10-08T23:21:07.329320+00:00 | 0h 00m 28.01s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/core-regression-r2.json) |
| native-movie-readback-2026-10-09 / core-gif-r1 | 2026-10-08T23:21:43.871874+00:00 / 2026-10-08T23:22:14.434001+00:00 | 0h 00m 30.56s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/core-gif-r1.json) |
| native-movie-readback-2026-10-09 / core-gif-r2 | 2026-10-08T23:22:57.864717+00:00 / 2026-10-08T23:23:20.274465+00:00 | 0h 00m 22.41s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/core-gif-r2.json) |
| native-movie-readback-2026-10-09 / app-gif-r1 | 2026-10-08T23:23:44.214524+00:00 / 2026-10-08T23:24:18.996988+00:00 | 0h 00m 34.78s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/app-gif-r1.json) |
| native-movie-readback-2026-10-09 / fmt-r1 | 2026-10-08T23:24:36.998544+00:00 / 2026-10-08T23:24:38.906361+00:00 | 0h 00m 01.91s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/fmt-r1.json) |
| native-movie-readback-2026-10-09 / clippy-r1 | 2026-10-08T23:25:27.343493+00:00 / 2026-10-08T23:25:31.161973+00:00 | 0h 00m 03.82s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/clippy-r1.json) |
| native-movie-readback-2026-10-09 / clippy-base-r1 | 2026-10-08T23:26:40.984046+00:00 / 2026-10-08T23:26:44.270644+00:00 | 0h 00m 03.29s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/clippy-base-r1.json) |
| native-movie-readback-2026-10-09 / clippy-scoped-r1 | 2026-10-08T23:27:42.024781+00:00 / 2026-10-08T23:27:45.143289+00:00 | 0h 00m 03.12s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-movie-readback-2026-10-09/repair/clippy-scoped-r1.json) |
| native-first-following-2026-10-09 / swift-compile | 2026-10-08T23:58:33.067826+00:00 / unknown | 0h 00m 01.61s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/swift-r1/swift-probe-execution.json) |
| native-first-following-2026-10-09 / swift-timing | 2026-10-08T23:58:34.675703+00:00 / unknown | 0h 00m 00.72s | [nonzero exit 1](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/swift-r1/swift-probe-execution.json) |
| native-first-following-2026-10-09 / swift-compile | 2026-10-08T23:59:32.411855+00:00 / unknown | 0h 00m 00.64s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/swift-probe-execution.json) |
| native-first-following-2026-10-09 / swift-timing | 2026-10-08T23:59:33.055980+00:00 / unknown | 0h 00m 00.63s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/swift-probe-execution.json) |
| native-first-following-2026-10-09 / platform-movie | 2026-10-09T00:02:28.515350+00:00 / 2026-10-09T00:02:36.842278+00:00 | 0h 00m 08.33s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/final-validation.json) |
| native-first-following-2026-10-09 / core-gif | 2026-10-09T00:02:36.843045+00:00 / 2026-10-09T00:02:42.910743+00:00 | 0h 00m 06.07s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/core-gif-result.json) |
| native-first-following-2026-10-09 / app-native-host | 2026-10-09T00:02:42.911341+00:00 / 2026-10-09T00:03:02.922210+00:00 | 0h 00m 20.01s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-2026-10-09/app-native-host-result.json) |
| native-first-following-rust-2026-10-09 / format | 2026-10-09T02:07:26.977001+00:00 / 2026-10-09T02:07:28.768288+00:00 | 0h 00m 01.79s | [nonzero exit 1](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/format-result.json) |
| native-first-following-rust-2026-10-09 / format-final | 2026-10-09T02:08:33.547874+00:00 / 2026-10-09T02:08:35.267705+00:00 | 0h 00m 01.72s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/format-final-result.json) |
| native-first-following-rust-2026-10-09 / platform-movie | 2026-10-09T02:08:35.268227+00:00 / 2026-10-09T02:08:43.631645+00:00 | 0h 00m 08.36s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/final-validation.json) |
| native-first-following-rust-2026-10-09 / core-gif | 2026-10-09T02:08:43.632338+00:00 / 2026-10-09T02:08:50.520541+00:00 | 0h 00m 06.89s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/core-gif-result.json) |
| native-first-following-rust-2026-10-09 / app-native-host | 2026-10-09T02:08:50.521201+00:00 / 2026-10-09T02:09:12.713136+00:00 | 0h 00m 22.19s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/app-native-host-result.json) |
| native-first-following-rust-2026-10-09 / strict-clippy | 2026-10-09T02:09:12.714028+00:00 / 2026-10-09T02:09:27.030693+00:00 | 0h 00m 14.32s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/final-validation.json) |
| native-first-following-rust-2026-10-09 / base-strict-clippy | 2026-10-09T02:11:29.249207+00:00 / 2026-10-09T02:11:32.793920+00:00 | 0h 00m 03.54s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/base-strict-clippy-result.json) |
| native-first-following-rust-2026-10-09 / scoped-clippy | 2026-10-09T02:11:32.794373+00:00 / 2026-10-09T02:11:40.958596+00:00 | 0h 00m 08.16s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/final-validation.json) |
| native-first-following-rust-2026-10-09 / base-scoped-clippy | 2026-10-09T02:12:25.700201+00:00 / 2026-10-09T02:12:32.081592+00:00 | 0h 00m 06.38s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/base-scoped-clippy-result.json) |
| native-first-following-rust-2026-10-09 / platform-scoped-clippy | 2026-10-09T02:12:32.082193+00:00 / 2026-10-09T02:12:40.642153+00:00 | 0h 00m 08.56s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/native-first-following-rust-2026-10-09/final-validation.json) |
| palette-qr-2026-10-09 / build-default | 2026-10-09T03:02:01.851439+00:00 / 2026-10-09T03:02:33.507997+00:00 | 0h 00m 31.66s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/build-default-result.json) |
| palette-qr-2026-10-09 / build-minimal | 2026-10-09T03:02:33.508611+00:00 / 2026-10-09T03:02:43.799337+00:00 | 0h 00m 10.29s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/build-minimal-result.json) |
| palette-qr-2026-10-09 / build-movie-fixtures | 2026-10-09T03:02:43.800060+00:00 / 2026-10-09T03:02:52.581401+00:00 | 0h 00m 08.78s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/build-movie-fixtures-result.json) |
| palette-qr-2026-10-09 / core-qr | 2026-10-09T03:02:52.582126+00:00 / 2026-10-09T03:03:01.188356+00:00 | 0h 00m 08.61s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/core-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-preview | 2026-10-09T03:03:01.188941+00:00 / 2026-10-09T03:03:16.162373+00:00 | 0h 00m 14.97s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/default-qr-preview-result.json) |
| palette-qr-2026-10-09 / default-launcher-qr | 2026-10-09T03:03:16.163137+00:00 / 2026-10-09T03:03:18.305589+00:00 | 0h 00m 02.14s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/default-launcher-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-jobs | 2026-10-09T03:03:18.306230+00:00 / 2026-10-09T03:03:19.284194+00:00 | 0h 00m 00.98s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/default-qr-jobs-result.json) |
| palette-qr-2026-10-09 / minimal-qr-preview | 2026-10-09T03:03:19.284853+00:00 / 2026-10-09T03:03:32.455805+00:00 | 0h 00m 13.17s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/minimal-qr-preview-result.json) |
| palette-qr-2026-10-09 / minimal-launcher-qr | 2026-10-09T03:03:32.456510+00:00 / 2026-10-09T03:03:34.400299+00:00 | 0h 00m 01.94s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/minimal-launcher-qr-result.json) |
| palette-qr-2026-10-09 / minimal-qr-jobs | 2026-10-09T03:03:34.401053+00:00 / 2026-10-09T03:03:35.426974+00:00 | 0h 00m 01.03s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/minimal-qr-jobs-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-preview | 2026-10-09T03:03:35.427751+00:00 / 2026-10-09T03:03:49.320451+00:00 | 0h 00m 13.89s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/movie-fixtures-qr-preview-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-launcher-qr | 2026-10-09T03:03:49.321030+00:00 / 2026-10-09T03:03:51.318032+00:00 | 0h 00m 02.00s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/movie-fixtures-launcher-qr-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-jobs | 2026-10-09T03:03:51.318642+00:00 / 2026-10-09T03:03:52.346881+00:00 | 0h 00m 01.03s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/movie-fixtures-qr-jobs-result.json) |
| palette-qr-2026-10-09 / format | 2026-10-09T03:03:52.347502+00:00 / 2026-10-09T03:03:54.244939+00:00 | 0h 00m 01.90s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/format-result.json) |
| palette-qr-2026-10-09 / candidate-strict-clippy | 2026-10-09T03:03:54.245585+00:00 / 2026-10-09T03:03:57.678392+00:00 | 0h 00m 03.43s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/candidate-strict-clippy-result.json) |
| palette-qr-2026-10-09 / candidate-app-scoped-clippy | 2026-10-09T03:03:57.678897+00:00 / 2026-10-09T03:04:04.039183+00:00 | 0h 00m 06.36s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/candidate-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / parent-strict-clippy | 2026-10-09T03:04:04.039828+00:00 / 2026-10-09T03:04:06.317030+00:00 | 0h 00m 02.28s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/parent-strict-clippy-result.json) |
| palette-qr-2026-10-09 / parent-app-scoped-clippy | 2026-10-09T03:04:06.317735+00:00 / 2026-10-09T03:04:13.468861+00:00 | 0h 00m 07.15s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3976/parent-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / build-default | 2026-10-09T03:25:41.315759+00:00 / 2026-10-09T03:25:58.971582+00:00 | 0h 00m 17.66s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/build-default-result.json) |
| palette-qr-2026-10-09 / build-minimal | 2026-10-09T03:25:58.975467+00:00 / 2026-10-09T03:26:08.787264+00:00 | 0h 00m 09.81s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/build-minimal-result.json) |
| palette-qr-2026-10-09 / build-movie-fixtures | 2026-10-09T03:26:08.796164+00:00 / 2026-10-09T03:26:17.926987+00:00 | 0h 00m 09.13s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/build-movie-fixtures-result.json) |
| palette-qr-2026-10-09 / core-qr | 2026-10-09T03:26:17.927961+00:00 / 2026-10-09T03:26:27.743256+00:00 | 0h 00m 09.82s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/core-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-preview | 2026-10-09T03:26:27.745699+00:00 / 2026-10-09T03:26:48.448234+00:00 | 0h 00m 20.70s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/default-qr-preview-result.json) |
| palette-qr-2026-10-09 / default-launcher-qr | 2026-10-09T03:26:48.449246+00:00 / 2026-10-09T03:26:52.302777+00:00 | 0h 00m 03.85s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/default-launcher-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-jobs | 2026-10-09T03:26:52.303458+00:00 / 2026-10-09T03:26:53.472036+00:00 | 0h 00m 01.17s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/default-qr-jobs-result.json) |
| palette-qr-2026-10-09 / minimal-qr-preview | 2026-10-09T03:26:53.473077+00:00 / 2026-10-09T03:27:09.697165+00:00 | 0h 00m 16.22s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/minimal-qr-preview-result.json) |
| palette-qr-2026-10-09 / minimal-launcher-qr | 2026-10-09T03:27:09.698001+00:00 / 2026-10-09T03:27:13.231675+00:00 | 0h 00m 03.53s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/minimal-launcher-qr-result.json) |
| palette-qr-2026-10-09 / minimal-qr-jobs | 2026-10-09T03:27:13.232334+00:00 / 2026-10-09T03:27:14.377878+00:00 | 0h 00m 01.15s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/minimal-qr-jobs-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-preview | 2026-10-09T03:27:14.378563+00:00 / 2026-10-09T03:27:30.675984+00:00 | 0h 00m 16.30s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/movie-fixtures-qr-preview-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-launcher-qr | 2026-10-09T03:27:30.676999+00:00 / 2026-10-09T03:27:32.470822+00:00 | 0h 00m 01.79s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/movie-fixtures-launcher-qr-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-jobs | 2026-10-09T03:27:32.471677+00:00 / 2026-10-09T03:27:33.447573+00:00 | 0h 00m 00.98s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/movie-fixtures-qr-jobs-result.json) |
| palette-qr-2026-10-09 / format | 2026-10-09T03:27:33.448439+00:00 / 2026-10-09T03:27:35.294606+00:00 | 0h 00m 01.85s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/format-result.json) |
| palette-qr-2026-10-09 / candidate-strict-clippy | 2026-10-09T03:27:35.295230+00:00 / 2026-10-09T03:27:39.512564+00:00 | 0h 00m 04.22s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/candidate-strict-clippy-result.json) |
| palette-qr-2026-10-09 / candidate-app-scoped-clippy | 2026-10-09T03:27:39.513325+00:00 / 2026-10-09T03:27:45.875686+00:00 | 0h 00m 06.36s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/candidate-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / parent-strict-clippy | 2026-10-09T03:27:45.876152+00:00 / 2026-10-09T03:27:48.348582+00:00 | 0h 00m 02.47s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/parent-strict-clippy-result.json) |
| palette-qr-2026-10-09 / parent-app-scoped-clippy | 2026-10-09T03:27:48.349144+00:00 / 2026-10-09T03:27:54.751470+00:00 | 0h 00m 06.40s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-3ae/parent-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / build-default | 2026-10-09T03:37:50.432428+00:00 / 2026-10-09T03:38:08.898560+00:00 | 0h 00m 18.47s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/build-default-result.json) |
| palette-qr-2026-10-09 / build-minimal | 2026-10-09T03:38:08.899229+00:00 / 2026-10-09T03:38:19.511998+00:00 | 0h 00m 10.61s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/build-minimal-result.json) |
| palette-qr-2026-10-09 / build-movie-fixtures | 2026-10-09T03:38:19.512828+00:00 / 2026-10-09T03:38:28.172843+00:00 | 0h 00m 08.66s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/build-movie-fixtures-result.json) |
| palette-qr-2026-10-09 / core-qr | 2026-10-09T03:38:28.173471+00:00 / 2026-10-09T03:38:36.852423+00:00 | 0h 00m 08.68s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/core-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-preview | 2026-10-09T03:38:36.853185+00:00 / 2026-10-09T03:38:52.424309+00:00 | 0h 00m 15.57s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/default-qr-preview-result.json) |
| palette-qr-2026-10-09 / default-launcher-qr | 2026-10-09T03:38:52.425049+00:00 / 2026-10-09T03:38:55.495732+00:00 | 0h 00m 03.07s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/default-launcher-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-jobs | 2026-10-09T03:38:55.496499+00:00 / 2026-10-09T03:38:56.587477+00:00 | 0h 00m 01.09s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/default-qr-jobs-result.json) |
| palette-qr-2026-10-09 / minimal-qr-preview | 2026-10-09T03:38:56.588188+00:00 / 2026-10-09T03:39:10.518071+00:00 | 0h 00m 13.93s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/minimal-qr-preview-result.json) |
| palette-qr-2026-10-09 / minimal-launcher-qr | 2026-10-09T03:39:10.519127+00:00 / 2026-10-09T03:39:13.338764+00:00 | 0h 00m 02.82s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/minimal-launcher-qr-result.json) |
| palette-qr-2026-10-09 / minimal-qr-jobs | 2026-10-09T03:39:13.339561+00:00 / 2026-10-09T03:39:14.379220+00:00 | 0h 00m 01.04s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/minimal-qr-jobs-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-preview | 2026-10-09T03:39:14.379840+00:00 / 2026-10-09T03:39:29.551535+00:00 | 0h 00m 15.17s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/movie-fixtures-qr-preview-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-launcher-qr | 2026-10-09T03:39:29.552261+00:00 / 2026-10-09T03:39:32.547997+00:00 | 0h 00m 02.99s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/movie-fixtures-launcher-qr-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-jobs | 2026-10-09T03:39:32.548628+00:00 / 2026-10-09T03:39:33.582684+00:00 | 0h 00m 01.03s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/movie-fixtures-qr-jobs-result.json) |
| palette-qr-2026-10-09 / format | 2026-10-09T03:39:33.583391+00:00 / 2026-10-09T03:39:35.470029+00:00 | 0h 00m 01.89s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/format-result.json) |
| palette-qr-2026-10-09 / candidate-strict-clippy | 2026-10-09T03:39:35.471053+00:00 / 2026-10-09T03:39:39.038682+00:00 | 0h 00m 03.57s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/candidate-strict-clippy-result.json) |
| palette-qr-2026-10-09 / candidate-app-scoped-clippy | 2026-10-09T03:39:39.039396+00:00 / 2026-10-09T03:39:46.031008+00:00 | 0h 00m 06.99s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/candidate-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / parent-strict-clippy | 2026-10-09T03:39:46.031708+00:00 / 2026-10-09T03:39:48.356694+00:00 | 0h 00m 02.32s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/parent-strict-clippy-result.json) |
| palette-qr-2026-10-09 / parent-app-scoped-clippy | 2026-10-09T03:39:48.357277+00:00 / 2026-10-09T03:39:54.805406+00:00 | 0h 00m 06.45s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-83ae/parent-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / build-default | 2026-10-09T03:51:46.597646+00:00 / 2026-10-09T03:52:03.874075+00:00 | 0h 00m 17.28s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/build-default-result.json) |
| palette-qr-2026-10-09 / build-minimal | 2026-10-09T03:52:03.874745+00:00 / 2026-10-09T03:52:14.047508+00:00 | 0h 00m 10.17s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/build-minimal-result.json) |
| palette-qr-2026-10-09 / build-movie-fixtures | 2026-10-09T03:52:14.048220+00:00 / 2026-10-09T03:52:22.766789+00:00 | 0h 00m 08.72s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/build-movie-fixtures-result.json) |
| palette-qr-2026-10-09 / core-qr | 2026-10-09T03:52:22.767456+00:00 / 2026-10-09T03:52:31.644285+00:00 | 0h 00m 08.88s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/core-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-preview | 2026-10-09T03:52:31.644953+00:00 / 2026-10-09T03:52:47.222089+00:00 | 0h 00m 15.58s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/default-qr-preview-result.json) |
| palette-qr-2026-10-09 / default-launcher-qr | 2026-10-09T03:52:47.222839+00:00 / 2026-10-09T03:52:49.678917+00:00 | 0h 00m 02.46s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/default-launcher-qr-result.json) |
| palette-qr-2026-10-09 / default-qr-jobs | 2026-10-09T03:52:49.679575+00:00 / 2026-10-09T03:52:50.709571+00:00 | 0h 00m 01.03s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/default-qr-jobs-result.json) |
| palette-qr-2026-10-09 / minimal-qr-preview | 2026-10-09T03:52:50.710277+00:00 / 2026-10-09T03:53:04.034183+00:00 | 0h 00m 13.32s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/minimal-qr-preview-result.json) |
| palette-qr-2026-10-09 / minimal-launcher-qr | 2026-10-09T03:53:04.035120+00:00 / 2026-10-09T03:53:07.085843+00:00 | 0h 00m 03.05s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/minimal-launcher-qr-result.json) |
| palette-qr-2026-10-09 / minimal-qr-jobs | 2026-10-09T03:53:07.086641+00:00 / 2026-10-09T03:53:08.098273+00:00 | 0h 00m 01.01s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/minimal-qr-jobs-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-preview | 2026-10-09T03:53:08.098937+00:00 / 2026-10-09T03:53:21.767662+00:00 | 0h 00m 13.67s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/movie-fixtures-qr-preview-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-launcher-qr | 2026-10-09T03:53:21.768470+00:00 / 2026-10-09T03:53:24.680415+00:00 | 0h 00m 02.91s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/movie-fixtures-launcher-qr-result.json) |
| palette-qr-2026-10-09 / movie-fixtures-qr-jobs | 2026-10-09T03:53:24.681083+00:00 / 2026-10-09T03:53:25.683077+00:00 | 0h 00m 01.00s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/movie-fixtures-qr-jobs-result.json) |
| palette-qr-2026-10-09 / format | 2026-10-09T03:53:25.683839+00:00 / 2026-10-09T03:53:27.655572+00:00 | 0h 00m 01.97s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/format-result.json) |
| palette-qr-2026-10-09 / candidate-strict-clippy | 2026-10-09T03:53:27.656394+00:00 / 2026-10-09T03:53:30.832005+00:00 | 0h 00m 03.18s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/candidate-strict-clippy-result.json) |
| palette-qr-2026-10-09 / candidate-app-scoped-clippy | 2026-10-09T03:53:30.832637+00:00 / 2026-10-09T03:53:37.305288+00:00 | 0h 00m 06.47s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/candidate-app-scoped-clippy-result.json) |
| palette-qr-2026-10-09 / parent-strict-clippy | 2026-10-09T03:53:37.305903+00:00 / 2026-10-09T03:53:39.504519+00:00 | 0h 00m 02.20s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/parent-strict-clippy-result.json) |
| palette-qr-2026-10-09 / parent-app-scoped-clippy | 2026-10-09T03:53:39.505110+00:00 / 2026-10-09T03:53:45.720204+00:00 | 0h 00m 06.21s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/palette-qr-2026-10-09/native/qr-native-45d5/parent-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / build-default | 2026-10-09T04:56:36.959696+00:00 / 2026-10-09T04:56:54.219143+00:00 | 0h 00m 17.26s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/build-default-result.json) |
| popup-qr-2026-10-09 / build-minimal | 2026-10-09T04:56:54.219822+00:00 / 2026-10-09T04:57:04.136241+00:00 | 0h 00m 09.92s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/build-minimal-result.json) |
| popup-qr-2026-10-09 / build-movie-fixtures | 2026-10-09T04:57:04.136992+00:00 / 2026-10-09T04:57:12.963596+00:00 | 0h 00m 08.83s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/build-movie-fixtures-result.json) |
| popup-qr-2026-10-09 / inventory-default | 2026-10-09T04:57:12.964371+00:00 / 2026-10-09T04:57:27.345449+00:00 | 0h 00m 14.38s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/inventory-default-result.json) |
| popup-qr-2026-10-09 / default-qr | 2026-10-09T04:57:27.346810+00:00 / 2026-10-09T04:57:33.324455+00:00 | 0h 00m 05.98s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/default-qr-result.json) |
| popup-qr-2026-10-09 / default-launcher | 2026-10-09T04:57:33.325551+00:00 / 2026-10-09T04:57:37.650667+00:00 | 0h 00m 04.32s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/default-launcher-result.json) |
| popup-qr-2026-10-09 / inventory-minimal | 2026-10-09T04:57:37.651414+00:00 / 2026-10-09T04:57:50.002230+00:00 | 0h 00m 12.35s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/inventory-minimal-result.json) |
| popup-qr-2026-10-09 / minimal-qr | 2026-10-09T04:57:50.005106+00:00 / 2026-10-09T04:57:56.087102+00:00 | 0h 00m 06.08s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/minimal-qr-result.json) |
| popup-qr-2026-10-09 / minimal-launcher | 2026-10-09T04:57:56.088499+00:00 / 2026-10-09T04:58:00.562369+00:00 | 0h 00m 04.47s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/minimal-launcher-result.json) |
| popup-qr-2026-10-09 / inventory-movie-fixtures | 2026-10-09T04:58:00.563398+00:00 / 2026-10-09T04:58:14.598705+00:00 | 0h 00m 14.03s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/inventory-movie-fixtures-result.json) |
| popup-qr-2026-10-09 / movie-fixtures-qr | 2026-10-09T04:58:14.601781+00:00 / 2026-10-09T04:58:21.773524+00:00 | 0h 00m 07.17s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/movie-fixtures-qr-result.json) |
| popup-qr-2026-10-09 / movie-fixtures-launcher | 2026-10-09T04:58:21.774479+00:00 / 2026-10-09T04:58:25.884987+00:00 | 0h 00m 04.11s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/movie-fixtures-launcher-result.json) |
| popup-qr-2026-10-09 / inventory-core-qr | 2026-10-09T04:58:25.886044+00:00 / 2026-10-09T04:58:32.951218+00:00 | 0h 00m 07.06s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/inventory-core-qr-result.json) |
| popup-qr-2026-10-09 / core-qr | 2026-10-09T04:58:32.952311+00:00 / 2026-10-09T04:58:35.329709+00:00 | 0h 00m 02.38s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/core-qr-result.json) |
| popup-qr-2026-10-09 / format | 2026-10-09T04:58:35.330459+00:00 / 2026-10-09T04:58:37.303814+00:00 | 0h 00m 01.97s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/format-result.json) |
| popup-qr-2026-10-09 / candidate-default-strict-clippy | 2026-10-09T04:58:37.304549+00:00 / 2026-10-09T04:58:40.373272+00:00 | 0h 00m 03.07s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/candidate-default-strict-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-default-app-scoped-clippy | 2026-10-09T04:58:40.373735+00:00 / 2026-10-09T04:58:47.138214+00:00 | 0h 00m 06.76s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/candidate-default-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-minimal-strict-clippy | 2026-10-09T04:58:47.138839+00:00 / 2026-10-09T04:58:49.095471+00:00 | 0h 00m 01.96s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/candidate-minimal-strict-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-minimal-app-scoped-clippy | 2026-10-09T04:58:49.096147+00:00 / 2026-10-09T04:58:55.455395+00:00 | 0h 00m 06.36s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/candidate-minimal-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-movie-fixtures-strict-clippy | 2026-10-09T04:58:55.455989+00:00 / 2026-10-09T04:58:57.780536+00:00 | 0h 00m 02.32s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/candidate-movie-fixtures-strict-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-movie-fixtures-app-scoped-clippy | 2026-10-09T04:58:57.781086+00:00 / 2026-10-09T04:59:04.310244+00:00 | 0h 00m 06.53s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/candidate-movie-fixtures-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / clean-before-baseline-lint | 2026-10-09T04:59:04.310778+00:00 / 2026-10-09T04:59:05.085473+00:00 | 0h 00m 00.77s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/clean-before-baseline-lint-result.json) |
| popup-qr-2026-10-09 / baseline-default-strict-clippy | 2026-10-09T04:59:05.086130+00:00 / 2026-10-09T04:59:07.605277+00:00 | 0h 00m 02.52s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/baseline-default-strict-clippy-result.json) |
| popup-qr-2026-10-09 / baseline-default-app-scoped-clippy | 2026-10-09T04:59:07.605786+00:00 / 2026-10-09T04:59:14.157516+00:00 | 0h 00m 06.55s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/baseline-default-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / baseline-minimal-strict-clippy | 2026-10-09T04:59:14.158120+00:00 / 2026-10-09T04:59:16.045340+00:00 | 0h 00m 01.89s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/baseline-minimal-strict-clippy-result.json) |
| popup-qr-2026-10-09 / baseline-minimal-app-scoped-clippy | 2026-10-09T04:59:16.045888+00:00 / 2026-10-09T04:59:22.186467+00:00 | 0h 00m 06.14s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/baseline-minimal-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / baseline-movie-fixtures-strict-clippy | 2026-10-09T04:59:22.187210+00:00 / 2026-10-09T04:59:24.408868+00:00 | 0h 00m 02.22s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/baseline-movie-fixtures-strict-clippy-result.json) |
| popup-qr-2026-10-09 / baseline-movie-fixtures-app-scoped-clippy | 2026-10-09T04:59:24.409471+00:00 / 2026-10-09T04:59:31.068265+00:00 | 0h 00m 06.66s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/baseline-movie-fixtures-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / reconstruct-candidate-inventory | 2026-10-09T05:03:10.101538+00:00 / 2026-10-09T05:03:30.797339+00:00 | 0h 00m 20.70s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/reconstruct-candidate-inventory-result.json) |
| popup-qr-2026-10-09 / diagnostic-candidate-fresh | 2026-10-09T05:03:56.701370+00:00 / 2026-10-09T05:04:00.529372+00:00 | 0h 00m 03.83s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/diagnostic-candidate-fresh-result.json) |
| popup-qr-2026-10-09 / diagnostic-candidate-learned | 2026-10-09T05:04:00.531512+00:00 / 2026-10-09T05:04:04.299809+00:00 | 0h 00m 03.77s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/pre-repair-4589/diagnostic-candidate-learned-result.json) |
| popup-qr-2026-10-09 / negative-inventory-default | 2026-10-09T05:12:28.147582+00:00 / 2026-10-09T05:12:52.758706+00:00 | 0h 00m 24.61s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/negative-evidence/negative-inventory-default-result.json) |
| popup-qr-2026-10-09 / negative-delayed-read-regressions | 2026-10-09T05:12:52.760111+00:00 / 2026-10-09T05:12:54.303763+00:00 | 0h 00m 01.54s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/validation.json) |
| popup-qr-2026-10-09 / build-default | 2026-10-09T05:13:32.990803+00:00 / 2026-10-09T05:13:45.152176+00:00 | 0h 00m 12.16s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/build-default-result.json) |
| popup-qr-2026-10-09 / build-minimal | 2026-10-09T05:13:45.152886+00:00 / 2026-10-09T05:13:56.206937+00:00 | 0h 00m 11.05s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/build-minimal-result.json) |
| popup-qr-2026-10-09 / build-movie-fixtures | 2026-10-09T05:13:56.207844+00:00 / 2026-10-09T05:14:05.082150+00:00 | 0h 00m 08.87s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/build-movie-fixtures-result.json) |
| popup-qr-2026-10-09 / inventory-default | 2026-10-09T05:14:05.083206+00:00 / 2026-10-09T05:14:19.320183+00:00 | 0h 00m 14.24s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/inventory-default-result.json) |
| popup-qr-2026-10-09 / default-qr | 2026-10-09T05:14:19.321766+00:00 / 2026-10-09T05:14:25.499377+00:00 | 0h 00m 06.18s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/default-qr-result.json) |
| popup-qr-2026-10-09 / default-launcher | 2026-10-09T05:14:25.500412+00:00 / 2026-10-09T05:14:30.297669+00:00 | 0h 00m 04.80s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/default-launcher-result.json) |
| popup-qr-2026-10-09 / inventory-minimal | 2026-10-09T05:14:30.319584+00:00 / 2026-10-09T05:14:43.361083+00:00 | 0h 00m 13.02s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/inventory-minimal-result.json) |
| popup-qr-2026-10-09 / minimal-qr | 2026-10-09T05:14:43.363787+00:00 / 2026-10-09T05:14:49.384194+00:00 | 0h 00m 06.02s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/minimal-qr-result.json) |
| popup-qr-2026-10-09 / minimal-launcher | 2026-10-09T05:14:49.385329+00:00 / 2026-10-09T05:14:54.090912+00:00 | 0h 00m 04.70s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/minimal-launcher-result.json) |
| popup-qr-2026-10-09 / inventory-movie-fixtures | 2026-10-09T05:14:54.091684+00:00 / 2026-10-09T05:15:07.353658+00:00 | 0h 00m 13.26s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/inventory-movie-fixtures-result.json) |
| popup-qr-2026-10-09 / movie-fixtures-qr | 2026-10-09T05:15:07.355115+00:00 / 2026-10-09T05:15:12.632431+00:00 | 0h 00m 05.28s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/movie-fixtures-qr-result.json) |
| popup-qr-2026-10-09 / movie-fixtures-launcher | 2026-10-09T05:15:12.633444+00:00 / 2026-10-09T05:15:17.425985+00:00 | 0h 00m 04.79s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/movie-fixtures-launcher-result.json) |
| popup-qr-2026-10-09 / inventory-core-qr | 2026-10-09T05:15:17.426761+00:00 / 2026-10-09T05:15:23.635092+00:00 | 0h 00m 06.21s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/inventory-core-qr-result.json) |
| popup-qr-2026-10-09 / core-qr | 2026-10-09T05:15:23.636100+00:00 / 2026-10-09T05:15:25.949659+00:00 | 0h 00m 02.31s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/core-qr-result.json) |
| popup-qr-2026-10-09 / format | 2026-10-09T05:15:25.950309+00:00 / 2026-10-09T05:15:27.838013+00:00 | 0h 00m 01.89s | [success](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/format-result.json) |
| popup-qr-2026-10-09 / candidate-default-strict-clippy | 2026-10-09T05:15:27.839040+00:00 / 2026-10-09T05:15:30.928707+00:00 | 0h 00m 03.09s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/candidate-default-strict-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-default-app-scoped-clippy | 2026-10-09T05:15:30.929398+00:00 / 2026-10-09T05:15:37.811927+00:00 | 0h 00m 06.88s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/candidate-default-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-minimal-strict-clippy | 2026-10-09T05:15:37.812516+00:00 / 2026-10-09T05:15:39.766888+00:00 | 0h 00m 01.95s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/candidate-minimal-strict-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-minimal-app-scoped-clippy | 2026-10-09T05:15:39.767410+00:00 / 2026-10-09T05:15:46.055283+00:00 | 0h 00m 06.29s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/candidate-minimal-app-scoped-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-movie-fixtures-strict-clippy | 2026-10-09T05:15:46.055926+00:00 / 2026-10-09T05:15:48.357942+00:00 | 0h 00m 02.30s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/candidate-movie-fixtures-strict-clippy-result.json) |
| popup-qr-2026-10-09 / candidate-movie-fixtures-app-scoped-clippy | 2026-10-09T05:15:48.358485+00:00 / 2026-10-09T05:15:54.870116+00:00 | 0h 00m 06.51s | [nonzero exit 101](https://github.com/BelloWare/BelloBox/blob/0e13ee6def214425865759591c37539429530d71/rust/docs/validation/popup-qr-2026-10-09/native/repaired-4589/evidence/candidate-movie-fixtures-app-scoped-clippy-result.json) |
| shared-find-editor-2026-10-09 / format | 2026-10-09T05:22:59.610107+00:00 / 2026-10-09T05:23:01.461582+00:00 | 0h 00m 01.85s | success; unpublished receipt 2c4f8ba5b53f6bbf (public link unavailable) |
| shared-find-editor-2026-10-09 / inventory | 2026-10-09T05:23:01.462186+00:00 / 2026-10-09T05:24:08.680739+00:00 | 0h 01m 07.22s | success; unpublished receipt 820ee26d09bec870 (public link unavailable) |
| shared-find-editor-2026-10-09 / library-tests | 2026-10-09T05:24:08.681629+00:00 / 2026-10-09T05:24:10.227965+00:00 | 0h 00m 01.55s | success; unpublished receipt 46eccb63855c1efe (public link unavailable) |
| shared-find-editor-2026-10-09 / strict-clippy | 2026-10-09T05:24:10.228765+00:00 / 2026-10-09T05:24:35.059802+00:00 | 0h 00m 24.83s | success; unpublished receipt 4ffa0c0823c21873 (public link unavailable) |
| shared-find-editor-2026-10-09 / clean-before-ordinary-library | 2026-10-09T05:24:35.060453+00:00 / 2026-10-09T05:24:35.784474+00:00 | 0h 00m 00.72s | success; unpublished receipt 40f2dbf753c14fae (public link unavailable) |
| shared-find-editor-2026-10-09 / ordinary-library | 2026-10-09T05:24:35.785161+00:00 / 2026-10-09T05:25:00.584008+00:00 | 0h 00m 24.80s | success; unpublished receipt c245631805ae1110 (public link unavailable) |

## Cargo phase evidence

507 separately reported phase durations are preserved item-by-item in duration-data.json with log line links. They are excluded from total command/runner accounting because they are nested and cannot establish full command wall time. “Finished … in Xm Ys” is Cargo build/check duration; “test result … finished in Xs” is test-harness execution only. Media/movie sample durations are not work durations.

## Dated implementation/publication milestones (not durations)

| UTC commit timestamp | Milestone |
|---|---|
| 2026-10-04T04:43:29+00:00 | [3dc2fa3c58: Publish Rust GPUI migration checkpoint with shared workbench](https://github.com/BelloWare/BelloBox/commit/3dc2fa3c585927aca0493018c3830d7d1cffedbb) |
| 2026-10-04T05:39:32+00:00 | [ccd7ef3caf: Add bounded Rust screenshot editor and OCR privacy guards](https://github.com/BelloWare/BelloBox/commit/ccd7ef3caf26151da405297b5c473ef11b4ae1c3) |
| 2026-10-04T06:01:33+00:00 | [3a91ece646: Add screenshot label movement and opaque color controls](https://github.com/BelloWare/BelloBox/commit/3a91ece646f86d774891e02bc9de321ef8c2efa9) |
| 2026-10-05T03:36:05+00:00 | [299b329b1d: Restore source converter and cookie option controls](https://github.com/BelloWare/BelloBox/commit/299b329b1d5e857df4f3c4b39810ca3966ed35e4) |
| 2026-10-05T03:37:48+00:00 | [db679011ce: Preserve CRLF line endings in shared editor deletion](https://github.com/BelloWare/BelloBox/commit/db679011ced3dd5f5c73a577d9f938fd44d9294b) |
| 2026-10-05T04:03:06+00:00 | [a2e3949653: ci(rust): capture isolated Linux UI smoke evidence](https://github.com/BelloWare/BelloBox/commit/a2e394965323ffa9ad7621d7ab008a42ac7de94b) |
| 2026-10-05T04:07:46+00:00 | [1e4692b557: Preserve shared editor IME state until accepted commit](https://github.com/BelloWare/BelloBox/commit/1e4692b5576a27f11d85b7bd1bd1a778e60d2e38) |
| 2026-10-05T04:09:44+00:00 | [d12137ceda: ci(rust): discover installed Mesa software Vulkan manifest](https://github.com/BelloWare/BelloBox/commit/d12137cedacb5257b4ddfce51dd2dd687be8d07a) |
| 2026-10-05T04:10:17+00:00 | [1b58111f37: Restore source-shaped snippet library controls and isolated persistence](https://github.com/BelloWare/BelloBox/commit/1b58111f37304e101ed73d9ee8d649c67e06aa09) |
| 2026-10-05T04:15:01+00:00 | [ee0d27a89a: Restore composed-character arrows and selection collapse in shared editor](https://github.com/BelloWare/BelloBox/commit/ee0d27a89aa52524c29b2be5937716b5e799e748) |
| 2026-10-05T04:17:21+00:00 | [a252296c80: Avoid reentrant X11 quit when closing the final window](https://github.com/BelloWare/BelloBox/commit/a252296c80d81f9a5072ea9b0993a01257f4d650) |
| 2026-10-05T04:31:58+00:00 | [52960290e1: Restore saved-snippet menu keyboard focus and safe dismissal](https://github.com/BelloWare/BelloBox/commit/52960290e15b711ae8fdb7bbcc292f1d9e594377) |
| 2026-10-05T04:37:37+00:00 | [1c6651403e: ci(rust): validate native Apple Silicon builds on standard runners](https://github.com/BelloWare/BelloBox/commit/1c6651403e31676723f5f9dee016b731d1c3e387) |
| 2026-10-05T04:42:26+00:00 | [84260fb4ff: fix(home): preserve source two-line card tail truncation](https://github.com/BelloWare/BelloBox/commit/84260fb4ff254d65c621c6a7540b95095d5b330a) |
| 2026-10-05T04:51:35+00:00 | [9a14c81a8c: Match source snippet placeholder and rendering semantics](https://github.com/BelloWare/BelloBox/commit/9a14c81a8c256e6184235ef639978b5a1b26cb86) |
| 2026-10-05T04:53:49+00:00 | [9b2a7607a5: test(workbench): separate filesystem byte-name fixtures by platform](https://github.com/BelloWare/BelloBox/commit/9b2a7607a50adb278a424354730ad434847a2bc3) |
| 2026-10-05T05:03:16+00:00 | [0abbb56148: ci(rust): keep checks public-only without artifact storage](https://github.com/BelloWare/BelloBox/commit/0abbb56148f2076cc1af8823473ecc5bb2e1f95e) |
| 2026-10-05T05:16:47+00:00 | [d5db070fca: fix(macos): isolate offline Rust preview packaging](https://github.com/BelloWare/BelloBox/commit/d5db070fca6d7335b810deefc99793ed067655c6) |
| 2026-10-05T05:18:20+00:00 | [4807399062: Restore source snippet field controls with bounded active-input retention](https://github.com/BelloWare/BelloBox/commit/4807399062d52d29a371cc19748e44e573061967) |
| 2026-10-05T06:38:54+00:00 | [033e02fb46: Retain inactive editor state without losing undo history](https://github.com/BelloWare/BelloBox/commit/033e02fb46d0ae8d5cd7d2c337a7103e2057090f) |
| 2026-10-05T06:57:01+00:00 | [c1bb4c2e7d: Remove unused direct error dependencies](https://github.com/BelloWare/BelloBox/commit/c1bb4c2e7dcbe7b2446d1294b62289831dba29a4) |
| 2026-10-05T07:18:45+00:00 | [36b05f2f90: Retain bounded snippet field undo state across viewport eviction](https://github.com/BelloWare/BelloBox/commit/36b05f2f900fe034a1652a973f7f68673442caf4) |
| 2026-10-05T08:25:23+00:00 | [b064e19951: Restore source-faithful offline World Clock planner core](https://github.com/BelloWare/BelloBox/commit/b064e199515bc341ad5d735c97caf32e74b45b4b) |
| 2026-10-05T09:02:11+00:00 | [6647454a9e: Resolve system IANA time zone without new dependencies](https://github.com/BelloWare/BelloBox/commit/6647454a9e7d41085fce758b061f3f39afa7505b) |
| 2026-10-05T09:08:51+00:00 | [c7736e1acf: Restore dedicated offline World Clock planner UI](https://github.com/BelloWare/BelloBox/commit/c7736e1acf546f0cf07f33ba86f27c4b92fee3c2) |
| 2026-10-05T09:35:27+00:00 | [614b28d854: test(ci): bound and diagnose isolated Linux UI startup](https://github.com/BelloWare/BelloBox/commit/614b28d85439dd9003f328fb3cfee6e5732640a3) |
| 2026-10-05T10:17:20+00:00 | [b8ae80b675: Restore source World Clock picker and reference menu details](https://github.com/BelloWare/BelloBox/commit/b8ae80b6753ecbabbc717df406e45c9112ab7dd3) |
| 2026-10-05T11:17:23+00:00 | [84cada0646: Restore offline World Clock launcher preview and handoff](https://github.com/BelloWare/BelloBox/commit/84cada0646913ec32830f303d636fc9136618c95) |
| 2026-10-05T16:37:50+00:00 | [10ee0d615f: Move QR previews and snapshot saves off the UI executor](https://github.com/BelloWare/BelloBox/commit/10ee0d615f81027ee05670078879fe7dca4e1b2a) |
| 2026-10-06T05:20:04+00:00 | [9ca02b05e4: Avoid idle AI stream repaint notifications](https://github.com/BelloWare/BelloBox/commit/9ca02b05e4a21cb54036d97ffb8feffb3e606e97) |
| 2026-10-06T05:29:09+00:00 | [393133cd19: Expose focused editable workbench text to host shortcuts](https://github.com/BelloWare/BelloBox/commit/393133cd19d134ffd93c3a86449d94a7b1040683) |
| 2026-10-06T05:53:16+00:00 | [f920d98cbf: Restore source list-set operations and matching controls](https://github.com/BelloWare/BelloBox/commit/f920d98cbf3246665087a4793b39662b284a002b) |
| 2026-10-06T06:11:14+00:00 | [0338bc3da6: Restore source string-literal formats and controls](https://github.com/BelloWare/BelloBox/commit/0338bc3da6e88f2604ca3ede7d8b67b62b596e75) |
| 2026-10-06T06:25:08+00:00 | [69ae3d032c: Restore source IPv4 subnet validation and results](https://github.com/BelloWare/BelloBox/commit/69ae3d032cc10c94e8b3340935ad3ff876b16594) |
| 2026-10-06T06:56:34+00:00 | [eb54763ead: Restore source permission calculator and interactive grid](https://github.com/BelloWare/BelloBox/commit/eb54763eadccc7c61c12ea59a44a38990ed9bde8) |
| 2026-10-06T07:37:00+00:00 | [508546a128: Restore source number-base GUI while preserving CLI behavior](https://github.com/BelloWare/BelloBox/commit/508546a1282ffddc4561ee17c8497153a5087ef9) |
| 2026-10-06T07:59:17+00:00 | [e4e508fd2a: Add bounded local GIF encoding and staged export foundation](https://github.com/BelloWare/BelloBox/commit/e4e508fd2aff1b12aa26359776d44623ec715238) |
| 2026-10-06T08:11:13+00:00 | [a90a99d82f: Add bounded scrolling-session engine and debug editor handoff](https://github.com/BelloWare/BelloBox/commit/a90a99d82f7856291e64c0b25cdccdd1a6d21446) |
| 2026-10-06T08:38:23+00:00 | [b2e27d6cd7: Add bounded macOS 14 ScreenCaptureKit one-shot adapter](https://github.com/BelloWare/BelloBox/commit/b2e27d6cd730afbfff69a834568da9c4e4334459) |
| 2026-10-06T09:19:44+00:00 | [4c575d228a: Move native screenshot PNG encoding to an owned background dispatch context](https://github.com/BelloWare/BelloBox/commit/4c575d228ac75bd79ef5af1760982cc07d61e3cf) |
| 2026-10-06T09:57:48+00:00 | [08f71555bd: Add immutable frozen Area selection model and debug selector](https://github.com/BelloWare/BelloBox/commit/08f71555bd8165ab009c74b23e84ba610160641b) |
| 2026-10-06T10:20:30+00:00 | [38ea32b727: Add owned macOS Area overlay and visibility policy helpers](https://github.com/BelloWare/BelloBox/commit/38ea32b727aa6c5c257647057999a9c064ec540f) |
| 2026-10-06T20:23:29+00:00 | [4a5dbc9fcd: Guard owned Area presentation and app-deactivation lifetimes](https://github.com/BelloWare/BelloBox/commit/4a5dbc9fcd680b7fbde613ab7502bbdb1bc69ac7) |
| 2026-10-06T20:34:37+00:00 | [bdd91683f5: Add gated main-display Area capture transaction and frozen selector](https://github.com/BelloWare/BelloBox/commit/bdd91683f51734a1d156388834969eaf73d9301a) |
| 2026-10-06T20:54:57+00:00 | [bf023f87ad: Add gated native movie reader and synthetic GIF integration tests](https://github.com/BelloWare/BelloBox/commit/bf023f87ade57954ba15aeadfca1d90274bd9267) |
| 2026-10-06T23:10:15+00:00 | [82c879f3fe: Fix cross-platform movie fixtures and UI smoke activation readiness](https://github.com/BelloWare/BelloBox/commit/82c879f3fea2443041d458c45926584c900e809f) |
| 2026-10-06T23:13:19+00:00 | [57f2bbcdd4: Add bounded independent-window identity and geometry policy](https://github.com/BelloWare/BelloBox/commit/57f2bbcdd4db66aa8c68b63b4fffcdab241d4ac1) |
| 2026-10-06T23:16:09+00:00 | [cef73a4b53: Port pure World Clock copilot proposal planning](https://github.com/BelloWare/BelloBox/commit/cef73a4b53b8d2d62cbfc83f9a6674b163e5f3af) |
| 2026-10-06T23:59:05+00:00 | [79c55f1bc8: Add gated callback-local native window capture adapter](https://github.com/BelloWare/BelloBox/commit/79c55f1bc85c23f6bd48ba647e917b629c77e0b6) |
| 2026-10-07T00:08:55+00:00 | [760561d25c: Correct native Window validation fixture and CF input declarations](https://github.com/BelloWare/BelloBox/commit/760561d25c6da2434877833d86f3536fc547f066) |
| 2026-10-07T00:20:26+00:00 | [da9809a950: Add synthetic frozen Window hover and click selection](https://github.com/BelloWare/BelloBox/commit/da9809a950aa6d93e79e2121911750bb0c7e0261) |
| 2026-10-07T03:39:00+00:00 | [0b8f44d2cd: Add guarded Window refresh policy and image-aware Undo history](https://github.com/BelloWare/BelloBox/commit/0b8f44d2cdcd9f5cb6580431ca7ac5cdbd46027d) |
| 2026-10-07T04:04:51+00:00 | [f1c21ae4f2: Preserve complete Rust migration handoff and unfinished alpha source](https://github.com/BelloWare/BelloBox/commit/f1c21ae4f22b14dc10c5a57651499b05cc37ec92) |
| 2026-10-07T04:24:23+00:00 | [adb877e58d: test(rust): restore synthetic CoreGraphics alpha oracle](https://github.com/BelloWare/BelloBox/commit/adb877e58dd65370a8fdcfd1617cdcc68a041aa1) |
| 2026-10-07T04:31:44+00:00 | [780ecd9496: test(rust): report full native alpha sampling mismatch corpus](https://github.com/BelloWare/BelloBox/commit/780ecd94967651a207c18a58f5ae4e60e7c571b4) |
| 2026-10-07T04:54:58+00:00 | [ebaa29d71b: Connect synthetic Window refresh through editor and preserve interactive QA](https://github.com/BelloWare/BelloBox/commit/ebaa29d71b3de89f11e64e4aaa1d02011e4d5c1f) |
| 2026-10-07T05:13:33+00:00 | [c41ce17a52: Use source-shaped CoreGraphics masking through guarded Window refresh](https://github.com/BelloWare/BelloBox/commit/c41ce17a520a9bc39073e2302c5a061d29dd27df) |
| 2026-10-07T05:26:05+00:00 | [603b4d3549: Record exact native alpha validation and remaining fidelity limits](https://github.com/BelloWare/BelloBox/commit/603b4d3549f362aa69ad2f2dfd8470f8a569827b) |
| 2026-10-07T06:01:55+00:00 | [c960d6e1a8: Add bounded converter workflow and verified synthetic GIF preview](https://github.com/BelloWare/BelloBox/commit/c960d6e1a80ad915e7868133d8c1e95b538d076b) |
| 2026-10-07T06:28:17+00:00 | [0a73d42458: Keep Area selection and editor in the owned overlay with safe export and cleanup](https://github.com/BelloWare/BelloBox/commit/0a73d42458b081e4d49c946acfeb1c0245dad011) |
| 2026-10-07T06:55:50+00:00 | [f403966c4a: Integrate supplied Window selection and refresh in owned overlay](https://github.com/BelloWare/BelloBox/commit/f403966c4a55dc8af25fb7d725035e7f0cc72faf) |
| 2026-10-07T08:07:33+00:00 | [505e8f193d: Compile guarded Window backend path and verify desktop dialogs](https://github.com/BelloWare/BelloBox/commit/505e8f193de858163474e2af269d2e3ca51e475a) |
| 2026-10-07T09:27:43+00:00 | [3f370a45b9: Wire owned native Window catalog behind closed admission](https://github.com/BelloWare/BelloBox/commit/3f370a45b9baca8101b82b2c6b8c924b1096a1b7) |
| 2026-10-07T11:46:12+00:00 | [53b62db5d6: Integrate owned paused movie preview and guarded converter](https://github.com/BelloWare/BelloBox/commit/53b62db5d68c074d7d6fe54e41f900f8324f026d) |
| 2026-10-07T11:46:19+00:00 | [5be64e4bc3: Normalize generated movie test destination on macOS](https://github.com/BelloWare/BelloBox/commit/5be64e4bc32851e08efdd3c591ac89efe9e6b015) |
| 2026-10-07T12:24:32+00:00 | [a95056a125: Characterize native sparse movie sample retiming exactly](https://github.com/BelloWare/BelloBox/commit/a95056a125b1d5fe17449f04ba0f03e69259da4d) |
| 2026-10-07T12:44:31+00:00 | [0041012edc: Verify native leading-gap frame identity without timestamp assumptions](https://github.com/BelloWare/BelloBox/commit/0041012edc718a53f9f7bdf7e9889507d2a9817f) |
| 2026-10-07T13:49:47+00:00 | [26dd6ed9f0: fix: retire converter images after window return and preserve trim visibility](https://github.com/BelloWare/BelloBox/commit/26dd6ed9f0bbdc8a4107ea177cbce3003b77def7) |
| 2026-10-07T15:37:40+00:00 | [91f9a8484a: feat: add bounded image OCR consent and editor workflow](https://github.com/BelloWare/BelloBox/commit/91f9a8484a09801845b83505fcfbd32183bc6e6d) |
| 2026-10-07T17:51:20+00:00 | [1f5874ce74: ci: execute image OCR app coverage on macOS](https://github.com/BelloWare/BelloBox/commit/1f5874ce741b8de21fa09c747a5bada7488e2982) |
| 2026-10-07T21:43:28+00:00 | [f61d0e4da1: feat: add gated recording workflow and drain owned media on close](https://github.com/BelloWare/BelloBox/commit/f61d0e4da194fe0da90c54c034d0e51106a8eacd) |
| 2026-10-07T21:51:33+00:00 | [8712627de8: fix: apply workspace formatting to recording checkpoint](https://github.com/BelloWare/BelloBox/commit/8712627de818058badbc4f7378b0822d792feea2) |
| 2026-10-07T22:16:49+00:00 | [d193ca393b: test: bound native RGB rounding and scope recording fixture helper](https://github.com/BelloWare/BelloBox/commit/d193ca393bd5db75f8bdb00e0861fde5a7c20cab) |
| 2026-10-08T02:48:39+00:00 | [e83337517d: test: compare native movie output with independent Swift renderer](https://github.com/BelloWare/BelloBox/commit/e83337517d1e93ac1eb89c269a002e6431d14b65) |
| 2026-10-08T03:15:38+00:00 | [fc7d894702: test: preserve canonical GIF result paths across cancelled retries](https://github.com/BelloWare/BelloBox/commit/fc7d894702299849ac8c2d8b2521695598284ff6) |
| 2026-10-08T03:17:12+00:00 | [928ac9fd76: docs: record complete macOS recording App validation](https://github.com/BelloWare/BelloBox/commit/928ac9fd76c1ba0ad73c199a0a7691c7dee46f5e) |
| 2026-10-08T04:50:21+00:00 | [a2389dac0c: Port pure recording output settings from Swift](https://github.com/BelloWare/BelloBox/commit/a2389dac0cc425b1b179ab2b3e6c3483d4e201fa) |
| 2026-10-08T19:02:30+00:00 | [7a3ecdcf63: Integrate guarded app Quit with pointer-safe refusal and physical work drain](https://github.com/BelloWare/BelloBox/commit/7a3ecdcf63d250519a1f99152c8f537dbc95d4ba) |
| 2026-10-08T19:54:21+00:00 | [dec241107f: Connect explicit provider model discovery and connection tests to Settings](https://github.com/BelloWare/BelloBox/commit/dec241107f359146d0e63fb051dfafe94e1425e1) |
| 2026-10-08T20:23:16+00:00 | [e9cdb40c64: fix(rust): retain provider setup Quit ownership through HTTP retirement](https://github.com/BelloWare/BelloBox/commit/e9cdb40c6475a9ba006cd245050fc650a82780ba) |
| 2026-10-08T20:44:40+00:00 | [f30b81f0fd: docs(rust): record interactive provider settings loopback validation](https://github.com/BelloWare/BelloBox/commit/f30b81f0fd62e7bb29b8809a4f57471b46b01450) |
| 2026-10-08T20:56:50+00:00 | [c3ced91eeb: feat(rust): add dedicated World Clock Copilot workflow](https://github.com/BelloWare/BelloBox/commit/c3ced91eeb7678305f770cfc144352a9643d4327) |
| 2026-10-08T21:06:50+00:00 | [2a5f95b35d: docs(rust): preserve interactive Clock Copilot workflow evidence](https://github.com/BelloWare/BelloBox/commit/2a5f95b35d27f33b19e4530e696a06da496fc9b8) |
| 2026-10-08T21:22:49+00:00 | [9ce80e005e: feat(rust): hand off palette Clock Copilot conversations safely](https://github.com/BelloWare/BelloBox/commit/9ce80e005e9d8e896cb31ea9dc98aafc30000179) |
| 2026-10-08T22:01:47+00:00 | [3bbef6854b: fix(rust): make compact Clock Copilot input and suggestions clear](https://github.com/BelloWare/BelloBox/commit/3bbef6854b5b1323c8573409f7f7b42ea71022d6) |
| 2026-10-08T22:34:55+00:00 | [abd7247c25: feat(rust): persist per-model generation preferences across HTTP workflows](https://github.com/BelloWare/BelloBox/commit/abd7247c25c1736f7a4fd6a34032e7af9d7a1f5d) |
| 2026-10-08T23:28:45+00:00 | [fbd5eea70c: Fix strict GIF completion before advancing frames](https://github.com/BelloWare/BelloBox/commit/fbd5eea70c287cb92844ad46894ba8a4154191ee) |
| 2026-10-08T23:29:04+00:00 | [7b9a99ae7c: Include preserved GIF readback validation logs](https://github.com/BelloWare/BelloBox/commit/7b9a99ae7c19b8251836cc82b6618101e8f8a55e) |
| 2026-10-08T23:39:21+00:00 | [2d9b170ed8: fix(rust): integrate strict GIF completion repair with verified evidence](https://github.com/BelloWare/BelloBox/commit/2d9b170ed81b33ed3adfa080dfbadc89b2f21f2f) |
| 2026-10-09T00:06:53+00:00 | [8448568578: docs(rust): record native first-following fixture limitation](https://github.com/BelloWare/BelloBox/commit/8448568578a8eadd76b0c3ddb5bd7022595e4402) |
| 2026-10-09T01:35:46+00:00 | [5934b98978: docs(rust): prove positive native movie timestamp](https://github.com/BelloWare/BelloBox/commit/5934b9897854a4102ef71d6463d4b4f7a3005158) |
| 2026-10-09T02:17:28+00:00 | [59339af1fa: Test native first-following movie selection with fixed composition fixtures](https://github.com/BelloWare/BelloBox/commit/59339af1fa422acea4067207c55344666c3fa4ab) |
| 2026-10-09T02:44:47+00:00 | [25e78766d2: test(rust): validate first-following native movie fixtures](https://github.com/BelloWare/BelloBox/commit/25e78766d2463c186e7dbdf4d85029401290d062) |
| 2026-10-09T02:57:28+00:00 | [3976bd8632: feat(rust): add editable palette QR preview](https://github.com/BelloWare/BelloBox/commit/3976bd8632d8ecfaba9350b750f46e6e457b68d3) |
| 2026-10-09T03:21:43+00:00 | [3ae7349bd7: fix(rust): guard QR clipboard capability and modal key release](https://github.com/BelloWare/BelloBox/commit/3ae7349bd76d4d0c86b368c3fee21ab2ca531ace) |
| 2026-10-09T03:35:22+00:00 | [83ae8b0b58: fix(rust): track held activation across palette input focus](https://github.com/BelloWare/BelloBox/commit/83ae8b0b589506faa95b6894cc042f642a5a4b24) |
| 2026-10-09T03:47:54+00:00 | [45d5e841e8: test(rust): isolate QR held-key fixture from learned ranking](https://github.com/BelloWare/BelloBox/commit/45d5e841e89d8062816dc9df24789cf603be22c9) |
| 2026-10-09T04:00:14+00:00 | [15197a71d8: docs(rust): validate editable palette QR workflow and input guards](https://github.com/BelloWare/BelloBox/commit/15197a71d81c620bf60da18f0d066505e4c0dc99) |
| 2026-10-09T04:51:34+00:00 | [4589e02808: fix(qr): guard popup actions and constrain decoded preview to final host bounds](https://github.com/BelloWare/BelloBox/commit/4589e0280844df4a070533c7babb9d01cf430905) |
| 2026-10-09T05:20:14+00:00 | [80e6d41a68: test(copilot): normalize accepted fixture sockets and cover delayed request bytes](https://github.com/BelloWare/BelloBox/commit/80e6d41a68fc0e2bf186272fb788a301f4f9e744) |
| 2026-10-09T05:31:49+00:00 | [0e13ee6def: docs(qr): bind popup GUI, native fixture repair and final source validation](https://github.com/BelloWare/BelloBox/commit/0e13ee6def214425865759591c37539429530d71) |

## Unknown coverage and prospective tracking

The union of CI intervals leaves untimed gaps before, between and after jobs. Those gaps can contain parallel development, review, interactive validation, waiting, pauses or inference; this record cannot distinguish them. Local receipt coverage is also incomplete and some receipts lack start/end timestamps. Earliest/latest commits do not define continuous effort. No total project hours or completion percentage is defensible from these records.

For subsequent work, record a stable operation ID, repository/source SHA, category, UTC start/end, monotonic elapsed seconds, command/result and resource. Record waits separately with their reason and end condition; use parent IDs for nested build/test phases; preserve failures and retries. Record inference only from direct model request telemetry when available. Keep parallel resource sums and interval-union elapsed coverage separate.
