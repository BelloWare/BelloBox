# Shared text presentation dependency slice

Source worktree: `/workspace/shared/box-find-presentation`, exact base BelloBox
15197a71d81c620bf60da18f0d066505e4c0dc99 (tree 1ca9979fab6bff4f20d3abee773b411104b029ad).
No commit/ref publication, Agent dependency pin, popup source, native permission,
provider/credential, release, screenshot or spending change was made by this worker.
This is shared backend/view infrastructure until the Agent Find consumer is wired
and validated end-to-end. It is not a complete Find workflow or native acceptance.

## Frozen source

`source-manifest.json` SHA-256:
57d8d265b32d1b4e79680ec55ecf945242ec1dea3d6e9089ea4af77be238509b

Five paths, complete postimages in the worktree, patch in `presentation.patch`:
- bello-workbench-ui/Cargo.toml: test-only GPUI test-support dev dependency; no new version/package or production feature expansion.
- src/editor_view.rs: transient presentation, nonfocusing scroll, one-shot measured paint receipt, separately explicit select_all command.
- src/text_presentation.rs: public value/error/receipt types and strict bounded validation.
- src/editor_presentation_tests.rs: GPUI TestPlatform scene/layout and editing invariants.
- src/lib.rs: public exports.

No Cargo.lock change. Root approved the narrow test dev-dependency and explicit
select_all helper as extensions to the original editor/module/export ownership.

## Public API and consumer contract

TextDecoration { range: Range<usize>, color: Hsla }
TextPresentation { token: u64, text: Arc<str>, decorations: Arc<[TextDecoration]>, emphasized: Option<TextDecoration> }
PresentationError: TextChanged, InvalidRange, LimitExceeded, Busy, StaleToken.
PresentationGeometry: token, host_generation, layout_generation, window_id,
first_visible_fragment (window coordinates, clipped), fully_visible.

EditorView methods:
- set_text_presentation(Option<TextPresentation>, cx) -> Result<(), PresentationError>
- reveal_presented_range(token, range, cx) -> Result<(), PresentationError>
- measure_presented_range(token, range, host_generation, FnOnce(PresentationGeometry, &mut Window, &mut App), cx) -> Result<(), PresentationError>
- cancel_presentation_reveal(token, cx)
- invalidate_presentation_geometry(cx)
- select_all(cx) -> Result<(), PresentationError> (explicit editing selection command; not part of decoration semantics)

Call reveal BEFORE arming measurement: a newer reveal cancels older measurement.
Install/clear/draw/reveal never change text, undo, selection, marked text, Vim search
or keyboard focus, and never emit Changed. Selection is intentionally changed only
by the separately named explicit select_all command, which refuses IME/dragging.
Composition/dragging returns Busy for automatic reveal/measurement. Ordinary user
key/mouse/scroll navigation cancels pending automatic work. Landed scroll remains
independent of the old caret across redraws; actual key/edit navigation resumes
caret following. Clear does not jump back to a previous caret.

Ranges are nonempty original UTF-8 byte spans in the EXACT current text. Ordinary
spans must be sorted/nonoverlapping; emphasized may overlap them. Explicit limits
are 8 MiB bound text and 16,384 ordinary decorations. Errors are atomic; no silent
truncation. Hosts own query/document identity in token, including identical-text
replacement. set_text and edit-state transfers discard transient presentation.

Exact text equality is checked at installation/render and before delivering a
receipt. A pointer/length/revision hint guards individual lines between that
validation and paint; it is NOT document identity and allocator reuse is not an
identity guarantee. Sorted span binary search avoids scanning every match for
every line. No claimed measured performance speedup.

There is intentionally NO cached-geometry getter. Measurement is armed once and
only actual successful text paint records clipped source fragments. Prepaint alone,
fully offscreen/clipped targets and text-paint failures do not invent geometry.
Window::defer runs the receipt at the end of that paint effect cycle, outside the
editor borrow. Before receipt, exact bytes, request serial, layout generation,
window ID/bounds and inner scroll offset are rechecked. It describes GPUI scene
paint, not OS display presentation or native accessibility acceptance.

The consumer MUST check its CURRENT outer navigation/controller/chat/window/lifetime
generation inside the callback before acting. Receipt coordinates must not be kept
across later outer scrolling/layout. Call invalidate_presentation_geometry before
outer geometry changes and rearm after them. GPUI exposes no public generic window
frame/dirty epoch; this API does not invent one. A host that changes outer layout
without changing its own navigation generation violates this explicit contract.
No animation-frame retry loop is installed; offscreen targets can leave an armed
request without a receipt, so the host must bound retries/timeouts and give an
honest unavailable/row-level destination. Remounts must reinstall current tokens;
weak-entity delivery prevents using a dropped editor.

## Validation on the frozen source

- `cargo test --locked -p bello-workbench-ui --lib`: 37 passed, 0 failed/ignored, presentation-test-r9.log.
- `cargo clippy --locked -p bello-workbench-ui --all-targets -- -D warnings`: passed, clippy-final.log.
- `cargo fmt --all -- --check`: passed, fmt-final.log.
- `cargo build --locked -p bello-workbench-ui --lib`: ordinary production library build passed, ordinary-lib-build.log. This is not a clean whole-app binary/GUI seal. The later consumer must build its own final ordinary app package deliberately.
- Existing proc-macro-error2 future-compatibility warning remains; no new source warnings in final checks.

Tests preserve pending Undo grouping, Unicode/CRLF, selection/caret/search/focus,
IME/drag state, atomic invalid-range/exact-text/limit errors, set_text revision-reset
ABA, new request cancellation, public same-length revision-zero replacement after
paint, explicit postpaint/pre-defer invalidation, actual clipping/fully_visible,
wrapped multiline+CRLF source fragments, prepaint-only no receipt, offscreen no
fake geometry, inner and horizontal scroll independent of caret, real cached-child
set/clear rerender (with unchanged-parent negative control), and explicit select_all.
These are TestPlatform unit/scene checks. No actual interactive GUI, OS IME,
macOS AX/focus/scroll acceptance, signing or release claim.

## Development failure provenance

Raw logs are retained and hashed by log-manifest.json; final passes do not erase them.
- First 26-test compile passed before GPUI tests were added.
- r1 GPUI test compile failed from wildcard-imported gpui test macro recursively expanding #[test]; fixed explicit imports.
- r2:31 passed, unused AppContext import warning; removed unused import.
- r3:32/34 passed. New fixture incorrectly awaited legitimate automatic paint before asserting no receipt; UniformList's generic logical child index was not a valid uniform-row scroll oracle. Replaced with inside-cycle prepaint-only canvas and real negative Y offset/paint checks.
- r4:33/34 passed. Direct prepaint outside GPUI draw phase correctly tripped content_mask phase assertion; corrected fixture uses canvas prepaint in the real draw phase.
- r5:34 passed; r6:36 passed with clipping/wrapped cases.
- Clippy r1 rejected collapsible if and intentional one-range Vec fixture construction; source simplified and fixture used iter::once, no lint suppression.
- r7 test compile had a renamed fixture variable and missing AppContext import while adding cached host; corrected.
- r8/r9:37 passed; r9 strengthens cached-child proof beyond mere metadata invalidation.

Independent source review is preserved in independent/review.md. The integrator must
integrate that final finding, verify exact source/consumer tree and publish normally
only after the planned consumer wiring/gates. Shared code is counted once in Box.

Editorial note for publication: one internal review-routing identifier was omitted; technical claims and raw logs are unchanged.
