# Interactive generation-preferences validation

2026-10-08 22:18–22:24 UTC. Actual ordinary native GPUI application on the assistant's cloud Linux/X11 desktop, driven only through CUA. No Mac, credentials, paid providers, image OCR, Codex, vault, native permission or release acceptance.

## Provenance

Executed sealed ordinary binary `/workspace/shared/box-generation-evidence/bellobox-generation-r1`, SHA-256 `e8962356f1c752d93d4234a73bdc2b9782eb4b18f4aebc56ec3162069987b203`. Parent supplied integrated source `/workspace/shared/box-generation-integrated`, base 3bb plus reviewed r1; source manifest SHA-256 `54fd3706d632b760dc99cc8168e73b74d17df08826a0538065c179bd7589f673`. This worker independently checked the binary hash, copied the source manifest, and made no application source/build/remote writes.

Launch used isolated config, numeric loopback 127.0.0.1:33909, fake runtime key, ordinary lavapipe setup. Provider/endpoint/model environment overrides explicitly unset. Initial Settings category environment chooses category only; actual Home click opens window. server.py seeded test config before launch, never modified it afterward. All subsequent preference edits came from actual UI actions. launch.sh captures exit code and stops server.

## Observed workflows

- Home → Settings → AI Provider. Chose Custom temperature, clicked + to 1.1, clicked High reasoning. Controls save automatically; no separate Save button exists. Saved configuration independently confirms custom_temperature true, temperature 1.1, reasoning_effort high. No network request existed before explicit Test.
- Explicit Test connection produced POST /v1/chat/completions with exact short-hello prompt, temperature 1.1 and reasoning_effort high. UI displayed Connected: Hello from GUI fixture (07).
- Explicit Load produced the only GET /v1/models. Chose gui-other-model, which displayed defaults; selected Low reasoning for its independent profile. Selected gui-selected-model again and observed restored 1.1/High (12). Two distinct entries captured in config-two-profiles.json.
- Edited endpoint suffix from /v1 to /v1x (with accidental trailing spaces from Tab); new endpoint displayed defaults. Removed suffix/spaces and observed restored 1.1/High (14). No request sent during editing or return.
- Closed Settings through window-manager close, leaving Home open, then reopened via Home Settings. New window showed persisted 1.1/High (15).
- Clicked Reset this model. Settled UI returned to Model default (17). Config entries decreased 2 → 1; remaining other-model Low entry was byte-for-byte equal to its previous entry, including timestamp. requests.jsonl still had exactly two requests after all model/endpoint/edit/reopen/reset activity.
- Switched to Anthropic-compatible. This populated official default endpoint but sent no request. Replaced endpoint with verified numeric loopback before any Test, entered synthetic gui-anthropic model, chose Custom then Token budget. UI hid temperature with explicit suppression explanation, showed thinking budget 4096 and effective output token limit 6144 (23).
- Explicit Anthropic Test produced POST /v1/messages with thinking.type enabled, budget_tokens 4096, max_tokens 6144 and no temperature, despite stored Custom temperature choice. UI displayed Connected: Hello from GUI fixture (25). Important limitation: server intentionally remained the small pre-existing Chat SSE fixture. Generic decoder accepted that response; this validates actual UI request dispatch/options and generic response display, not Anthropic-specific event decoding.
- Exactly three network requests total: Chat Test, explicit Load, Anthropic Test. Original requests.jsonl retained. No automatic request on open/edit/save/model switch/provider switch/reset/reopen.
- Fresh Ctrl+Q after requests retired closed both application windows and returned terminal; recorded process exit code 0. Desktop free, fixture server terminated by trap.

## Input and snapshot limitations

All PNG files are unmodified original CUA screenshots. Bound Settings screenshots are 1178×814; desktop exit screenshot 1364×1024. Some immediate screenshots capture the old rendered frame before next paint (02 after Custom, 05 after High, 16 after Reset); later screenshots plus saved config verify settled outcomes. These original transient frames are retained, not overwritten or manufactured.

First Home global click did not open Settings; bound-window click after screenshot worked. Reopening likewise needed repeated global/bound attempts; exact transport cause unknown. First High click used the position before Custom expanded layout, so did not select High; after scrolling and fresh observation, correct High click succeeded. No pass is attributed to unsuccessful attempts.

When editing endpoint, bound pressKey('colon') and pressKey('shift+semicolon') emitted semicolons; invalid endpoint banner appeared and no invalid value was persisted/requested (19). Global Sky press_key Shift_L+semicolon correctly entered colon and cleared error (20). Bound Tab inserted four spaces rather than leaving input; these were removed explicitly. One attempted model entry after endpoint error used an old layout coordinate and had no effect; after fresh observation, correct model input succeeded. These input failures are not established application defects.

No Responses API interaction, app-process restart persistence, two customized endpoint profiles, real provider capabilities, performance improvement, native macOS, TCC/AX/Keychain/signing, or release acceptance is claimed. Window reopen persistence and model-profile isolation were actually exercised. Existing automated tests cover wider combinations separately.

## Artifacts

26 original PNGs; launch.sh/server.py; requests.jsonl; endpoint; config snapshots; final config/settings.json; app/server logs; exit-code; copied source manifest and binary checksum. SHA256SUMS.json records all artifacts other than itself. Root owns any later publication.
