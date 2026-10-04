# Bello shared GPUI workbench

`WorkbenchView::new(root, window, cx)` mounts lazy folder browsing, a file
editor, Changes and History tabs. Clicking staged/worktree chips reads that
specific patch; clicking a history commit reads its first-parent patch.
History is paginated. File switches preserve dirty drafts through explicit
Save and open / Discard and open / Keep editing choices. File/Git reads are
background tasks, protected against stale results; leaving a Git request
cancels its process. Rows are rendered with `gpui::uniform_list`.

`EditorView::new(text, window, cx)` starts ordinary text mode. Public APIs:
`text`/`get_text`, `set_text`, `set_vim`, `set_read_only`, `set_compact`,
`focus`, `set_appearance`, and GPUI `Focusable`. `EditorAppearance` separates
plain-input chrome from file-viewer chrome and exposes host font/spacing/colors. It emits `EditorEvent::Changed` and
`EditorEvent::SaveRequested`. `set_compact(true)` hides gutter/status chrome
and suppresses Enter, suitable for search fields. Plain appearance wraps to the
measured viewport width; code appearance does not. Wrapping uses GPUI font
metrics and a cached physical-to-visual row map, preserving source byte offsets
for selections, clipboard, IME and line numbers. The input handler supports
UTF-16 platform ranges and IME marked text; macOS composition needs on-device
validation. Multiline copy/cut/paste and mouse/shift-arrow selection are wired.

The Vim subset is explicitly documented in the core crate. The UI shapes a bounded horizontal slice of each visible line and follows the
caret horizontally; backing-buffer byte offsets remain intact for selection and
IME. This currently uses an approximate monospace viewport width, so wide-glyph
horizontal navigation needs additional runtime coverage. Diff display caps each
shaped line at 8,192 scalars (the raw patch remains in the Git result). There is currently no syntax coloring,
file watcher/live reload, find UI in ordinary mode, blame panel, minimap,
accessibility bridge, or multi-file tab system. These are remaining port work.

`WorkbenchAppearance` controls the shared workbench palette and visibility of
the internal tree/toolbar. `set_panel(WorkbenchPanel::Changes, cx)` lets the
host's existing navigation own panel selection without a second explorer.

App hosts must check `WorkbenchView::has_unsaved_changes(cx)` before destroying
the view/window; the shared component cannot intercept OS app termination.
Use the same exact GPUI version/features in both apps. BelloAgent should pin a
BelloBox Git revision containing these two crates; a sibling path is only a
local development convenience.

## Optional latency probes

Set `BELLO_PERF_LOG=/absolute/path.jsonl` before launching a benchmark process.
Logging is off by default. The shared editor records no text, key characters,
file names, paths, selections or credentials. It emits numeric editor/sample
IDs, input category, coalesced-input counts and timing only. A bounded 1,024
record `try_send` queue feeds a dedicated writer thread in batches of at most
64; a full/disconnected queue increments a drop counter rather than blocking
the event thread. `editor_telemetry_snapshot()` reports attempted/written/dropped
records and writer errors. Close a benchmark after allowing its writer to drain;
forced process termination can leave queued records unwritten.

- `editor_input_to_paint`: duration from the handled input callback start to the
  corresponding editor row's CPU paint completion. Includes editor/layout work,
  but not the rest of the window's painting or GPU/display completion.
- `editor_input_to_following_frame_callback`: duration from the same input to
  GPUI's next frame callback, registered after that paint. GPUI 0.2.2 executes
  this callback before the following frame's draw. It is **not** presentation
  latency or a measured frame rate.

Each record has `duration_microseconds` (earliest input in the coalesced batch),
`last_input_microseconds`, `coalesced_inputs`, and `dropped_samples_total`.
A sample may combine several inputs before one render. Never count every input
as an independently presented frame or confuse a launch timestamp with a
latency. The host can use `has_marked_text()` to avoid sending a composer while
IME text is still being composed. `measured_content_height(width, window)` and
`EditorEvent::LayoutChanged` support source-native composer height limits.
