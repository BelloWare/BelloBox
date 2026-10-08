# Explicit HTTP provider setup

Base: `7a3ecdcf63d250519a1f99152c8f537dbc95d4ba`.

## Source contract and scope

The Swift specification is `UI/ProviderConfigView.swift` (`loadModels`, `runTest`,
transient invalidation) and `AI/AIClient.swift` lines 160–209 (models request and
sorted unique IDs). Rust now exposes the explicit Settings Load and Test actions.
Opening Settings, editing fields, changing providers, choosing a model and saving
ordinary model metadata do not send a request. Codex remains unavailable. No key
entry, key persistence, native vault, generation-option UI or native gate changes
are included.

Load issues GET at the configured `/models` route. OpenAI Chat/Responses use an
optional Bearer key; Anthropic uses its existing runtime key and version header.
The list is sorted and deduplicated, with a 1 MiB response bound, 4,096 input-entry
bound and 256-byte single-line model IDs. Empty, malformed, oversized and failed
lists retain manual entry and source presets. The discovered menu has a bounded
scrolling viewport. Invalid provider responses are never copied into error text.

Test sends the exact source hello instruction using the visible provider,
endpoint, request API, model and system prompt. It shares the existing streaming
request defaults and transport and displays a short response preview. Choosing a
model persists only ordinary provider metadata, which Ask AI reloads on send.
Explicit `BELLOBOX_AI_PROVIDER`, `BELLOBOX_AI_ENDPOINT` and `BELLOBOX_AI_MODEL`
process overrides retain their existing precedence for Ask AI. Settings explains
this distinction; a successful visible-fields Test does not certify an overridden
Ask AI configuration.

## Lifecycle and transport boundaries

Provider, endpoint and API changes clear discovered models and invalidate all
pending setup results. Model/prompt changes retain the discovered list but
invalidate pending loads/tests and prior success. Cancel and actual window destruction
signal cancellation even if its view entity is retained; a reopened view cannot adopt an old result. Setup actions
share one physical transport admission lease. Invalidating or dropping the view
cannot release it while an old blocking request remains active. Repeat clicks and
retry are rejected while that worker drains. The lease retires after transport
returns, before publishing its result.

Cancellation is cooperative: a blocked read is bounded by its transport timeout,
not immediately interrupted. Load has a 30-second total timeout and a 5-second
connect timeout. Test inherits the ordinary stream's 120-second total / 10-second
connect timeout and 8 MB response bound. Both disable redirects and proxy use;
provider credentials cannot follow a redirect or an environment proxy. Test
retains only a bounded preview and still drains/validates stream completion.

## Evidence and remaining limits

All new network fixtures bind numeric `127.0.0.1`, use fixed synthetic keys,
never consult process credentials and never send an external provider request.
The GPUI test host uses an isolated temporary settings path. Its complete flow
opens Settings, confirms no request, edits a model and confirms no request, loads
and deduplicates models, selects/persists the model, tests it, then calls the actual
Ask AI settings resolver/request builder/streaming transport and verifies the same
model on the wire and the streamed answer. This is synthetic GPUI host evidence,
not manual pointer-driven acceptance or a macOS runtime claim.

Additional tests cover provider-specific headers/routes and exact hello payloads,
empty/error/manual fallback, body and ID limits, 302 rejection, no echoed private
error body, cancellation before and after dispatch, a finite stalled-read timeout,
configuration override precedence, delayed results after actual endpoint/provider/API/model
callbacks, retained-entity window destruction, and actual Settings close/reopen
while the old transport holds its physical lease. Existing capture, recording, native and vault
gates are unchanged. Actual Settings pointer ergonomics remain unaccepted in the
current cloud input environment.

## Final frozen-source validation

- Core library: 394 passed (including four new pure provider-setup tests).
- Full recording-fixture App suite: 378 passed, zero failed; one child-only test is
  marked ignored in the parent harness and is explicitly executed by the isolated
  ambient-proxy parent test. The parent asserts the child's successful execution.
- The exact App binary was rerun with provider/key overrides unset and an empty
  temporary configuration root: 378 passed, zero failed, same child-only marker.
- Strict recording-fixture App/test Clippy, strict default production App Clippy,
  strict core/test Clippy, formatting and diff checks passed.
- Independent r3 source review found no blocking issue and verified all seven
  implementation/test postimages against the frozen manifest.

The copied test binary SHA256 was
`7cc6e08d9020a349cfbeabfeef9993efb42ce251c26c1b6a5d30ba5691d555a8`.
This is a test harness, not an interactive application binary or GUI screenshot
receipt. The binary is not committed. `receipt.json` binds source and log hashes.

The earlier strict-lint rejection of two fixture read counts is preserved as
`earlier-clippy-failure.log`; those reads now assert a positive amount. The final
strict logs and final full-suite results supersede that development-stage failure.
An intermediate edit typo was repaired before the frozen r3; no failing source
was published. No live provider, credential, paid service, native capture,
permission, release or manual pointer acceptance was used for these checks.
