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
range start was initially a hypothesis, subsequently verified below.

## Focused follow-up

The production decoder, range, cancellation, source identity and seek algorithm are
unchanged. A generated-only diagnostic accepts the existing sealed fixture owner,
never a pathname, and reads its complete source range on a worker with an eight-
frame output cap. Ordinary native admission remains closed.

The host test now requires exact full-range PTS `[0, 130, 130.1]`, then checks that
bounded seek 50 returns PTS 48 and that every decoded RGBA byte equals the original
PTS-0 frame. It also checks the later PTS-130.1 sample against its original pixels.
Short and delayed-first fixtures have their decoded full-range PTS checked too. Diagnostics
print only fixture timestamps and equality outcomes, never pixel payloads. An
unexpected trace or image still fails strictly; arbitrary frames are not accepted.

Exact `a95056a125b1d5fe17449f04ba0f03e69259da4d` native run `37620785861`,
job `112790231021`, passed the strict sparse trace and complete RGBA comparisons:
request50 returned48 with identical PTS0 pixels, and request130.1 retained the last
original frame exactly. Linux `37620785940` also passed. The same native test then
failed because the delayed-first writer fixture decoded an additional PTS0 frame,
returning `[0, 0.1, 0.2, 0.3]` rather than the submitted `[0.1, 0.2, 0.3]`.
Build/packaging remained skipped after that failure.

The follow-up retains the proven exact sparse checks. For the leading-gap fixture,
it requires the observed exact decoded trace, then requires bounded seek0 to
return0 and match every RGBA byte/dimension of the full-range leading sample.
A diagnostic reports whether that leading frame repeats the first submitted
sample or is opaque black; neither classification is assumed before native
execution. This is leading-gap reader behavior, not first-following coverage.
The first-following algorithm branch remains source-reviewed without a native
fixture proven to enter it.

Exact published `0041012edc718a53f9f7bdf7e9889507d2a9817f` passed Linux
`37623218882` and macOS `37623218829`, job `112798400955`. Both generated
converter-host tests passed. The leading-gap decoded trace was exactly
`[0, 0.1, 0.2, 0.3]`; bounded/full-range leading RGBA matched exactly. The
leading frame was opaque black and did not repeat the first coded frame
(`repeats_first=false`, `opaque_black=true`). Sparse exact-pixel checks passed
again. Native build and isolated preview packaging also completed successfully.
This evidence is generated-media decoding, not actual native GUI/capture proof.

Before this later test-only correction, local Apple-target platform all-target Clippy with movie-fixtures passed after refreshing
only the affected core/platform Apple outputs in the shared warm target. That is
metadata/type checking, not Apple execution. No actual screen, microphone, user
movie or permission was accessed.

## Timestamp contract

The preview caption reports the requested time and returned AVAssetReader sample
PTS. It does not promise the original coded-sample timestamp or AVPlayer-equivalent
seeking. FrameUnavailable means no acceptable sample was returned within the bounded
reader window; it is not inferred solely from gaps between submitted/coded samples.
The existing bounds still reject returned PTS outside the window. This
specific sparse held sample is proven clipped/retimed without pixel changes by
the exact a950 native run, independently of its later leading-gap failure. No interpolation, color or full-VFR parity
claim follows.

The contemporaneous deferred-image-eviction/trim-width follow-up remains a separate
uncommitted UI change; it is not part of this native test correction checkpoint.
