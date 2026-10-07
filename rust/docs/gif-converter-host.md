# Standalone GIF converter host checkpoint

Historical standalone-host checkpoint. The later [paused movie review workflow](movie-preview-host.md)
adds selected-source identity, bounded source preview/trim seeking, Movie/GIF
switching and physical worker retirement. Earlier missing-source-preview and
test-only native backend statements are superseded there; native activation and
continuous source playback/audio remain unavailable. Evidence below retains its
original source/binary scope.

Behavioral sources: `BelloBox/UI/GIFExportViews.swift` (`VideoToGIFViewModel`,
`VideoToGIFContent`, `GIFTrimControls`, `AnimatedGIFNSView`) and
`BelloBox/Recording/GIF/GIFTranscoder.swift`. This is a Rust migration checkpoint,
not native movie-conversion or whole-workflow parity.

The `videoToGIF` Home/palette route opens a separate GPUI window. Choosing a movie
is explicit; generation-fenced worker metadata cannot overwrite a newer choice
or a closed window. Source replacement resets trim and result while retaining
frame-rate, longest-edge and repeat choices. Native `MovieAsset` is created and
consumed only on its worker; the existing production Unavailable gate remains
unchanged, before source I/O. No codec, capture, recording or permission capability
is enabled by the host.

The DEBUG-only `BELLOBOX_GIF_FIXTURE=1` route supplies generated 320×180 color
frames for three seconds through the same host and existing bounded Rust exporter.
Launch with `BELLOBOX_TOOL=videoToGIF` and an isolated `BELLOBOX_CONFIG_DIR`.
A clearly labeled fixture button exports to a uniquely named temporary GIF;
ordinary Save uses the system destination dialog. No existing file is overwritten:
GPUI's path result alone does not certify the source's overwrite approval flow.
A failure or cancellation preserves the source and any earlier successful GIF.
Cancel linearizes against publication using `ExportControl`: cancellation before
publication prevents output, while an already-published file is truthfully reported
as saved. Busy remains set until the worker settles. Closing cancels both movie
and export controls and fences late completion; opaque native calls are not claimed
to be forcibly interrupted.

Encoding progress uses a single atomic counter, polled only while the job is
active, rather than an unbounded notification queue. One job owns one plan and
single-use export control. File size is reported only after validated publication.
Source paths, options, images and output files are not stored in preferences.
Only the existing explicit tool-open usage metadata follows normal app behavior.

## Trim and exported-result review

The paired Start/End sliders use the Swift 0.1-second UI minimum and exact
push/pull/max-duration coupling. Numeric fields retain full-precision defaults,
reject invalid drafts and freeze while a dialog/export owns the form. The backend
retains its separate 0.05-second minimum. These are explicit numeric controls in
addition to sliders; native source-video seek is not claimed.

Only this converter's completed export results have a preview. It opens a regular
file with `O_NONBLOCK|O_NOFOLLOW`, caps input reads at 256 MiB, validates expected
full-canvas dimensions/frame count, and bounds decoder output to 16 MiB. Delays
must satisfy the own export's 2–20-centisecond range and exact total duration;
corrupt timing is rejected rather than clamped into a long-lived timer.

One next frame is decoded and prepared on a worker while the current frame is
visible. PNG encoding/decoding and the core RGBA copy are transient allocations.
GPUI's pinned `Image::to_image_data` converts PNG RGBA to its BGRA RenderImage;
no pixel format is guessed. The UI owns the current prepared image and at most
one prefetched prepared frame. It directly paints RenderImage, avoiding per-frame
asynchronous asset-loader blanks. The replaced GPU image is explicitly released
only after its replacement is prepared; reset/release also retire the current
image. Opaque renderer allocations and transient old/new resources are additional,
not a whole-process memory guarantee.

Identity is pinned at first preview open, not atomically at export publication.
A metadata-only cloned handle checks length and modification time before/after
reads, including EOF. Repeats consume the old decoder and rewind its pinned File,
never reopen a path that could now name another GIF. These checks detect ordinary
changes but do not form a hostile-filesystem immutability guarantee.

Playback always starts paused. Explicit Play/Pause uses centisecond frame delays,
keeps no recurring timer while paused, stops at EOF for Once and rewinds for Loop,
using the completed job's options rather than newer edits. Rechoose, reconversion
and close cancel the owned preview task and fence late decode/timer delivery.
Close releases retained reader/frame state even if another entity owner remains.
A synchronous bounded decode already executing cannot be forcibly interrupted.

A GPUI executor deadline schedules presentation after predecode, so ordinary
decode time overlaps the display delay instead of being added to it. Late decoding
shows the next frame when ready without catch-up bursts. Native Reduce Motion/
autoplay parity and measured renderer presentation timing are not claimed.
Always-paused is the conservative fallback until a reviewed native accessibility
preference path exists.

## Intentional limits

Native movie selection currently produces a visible Unavailable error; it cannot
convert an actual user's movie. Native movie playback/scrubbing is not implemented.
The file clipboard API is unavailable in the current GPUI seam, so the explicitly
labeled action copies a path, not a file object. Reveal uses the platform's folder
reveal action. Interactive launcher option handoff, native popup/minimize/Spaces,
source file-type filtering and native accessibility/IME validation remain open.
No full usable-converter or macOS performance/parity claim is made.

## Validation

Thirty-six focused controller/GPUI regressions pass. They cover real staged
synthetic export, retained source/results, invalid drafts and exact fractional
defaults, source-shaped short-clip sliders, stale callbacks, source/target safety,
cancellation on either side of publication, actual worker-to-host preview,
Once/Loop/pause/deadline behavior, rapid Pause→Play without skipped/duplicate
frames, replacement of a GIF path between repeats, malformed long delays,
retained-entity close, GPUI BGRA/opaque-alpha pixels and explicit editor Tab focus.
GPUI TestPlatform lacks Open-path dialogs, so the initial Cmd+O test checks the
exact guarded admission; actual Linux shortcut routing was exercised separately.

Final-source focused commands (from `rust/`):

- `cargo test --locked -p bellobox-app gif_converter:: -- --test-threads=1`: 36 passed.
- `cargo test --locked -p bellobox-core recording::gif:: -- --test-threads=2`: 30 passed.
- `cargo clippy --locked -p bellobox-app -p bellobox-core --all-targets -- -D warnings`: passed.
- `cargo build --locked -p bellobox-app`: passed.
- `cargo rustc --locked -p bellobox-app --bin bellobox --tests --profile test -- -C debug-assertions=no --emit=link`, then the exact compiler-reported test artifact's `gif_converter::` filter: 36 passed. This is a release-style cfg test harness, not an optimized whole-workspace release or native macOS execution claim.

### Actual synthetic Linux interaction

The accepted candidate binary SHA-256 is
`9d75021db47df2af330ad35bb52bfbb604bd29b155dc6e0afd09f1a5e92118d2`.
Its exact owned-source hashes, baseline tree, screenshot bytes and reviewed LOC
are in [the manifest](validation/gif-converter/manifest.json). Seven unedited CUA
image outputs are retained with `.jpg` extensions matching their actual JPEG
magic, not renamed as PNG. The candidate excludes concurrent inline Area work.

Observed on that binary:

- Start→Tab→End entry produced the intended 0.5–1.5-second trim. The paired sliders
  moved the opposite endpoint to retain valid source bounds. Invalid pasted trim
  text was rejected without changing the accepted plan or previous result.
- Cancel during a real synthetic export reported cancellation before publication;
  no new output or staged sibling remained. The source and options stayed usable.
- At 10 fps, both Once and Loop exports were independently decoded as 320×180,
  10 frames, ten 100 ms delays and owner-only mode 0600. Once contained no repeat
  metadata; Loop contained repeat count 0. Output sizes were 10,407/10,426 bytes.
- Preview starts paused. Play kept a visible prepared frame; Once stopped after
  the clip, Loop continued, and rapid Play/Pause/Play remained responsive.
- Re-conversion preserved the source/options and delivered a new result. Explicit
  Copy GIF path pasted text into the editor; this is not a native file clipboard.
  Reveal opened the output folder; specific-file selection was not established.
- At 600×630 and the 560×600 minimum, scrolling reached controls and result actions.
  The whole content scrolls, unlike Swift's fixed-chrome layout. Escape closed the
  popup; normal reopen showed default options, no previous result and the native
  decoding-unavailable notice. The initial platform shortcut reached the chooser.

**Verified environment blocker:** both system Open and Save dialogs failed in the
cloud desktop. Their visible error states recovered without losing prior results.
No successful native movie selection, actual movie conversion or system-dialog
save is claimed. The explicit app-owned fixture export exercised real filesystem
publication independently. macOS/Spaces/IME, native movie playback, Reduce Motion
integration and full accessibility fidelity remain unvalidated. The Linux window
manager also displayed the titlebar's Unicode dash incorrectly; body labels were
readable. No performance parity or native frame-presentation measurement is implied.

An earlier candidate (`1e302441…`) exposed a skipped End-field Tab stop and transient
PNG asset-loader blanks during playback. Those defects were fixed in source and
retested on the accepted candidate; earlier observations are not relabeled as
final evidence. Independent reviews also corrected stale fixture labeling,
release-test cfg, publication/first-preview identity scope and repeat-path identity.

### LOC scope

This checkpoint adds 1,178 production and 890 test/support nonblank physical Rust
lines, with no benchmark lines. Positive test/DEBUG fixture spans start at their
cfg attributes; unguarded host scaffolding remains production. Comments count.
Against the reviewed native-mask baseline, totals become 43,445 production,
20,692 test/support and 105 benchmark lines. The per-file delta and exact support
spans are in the manifest. These are source counts, not a completion percentage.


## Later cloud dialog environment acceptance

The original dialog failures above describe that original runtime environment.
The restored process-local FileChooser portal now passes actual Open/Save
cancel/reopen, synthetic GIF Save, and valid generated MP4 selection-to-native-
unavailable handling on the exact later published `f403966` app binary. See
[cloud-dialog-acceptance.md](cloud-dialog-acceptance.md) for full hashes, original
screenshots, verified files and launch recipe. Native MovieAsset decoding and
macOS dialog acceptance remain open.
