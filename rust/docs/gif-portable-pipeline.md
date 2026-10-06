# Portable GIF pipeline: design and evidence

This bounded core slice ports GIF planning, timing, sequential frame selection,
encoding, validation and transactional export semantics from the Swift sources.
It does **not** complete recording or movie conversion. No CLI or desktop route
is enabled. Fixtures are synthetic RGBA only; no capture, provider or ffmpeg
process is invoked.

## Source anchors

- `BelloBox/Recording/GIF/GIFTranscoder.swift`: `GIFExportOptions.normalized`,
  `GIFExportPlan.make/delays`, `GIFTranscoder.encode/validate/isSameFile`,
  `GIFFrameRenderer` and `GIFLoopBlock`.
- `BelloBox/Recording/RecordingFileStore.swift`: private sibling staging and
  atomic publication rather than writing the destination in place.
- `BelloBox/Recording/RecordingOutputTransaction.swift`: cancellation and
  publication serialized by the same lock.

## API and missing adapters

`recording::gif` exposes immutable validated `GifExportPlan`, normalized
`GifExportOptions`, `GifSourceInfo`, a validated `DisplayRgbaFrame`,
`SequentialFrameSource`, `ExportControl`, `ReplacePolicy` and `export_gif`.

An injected source returns one timestamped, display-oriented top-left/y-down,
straight-alpha RGBA frame at a time. It must report genuine decode failures as
errors and observe cancellation during any long/blocking operation. It may seek
to or slightly before the clip start before construction. The core never seeks,
executes subprocesses, opens a movie decoder, or requests system permissions.
A file-backed adapter must pass its source path for identity protection; None
is reserved for genuinely synthetic/non-file-backed input.

Missing production adapters remain explicit:

- AVFoundation movie metadata, sequential decoding, stream-range setup and
  preferred-transform orientation/color management.
- ScreenCaptureKit capture, audio, recording lifecycle/review UI and permissions.
- Non-Unix stable file identity and owner-only file security where native APIs
  are needed. ALL filesystem export is unavailable on non-Unix before path I/O,
  source callbacks or staging, including synthetic/fresh destinations. Pure
  planning and RGBA/codec building blocks do not require those native adapters.

Display orientation is supplied by the adapter, not inferred from pixels. Core
bilinear downsampling composites RGBA over opaque black. Its interpolation and
color handling are not claimed to be pixel-identical to Core Graphics.

## Source behavior preserved

- Defaults: 15 fps, 640-pixel longest edge, forever loop, start 0, end of source.
  FPS is clamped to 5–20, edge to 64–1080. Nonfinite/negative trim starts become
  zero; invalid trim ends become unset. Serde wire keys match Swift camelCase.
  Each omitted or explicit-null field takes the source decodeIfPresent default;
  serialization omits unset trimEnd like Swift encodeIfPresent.
  FPS/width use signed 64-bit integers, matching macOS Swift Int before clamping.
  Source-produced integer JSON roundtrips; numeric strings and floating/exponent
  JSON tokens for integer fields are rejected. Foundation's numeric coercion and
  exact JSON diagnostics are not reproduced. Serialize normalized options when
  input could contain nonfinite trim values: serde_json encodes a raw nonfinite
  float as null, while Foundation's default nonfinite encoding strategy throws.
- Trim is clamped to the movie, 120 seconds and the defensive 3,000-frame cap;
  clip lengths below 0.05 seconds are rejected. The current 120s/20fps bounds
  yield at most 2,400 frames. Portrait uses its longest displayed edge; no upscale.
- Frame count uses `ceil(duration * fps - 1e-6)`. Integer centisecond delays are
  differences between rounded cumulative times, with the final partial frame
  included. Delays below 2cs borrow from earlier frames without changing the
  rounded overall duration. At 15fps the sequence starts 7,6,7,7,6,7.
- Sampling retains current and one pending frame. For each output target it
  consumes samples up to target plus half a frame, bounded by trim end +1ms.
  When no current frame exists it can use the first pending in-range frame;
  clean early EOF holds the last frame for the entire planned count.
- Forever is exactly one NETSCAPE repeat block with count 0. Once writes no
  repeat block at all. No structural stripping is needed because the Rust
  encoder emits repeat metadata only when requested.

## Explicit additional safety limits

These are Rust core resource protections, distinct from source option
normalization. There is no cumulative pixel-work cap that shortens valid plans.
All original 120s/20fps/1080 option combinations can be planned for supported
source frame sizes. Actual export can fail on the limits below, never truncate.

- Each source frame: at most 8,388,608 pixels / 32 MiB RGBA; dimensions at most
  16,384. This admits 3840×2160 4K input, but not 8K. Frame dimensions must match
  the rounded display size declared in the source metadata throughout the stream.
- At most current + one pending source frame (64 MiB retained RGBA). Output
  RGBA is at most 1080²×4 bytes, followed by one palette/index frame and bounded
  codec scratch. Conservative core pixel/codec working-set envelope is 96 MiB;
  this is not an allocator-enforced process RSS limit. Provider-owned buffers,
  allocator overhead and thread stacks are outside that envelope. Readback
  begins after encoding/source-frame scratch is released and decodes one frame
  at a time with an explicit 1080²×4 decoder output limit.
- At most 30,000 input samples, checked before asking the source to allocate the
  next sample. Equal timestamps are allowed but cannot produce an infinite loop.
  Out-of-order, negative or nonfinite timestamps fail. This limit also applies
  when an adapter unnecessarily starts decoding far before the trim start.
- At most 256 MiB encoded bytes, enforced by a counting writer on every write.
  Write, trailer-finalization and flush errors propagate. No output truncation,
  partial publication or silent frame dropping is accepted.
- Checked size arithmetic, per-row render cancellation, per-sample/per-frame
  cancellation, cancellation-aware codec writes and validation reads. Quantizing
  or LZW processing a single bounded frame is synchronous and not interruptible
  inside the third-party codec call. Adapters must bound their own allocations
  and cancellation latency.

`gif = '=0.14.2'` is a direct dependency solely to control repeat-block omission,
explicit finalization errors and bounded readback. The package/version was
already locked and cached. Only the core dependency edge is added to Cargo.lock;
no new package, version or fetch is needed.

## File and cancellation guarantees

The destination directory must already exist and be trusted against hostile
concurrent mutation of its parent directories AND relevant source, destination
and stage directory entries/leaves. Its canonical path and Unix identity are
rechecked immediately under the publication fence. The stage path is also checked
against the open validated stage inode before publication. These detect ordinary
interference but leave check/use races: this std-filesystem slice is not an
openat/dirfd sandbox against a malicious owner/writer of the relevant tree.
No parent directories are created; no movie bytes are read for conversion.

Source canonical path and an open read-only source file identity are retained.
Same paths, canonical/symlink aliases, and Unix device+inode hard-link aliases
are rejected both before work and immediately before publication. Destination
leaf symlinks, directories and nonregular files are always rejected, even when
replacement is authorized. Final filesystem operations never follow leaf
symlinks or open the destination for in-place writes.

A randomized hidden sibling is opened with `create_new`, read/write access and
Unix mode 0600. Non-Unix export fails before any filesystem work until equivalent
private-file and stable-identity adapters exist. The staged file is synced and
read back through its owned handle. RAII attempts to remove the private stage on
success, error, cancellation or unwind only while its path still names the owned
inode; it does not knowingly unlink a replacement. It never removes a prior
destination or the source. The identity check does not close hostile TOCTOU races.
Process crashes, external stage movement or cleanup failure can leave an orphan
private stage; crash recovery and directory fsync durability are not claimed.

Default `RefuseExisting` uses atomic hard-link publication, then removes the
private stage. It never uses an exists-check followed by an overwriting rename.
A filesystem that cannot create hard links returns an error, with no unsafe
fallback. `ReplaceExistingFile` explicitly opts into same-directory rename of a
regular file; errors preserve the old destination. Successful publication keeps
the new file owner-only on Unix.

`ExportControl` is single-use: Ready → Running → Published/Failed, with Cancelled
terminal when cancellation wins. `cancel` and the final no-clobber link/rename
hold the same mutex. If cancel linearizes first, publication cannot execute;
if publication wins, cancel returns AlreadyPublished and does not remove the
completed file. A callback reporting all frames encoded is not a success event;
only the final successful result establishes readback and publication.

## Validation and synthetic tests

Validation first walks GIF header/descriptors/color tables/extensions/image
sub-blocks structurally with fixed scratch buffers. It rejects malformed,
missing/extra or duplicate loop metadata, wrong counts/delays/dimensions,
truncated blocks, missing trailer and trailing bytes. Identifiers in compressed
pixels/comments cannot be mistaken for repeat blocks. A second pass fully
LZW-decodes every frame with frame consistency and end-code checks and verifies
planned dimensions/count/delays. No full encoded-file allocation is needed.

Synthetic tests cover option defaults and normalization, portrait/no-upscale
planning, source maxima, all 5–20fps rates with partial endings, timing-budget
borrowing, timestamp/dimension errors, trim cutoff/tolerance/fallback/EOF hold,
alpha black compositing and display orientation, once/forever metadata, malformed
blocks and compressed pixels, capped output, pathological streams, cancellation
at multiple stages, final publication/cancel races, default no-clobber races,
explicit replacement, same-path/symlink/hard-link protection, Unix permissions,
source errors and panic cleanup. No actual screen, movie or audio is accessed.

Initial verification executed in the isolated GIF worktree on 2026-10-06:

- `CARGO_BUILD_JOBS=2 cargo test --offline --locked -p bellobox-core recording::gif --lib -- --test-threads=2`
- 27 passed, 0 failed, 189 filtered; compilation 4.13s, tests 0.03s.
- Direct cached rustfmt on the recording files; `git diff --check` clean.
- Lock accepted in locked/offline mode. Diff adds only the existing gif dependency
  to bellobox-core; no package/version/checksum changes or network fetch.
- No broad workspace matrix, actual movie/recording, desktop/UI route, ffmpeg,
  non-Unix filesystem semantics, native orientation or large-memory performance
  validation was executed. Production/UI parity remains unavailable.


Review follow-ups prepared after the initial test run:

- Non-Unix filesystem export/staging now fails before side effects, with a
  platform-gated regression covering synthetic/fresh/existing destinations.
- Source JSON camelCase, omitted/null defaults and 64-bit integer/clamp fixtures.
- Stage ownership check before publish and best-effort ownership-aware cleanup;
  a substituted stage is neither published nor deleted. Trusted-tree assumption
  explicitly includes all relevant entries/leaves; hostile TOCTOU remains out of scope.
- Final focused rerun after all review fixes: the same locked/offline command
  passed 30 tests, 0 failed, 189 filtered; compilation 4.26s, tests 0.04s.
- The non-Unix-only rejection fixture is present but was not executed on this
  Linux host. Linux verifies the selected platform gate and all Unix paths.
- All owned Rust files were formatted; `git diff --check` is clean. No native
  capture/decoder, app/UI integration, external fixture decoder or broad matrix
  validation is claimed by this lane.

Integration verification on the current Number Base checkpoint:

- The additive module and existing-lock dependency edge were applied to
  `508546a1282ffddc4561ee17c8497153a5087ef9` without replacing other feature files.
- All 30 focused GIF tests passed in that integrated tree (196 filtered out).
- Strict core-library Clippy identified two nested-if style warnings; equivalent
  let-chain guards were applied before the final focused rerun.
- Final independent review found no remaining blocker for this foundation scope.
