# BelloBox Window synthetic Linux GUI QA

Date: 2026-10-07 UTC. Linux cloud desktop, CUA actual clicks/keys/drag; production debug binary SHA256 `a07a996f0102008636ff1729ca75b85ac20f18848c89b4170cd3e7f1b9b98b98`.

The initial evidence below applies to the exact initial binary above. A separate native ±1-size alpha-resampling mismatch was identified; these same-size fixture checks do not cover that case. No sampler correction was made in the final host binary. See final scoped recheck below; the native mismatch remains open.

Launch from cloud desktop terminal: `bash build-environment/box-gui/launch.sh refresh` or `frozen`, with full path rooted at migration workspace. Script uses isolated config and generated app-owned synthetic pixels only; no real capture, provider, permission, Mac, TCC, or Spaces access.

## Actual observed passes
1. Orange front hover outlined 680×540. Click opened Screenshot editor labeled `Window · synthetic alpha on frozen pixels · 680 × 540 px`. Orange colors retained (never purple), rounded corners visibly exposed editor background. Retained screenshots: `02-front-hover.jpg`, `03-front-alpha.jpg`.
2. Selected rectangle tool, dragged annotation, Ctrl+Z removed annotation leaving source/dimensions unchanged. Ctrl+Shift+Z restored rectangle. Source remained orange680×540. Retained `05-redo.jpg` shows the restored rectangle; the preceding Undo removal was directly observed in the same interaction sequence. One immediate render capture was temporarily blank; subsequent Undo/Redo captures fully rendered and runtime log stayed empty.
3. Close with annotation showed discard dialog. Keep Editing preserved annotation. Close→Discard closed editor and selector; no orphan windows. Retained screenshots: `06-close-confirmation.jpg`, `07-keep-editing.jpg`.
4. Relaunched refresh, hover exposed blue back reported840×460. Click opened `Window · synthetic independent pixels · 840 × 460 px`. Blue filled formerly orange-overlapped region; rounded corners visible. Retained `09-back-independent.jpg` shows the editor after the directly observed 840×460 hover and selection sequence.
5. Closed unchanged editor without discard dialog. Relaunched frozen, selected exposed blue: `Frozen Window · synthetic visible pixels · 840 × 460 px`, orange overlap retained. Retained `10-back-frozen-overlap.jpg` demonstrates difference from independent refresh.
6. Relaunched selector: click blank area produced no editor. Drag9px horizontally on orange produced no editor and explicit dragging-not-selection hint. Retained screenshot: `11-selector-rejection.jpg`.
7. Drag7×7px on orange opened680×540 front editor (L-infinity threshold behavior). Retained screenshot: `12-short-drag.jpg`. Native inventory briefly raced a disappearing selector with X11 GetProperty error; next inventory confirmed editor normally.
8. Repeated launch then Escape dismissed selector with no editor. Repeated launch then Alt+Tab focus loss dismissed selector with no editor. Inventories confirmed no orphan windows after each.

## Not claimed
No native macOS transparency/capture/privacy/Spaces/permission verification; no numerical alpha or RGB inference from JPEG screenshots; no independent real-screen capture; no ±1-size resampling coverage in this GUI fixture; no newer-navigation concurrency stress. Model/core/GPUI tests cover other edges separately. No source mutation during GUI QA.


## Final host-binary scoped recheck — 2026-10-07 04:43–04:45 UTC

Binary SHA256 before and after QA: `c52f3a7960c292dbaff9107de3c7bb35fe3a740b147fd1dbb93d960a57a1dbe6`.

Compiled source identity: base `780ecd94967651a207c18a58f5ae4e60e7c571b4`, plus modified build-input hashes in `source-inputs.json` (manifest SHA256 `2a2ecbf7743e0213b0618962eff2f3bc44b38552196dd44ea0b5e69993399664`). Source manifest and binary hashes were verified directly before QA; binary hash remained unchanged afterward. Later screenshot/report-only changes do not affect build inputs.

Actual recheck used the same isolated refresh/frozen launch script:
- Refresh front: orange 680×540 editor, alpha-on-frozen source label, visually rounded corners, no purple replacement. `final-front-alpha.jpg`.
- Refresh back: blue 840×460 editor, independent-pixels source label, orange overlap absent. `final-back-independent.jpg`.
- Frozen back: 840×460 frozen-visible-pixels source label, orange overlap retained. `final-back-frozen.jpg`.
- Closed each unedited editor without confirmation; selector/editor windows were gone after each close.
- Relaunched selector and clicked Cancel; inventory showed no remaining BelloBox selector/editor. Runtime log was empty.

All these scoped same-size host GUI checks passed. This final recheck did not repeat annotation/dirty-close/drag/focus cases; those remain explicitly tied to the initial binary above. An asynchronous selector→editor transition produced one stale-window GetWindowAttributes error; refreshed inventory showed normal editor and QA continued. Native ±1-size alpha resampling remains open. These JPEGs support observed UI behavior, not numerical pixel or macOS fidelity claims.
