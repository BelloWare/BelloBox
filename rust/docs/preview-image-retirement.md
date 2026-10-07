# Converter preview retirement and trim-field layout

GPUI 0.2.2 temporarily removes the current window from `App.windows` during
its callbacks. Calling `App::drop_image(image, None)` in such a callback therefore
misses that window's atlas. Converter rechoose/reset, prefetched GIF presentation
and retained close could take this path. This is a source-proven disposal risk;
no GPU-byte leak or peak-memory measurement was made.

The shared converter disposal helper retains the image in an App-owned deferred
closure until borrowed windows have returned, then calls the real GPUI eviction.
It is independent of the converter entity and window lifetime. All six converter
cleanup call sites use it, including asynchronous source-frame replacement.
Movie/GIF navigation continues to retain both reviewed images intentionally.
Decoder ownership, physical media admission, cancellation and native activation
are unchanged.

Six GPUI regressions exercise same-window rechoose, paused/prefetched GIF resume,
asynchronous source-frame replacement, close with a retained entity, removed
window/entity release, and trim-field layout. The disposal observer calls the
actual helper and checks that every remaining live window is accessible before
eviction. GPUI's private test atlas is not instrumented, so this establishes
correct dispatch and ownership rather than GPU allocation accounting.

Trim fields now reserve 120 logical pixels without flex shrinking. The 560-pixel
converter layout test checks Start and End at end-caret values `0.642`, `120.000`
and `7200.642`, with unchanged text and distinct visible character positions.
An earlier movie screenshot showed a short `642` tail; whether its prefix was
clipped or the entered spelling omitted a leading zero was not established.
The layout regression is explicit and does not invent a proven historical input.

Before integration with OCR staging, all 50 converter tests and all 245 app tests
passed, as did strict all-target app Clippy and formatting. Actual desktop checks
on the immutable combined candidate passed: full `0.642` at the end caret,
real trimmed export, retained Movie/GIF switches, rechoose reset and close/reopen.
The actual viewport was 1180×812; the 560×600 case is deterministic GPUI coverage.
The candidate also contains separately attributed OCR work. Its full 181-input
manifest, six original screenshots and independently decoded GIF are recorded in
`validation/preview-disposal-2026-10-07/QA-report.md`. Source hashes and the separate
six-file count delta are in
`validation/preview-disposal-2026-10-07/manifest.json`.

The delta is 15 production and 234 test/support nonblank Rust lines, including
comments, with no benchmark change. Over published `0041012edc718a53f9f7bdf7e9889507d2a9817f`,
this gives 48,167 production / 24,690 support / 105 benchmark. OCR work is excluded.
