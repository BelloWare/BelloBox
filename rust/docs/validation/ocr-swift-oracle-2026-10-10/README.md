# Local OCR: first macOS run and Swift oracle — 2026-10-10

Rust's Apple Vision OCR (`bello_platform::macos_native::recognize_text`) had
never run on macOS. This record runs it on generated images (rendered text, no
captured pixels) next to an oracle built from Swift Box 0.0.77's own OCR code,
and records the parity defects found and fixed.

Mac: Apple M3 Max virtual machine, macOS 14.8, Xcode 16.1. Vision prints
`IOServiceMatchingfailed for: AppleM2ScalerCSCDriver` in this VM; recognition
is unaffected.

## Inputs

`make-fixtures.swift OUT` draws six PNGs (`fixtures.sha256`): `paragraphs`
(title, two paragraphs, footer), `columns` (two columns and a footer), `mixed`
(English and Japanese lines), `tall-two-bands` (700 × 3600 px, just over one
3200 px band), `tall` (1000 × 8000 px, 240 lines) and `blank`. `paragraphs`,
`columns` and `blank` are also checked in as test data
(`crates/bello-platform/tests/data/ocr`).

## Oracles

- **Image oracle** (`swift-src/`): `main.swift` plus `Shims.swift`, compiled with
  `OCRTileSegmenter.swift`, `OCRBoundingBoxConverter.swift`, `OCRModels.swift`
  and `OCRResultFormatter.swift` taken unchanged from `e43b1c4`
  (`BelloBox/Screenshot/OCR/`). It runs `MacVisionOCRService.recognize` for an
  imported, unannotated image: the request configuration is copied from
  `performVisionOCR` with `OCROptions.default`; `OCRImagePreprocessor` is left
  out because for such an image it renders the image unchanged.
- **Layout oracle** (`layout-src/main.swift`, same four Swift files): Swift's
  reading order, overlap de-duplication and plain-text formatting on 80
  fixed-seed region sets, plus the tile bands for nine image heights. Its output
  is the checked-in `crates/bello-platform/tests/data/ocr_layout_swift_oracle.json`,
  which `ocr_layout::tests` asserts exactly on every platform.

Build: `swiftc -O -o swift-ocr-oracle swift-src/*.swift <the four files>`; the
Rust side is `rust-runner/` (a standalone crate on the checked-out
`bello-platform`).

## Results

Before the fix (`1ecbaa3d` release binary, `--ocr`), Rust joined Vision's
observations in Vision's order (`outputs/*.rust-before.txt`):

| Fixture | Swift 0.0.77 pipeline | Rust before |
|---|---|---|
| paragraphs | blank line between title, paragraphs and footer | no paragraph breaks |
| columns | row by row across both columns | each column top to bottom |
| mixed | paragraph break after the heading | no break |
| tall (8000 px) | read in 3 overlapping bands: 228 of 240 numbered lines (Vision's own misreads aside) | **nothing recognized** (Vision scales the whole page down) |
| blank | "No text was found in this screenshot." | empty text reported as success |

After: `bello-platform/src/ocr_layout.rs` ports `OCRTileSegmenter` (bands and
overlap de-duplication), `OCRBoundingBoxConverter` (pixel rectangles and
reading order) and `OCRResultFormatter.plainText`; the macOS backend reads an
upright image in Swift's bands, collects each line's text and rectangle, and
returns Swift's text or its no-text error. On every fixture the Rust output is
**byte-identical** to the Swift oracle (`outputs/*.swift.txt` vs `*.rust.txt`,
the blank errors in `blank.*.err`), with the same time (3 interleaved runs, ms):

| Fixture | Swift | Rust |
|---|---|---|
| paragraphs | 864 / 834 / 814 | 862 / 846 / 847 |
| columns | 715 / 735 / 698 | 739 / 717 / 751 |
| mixed | 536 / 482 / 670 | 521 / 505 / 507 |
| tall-two-bands | 1854 / 1911 / 1746 | 1789 / 1764 / 1747 |
| tall | 7045 / 7017 / 7249 | 7269 / 7131 / 6781 |

The work is Vision's in both apps; Rust neither gains nor loses time.

Known differences that remain: an image whose file orientation is not "up" is
read whole in its rotated frame (Swift renders imported images upright before
tiling); de-duplication compares texts by bytes, Swift by canonical
equivalence (both readings come from the same recognizer). Swift's own
pipeline is imperfect on `tall-two-bands` (rows 1–15 are not returned and some
rows are separated by blank lines); Rust now reproduces that output exactly,
which is what parity means here.

## In CI

`ocr_layout::tests` (all platforms) asserts the layout oracle. On macOS,
`vision_text_comes_out_in_swift_reading_order_with_paragraph_breaks` runs Vision
on the three checked-in fixtures and asserts the reading order, paragraph
structure and no-text error rather than exact words, since Vision's wording can
change between macOS versions.
