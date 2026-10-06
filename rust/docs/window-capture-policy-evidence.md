# Pure independent-window capture policy

This checkpoint adds metadata policy only. It does not enumerate or capture
windows, enable a chooser, add dependencies or implement an interim list UI.
The earlier unpublished candidate was lost with the workspace; this is freshly
reconstructed and independently reviewed source, with new verification.

The active Swift CaptureWindowCatalog is authoritative: normal-layer independent
windows, non-own positive PID, on-screen, alpha above .01, floored width/height
at least 8 and floored area at least 96. The legacy list's 20-point threshold is
not used. The current bounded target is a window wholly within the unrotated main
display; broader layouts and visible-frame/system-surface fallback remain absent.

Selected CG metadata, callback-local submitted-source evidence, and fresh CG
observations remain distinct. The exact native source is checked and its bundle
bound at submission. Completion checks fresh CG evidence; it does not reread the
old native object or claim a second bundle validation. Conflicting supplied
bundle evidence is rejected.
Frame, owner, display topology, session/generation and output size are checked.
Selection and plan retain their original cancellation flags, so substituting a
fresh caller flag cannot revive cancelled work. Metadata matches do not prove an
atomic window incarnation or substitute for actually retaining an SCWindow.

Sizing follows ScreenCoordinateSpace's independent per-axis maximum of CG pixels
and AppKit frame times backing scale, then the source's final output rounding.
No Window plan can convert to display capture/cropping if the target disappears.
The test-gated callback-local adapter now shares capture lease/encoding machinery
and validates fresh CG/topology/session state. Native lifetime/runtime validation
and the final host document/publication check remain gates. See
[native adapter scope](native-window-capture-evidence.md).

Fresh isolated verification: 29 focused tests pass; platform suite 99 passes,
3 existing opt-in subprocess tests ignored; strict Linux/Apple Clippy and Apple
test-target checking pass. After additive integration with the movie checkpoint,
29 focused tests and Linux strict library Clippy pass again. Native runtime and
full Window screenshot behavior are not claimed.
