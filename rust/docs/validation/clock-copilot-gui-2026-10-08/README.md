# Actual Clock Copilot cloud GUI validation

2026-10-08, 20:55–20:59 UTC. Own cloud Linux/X11 desktop, Mesa lavapipe; actual ordinary GPUI App interaction through CUA. No user's Mac, real provider, real credential, source edit or remote write was used.

## Provenance

Source: f30b81f0fd62e7bb29b8809a4f57471b46b01450 plus reviewed clock-copilot-r2.patch in /workspace/shared/box-clock-integrated. Frozen source-manifest-r2.json SHA-256 27b87dcd0e288ffe68eda9116e8a545a954b15c1980ff8383f0d560ba7e541e3, copied from owner evidence. Owner supplied final ordinary binary after package-clean tests/build; copied bytes independently checked as c533b5097a5cdf91abb2ad765854fcf65634fca5af25eebe0d9bff9ce744702a. bellobox-clock is that executed binary. No rebuild by GUI validator.

launch.sh uses isolated BELLOBOX_CONFIG_DIR, runtime synthetic-clock-gui-key and numeric loopback http://127.0.0.1:37075/v1. No endpoint/model/provider process overrides. server.py returns an explicit synthetic Chat Completion SSE response proposing Asia/Tokyo. No request occurred on opening Home, World Clock or Copilot, or typing the first draft.

## Verified workflow

1. Home World Clock card opened dedicated World Clock (01–02). Ctrl+J opened Copilot with focused empty draft (03).
2. CUA keyboard input entered `add tokyo please` (04). Explicit Send produced exactly one POST /v1/chat/completions with clock-gui-model, stream=true and max_completion_tokens=4096. The exact raw request in requests.jsonl includes Clock system prompt, live UTC/current/reference/LA+UTC context and the user's question, without selected-text wrapper.
3. The answer visibly read `Tokyo is nine hours ahead of UTC.` (05). Scrolling the transcript revealed `Add Tokyo` and explicit Apply (06). Before Apply, config-before-apply.json had no stored zone list and the visible planner had two locations.
4. Explicit Apply changed visible location count to three and rendered `Applied` / `Applied: Add Tokyo` (07). config-after-apply.json persisted America/Los_Angeles, UTC and Asia/Tokyo with unchanged LA reference. Hiding panel via its button exposed Tokyo/JST UTC+09:00, next-day date (09). The live-time mode remained active; no suggested time mutation occurred.
5. Titlebar Close closed World Clock while Home remained. Opening the Home card again created a new Clock window with the same three saved locations (10). Ctrl+J revealed a fresh empty conversation (11). The request log still had one request before the second explicit Send.
6. Typed `check tokyo`, clicked Send while fixture was held before headers. admitted-held proves the second request physically reached the loopback server. UI showed Thinking, one user bubble, enabled Cancel (12). Its request context includes all three saved zones and no prior conversation history.
7. Explicit Cancel changed UI to `Stopping; waiting for the request to retire…` (13). Clicking titlebar Close while still held retained the Clock window. Saved screenshot 14 shows generic Stopping and `Cancelled. Nothing was changed.`; it does not show the close-specific notice. A subsequent screenshot emitted by that same CUA call, before the following call released the hold, showed both Stopping and `Stopping the Copilot request. Close again after it finishes.` That later live tool image was not saved as a PNG. The portable artifacts establish window retention after the Close attempt while held; the exact close-specific notice is archived only in the later post-retirement screenshot 15. Home remained alive.
8. Removing the fixture hold let HTTP retire. No assistant answer or proposal from the cancelled request appeared; Retry became enabled and busy text disappeared (15). The close-refusal notice remained stale (see limitation below). A fresh titlebar Close then closed Clock successfully, returning Home (16). Fresh Ctrl+Q from Home exited App, launch recorded exit-code 0. Total requests stayed exactly two.

## Input observations and limits

Screenshots are original unmodified bound-window PNGs, 1178x814. Window origin was desktop (93,117); button input used explicit Sky pointer move plus direct desktop click. Keyboard input used documented bound pressKey. Requested Shift+A/Shift+T produced lowercase text, which the actual screenshot/request preserve. A later bound Ctrl+J attempt after Apply did not hide the panel; the explicit Hide Copilot button did. Screenshot 08-three-locations.png only shows count three with panel still visible; screenshot 09 is the actual Tokyo-row evidence. These unsuccessful input attempts are not treated as application defect proof.

The post-retirement notice in screenshot 15 still says `Stopping the Copilot request. Close again after it finishes.` even though Retry is enabled and physical busy text is gone. This is a minor stale-notice wording issue reported to the owner; fresh Close succeeded, and cancelled output remained suppressed.

This validates the dedicated Clock window, Chat Completion loopback happy path, explicit location Apply/persistence across window close/reopen, cancelled held HTTP, window retention after Close while held, and fresh close after retirement. The exact held-state close-refusal notice was observed in a later live CUA screenshot but is not present in the saved held-state PNGs; do not cite screenshot 14 as that notice. It is not macOS GUI/TCC/AX/Keychain/signing/performance/release acceptance, real model quality, Anthropic/Responses GUI acceptance, process-restart persistence, app Quit while held, last-window Close, palette handoff, time-change Apply, stale-planner Apply, or Settings deep-link acceptance. Automated tests are separate evidence. Existing AI Settings button does not deep-link to AI Provider and was not tested here.

## Evidence chronology correction

Root image review identified the screenshot-14 attribution error. Original PNGs were preserved. Screenshot 14 was saved at 20:59:03 UTC before the subsequent live screenshot had reflected the close-specific notice. The fixture release occurred in the next tool call; released-at.txt records it. Screenshot 15 was saved at 20:59:47 UTC after retirement. Filenames describe attempted actions and must not be used as acceptance assertions. This report was corrected at 21:03 UTC to keep portable evidence distinct from the additional live tool observation.

## Evidence

01–16 PNG files, requests.jsonl, config-launch/before-apply/after-apply/final.json, source-manifest-r2.json, server.py, launch.sh, app.log, server.log, admitted-held, released-at.txt and exit-code. SHA-256 inventory is evidence-sha256.json. Binary is retained locally; root owns publication.
