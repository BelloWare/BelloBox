# Native raw-movie oracle and test-build cfg correction

Baseline: `8712627de818058badbc4f7378b0822d792feea2`. These changes are test/support
only. No production reader, writer, admission gate, fixture bytes or historical
GUI evidence is changed. The physical Rust delta is **0 production / +169 support
/ 0 benchmark**, yielding **52,593 / 31,349 / 105**. The complete changed-file
hashes and classification are in [source-and-loc.json](source-and-loc.json).

## Diagnosis and bounded correction

[Original macOS run 37691415517](https://github.com/BelloWare/BelloBox/actions/runs/37691415517)
failed the raw movie's byte-identity assertion at frame 0. Of 24,576 RGBA values,
11,008 RGB values differed, all by exactly one code value; all alpha values were
exact. The other six recording failures were SERIAL mutex poison cascades.
The raw fixture is genuinely RGB24, not H.264: all 221,184 mdat bytes match the
finite recipe exactly, and independent installed FFmpeg decoding produces all
294,912 expected RGBA bytes exactly. See [raw fixture proof](raw-fixture-proof.json)
and [original failure summary](original-macos-failure.json).

The native test observes AVAssetReader's requested BGRA samples after DeviceRGB
CoreGraphics rendering. Swift `Recording/GIF/GIFTranscoder.swift` uses the same
output format and rendering design. Apple's [AVAssetReaderTrackOutput API](https://developer.apple.com/documentation/avfoundation/avassetreadertrackoutput)
provides decoded output in the selected format; Quartz [color rendering](https://developer.apple.com/library/archive/documentation/GraphicsImaging/Conceptual/drawingwithquartz2d/dq_color/dq_color.html)
is not a raw sample-byte extraction contract. The available observation does not
isolate the rounding to AVFoundation versus CoreGraphics, and does not establish
a universal AVFoundation ±1 guarantee. It supports correcting this fixture's
oracle without changing the source-shaped production behavior.

The corrected oracle keeps every pixel of all 12 ordered frames, exact dimensions,
exact storage length, exact opaque alpha, PTS error strictly below 1 ms, exact
frame count/end-of-stream and reader completion. Only RGB permits an absolute
error of one code value. A larger error remains a failure to investigate, never
an automatic reason to increase tolerance. An independent test retains exact
raw payload identity, including corruption at its first and final bytes.
The generated AVAssetWriter output remains a separate lossy H.264 test with
its unchanged timing/count/sentinel assertions.

The serial test mutex protects no data. Recovering its advisory poison lets
independent tests execute while preserving the first panic. It does not reset
native admission, ownership or worker state. A deliberate panic/recovery test
checks this separately.

The unrelated default-App Clippy failure on `8712627` was an unused test-only
`shutdown::panic_next` helper. Its sole caller is behind `recording-fixtures`;
the helper now has the same test-plus-feature cfg. No ordinary-build behavior
changes and no lint allowance is added.

## Validation and limits

- Linux platform default: 164 passed, 3 ignored
- Linux platform recording-fixtures: 164 passed, 3 ignored
- Strict all-target platform Clippy: default and recording-fixtures passed
- Strict all-target App Clippy: default and recording-fixtures passed
- Workspace cargo fmt check passed
- Eight deliberately broken oracles were rejected: RGB error 2, alpha error 1,
  skipped pixels, PTS, dimensions, lengths, raw payload and restored poison
  cascades. All final oracle/test hashes match the restored control hashes.

Logs are preserved beside this file. The first App-Clippy staging attempt lacked
two unchanged compile-time inputs (the app icon and project.yml); both were
restored from the exact baseline before the successful checks. That staging
failure is retained separately. The dependency future-incompatibility notice
for proc-macro-error2 2.0.1 is unchanged and did not fail strict project Clippy.

The negative-control runner requires an **isolated writable checkout** and a
separate evidence directory. With cached official dependencies, run:

    python3 run-negative-controls.py --repository /path/to/isolated/checkout --evidence /path/to/evidence

No App codegen/test execution or GUI replay was performed for this correction.
Native macOS raw decoding, generated-writer execution and full exact-commit CI
remain pending. Prior GUI artifacts attest to their original recorded source
and binary hashes only; they do not validate this candidate.
