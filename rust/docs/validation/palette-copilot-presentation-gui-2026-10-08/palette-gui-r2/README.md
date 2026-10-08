# Compact Copilot presentation: first interactive review

Actual cloud Linux desktop replay, 2026-10-08 21:43–21:48 UTC. Ordinary application binary SHA256 `7808439001d08e72830afca3a5c48dd5a91eafd691ab8400bded9a01e670c23f`, verified before and after. Source checkpoint supplied by integrator was 9ce80e005e9d8e896cb31ea9dc98aafc30000179 plus presentation patch; validator independently confirmed copilot.rs SHA256 `7a7d90624ad1564b555b63e885dfff7fd89b8e1a06d20f746f1b5c00b8097df9` before review. No source edits/builds/remote writes by this validator.

## Observations

- Empty compact input now has a visible bordered card and example placeholder. Actual pointer click focuses the draft; `plan tokyo` entered with key events. No provider request on opening/searching/typing.
- **Failure:** typed text was vertically clipped. Original02 is the actual failure frame, preserved unchanged. This build is not accepted as the final visual fix.
- Explicit Send generated one numeric-loopback request. Empty palette was 680×447; with conversation 680×551. Desktop screenshots are 1364×1024. Light appearance; no scale override was set. These are observed window dimensions, not the fixed261/365 synthetic test acceptance sizes.
- Scrolling the bounded transcript exposes Apply time and `Open World Clock to apply: Add Tokyo`. Explicit Apply time changes planning to 2030-01-02 03:00 UTC. Original07 shows `Time applied` with the deferred Tokyo label.
- A second explicit request returns the same suggestion. Since time already matches but Tokyo is absent, only deferred Tokyo is shown, with no redundant time Apply (original08).
- Escape restores search focus; Return opens full World Clock with the conversation and planning instant (original09). Actual transcript scrolling reaches remaining Add Tokyo Apply. Click applies Tokyo; persisted settings contain Tokyo. Original10 was captured immediately after click and precedes the settled visual repaint, so the filename alone is not proof of its completed state. Reopened palette visibly showed all three zones; config/settings.json independently records persistence.
- Third explicit request in reopened palette offers only time Apply because Tokyo exists. After actual time Apply, a fourth identical request shows `Already in effect.` with no Apply button (original11).
- Fifth explicit request is physically held by the synthetic server. Visible button switches to Cancel (original12). Clicking Cancel and releasing the fixture returns to Send with no assistant response for the held question (original13; transcript scroll may hide the cancellation status). Clean Ctrl+Q exits0. No app remains.

## Input caveats and original frames

Initial search typing raced window creation: the first character of `clock` was absent, leaving `lock`, which still matched World Clock. Later reopened search correctly showed `clock`. Some click/scroll actions needed an explicit pointer move and a fresh retry before state changed; the initial unchanged frames are retained. Original03 captures resize/Thinking before settled repaint; original04 is the settled state. Original06 is the first unchanged Apply click; original07 is the successful settled result. These frames are not reconstructed or altered.

Fixture is copied from the prior replay into its own directory/config. Requests use only127.0.0.1 and a runtime-only fake key; request logs omit headers. Five requests exactly. launch.sh/server.py preserve setup, app.log/server.log preserve output, exit-code is0. Binary itself is held separately in the build evidence directory. No native macOS/TCC/AX/Keychain/signing/performance or production-gate acceptance is claimed. No exhaustive input-method or accessibility claim. New padding repair is checked separately in sibling palette-gui-r3.
