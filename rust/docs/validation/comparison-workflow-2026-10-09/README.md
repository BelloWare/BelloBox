# Text Comparison validation, 2026-10-09

Final ordinary binary SHA-256:
`60e7eb1913bf67c10beb73344cc88015979d56338c4ddf51cf9a4eba118a603b`.
The 238-file compiled-source manifest is
`bb947687fa7c913c609762fa97167d364b3702e423dd793fb02a803a7f099b78`.
All entries matched after final GUI closure. Binary and screenshots are not
published here. Native macOS CI and the exact 115-vector Swift oracle remain
pending; Linux binding tests do not establish native execution.

## Test stages, without transferring counts between source revisions

- Core final engine/default: 529 passed, 3 native ignored. Minimal: 379 passed,
  2 native ignored. The Core source did not change afterward.
- Initial complete App matrix: default 531 passed / 2 ignored; minimal 500 / 2;
  recording-fixtures 547 / 2. All three strict configurations passed.
- Cloud GUI discovered Compare-local compact input configuration swallowed Enter.
  Inputs were made noncompact and result backgrounds translucent. Added two
  regression tests. Their first timing assertions needed a second virtual debounce
  advance; those failed attempts are retained. Then full default App passed 533 / 2,
  recording-focused comparison passed 19, and all three strict configurations passed.
- Final small delta clears misleading Working locally status on accepted errors
  and symmetrically fences stale C/X at Compare capture. Final source passed 7
  session + 20 UI/Words tests, all-target default strict App Clippy, formatting,
  and a clean ordinary build. The earlier full feature matrix is not represented
  as a full rerun of this final delta.
- Shared editor: Core unit 78 passed / 1 ignored, separate editor_invariants target
  1 passed, UI unit 39 passed. Thus the complete checked Core count is 79 passed / 1
  ignored, not a missing test. Shared source remains byte-identical.
- Packaging policy: 11 Python tests, shell syntax, diff whitespace checks passed.
  Final notice documentation was included in the final policy rerun.

`commands.json` contains exact commands, timestamps, wall durations and exits.
`phase-*.txt` concatenates exact raw command output into bounded text chunks;
failures and warnings are retained. The upstream proc-macro-error2 future-
incompatibility advisory remains; it is not a newly introduced Rust warning.

## Negative controls and review

Seven mutations failed the intended assertions: empty-second admission, explicit
Copy token fence, generation publication, physical-worker ownership, trailing empty
line, canonical matching and accepted-error status. Each was restored and its
package cleaned before subsequent final builds. Two C-only capture mutants
survived the dispatched C/X test, including one with a fresh Copy positive control.
GPUI redraws dirty windows before dispatch; Compare render synchronizes tokens and
clears stale output first. The X guard is defense-in-depth, not a demonstrated
stale-copy vulnerability. `mutation-results.json` preserves these outcomes.

Independent review found no remaining blocking source issue in its bounded scope;
its exact final source/binary binding and separate executed checks are recorded in
`independent-review.txt` and `independent-review-manifest.json`.

## Interactive cloud acceptance

`gui-receipt.json` separates the superseded Enter/status observations from the
final binary. Final dark 740×560 and light 820×660 task-window checks passed:
multiline/Shift-Enter, explicit Paste/Clear, literal tabs, undo across modes,
keyboard Pin/Use Pin, process-lifetime pin, Lines/Words/JSON, error recovery,
read-only result selection and scrolling, and complete Copy. Native editor AX
content exactly matched all 2,000 signed Unicode Lines and all 6,000 signed Words
across 47 pages, including copying while viewing page 2. Oversized 500,001-byte
Paste rejected without truncating the retained draft. Both final processes exited
normally with unchanged binary hash. Some cloud font emoji glyphs use fallback
outlines; literal Unicode Copy content was verified.

The search shortcut was invoked but its separate popup was not captured in the
final scoped GUI pass. Home launch was observed on the superseded status binary;
route source is unchanged. Cancellation races and native input composition are
covered by programmatic tests; this is not a macOS IME, accessibility or universal
pixel-parity claim.

## Accounting and timing

The LOC ledger validates every immutable baseline Rust inventory hash and rejects
five accounting negative controls. Baseline `60504f2a` already includes the 23 URL
oracle support lines, so those are excluded from comparison additions. Comparison
adds 1,911 production / 1,584 support / 0 benchmark lines; cumulative counts are
63,961 / 47,242 / 105. Shared editor is counted once under BelloBox.

`timing.json` distinguishes mixed activity windows, nested command durations and
GUI process intervals. Inference time is unavailable and was not estimated.
