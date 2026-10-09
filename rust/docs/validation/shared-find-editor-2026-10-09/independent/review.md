# Shared EditorFind independent review

Reviewed WIP source in `/workspace/shared/box-find-presentation` against BelloBox `15197a71d81c620bf60da18f0d066505e4c0dc99` on 2026-10-09. This is source review pending owner freeze and focused test evidence, not native GUI acceptance. No product edits or Cargo invocation by reviewer.

## Findings sent to owner

1. Per-line exact-text comparisons and scans over all decorations multiply work by visible-row count. With explicit maxima of 8 MiB and 16,384 spans, prepaint/paint should avoid repeated document-wide work. Owner has accepted sorted-span partitioning and one exact render/request validation, retaining exact final receipt checks. Pointer/length/revision checks may optimize same-paint consistency but cannot be replacement identity (allocator ABA is possible).
2. A failed `ShapedLine::paint` is logged but the original code still inserts a measured fragment and emits a completed-paint receipt. Receipt eligibility must depend on successful text paint. Fix requested.
3. Additional important coverage requested: wrapped multi-fragment/CRLF target; partial ancestor clipping and honest fully_visible; outer translation/resize invalidation before deferred callback; replacement with same revision before receipt; newest reveal cancellation; success after cancellation/rearm. Owner already independently found and is fixing older measurement retained by a newer reveal.

## Inspected safety properties

Validation checks original UTF-8 boundaries and sorted nonoverlapping spans, exact text, and limits before replacing state. Emphasized overlap is intentional. Presentation operations do not take focus, edit text/undo, alter selection or Vim search; explicit select_all is a separate selection command and rejects composition/dragging. set_text and edit-state transfers clear transient state. Callback uses weak entity, window identity, request serial, editor layout generation, inner scroll offset and current exact text; host lifetime/navigation/outer-layout generation remains a required caller check.

GPUI 0.2.2 Window::defer schedules the end of the effect cycle. Window::draw completes the scene and marks needs_present; this is not proof of platform presentation or native visibility. Receipt documentation must describe paint/scene geometry only. There is no public platform frame generation invented by this implementation.

## Evidence limits

Owner holds Cargo lane. Initial tests and evolving fixtures are owner-run; final hashes, fresh test results and disposition of findings remain pending. No screenshots, speedups, actual native GUI, release, or feature parity claim.

## Final disposition — 2026-10-09 05:03 UTC

**No remaining blocking finding in reviewed shared-editor scope.** Owner implemented sorted span partitioning, same-paint validation hints with exact render/request/final-receipt authority, suppression of geometry after text paint failure, and a fresh paint-mask intersection. Newest reveal now cancels earlier measurement. Pointer comments explicitly limit their role; callers still own identical-text document/lifetime identity.

Frozen manifest SHA-256: `57d8d265b32d1b4e79680ec55ecf945242ec1dea3d6e9089ea4af77be238509b`. All five listed postimage hashes and byte lengths were independently verified. Final editor_view.rs SHA-256: `384588661e1e70c66730a7e0318b1a574116d789d6f153a6e8b5139d5621da11`.

Inspected final owner logs: presentation-test-r9.log (37 passed), clippy-final.log (strict all-target checks passed), fmt-final.log (empty successful output), ordinary-lib-build.log (passed). Clippy/build retain a dependency future-incompatibility notice for proc-macro-error2; do not summarize as entirely warning-free.

Independently executed the exact owner-built test binary named in r9 log, with two test threads: **37 passed, 0 failed/ignored, exit 0**. `independent-lib-tests.log` and `test-binary.sha256` record this rerun. This is independent test execution of the owner build, not an independent rebuild; no Cargo/build lane was used.

Additional final tests inspected cover partial clipping, same-length revision-zero public engine replacement after paint, explicit geometry invalidation after paint before deferred delivery, wrapped/CRLF multiple fragments, and cached unchanged-child control versus set/clear causing actual rerender generations. Older failing fixture logs remain in owner's evidence directory; final pass does not erase those attempts.

Remaining integration duties: host must install exact preview text/ranges, invalidate on outer geometry or lifetime changes, reject stale host-generation receipts, bound retries when no visible fragment exists, and distinguish honest row-level destinations from exact landing. This shared infrastructure plus TestPlatform tests alone is not a usable complete Find workflow, actual GUI validation, native macOS acceptance, or a performance benchmark.
