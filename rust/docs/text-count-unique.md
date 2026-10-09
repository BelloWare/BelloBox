# Bounded Text count and canonical Unique semantics

Base: Rust `d872f3c659dfb95dd128e99c46cdcdda073e1f80`, tree
`3f212f1e2edf210590e7780a5ac8c0542f3ff929`, parent
`0c4a610c9e3aff8fdc9fe7aaf903c7412e95397e`.
Swift main was verified at 2026-10-09 10:19:53 UTC as
`e43b1c4595c42383e087fe70f38c28d07c31dda0`, tree
`5ca17248d54fe50c5617d597be7ee6794bde6ea8`.

## Source contract and scope

[TextTransforms.swift lines 281–332](https://github.com/BelloWare/BelloBox/blob/e43b1c4595c42383e087fe70f38c28d07c31dda0/BelloBox/Tools/TextTransforms.swift#L281-L332)
uses String.count for total characters but Unicode scalars for the separate
non-whitespace statistic. This asymmetry is retained, not silently standardized.
Extended grapheme segmentation uses existing unicode-segmentation 1.13.3.
Words and token heuristic calculations are unchanged.

Counts normalize CRLF and bare CR exactly like LineTool; empty text has zero
lines and trailing separators retain a final empty row. Other Unicode line
separators are not newly treated as line boundaries. A shared normalization
helper keeps count/line-operation separator handling together.

Unique uses NFC keys through existing unicode-normalization 0.1.25, retaining the
first original spelling and ordering. It does not normalize emitted bytes, trim,
fold case, remove empty rows, or change any other Lines operation.

CLI, Count preview, full popup and complete copied summaries now identify total
characters as graphemes. The scope note/copy identifies the non-whitespace scalar
unit and retains the token heuristic caveat. Pretty remains JSON-only; sort
remains ordinal. Trim/Remove empty Foundation whitespace differences remain
unresolved. No dependency, lockfile, provider, shared editor or native gate changes.

## Fixtures and evidence boundaries

Core fixtures cover decomposed accents, a family emoji, a flag, CRLF grapheme
counting, mixed CR/LF/CRLF, trailing rows, canonical Hangul, reverse first spelling,
case/space distinctions and duplicate empty rows. Execute-route tests cover the
same numeric/copy contract. Actual GPUI preview and popup tests cover Count labels,
explicit complete Copy, Unicode Unique output/Copy/chain and byte-exact Reset.

Final-source validation passed: Core default 505 cases, full minimal Core 372
(one native-only oracle explicitly ignored on Linux in each);
App default 497 and recording-feature App 513 (two existing ignored in each).
Strict all-target lint passed for Core default/minimal and App default/recording;
formatting, minimal App check and a package-clean ordinary build passed.
Both scalar-total and exact-byte-Unique source mutants failed the intended
assertions; original source bytes were restored before final checks.
See the [validation record](validation/text-semantics-2026-10-09/validation.json).
Actual ordinary-binary Linux GUI acceptance passed within the recorded light-mode
scope: mixed Unicode/newline Count, complete popup Copy, compact 224-point layout,
fresh Count handoff, measured 720×520 popup, canonical Unique byte-copy, chaining
and Reset. See [GUI receipt](validation/text-semantics-2026-10-09/gui-acceptance.json). A tiny pinned-source Swift
Foundation oracle was pending at this original Linux checkpoint; the terminal
0717 native result is recorded below. Its
whitespace probes are research-only, not authority to change Trim/Remove empty.
Unicode-version/locale differences and full Text Tools parity remain qualified.

## Observed timing and failures

Read-only audit: 2026-10-09 10:19:32–10:20:43 UTC (71 seconds).
Implementation started 10:21:43 UTC. The first focused core compile failed because
a mechanical edit referenced but did not insert normalized_newlines. The helper
was inserted and the failure log/timing retained before rerunning. Timings are
observed wall-clock command/research windows, not model-inference durations or
completion estimates.

A second compile-only failure came from importing GPUI's test attribute through
a wildcard in the new preview test module, recursively resolving its generated
`#[test]`. Narrow imports corrected it. Focused core (10 tests) and the App
text-filter suite then passed. Both original failure logs are retained.

The first full no-default-features Core invocation exposed an existing ungated
JSON formatter integration target referencing the feature-disabled developer
module. The test target now has `#![cfg(feature = "developer-tools")]`, matching
that module's availability; no production feature changed. Full minimal Core
and default Core subsequently passed, explicitly verifying that the default
JSON formatter integration target still runs all 17 cases and minimal excludes it. The failing
invocation remains in evidence rather than being replaced by a library-only pass.


## Source accounting and handoff

The verified delta is +9 production, +502 support, +0 benchmark nonblank Rust
lines; cumulative totals are 60,991 / 44,859 / 105. The baseline inventory and
five negative controls were checked, counting shared editor code only once.
No dependency/lockfile changes occurred. The initial and corrected command logs
and observed timings are retained alongside the source-mutant results.

The ordinary binary was sealed after final gates at 2026-10-09 10:33:30 UTC:
SHA-256 `cb4e1d9d855817639fd18697c6c387aa77f06c6da1c67665a7dcd59e5cde8c56`.
Its product-source manifest is
`0e53ad2afc986c31dbae650c138775b53e9e14c83dd1e408223f9a5148bcaf32`.
At that original checkpoint, independent native oracle and exact-published-commit
CI were pending; the bounded Linux GUI acceptance passed. The later terminal
0717 result below supersedes that pending status without changing the old receipts.


## Reproducible native-host oracle gate

`bellobox-core/tests/text_swift_oracle.rs` embeds the fixed 11 Count, 7 Unique and
14 whitespace diagnostic groups, binds the entire checked-in Swift source and
its exact LineTool/TextStats excerpt by SHA-256, and rejects source/fixture drift.
The Rust driver typechecks on Unix. Linux runs its source/fixture binding test;
the native comparison is explicitly ignored there, not reported as a pass.
The combined full default/minimal Core suites and strict all-target lint passed
after integration; the default JSON formatter integration still runs 17 cases.
One initial Rust Command-borrow typecheck failure was corrected and preserved in
[native-oracle logs](validation/text-semantics-2026-10-09/native-oracle/linux-timings.json).

The existing public-repository macOS job inventories the named native test, then
its existing full Core suite runs that test once with captured output. It compiles
only the hash-bound Foundation declarations, compares all four Count numbers and
Unique output bytes to Rust for the bounded vectors, and prints macOS/Swift/SDK,
Foundation, locale and command-time metadata. Whitespace membership, Trim and
Remove empty are diagnostics only. Processes have 120-second/256-KiB output bounds.
No application launch, capture, permission request, credential, provider, paid
service, new job or dependency is introduced.

The original gate was pending until exact-commit macOS CI completed; the verified
terminal result below now supplies that evidence. The peer request alone was never
a substitute for that result. Passing
this gate establishes only its recorded host/version/vector scope, not universal
Foundation or Unicode-version parity. The original ordinary binary and its six
postimages remain unchanged; the candidate source manifest adds the separate test
and workflow while retaining the original GUI/binary manifest binding.


## Terminal exact-commit validation (2026-10-09 11:20 UTC)

Commit `0717b247370d0e30b35789fbd190b39eeecf09e8`, tree
`4c6f216b032bdda1d2b7f0560b46b0ca664b97b8`, passed both exact CI jobs:
[Linux run 37921262198](https://github.com/BelloWare/BelloBox/actions/runs/37921262198)
(695 seconds job wall time) and
[macOS run 37921262155](https://github.com/BelloWare/BelloBox/actions/runs/37921262155)
(933 seconds job wall time). These are observed CI durations, not model inference
or interactive app performance.

The native oracle passed all 11 Count and 7 Unique vectors on arm64 macOS 26.6.2
(build 25G83), Swift 6.3.3 (`swiftlang-6.3.3.1.3`), SDK 26.5, Foundation 5026.6,
locale `en_US` / `en-US`. The 14 whitespace groups remain diagnostic only; no
Trim/Remove Empty parity claim or implementation change follows. Oracle compile
was 1.806 seconds and execution 0.142 seconds on that runner. This establishes
only the pinned source, fixtures and recorded host/version scope.

See the [terminal receipt](validation/text-semantics-2026-10-09/terminal-ci/terminal-receipt.json),
[bounded native excerpt](validation/text-semantics-2026-10-09/terminal-ci/native-oracle-excerpt.log)
and [native observations](validation/text-semantics-2026-10-09/terminal-ci/native-oracle-output.json).
Historical pending records and original ordinary-binary/GUI receipts are preserved.
These results apply to the Count/Unique checkpoint, not the subsequent URL editor.
