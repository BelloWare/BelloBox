# Full-window Text Comparison

This local workflow follows the pinned Swift TextComparison engine and
UtilityWorkbench controls: two independent literal drafts, Lines, Words and JSON
fields, a Lines-only Ignore whitespace toggle whose value survives mode changes,
per-side Paste and Clear, Pin this text and Use pinned text, Cancel, Refresh and
Copy Result. There is no comparison Example, Use as Input or Replace Selection.
The legacy CLI `execute("compare", left, right)` contract remains separate and
unchanged. Home and the palette open the new window; the compact palette remains
a read-only Lines preview without interactive comparison/session transfer.

## Ownership and interaction

Each window owns its editor entities, options, request generation and at most one
physical worker. New Window starts with empty drafts and default options. Only the
explicit pin is app-global: a bounded immutable memory snapshot, retained when all
comparison windows close and discarded when the process quits. No draft, result,
pin or clipboard content is persisted. Opening and mode changes do not read the
clipboard; each Paste action reads only on explicit activation.

Changes clear the signed result, colored surface and Copy readiness immediately.
Synchronous editor content/rejection tokens fence Copy even before a queued editor
notification runs, including Cut from a selected read-only result. Session generation, cancellation, options and both drafts
fence publication. A rejected paste/edit retains its draft with a visible error;
changing the other side or mode cannot silently calculate the retained old text.
Editing or explicitly replacing the rejected side recovers. Aggregate-size errors
recover when the combined drafts fit again. Cancel and close retire pending work;
a held physical worker drains before the latest desired request can start.

Literal editor undo/selection are retained across option and appearance changes.
Tab/Shift-Tab visit visible enabled controls and inputs; Enter/Space activate a
focused control once per press. Cmd/Ctrl+N opens an independent window, Cmd/Ctrl+K
opens search, Cmd/Ctrl+Shift+C copies the complete signed result, and Cmd/Ctrl+W
closes only that window. Active composition owns its keys and blocks destructive
draft actions, pin actions and option changes. Native macOS IME/clipboard/window
behavior remains a separately scoped runtime validation.

Lines and JSON use the existing selectable, virtualized read-only editor with
per-row semantic colors, retaining the full signed result. Words use a tool-local
selectable inline surface with removed words struck through. Word shaping is
bounded to 128 spans and 16,384 UTF-8 bytes per visible page; every page is reachable,
including long words split at extended-grapheme boundaries. A single pathological
grapheme larger than 16,384 bytes is split at scalar boundaries to retain the
shaping bound; complete Copy remains byte-exact. Selection and Cmd/Ctrl+C operate
on the displayed inline page; Copy Result always copies all signed rows from all
pages. Pagination is an intentional difference from Swift's full inline paragraph.
No shared editor crate was changed for comparison.

## Semantics and limits

Both input documents together are bounded to 500,000 UTF-8 bytes; no truncation is
used. This preserves the Rust bound instead of Swift's 512,000 bytes per field.
The engine accepts at most 8,000 combined lines, words or flattened JSON fields,
and at most 4,000,000 output bytes. Expensive phases check cancellation.

- Lines normalize CRLF and bare CR, distinguish truly empty input from one blank
  token, and retain the final empty row after a newline.
- Words and whitespace collapse follow the first scalar's Unicode whitespace
  property for each extended grapheme, following Swift Character.isWhitespace.
  Collapse changes output tokens as in Swift; it is not just a matching key.
- Matching uses canonical NFC equality, while same rows retain the left token's
  original spelling. Alignment follows the Swift 6.3.3 CollectionDifference traversal
  using bounded linear-space Myers reconstruction. Exact native runtime evidence is
  separately recorded; no exhaustive tie or Unicode-version equality is implied.
- JSON uses the formatter's existing lossless parser. Bracket-quoted object paths,
  arrays, empty objects/arrays and scalar leaves are retained. Keys are ordered by
  canonical Swift-string identity and canonical duplicates are rejected. Number
  lexemes such as 1 and 1.0 differ. Flattening bounds expanded paths, tokens and
  output before accumulation; it does not duplicate the JSON parser.

The source model's idle gate is asymmetric: whitespace-only first input with an
empty second input is idle; any nonempty second input admits comparison, including
blank-only text. Invalid JSON remains an error in JSON fields mode even if the
other side is empty. Signed Copy uses `+ `, Unicode `− ` and two-space same-row
prefixes, with no status heading and no extra final newline.

## Reference and evidence

The immutable reference is Swift main
`e43b1c4595c42383e087fe70f38c28d07c31dda0`, primarily
`DeveloperTools/InspectionTools.swift:3–59`, `DeveloperTools/DeveloperJSON.swift`,
`Launcher/UtilityWorkbenchModel.swift` and `Launcher/UtilityWorkbenchView.swift`.
The migration handoff is
`f1c21ae4f22b14dc10c5a57651499b05cc37ec92:rust/docs/MIGRATION_HANDOFF.md`.

The new source is based on published URL workflow
`fd434af6056744f26577ad2c78cc2a44f77e7f6a`, whose tree
`c546c85a4850969ef0474090efdecb9a6a8b5e7a` is byte-identical to local URL candidate
`9794521b88fbd8288a1c63d34ed667efab19788d`. Changing that local parent imported no
source changes. URL production and both shared editor crates are preserved byte-for-byte. The
separate URL oracle ID-support correction is carried forward: fixture IDs admit
digits, and a Linux-runnable test checks safe/unique IDs before native execution.

Validation receipts distinguish portable core tests, actual GPUI session tests,
ordinary-build cloud GUI observations, exact-commit CI and native Swift execution.
The hash-bound 115-case native oracle compares acceptance, full row kind/order/text
and complete signed-copy bytes. Native execution is not established by a Linux
binding test. No test count or source LOC count establishes performance, complete
macOS parity or a whole-project completion percentage.

## Source accounting

The [reproducible LOC ledger](validation/comparison-workflow-2026-10-09/loc/ledger.json)
counts nonblank physical Rust lines, including comments and explicit test-only
spans. Relative to published `60504f2ab29d707d47a56282351fd9f75a1ae7ad`
it adds 1,911 production and 1,584 support lines, no benchmark lines; cumulative
counts are 63,961 production / 47,242 support / 105 benchmark lines. The baseline
already includes the separately verified 23-line URL oracle ID-support fix; those
lines are not counted again as comparison additions. Unchanged inventory hashes were checked against Git and five accounting
negative controls were rejected. Shared editor source is unchanged and counted
only once under BelloBox. These are accounting figures, not delivery or quality
percentages.

## Published native closure (2026-10-09)

Exact published source `41bccf467b40daacd2d715c759165aaf0b84ddf9` passed
[Linux 37938156319](https://github.com/BelloWare/BelloBox/actions/runs/37938156319)
and [macOS 37938156317](https://github.com/BelloWare/BelloBox/actions/runs/37938156317).
The native URL oracle completed and the comparison oracle checked 115 exact
acceptance/typed-row/copy vectors using Apple Swift 6.3.3. Native preview notice
negative tests and actual offline ad-hoc package validation also passed. This
supersedes the earlier pending-CI statements for that source only. Native
interactive GUI, platform IME and universal Foundation parity remain unverified.
