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

Selected CG metadata, metadata from the retained native object, and fresh CG
observations remain distinct. CG's missing bundle ID cannot erase a retained
bundle; a conflicting supplied bundle or changed retained bundle is rejected.
Frame, owner, display topology, session/generation and output size are checked.
Selection and plan retain their original cancellation flags, so substituting a
fresh caller flag cannot revive cancelled work. Metadata matches do not prove an
atomic window incarnation or substitute for actually retaining an SCWindow.

Sizing follows ScreenCoordinateSpace's independent per-axis maximum of CG pixels
and AppKit frame times backing scale, then the source's final output rounding.
No Window plan can convert to display capture/cropping if the target disappears.
Native object lifetime, shared capture leases, deadlines, privacy preflight and
exactly-once publication remain future adapter/caller obligations.

Fresh isolated verification: 29 focused tests pass; platform suite 99 passes,
3 existing opt-in subprocess tests ignored; strict Linux/Apple Clippy and Apple
test-target checking pass. After additive integration with the movie checkpoint,
29 focused tests and Linux strict library Clippy pass again. Native runtime and
full Window screenshot behavior are not claimed.
