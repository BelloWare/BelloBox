# Bounded paused movie review and converter ownership

This checkpoint connects a selected movie identity, paused source preview,
Start/End seek, trim/export and Movie/GIF result switching through the existing
converter. The behavioral references remain `UI/GIFExportViews.swift:245–278,
445–453,487–491` and `Recording/GIF/GIFTranscoder.swift`.

All ordinary native movie and Area/Window capture gates remain closed. This is
production-compiled native composition with generated-media test coverage
awaiting exact native CI, not an activation of arbitrary movie decoding, continuous AVPlayer playback, audio,
recording, permissions or a claim of complete native converter parity.

## One selected source and one owned worker lane

Inspection now returns `Inspection { info, selected }`. `SelectedMovie` is one
pointer-free Arc owner retaining the canonical path, original regular-file
handle, device/inode, length, modification and change timestamps. Export and
preview reopen an AVURLAsset only using that same selection owner; they do not
select an equal-looking new path or trust four media metadata scalars. Replacing
a source with another file containing identical bytes is detected. The nonblocking
no-follow open prevents a replacement FIFO from turning selection into a blocking
read. Close clears the retained source/descriptor even if the entity survives.

This is ordinary filesystem-change detection, not frozen movie bytes, a malicious-
filesystem boundary or an atomic identity guarantee. AVFoundation reads a URL,
not the retained descriptor. Mutations or replacements that happen and revert
between checks can escape detection; a final source check is not atomic with
publishing a separate GIF. Nothing here claims otherwise.

`gif_converter/worker.rs` owns one active ticket and one replaceable pending
request. Inspection, paused seeking and export use the same actual request worker.
New source/seek requests cancel the active logical request and coalesce pending
work. An active ticket retires only after native work, native object disposal and
prepared-image work return. No new decoder is admitted merely because a generation
was cancelled. A stale completion retires only its own ticket before the latest
request starts. Completed responses cannot form an unbounded queue.

Native assets, tracks, readers, outputs and CF/CG render objects stay on their
creation worker. Only owned selection/metadata/cancellation/pixels cross to the
host. Metadata completion blocks own a Rust callback ticket and lease. Cancellation
or timeout does not release admission while a copied callback remains. The owner
waits for its own physical ticket drain, then rechecks cancellation, the original
absolute deadline and source identity before delivering metadata. A framework
that never releases a block can hold the retiring worker indefinitely; cooperative
cancelLoading does not prove all opaque native loader work has stopped.

A newly opened converter waits cooperatively for the process-global native lease
instead of inheriting transient Busy from an older closing converter. Admission
wait and metadata share one absolute deadline. The waiter checks its own
cancellation and never releases another owner's lease. Source-token filesystem
selection may precede this wait; native object creation cannot. This wait runs
on a worker, not the UI thread.

## Paused seeking contract

Seeking is independent of export's 0.05–120-second duration and 5–20 fps rules.
Requests must be finite, nonnegative and within the source duration. A bounded
window starts at most two seconds before the requested time and extends at most
250 ms afterward, clipped to the clip. Native CMTime values use the existing
600-timescale representation. Source PTS is explicitly checked against the window.

The last sample at or before the requested time wins. If the first in-window sample
is later, that first following sample is used. End == duration targets the last
displayed sample rather than demanding a nonexistent EOF frame. Short clips can
be previewed even when too short for export; long clips can seek beyond 120 seconds.
Sparse/VFR intervals with no sample in this bounded window show an explicit
unavailable-frame error. A frame with a long preceding presentation interval but
PTS outside the window is not silently imported. The caption shows both requested
and actual source PTS. This deliberately bounded behavior is not exact AVPlayer
zero-tolerance seeking or continuous playback.

The host retains one source RenderImage and at most one preparing replacement,
with a 720-pixel longest-edge cap and no upscaling. Paused frames have no playback
timer. The existing exported-GIF reader still prefetches one next frame and owns
its Once/Loop delays. Movie/GIF switching retains the same selection and saved
result. Trim seeks update the movie while preserving the explicit Movie/GIF
choice and independent GIF playback, as in Swift. Export retries preserve an
earlier result on cancellation/failure. A source seek preempted by export remains
visibly pending; terminal export reschedules the latest intended time even if an
older source image or GIF exists. Completed requested time is stored separately
from actual PTS, so End/VFR differences are never mistaken for unfinished work. Choose
Another is unavailable while conversion owns the form, matching the Swift host;
close always cancels. Open/Save cancellation does not replace the chosen source.

Native frame storage remains capped separately. Full decoded frame/CFData/CG
allocations, PNG/resize transients and opaque codec/renderer memory are additional;
720-pixel preview output is not a whole-process memory guarantee or latency claim.

## Last-sample and publication boundary

`SequentialFrameSource::finish` runs exactly once after successful encoding,
staged sync and readback, immediately before core publication, outside the export
control's publication mutex. `NativeFrames` checks decoder status, the original
selection and cancellation on the same native owner thread. Reading is a valid
terminal sampling state for an intentional trim; Failed, Cancelled and Unknown
are errors. Native cancellation is serialized after the last sample read, never
concurrent with copyNextSampleBuffer. Repeated successful final checks do not
mistake the adapter's own intentional retirement for a decoder failure.

Failure, late cancellation or detected source mutation removes staging and keeps
previous output. Core cancellation still linearizes against final publication;
an already-published result is reported truthfully rather than called cancelled.

## Generated-media boundary and remaining rendering difference

`movie-fixtures` is a nondefault cross-crate CI feature. Its public constructor
creates a private owned directory and a fixed generated MOV from finite enum
choices; it accepts no external pathname. It returns that fixture's sealed source
owner. No global gate override or DEBUG arbitrary-path decoding exists. Ordinary
MovieAsset::open remains unavailable even in the fixture-feature test binary.
Linux's explicitly labeled DEBUG fixture generates pixels without an input file
and exercises the same host/controller/core export lifecycle; it does not decode
an MP4 or establish native Apple runtime behavior.

Rust currently applies the preferred transform into a full display-sized RGBA
frame, then the existing core performs bilinear output scaling. Swift's
GIFFrameRenderer uses CoreGraphics Medium interpolation directly at output size.
The new paused preview adds its own bounded Triangle resize. These paths are not
claimed pixel-identical. No invented tolerance or broad color/orientation fidelity
claim is substituted for a strict output-size CoreGraphics oracle. ICC/HDR,
original movie color, codecs beyond the fixture, accessibility/IME/AppKit/Spaces,
continuous playback/audio and real native GUI acceptance remain open.

## Validation

Exact source/binary hashes, commands, original unedited screenshots, saved GIF and
scoped interaction observations are recorded in
[validation/movie-preview-2026-10-07](validation/movie-preview-2026-10-07/validation.json).
Native execution is a separate exact-commit Apple CI gate. The workflow now has an
explicit generated converter-host step; source review and Apple-target metadata
Clippy alone are not native execution. The parent handoff records final CI status.


### Immutable Linux acceptance

The first immutable candidate exposed an idle caption pairing a cancelled newer
request with an older prepared frame. All20 original screenshots and exact source
inputs are retained, rather than relabeled as success. The repaired candidate
`5d9a996ced75be919acf97474fe59ba224df268afb935746e38a4e1f20b9edba`
passes the actual0.642 preemption/cancel recovery, coalesced End seek, system Save
cancel/reopen and independently decoded output, Movie/GIF switch, preview
rechoose/close and held-export previous-result retention/retry/close/reopen.
Fifteen additional original screenshots identify that final binary. The EXPORT
fixture stalls before encoding/staging; core regressions separately cover staged
cleanup. See the exact QA report and validation manifest for candidate boundaries.

Final focused suites pass 44 ordinary and 44 app-only nonDEBUG converter tests,
32 core GIF tests and 137 portable platform tests (three existing ignored).
Strict app/platform Clippy, minimal check, normal build, nonDEBUG metadata and
Apple-target platform metadata Clippy pass. These do not replace exact Apple
execution. The reviewed delta is +1,414 production/+171 support, yielding 48,152 /
24,371 /105 benchmark nonblank Rust lines; 717 preserved support lines become
production and 3 move in reverse. The scoped verifier and positive ranges are
retained alongside validation. These counts are not a feature percentage.
