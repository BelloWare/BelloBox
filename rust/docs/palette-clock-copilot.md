# Palette Clock Copilot and conversation handoff

This Rust workflow extends the dedicated World Clock Copilot on source checkpoint
`c3ced91eeb7678305f770cfc144352a9643d4327`. Integration is based on
`2a5f95b35d27f33b19e4530e696a06da496fc9b8`, whose additional changes preserve the
preceding dedicated-window interactive evidence. Those original screenshots and
observations are unchanged.

## User workflow

The World Clock palette row keeps its four-zone, preference-free planner. Its
Copilot draft sends only through explicit Send or Return inside that draft.
Opening, searching, changing rows, editing and handing off do not send requests.
Retry uses the saved unanswered question with fresh planner/provider context and
adds no duplicate user bubble. Cancel and Clear invalidate presentation while the
physical HTTP request retains its admission and Quit blocker. Clear keeps unsent
text. Merely switching rows preserves the in-memory conversation.

An answer can offer **Apply time** in the palette. Location proposals remain
visible as deferred work. Search-field Return opens the dedicated World Clock
with the exact planned instant or live intent, chosen reference, conversation,
per-message applied parts and unsent draft. The full planner keeps its complete
location list. An explicit subsequent Apply can install the remaining locations;
only that location change persists preferences. Handoff itself writes no settings.
A plain repeat-open preserves the existing dedicated conversation. An explicit
empty handoff replaces it with an empty conversation.

The secondary draft owns composition, arrows and ordinary text. Return submits
once; held Return cannot activate a newly focused control. Escape restores search
focus and held Escape cannot then close the palette. Tab/Shift-Tab reach the draft
and current Copilot controls. Ctrl/Cmd+K returns to empty search without sending
or erasing the draft. Apply focus is stable per message while live revisions
change; stale captured Apply payloads still require a fresh review.

## Bounds and lifetime

- Existing question limits remain 2,000 graphemes and 8,192 UTF-8 bytes. Context
  retains six bounded history turns; the conversation retains at most 40 messages.
  Parsed answers retain the existing 4,000-scalar bound and physical transport
  output is bounded to 64 KiB before parsing.
- Handoff drafts have an explicit **8,192-byte** transfer ceiling, checked before
  cloning the draft or cancelling/activating anything. Over-limit transfer shows
  an error and keeps the source draft and window intact. This bounded difference
  is intentional; there is no silent truncation. Empty or otherwise unsendable
  drafts within the byte limit can still transfer for editing.
- Core snapshots are opaque. Restore remaps messages onto destination-monotonic
  IDs and advances its generation. Old Complete, Copy and Apply callbacks cannot
  address the restored conversation. Active source work restores as cancelled,
  retryable state; transfer never starts an automatic retry.
- One shared worker module owns one physical Copilot admission lane for palette
  and dedicated windows. The physical worker owns the lease and app Quit blocker.
  Cloneable guards only observe retirement or request cancellation; dropping a
  guard does not release either owner.
- The launcher retains guards independently of preview entities, including after
  clipboard/selection replacement and non-clock row changes. All native and
  programmatic close/deactivation paths cancel and refuse while HTTP is active.
  After retirement a fresh close is required; no old close request is replayed.
- Target adoption stages planner and transcript before mutation and returns an
  error on failure. Source remains available on failure. Transferred guard is
  installed before target activation can deactivate the source; older target
  physical work also remains guarded. Repeated transfer deduplicates the same
  physical observer. Retained closed source entities cannot send or reopen work.
- The dedicated window's close-specific Stopping notice clears when physical work
  retires without erasing unrelated errors or Apply feedback. The earlier GUI
  evidence that exposed the stale notice remains preserved.

## Validation scope

See `validation/palette-clock-copilot-2026-10-08/` for the exact source manifest,
package-clean build record and validation results. The tests exercise actual GPUI
entities/action routes and numeric-loopback HTTP, including held transport across
native-close callbacks, deactivation, replacement, successful/failed handoff and
adoption into a physically busy destination. They are synthetic tests; they do
not establish interactive pointer/keyboard ergonomics or native macOS acceptance.

No production capture/tool/vault gates, credentials, media shutdown policy,
permissions, signing, release, or performance claims are changed. The same
runtime-only provider settings and explicit network consent behavior apply.
