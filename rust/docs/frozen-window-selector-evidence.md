# Frozen Window selector: synthetic fixture only

The Window-only model selects from a bounded ordered catalog over immutable display
pixels. It is exposed only through BELLOBOX_WINDOW_FIXTURE=1 on a DEBUG build,
then the existing Screenshot route. Production Area and Window capture remain disabled.
No real window enumeration, capture, permission request or live replacement is added.

Source contracts are CaptureWindowCatalog, CaptureSelectionResolver.windowOnly,
CaptureOverlayController.updateHover and document(fromSnapshots:). Eligible windows
retain front-to-back order. Mouse-down latches the hover target; both clamped final
axes must be strictly below 8 points to commit. Blank clicks and longer drags never
fall through to Area or Screen. Cocoa window edges and local viewport edges retain
their distinct half-open boundaries.

Commit evidence is immutable. A worker copies only the selected rectangle into a
new window-sized base image, with no initial crop, annotations or Undo. Clearing a
later crop cannot expose surrounding display pixels. Frozen overlap remains visible.
Independent live replacement and alpha masking are separate future steps.

The existing frozen Area owner supplies immutable tiles, topology and cancellation.
The fixture retains both global launch and local commit identity through visible
selection and final editor handoff. New navigation, close, Escape, loss of activation,
cancellation or stale work prevents publication. The separate-editor handoff remains
a known difference from Swift's inline overlay editor.

## Fixture expectations

The synthetic display is 800x500 points and 1600x1000 pixels. Its orange front window
is at (300,80,340,270), the blue back window at (100,170,420,230). Overlap selects the
front window. Clicking it produces 680x540 pixels; clicking exposed blue produces
840x460 pixels including frozen orange overlap. Blank clicks and >=8-point drags
produce no editor; a 7x7 gesture remains a click. These are expected gestures, not
observed GUI results.

## Validation and remaining gates

All 20 focused core tests pass, including a fresh integration run on 760561d.
Strict core Clippy and formatting pass; independent source review cleared the final
implementation. The offline application check stops before compilation because a
locked dependency is absent from the local cache. Actual GPUI compilation and two
new fixture tests require existing CI; no local GUI run is claimed.

Native catalog/topology integration, production overlay enablement, source inline
editing, live image/alpha refresh and actual macOS pointer/focus/Spaces/TCC behavior
remain separate milestones. Synthetic selection does not validate those workflows.
