//! Mountable GPUI views shared by BelloBox and BelloAgent.
mod appearance;
mod editor_view;
mod telemetry;
mod workbench_view;
mod wrapping;
pub use appearance::{EditorAppearance, WorkbenchAppearance};
pub use editor_view::{EditStateError, EditorEditState, EditorEvent, EditorView};
pub use telemetry::{EditorTelemetrySnapshot, editor_telemetry_snapshot};
pub use workbench_view::{WorkbenchPanel, WorkbenchView};
/// Views route their keyboard events locally, so there are no global bindings.
pub fn init(_cx: &mut gpui::App) {}
