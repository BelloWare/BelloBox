# Strict GIF completion repair — 2026-10-09 Singapore

Participant: migration-peer. Exact base `abd7247c25c1736f7a4fd6a34032e7af9d7a1f5d`,
tree `167c1e1e0209320f725153cbcd2063ddd134bbe5`. Dot reserved the five source
paths in [coordination comment 6070788241](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6070788241).
This candidate is for review and integration by dot into `rust`.

## Result and cause

The unchanged native host test previously failed when exporting the generated
mirrored movie, with `InvalidOutput` from staged GIF validation. The preserved
794-byte GIF contains exactly 3,072 indices and an EOI code in each frame.
The locked gif 0.14.2 decoder's pixel fill can finish before consuming EOI.
Advancing metadata uses a discard sink that can skip the remaining compressed
input, then reports `EndCodeNotFound`. This is reproduced by the portable
regression and the external completion probe; it is not inferred from a green
test on different generated pixels.

The shared private decoder now fills the exact RGBA buffer, explicitly drains
completion with one additional RGBA pixel of capacity, and requires DataEnd
without any additional pixel. Strict EOI checking stays enabled. Errors are
terminal, extra decoded pixels are rejected, and no bytes or pixels are padded.
Export validation retains its structural pass, loop/delay/frame-count checks,
size limits and cancellation checks. Preview uses the same strict decoder and
checks exact EOF through the recovered buffered reader after the trailer. The
extra byte of read capacity distinguishes physical EOF at the byte limit from
an EOF manufactured by `Take`. Full-canvas bounds and opaque palette entries are
checked before a frame is returned; RGBA storage transfers to preview without
retaining another full frame in the decoder.

No encoder, dependency, lockfile, native adapter, App source, capture, tool or
vault gate changed. `gif.rs` changes only private module declarations. The
original native failure is retained in `baseline/app-native-movie.log`.

## Validation

Host: macOS 14.8 (23J21), arm64; Rust/Cargo 1.91.1, Swift 6.0.2, macOS SDK 15.1.
Commands run in the candidate `rust` directory with offline locked dependencies,
two build jobs, an external target directory, deployment target 14.0, isolated
configuration and no provider credentials. Full command/environment/timestamps,
exit codes, raw-log hashes and test binary hashes are in the adjacent receipts.

| Check | Result | Evidence |
| --- | --- | --- |
| Exact-base App native host tests | 1 passed, 1 failed | `baseline/app-native-movie*` |
| Core GIF suite after repair | 49 passed, 0 failed, 0 ignored | `repair/core-gif-r2.*` |
| App converter suite with `movie-fixtures` | 52 passed, 0 failed, 0 ignored, including both unchanged native host tests | `repair/app-gif-r1.*` |
| Unchanged native/replay diagnostic program | All 6 native and 6 replay exports succeed; no staging remains | `repair/probe-run.stdout`, `repair/native-replay-verification.json` |
| Workspace format check | Passed | `repair/fmt-r1.*` |
| Strict core Clippy, all targets | Existing macOS `settings.rs:253` `needless_return` warning blocks `-D warnings`; exact base reproduces it | `repair/clippy-r1.*`, `repair/clippy-base-r1.*` |
| Core Clippy, all targets, only that lint allowed | Passed; no source-level lint suppression | `repair/clippy-scoped-r1.*` |

The final 14 new regression tests cover the original strict failure, independent
RGBA hashes, preview pixels/delays/terminal None/rewind, first and last frame
missing/early EOI, extra output, invalid dictionary/palette references, missing
subblock terminator, invalid delay/bounds, missing trailer/trailing bytes,
dictionary growth, legal one-byte subblock reblocking, fragmented reads,
cancellation and terminal failure state.

Completion read failures and cancellation are injected at the first and last
frame's subblock terminator through the same staged-validation routine. A
separate raw decoder test proves exact pixel fill has already succeeded before
the injected read. These validator/Target tests preserve the prior destination
and remove staging. They do not claim an injected OS failure in a native export.
The existing public export cancellation/publication tests also pass.

An initial extra test attempted to inject at the EOI byte. Bytewise gif decoding
can fetch that byte during pixel fill, so its assertion correctly failed. The
offset was corrected to the terminator and independently checked. The failed
run (`core-gif-r1`: 48 passed, 1 failed) and earlier development runs are retained.
No production change was required for that test correction.

## Byte and pixel evidence

The mirrored Once fixture is exactly 794 bytes, SHA-256
`6629b6303725b54e46052c802b3d1c430e1fd665b6194920946a5971f2063d35`.
The mirrored Loop variant is 813 bytes, SHA-256
`bceb757c9968bd8d1b97f9ae57337d29145f940f96dd5d921af0d854c2c546cc`.
All six orientation/loop outputs match both the pixel replay and the preserved
baseline diagnostic bytes exactly. The repair changes readback, not encoding.

Dot's independent [fixture oracle](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6070835864)
produces these RGBA SHA-256 values, now asserted by the regression:

- Frame 0: `ff8565eb548b7addfec7a86e9620afb730975d4e38859011d1f1efd72a9e5655`
- Frame 1: `8b6e6610ec5d5eefdcde5f4e539fdaba94eed64319ffb4bbb4278c5f8763f64d`

The copied oracle, with one terminal newline, has dot's reported SHA-256
`4f740252057254e1de01be577aa34b27b53347615225612590161e39bf4f8226`;
its local replay result is included. It is an opaque, noninterlaced, full-canvas
fixture oracle, not a production or general GIF validator. The raw issue
extraction's extra blank line remains in local evidence. The earlier diagnostic
program still prints the legacy strict decoder error for the mirrored output;
those lines are intentional reproduction evidence, not repair export failures.

## Scope and provenance

`SOURCE-MANIFEST.json` identifies all five source postimages and verifies all
1,339 other tracked base paths byte-for-byte by Git blob. `SHA256SUMS` covers the
evidence package. Intermediate binaries remain in the external cache; their
hashes are preserved in receipts. The original assessment is also recorded in
[comment 6070748316](https://github.com/BelloWare/BelloBox/issues/1#issuecomment-6070748316).

Only generated fixture MOV/RGBA/GIF data was used. No real user media, application
launch, new host permission, capture/Accessibility read, Keychain/vault access,
real provider call, signing, installation or release validation is claimed.
The ordinary native movie gate remains closed. The observed AppleM2ScalerCSCDriver
notice appears in both the original assessment and passing native runs and is
retained without attributing the GIF failure to it. Broader codec/color/timing
coverage remains outside this bounded repair.
