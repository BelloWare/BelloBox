# Popup QR v3 actual Linux GUI acceptance

Validated 2026-10-09 UTC (parent clock). Cloud desktop filesystem clock is offset; action order is authoritative. V2 failing observations remain separately preserved in v2-partial-acceptance.md and v2/ logs. No images or capture inventories are part of this text receipt.

## Frozen identity and setup

- Candidate commit 4589e0280844df4a070533c7babb9d01cf430905; tree 294c99a7847757d2c41217834b8dc87eab9e4765.
- Ordinary package-clean default-feature binary SHA256 fe662658648c356888745ab72aaf0b8c9df85252f7b559680b7bc803ace7c174 independently rechecked.
- Manifest SHA256 e4cbbb6cb16a3c459b2b77ff4c86f858fba746930218709da819fb77737fd04e independently rechecked; all seven source file hashes matched.
- Explicitly closed v2 popup. Desktop-process check showed no v2 executable before launching v3. Original v2 PNG/logs retained.
- Cloud Linux X11, existing Mesa lavapipe environment and officially extracted GTK portal prerequisites, isolated dbus-run-session. Desktop launch through CUA Terminal; no global services, source/Cargo/remote writes, user Mac, credentials, spending or screenshot publication.

## Layout and decoded painting: passed

CUA window inventory independently reports client sizes, excluding window-manager decoration. Client origin was (93,117). After the image actually decoded/painted:

- 400×480: all QR finders and the full white footprint fit inside preview; no editor/header overlap; footer/status visible.
- 520×620: same containment, with larger QR; footer/status visible.
- 520×700: same containment, larger available preview; footer/status visible.
- Resized tall→minimum→preferred with accepted QR retained; no return of v2 overflow. Later dense 2,000-byte code was also contained at preferred and minimum sizes.
- Long held-key refusal text at preferred size scrolled to its final focused-Save recovery sentence.
- Saved long path at 400×480 scrolled to terminal filename suffix end.png while footer remained visible. The capability explanation remains in the scrollable slot and is visible at its top; naturally it can scroll off while reading later lines.
- No scanner or integral pixel/scannability claim follows from containment.

## Clipboard and editing: passed

- Copy Image visibly disabled on X11, with persistent unavailable/Save PNG explanation during ordinary state, Save, cancellation, validation errors and refusal.
- Independently typed external-popup-clipboard-sentinel-v3 into Mousepad, selected/copied there. Clicked popup Copy Image, returned to Mousepad, created a fresh document and pasted. Exact sentinel remained. Fixture is 36 UTF-8 bytes; SHA256 83e2a06ddc8926844af56368d594560a3946248bae7dd8e692b3c2e81a8f493b. This is an external text-preservation test, not external PNG delivery.
- Explicit Paste Text replaced popup draft and generated the changed QR. Previous preview was visibly invalidated during generation.
- Clear produced empty editor, 0-byte count and Enter text to encode. Save reported 1–2,000-byte validation error without opening chooser.
- Pasted exactly 2,001 ASCII bytes; preserved 2,001-byte count, red remove-one-byte message and no QR. Save opened no chooser.
- Ctrl+End then Backspace in editor reduced to exactly 2,000 bytes and produced the dense accepted QR.

## Save, no-overwrite and held keys: passed within stated scope

Popup portal log records exactly FOUR SaveFile requests, matching four observed choosers:
1. Ordinary original Save → v3-original.png.
2. Fresh pointer Save after held-key recovery → Cancel, no file written.
3. Edited-draft Save to existing disposable v3-original.png; accepted anticipated GTK Replace warning under explicit test authorization. App refused with File exists (os error 17), existing files are never overwritten. Original bytes/hash matched retained copy afterward.
4. Dense 2,000-byte QR saved under long synthetic filename.

- Original PNG: 528×528 grayscale L, 3,353 bytes; SHA256 3ae3d4a72c51418e5abb57d19da7b9db514ee398ac051850be6eb47ef8209c74. Byte-identical to original v2 export for same 20-byte payload popup-qr-v2-sentinel.
- Dense PNG: 708×708 grayscale L, 27,421 bytes; SHA256 9b7145b1a27e5eccc99a1cbe312edb543f0bfa2ad39ebbc40c5c00fac84bb049. Full file names and metadata in png-receipts.json.
- Actual immediate held Return+mouse Save refused with release/retry explanation, valid accepted image retained, unchanged 20-byte draft, no chooser and no automatic replay after key release.
- On focused non-editing Save, a normal Return press/release left draft unchanged and opened no chooser. Subsequent fresh mouse Save created exactly one chooser (request 2).
- Repeated same immediate chord test with Space after Cancel; refusal again, no extra chooser; normal Space press/release on focused Save did not mutate draft or replay Save. Portal count remained 2 until the later deliberate edited-draft Save.
- These held-key tests began with valid accepted QR and Save focus, so they do not confuse rejection with editor-induced image invalidation.

## Palette shared-policy smoke: passed

Separately launched same v3 ordinary binary in Home, opened palette, selected QR, typed v3. Current QR rendered (2 bytes, 29×29 modules including quiet zone). Copy Image stayed disabled with X11/Save PNG explanation; explicit click produced no success claim. Save opened one actual chooser and Cancel returned Save cancelled plus capability explanation. Exactly ONE SaveFile request in separate v3-home/portal.log; no file written by this smoke.

## Cleanup and limitations

Popup, palette and Home closed normally with no chooser pending. Both isolated portal sessions exited. Desktop ps check cleanup-processes.txt is empty for ordinary binaries, xdg portal/permission-store and task dbus-run-session processes. No source modification by validator.

Input used documented cloud CUA sky.click key chords and press_key calls. Immediate held-key mouse chords are established; this did not simulate an independently held key across a long wait, a missed release outside the window, simultaneous independent Enter+Space hold/release ordering, or sustained hardware auto-repeat. Recovery behavior on the focused non-editing Save is observed, but missed-outside-release setup and selection preservation under that setup remain portable test evidence. No native macOS NSPasteboard/AppKit/IME/AX, hardware keyboard, Wayland external clipboard, hardware-GPU, or independent scanner acceptance. Portable/TestPlatform snapshot and image clipboard tests remain distinct.

CUA terminal/Mousepad type_text sometimes failed to expose Paste or insert text; documented key entry was used instead. GTK filename/location text entry worked. No such helper-input failure is reported as a product failure.

The independent Mousepad source document and post-Copy fresh paste document were both saved during editor cleanup as clipboard-source.txt and clipboard-observed.txt. Both are exactly 36 bytes and share the sentinel SHA256 above. These are actual GUI-originated source/observed text receipts, not regenerated expected-output files.
