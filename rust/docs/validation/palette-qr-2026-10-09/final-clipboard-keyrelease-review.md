# Final clipboard/key-release delta: static review

Reviewed 2026-10-09 UTC, source a78203800a1c5d462814991da4cf65e60141a672, documentation HEAD 218470c8. Verified all eight source-manifest SHA-256 values against both the local files and Git object 3ae7349bd76d4d0c86b368c3fee21ab2ca531ace (tree 39639ee80be98a6299d525018c73d313ced54aff).

## Verdict: one remaining mixed-input blocker

A locally held editor-origin Enter/Space is not tracked before pointer Save. In launcher_qr_ui.rs handle_key, activation_keys_down is set only when consume_release already exists or a focused button handles activation. The editor_focused early return leaves the flags false. Search-origin Space follows the same untracked path. save() therefore cannot reject the still-held key if the user clicks Save before releasing it. The newly opened chooser may receive that repeat, the same consequential failure class that motivated deferred keyboard Save.

Concrete regression to add: start with an accepted nonblank draft, focus its editor, dispatch Enter key-down without key-up, allow the updated preview to settle, pointer-activate Save, and assert no chooser. Releasing the key must not replay the rejected mouse action; a fresh Save activation should work. Also cover Space/search focus and observation during composition without preventing the editor/IME's own key handling. The current mixed-input test starts by arming the Save button itself, so does not exercise this case.

This is a source-derived counterexample, not a newly executed GUI reproduction. No Cargo, GUI, product edit, screenshot or remote write was performed during this review.

## Accepted parts and test adequacy

- The clipboard guard is fail-closed and queries the actual GPUI compositor rather than environment variables. Verified pinned GPUI source: X11 publishes text and keeps a private item cache; Wayland advertises text/private-cache MIME; macOS routes Image entries to the image pasteboard writer. Empty compositor names enable ordinary production Copy only on macOS. The test-only empty-name fixture exception is explicit.
- Disabled Copy leaves clipboard untouched, is skipped in Tab traversal and displays Save guidance. Native clipboard success is still unproved; full-popup Linux image copying remains outside this palette fix.
- Deferred Save correctly waits for the owned activation release. Bounded Enter/Space overlap cancels the armed action until both releases. Tests dispatch both activation orders and release orders, plus a held repeat between releases and a fresh subsequent activation.
- Focus/row loss, edit ABA, retirement, Tab away/back, Escape and tracked-key mixed-input cancellation have focused tests. The native modifier corrections use Cmd on macOS and Ctrl otherwise. Existing failure evidence should stay preserved rather than being relabeled as a pass.
- Source changes to launcher_ui.rs in this delta are tests only. Existing Clock physical-retirement/close/quit implementation is unchanged by this delta.

## Hashes for changed source paths

- launcher_qr_ui.rs: 9a6ffc8265f5a45ed6c4ed1747fe5be16fb4ab7564a14c95c14d3c1a8c4150fe
- launcher_qr_ui/tests.rs: cce9d1b25342b40751c621aabc801e220312393820f7156d3df701a89b8f9c98
- launcher_ui.rs: 310c54714d8fb764feb40f76ecd05bfb069cce49395763de975ee3752d16e4b5
- docs/palette-qr.md: c31b3a21a8598dea818db244c4e5d15333789e4514e778f93bd5fcde7316ac99

The implementation owner's 49-test default/minimal/movie runs, strict checks and sealed d4702add actual-GUI results are separate reported evidence, not commands rerun by this reviewer. Two-key overlap remains synthetic-only. Native replacement CI is pending per the parent; this review does not change that status or claim whole-QR clipboard acceptance.
