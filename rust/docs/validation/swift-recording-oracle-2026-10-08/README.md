# Independent Swift-source recording oracle

Baseline: `d193ca393bd5db75f8bdb00e0861fde5a7c20cab`.
This correction changes test/support code and CI only. Production Rust movie
decoding/writing, native admission gates, the original Swift tree, and the raw
movie fixture remain unchanged.

## Why the previous oracle was wrong

[macOS run 37695162872](https://github.com/BelloWare/BelloBox/actions/runs/37695162872)
reached frame 4 and returned blue 253 instead of raw 255 at pixel (8, 0).
The earlier one-code-value allowance had been inferred from frame 0 alone.
It was not an AVFoundation decoding guarantee. Increasing it again would replace
one unproven threshold with another. This change removes that threshold.

The prior default platform suite was **205 passed / 1 failed / 3 ignored**.
Its generated native writer, cancellation and finish-callback tests passed;
mutex poison no longer hid them. The dedicated recording-feature and full App
steps were skipped after the raw oracle failure. Those results remain evidence
for their exact original commit, not this correction.

## Exact behavioral reference

The test builds a separate Swift executable with the installed Apple toolchain.
It extracts the complete `GIFFrameRenderer` and the `AVAssetReaderTrackOutput`
setup verbatim from the checked-in `BelloBox/Recording/GIF/GIFTranscoder.swift`.
The whole original file and both exact excerpts have independent SHA-256 bindings.
Changed source, changed extraction boundaries and missing driver substitution
markers fail before compilation. The driver is separate from the original
excerpts; it does not use Rust's pixel recipe or renderer implementation.

The Swift process opens the same sealed, retained raw movie. Its renderer's image
bytes are copied row-by-row, removing storage padding only. The driver validates
8-bit RGBA storage, alpha placement and byte order. It does not redraw, clamp,
color-convert, reorder channels or otherwise normalize reference pixels.
Rust and Swift must match every RGBA byte and presentation timestamp exactly for
all 12 ordered frames, along with dimensions and storage length. The reference
has an exact frame count and opaque alpha; its PTS must additionally remain within
1 ms of the independent raw recipe. Reader completion/end-of-stream is checked.

Every frame logs per-channel difference counts and maximum absolute differences
for Rust/Swift, Rust/raw and Swift/raw before any pixel-equivalence failure is
reported. Raw/rendered differences are diagnostics, never acceptance thresholds.
The exact checked-in raw payload test remains separate and unchanged. H.264
writer assertions also remain separate.

The original Swift and fixture hashes, excerpt hashes, driver and generated
reference hashes are in [swift-source-bindings.json](swift-source-bindings.json),
[source-and-loc.json](source-and-loc.json), and the final source verification.
[reference.generated.swift.txt](reference.generated.swift.txt) is a reviewable
generated example, not a second production implementation.

## Child ownership and bounds

- Each compiler/reference child receives null stdin and stdout
- Each gets its own process group and a 120-second limit
- Stderr is polled every 20 ms; observing more than 64 KiB aborts the group
- The stderr threshold is an abort-on-observed-limit, not a hard write cap;
  output may exceed it between polls
- Failure diagnostics read at most 4 KiB
- The reference writes exactly 295,164 bytes for 12 frames; Rust validates the
  file size and bounds its read before parsing
- Unwinding/timeout kills only the owned process group and waits for its child;
  no process-name or unrelated-process termination occurs
- Private temporary inputs/results are removed by their scoped owner

Short Linux fixture tests verify the timeout, delayed descendant cleanup and
stderr abort. Mutants that remove those protections fail their controls.

## Verified locally

- Focused recording suite: **31 passed**
- Entire Linux platform default suite: **168 passed / 3 ignored**
- Entire Linux platform recording-fixtures suite: **168 passed / 3 ignored**
- Strict all-target platform Clippy: default and recording-fixtures passed
- Workspace cargo fmt check passed
- **16 mutation controls detected**, including one-byte RGB/alpha changes,
  omitted pixel/length/raw-payload checks, relaxed PTS equality, source/extraction
  binding changes, altered original Swift, missing substitution, protocol
  count/dimension/alpha changes, missing timeout/stderr abort and parent-only kill

The initial local extraction-control assertion used a shorter marker that began
at the same offset and therefore selected unchanged bytes. It was corrected to
actually change the extracted span; that initial failure log is retained.
Final source hashes match the restored mutation-control inputs.

`sha2` 0.10.9 was already locked in the workspace. The only dependency change is
an exact test-only dependency edge from bello-platform, with its corresponding
lockfile entry; no package version was added or updated and no download occurred.

The Rust LOC delta is **0 production / +319 support / 0 benchmark**, resulting in
**52,593 production / 31,668 support / 105 benchmark** nonblank physical lines.
The additional Swift driver is **75 test-only nonblank lines**, reported separately
from Rust LOC. Counts include comments and preserve previous classifications.

Run the mutation harness only against an isolated writable checkout:

    python3 run-negative-controls.py --repository /path/to/isolated/checkout --evidence /path/to/evidence

## Native gate still pending

Linux type-checks the Rust child driver and exercises its process controls. It
cannot compile AVFoundation Swift or validate native pixels. No native pass is
claimed yet. A new explicit macOS CI step verifies that the exact native test is
listed, runs it with its full name and prints all frame summaries. Existing default
and recording-feature suites also continue to include the native test.
Acceptance requires that exact published commit's macOS run, including the later
recording-feature and full App gates. No live capture, permissions, user device,
provider request, GUI replay, release or production feature enablement is involved.
