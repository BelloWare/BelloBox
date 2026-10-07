# Converter image retirement and trim field acceptance

Actual Linux cloud computer-use checks ran on 2026-10-07 at 13:36:57–13:37:48 UTC.
The immutable combined candidate was
`4a4cf2f4f55ea798292674be4c0cbefc6f57b100024b2e439d7c1da3c2c78e53`
(142,431,896 bytes). `combined-source-inputs-candidate1.json` records all 181
compiled source/artwork/project inputs, independently rehashed before these
checks. The candidate includes separately attributed, then-unpublished OCR work;
it is not described as a disposal-only executable. The six converter/disposal
source hashes exactly match this checkpoint's LOC manifest. Later OCR caption and
inline layout repairs are separate and do not replace these original screenshots.

All six images are unedited CUA JPEG originals. `gui-screenshots.json` records
capture times, byte sizes, binary linkage and SHA-256. The actual movie route is
explicitly labeled synthetic and has no input movie file.

- `59-converter-start-prefix-at-end-caret.jpg`: Start `0.642` is fully visible
  with the caret at its end; End is `3`. Parsing/precision are unchanged.
- `60-converter-export-review-after-trim.jpg`: Apply and real Rust GIF export
  produced a retained result. Independent decoding of `trimmed-review.gif`
  confirms 320×180, 36 frames, 2,360 ms total, looping, 32,094 bytes. Exact hash
  and all frame delays are recorded in `output-verification.json`.
- `61-converter-retained-movie-switch.jpg` and
  `62-converter-retained-gif-switch.jpg`: Movie/GIF switching retained review.
- `63-converter-rechoose-cleared-old-review.jpg`: Reloading the generated source
  cleared the old result through the actual rechoose/reset path.
- `64-converter-reopened-no-stale-review.jpg`: Closing and reopening did not
  resurrect old review state. The app was closed and the desktop released.

This actual converter window was 1180×812. The smaller 560×600 Start/End layout
is covered by the deterministic GPUI regression, not by a claim that these
screenshots used that smaller viewport. Tests verify `0.642`, `120.000` and
`7200.642` with every character position visible at the end caret.

Before combined integration, six new regressions, all 50 converter tests and all
245 app tests passed. On the combined candidate, all 285 app tests, 277 app-only
no-debug-assertions tests, strict app/platform Clippy and the minimal-feature
check passed. These broader counts include separate OCR work. The public GPUI
regressions observe real image-disposal dispatch only after all live windows
return to App; they do not instrument the private atlas or measure GPU bytes.
No actual native movie decoding or capture was exercised by these Linux checks.
