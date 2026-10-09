# Text Tools palette and independent popup snapshot

This bounded workflow follows Swift main
`e43b1c4595c42383e087fe70f38c28d07c31dda0`:
[compact preview](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Launcher/LauncherUtilityPreviews.swift#L104-L214),
[handoff values](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Launcher/LauncherHandoff.swift#L30-L36),
[popup creation](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/UI/SelectionOverlayController.swift#L550-L567),
and [popup model and footer](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/UI/TextToolsPopupView.swift).
It does not establish full Text Tools engine or native macOS parity.

## Workflow

The 224-point compact row has Case, Encode, Decode, Pretty, Hash, Lines and Count.
Case/Lines have an option menu; Encode/Decode have direct choice controls. The
row has selectable complete primary output, four individually copyable hash rows,
count tiles and explicit complete Copy. It has no input editor or Use as Input.
Independent case, encoding, decoding and line-operation choices survive category,
row and query changes. Replacing the palette selection retires the old state.

Open snapshots the original selection and all choices into a **fresh** popup
session. Unlike the JSON workbench handoff, no worker/entity/cancellation identity
is transferred. Destination creation failure, close during admission or shutdown
refuses Open without consuming the palette. Successful Open latches closure before
another same-turn action can open the original generic route. Only successful
explicit snapshot opens update the existing tool/category usage metadata.

The popup focuses its literal input and keeps its own original selection. Paste,
Clear, Reset, retained category options, eligible Use as Input and guarded Copy
complete the workflow. Reset restores opening text after multiple transformations;
Hash/Count never chain. Ordinary native Copy remains editor-owned; modified result
Copy is scoped to the full popup. Category number shortcuts, Tab traversal and
held activation guards do not open another tool or type into the input. Minimum
popup dimensions are 720×520, matching the source.

## Bounded ownership and stale results

Each owner retains one admitted original, latest draft and option set. It runs at
most one physical calculation and coalesces edits into one latest pending request.
A 60 ms Rust scheduling delay is an implementation choice, not a Swift timing or
latency claim. A synchronous engine call may drain after cancellation/close, but
its old generation cannot publish or copy. Retired entities cannot be revived by
retained callbacks. Popup close retires its session even when a test/other holder
still retains that entity. Closing a palette never retires a destination session.

The preview defers transformation above 64,000 UTF-8 bytes. Full input admission
remains 500,000 bytes; complete output plus structured copies is bounded to
4,000,000 bytes. Explicit popup Paste keeps an oversized rejected editor draft
visible, rejects it before cloning into calculation state, clears old result/Copy
and leaves category changes rejected until the draft is repaired or Reset. No
input/output is silently truncated. Snapshot admission rechecks the original and
current retained input before cloning; rejected input cannot open an older draft.
Copy and chain also compare current editor bytes against the calculation input,
covering an editor change whose notification has not yet run.

## Existing engine differences remain explicit

The UI reuses existing text engines; no parser, dependency, lockfile or shared
editor changed. Category scope labels disclose:

- Pretty is **JSON only** in Rust. Swift Text Tools uses its separate
  [PrettyPrinter](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Tools/TextTransforms.swift#L348-L373),
  which detects/reindents JSON, XML, HTML and brace code. The JSON Formatter's
  token-preserving/sorting successor does not establish Text Tools Pretty parity.
- Lines sorting is ordinal; Swift uses localized case-insensitive ordering.
- Character counts use Unicode scalars and tokens a generic heuristic. Swift
  grapheme counts and provider/model controls are not reproduced here.
- Full Unicode case/decoder equivalence is not newly claimed.

Hash presentation strictly validates the existing engine's four algorithm rows,
widths, hexadecimal payloads and exact compatibility footer, refusing extra rows.
UI Copy serializes complete `Algorithm: digest` rows, or one complete digest for
an individual Copy. It never parses the generic CLI help paragraph; CLI output
semantics stay unchanged. Count results come directly from typed count fields and
remain labeled as scalar/heuristic data.

Replace-in-place, native selection identity, global activation/anchoring, native
materials/IME/accessibility and token-model settings remain separate gaps. All
capture/recording/movie/provider/OCR-upload/vault gates remain unchanged; no
credentials, provider requests or implicit clipboard access are added.

## Validation

See the [validation record](validation/text-tools-2026-10-09/validation.json).
The first corrected focused App run passed 31 tests, including 24 new state,
launcher and popup tests. They cover all four retained choices through actual
launcher-to-popup opening, original text/Reset, independent owners, admission
failure/close/quit, oversized explicit Paste, stale/busy Copy, bounded worker
coalescing and late success/error retirement, complete hash serialization, menu
keyboard ownership and held full-popup controls.

Final-source default App tests (493 passed, two existing ignored), recording-feature
App tests (509 passed, two existing ignored), strict all-target lint in both
configurations, minimal-feature check, formatting and the package-clean ordinary
build passed. The immutable ordinary binary passed the exercised Linux GUI workflow: seven
compact categories, retained options, fresh popup, chaining/Reset/Copy, independent
windows and 720×520 layout. See [qualified GUI receipt](validation/text-tools-2026-10-09/gui-acceptance.json)
for unexercised dark/oversize/repeat details. QR Save failed on both this binary
and the previous 0c4 sealed binary in the same session; no Text Tools-specific
regression was demonstrated. The unchanged candidate then passed native Save
(one 522×522 PNG) and second-dialog Cancel using the existing official FileChooser
portal in an isolated cloud-desktop session. No product fix was needed. See the
[preserved differential](validation/text-tools-2026-10-09/qr-save-differential.json) and
[portal retest](validation/text-tools-2026-10-09/qr-portal-retest.json).
Exact-commit CI and native interactive acceptance remain separate pending gates.
