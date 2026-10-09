# Bounded full-window URL & Query Inspector

The separate `url_editor::Draft` GUI API and `url_ui` full window preserve ordered
query items, duplicate names, empty values, bare flags, literal plus signs and
stable row identities. Source userinfo is retained privately for rebuilding; it
is not used for authentication. No network, navigation, persistence, or URL logging
is performed. Clipboard reading happens only on explicit Paste. The existing
CLI `developer::url_tool` JSON report and query-edit options are unchanged.

## Workflow and intentional differences

Each window starts an independent draft. Initial input, Paste and Example inspect
locally; subsequent typing requires **Inspect URL**. This explicit bounded action
is a Rust workflow difference from Swift's scheduled automatic parse. A full
result exists only after **Build URL**. Source/field/row changes invalidate it;
Copy Result and Use as Input verify synchronous editor tokens before using the
complete result. A rejected edit/paste cannot revive an old result with Build:
Clear or an explicit valid source replacement is needed first.

Eight query rows are mounted per page. First/Previous/Next/Last make every row
accessible and the count/page label makes pagination visible. All rows remain in
the draft, including the complete 3001-item test fixture. Page navigation flushes
current edits; an active IME composition blocks paging, Build and source
replacement. Stable IDs prevent removal from retargeting an adjacent row.
Tab/Shift-Tab traverse mounted editors; editor-native navigation/selection remain
in place. No compact editor/session handoff is claimed: the palette preview is
read-only inspection and directs users to the full window.

## Bounds and lexical contract

Source and each editable field are capped at 500,000 UTF-8 bytes without
truncation. The aggregate edited draft is independently bounded to 500,000 bytes;
Build validates the complete data and caps output at 4,000,000 UTF-8 bytes. The
shared editor's default 8 MiB behavior is unchanged; URL hosts opt into the lower
limit. Monotonic content/rejection tokens cover queued notifications, rejected
paste and IME edits without cloning all query items during each frame.

Only complete HTTP(S) URLs are accepted. Parsing separates lexical components,
strictly checks percent escapes/UTF-8 and uses the existing `url::Url` dependency
for validity checks. It does not use WHATWG-normalized spelling as output. Source
host case, explicit default/leading-zero port spelling, empty paths and dot
segments are intentionally retained. Empty fragments/query lists are removed on
Build, as the source draft's optional-field model requires. Decoded path/query/
fragment text is percent-encoded per component; plus is literal, never form-space.
Control characters/backslashes in source URLs, ambiguous userinfo, malformed
bracket authorities, explicit empty ports, whitespace/delimiters in edited hosts,
non-absolute nonempty paths and ports outside 1–65535 fail explicitly.

This bounded contract is not universal Foundation URLComponents parity. Native
host/IDNA handling, percent-reserved path spelling, leading-zero ports, unusual
query segments and punctuation require the pinned native oracle. The hash-bound
`url_swift_oracle` test records synthetic parse/build observations and gates only
its declared exact subset; diagnostic differences remain gaps. Linux GPUI and
portable tests do not establish macOS focus/IME/AppKit/clipboard equivalence.

## Evidence

Focused source tests cover lexical components, ordered flags/duplicates/plus,
3001 rows with a final-row edit, invalid authority/UTF-8/ports, stable identity and
aggregate limits. GPUI tests cover independent windows, close/reopen, explicit
Build, same-callback stale-copy fencing, Clear/chain, rejected-paste latching and
IME navigation blocking. Shared-editor tests cover lower-limit preservation and
atomic content/rejection tokens. Validation receipts distinguish test, ordinary
build, cloud GUI, native oracle and CI results; no test count is a performance or
whole-migration completion claim.


## Source binding and accounting

The reference is Swift main `e43b1c4595c42383e087fe70f38c28d07c31dda0`,
verified by the parent on 2026-10-09. Primary source sections are
`InspectionTools.swift:143–191`, `UtilityWorkbenchView.swift:266–314`, and
`UtilityWorkbenchModel.swift:48–50,164–169`. Full-file SHA-256 bindings:

- InspectionTools: `4b74934aa3bdf1bfe88a2d25c5bfd6a63c58b536f7d81f4f03fc9efdab5f2fc7`
- UtilityWorkbenchView: `5e9d060b6303fbb4a7c7f4c2cd9a30b92802f14f60be1efeca44df159c1cb2f4`
- UtilityWorkbenchModel: `a77ad5fe5f3d10f0348eb299af81f66833b2323c6e03817cb4fdc52d2650493a`

The [LOC ledger](validation/url-workflow-2026-10-09/loc/ledger.json) counts
nonblank physical Rust lines including comments, with explicit positive
`cfg(test)` spans and child tests separated. Relative to immutable `0717b247`,
this closure adds 1,059 production and 776 support lines, no benchmark delta.
Cumulative counts are 62,050 production / 45,635 support / 105 benchmark lines.
Shared editor changes are counted once under Box. All unchanged inventory hashes
were verified against Git and five accounting mutations were rejected. These are
source accounting figures, not parity percentages, quality or performance scores.

## Validation closure (2026-10-09)

Cloud Linux default/minimal Core and App suites, recording-fixture App suite,
shared editor suites, formatting, and strict Clippy are recorded in the
[validation receipts](validation/url-workflow-2026-10-09/). The additional minimal
strict run found feature-inapplicable JSON test imports/helper; narrow positive
feature guards repaired those support-only warnings. The eight original JSON
engine tests remain enabled and passing by default. Earlier compile failures and
negative-control compile blocks are retained rather than counted as passing tests.
Five targeted semantic/invalidation mutations were killed. An incorrectly scoped
first stale-token mutation survived a redundant notification optimization and was
not counted as a kill; the corrected authoritative readiness mutation failed its
regression test. Package-scoped cleans separated mutant and trusted final builds.

Actual cloud GUI receipts bind each run to its ordinary binary. The earlier broad
run covered light/dark at measured 740×560, independent windows, ordered query
editing, complete copy/chaining, and the 3,001-row last-page fixture. A later
lint-clean production run added scheme/port/path/fragment edits, invalid-port
recovery, complete copy into a fresh window, and a last-row edit/chain round-trip.
The intervening renderer-only change and final support-only guards are explicit;
old-binary observations are not presented as exact-final-source execution.
Native Foundation execution remains pending exact-commit macOS CI; real platform
IME and hardware-GPU behavior were not exercised on this cloud desktop. Wall times
are command observations, not benchmarks. Model inference time is unavailable.

## Published native closure (2026-10-09)

Exact published source `41bccf467b40daacd2d715c759165aaf0b84ddf9` passed
[Linux 37938156319](https://github.com/BelloWare/BelloBox/actions/runs/37938156319)
and [macOS 37938156317](https://github.com/BelloWare/BelloBox/actions/runs/37938156317).
The native URL oracle completed and the comparison oracle checked 115 exact
acceptance/typed-row/copy vectors using Apple Swift 6.3.3. Native preview notice
negative tests and actual offline ad-hoc package validation also passed. This
supersedes the earlier pending-CI statements for that source only. Native
interactive GUI, platform IME and universal Foundation parity remain unverified.
