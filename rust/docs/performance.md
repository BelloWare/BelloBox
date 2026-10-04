# Rust performance verification

Status: instrumentation/setup in progress. No performance result is implied by a
successful compile, a fast unit test, or a screenshot. macOS runtime validation
must be performed separately on actual Apple hardware.

## Reproducible measurements

Record exact commit, build profile, Rust version, OS, CPU/RAM, display resolution,
scale, compositor, refresh rate and renderer. Compare release builds, with identical
fixtures and viewport sizes. Record warm and cold-cache results separately; don't
claim a cold run unless the cache state is controlled. Keep raw samples.

`python perf/measure.py --trials 20 --output perf/results/process.json -- <binary> <benchmark args>`
measures process wall time and sampled RSS, not frame time. The child must terminate.
It does not collect child-process RSS; short-lived peaks can fall between samples.
Failed/time-out trials remain visible and are excluded from latency percentiles.

App instrumentation uses `BELLO_PERF_LOG` when implemented. Report event names and
units explicitly. CPU time in a GPUI render callback is not GPU completion, a
presented frame, input-to-photon latency or achieved FPS. Software-renderer timings
are useful for regression detection but must not be projected onto Apple GPUs.

## Representative workloads

- File tree: expand/collapse 10k and 100k-entry fixtures lazily; no recursive initial
  traversal; preserve responsiveness during IO and changing roots.
- Git: bounded history pagination; 1k/10k-line and large/binary diffs; cancel stale
  requests and test repositories changing mid-request.
- Editor: small source, 1 MiB file, long lines, Unicode graphemes, rapid insert/undo,
  Vim motion/operator sequences, file-reload conflicts and IME composition.
- BelloBox: search the actual catalog, JSON/data processing at supported bounds,
  cancel/restart calculations, transfer preview state to independent windows.
- BelloAgent: replay deterministic streaming fixtures, queue operations, long
  transcripts, cancel mid-stream and receive stale response events.

## UI smoothness acceptance

Budget targets (not achieved measurements): at 60 Hz, 16.67 ms/frame; at 120 Hz,
8.33 ms/frame. Measure presentation interval p50/p95/p99 and missed deadlines on
real display runs when a valid instrument exists. Record scroll/typing latency
separately. Keep expensive file/git/parse/HTTP work off the UI executor and avoid
per-token transcript-wide layout. Lists and editors should render visible ranges.

Check both 1280x800 and minimum usable window sizes, repeated focus/cancel/reopen,
keyboard navigation, IME, and large-file safeguards. Screenshots document appearance;
they do not prove smoothness or behavioral parity.

## Current verification limits

Native Swift baseline cannot run on Linux. Linux-vs-macOS benchmark numbers are not
a controlled comparison. macOS signing, TCC, Keychain, Sparkle upgrade, capture,
recording and Apple GPU behavior require a separate Mac validation run.

## Test data and privacy

Use deterministic synthetic documents and local fixture gateways for automated
benchmarks. Do not include user conversations, credentials, private file contents
or provider payloads in telemetry or committed reports. The process harness
captures a bounded stderr tail for diagnostics; run it only with synthetic inputs
and review that tail before publishing results.
