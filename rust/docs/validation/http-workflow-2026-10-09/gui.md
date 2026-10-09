# HTTP workflow GUI observations

These are scoped textual observations on the ordinary, source-sealed cloud Linux
Mesa lavapipe binary. No screenshot or binary is included.

Binary SHA-256: `e3ff82259f3652a8c590db13835028017e498173c16a82e8bd80e9c67268ea92`.

Source manifest SHA-256: `c2d912d23e8da8186f89e7a35a6f06d48cffc57c8df9ebc1b9a6800950425c64`.

## Observed workflows

- Initial cURL import and reviewed PATCH fields caused zero fixture connections before explicit Send.
- Ctrl-Shift-Return and Ctrl-Alt-Return caused zero connections and dirtied import text; explicit re-import restored review.
- Visible standard-method panel owned Ctrl-Return with zero connections and Send disabled.
- Explicit Ctrl-Return sent PATCH /inspect with exact39-byte UTF-8 JSON, integer9007199254740993 and snow/emoji; fixture recorded explicit X-Fixture header.
- 201 response rendered sorted headers and lossless pretty JSON; native Copy Response pasted beginning status and ending Unicode body into independent empty window. GPUI exact-copy tests separately establish full byte equality and2MB bound.
- New Window started blankGET, and closing it preserved the original response.
- Editing URL cleared old response and Copy availability.
- Two Send shortcuts while one held body was live produced one request. Explicit Cancel closed its socket, withheld partial response and restored Send.
- Editing a second live held request closed its socket without publishing stale output. Explicit retry returned404 with its inspectable body.
- 307 response displayed Location and redirect body; trap endpoint received zero requests.
- Closing second window with live heldGET closed only that socket; first window survived.
- Last-window Ctrl-W during live heldPATCH closed socket and exited0.
- Dark-mode process restart with no seed had empty import/URL/headers/body, GET method, no result and zero connections. Isolated settings contained neither loopback URL, large integer/body nor response text.
- Native resize verified740x560. Method choices remained visible below trigger; native Down/Return choseDELETE and restored trigger focus. Escape retained full window.
- At740x560 outer scrolling exposed the footer disclaimer while Send/Cancel/Copy stayed fixed and reachable.
- App-controlled Ctrl-Q during a live heldDELETE closed socket then exited0.
- Search from a completed response preserved that independent HTTP window and its response (first run).
- Home Developer filter http displayed the HTTP & cURL card; clicking it opened an independent blank GET HTTP window (third run).
- Ctrl-K from the Home-opened HTTP window displayed the task-owned palette with HTTP & cURL selected; clicking its Open button created a second independent blank GET HTTP window. Both windows were present; Home navigation and both Open routes produced zero fixture connections.
- Ctrl-Q closed the route-verification app and all its task windows with exit 0; all three normal exits preserved the sealed ordinary binary hash.

## Boundaries

- No actual macOS/AppKit, OS-native IME, hardware-GPU or performance/frame-rate claim.
- Native Ctrl-Home in copied multiline editor produced no observed movement; Ctrl-A then Left exposed the beginning. This is not a Ctrl-Home acceptance claim.
- One native text injection failed with unavailable AT-SPI provider; per-character native key input recovered without resetting CUA. Some immediate captures preceded later rendered completion; no frame latency is inferred.
- System DNS cancellation is untested; async60s budget is not a hard physical-runtime disposal bound.
- Session publication-held GPUI fixtures concern post-network adoption, distinct from actual sockets above.
- No screenshots exported; observations are recorded as scoped textual acceptance.

The first two runs used the existing HTTP tool selector. The third exercised
actual Home and palette Open routes. The five held-socket closures were real
numeric-loopback transport observations, separate from publication-held model
tests. All three app processes exited 0 with the same binary hash.

See [machine-readable receipt](gui-receipt.json), [original GUI seal](gui-seal.json),
[original source manifest](gui-source-manifest.json), and the per-run synthetic
fixture JSON files. Recorded session intervals include observation/coordination;
they are not inference time, engineering effort, or performance measurements.
