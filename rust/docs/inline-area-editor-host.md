# Inline Area editor through the owned capture coordinator

This checkpoint replaces the main-display Area caller's separate-editor handoff
with an inline host for the actual `ScreenshotEditor`. Production Area remains
`PRODUCTION_AREA_ENABLED = false`; the public Window capture gate is unchanged.
No ScreenCaptureKit acquisition, Screen Recording permission, native display
capture or third-party pixels are used by the supplied-pixel validation route.

## Source and host boundary

Behavioral references are `CaptureOverlayController.swift:470–545,1544–1759`,
`ScreenshotPopupView.swift:356–409`, `SelectionResizeGeometry.swift` and
`AnnotationToolbar.swift`. The existing overlay window and coordinator own the
same transaction from freeze, selection and font preparation through inline
editing and retirement. No new screenshot popup opens on selection lock.

The selector mounts a real `ScreenshotEditor` entity. Its annotation canvas,
label editing, gestures, styles/masks, history and output preparation are shared
with the existing popup, whose crop-handle eligibility remains unchanged.
The inline shell supplies display/crop positioning and selection handles rather
than implementing another drawing, document or export engine.

The full display remains the immutable base. Preview tiles are generated in
full-display coordinates and clipped to the displayed crop, so interactive
adjustment changes only crop geometry. Annotations retain document-pixel
coordinates. Independent horizontal/vertical pixels-per-point ratios are used;
image ratios are not inferred from GPUI or the display backing scale. Initial
half/fractional pixel edges retain the source's outward rounding. Local-to-pixel
adjustment multiplies by the independently computed ratio, matching the Swift
operation instead of dividing by its rounded reciprocal.

Eight 22-point hit targets show 11-point circular handles (14 while active).
Resize and Select-move share the core `SelectionAdjustmentDraft`, hold the
original drag rectangle/pointer origin, and commit one crop history edit at
release. The live cutout follows the draft, later Crop/Reset Crop, Undo and Redo.
Undo during an uncommitted draft cancels that draft before older history.
Copy, Save and OCR's common output-snapshot preparation first commit any visible
adjustment draft, so exported/recognized pixels cannot come from the previous,
larger crop. Active annotation gestures, including Crop and opaque Mask, refuse
output until pointer release instead of exporting older unredacted pixels;
refusal preserves the gesture and its eventual single Undo step. Export still uses full-resolution core rendering, never preview PNGs.

The toolbar uses the source 980-point preferred width, 44-point height, 10-point
gap and 12-point clamping inset. Its tool/style/history controls reuse the popup;
Copy, Save, Cancel and Copy & Finish use the editor's existing actions. Controls
remain horizontally reachable on narrower overlays. Handles are above the toolbar
when it must lie inside a tall selection; menus, color controls and export locks
are above the handles. Inline menus/colors use bounded toolbar-relative anchors;
popup placement remains unchanged.

## Lifetime, cancellation and resources

Application-deactivation observation ends immediately when selection locks,
before asynchronous font preparation. Subsequent preparation and inline editing
ignore loss of app/key focus, do not reactivate the app, and retain transaction
ownership. Pre-lock app deactivation still cancels. Navigation, requester close,
explicit Cancel/Escape, invalid topology and transaction cancellation continue
to fence preparation, mutations and publication after lock.

Inline preview, Copy, Save-dialog/export and OCR futures are counted until their
physical completion, independently of the selector and editor entities. Copy's
terminal accounting uses an App-owned async context even when its window no
longer exists. Stale completion detecting changed topology retires the owner,
rather than leaving an unusable overlay or global busy state. Late Copy results
cannot update the clipboard after cancellation. Save rechecks the boundary before
starting its worker and checks the cancellation flag before the write; an already
started filesystem operation is not described as forcibly interruptible.

Retirement cancels UI jobs and transfers the editor's actual full session,
history and preview PNG owners to counted background disposal. A retained editor
entity keeps only an inert transparent 1×1 document. The selector's original
frozen tiles are also disposed through the coordinator. Busy is released only
when selector ownership, restoration and all counted work have drained. This
counts owned work/resources, not all opaque GPU/framework allocation lifetimes.

## Reproduce without capturing

A DEBUG build accepts `BELLOBOX_INLINE_AREA_FIXTURE=1` with the existing
`BELLOBOX_TOOL=screenshot` startup route. It prepares an app-generated 1000×640
point / 2000×1280 pixel surface and enters the same coordinator freeze completion,
selector, editor mount and retirement functions. A separately tagged host source
substitutes only supplied pixels and synthetic host boundaries. The production
`begin()` gate remains the first operation and has no environment bypass.
Use a separate `BELLOBOX_CONFIG_DIR`; do not click the ordinary Screen action.

GPUI tests additionally supply a 600×400 point / 1200×1000 pixel surface to expose
anisotropic errors. They cover same-window ownership; every handle and move;
shared Crop and Undo/Redo; annotation coordinates; OCR invalidation and actual
OCR snapshot bytes without invoking OCR; keyboard Copy during a live adjustment;
clipboard dimensions/pixels; Copy & Finish and external close with the editor
entity dropped; actual retired payload disposal; lock/deactivation timing;
requester/navigation/topology fences; production gate refusal; and actual handle,
menu and color-scrim hit order. See the final validation report for executed
counts and immutable candidate hashes in
[the retained QA report](validation/inline-area-2026-10-07/QA-report.md).

## Remaining limits

These are supplied-pixel host tests, not native AppKit/TCC/ScreenCaptureKit,
Spaces/Stage Manager, display hotplug, multi-display or Retina alignment acceptance.
The native Area entry remains restricted to its validated main-display topology.
The source live Scrolling Capture action/native acquisition is not enabled by
this toolbar. Window capture remains a fixed-size separate workflow without Area
resize handles. The pre-existing DEBUG Area fixture remains separate; use the
new explicit inline fixture for this coordinator's acceptance. Native material,
accessibility/IME, popup font/raster fidelity and system Save-dialog acceptance
remain separate gates. No performance/frame-rate or whole-feature parity claim
is made from these tests.

## Reviewed source accounting

Relative to the published converter baseline `c960d6e`, this checkpoint adds
828 production and 973 test/support nonblank Rust lines (comments included).
The resulting scoped totals are **44,273 production / 21,665 test/support /
105 benchmark** lines. Positive DEBUG/test-only spans and test modules are
classified separately. See [loc-delta.json](validation/inline-area-2026-10-07/loc-delta.json)
for owned-file hashes and reviewed ranges; these totals are not parity or
performance percentages.
