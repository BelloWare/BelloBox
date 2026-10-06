# Pure World Clock copilot proposal planning

This is the source `WorldClockCopilotSuggestion.plan`/`parts` calculation from
WorldClockAIResolver.swift, plus stable identifier validation from
WorldClockModels.swift. It does not parse responses, send provider requests,
apply changes, persist anything, enable UI or alter ordinary clock behavior.

A permitted time proposal becomes an action only at an absolute difference of
at least 60 seconds. Location planning independently respects the host gate;
valid proposed IDs are deduplicated in order, while current IDs remain literal.
Add preserves existing order. Replacement with no valid IDs cannot erase the
current list. Reference selection uses exact resulting membership, with the
source fallback when a replacement removes the current reference. Summaries
keep time, location and reference order. Proposed and actionable parts remain
separate so applying time alone does not hide an outstanding location proposal.

Nineteen focused tests cover fractional positive/negative threshold boundaries,
formatting callback use, all host-gate combinations, raw aliases and duplicates,
invalid/unchanged/reordered replacements, anchor precedence and fallback,
malformed current names, uncapped results and partial completion. The full clock
regression set passes 40 tests; strict core library/test Clippy, formatting and
diff checks pass after additive integration onto the published Window checkpoint.
Independent source and integration review found no blocking issue.

The existing chrono-tz 0.10.4 catalog supplies 597 IANA 2025b IDs. Foundation also
accepts GMT-offset identifiers that this catalog does not. The calculation does
not import the excluded response parser's 12-proposal limit or silently enforce
the current Rust Planner setter's 24-location limit; those are future input/apply
integration boundaries. This is not full copilot or native Foundation parity.
