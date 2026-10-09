# Regex Tester full-window workflow

## Source and implementation plan

The immutable Swift reference is main
`e43b1c4595c42383e087fe70f38c28d07c31dda0`, tree
`5ca17248d54fe50c5617d597be7ee6794bde6ea8`. The implementation starts at
Rust/timing head `12a8d8b103fd3563feb8862aecf630462e535f68`, whose latest
comparison source is `41bccf467b40daacd2d715c759165aaf0b84ddf9`.
The immutable migration handoff is
`f1c21ae4f22b14dc10c5a57651499b05cc37ec92:rust/docs/MIGRATION_HANDOFF.md`.

Source specification:
- `BelloBox/Launcher/UtilityWorkbenchView.swift:56–69,189–193`: pattern first,
  Ignore case, Multiline anchors, three output modes, replacement in Replace,
  and separate highlighted input.
- `BelloBox/Launcher/UtilityWorkbenchModel.swift`: empty pattern/default multiline,
  whitespace-only idle gate, live cancellation, retained drafts and chain rules.
- `BelloBox/DeveloperTools/InspectionTools.swift:100–139`: bounded collection,
  numbered capture details, extraction and replacement from the same matches.

The work is split into a typed Core inspection, an independently owned cancellable
App session, a literal-editor GPUI window, then focused engine/real GPUI tests,
an independent native Swift oracle, strict builds and scoped desktop review.
Existing comparison/URL session, focus and presentation patterns are reused.
No native production gate, dependency, Swift source or shared editor is changed.

## Implemented workflow

The developer route opens an independent Regex Tester window with pattern-first
focus. Input, pattern and replacement drafts stay in memory. Ignore case defaults
false and Multiline anchors true. Matches, Extract and Replace choose complete
outputs from one inspection; changing mode does not run the expression again.
Replacement survives leaving Replace. Every real draft/option edit invalidates
old results immediately and debounces one physical background worker; edits while
it drains coalesce to the latest request. Cancel, edit, rejection, close and
independent New Window ownership fence old completions. There is no implicit
clipboard read or external request.

Matches contains numbered details and optional capture groups, including
`(not matched)`. No matches yields `No matches.`. Extract joins complete matches
with newlines; Replace applies numbered templates to the same collected ranges.
Copy Result exports the complete selected output. Use as Input is available only
for nonempty changed Extract/Replace outputs that fit the 100000-byte input gate.
Results exceeding that input gate remain copyable when within the output bound.
The highlighted preview uses exact input bytes; empty matches retain their detail
ranges/counts/replacement positions but cannot paint a nonempty highlight span.

Tab/Shift-Tab traverse available controls and editors. Enter/Space activate a
focused button; Enter in pattern/replacement does not trigger a tool or insert a
line break. Literal pasted line breaks remain preserved in those fields. Search,
New Window and Close use Cmd-K/N/W (Ctrl on Linux); Escape leaves the window open.
Ctrl/Cmd-Shift-C copies complete output. Native clipboard Copy/Cut from stale
result/highlight selection is intercepted before editor actions. Marked IME input
suspends calculations and output; editor notifications resume after same-byte
unmark or commit even when no Changed event is emitted. Native platform IME
runtime remains a separate verification target.

## Bounds and intentional dialect limits

Input: 100000 UTF-8 bytes; pattern: 4000; replacement: 100000. These are admitted
before input-sized cloning/allocation. Compiled regex/DFA cache size limits are
2000000 bytes each. At most 128 capture groups and 1000 complete matches are
retained. Details, extraction and replacement share a combined 4000000-byte output
budget; each append is checked before expansion/allocation, with no unchecked
per-match replacement intermediate. No partial or truncated result is published.
These replacement/group/combined-output limits are additional portable safeguards.

A 300 ms cooperative budget and cancellation checks run between phases, matches,
and replacement segments. A single regex-library compile/search operation cannot
be forcibly interrupted, so this is not a hard real-time deadline or a whole-path
memory/performance guarantee. One physical worker per window avoids accumulating
in-flight computations. This is not an app-wide worker-concurrency bound.

Patterns use the existing Rust regex dependency, not ICU. Lookaround and pattern
backreferences are unsupported. Unicode classes, case folding and non-LF line
separator/anchor behavior may differ from Foundation. No broad ICU claim is made.
UI detail ranges use UTF-16 code units as Swift does; editor highlights use UTF-8
byte offsets. Both are typed separately. The legacy CLI still reports its original
UTF-8 byte offsets and JSON schema, flags i/m/s, 128000/4096 input/pattern bounds,
default pattern and Rust template syntax.

UI templates use numeric `$0`, `$1`, etc., unmatched/nonexistent captures as empty,
and backslash escaping, independently of Rust `Captures.expand`. Numeric template
width follows the number of capture groups; extra digits remain literal. Named
replacement syntax is not added. A source-bound native corpus checks these exact
rules rather than treating portable unit tests as Foundation evidence. The UI's
collection loop preserves zero-width matches immediately following nonempty
matches, which Rust `captures_iter` suppresses; this is also an explicit oracle case.

## Evidence and remaining limits

The native oracle pins the complete InspectionTools Swift SHA-256
`4b74934aa3bdf1bfe88a2d25c5bfd6a63c58b536f7d81f4f03fc9efdab5f2fc7`
and the unchanged RegexTester excerpt SHA-256
`cc1420898946283208a721b646c49dabb11bf3d00abbdd8c53c3a2bc096a1a52`.
Its 46 synthetic vectors compare acceptance, complete UTF-16 ranges, detail,
extraction and replacement bytes. The macOS job verifies that this test exists;
the ordinary complete Core suite executes it. Linux checks hash binding and
compilation but intentionally skips Foundation execution. Final native status
must be tied to the exact published commit, never inherited from prior CI.

Compact palette sessions remain read-only and do not transfer an interactive
Regex draft. AX Replace Selection, native glass/accessibility fidelity, actual
macOS IME, hardware-GPU behavior and native interactive GUI remain unverified.
The full-window workflow does not grant permissions, access credentials, persist
drafts, publish screenshots, enable capture/movie/recording gates or create releases.

The exact 46-vector shared corpus is separate from 22 native characterization
cases: 999/1000/1001 matches (including empty matches at the end), CRLF/CR/NEL/
Unicode line separators and multiline/whole-input anchors. Rust admits exactly
1000 collected matches; Swift's progress callbacks may stop at that boundary.
The native test reports both observations explicitly rather than assuming cap
identity or weakening the exact shared-corpus assertions. Both engines must
reject the 1001 cases. Pending native observation must remain labeled pending.

### Bounded draft admission difference

Unlike Swift's model, which retains an oversized literal input and then reports
an engine error, this portable window rejects an initial input over 100000 bytes
before mounting an editor and shows a blank field plus the limit error. Oversized
Paste or editor edits preserve the previous field exactly, invalidate outputs,
and require that field to be repaired before calculation. This is explicit
rejection, not truncation or whole-draft behavior parity. The source selection
and other independently owned windows are unchanged; a regression checks both.

### Scoped desktop review

The ordinary Linux binary was exercised with cloud Mesa lavapipe, in light and
dark appearance, including a 740×560 dark window. Pattern-first entry, numbered
captures, Ignore case, all three modes, retained replacement, complete Unicode
Copy into an independent new window, Extract/Replace chaining, Escape, scrolling
and normal owner close were observed. A separate pointer recheck of `^a` across
two lines showed Multiline unchecked with zero matches, then checked with one
match at UTF-16 `[9..<10]`.

Space/Return after a pointer-focused flag did not establish the expected native
keyboard state in that CUA session. This remains a delivery/focus uncertainty,
not a diagnosed source defect or a native keyboard pass. Dispatched GPUI keyboard
regressions separately assert Ignore case, Tab to the exact Multiline focus
handle, Space off and Enter on. Actual platform IME remains unverified despite
real GPUI marked-text lifecycle tests. No screenshots are published.
