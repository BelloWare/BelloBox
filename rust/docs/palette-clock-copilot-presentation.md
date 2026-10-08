# Compact Clock Copilot presentation repair

Base: `9ce80e005e9d8e896cb31ea9dc98aafc30000179`.
Specification: `BelloBox/UI/WorldClockCopilotView.swift`, compact inputRow and
suggestionRow. The first interactive palette validation found that the blank
18-point draft and unconditional small controls were hard to discover.

The existing 37-point input reservation now contains one bordered 30-point row:
sparkle, a 12-point-font / 20-point-height draft with an empty-field hint,
Send or Cancel, and explicit AI Settings. Horizontal padding is 8 points and
vertical padding 4 points. The 98-point transcript has 6-point spacing; Retry
and Clear appear only when relevant. Total preview reservations remain 261 and
365 points. Settings remains reachable even when configuration is unavailable;
the hint explains that a provider must be connected. Keeping Settings on the
input row is a deliberate compact adaptation rather than adding an unreserved
provider-notice row.

The placeholder is a local overlay. It forwards pointer focus to the existing
editor and disappears for nonempty or marked text. No shared editor behavior,
IME ownership, selection, Enter/Escape handling, worker, close, handoff, provider
transport, request validation, or authority is changed. The existing bounded
configuration-readiness cache controls hint wording only; actual Send reloads
and validates configuration independently. Opening, painting and editing never
send network requests.

Location deferral is recomputed from the current planner's location-only plan.
A suggestion mentioning a location already present no longer falsely offers
remaining work. Remaining applicable/deferred work wins over applied history;
labels distinguish Time applied, Locations applied, Partly applied, Applied,
and Already in effect. Intervening planner changes can make a previously applied
part actionable again. Invalid remaining plans keep their visible error.

Synthetic GPUI geometry and pointer-focus tests cover 320, 480 and 680-point
widths, both preview heights, and the actual rendered input/transcript bounds.
These tests are not interactive GUI evidence or macOS/TCC/AX/native acceptance.
All production capture/tool/vault/native gates remain unchanged.

## Remaining presentation differences

The input and Send remain usable without a configured provider; explicit Send
returns the existing validation error and preserves the draft. Swift disables
those controls. The transcript still uses plain role-labelled rows rather than
Swift's bubble styling and automatic scroll-to-status behavior. This repair
improves discoverability and suggestion-status correctness; it does not claim
complete Swift visual or accessibility parity.

## Typed-text clipping found during interactive review

The first repaired binary's empty hint was visible, but actual typed text was
clipped at its bottom. Its editor inherited 4-point top/bottom padding from the
shared plain appearance, making the 20-point line require 28 points inside a
20-point field. Outer-container geometry tests alone did not detect this.
The compact host now supplies zero internal editor padding; the enclosing card
still owns its source-shaped 8/4-point padding. A regression types ascenders and
descenders into the real EditorView and measures its complete content height
against the field at all three tested widths. Shared editor defaults are unchanged.
The original failed screenshot is retained as evidence; interactive recheck on the rebuilt binary confirmed the full text "plan tokyo
gjpqy" and caret visible, with placeholder-click focus and no request on typing.
That check used the cloud Linux desktop; it does not establish native macOS acceptance.
