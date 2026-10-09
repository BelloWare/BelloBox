# Positive native first-image timestamp: bounded diagnostic

The generated positive-composition movie's first actual decoded image is at
**60/600 seconds (0.1 seconds)**. The unmodified full-range native reader
returns exactly three images at **60/600, 120/600, 180/600**. Their complete
RGBA bytes match the zero-origin control's three images at **0/600, 60/600,
120/600**. Both movies retain their original bytes and identity after reads.

This demonstrates the positive-first-image prerequisite on this Mac. It does
**not** validate Rust before-first seeking, App export/retention, native IME,
live capture or an interactive workflow. Those require separate reservations
and checks. The earlier DelayedFirst normalization evidence remains unchanged.

## Scope

Exact base: `8448568578a8eadd76b0c3ddb5bd7022595e4402`, tree
`c5fa82a1ce43de5710644fb37a5a5413f89760b0`.
[Assignment](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6072270145),
[exact claim](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6072290553),
[directory acknowledgment](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6072320402).
Every addition is in this directory; all base files and modes are unchanged.
No platform/App source, dependency, native admission, capture/tool/vault gate,
existing evidence, screenshot, user media or external provider is involved.

## Construction and observations

`GenerateSamples.swift` creates three 64 x 48 synthetic RGBA images and uses
VideoToolbox to encode independent baseline H.264 IDR samples. Frame reordering
is disabled. The original RGBA, AVC sample bytes and decoder configuration are
retained in `samples/`. H.264 is lossy: matching decoded bytes between the two
movies is claimed; equality to pre-encoding RGBA is not claimed.

`author_mov.py` authors both QuickTime containers directly from those elementary
samples. No AVAssetWriter movie, movie template or post-read timing patch is
used. The media payloads, sample sizes, decoder configuration and sync sample
tables are identical. Both have one self-contained data reference, no edit
list and no composition-shift atom. All three decode durations are 60/600.
The candidate's composition offsets are each +60 ticks; the control's are 0.
Movie/track/media extents include the final presentation end: 240/600 for the
candidate and 180/600 for the control. Decode duration is 180/600 for both.

The independent fixed-file parser `verify_container.py` checked the complete
atom layout, sample tables, extents, IDR NAL types, references, offsets and
payload bytes **before** native reads (`container-preflight.json`). The movies
were made read-only for the original run; pre/post receipts retain their size,
SHA-256, device/inode, modification time and mode. Git does not retain the
0444 filesystem mode, so the receipts describe the original native run.

`NativeRead.swift` leaves AVAssetReader's default time range at zero through
positive infinity. It performs independent nil-output-settings passthrough and
BGRA decoded reads, retaining every returned record. It neither skips nor
retimes images. Visible BGRA pixels are converted to RGBA for byte comparison.

| Observation | Zero-origin control | Positive-composition candidate |
| --- | --- | --- |
| Container/native compressed media PTS | 0, 1/10, 1/5 | 1/10, 1/5, 3/10 |
| Container/native compressed media DTS | 0, 1/10, 1/5 | 0, 1/10, 1/5 |
| Actual decoded image PTS | 0, 1/10, 1/5 | 1/10, 1/5, 3/10 |
| Passthrough media/control records | 3 / 4 | 3 / 5 |
| Decoded images | 3 | 3 |
| Native segments | One nonempty segment | One nonempty segment |
| Passthrough / decoded completion | Completed / completed | Completed / completed |

The candidate's passthrough trace contains a PTS0 **zero-sample control record**;
it has no image and no sample bytes. It is preserved in `native-read.json`.
The decoded trace starts with an actual image at 1/10 and contains no black
lead frame. Decoded DTS and duration are invalid in the native output; the
trace preserves that fact without synthesizing values.

`verify_native.py` checks all **114 CMTime records**: 84 numeric rational/seconds
pairs, 26 invalid values and four positive-infinity request durations. Expected
sample/segment times are compared as exact integer fractions. It also checks
complete encoded and decoded bytes, source integrity, original control records,
reader completion, and the strict positive-first-image condition. Both MOVs
reproduce byte for byte from the preserved AVC samples using the Python author.

Movie SHA-256 (2,308 bytes each):

- Control: `ef59f7b291b1fbbe16969b65b97d5918e03224527fcbd68f7c984ce4ba7fed16`
- Candidate: `abd5e23b328507ea0c1c4f894e2d7d34cb5493ae0011b86413cb2821bd5959a8`

## Validation record and limits

Native run: 2026-10-09 01:29:14-01:29:15 UTC, macOS 14.8 (23J21), arm64,
Swift 6.0.2, macOS SDK 15.1. Both final Swift compilations had no warnings or
errors. Two earlier encoder compile failures exposed the Swift name/labels for
the block-based VideoToolbox API; original source and diagnostics are preserved
under `attempts/`. Neither failed compilation ran a native experiment. The
encoder's `IOServiceMatchingfailed for: AppleM2ScalerCSCDriver` diagnostic is
retained verbatim; encoding and all four reader passes completed successfully.

`compile-receipt.json` binds final Swift sources and local executable hashes.
`native-execution.json` records exact commands, timestamps, outcomes and raw
output hashes. `portable-verification-receipt.json` records verifier sources
and byte-identical MOV reproduction. `final-validation.json` binds base-file
preservation and the final evidence checks. `SHA256SUMS` covers delivered files
except itself. Executables remain in the local cache and are represented by
hashes; no screenshots are published. These parsers cover this small authored
pair, not arbitrary MOV input. No Rust test suite was rerun for this evidence-only
change; prior product test receipts are not relabeled as new results.

Portable verification from this directory:

```sh
python3 -B verify_container.py .
python3 -B verify_native.py .
shasum -a 256 -c SHA256SUMS
```

To repeat native work within an authorized environment, compile the two Swift
sources with `xcrun swiftc -target arm64-apple-macosx14.0` (`NativeRead.swift`
also needs `-parse-as-library`), generate samples into a new directory, author
new movies, verify containers, then read into another new directory. Exact
arguments and binary hashes are in the receipts. Keep originals and traces.

Apple's [composition-offset specification](https://developer.apple.com/documentation/quicktime-file-format/composition_offset_atom)
defines presentation time as decode time plus the sample offset. Its
[composition-shift description](https://developer.apple.com/documentation/quicktime-file-format/using_composition_offset_and_composition_shift_least_greatest_atoms)
explains the absent-shift case. These guided construction; the native behavior
reported here was measured independently.

## Proposed next fixture reservation (not started)

After dot reviews this evidence, propose a fixture-only follow-up in these exact
paths, at the then-acknowledged base:

- `rust/crates/bello-platform/src/movie/macos/fixture_media/positive-composition.mov`
- `rust/crates/bello-platform/src/movie/macos/fixture_media/zero-origin.mov`
- `rust/crates/bello-platform/src/movie/macos/fixtures.rs`
- `rust/crates/bello-platform/src/movie.rs` (fixture-only declarations)
- `rust/crates/bello-platform/src/movie/macos/tests.rs`
- `rust/crates/bellobox-app/src/gif_converter/native_host_tests.rs`
- A separately agreed new validation directory.

Use the proven generated bytes through the existing fixture gate. First prove
the full-range native PTS and byte identity in Rust, then compare a before-first
request (for example 0.05 seconds) with that first actual image using the existing
selection policy. Retain the DelayedFirst PTS0/black-frame control and existing
retirement/cancellation/source identity/prior-output checks. Any production
seek, decoder, admission or permission change remains outside that proposal.
