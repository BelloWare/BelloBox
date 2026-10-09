# Editable palette QR preview

The QR row owns one transient editor and draft for the palette lifetime. Searching
or selecting another row keeps that draft; replacing/clearing the selection and
closing the palette retire it. Open passes the exact draft, including an empty
string, to the existing QR popup. Other tools still receive the original selection.
The existing explicit-open usage bookkeeping remains in the desktop router.

## Bounds and side effects

Editing, construction, navigation and preview generation are offline. They do not
read the clipboard, save settings, record usage, or call a provider. Paste, Clear,
Copy Image and Save are separate explicit actions. Empty/nontext clipboard leaves
the draft intact. QR encoding accepts up to 2,000 UTF-8 bytes; the 64 KB preview
limit and 500,000-byte popup input limit remain separate. Over-limit text is never
truncated. Preview-over-limit drafts can be handed off; input-over-limit drafts
show a recovery notice and disable Open until replaced or shortened.

Encoding runs on the background executor. An edit immediately invalidates the
accepted image and generation. Owner/session tokens, activity, exact draft matching,
and cancellation fence both stale success and stale error. Inactive pending work
cannot publish; reactivation schedules the retained draft if needed. Only current
compact/enlarged rasters and export bytes are retained, plus one explicit pending
Save snapshot. No payload or draft is persisted.

## Geometry and input

The compact white card is 128 points with a six-point inset. Actual encoder module
counts include the four-module quiet zone on each edge. The enlarged side rounds
(module count × 2 + 12 + 8) up to eight points and clamps to 192–384 points.
Both card PNGs use integral physical pixels per module within a two-pixels-per-point
budget and display at their actual raster size divided by two, rather than stretching
to fill the card. The origin is snapped to that 2× grid. Dense compact codes do not
promise integral 1× display pixels or successful scanning. PNG exports preserve the
existing at-least-512-pixel, at-least-four-pixels-per-module encoder behavior.

The same editor remains mounted across Enlarge/Compact changes. Its selection,
undo, composition, arrows and multiline Return belong to the editor. Tab and
Shift-Tab traverse the editor and visible enabled controls, then return to search.
Escape returns preview focus to search; held Escape cannot immediately close it.
Ctrl/Cmd+K restores search. Button activation consumes held repeats and the matching
release, preventing a synthetic second click or typing into a newly focused editor.
The palette resizes for the reserved row height; its command list scrolls within
the display-height clamp, with the palette footer kept outside that scroll region.

## Save and known platform differences

Save captures the accepted PNG bytes at click time. Cancel writes nothing; editing
while the destination dialog is pending does not retarget a later approved save.
The existing private/no-overwrite publication helper runs off the UI thread.
An independent save-status generation prevents a late completion message from
replacing newer edit status. A quit blocker lives through the dialog/write.
Launcher navigation, Open, deactivation close and ordinary close cannot dismiss the
palette while its save is pending. Clock's existing physical-retirement and handoff
gates are unchanged.

The supported GPUI dialog API is used without native host changes. On Linux it
requests a modal SaveFile portal using the active window identifier. GPUI 0.2.2 on
macOS uses NSSavePanel beginWithCompletionHandler, not Swift's palette-attached
beginSheetModalForWindow behavior. Native sheet/focus behavior is not established.
No-overwrite Save intentionally differs from Swift replacement semantics.

## Evidence scope

Implementation references Swift main e43b1c4595c42383e087fe70f38c28d07c31dda0:
LauncherUtilityPreviews.swift, LauncherPreviewSessions.swift, LauncherModel.swift,
LauncherWindowController.swift, QRCodePopupView.swift and QRCodeGenerator.swift.
Rust baseline is 5934b9897854a4102ef71d6463d4b4f7a3005158. The complete immutable
f1c21ae4f22b14dc10c5a57651499b05cc37ec92 migration handoff was read.

Automated tests cover actual GPUI entities and test-platform clipboard/dialog
routes, stale completion, immutable save bytes, no-overwrite, limits, retention,
focus and full-popup routing. These are distinct from an actual desktop session.
See the accompanying text-only validation report for exact commands/results and
source/binary hashes. Linux GUI acceptance, native macOS clipboard/IME/sheets,
Retina rendering and independent scanner success must each be reported separately.
No screenshot/capture artifacts are published by this slice.
