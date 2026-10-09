# Whole-window activation observation: static review

Reviewed 2026-10-09 UTC against the uncommitted two-file delta on documentation HEAD 218470c8 (source a78203800a1c5d462814991da4cf65e60141a672).

Verdict: prior editor/search-origin held-key blocker is addressed by this source delta. No remaining blocking static finding within the palette modal-handoff scope. This supersedes the blocker verdict only for the exact hashes below; the earlier failed review remains preserved in final-clipboard-keyrelease-review.md.

- launcher_qr_ui.rs SHA-256: eabc812c8b04100fda10296e3390bef306f288275e442c7abc168a7826943e91
- launcher_ui.rs SHA-256: 6b24c8df37d9faaa6db01e2abc17c8bdafde57f0c7ad4ddf0c152967316f9d18

Root capture now observes physical Enter/Space before query or QR composition checks can return. Observation does not consume editor/IME events or arm Save. The window-owned shared two-bit tracker also covers keys pressed before QR entity creation and persists through row/session replacement. Key-up clears physical state before focus/row/IME routing; owned keyboard Save can therefore activate only after its physical release, and another held key still prevents opening.

Pointer/programmatic Save checks both physical and owned activation state, rejects while held, and does not replay afterward. Save ignores synthetic keyboard ClickEvent fallback, preventing an editor-origin release after pointer focus from becoming a new Save. Existing owned-activation overlap and cancellation checks remain intact. No unrelated popup, native gate, dependency or Clock implementation change appears in the two-file delta.

Three dispatched regressions target the actual prior gap: editor newline followed by settled preview and mouse refusal/release/fresh-mouse recovery; search Space before QR creation; query/editor composition preserving marked text and focus while physical tracking still blocks pointer Save. These are stronger than only setting a private flag or invoking the Save helper. git diff --check passed.

No Cargo or GUI run was performed by this reviewer; test compilation/execution and final sealed-binary acceptance remain required in their assigned lanes. Missing key-up after an external focus transition is handled conservatively by retaining the held state; this review does not claim global tracking of keys first pressed outside this window, native IME delivery guarantees, or full-popup Linux clipboard support.
