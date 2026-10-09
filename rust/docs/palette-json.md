# Interactive JSON palette and independent-window transfer

This bounded app-layer slice follows Swift main
`e43b1c4595c42383e087fe70f38c28d07c31dda0`, especially
[LauncherToolPreviews.swift](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Launcher/LauncherToolPreviews.swift),
[LauncherModel.open](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Launcher/LauncherModel.swift#L354-L373),
and [UtilityWorkbenchWindowController](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Launcher/UtilityWorkbenchWindowController.swift).
It does not claim complete JSON or native macOS parity.

## User workflow

The JSON palette row has Pretty, Minify and Validate controls, selectable complete
output, Cancel while busy, eligible Use as Input and explicit Copy. Swift has no
primary JSON input editor inside this compact row; this slice does not add one.
Use Clipboard/replacement selection supplies input, and Use as Input can replace
it with the current successful output. Query changes and visiting another row
retain the JSON draft and mode. Replacing the palette selection discards that
session. The source row reserves 224 points within the existing clamped palette.

Open transfers the same session to the existing independent JSON window. The exact
input, mode, result/error and running calculation survive. The destination keeps
the existing 820×660 / minimum 740×560 layout and full editable input. New Window
creates a fresh empty/default session. Searching from a full window leaves that
window open. Closing a window cancels only its own session. The JSON-specific
Use as Input now refuses Validate reports, unchanged/empty/oversized results,
busy work and errors. Copy has the same current-result guard.

Clipboard reads are explicit. Search-query Copy and selected read-only output Copy
retain their text responder ownership; this new row does not intercept ordinary
Ctrl/Cmd+C. Preview Escape returns focus to search, with held Escape fenced against
immediate dismissal. Mode/footer controls have bounded Tab traversal and activation
key/release consumption. The existing QR physical-key/Save and Clock transport
retirement policies are unchanged. Full-window Ctrl/Cmd+Shift+C copies the result;
marked input retains its existing input-method ownership.

## Session ownership and bounds

`json_session.rs` owns one generation and physical worker loop per session. Its
initial 220 ms debounce follows Swift's scheduling constant; this is not a measured
latency claim. While a parser runs, newer edits replace only the latest desired
state. There is no queue of per-keystroke parser jobs. At most one parser runs for
that session, with one current bounded draft and one immutable active input
snapshot; views, output strings and engine allocations have additional separate
bounds. This is not a whole-process memory cap.

Changing input/mode clears old visible output immediately. Token, owner and current
request checks fence stale successes and errors. Cancel invalidates publication;
synchronous parsing already in progress is not forcibly interrupted. New work
waits until that physical operation returns. Closing/retiring a session fences
all late publication. Retained old palette views can retire only Palette-owned
sessions; they cannot cancel an already transferred Window owner.

Destination creation is fallible. Palette ownership is removed only after creation
succeeds, the destination still exists, shutdown has not started and the session
is still transferable. Failed/closed/quit-during-admission attempts do not consume
the source draft. Ordinary transfer does not restart an in-flight calculation. A cancelled/no-result
session restarts only on explicit Open, matching Swift; any previous physical
parser still drains before the replacement runs.
The full host subscribes to the session and creates no second calculation owner.

The existing 64,000-byte preview and 500,000-byte draft bounds remain. Preview
oversize retains the complete draft, hides stale output and defers calculation
until full-window adoption. A rejected larger input cannot transfer an older valid
draft. The full editor may retain that rejected user text for repair, while the
calculation session does not duplicate or parse it. The unchanged core engine also
bounds combined input/options and output (4,000,000 bytes), so a draft near its
maximum can receive an explicit core limit error. Nothing is truncated.

## Historical formatter differences at the original palette slice

The following describes the earlier palette-only implementation at `0c80343`
and its predecessor. It is historical, not the current formatter contract.
The bounded formatter successor below supersedes these differences for the JSON
Formatter route only; other developer tools retain their existing semantics.

This slice reuses the existing Rust JSON engine at
[developer.rs](https://github.com/BelloWare/BelloBox/blob/75cd3609bc053a6869b996706642637fac8536ce/rust/crates/bellobox-core/src/developer.rs#L104-L125).
The Swift specification is the custom
[DeveloperJSON.swift](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/DeveloperTools/DeveloperJSON.swift),
not Foundation JSONSerialization. Swift recursively sorts object keys and retains
raw numeric lexemes. Existing Rust preserves insertion order and can normalize
number spellings such as exponent notation and negative zero; its canonical
Unicode key equality also differs from Swift String. No parser/dependency change
is included here. The successor corrects the old misleading Validate status to
`Valid JSON.`; validation makes no exact Swift number-lexeme parity claim.

The initial workflow test expecting Swift's sorted-key output failed against this
pre-existing engine. Its characterization is retained beside the validation
record. The final transfer test deliberately uses whitespace-changing Minify to
prove state preservation without asserting sorting. The tested integer
9007199254740993 is preserved; this isolated precision example does not establish
universal raw numeric spelling or complete JSON semantic parity.

## Current bounded formatter successor

The JSON Formatter now uses a separate bounded parser that preserves raw number
substrings, recursively orders keys using NFC comparison while retaining emitted
key spelling, and rejects canonical-equivalent duplicates locally. Arrays retain
order; `jsonPointer` keeps its pre-existing distinct-key semantics. Successful
Validate remains exactly `Valid JSON.`. No dependency, lockfile, or session-owner
change is involved. The existing 500,000-byte input/options, 4,000,000-byte output,
20,000-value and depth-64 guards remain; output is checked while streaming.

[Formatter scope and evidence](validation/json-formatter-2026-10-09/README.md)
records 501 Core tests, 19 App JSON regressions, strict/minimal checks, two real
source-mutant controls, and focused ordinary Linux GUI acceptance. A pinned Swift
oracle matched all 51 acceptance outcomes and all 16 accepted Pretty/Minify byte
outputs across 17 vectors on macOS 14.8 arm64 / Swift 6.0.2 / the recorded Foundation
build. This is bounded host/vector evidence, not deployed-runtime, universal
Unicode, error-text or Workbench validation-count parity. Complete native UI
acceptance and broader feature parity remain separate gates.

## Validation and remaining gates

See [validation record](validation/json-palette-2026-10-09/validation.json).
Nineteen focused JSON tests and the r1 default app suite (469 passed, two
existing ignored) passed locally: actual GPUI launcher/full-window
adoption, retained-source cleanup, failure and close/quit during destination
admission, independent close/reopen, query/output copy ownership, oversize refusal,
Validate chaining and stale/cancelled results. A test-only held worker exercises
98 coalesced edits and verifies one active calculation plus one latest successor;
ordinary admission and scheduling logic remains in use.

The r1 default app regression, strict all-target Clippy, minimal-feature check,
formatting and package-clean ordinary build passed on its frozen source. Its
recording-fixtures suite (485 passed, two existing ignored) and strict Clippy also
passed. The immutable r1 ordinary binary then passed observed Linux GUI transfer,
independence, keyboard/copy, malformed recovery and minimum-size checks, with
local Clock/QR navigation regressions; see [GUI receipt](validation/json-palette-2026-10-09/gui-r1.json).
A minimal successor changes only Core Validate's misleading preservation claim to
`Valid JSON.` and adds one regression. Its 22 Core JSON and 19 App JSON tests,
Core/App all-target strict Clippy, formatting and package-clean ordinary build
passed. The full 469/485 matrices remain prior-r1 evidence; exact final-commit CI
remains pending. The successor's focused Validate display, Copy and transfer/no-chaining
GUI recheck passed; see [receipt](validation/json-palette-2026-10-09/gui-r2.json). Actual
macOS focus, menu, IME and accessibility acceptance remains a separately authorized
native gate. No shared editor crate, dependency, lockfile, capture/tool/vault gate,
provider consent policy, native permission, signed release or update behavior is
changed. This workflow remains entirely offline apart from existing ordinary
local preference bookkeeping on an explicit tool open.
