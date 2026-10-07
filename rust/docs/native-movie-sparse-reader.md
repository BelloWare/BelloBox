# Native sparse-reader characterization

The exact published movie checkpoint `5be64e4bc32851e08efdd3c591ac89efe9e6b015`
passed Linux run `37616462056`. macOS run `37616462042`, job `112775863468`,
passed native compile/link, strict platform Clippy, 175 platform tests (3 existing
ignored), 357 core tests, 9 alpha/lifecycle tests and the supplied Window host mask.
Its generated landscape/portrait/mirrored converter-host test also passed, including
trim/output timing, pixel sentinel checks and retained source identity.

The second generated-host test failed at its sparse expectation: a seek to 50 seconds
returned `SourceFrame { requested: 50.0, actual: 48.0, png_bytes: 314 }` instead of
FrameUnavailable. Native build/packaging steps after that failed step did not run.
The fixture writer submits exactly three samples, at 0, 130 and 130.1 seconds.
Sparse submitted timestamps therefore do not by themselves prove the bounded
reader will return no sample. Clipping/retiming of a held sample to the requested
range start is a hypothesis to verify, not a tolerance to invent.

## Focused follow-up

The production decoder, range, cancellation, source identity and seek algorithm are
unchanged. A generated-only diagnostic accepts the existing sealed fixture owner,
never a pathname, and reads its complete source range on a worker with an eight-
frame output cap. Ordinary native admission remains closed.

The host test now requires exact full-range PTS `[0, 130, 130.1]`, then checks that
bounded seek 50 returns PTS 48 and that every decoded RGBA byte equals the original
PTS-0 frame. It also checks the later PTS-130.1 sample against its original pixels.
Short and delayed-first fixtures have their original PTS checked too. Diagnostics
print only fixture timestamps and equality outcomes, never pixel payloads. An
unexpected trace or image still fails strictly; arbitrary frames are not accepted.

This complete timestamp/pixel characterization is pending exact native CI. Local
Apple-target platform all-target Clippy with movie-fixtures passed after refreshing
only the affected core/platform Apple outputs in the shared warm target. That is
metadata/type checking, not Apple execution. No actual screen, microphone, user
movie or permission was accessed.

## Timestamp contract

The preview caption reports the requested time and returned AVAssetReader sample
PTS. It does not promise the original coded-sample timestamp or AVPlayer-equivalent
seeking. FrameUnavailable means no acceptable sample was returned within the bounded
reader window; it is not inferred solely from gaps between submitted/coded samples.
The existing bounds still reject returned PTS outside the window. Whether this
specific held sample is clipped/retimed without pixel changes is established only
when the new exact native test passes. No interpolation, color or full-VFR parity
claim follows.

The contemporaneous deferred-image-eviction/trim-width follow-up remains a separate
uncommitted UI change; it is not part of this native test correction checkpoint.
