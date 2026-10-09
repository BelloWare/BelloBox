# HTTP & cURL workflow

## Scope and source baseline

This is the ordinary independent HTTP & cURL window, reached through the existing
HTTP tool route. It adds local import/review, editable request fields, explicit
Send/Cancel, and bounded response inspection/Copy. It is an implemented slice,
not complete Swift workflow or native-platform parity. The legacy raw-request
CLI engine in `developer.rs::http` is unchanged. Compact palette editing and
compact-to-independent-window request/session transfer remain incomplete.

Reference: Swift main `e43b1c4595c42383e087fe70f38c28d07c31dda0`, tree
`5ca17248d54fe50c5617d597be7ee6794bde6ea8` (remote rechecked 2026-10-09 16:00 UTC).
Rust predecessor: `1943f5d2cddb02ca8fc637850d109c9af11ee2f1`, tree
`630ecf1c33479c2cb3d17e3db29e5684048500a4`. The immutable migration handoff at
`f1c21ae4f22b14dc10c5a57651499b05cc37ec92` remains the wider migration contract;
its historical verification is not transferred to this new checkpoint.

Source map:

- Swift [`HTTPRequestTool.swift`](../../BelloBox/DeveloperTools/HTTPRequestTool.swift):
  draft preparation, literal cURL tokenizer/import, ephemeral URLSession and response rendering.
- Swift [`UtilityWorkbenchModel.swift`](../../BelloBox/Launcher/UtilityWorkbenchModel.swift),
  [`UtilityWorkbenchView.swift`](../../BelloBox/Launcher/UtilityWorkbenchView.swift), and
  [`UtilityWorkbenchWindowController.swift`](../../BelloBox/Launcher/UtilityWorkbenchWindowController.swift):
  request fields, Send, cancellation, response state and shortcuts.
- Rust [`developer/http_request.rs`](../crates/bellobox-core/src/developer/http_request.rs):
  bounded pure import, private immutable prepared request and response rendering.
- Rust [`http_ui.rs`](../crates/bellobox-app/src/http_ui.rs) and
  [`http_session.rs`](../crates/bellobox-app/src/http_session.rs):
  independent GPUI window, memory-only session, edit/composition invalidation and publication fencing.
- Rust [`transport/http_request.rs`](../crates/bellobox-app/src/transport/http_request.rs)
  and [`shutdown.rs`](../crates/bellobox-app/src/shutdown.rs):
  fresh transport/runtime and worker-owned physical retirement admission.

## Interaction and request safety

Initial supplied text is imported locally. Subsequent import-field edits require
Import before Send. Plain Return in a request field never sends. Send is an explicit
button or exact Cmd-Return on macOS / Ctrl-Return on Linux. Method-menu ownership,
extra-modifier rejection and key-repeat suppression prevent accidental dispatch.
The method panel includes DELETE, GET, HEAD, OPTIONS, PATCH, POST and PUT, while
preserving imported custom methods. It is a GPUI panel, not an AppKit native menu.

Request fields are literal monospace editors. Search and New Window leave the
original window's draft/response intact; a fresh window has its own session.
Escape does not close the independent tool. Copy Response copies the full bounded
presentation, not only visible lines. Editing, rejected oversized input, marked
composition, Cancel, Close or Quit invalidates stale output and Copy immediately.
A later completion cannot resurrect retired state. Same-byte GPUI marked-text
regressions do not establish native OS IME behavior.

The importer never executes cURL, expands a shell/environment, or reads files.
Supported options follow the inspected Swift subset: request/header/data/JSON/URL,
GET/HEAD, literal inline cookies and Basic authorization, plus the supported
no-value flags. Non-raw `@file` bodies, cookie files, shell expansion/operators,
unknown options, multiple URLs and malformed quotes are rejected. Inline explicit
Cookie/Authorization headers can be sent after review; there is no implicit
credential lookup. Tests use synthetic credential strings only.

Preparation copies a private immutable snapshot. HTTP(S) URL authority/host are
required; URL userinfo, raw whitespace/control/backslash and malformed escapes
are refused. URL canonicalization uses Rust `url`; fragments are not transmitted.
Ordered duplicate headers are preserved in the prepared snapshot. Edited
Content-Length and Transfer-Encoding are validated then removed so transport
frames the actual edited body. Request/response content is memory-only, not saved
in application history or diagnostic logs.

## Bounds, transport and response

- Rust draft admission is 500,000 aggregate UTF-8 bytes across all request fields,
  checked again after normalization; the Swift import/body checks use 512,000 bytes.
- Method: at most 64 ASCII letters/hyphens. Import: at most 4,096 tokens.
- Header upper bounds: 256 fields and 64,000 bytes, including discarded framing
  input. Response display expansion is checked too. Reqwest 0.12.28 / Hyper 1.11.1
  impose a separate 100-field HTTP/1 response parser limit: 101 fields fail before
  Core's 256-field upper bound. This is an explicitly tested transport difference.
- A fresh ephemeral client/runtime disables ambient proxies, redirects, automatic
  retries, referer forwarding, and implicit cookie/credential lookup. No provider
  configuration or credential store is involved.
- Connect/read timeouts are 30 seconds; the asynchronous exchange has a 60-second
  absolute deadline. Header/body waits check cancellation on a 20 ms timer.
  Blocking system DNS and runtime disposal may outlast that logical deadline.
  Cancellation prevents publication, but no hard physical-shutdown deadline is
  claimed. The worker retains its shutdown admission until disposal really ends;
  a logically cancelled request does not reopen physical Send admission early.
- Redirects and 4xx/5xx remain inspectable responses. No status-based retry or
  redirect-following occurs. Core can render every three-digit status 100–999.
- Retained body is capped at exactly **2,000,000 bytes**, not 2 MiB. Truncation
  requires an actually observed byte beyond the cap. Exact-cap EOF, HEAD and empty
  responses are not declared truncated from Content-Length alone.
- Valid UTF-8 is retained; invalid UTF-8 (including a cap splitting a scalar)
  displays a binary-response description without lossy replacement. Complete,
  nontruncated JSON uses the existing lossless-number formatter within its
  500,000-byte / 20,000-node / 64-depth budget. Failure or excessive expansion
  falls back to the original text; a truncated JSON prefix is never prettified.
- Valid UTF-8 response header values remain text. Invalid UTF-8 values display
  `[non-UTF-8 bytes]` plus Rust `escape_ascii()` output, **not Latin-1 decoding**.
- Full Copy text is bounded at 2,064,512 bytes, including status/headers/body.
  Byte count means retained preview bytes, not total server size. English comma
  grouping and millisecond rounding are deterministic, not native locale parity.

## Validation checkpoint and remaining gates

At this documentation checkpoint, source-bound local results establish:

- 31 focused Core unit tests passed; seven isolated negative mutations were
  rejected (file import, framing replay, JSON joining, exact-cap truncation,
  decimal cap, UTF-8 loss and JSON fallback). Legacy raw CLI compatibility passed.
- Portable oracle binding/vector checks passed: 2 tests passed, native Swift test
  explicitly ignored on Linux. The corpus has 95 shared vectors (47 accepted,
  48 rejected) and 15 separately reported characterizations.
- 26 focused App/session/UI/transport tests passed, strict App all-target Clippy
  passed, and the ordinary App build passed. Aggregate Core checks passed 573 tests
  with 5 native tests ignored across harnesses; default App passed 578 with 2 ignored,
  recording-fixture App passed 594 with 2 ignored, and strict recording-feature
  Core/App Clippy passed. Minimal-feature App passed 500 with 2 ignored; minimal
  Core passed 379 across harnesses with 2 native tests ignored. Strict minimal
  Clippy and workspace formatting checks passed. Earlier failed attempts were repaired
  and rerun; their earlier source/test status is not silently transferred.
- All exercised requests were synthetic numeric-loopback requests. Dedicated
  transport tests cancel actual pending headers/body, test proxy isolation,
  no retry/redirect/cookie state, malformed/opaque headers, observed-byte caps,
  timeouts, continuous trickle deadlines and parser limits.
- Publication-held session tests deliberately delay adoption after transport has
  completed. They establish stale-result/terminal-payload retirement, not live
  socket cancellation. They do not replace actual close/quit GUI evidence.

Three ordinary-binary cloud Linux/lavapipe GUI runs now establish a scoped light/dark
full-window slice: inert initial import, exact explicit PATCH/Unicode body,
201/lossless JSON, visible Copy Response, fresh independent window, stale-output
invalidation, Cancel/edit/retry, inspectable 404 and 307, and no redirect-trap hit.
Five held loopback sockets physically closed across Cancel, edit, second-window
close, last-window Ctrl-W and app-controlled Ctrl-Q. All three processes exited 0 with
the same sealed binary hash. A dark empty restart retained no request/response
history; actual 740×560 resizing, scrolling, method-panel Down/Return and Escape
were observed. The first two runs used the existing BELLOBOX_TOOL selector. A
third run verified the actual Home Developer HTTP card and Search palette Open
routes, each opening an independent blank GET window with no network request.
Search preserved the original response. These are scoped textual observations; no screenshots were exported.
Native Copy was inspected at its beginning/end; exact byte equality and the full
bounded copy contract are established separately by the GPUI tests.

The tested binary SHA-256 is
`e3ff82259f3652a8c590db13835028017e498173c16a82e8bd80e9c67268ea92`;
its preexisting source-manifest SHA-256 is
`c2d912d23e8da8186f89e7a35a6f06d48cffc57c8df9ebc1b9a6800950425c64`.
Native Swift/Foundation oracle execution, exact published-commit Linux/macOS CI,
native macOS UI/IME/accessibility/materials, real DNS cancellation and unrestricted
URLSession equivalence are not established.
The native oracle compiles pinned pure Swift request/import/render statements;
it makes no URLSession request. Even a native corpus pass would not prove exact
prepared-URL equality, duplicate-header wire joining/casing, locale formatting,
all Swift grapheme edges or arbitrary network equivalence.

Swift automatically schedules import and disables request fields while sending;
this window uses explicit re-import and permits editing to invalidate/cancel old
work. Those interaction differences and the compact preview/session-transfer gap
remain explicit. Software-rendered Linux GUI evidence cannot prove macOS parity
or frame-rate/performance claims. No paid service, real credential, or user Mac
was used for these checks.

## Source accounting and evidence boundaries

The [validation package](validation/http-workflow-2026-10-09/README.md) records
[check outcomes](validation/http-workflow-2026-10-09/verification.json),
[GUI scope](validation/http-workflow-2026-10-09/gui.md), and the
[LOC ledger](validation/http-workflow-2026-10-09/loc.json).


Independent prospective LOC relative to the predecessor is **+1,891 production,
+1,966 test/support, +0 benchmark**, giving **67,188 / 50,357 / 105** for Box.
These are nonblank physical Rust lines including comments, using the prior Regex
ledger and exact positive test-gated spans/external test modules. Baseline Git
inventory/blobs are verified, unchanged candidate hashes must match, and shared
code is counted once under Box with no shared delta. Five negative accounting
controls reject wrong totals, changed classification, omitted changed files,
altered unchanged hashes and duplicate shared paths. Counts remain prospective
until the final publication source is checked; they are not parity percentages.

The GUI binary and its preexisting source manifest are separate evidence objects.
Documentation added afterward must have a separate supplement manifest; it must
not rewrite the GUI source seal or imply the new documents were part of that
binary's compilation. Provisional audit/review records stay outside product source
until deliberately packaged by the publication owner. Consult the exact eventual
CI SHA and final evidence supplement before upgrading any pending gate above.
