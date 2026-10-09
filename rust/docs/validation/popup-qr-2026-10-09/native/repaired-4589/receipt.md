migration-peer — completed the acknowledged two-file test-fixture repair from #issuecomment-6074651259 / claim #issuecomment-6074666951 / acknowledgment #issuecomment-6074697050.

Exact base: `4589e0280844df4a070533c7babb9d01cf430905`, tree `294c99a7847757d2c41217834b8dc87eab9e4765`. **Three ordinary native builds passed; the repaired native matrix passed 307 tests, with 0 failures and 3 existing ignored cases. Format passed.** The original pre-repair 296/5/3 matrix, fresh/learned 52/0/1 diagnostics and 3/3 socket probe remain separately preserved in [receipt part1](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074717477) and [part2](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074718876).

The patch explicitly clears inherited nonblocking mode on the accepted socket in each of the two claimed test helpers. It retains the listener's 8-second admission deadline, the reader's 5-second timeout, all existing assertions and production networking. Each new regression withholds all headers until the reader is waiting, then withholds the body after sending headers; both waits must remain pending before the exact request is returned. The accepted socket's read timeout is checked after the read. Only generated numeric-loopback data is used.

Meaningful negative control: the exact base plus both new tests, with the socket normalization omitted, compiled natively and failed both tests with the original immediate macOS OS 35 WouldBlock at the two read_request helpers (0 passed/2 failed/0 ignored). The negative binary matches its native inventory and is preserved at SHA-256 `a4bc9fb11c65bd44876b44ec516a1216a4b37813ad5a71d058ae685722af108d`. Both identical regressions then passed in default, minimal and movie-fixtures builds with the normalization present. These deliberate negative-control failures are separate from the final green runtime matrix.

| Repaired native invocation | Passed | Failed | Ignored |
|---|---:|---:|---:|
| Default App qr | 46 | 0 | 0 |
| Default App launcher | 54 | 0 | 1 |
| Minimal App qr | 46 | 0 | 0 |
| Minimal App launcher | 54 | 0 | 1 |
| Movie-fixtures App qr | 46 | 0 | 0 |
| Movie-fixtures App launcher | 54 | 0 | 1 |
| Core QR, once | 7 | 0 | 0 |

The matrix selected 81 distinct names (80 runnable plus the existing ignored readiness-cache child), with intentional overlap across filters/variants. Every selected name and count matches the corresponding native compiled inventory, and execution binary hashes match the inventories. The child remains invoked only by its own isolated parent test; no ignored test was manually enabled. All previously failing copilot/lifecycle tests now pass in the movie-fixtures invocation.

Native strict Clippy remains blocked by the same baseline diagnostics in all three variants: full strict exit 101 with core settings.rs:253 needless_return; App-scoped --no-deps strict exit 101 with seven nonminimal_bool diagnostics. All six comparisons match the exact 15197 source-anchor/multiplicity baseline from the immediately preceding pre-repair run. That preserved baseline was carried forward, not rerun for this test-only patch. No lint fix/suppression or native strict pass is claimed. Ordinary build/test/format logs contain no compiler warnings.

Both negative and positive sources were freshly compiled after cleaning the owned core/App packages. All 1926 source files/modes were verified before/after; only the two acknowledged test paths differ from 4589. The original 4589 and 15197 snapshots and 111 pre-repair artifact files remain hash-unchanged. The original checkout stays clean at 59339af1fa422acea4067207c55344666c3fa4ab. Host: macOS 14.8 (23J21), arm64; Rust/Cargo 1.91.1, Swift 6.0.2, SDK 15.1, deployment 14.0. Offline locked dependencies, two Cargo jobs, generated isolated configuration, one test thread.

Delivery is the exact-base patch below for dot's integration. git apply --check and actual application to pristine claimed-file preimages succeeded; applied postimages match exactly. No shared rust publication. Production capture/tool/vault gates and permission boundaries are unchanged. No actual desktop/clipboard/provider/credential/capture/TCC/AX interaction, signing, install, release or spending occurred. Synthetic native tests do not close actual GUI acceptance.

Patch SHA-256: `6420ac93d90893255a51cbedd9485d04ec3c47043488c74159264c0b8bc58ae4` (5118 bytes).
Postimage SHA-256:
```text
ba7f979d6be8654678bf4abe10cba4c2ee075db731fad3b86bb514c502276082  rust/crates/bellobox-app/src/launcher_clock_ui/copilot/tests.rs
92f9d1ce762a9f7391f6ff1ef3f154f49cb1daf44d8f885a36b016930758300e  rust/crates/bellobox-app/src/launcher_ui/copilot_lifecycle_tests.rs
```
Ordinary native binary SHA-256:
```text
a1f8517e07a05505a678da9e1eebc4b813dc5726476891c5c2cd7e1845d39878  build-default
2e50e8e2318c7d980862aa76bd46bdd3038232b958a7aec0b888db906d42c0d1  build-minimal
c1b297aa2fda2f875e403f45d5400e8cb8e1ac24b3ee1070a0043c4f10547bb4  build-movie-fixtures
```
Validation SHA-256 `570235d19f588dd9e6d6d4a59edbb247038783b5018654f7dafd8addcf2981ff`; lint-comparison SHA-256 `d1cd90a0c8f2be0d389810478ccca67e030538336c63d95638a830a44ddf16a8`.


Full transport: https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074800250 and https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6074801791

The exact patch, copied Rust postimages/negative-control source and orchestration Python scripts are excluded from this text package. Original hashes remain in bundle-verification.json and the issue transport. Final integration binds the patch to an immutable successor separately.
