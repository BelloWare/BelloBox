# Native movie reader: reconstructed, production-disabled CI checkpoint

Historical reconstruction checkpoint. The later [movie preview host](movie-preview-host.md)
promotes this implementation to normal native compilation behind an explicit
closed admission gate, carries one selected source through preview/export and
adds same-host generated-media tests. The earlier cfg(test)-only descriptions
below retain historical scope; exact native results are recorded per checkpoint.

## Status and provenance

The previous workspace, uncommitted candidate and temporary patches were lost.
This tree is newly reconstructed from the visible task record and published
Swift/Rust source, based on 38ea32b727aa6c5c257647057999a9c064ec540f. It is not a
byte-identical recovery, and prior test results do not certify it.

Production `MovieAsset::open`, `MovieAsset::reader` and `MovieReader::next_frame`
return Unavailable on every platform before source I/O or callbacks.
`NATIVE_MOVIE_READER_IMPLEMENTED` is false. The actual typed implementation is
compiled under `cfg(all(target_os = "macos", test))`, so the existing GitHub macOS
workflow will compile and run it rather than silently excluding the native code.
No app route, importer, recording capability or permission request is enabled.
The adapter decodes movies and converts frames to feed the existing Rust GIF
encoder; it is not GIF decoding. Hardware acceleration is codec/path dependent.

## Implemented draft APIs and source behavior

- `MovieAsset::open(local_path, MovieCancellation)` prepares validated source info.
  `MovieInfo` retains exact preferred-transform display extents; frame dimensions
  are rounded separately. Source paths remain caller-visible for export identity.
- `MovieReadRange::new(start, end, frame_interval)` accepts finite nonnegative
  source time, 0.05–120 seconds and 5–20fps intervals; decoder range includes one
  frame of tolerance. Core GIF sampling controls the exact trim cut/delays.
- `MovieAsset::reader(range)` and worker-bound `MovieReader::next_frame(probe)`
  preserve source presentation timestamps. MovieFrame transfers owned RGBA via
  `into_parts()` without a second full-frame copy. Debug excludes image pixels.
- A test-only caller-owned `SequentialFrameSource` wrapper maps these frames into
  core `DisplayRgbaFrame`; the production platform has no dependency on core.

The native code uses pinned typed AVFoundation/CoreMedia/CoreVideo/CoreGraphics
bindings. No Swift/Objective-C implementation shim or guessed struct-return ABI.
The sole custom selector is the omitted legacy key-status method, typed as the
binding's AVKeyValueStatus/NSInteger with NSString and nullable NSError** inputs.
AVMediaTypeVideo and writer constants are checked/unwrapped according to their
actual generated nullable declarations. Metadata keys are asynchronously loaded
and each status checked before getters; failures are not guessed from nil output.

The source guard canonicalizes and preflights a nonempty regular file before
opening a read-only descriptor, then checks pathname device/inode, length and
mtime before and after native reads, including marker and EOF reads. This detects
ordinary changes; it is not a hostile-filesystem TOCTOU guarantee or immutable
snapshot. URL-based AVFoundation does not automatically read that retained fd.
Relevant source directory entries are trusted against hostile replacement.
AVURLAsset is file-only with ReferenceRestrictions=ForbidAll; external local and
remote media references are denied. Protected/no-video/invalid sources fail.

Decoded32BGRA samples are presentation ordered. Marker samples count toward the
sample cap. Only AVAssetReaderStatus::Completed maps to clean EOF; unknown,
reading and failed states after nil map to DecodeFailed, cancellation separately.
All terminal failures, including sample cap, cancel/retire the reader on its
owner worker. Reader/output fields release before the owning asset/final lease.

Coded pixel dimensions need not equal natural display dimensions. Both are
bounded independently; decoded pixels are drawn into the natural rectangle,
matching Swift's handling of presentation sizing. Rendering applies the full
preferred affine transform, standardized origin, rotation/mirroring and source
Y-axis conventions. Output is explicitly RGBA over opaque black.

## Ownership, cancellation and memory bounds

Native objects remain !Send/!Sync on their creation worker. Only atomic state and
owned notification-only block captures cross threads. Core publication is fenced
immediately when cancelled; native cancelReading runs only between synchronous
copyNextSampleBuffer calls or during owner-thread teardown, never concurrently.
Synchronous decode interruption and a hard native decoder timeout are not promised.
This intentionally avoids the original Swift ReaderBox's cancellation overlap.

Metadata wait has a 30-second cooperative deadline shared across asset/track key
loads. cancelLoading requests cancellation but supplies no join. The admission
lease bounds adapter-owned readers and copied callback contexts; callback release
or cancellation does not prove opaque AVFoundation loader work has quiesced.
Framework loader work may outlive a timeout return. There is no claim of a strict
whole-process native-job drain or hard native memory limit.

Limits: source/display≤8,388,608 pixels and16,384 per dimension; packed BGRA/RGBA
≤32MiB, padded native pixel storage≤64MiB,≤30,000 native sample reads. Full
height*stride storage, including final-row padding, is checked with arithmetic
overflow protection before any raw byte access. Natural/display dimensions and
all affine values must be finite and valid before allocations/CG context setup.

Pixel buffers have matched read-only lock/unlock. Visible BGRA is copied while
locked into a bounded packed Vec, then copied into owned CFData; the lock and Vec
are released before drawing. CGDataProvider retains CFData, never an unlocked
borrowed CV address, so framework image caching cannot dangle that source pointer.
The output bitmap context borrows the stable RGBA Vec and drops before it moves.

Visible adapter-managed payload peak is at most 128MiB: decoded storage≤64MiB plus
packed32MiB+CFData32MiB, or decoded64MiB+CFData32MiB+output32MiB. Add caller/core
retained frames, allocator overhead and opaque AVFoundation/CG allocations or
caches separately. This is not the core encoder's96MiB envelope and not a process
RSS guarantee. Native CG drawing/codec work may be synchronous within these sizes.

## Dependencies and actual acquisition evidence

Target-macOS normal dependencies, defaults disabled:
- objc2=0.6.4; objc2-foundation=0.3.2 with std, NSArray, NSDictionary, NSError,
  NSObject, NSObjCRuntime, NSString, NSURL, NSValue and objc2-core-foundation
- objc2-core-foundation=0.3.2 with std, CFCGTypes, objc2
- objc2-av-foundation=0.3.2 with std, AVAsset, AVAssetReader,
  AVAssetReaderOutput, AVAssetTrack, AVAsynchronousKeyValueLoading, AVMediaFormat,
  block2, objc2-core-foundation, objc2-core-media
- objc2-core-media=0.3.2 with std, CMBase, CMSampleBuffer, CMTime, CMTimeRange,
  objc2, objc2-core-video
- objc2-core-video=0.3.2 with std, CVBase, CVBuffer, CVImageBuffer,
  CVPixelBuffer, CVReturn, objc2
- objc2-core-graphics=0.3.2 with std, CGAffineTransform, CGBase,
  CGBitmapContext, CGColorSpace, CGContext, CGDataProvider, CGGeometry, CGImage, objc2
- Existing block2=0.6.2 and raw-window-handle=0.6.2 preserved.

Approved macOS dev-only additions: AVFoundation's AVAssetWriter,
AVAssetWriterInput and AVVideoSettings features; existing workspace bellobox-core
path with default-features=false. Writer fixtures append typed CMSampleBuffers,
so no AVFoundation optional CoreVideo/pixel-buffer-pool feature is required.

Fresh official Cargo resolution added seven 0.3.2 packages, without changing
existing package versions/checksums: the four approved media frameworks plus
core-audio/core-audio-types/io-surface weak optional entries. Two independent
manifest feature walks found the last three inactive under the selected
production features. Cargo's actual native active graph is still unverified:
offline check nevertheless requires their source manifests before it compiles.
No direct audio or IOSurface capability is selected or used.

The exact newly approved six-package official crates.io acquisition succeeded
with exit 0, and each materialized archive SHA256 matches its official metadata:
AV478ae33f..., CM05ec5768..., CVd425caf1..., CGe022c9d0..., CF2a180dd8...,
Foundatione3e0adef.... Existing support objc2/block2/objc2-encode/dispatch2 sources
were also restored. Raw command, output and full hashes are retained in the local
native-movie-evidence directory for the parent; dependency proposal is separable.

The additionally approved three transitive sources remain absent. First attempt
was cancelled by automatic approval review. Its single authorized identical-call
retry failed before Cargo ran with a bubblewrap sandbox bootstrap error:
`Can't mkdir parents for /root/.codex: Not a directory`.
A later explicitly approved identical retry hit the same bootstrap failure.
No alternate transport, hidden-path inspection or security-configuration
modification was used. Existing GitHub native CI is
the intended next validation environment; this checkpoint claims no native pass.

## Tests and remaining evidence gates

Final checks on the production-disabled, reconstructed source tree:
- Offline locked focused Linux platform movie tests: 11 passed, 0 failed, 64 filtered;
  compilation1.18s, tests0.00s.
- Strict offline locked platform-library Clippy with -D warnings passed in0.35s.
- Owned-file rustfmt and git diff --check passed.

macOS-only fixtures are written but NOT compiled or executed locally:
- Four asymmetric corner colors verify identity, 90-degree rotation, mirror,
  BGRA→RGBA byte order, alpha255 and black compositing; coded/natural dimension
  mismatch regression and cancelled render are included.
- Tiny Rust-only AVAssetWriter H.264 MOV verifies native presentation timestamps
  and portrait output before the wrapper feeds core staged GIF export in both
  loop modes. Structure/count/duration/source preservation are checked.
- A native magenta sentinel is observed in decoded frames, but the generated GIF
  is not independently pixel-decoded here: do not claim this new fixture alone
  proves GIF sentinel exclusion. Existing core sampling tests cover trim policy.
- Cancellation preserves prior destination/source and removes staged GIF output;
  sample-limit failure retires reader. Nil-status mapping is checked directly.
- Malformed metadata open and pre-cancel are tested; this is not a midstream
  corrupt-decoder/EOF runtime test and does not prove opaque loader quiescence.
- Native fixtures serialize admission-sensitive jobs with one test mutex.

The next gate is macOS CI compilation/link plus synthetic fixture execution.
ARM64 CI does not establish Intel runtime parity. Production must remain disabled
until native failures are resolved and source/CI results independently reviewed.

Sources: BelloBox/Recording/GIF/GIFTranscoder.swift and the exact acquired objc2
0.3.2 framework manifests/generated Rust declarations. The AVAssetReader header
contract explicitly forbids cancelReading concurrent with copyNextSampleBuffer.

## First native CI execution and fixture corrections

On commit `bf023f87ade57954ba15aeadfca1d90274bd9267`, macOS run
`37530185860` compiled/linked all native targets and passed all six native movie
fixtures, including MOV decoding, PTS/orientation/alpha, cancellation and staged
Rust GIF export. Its overall job failed two portable test expectations because
macOS resolves `/var` through `/private/var`; the source guard correctly returned
canonical paths. Expectations now compare canonical paths and include a
symlinked-parent fixture. The native implementation's deprecated bitmap aliases
were replaced with their exact typed constant values, without changing bits.

That commit's Linux Rust tests, lints and builds passed. The live smoke failed
its initial window activation before any UI check. Activation now retries bounded
requests and verifies the actual active window while checking child liveness;
this does not establish live recovery until the next CI run succeeds. The wait
has a 30-second polling deadline plus at most the final two bounded probes.

Fresh local corrections pass 12 movie tests and 14 harness tests. Production
remains unavailable, and overall CI success is still pending for these fixes.
