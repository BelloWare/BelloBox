# Dedicated World Clock Copilot

This slice connects the dedicated Rust World Clock to the explicitly configured
HTTP provider. It follows `WorldClockCopilot.swift`, `WorldClockAIResolver.swift`
and `WorldClockCopilotView.swift`. The palette's conversation preview and handoff
remain separate work; opening a dedicated planner with an existing time/zone
handoff clears the dedicated conversation, while plain repeat-open preserves it.

## Workflow and ownership

Opening Copilot and editing its question send nothing. Send or an explicitly
chosen suggested question sends the current planner context, at most six bounded
history turns and the question. Ordinary Send clears the draft only after physical
worker admission; suggested questions preserve unrelated unsent draft. Retry uses
fresh context and configuration without duplicating the user message. Clear
forgets conversation, proposals and retry intent while preserving the draft.
Hide and Cancel invalidate presentation and signal transport cancellation.

All three existing HTTP provider shapes are supported through the same endpoint,
header and credential validation as Ask AI. The World Clock source system prompt
replaces the writing-assistant prompt. The raw bounded user prompt replaces only
the corresponding provider user field, avoiding the writing selection wrapper.
Saved provider settings and explicit process overrides follow existing precedence.
Keys are runtime-only; Codex app-server, persistent keys and native vault are not
added. There is no request on open, render, edit, Copy or Apply.

One process-wide physical lane stays occupied until HTTP returns, even if the UI
has cancelled or has been forcibly removed. A worker-owned QuitBlocker refuses
app-controlled Quit during that physical operation. Clock's local close guard
cancels and refuses ordinary Close while physical work remains, regardless of
other windows. It requires a fresh Close after retirement and then applies the
existing app shutdown gate. Forced removal is tested separately and fences stale
publication; native Dock/forced termination remains best effort. No shared media
shutdown policy is changed.

## Proposal review and application

Replies are bounded plain text plus optional parsed proposals. Copy copies only
the answer. Apply stages a clone, validates its full resulting zones/reference/time
and adopts it only on explicit activation. The reviewed action includes immutable
message ID and planner revision. Every production planner mutation advances that
revision, including successful handoff and live minute refresh. Within the same
minute `refresh_now` does not change the stored instant. Suggestions never mutate
after insertion, and dedicated-window capabilities are fixed. An old action after
any such change refuses and requires a newly reviewed Apply. The final prepared
plan independently compares the full planner snapshot before committing.

Location overflow never silently truncates the current list: a valid time part
may be applied separately with a visible location warning. Only zone/reference
preferences are persisted; selected time and transcripts remain ephemeral. A save
failure reports that the change exists in the current window but was not saved.
Per-message applied parts survive subsequent proposals and never suppress a newly
applicable change after the user edits the planner.

## Explicit bounds and remaining scope

- Question: 2,000 grapheme clusters and 8,192 UTF-8 bytes.
- Conversation: 40 in-memory messages; prompt history: last six turns, each 600
  Unicode scalars plus ellipsis, newline-flattened.
- Raw planner prompt: 64,000 bytes; final streamed Clock text: 65,536 bytes.
  Existing transport body/time/connect limits remain additional independent caps.
- Displayed answer: 4,000 Unicode scalars plus ellipsis; at most 12 proposed zones;
  existing planner maximum: 24 zones. IANA catalog support does not imply
  Foundation's extra GMT-offset aliases.
- Transcript and entire Copilot panel have independent scroll bounds so long
  answers/warnings do not force action controls outside the minimum window.
- No paid provider, real credential, Mac/TCC/AX/Keychain, signing, performance or
  release acceptance follows from the synthetic tests. Actual interactive GUI
  evidence, if captured, must be listed separately from GPUI test-host coverage.
