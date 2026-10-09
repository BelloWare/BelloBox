# Native first-following movie fixture validation

Base: `5934b9897854a4102ef71d6463d4b4f7a3005158`, tree `0beefc3c3a556cf055d4273e227f1e1d1cf3511f`.
Reservation: [6072683205](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6072683205), acknowledged in [6072707893](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6072707893).

The previous AVAssetWriter `DelayedFirst` fixture decodes a black image at PTS 0, so it cannot exercise a before-first request. This change adds the two already proven, independently authored MOV files as fixed `GeneratedTiming::PositiveComposition` and `ZeroOriginComposition` cases under the existing nondefault `movie-fixtures` gate. The MOV bytes are copied unchanged from the accepted [positive-composition diagnostic](../native-positive-composition-2026-10-09/README.md). Their orientation is fixed to landscape; other orientations are refused without rewriting the media.

The new platform test first reads complete native image traces and verifies exact MOV hashes, timestamps, dimensions and full RGBA equality. The positive image times are **0.1, 0.2, 0.3 seconds**; the zero-origin control times are **0, 0.1, 0.2**. A request at **0.05 seconds returns the actual 0.1-second first image** with exact full-range pixels. Later/interior and EOF seeks return the expected original frames, and every platform reader/seek releases admission. The three Rust RGBA hashes also match the same-host decoded control bytes retained by the earlier Swift diagnostic; no pre-encoding or cross-host color-parity claim is made.

The new App test uses the existing Request/Controller/Model workflow without launching a GUI. It proves the same before-first result and exact preview pixels, then exports the positive range 0.05–0.35 and zero-origin range 0–0.3: three 64×48 frames, one-shot playback and centisecond delays **[10, 10, 10]**. Their decoded GIF frame pixels match each other exactly. Cancellation retains the prior result and prior file bytes; same-byte inode replacement rejects both seek and export without changing prior output or leaving a staged export. All selected MOV bytes are checked unchanged before the intentional identity-replacement probe.

The old `DelayedFirst` control still reports `[0, 0.1, 0.2, 0.3]`, with a non-repeated opaque black PTS-0 image. Existing short/sparse, orientation, cancellation, sample-limit, reader retirement, callback drainage, source identity, late validation, and strict GIF readback checks remain in the selected suites.

## Results

Host: macOS 14.8 (23J21), arm64; Rust/Cargo 1.91.1; Swift 6.0.2; macOS SDK 15.1. Offline/locked, two build jobs, deployment target 14.0, isolated configuration, external build cache and no provider environment variables. Core-generated data and AVFoundation fixtures only; no user media, GUI/capture/TCC/AX, provider/key/vault, screenshots, signing, installation or release work.

| Suite | Result | Raw log SHA-256 |
| --- | --- | --- |
| platform-movie | 24 passed, 0 failed/ignored | `b969b8e9c5b28c2e0fe8fed87401ebfcdf9be25b91cf90f0338622487451bb71` |
| core-gif | 49 passed, 0 failed/ignored | `a2105ddb75205649a05a67000ca7cae513e37029ee33672f93c935f14100dd36` |
| app-native-host | 3 passed, 0 failed/ignored | `d92293162943c2a0404e86a826c68718967a35474e94249b2da8fa752fe0b545` |

**76 passed; zero failed or ignored.** The final workspace formatting check passes. Platform strict all-target Clippy with `--no-deps` passes. Full strict Clippy does **not** pass on this native toolchain:

- Both candidate and a separately verified exact-base snapshot fail `clippy::needless_return` at unchanged `bellobox-core/src/settings.rs:253`.
- A diagnostic `--no-deps` run on both source states exposes exactly the same seven `clippy::nonminimal_bool` diagnostics: `bellobox-app/src/desktop.rs:208,210`, `screenshot_ui/quit_tests.rs:129,166`, and `shutdown/tests.rs:525,582,648`.
- Those files are outside this reservation and remain byte-identical to the base. No lint allowances or production fixes were added. The scoped platform result is not represented as a replacement for the failed full strict gate.

The native test logs contain the existing `IOServiceMatchingfailed for: AppleM2ScalerCSCDriver` diagnostic, once in each native process; all decodes and exports completed. They contain no Rust compiler warnings. The first formatting check failed because the App module had been formatted with the 2021 edition; formatting that claimed file with its actual 2024 edition fixed the mismatch before any native test ran. The initial failure and repair note are retained. Raw logs retain their original whitespace; the local `.gitattributes` disables Git whitespace diagnostics only for those `.log` evidence files.

## Scope and source identity

Only four existing source files changed, all within the acknowledged fixture/test scope; two fixed MOVs were added. The first production portion of `movie.rs`, all decoder/seek/admission implementation files, dependencies, Cargo lockfile and permission gates are unchanged. Ordinary native open remains unavailable even when the fixture feature is enabled, as asserted through the App test.

All **1,529 other base files and modes** are unchanged. The **1,535 build input files** were verified unchanged after native execution. `source-postimages.json` binds the six changed/added fixture and test files; `build-source-before.json` SHA-256 is `2f7d188b6b98bfbcfcca6d3ef048fb73043926b91c34d54947bbe2371181e940`. `final-validation.json` records the verified results and failures; its SHA-256 is `b62a08b3be08ec4b5ac01b4e3411e720265443df199da4e6bf2a0ea13a948b60`.

## Reproduction and evidence

The exact commands and counts are in `commands.json`; run them from `rust` with the environment captured in `host-toolchain.json`. The first four final commands pass, and the last full strict-Clippy command reproduces the known base failure on this host. `run-validation.py` retains original raw logs, timestamps and test executable hashes. `run-lint-comparison.py` records candidate/base lint comparisons; their source snapshot identity is in `base-lint-source.json`. The archival runners use the original local paths in `setup.json` and refuse to overwrite result files.

`python3 rust/docs/validation/native-first-following-rust-2026-10-09/verify_evidence.py` verifies the complete source scope, media provenance, selected test names/counts, full-range hashes and timestamps, before-first/old-control observations, export delays, and identical base lint diagnostic sets. It uses only local files and Git, writes a fresh final receipt/postimage file, and does not rerun native tests. The recorded final receipt is indexed by `SHA256SUMS`; rerunning the verifier changes its timestamp, so preserve the recorded evidence before doing so.

No native policy failure required a production change. This is a fixture-backed native CLI result, not production admission or actual GUI acceptance. Root remains the sole `rust` integrator.
