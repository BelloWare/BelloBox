# Pure Window refresh and image-aware history

This checkpoint adds source-backed refresh decisions and preparation of an already
supplied image. It performs no capture, window query, permission request, OCR or
host/UI publication. Production Area/Window capture remains disabled.

The pure policy follows CaptureWindowCatalog.isOccluded and ImageAlphaMask: scan
front-to-back until the target, account for own regular windows and source-eligible
special layers, preserve missing-evidence defaults, and choose to keep frozen
pixels, apply independent alpha, or replace with the independent image. The native
host must supply its actual window levels; the selector's narrower catalog cannot
serve as occlusion evidence. Source image-size tolerance is retained.

Preparation checks selection/session/topology and cancellation; final replacement
checks the same edit-session/base-generation token and CURRENT empty annotations
and absent crop. Editing then Undo to clean remains eligible. Font preparation is
preserved. Accepted replacement increments revision so pending OCR becomes stale,
adds no Undo step, and does not by itself wire the UI's preview/OCR changed() path.

## History prerequisite and memory policy

Existing editor history now retains a shared immutable base with each annotation/
crop snapshot. Undo and Redo restore the matching image dimensions and geometry.
A monotonic base epoch outside snapshots prevents returning to an older image from
reviving an old refresh request. Dirty baseline remains annotation/crop-only and
does not pin the original image or make a clean refresh appear edited.

The combined retained-history cap is explicitly 64 MiB, replacing the former
16,000,000-byte metadata-only cap. It covers annotation/crop metadata plus distinct
historical RGBA Vec capacities across both stacks. The active image is excluded;
Arc allocation identity deduplicates shared pixels. Existing 64-step limits remain.
Oldest history is evicted beyond these limits. This supports a common 4K historical
base but is deliberately bounded, unlike Swift's unrestricted whole-document
history; larger captures may lose cross-base Undo history. Active images, detached
render jobs and transient preparation have separate existing bounds, so 64 MiB is
not a claim about total process memory.

## Validation and remaining gates

Independent source review passed. All 21 focused refresh/history regressions and
161 affected screenshot tests pass, covering occlusion order/layers, alpha masks,
clean-after-Undo eligibility, stale document/session/topology/cancellation,
resized-image crop restoration, old-image token revival and tiny-budget eviction.
Strict core Clippy and formatting pass. Tiny fixtures test pruning without large
memory allocations; no broad benchmark or frame-performance claim is made.

Portable alpha arithmetic/resampling is deterministic, but byte-for-byte
CoreGraphics color-space and rounding equivalence remains unverified. Native
capture delivery, live host wiring, preview/OCR UI invalidation and interactive
macOS lifecycle behavior remain separate gates. No production Window enablement
or live refresh UI is introduced here.
