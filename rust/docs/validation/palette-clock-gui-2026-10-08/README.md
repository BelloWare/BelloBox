# Palette Clock Copilot: actual cloud Linux GUI replay

Executed 2026-10-08 21:26–21:30 UTC against ordinary BelloBox binary matching published source checkpoint `9ce80e005e9d8e896cb31ea9dc98aafc30000179`.

Binary SHA-256: `4443356db62ab291b6ff4c2adf6e6fd96e4c701d2bf7e383e8d0dc4465f8e9af`, independently checked before and after replay. All 27 publication-manifest entries were checked against local integrated postimages/deletions; `source-verification.json` records them. No source changes or rebuild were performed by this validator. Parent independently verified these postimages against the published commit. This replay did not independently refetch the remote.

## Environment and fixture

Cloud Linux desktop, ordinary application, isolated settings directory, existing process-scoped lavapipe runtime. No synthetic GPUI test host. Direct Sky pointer movement and desktop clicks/key events drove the UI. The provider was numeric loopback `127.0.0.1:33525`, ephemeral local server, runtime-only synthetic key. Neither credentials nor public inference were used. Request records intentionally omit headers. `server.py` and `launch.sh` preserve the fixture; ordinary binary itself is not duplicated in this evidence package.

The first response proposes `2030-01-02T03:00:00Z` and adding `Asia/Tokyo`. The second response is physically held until an explicit marker removal. App and server logs are empty. App exit code is 0.

## Observed successful workflow

1. Home search opens the palette. Typing `clock` selects the World Clock preview.
2. A direct click on the draft followed by desktop key events visibly enters `plan tokyo`. No requests existed after open/search/edit.
3. Explicit Return in the draft sends exactly one POST. The reply offers time Apply and shows locations deferred to full World Clock.
4. Clicking Apply time changes the preview to planning at 2030-01-02 03:00 UTC (Los Angeles 2030-01-01 19:00). The settings file is byte-identical before/after this action.
5. Typed `unsent draft`, Escape restored search focus, then Return opened dedicated World Clock. The exact user and assistant text, planned instant, and unsent draft are visibly present. Request count remains one. Settings remain byte-identical through handoff.
6. Scrolling the transcript exposes `Add Tokyo` and its remaining Apply. An explicit click adds Tokyo, shows three locations and Applied. Only then does settings gain zone_ids [America/Los_Angeles, UTC, Asia/Tokyo] plus anchor_zone_id. The proposed instant is not persisted. No additional provider request occurs.
7. Dedicated close succeeds. Reopening palette Clock shows all three persisted locations and a fresh empty conversation.
8. Explicit `held question` Send starts the second POST and the server creates `admitted-held`. The palette shows Thinking. Escape then search Return transfers the unanswered question to the dedicated window in cancelled/retryable state, with Stopping while physical HTTP remains held. No third request occurs.
9. Actual titlebar close is refused. The dedicated window remains visible and says to close again after the request finishes.
10. At the timestamp in `released-at.txt`, the fixture marker is removed. Both physical and close-specific Stopping notices disappear; the unanswered notice remains. The late response does not become an assistant answer. There is no automatic close and no automatic retry.
11. A fresh titlebar close succeeds. Home remains. Ctrl+Q exits the ordinary app with code 0; desktop inventory confirms no BelloBox windows remain.

## Original screenshots

01 Home; 02 palette open; 03 Clock preview; 04 first Tab experiment (reference menu); 05 visible draft; 06 proposed answer; 07 time applied/deferred locations; 08 unsent draft before handoff; 09 transferred conversation/draft; 10 remaining location Apply after transcript scroll; 11 three locations/Applied; 12 Home after close; 13 reopened persisted locations; 14 held palette request; 15 held handoff; 16 actual close refusal; 17 retired request with no stale stopping notice; 18 Home after fresh close.

All screenshots are original 1364×1024 desktop captures, without cropping, compositing, reconstruction, or generated content. Individual hashes are in `evidence-sha256.json`.

## Unsuccessful inputs and limitations

Initial verification script treated a deletion as a readable file and stopped; it was corrected to assert absence for the manifest deletion. A launch command issued before this preparation completed found no launch script; the command was rerun after verification. Neither attempted launch created a provider request.

A first Tab from palette search focused the reference control rather than the question. Typing `plan tokyo` there did not populate the draft; the space opened its menu. Escape closed that menu, then an observed direct draft click and keyboard input worked. Screenshot04 preserves this experiment. The palette draft is an unlabelled, visually almost invisible 18px line and action controls are small; this is a discoverability/ergonomics concern, not a failed data-flow assertion. The transcript required scrolling to reveal the dedicated remaining Apply; this succeeded using actual pointer scroll.

This is one focused interactive cloud Linux replay. It does not establish native macOS/TCC/AX/Keychain/signing/release acceptance, production capture/tool/vault readiness, performance, all input-method behavior, accessibility, exhaustive keyboard navigation, overflow limits, or all held-request source/target permutations. Bounds are source/test evidence, not exhaustively entered through this replay: question 2,000 graphemes and 8,192 UTF-8 bytes; handoff draft 8,192 bytes; six bounded history turns; at most 40 messages; answer 4,000 scalars; transport output 64KiB. No bounds were changed. The tested happy path and held handoff are interactive evidence, not merely startup smoke.
