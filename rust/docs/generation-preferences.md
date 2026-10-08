# Per-model generation preferences

Source specification: BelloBox main `e43b1c4595c42383e087fe70f38c28d07c31dda0`.
`BelloBox/AI/AIGenerationOptions.swift` defines optional wire fields, normalization,
identity and thinking allowances. `Settings/AppSettings.swift` resolves selected
model options and retains the 128 most recently modified profiles. Final request
builders were inspected: `AI/AIClient.swift` 240–301 and
`Screenshot/OCR/AIImageClient.swift` 124–194. `WorldClockAIResolver.swift` 256–264
clones current config and calls the same AI client.

Settings → AI Provider → Model behavior edits only ordinary local preferences.
The key includes provider family, normalized endpoint and trimmed model name;
Chat and Responses share the OpenAI family, matching Swift. Merely changing the
provider, endpoint, API format or model does not create a profile. Reset removes
only the current profile. New models use provider-default optional fields.
Runtime overrides resolve a profile for the effective provider/endpoint/model,
not the visible Settings route. An accepted request owns an immutable options
snapshot. Settings Test checks the visible route, as before.

Temperature steps by 0.1 (0–2 OpenAI,0–1 Anthropic); default omits the field.
Reasoning maps to `reasoning_effort`, `reasoning.effort`, or
`output_config.effort`. Anthropic excludes none/minimal efforts. Thinking modes
are omitted/default, disabled, adaptive, or enabled with a 1024–32768 budget.
Thinking suppresses temperature without erasing its saved value. Anthropic output
is 1024–65536, raised to at least 8192 for adaptive and budget+2048 for budgeted.

Intentional source-parity wire change: Chat/Responses omit output-limit fields,
including image requests. The previous Rust builders always sent 4096. Swift's
final dictionaries do not send those fields; only Anthropic sends max_tokens.
The existing bounded local transport body/stream/time limits remain in force.
Image approval descriptions now describe the actual output/temperature fields.

One shared option application is used by text, Clock Copilot and the existing
confirmed-image request builder. Image authority validation, immutable snapshots,
explicit confirmation, image privacy preparation and production enablement gates
remain in their existing paths. No ordinary OCR activation, Codex configuration,
key persistence, vault migration, permissions or native capture enablement is
part of this change. No legacy Swift defaults or Keychain are read or migrated.

UI uses wrapping labeled choice buttons and steppers rather than native SwiftUI
menus, slider or disclosure animation. It exposes all HTTP options and bounds;
Codex controls remain unavailable. Synthetic tests and CI do not establish actual
interactive UI usability, macOS accessibility, native acceptance or performance.
