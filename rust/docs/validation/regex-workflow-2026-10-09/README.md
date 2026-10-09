# Regex Tester validation scope — 2026-10-09

The source plan and intentional differences are in [Regex workflow](../../regex-workflow.md).
This packet contains only source-bound text receipts, no screenshots, binary artifacts,
credentials or real provider material. Synthetic strings are used throughout.

## Frozen inputs and scope

Implementation base 12a8d8b103fd3563feb8862aecf630462e535f68; publication parent 8a2024a5f8ba929f0f56ebbd7dea9966644764da preserves newer timing leaves.
Swift main e43b1c4595c42383e087fe70f38c28d07c31dda0, tree 5ca17248d54fe50c5617d597be7ee6794bde6ea8.
The 17 modified/new source files are bound by source-inventory.json. final-source-manifest.json
includes 260 crate/root-Cargo/CI/document inputs. This is a later frozen inventory,
not a retroactive GUI-time seal: the original 254-entry gui-source-manifest.json omitted
root Cargo.toml/lock. supplemental-build-inputs.json verifies those current bytes
against immutable base and explains the delayed observation. Every build/test used
--locked. post-gui-source-delta.json records only later support test changes;
production remained identical to the ordinary GUI binary in binary.sha256.

## Portable gates

- Core 496 unit +44 integration passed; 4 native tests ignored on Linux.
- Final exact-source default App 552 passed / 2 ignored, including all 17 Regex tests
  and explicit Multiline focused-handle Tab/Space/Enter assertions.
- Recording-fixture App 568 passed / 2 ignored before the final support-only keyboard
  assertion expansion; identical production. Final all-target feature strict passed.
- Minimal App 500 passed / 2 ignored and Core 376 unit +3 integration / 2 native ignored;
  developer Regex is disabled in this graph.
- Default, recording-feature and minimal Core/App all-target strict passed;
  final formatting and ordinary build passed. Upstream proc-macro-error2
  future-compatibility advisory remains visible in Cargo output.
- Actual ordinary binary preserved legacy CLI Unicode/UTF-8 offsets and schema.

local-validation.json includes exact command endpoints, outcomes, original log hashes
and test summaries; commands.jsonl retains failed attempts and successful retries.
Three intended semantic regressions were caught and exact Core restored before full
gates. output-bound-mutant-excerpt.json replaces a 5.3 MB repeated synthetic assertion
payload with its hash/size and a bounded assertion excerpt. Full raw log is retained
locally, not published. The first compiler-format and Clippy-expression fixes and
storage-full linker failure are retained; narrow cache cleanup preceded successful
retry. These are measured command intervals, not implementation or inference effort.

## Desktop and native limits

Cloud Linux Mesa lavapipe ordinary-binary sessions verified light/dark, 740×560 dark,
pattern focus, captures/options/modes/template retention, full Unicode Copy into an
independent new window, chaining, scrolling, Escape and normal close. A pointer
Multiline off/on recheck of ^a verified zero/one matches with UTF-16 [9..<10].
Native Space/Return after pointer focus remained inconclusive; exact dispatched
GPUI coverage passed. Do not infer a source defect or native keyboard success.
Actual OS IME/macOS interactive GUI/hardware GPU/production permissions are unverified.
The 46 exact Foundation vectors and 22 separate cap/anchor characterization cases are
wired into macOS CI but pending the exact published commit. Linux source binding is
not native execution. No broad ICU equality, including cap 1000 behavior, is claimed.
Prior URL/comparison CI closure in the parity ledger applies only to 41bcc source.

## LOC

loc/ledger.json verifies the immutable baseline inventory and explicit nonblank Rust
production/support classification. Delta +1336 production/+1149 support; candidate
cumulative 65297 production / 48391 support / 105 benchmark. Shared files count once under
Box. Five accounting mutations were rejected; independent-loc-review.json confirms
an independent complete reproduction. Concurrent Agent integration needs its own
combined reconciliation. Inference time is unavailable. No production gates changed.
