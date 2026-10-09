# HTTP full-window implementation handoff

## Result

The App implementation and Linux acceptance matrix are complete on the frozen source. Parent-owned integration/publication and exact native CI remain outstanding. No product source was changed after the ordinary GUI build. No screenshot files were exported or uploaded.

Owned implementation files:
- `rust/crates/bellobox-app/src/http_session.rs`
- `rust/crates/bellobox-app/src/http_session/tests.rs`
- `rust/crates/bellobox-app/src/http_ui.rs`
- `rust/crates/bellobox-app/src/http_ui/tests.rs`
- `rust/crates/bellobox-app/src/transport/http_request.rs`
- `rust/crates/bellobox-app/src/transport/http_request/tests.rs`

These provide an independent Home/palette-opened HTTP window, inert cURL/URL import, editable review, standard/custom method choices, explicit Send/Cancel, all-status inspection, complete bounded Copy, and close/quit retirement. The existing raw CLI and shared editor are unchanged. Imports, editing, navigation, and opening windows do not send. There is no draft/response persistence or content logging.

Dedicated transport disables redirects, ambient proxies, automatic retries, referer, and cookie history; uses no provider credentials or shell. It bounds connect/read waits to 30 seconds and the async exchange to 60 seconds. Physical admission remains held until the owned runtime is disposed. The response retains at most 2,000,000 observed bytes; truncation requires an actual extra byte, not Content-Length alone. Invalid-UTF-8 response header bytes are explicitly tagged and escaped, preserving status/body inspection. Core's header bounds are checked before raw copying and after display expansion.

## Checks

All final commands are source-bound in `app-commands.json` with original manifest hashes; final commands share `compiled-source-manifest.json`.
- Focused HTTP App: 26 passed.
- Default Core: 527 unit plus 46 integration tests passed; 5 native tests ignored.
- Default App: 578 passed; 2 ignored.
- Recording-fixture App: 594 passed; 2 ignored.
- No-default-feature App: 500 passed; 2 ignored.
- No-default-feature Core: 376 unit plus 3 integration tests passed; 2 native tests ignored.
- Strict Clippy: default App, recording Core/App, and no-default Core/App passed.
- Full workspace rustfmt check and ordinary no-fixture App build passed.

Synthetic transport coverage includes exact PATCH/header/Unicode JSON, duplicate headers, all 3xx/4xx/5xx response semantics, redirect trap, cookie isolation, ambient proxy trap, cap-minus/exact/plus, split multibyte boundary, HEAD, empty/204/304, binary/invalid UTF-8, chunked bodies, deceptive Content-Length, disconnect, no retry, header limits/obs-text, live header/body cancellation, read timeout, and continuous-trickle absolute deadline. Accepted test sockets explicitly restore blocking mode for native portability.

Session/UI coverage includes repeated Send suppression, cancel/edit/retry/stale results, independent windows, retirement with retained entities, same-byte marked/unmark mutation, stale Copy/Cut suppression before observers, exact shortcut modifiers, method-menu shortcut ownership and focus restoration, and complete 2 MB Copy beyond the visible preview prefix. Publication-held session fixtures pause after network completion; they are not described as live-socket tests.

## Actual cloud GUI

`gui-receipt.json` records three ordinary-binary process sessions, all exit 0, in light/dark appearances with software rendering. The sealed binary SHA-256 is `e3ff82259f3652a8c590db13835028017e498173c16a82e8bd80e9c67268ea92`; source-manifest SHA-256 is `c2d912d23e8da8186f89e7a35a6f06d48cffc57c8df9ebc1b9a6800950425c64`.

Observed acceptance:
- Initial imported request, rejected modifier shortcuts, and an open method menu caused zero fixture connections.
- Explicit Send produced the exact synthetic PATCH/header/39-byte UTF-8 JSON; the large integer and Unicode survived response rendering.
- Native Copy Response pasted the beginning/status and end/Unicode body into a new independent window; exact full byte equality is separately covered by GPUI tests.
- Repeated Send during a live held body admitted one request. Cancel, request editing, second-window close, last-window Ctrl-W, and app-controlled Ctrl-Q retired five actual held sockets without publishing partial/stale output.
- Explicit retry displayed 404 body; 307 Location/body was inspectable and the redirect trap received zero requests.
- New Window, Search, and closing another window preserved the existing response.
- Restart had blank request fields, GET, no response, and no fixture connections; isolated settings contained no fixture URL/body/response content.
- Actual 740×560 resize retained reachable fixed footer controls, visible method choices, keyboard choice/focus restoration, and scrolling access to the disclaimer.
- Home Developer HTTP card opened an independent blank window. HTTP Search opened the task-owned palette; its Open action created a second independent blank window. The entire route probe made zero requests.

Source files and binary were rechecked unchanged after GUI (`source-unchanged-final.json`, 245 .rs/root Cargo files). The original two-run receipt and source/binary seal remain unchanged; the final three-run receipt is separate. Documentation/evidence additions use a separate supplement.

## Failures, distinctions, and remaining gates

The initial measured test command failed at linking because only the Rust environment was sourced, without the existing GPUI sysroot overlay. The original log is retained; sourcing both existing environment scripts yielded the successful retry. One initial evidence-harness path error was corrected before Cargo and retained. One native text-injection provider call failed; per-character native key input recovered without resetting CUA. Immediate captures sometimes preceded a later rendered state; no latency is inferred. Native Ctrl-Home did not produce observed movement and is not accepted; Ctrl-A then Left exposed the response beginning.

Reqwest/Hyper imposes an upstream HTTP/1 limit of 100 response fields; the 101-field rejection is characterized. Core's 256-field bound is an additional ceiling, not a transport promise. System DNS/runtime disposal may outlast the logical 60-second async budget; loopback tests do not establish DNS cancellation or a hard physical-retirement deadline.

This does not establish native macOS/AppKit, OS-native IME, hardware GPU, accessibility completeness, or performance parity. GPUI marked-span tests are distinct from OS-native IME acceptance. Compact editable palette preview/session transfer is explicitly deferred. Later import edits require Import before Send; direct review edits cancel stale work. The GPUI method panel is not an AppKit native menu.

Parent must verify publication and native CI at the exact integrated commit. The documentation owner's source-verified provisional ledger reports +1,891 production and +1,966 support nonblank Rust lines, benchmark delta 0, against published 1943f5d; root owns final accounting and shared-path deduplication.

All reported command and GUI durations are observed intervals only. Editing effort and inference time are unavailable. Cargo and desktop are released, with no task windows left open.

Publication note: evidence filename references above were adapted to this package.
Original handoff SHA-256: `3809794f994c853a238cec8d721f591b41106ada902c6529b7589e772812e2e4`.
