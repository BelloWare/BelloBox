# Native first-following fixture assessment — verified limitation

Six generated QuickTime/H.264 cases all produced a decoded frame at PTS 0 on
this Mac. None meets the strict precondition for proving the native seek
policy's first-following branch. The first-following acceptance gap remains
open; no production or existing test-suite source was changed.

The assessment was reserved by dot in
[comment 6071355490](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6071355490)
and acknowledged in
[comment 6071374243](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6071374243).
Exact base: `2d9b170ed81b33ed3adfa080dfbadc89b2f21f2f`, tree
`5e63d54223d869606f40be48715c865727c69441`. All 1,415 base files and modes
remain identical; every addition belongs to this validation directory.

## Observed timelines

All samples submitted by the probe have duration 0.1 seconds. Each movie has
one 64 × 48 video track and self-contained media references. Times below are
seconds; full rational CMTime values, flags, reader statuses, attachments,
container segments and per-frame RGBA hashes are in `swift-timing.json`.

| Generated case | Writer session start | Submitted PTS | Decoded full-range PTS | Leading empty edit / decoded black frame |
| --- | --- | --- | --- | --- |
| Regular control | 0 | 0, .1, .2, .3 | 0, .1, .2, .3 | No / no |
| Existing delayed-first control | 0 | .1, .2, .3 | 0, .1, .2, .3 | Yes / yes |
| Session starts at first sample | .1 | .1, .2, .3 | 0, .1, .2 | No / no |
| Negative session start | −.1 | 0, .1, .2 | 0, .1, .2, .3 | Yes / yes |
| Long leading gap | 0 | 3.1, 3.2, 3.3 | 0, 3.1, 3.2, 3.3 | Yes / yes |
| Explicit native empty edit | 0 | 0, .1, .2, .3 | 0, .1, .2, .3, .4 | Yes / yes |

The last case uses `AVMutableMovie.insertEmptyTimeRange` before any diagnostic
read, adding a 0.1-second empty range at the beginning. Both its original
writer output and the resulting self-contained movie are preserved. No movie
or trace is modified after reading to manufacture a positive timestamp.

For the gap cases the PTS0 frame hashes to exactly 64 × 48 opaque black RGBA:
`5e9d5b686a0c4122c77d69144a5874dcbbcd05c1b4717bb11d8d730cc17a6659`.
The raw sample-size table contains only the submitted encoded samples; the
extra black frame is decoded output associated with the leading empty edit,
not an extra submitted image. Compressed-reader output includes zero-sample
control records; these are retained and counted separately from media samples.

Apple documents the writer's mapping between submitted times and movie time,
including a leading empty edit when the first submitted sample is later than
the session start. See
[startSession(atSourceTime:)](https://developer.apple.com/documentation/avfoundation/avassetwriter/startsession(atsourcetime:)).
The local SDK declaration in `AVAssetWriter.h` was also inspected. These API
semantics explain the container construction; the decoded behavior above was
measured, not inferred from the documentation.

## Checks and limits

Host: macOS 14.8 (23J21), arm64; Rust/Cargo 1.91.1, Swift 6.0.2, SDK 15.1.
`TimingProbe.swift` completed all six cases. `verify_assessment.py` verifies
unchanged original MOV hashes, self-contained data references, completed
compressed/decoded readers, rational timing records, exact decoded sequences,
opaque-black hashes, and rejection of every case by `first_decoded_pts > 0`.

The fixed-file parser preserves raw `stts`, `ctts`, `cslg`, `elst`, media/movie
timescales and sample counts in `assessment.json`. It interprets these
QuickTime `ctts` deltas as signed and cross-checks the resulting table PTS
against independent compressed-reader media PTS. It keeps the stored `cslg`
fields separately without applying an additional shift. This is a verifier
for these exact small generated files, not a general MOV parser. QuickTime
composition deltas can be negative; see Apple's
[composition offset atom](https://developer.apple.com/documentation/quicktime-file-format/composition_offset_atom).

The existing Rust source was rebuilt after cleaning the three affected
packages in the external target directory. All **74 tests passed**, with
**0 failed and 0 ignored**:

| Suite | Passed | Command after `cargo test --offline --locked` |
| --- | --- | --- |
| Platform movie | 23 | `-p bello-platform --features movie-fixtures --lib movie:: -- --nocapture --test-threads=1` |
| Core GIF | 49 | `-p bellobox-core --lib recording::gif:: -- --nocapture --test-threads=1` |
| App native host | 2 | `-p bellobox-app --features movie-fixtures --bin bellobox gif_converter::native_host_tests:: -- --nocapture --test-threads=1` |

The unchanged App control reports exact bounded/full-range RGBA equality for
DelayedFirst, PTS0, `repeats first=false`, and `opaque black=true`. Thus its
existing passing assertion is retained as normalization evidence. No new App
first-following assertion is presented as passed. Reader retirement,
cancellation, source identity, prior-output protection and native admission
checks remain in the passing existing suites.

No Rust compiler warnings appeared in these three logs. The standalone Swift
probe emitted six synchronous-API deprecation warnings and one unnecessary
`try` warning; the exact source and compiler log are retained. The native
`AppleM2ScalerCSCDriver` notice and expected caught-panic output from existing
GIF tests are also retained. No interactive GUI, capture, TCC/AX, user media,
real provider/key/vault, signing, installation or release was exercised.

## Preserved diagnostic corrections

- `swift-r1/`: the first probe retained only eight compressed records and
  stopped on the explicit-empty-edit case's ninth record. The final probe
  keeps up to 32 compressed records, including zero-sample control records;
  the decoded image bound remains eight. Original source, partial output,
  compiler/driver logs, execution receipt and movies are preserved. This was
  a diagnostic bound failure, not a production test failure.
- `container-parser-r1/`: rejected draft interpretation of version-0 `ctts`
  as unsigned. Its derived large PTS values are not accepted evidence.
- `container-parser-r2/`: rejected extra application of the stored `cslg`
  shift. The independent compressed PTS check failed. The final script keeps
  raw fields, uses signed table arithmetic, and requires the independent PTS
  match. No native trace or MOV was rewritten during these corrections.

`swift-probe-execution.json` binds the final Swift source, compiled binary,
commands and original output hashes. `assessment-execution.json` binds the
final Python verifier and output. `final-validation.json` binds all base
postimages, Rust test identities, binaries and logs. `SHA256SUMS` covers every
delivered file except itself. Native executables remain in the local evidence
cache and are represented here by hashes; original generated media are included.
The full Git whitespace check flags original test-log trailing spaces and blank
EOF lines only; every non-log addition passes (`diff-check.json`). Raw logs
are preserved byte for byte.

To verify the delivered assessment without accessing native APIs:

```sh
python3 verify_assessment.py .
shasum -a 256 -c SHA256SUMS
```

To rerun the same Swift strategies on an authorized Mac, compile
`TimingProbe.swift` with `xcrun swiftc -target arm64-apple-macosx14.0`, then run
that executable with a **new, nonexistent output directory**. Preserve stdout,
stderr and the generated directory before comparison. New encodings may have
different file hashes; the included receipts refer to the original run.

## Proposed next route — unassigned

Stop this bounded assessment here, as dot requested. A separate claim could
investigate an independently authored, self-contained generated MOV whose
media sample timing begins positively without a leading empty edit, with raw
sample tables and an independent compressed/uncompressed reader trace proving
that precondition before adding a Rust fixture. This is an untested proposal,
not a claim that the route will succeed or that all other fixture strategies
are impossible. It needs dot's new reservation. Production seek policy must
not be changed to discard the observed black frame to make a test pass.
