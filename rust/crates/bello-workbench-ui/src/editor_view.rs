use crate::telemetry::{InputKind, Tracker};
use crate::text_presentation::{intersection, validate_range};
use crate::{
    EditorAppearance,
    wrapping::{DisplayRow, WrappedRows},
};
use crate::{PresentationError, PresentationGeometry, TextPresentation};
use bello_workbench::editor::{Editor, Key, Mode, TextBuffer};
use gpui::{prelude::*, *};
use std::time::Instant;
use std::{collections::BTreeMap, ops::Range};
const MAX_DRAW_LINE: usize = 16_384;
#[derive(Clone, Debug)]
pub enum EditorEvent {
    Changed,
    SaveRequested,
    LayoutChanged,
}
/// In-memory, move-only editing state. Contains no GPUI/platform handles,
/// composition, layout, subscriptions or persisted data. Deliberately neither
/// Clone nor Debug: moving state must not duplicate history or expose its text.
pub struct EditorEditState {
    engine: Editor,
    selection: Option<Range<usize>>,
}
impl EditorEditState {
    /// Conservative cache accounting, not actual allocator/RSS measurement.
    /// Includes capacities and an allocation allowance; see Editor::accounted_bytes.
    pub fn accounted_bytes(&self) -> usize {
        self.engine
            .accounted_bytes()
            .saturating_add(std::mem::size_of::<Self>() - std::mem::size_of::<Editor>())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditStateError {
    Focused,
    Composing,
    Dragging,
    MissingState,
    StaleText,
    EditedDestination,
    IncompatibleConfiguration,
}

fn check_edit_state_transfer(
    focused: bool,
    marked: &Option<Range<usize>>,
    dragging: &Option<usize>,
) -> Result<(), EditStateError> {
    if focused {
        return Err(EditStateError::Focused);
    }
    if marked.is_some() {
        return Err(EditStateError::Composing);
    }
    if dragging.is_some() {
        return Err(EditStateError::Dragging);
    }
    Ok(())
}
fn take_edit_state(
    engine: &mut Editor,
    selection: &mut Option<Range<usize>>,
    focused: bool,
    marked: &Option<Range<usize>>,
    dragging: &Option<usize>,
) -> Result<EditorEditState, EditStateError> {
    check_edit_state_transfer(focused, marked, dragging)?;
    let mut detached = Editor::new(String::new());
    detached.read_only = true;
    Ok(EditorEditState {
        engine: std::mem::replace(engine, detached),
        selection: selection.take(),
    })
}
fn restore_edit_state(
    engine: &mut Editor,
    selection: &mut Option<Range<usize>>,
    state: &mut Option<EditorEditState>,
    focused: bool,
    marked: &Option<Range<usize>>,
    dragging: &Option<usize>,
) -> Result<(), EditStateError> {
    check_edit_state_transfer(focused, marked, dragging)?;
    let saved = state.as_ref().ok_or(EditStateError::MissingState)?;
    if engine.text() != saved.engine.text() {
        return Err(EditStateError::StaleText);
    }
    if engine.revision() != 0 {
        return Err(EditStateError::EditedDestination);
    }
    if engine.vim != saved.engine.vim || engine.read_only != saved.engine.read_only {
        return Err(EditStateError::IncompatibleConfiguration);
    }
    let saved = state.take().expect("checked above");
    *engine = saved.engine;
    *selection = saved.selection;
    Ok(())
}

type MeasurementCallback = Box<dyn FnOnce(PresentationGeometry, &mut Window, &mut App)>;
struct PresentationMeasurement {
    serial: u64,
    token: u64,
    host_generation: u64,
    range: Range<usize>,
    callback: MeasurementCallback,
}
#[derive(Clone)]
struct PaintedFragment {
    range: Range<usize>,
    bounds: Bounds<Pixels>,
    unclipped: bool,
}

pub struct EditorView {
    pub engine: Editor,
    appearance: EditorAppearance,
    telemetry: Option<Tracker>,
    wrapped: WrappedRows,
    viewport_width: f32,
    measured_viewport_width: Option<f32>,
    style_revision: u64,
    reveal_cursor: bool,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    marked: Option<Range<usize>>,
    selection: Option<Range<usize>>,
    layouts: BTreeMap<usize, (ShapedLine, Bounds<Pixels>, usize)>,
    horizontal_column: usize,
    draw_columns: usize,
    dragging: Option<usize>,
    compact: bool,
    requested_read_only: bool,
    presentation: Option<TextPresentation>,
    validated_paint_text: Option<(usize, usize, u64)>,
    presentation_reveal: Option<Range<usize>>,
    presentation_holds_scroll: bool,
    presentation_measurement: Option<PresentationMeasurement>,
    measurement_serial: u64,
    layout_generation: u64,
    measurement_frame_scheduled: Option<u64>,
    painted_fragments: BTreeMap<usize, PaintedFragment>,
}
impl EventEmitter<EditorEvent> for EditorView {}
impl Focusable for EditorView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl EditorView {
    pub fn new(text: String, _: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            engine: Editor::new(text),
            appearance: EditorAppearance::code(),
            telemetry: Tracker::configured(),
            wrapped: WrappedRows::default(),
            viewport_width: 640.,
            measured_viewport_width: None,
            style_revision: 0,
            reveal_cursor: true,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            marked: None,
            selection: None,
            layouts: BTreeMap::new(),
            horizontal_column: 0,
            draw_columns: 120,
            dragging: None,
            compact: false,
            requested_read_only: false,
            presentation: None,
            validated_paint_text: None,
            presentation_reveal: None,
            presentation_holds_scroll: false,
            presentation_measurement: None,
            measurement_serial: 0,
            layout_generation: 0,
            measurement_frame_scheduled: None,
            painted_fragments: BTreeMap::new(),
        }
    }
    /// Explicit editing-view selection command for hosts such as a Find field.
    /// Unlike presentation decoration, this intentionally changes selection.
    /// It never claims focus or alters text/history, and refuses platform-owned
    /// composition or an active drag rather than discarding either.
    pub fn select_all(&mut self, cx: &mut Context<Self>) -> Result<(), PresentationError> {
        if self.marked.is_some() || self.dragging.is_some() {
            return Err(PresentationError::Busy);
        }
        self.abandon_presentation_navigation();
        self.selection = Some(0..self.text().len());
        cx.notify();
        Ok(())
    }
    /// Install transient drawing only. Validation is atomic; no text, input,
    /// selection, focus, undo, or Changed event is touched. Clearing never jumps
    /// back to the caret. Hosts must bind their document identity in `token`.
    pub fn set_text_presentation(
        &mut self,
        presentation: Option<TextPresentation>,
        cx: &mut Context<Self>,
    ) -> Result<(), PresentationError> {
        if let Some(value) = &presentation {
            value.validate(self.text())?;
        }
        self.presentation = presentation;
        self.validated_paint_text = self
            .presentation
            .as_ref()
            .map(|_| self.paint_text_identity());
        self.presentation_reveal = None;
        self.presentation_measurement = None;
        self.invalidate_presentation_layout();
        cx.notify();
        Ok(())
    }
    fn paint_text_identity(&self) -> (usize, usize, u64) {
        (
            self.text().as_ptr() as usize,
            self.text().len(),
            self.engine.revision(),
        )
    }
    // Exact comparison at installation/render is the authority. These guards
    // detect ordinary pointer/revision changes between validation and paint,
    // without rescanning the full document for each row. Allocator reuse is not
    // document identity: final receipt validation compares exact bytes again,
    // and the host token/lifetime must fence identical-text document replacement.
    fn paint_presentation(&self) -> Option<&TextPresentation> {
        (self.validated_paint_text == Some(self.paint_text_identity()))
            .then_some(self.presentation.as_ref())
            .flatten()
    }
    fn checked_presentation(&self, token: u64) -> Result<&TextPresentation, PresentationError> {
        let value = self
            .presentation
            .as_ref()
            .ok_or(PresentationError::StaleToken)?;
        if value.token != token {
            return Err(PresentationError::StaleToken);
        }
        if value.text.as_ref() != self.text() {
            return Err(PresentationError::TextChanged);
        }
        Ok(value)
    }
    /// Scroll only, without editing the caret or claiming keyboard focus.
    /// Newest accepted request wins. Busy leaves composition/dragging intact.
    pub fn reveal_presented_range(
        &mut self,
        token: u64,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), PresentationError> {
        self.checked_presentation(token)?;
        validate_range(self.text(), &range)?;
        if self.marked.is_some() || self.dragging.is_some() {
            return Err(PresentationError::Busy);
        }
        self.presentation_measurement = None;
        self.presentation_reveal = Some(range);
        self.presentation_holds_scroll = true;
        self.reveal_cursor = false;
        self.invalidate_presentation_layout();
        cx.notify();
        Ok(())
    }
    /// Cancel pending reveal AND measurement for this token only. This does not
    /// scroll to a previous position. A newer presentation is left untouched.
    pub fn cancel_presentation_reveal(&mut self, token: u64, cx: &mut Context<Self>) {
        if self
            .presentation
            .as_ref()
            .is_some_and(|value| value.token == token)
        {
            self.presentation_reveal = None;
            self.presentation_measurement = None;
            self.invalidate_presentation_layout();
            cx.notify();
        }
    }
    /// Arm one measurement. The callback runs once at the end of the paint effect cycle after a fresh frame
    /// paints a visible fragment, with no EditorView borrow held. There is no
    /// cached-geometry getter. Offscreen/clipped targets produce no receipt and
    /// do not schedule animation loops: the host bounds retries/timeouts.
    ///
    /// Receipt coordinates belong ONLY to that completed paint. The callback
    /// must recheck its host navigation/lifetime generation before acting and
    /// must not retain coordinates across a later outer scroll/layout. Input,
    /// text replacement, cancellation and newer requests suppress old callbacks.
    pub fn measure_presented_range(
        &mut self,
        token: u64,
        range: Range<usize>,
        host_generation: u64,
        callback: impl FnOnce(PresentationGeometry, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Result<(), PresentationError> {
        self.checked_presentation(token)?;
        validate_range(self.text(), &range)?;
        if self.marked.is_some() || self.dragging.is_some() {
            return Err(PresentationError::Busy);
        }
        self.measurement_serial = self.measurement_serial.wrapping_add(1);
        self.presentation_measurement = Some(PresentationMeasurement {
            serial: self.measurement_serial,
            token,
            host_generation,
            range,
            callback: Box::new(callback),
        });
        self.invalidate_presentation_layout();
        cx.notify();
        Ok(())
    }
    /// Hosts call this before changing outer geometry. A fresh armed measurement
    /// is needed afterwards. It is not a substitute for checking host generation
    /// inside the one-shot measurement callback.
    pub fn invalidate_presentation_geometry(&mut self, cx: &mut Context<Self>) {
        self.presentation_measurement = None;
        self.invalidate_presentation_layout();
        cx.notify();
    }
    fn invalidate_presentation_layout(&mut self) {
        self.layout_generation = self.layout_generation.wrapping_add(1);
        self.painted_fragments.clear();
        self.measurement_frame_scheduled = None;
    }
    fn abandon_presentation_navigation(&mut self) {
        self.presentation_reveal = None;
        self.presentation_measurement = None;
        self.invalidate_presentation_layout();
    }
    fn discard_presentation(&mut self) {
        self.presentation = None;
        self.validated_paint_text = None;
        self.abandon_presentation_navigation();
    }
    /// Let virtual hosts keep interacting rows mounted before attempting eviction.
    /// The transfer methods repeat this check immediately before mutation.
    pub fn suspension_blocker(&self, window: &Window) -> Option<EditStateError> {
        check_edit_state_transfer(self.focus.is_focused(window), &self.marked, &self.dragging).err()
    }
    /// Suspend an inactive view before dropping it. On success this view is an
    /// empty placeholder and must be dropped, not reused. Failure is nonmutating.
    /// This does not commit an open edit group or emit Changed.
    pub fn take_edit_state(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<EditorEditState, EditStateError> {
        let state = take_edit_state(
            &mut self.engine,
            &mut self.selection,
            self.focus.is_focused(window),
            &self.marked,
            &self.dragging,
        )?;
        self.invalidate_edit_state_layout();
        cx.notify();
        Ok(state)
    }
    /// Restore into a newly created, already configured inactive view whose
    /// text still exactly matches the retained state. Errors leave both intact.
    /// Configure compact/Vim/read-only before restoration: mode setters can
    /// finish edit groups. Hosts must also validate their document identity and
    /// generation; equal text alone cannot detect replacement by another document.
    /// No Changed event is emitted and pending edit groups remain open.
    pub fn restore_edit_state(
        &mut self,
        state: &mut Option<EditorEditState>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<(), EditStateError> {
        restore_edit_state(
            &mut self.engine,
            &mut self.selection,
            state,
            self.focus.is_focused(window),
            &self.marked,
            &self.dragging,
        )?;
        self.invalidate_edit_state_layout();
        cx.notify();
        Ok(())
    }
    fn invalidate_edit_state_layout(&mut self) {
        self.discard_presentation();
        self.presentation_holds_scroll = false;
        self.layouts.clear();
        self.wrapped.clear();
        self.measured_viewport_width = None;
        self.horizontal_column = 0;
        self.reveal_cursor = true;
    }
    pub fn set_appearance(&mut self, mut appearance: EditorAppearance, cx: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        appearance.validate();
        self.appearance = appearance;
        self.measured_viewport_width = None;
        self.style_revision = self.style_revision.wrapping_add(1);
        self.wrapped.clear();
        self.reveal_cursor = !self.presentation_holds_scroll;
        self.layouts.clear();
        cx.notify();
    }
    pub fn appearance(&self) -> &EditorAppearance {
        &self.appearance
    }
    pub fn set_composer_mode(&mut self, cx: &mut Context<Self>) {
        self.compact = false;
        self.set_vim(false, cx);
        self.set_appearance(
            EditorAppearance {
                font_size: 14.,
                line_height: 21.,
                padding_x: 12.,
                padding_y: 8.,
                ..EditorAppearance::plain()
            },
            cx,
        );
    }
    pub fn has_marked_text(&self) -> bool {
        self.marked.is_some()
    }
    /// Last measured visual-row count. Subscribe to LayoutChanged for reflow.
    pub fn display_row_count(&self) -> usize {
        self.display_count()
    }
    /// Measure the complete editor content using its proposed outer width.
    /// The host can clamp the result to its native composer's min/max height.
    pub fn measured_content_height(&mut self, width: f32, window: &mut Window) -> f32 {
        let gutter = if self.appearance.show_gutter && !self.compact {
            self.appearance.gutter_width
        } else {
            0.
        };
        // Once painted, real row bounds are authoritative. Replacing them with
        // a host's estimate each render creates an estimate -> actual -> notify
        // feedback loop whenever border/padding arithmetic differs by a pixel.
        self.viewport_width = measurement_width(
            width - self.appearance.padding_x * 2. - gutter,
            self.measured_viewport_width,
        );
        self.prepare_wrap(window);
        self.display_count() as f32 * self.appearance.line_height
            + self.appearance.padding_y * 2.
            + if self.appearance.show_status && !self.compact {
                26.
            } else {
                0.
            }
    }
    pub fn text(&self) -> &str {
        self.engine.text()
    }
    pub fn get_text(&self) -> &str {
        self.text()
    }
    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.discard_presentation();
        self.presentation_holds_scroll = false;
        let vim = self.engine.vim;
        let read_only = self.requested_read_only;
        self.engine = Editor::new(text);
        self.engine.set_vim(vim);
        self.engine.read_only |= read_only;
        self.marked = None;
        self.selection = None;
        self.layouts.clear();
        self.horizontal_column = 0;
        self.wrapped.clear();
        self.reveal_cursor = true;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }
    pub fn set_vim(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if set_vim_unless_marked(&mut self.engine, &self.marked, &mut self.selection, enabled) {
            self.abandon_presentation_navigation();
            cx.notify();
        }
    }
    pub fn set_read_only(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.requested_read_only = enabled;
        self.engine.read_only =
            enabled || self.text().len() > bello_workbench::editor::MAX_EDIT_BYTES;
        cx.notify();
    }
    pub fn set_compact(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        self.compact = enabled;
        if enabled {
            self.engine.set_vim(false);
        }
        cx.notify();
    }
    /// Reveal an explicit 1-based line without replacing text or undo history.
    pub fn reveal_line(&mut self, line: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        self.presentation_holds_scroll = false;
        let row = line
            .saturating_sub(1)
            .min(self.engine.buffer.line_count().saturating_sub(1));
        self.engine.set_cursor(self.engine.buffer.line_start(row));
        self.selection = None;
        self.marked = None;
        self.reveal_cursor = true;
        self.focus.focus(window);
        cx.notify();
    }
    pub fn focus(&self, window: &mut Window) {
        self.focus.focus(window);
    }
    fn selected(&self) -> Range<usize> {
        self.selection
            .clone()
            .or_else(|| self.engine.selection())
            .unwrap_or(self.engine.cursor..self.engine.cursor)
    }
    fn to_utf16(&self, r: Range<usize>) -> Range<usize> {
        self.engine.buffer.utf16_offset(r.start)..self.engine.buffer.utf16_offset(r.end)
    }
    fn byte_range_for_utf16(&self, r: Range<usize>) -> Range<usize> {
        self.engine.buffer.byte_offset(r.start)..self.engine.buffer.byte_offset(r.end)
    }
    fn changed(&mut self, before: u64, cx: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        self.presentation_holds_scroll = false;
        if self.engine.revision() != before {
            self.discard_presentation();
            cx.emit(EditorEvent::Changed);
        }
        self.reveal_cursor = true;
        cx.notify();
    }
    fn wraps(&self) -> bool {
        self.appearance.wrap_lines && !self.compact
    }
    fn display_count(&self) -> usize {
        if self.wraps() {
            self.wrapped.rows.len().max(1)
        } else {
            self.engine.buffer.line_count()
        }
    }
    fn display_row(&self, row: usize) -> DisplayRow {
        if self.wraps() {
            self.wrapped.rows.get(row).cloned().unwrap_or(DisplayRow {
                source_row: 0,
                range: 0..0,
                first: true,
            })
        } else {
            DisplayRow {
                source_row: row,
                range: self.engine.buffer.line_range(row),
                first: true,
            }
        }
    }
    fn display_row_at(&self, offset: usize) -> usize {
        if self.wraps() {
            self.wrapped.row_at(offset)
        } else {
            self.engine.buffer.row_at(offset)
        }
    }
    fn move_visual_row(&mut self, down: bool) {
        let row = self.display_row_at(self.engine.cursor);
        let current = self.display_row(row);
        let column = self.text()[current.range.start..self.engine.cursor.min(current.range.end)]
            .chars()
            .count();
        let target = if down {
            row.saturating_add(1).min(self.display_count() - 1)
        } else {
            row.saturating_sub(1)
        };
        let target = self.display_row(target);
        let offset = self.text()[target.range.clone()]
            .char_indices()
            .nth(column)
            .map(|(i, _)| target.range.start + i)
            .unwrap_or(target.range.end);
        self.engine.set_cursor(offset);
    }
    fn prepare_wrap(&mut self, window: &mut Window) {
        if self.wraps() {
            let mut wrapper = window.text_system().line_wrapper(
                font(self.appearance.font_family.clone()),
                px(self.appearance.font_size),
            );
            let width = self.viewport_width.max(1.);
            self.wrapped.refresh(
                &self.engine.buffer,
                self.engine.revision(),
                width,
                self.style_revision,
                |text| {
                    wrapper
                        .wrap_line(&[LineFragment::text(text)], px(width))
                        .map(|boundary| boundary.ix)
                        .collect()
                },
            );
            self.horizontal_column = 0;
        }
    }
    fn record_input(&mut self, kind: InputKind, started: Instant) {
        if let Some(tracker) = self.telemetry.as_mut() {
            tracker.input(kind, started);
        }
    }
    fn trace_scroll(&mut self, _: &ScrollWheelEvent, _: &mut Window, _: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        self.record_input(InputKind::Scroll, Instant::now());
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        self.presentation_holds_scroll = false;
        let input_started = Instant::now();
        self.prepare_wrap(window);
        let k = &event.keystroke;
        // Composition owns every key, including Escape/Enter, until the
        // platform commits or clears its marked range. Never save or switch
        // Vim mode using an intermediate composition string.
        if self.marked.is_some() {
            return;
        }
        if !self.compact && self.engine.mode == Mode::Insert && k.key == "enter" {
            let before = self.engine.revision();
            let selection = self.selected();
            replace_selection_with_newline(&mut self.engine, selection);
            self.selection = None;
            self.record_input(InputKind::Key, input_started);
            self.changed(before, cx);
            cx.stop_propagation();
            return;
        }
        let command = k.modifiers.platform || (cfg!(target_os = "linux") && k.modifiers.control);
        let before = self.engine.revision();
        if self.compact && k.key == "enter" {
            cx.stop_propagation();
            return;
        }
        if command {
            match k.key.as_str() {
                "s" => {
                    cx.emit(EditorEvent::SaveRequested);
                    cx.stop_propagation();
                    return;
                }
                "a" => {
                    let _ = self.select_all(cx);
                    cx.stop_propagation();
                    return;
                }
                "c" | "x" => {
                    let r = self.selected();
                    if !r.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            self.text()[r.clone()].into(),
                        ));
                        if k.key == "x" {
                            self.engine.replace_range(r, "");
                            self.selection = None;
                        }
                    }
                    self.record_input(InputKind::Key, input_started);
                    self.changed(before, cx);
                    cx.stop_propagation();
                    return;
                }
                "v" => {
                    if let Some(s) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        self.replace_text_in_range(None, &s, window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                "z" => {
                    if k.modifiers.shift {
                        self.engine.redo()
                    } else {
                        self.engine.undo()
                    };
                    self.selection = None;
                    self.record_input(InputKind::Key, input_started);
                    self.changed(before, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        let key = match k.key.as_str() {
            "escape" => Some(Key::Escape),
            "enter" => Some(Key::Enter),
            "backspace" => Some(Key::Backspace),
            "delete" => Some(Key::Delete),
            "left" => Some(Key::Left),
            "right" => Some(Key::Right),
            "up" => Some(Key::Up),
            "down" => Some(Key::Down),
            "tab" if self.engine.mode == Mode::Insert => {
                self.replace_text_in_range(None, "    ", window, cx);
                cx.stop_propagation();
                return;
            }
            "r" if k.modifiers.control => Some(Key::Ctrl('r')),
            _ => None,
        };
        if let Some(key) = key {
            let old = self.engine.cursor;
            if matches!(key, Key::Backspace | Key::Delete)
                && self.selection.as_ref().is_some_and(|r| !r.is_empty())
            {
                let r = self.selection.take().unwrap();
                self.engine.replace_range(r, "");
            } else if !self.engine.vim && matches!(key, Key::Left | Key::Right) {
                self.selection = self.engine.move_horizontal(
                    key == Key::Right,
                    k.modifiers.shift,
                    self.selection.take(),
                );
            } else {
                if self.wraps()
                    && self.engine.mode == Mode::Insert
                    && matches!(key, Key::Up | Key::Down)
                {
                    self.move_visual_row(key == Key::Down);
                } else {
                    self.engine.key(key);
                }
                if k.modifiers.shift
                    && !self.engine.vim
                    && matches!(key, Key::Left | Key::Right | Key::Up | Key::Down)
                {
                    let anchor = self
                        .selection
                        .as_ref()
                        .map(|r| if old == r.start { r.end } else { r.start })
                        .unwrap_or(old);
                    self.selection =
                        Some(anchor.min(self.engine.cursor)..anchor.max(self.engine.cursor));
                } else {
                    self.selection = None;
                }
            }
            self.marked = None;
            self.record_input(InputKind::Key, input_started);
            self.changed(before, cx);
            cx.stop_propagation();
            return;
        }
        if self.engine.mode != Mode::Insert && !k.modifiers.control && !k.modifiers.platform {
            if let Some(text) = k.key_char.as_ref().or(Some(&k.key)) {
                for c in text.chars() {
                    self.engine.key(Key::Char(c));
                }
            }
            self.selection = None;
            self.record_input(InputKind::Key, input_started);
            self.changed(before, cx);
            cx.stop_propagation();
        }
    }
    fn mouse_index(&self, p: Point<Pixels>) -> Option<usize> {
        let (_, (line, bounds, base)) = self
            .layouts
            .iter()
            .find(|(_, (_, b, _))| p.y >= b.top() && p.y < b.bottom())?;
        Some(*base + line.closest_index_for_x(p.x - bounds.left()))
    }
    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.abandon_presentation_navigation();
        self.presentation_holds_scroll = false;
        self.focus.focus(window);
        if let Some(p) = self.mouse_index(e.position) {
            let old = self.engine.cursor;
            self.engine.set_cursor(p);
            if e.modifiers.shift {
                self.selection = Some(old.min(p)..old.max(p));
                self.dragging = Some(old);
            } else {
                self.selection = None;
                self.dragging = Some(p);
            }
            cx.notify();
        }
    }
    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(anchor) = self.dragging
            && let Some(p) = self.mouse_index(e.position)
        {
            self.engine.set_cursor(p);
            self.selection = Some(anchor.min(p)..anchor.max(p));
            cx.notify();
        }
    }
}
impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = self.byte_range_for_utf16(r);
        *actual = Some(self.to_utf16(r.clone()));
        Some(self.text()[r].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.to_utf16(self.selected()),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|r| self.to_utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_started = Instant::now();
        if self.engine.mode != Mode::Insert {
            return;
        }
        let before = self.engine.revision();
        let r = r.map(|r| self.byte_range_for_utf16(r));
        replace_input_text(
            &mut self.engine,
            &mut self.marked,
            &mut self.selection,
            r,
            text,
        );
        self.record_input(InputKind::Text, input_started);
        self.changed(before, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_started = Instant::now();
        if self.engine.mode != Mode::Insert {
            return;
        }
        let before = self.engine.revision();
        let r = r
            .map(|r| self.byte_range_for_utf16(r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selected());
        let start = r.start;
        if self.engine.replace_range_grouped(r, text) {
            self.marked = (!text.is_empty()).then_some(start..start + text.len());
            if let Some(selected) = selected {
                let b = TextBuffer::new(text.into());
                let r = start + b.byte_offset(selected.start)..start + b.byte_offset(selected.end);
                self.engine.cursor = r.end;
                self.selection = Some(r);
            } else {
                self.selection = None;
            }
        }
        self.record_input(InputKind::Composition, input_started);
        self.changed(before, cx);
    }
    fn bounds_for_range(
        &mut self,
        r: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let r = self.byte_range_for_utf16(r);
        let row = self.display_row_at(r.start);
        let (line, b, start) = self.layouts.get(&row)?;
        Some(Bounds::new(
            point(
                b.left() + line.x_for_index(r.start.saturating_sub(*start).min(line.len)),
                b.top(),
            ),
            size(px(2.), px(self.appearance.line_height)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.mouse_index(p)
            .map(|b| self.engine.buffer.utf16_offset(b))
    }
}
struct LineElement {
    editor: Entity<EditorView>,
    row: usize,
}
struct LinePaint {
    line: ShapedLine,
    selection: Option<PaintQuad>,
    cursor: Option<PaintQuad>,
    byte_start: usize,
    decorations: Vec<PaintQuad>,
    measured_fragment: Option<PaintedFragment>,
    layout_generation: u64,
}
impl IntoElement for LineElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for LineElement {
    type RequestLayoutState = ();
    type PrepaintState = LinePaint;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(self.editor.read(cx).appearance.line_height).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> LinePaint {
        let ed = self.editor.read(cx);
        let display = ed.display_row(self.row);
        let raw = &ed.text()[display.range.clone()];
        let skipped = if ed.wraps() {
            0
        } else {
            raw.char_indices()
                .nth(ed.horizontal_column)
                .map(|(i, _)| i)
                .unwrap_or(raw.len())
        };
        let raw = &raw[skipped..];
        let end = raw
            .char_indices()
            .nth(if ed.wraps() {
                MAX_DRAW_LINE
            } else {
                ed.draw_columns.min(MAX_DRAW_LINE)
            })
            .map(|(i, _)| i)
            .unwrap_or(raw.len());
        let text: SharedString = raw[..end].to_string().into();
        let style = window.text_style();
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: ed.appearance.text,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(text, px(ed.appearance.font_size), &[run], None);
        let start = display.range.start + skipped;
        let end = start + end;
        let drawn = start..end;
        let mut decorations = Vec::new();
        if let Some(presentation) = ed.paint_presentation() {
            let first = presentation
                .decorations
                .partition_point(|value| value.range.end <= start);
            for decoration in presentation.decorations[first..]
                .iter()
                .take_while(|value| value.range.start < end)
                .chain(presentation.emphasized.iter())
            {
                if let Some(range) = intersection(&decoration.range, &drawn) {
                    decorations.push(fill(
                        Bounds::from_corners(
                            point(
                                bounds.left() + line.x_for_index(range.start - start),
                                bounds.top(),
                            ),
                            point(
                                bounds.left() + line.x_for_index(range.end - start),
                                bounds.bottom(),
                            ),
                        ),
                        decoration.color,
                    ));
                }
            }
        }
        let measured_fragment = ed
            .presentation_measurement
            .as_ref()
            .and_then(|measurement| {
                if ed.paint_presentation()?.token != measurement.token {
                    return None;
                }
                let range = intersection(&measurement.range, &drawn)?;
                let original = Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(range.start - start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(range.end - start),
                        bounds.bottom(),
                    ),
                );
                let clipped = original
                    .intersect(&bounds)
                    .intersect(&window.content_mask().bounds);
                (clipped.size.width > px(0.) && clipped.size.height > px(0.)).then_some(
                    PaintedFragment {
                        range,
                        bounds: clipped,
                        unclipped: original == clipped,
                    },
                )
            });
        let selected = ed.selected();
        let a = selected.start.max(start).min(end) - start;
        let b = selected.end.max(start).min(end) - start;
        let selection = (b > a).then(|| {
            fill(
                Bounds::from_corners(
                    point(bounds.left() + line.x_for_index(a), bounds.top()),
                    point(bounds.left() + line.x_for_index(b), bounds.bottom()),
                ),
                ed.appearance.selection,
            )
        });
        let cursor = if ed.display_row_at(ed.engine.cursor) == self.row {
            let offset = ed.engine.cursor.saturating_sub(start).min(line.len);
            let x = line.x_for_index(offset);
            let width = if ed.engine.mode == Mode::Insert {
                px(2.)
            } else {
                let next = ed
                    .engine
                    .buffer
                    .next(ed.engine.cursor)
                    .saturating_sub(start)
                    .min(line.len);
                (line.x_for_index(next) - x).max(px(8.))
            };
            Some(fill(
                Bounds::new(
                    point(bounds.left() + x, bounds.top()),
                    size(width, px(ed.appearance.line_height)),
                ),
                if ed.engine.mode == Mode::Insert {
                    ed.appearance.caret
                } else {
                    ed.appearance.normal_caret
                },
            ))
        } else {
            None
        };
        LinePaint {
            line,
            selection,
            cursor,
            byte_start: start,
            decorations,
            measured_fragment,
            layout_generation: ed.layout_generation,
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut LinePaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        if self
            .editor
            .read(cx)
            .display_row_at(self.editor.read(cx).engine.cursor)
            == self.row
        {
            window.handle_input(
                &focus,
                ElementInputHandler::new(bounds, self.editor.clone()),
                cx,
            );
        }
        if state.layout_generation == self.editor.read(cx).layout_generation
            && self.editor.read(cx).paint_presentation().is_some()
        {
            for decoration in state.decorations.drain(..) {
                window.paint_quad(decoration);
            }
        }
        if let Some(s) = state.selection.take() {
            window.paint_quad(s);
        }
        if let Err(error) = state.line.paint(
            bounds.origin,
            px(self.editor.read(cx).appearance.line_height),
            window,
            cx,
        ) {
            eprintln!("Editor text paint failed: {error}");
            // A shaped/prepainted fragment is not evidence that its text painted.
            state.measured_fragment = None;
        }
        if focus.is_focused(window)
            && let Some(c) = state.cursor.take()
        {
            window.paint_quad(c);
        }
        let batch = self.editor.update(cx, |ed, _| {
            let cursor_row = ed.display_row_at(ed.engine.cursor) == self.row;
            ed.telemetry
                .as_mut()
                .and_then(|tracker| tracker.take_for_paint(cursor_row))
        });
        if let Some(batch) = batch {
            batch.paint();
            window.on_next_frame(move |_, _| batch.following_frame());
        }
        if let Some(fragment) = state.measured_fragment.as_mut() {
            let clipped = fragment
                .bounds
                .intersect(&bounds)
                .intersect(&window.content_mask().bounds);
            fragment.unclipped &= clipped == fragment.bounds;
            fragment.bounds = clipped;
            if clipped.size.width <= px(0.) || clipped.size.height <= px(0.) {
                state.measured_fragment = None;
            }
        }
        let schedule = self.editor.update(cx, |ed, _| {
            if state.layout_generation != ed.layout_generation {
                return None;
            }
            if let Some(fragment) = state.measured_fragment.take() {
                ed.painted_fragments.insert(self.row, fragment);
            }
            let measurement = ed.presentation_measurement.as_ref()?;
            if ed.measurement_frame_scheduled == Some(ed.layout_generation) {
                return None;
            }
            ed.measurement_frame_scheduled = Some(ed.layout_generation);
            Some((
                ed.layout_generation,
                measurement.serial,
                ed.scroll.0.borrow().base_handle.offset(),
                window.bounds(),
                window.window_handle().window_id(),
            ))
        });
        if let Some((generation, serial, scroll_offset, window_bounds, window_id)) = schedule {
            let editor = self.editor.downgrade();
            window.defer(cx, move |window, cx| {
                let Some(editor) = editor.upgrade() else {
                    return;
                };
                let receipt = editor.update(cx, |ed, _| {
                    if generation != ed.layout_generation
                        || ed.scroll.0.borrow().base_handle.offset() != scroll_offset
                        || window.bounds() != window_bounds
                        || window.window_handle().window_id() != window_id
                    {
                        return None;
                    }
                    let request = ed.presentation_measurement.as_ref()?;
                    if request.serial != serial || ed.checked_presentation(request.token).is_err() {
                        return None;
                    }
                    let first = ed.painted_fragments.values().next()?;
                    let first_visible_fragment = first.bounds;
                    let mut covered = request.range.start;
                    let mut fully_visible = true;
                    for fragment in ed.painted_fragments.values() {
                        if !fragment.unclipped {
                            fully_visible = false;
                        }
                        if fragment.range.start > covered
                            && !ed.text()[covered..fragment.range.start]
                                .bytes()
                                .all(|b| b == b'\r' || b == b'\n')
                        {
                            fully_visible = false;
                        }
                        covered = covered.max(fragment.range.end);
                    }
                    fully_visible &= covered == request.range.end;
                    let request = ed.presentation_measurement.take()?;
                    Some((
                        request.callback,
                        PresentationGeometry {
                            token: request.token,
                            host_generation: request.host_generation,
                            layout_generation: generation,
                            window_id,
                            first_visible_fragment,
                            fully_visible,
                        },
                    ))
                });
                if let Some((callback, geometry)) = receipt {
                    callback(geometry, window, cx);
                }
            });
        }
        self.editor.update(cx, |ed, cx| {
            let width = f32::from(bounds.size.width).max(1.);
            ed.measured_viewport_width = Some(width);
            if ed.wraps() && (ed.viewport_width - width).abs() > 1. {
                ed.viewport_width = width;
                ed.invalidate_presentation_layout();
                cx.notify();
            }
            ed.layouts
                .insert(self.row, (state.line.clone(), bounds, state.byte_start));
        });
    }
}
impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self
            .presentation
            .as_ref()
            .is_some_and(|value| value.text.as_ref() != self.text())
        {
            self.discard_presentation();
            // Public engine replacement can reset its revision: do not reuse an
            // old wrapping map merely because the new engine has the same number.
            self.wrapped.clear();
        }
        self.validated_paint_text = self
            .presentation
            .as_ref()
            .map(|_| self.paint_text_identity());
        self.invalidate_presentation_layout();
        let previous_rows = self.display_count();
        self.prepare_wrap(window);
        if previous_rows != self.display_count() {
            cx.emit(EditorEvent::LayoutChanged);
        }
        if let Some(range) = self.presentation_reveal.take()
            && self.marked.is_none()
            && self.dragging.is_none()
            && self.presentation.is_some()
        {
            self.scroll
                .scroll_to_item(self.display_row_at(range.start), ScrollStrategy::Center);
            if !self.wraps() {
                self.horizontal_column =
                    self.engine.buffer.position(range.start).1.saturating_sub(4);
            }
        }
        if self.reveal_cursor {
            self.scroll
                .scroll_to_item(self.display_row_at(self.engine.cursor), ScrollStrategy::Top);
            self.reveal_cursor = false;
        }
        let width = self
            .layouts
            .values()
            .next()
            .map(|(_, b, _)| f32::from(b.size.width))
            .unwrap_or(640.);
        let visible_columns = ((width / 8.).floor() as usize).saturating_sub(3).max(4);
        let cursor_column = self.engine.buffer.position(self.engine.cursor).1;
        if self.wraps() {
            self.horizontal_column = 0;
        } else if !self.presentation_holds_scroll && cursor_column < self.horizontal_column {
            self.horizontal_column = cursor_column;
        } else if !self.presentation_holds_scroll
            && cursor_column >= self.horizontal_column + visible_columns
        {
            self.horizontal_column = cursor_column.saturating_sub(visible_columns - 1);
        }
        self.draw_columns = (visible_columns + 16).min(MAX_DRAW_LINE);
        self.layouts.clear();
        let mode = if !self.engine.vim {
            "TEXT"
        } else {
            match self.engine.mode {
                Mode::Normal => "NORMAL",
                Mode::Insert => "INSERT",
                Mode::Visual => "VISUAL",
                Mode::VisualLine => "VISUAL LINE",
                Mode::Search => "SEARCH",
            }
        };
        let (row, col) = self.engine.buffer.position(self.engine.cursor);
        let status = if self.engine.mode == Mode::Search {
            format!("/{}", self.engine.search)
        } else {
            format!(
                "{}    {}:{}    {}",
                mode,
                row + 1,
                col + 1,
                self.engine.message
            )
        };
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .when_some(self.appearance.background, |d, bg| d.bg(bg))
            .text_color(self.appearance.text)
            .font_family(self.appearance.font_family.clone())
            .text_size(px(self.appearance.font_size))
            .px(px(self.appearance.padding_x))
            .py(px(self.appearance.padding_y))
            .key_context("BelloEditor")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_scroll_wheel(cx.listener(Self::trace_scroll))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = None),
            )
            .child(
                uniform_list(
                    "editor-lines",
                    self.display_count(),
                    cx.processor(|this, range: Range<usize>, _, cx| {
                        range
                            .map(|row| {
                                let active = this.display_row_at(this.engine.cursor) == row;
                                let display = this.display_row(row);
                                div()
                                    .id(row)
                                    .w_full()
                                    .flex()
                                    .h(px(this.appearance.line_height))
                                    .when(active && this.appearance.active_line.is_some(), |d| {
                                        d.bg(this.appearance.active_line.unwrap())
                                    })
                                    .when(!this.compact && this.appearance.show_gutter, |d| {
                                        d.child(
                                            div()
                                                .w(px(this.appearance.gutter_width))
                                                .flex_shrink_0()
                                                .pr_3()
                                                .text_right()
                                                .text_color(this.appearance.line_number)
                                                .text_size(px(this.appearance.gutter_font_size))
                                                .child(if display.first {
                                                    (display.source_row + 1).to_string()
                                                } else {
                                                    String::new()
                                                }),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .h(px(this.appearance.line_height))
                                            .overflow_hidden()
                                            .child(LineElement {
                                                editor: cx.entity(),
                                                row,
                                            }),
                                    )
                            })
                            .collect()
                    }),
                )
                .track_scroll(self.scroll.clone())
                .flex_1()
                .min_h_0(),
            )
            .when(!self.compact && self.appearance.show_status, |d| {
                d.child(
                    div()
                        .h(px(26.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .bg(self.appearance.status_background)
                        .text_color(self.appearance.status_text)
                        .text_size(px(11.))
                        .child(status)
                        .when(self.appearance.show_vim_toggle, |d| {
                            d.child(
                                div()
                                    .id("vim-toggle")
                                    .cursor_pointer()
                                    .text_color(self.appearance.accent)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_vim(!this.engine.vim, cx)
                                    }))
                                    .child(if self.engine.vim {
                                        "Vim subset: on"
                                    } else {
                                        "Vim: off"
                                    }),
                            )
                        }),
                )
            })
    }
}

fn measurement_width(proposed: f32, observed: Option<f32>) -> f32 {
    observed
        .filter(|width| width.is_finite() && *width > 0.)
        .unwrap_or(proposed)
        .max(1.)
}
fn replace_selection_with_newline(editor: &mut Editor, selection: Range<usize>) {
    let newline = editor.buffer.newline();
    editor.replace_range_grouped(selection, newline);
}
/// Mode changes must not discard a platform-owned composition. The user can
/// request the toggle again after the input method commits or unmarks it.
fn set_vim_unless_marked(
    editor: &mut Editor,
    marked: &Option<Range<usize>>,
    selection: &mut Option<Range<usize>>,
    enabled: bool,
) -> bool {
    if marked.is_some() {
        return false;
    }
    editor.set_vim(enabled);
    *selection = None;
    true
}
/// Byte ranges have already been converted from the platform's UTF-16 ranges.
/// Keep composition state until the engine accepts the replacement, including
/// when a size limit or read-only transition rejects a final IME commit.
fn replace_input_text(
    editor: &mut Editor,
    marked: &mut Option<Range<usize>>,
    selection: &mut Option<Range<usize>>,
    range: Option<Range<usize>>,
    text: &str,
) -> bool {
    if editor.mode != Mode::Insert {
        return false;
    }
    let range = range
        .or_else(|| marked.clone())
        .or_else(|| selection.clone())
        .or_else(|| editor.selection())
        .unwrap_or(editor.cursor..editor.cursor);
    if !editor.replace_range_grouped(range, text) {
        return false;
    }
    *marked = None;
    *selection = None;
    true
}
#[cfg(test)]
mod input_tests {
    use super::{
        measurement_width, replace_input_text, replace_selection_with_newline,
        set_vim_unless_marked,
    };
    use bello_workbench::editor::{Editor, Key, MAX_EDIT_BYTES, Mode};

    #[test]
    fn retained_state_keeps_pending_group_unicode_crlf_and_selection() {
        let mut editor = Editor::new("😀\r\né".into());
        editor.set_cursor(editor.text().len());
        editor.insert_text("甲");
        let cursor = editor.cursor;
        let mut selection = Some(0..4);
        let pointer = editor.text().as_ptr();
        let mut state =
            Some(super::take_edit_state(&mut editor, &mut selection, false, &None, &None).unwrap());
        assert_eq!(editor.text(), "");
        assert!(editor.read_only);
        editor.insert_text("late callback");
        assert_eq!(editor.text(), "");
        assert_eq!(selection, None);
        let mut target = Editor::new("😀\r\né甲".into());
        super::restore_edit_state(&mut target, &mut selection, &mut state, false, &None, &None)
            .unwrap();
        assert!(state.is_none());
        assert_eq!(target.text().as_ptr(), pointer);
        assert_eq!(target.cursor, cursor);
        assert_eq!(selection, Some(0..4));
        target.insert_text("乙");
        target.undo();
        assert_eq!(target.text(), "😀\r\né");
        target.redo();
        assert_eq!(target.text(), "😀\r\né甲乙");
        target.undo();
        let mut state =
            Some(super::take_edit_state(&mut target, &mut selection, false, &None, &None).unwrap());
        let mut target = Editor::new("😀\r\né".into());
        super::restore_edit_state(&mut target, &mut selection, &mut state, false, &None, &None)
            .unwrap();
        target.redo();
        assert_eq!(target.text(), "😀\r\né甲乙");
    }

    #[test]
    fn rejected_state_transfers_leave_both_owners_intact() {
        use super::EditStateError::*;
        for (focused, marked, dragging, error) in [
            (true, None, None, Focused),
            (false, Some(0..1), None, Composing),
            (false, None, Some(0), Dragging),
        ] {
            let mut editor = Editor::new("a".into());
            editor.set_cursor(1);
            editor.insert_text("b");
            let mut selection = Some(0..1);
            let pointer = editor.text().as_ptr();
            assert_eq!(
                super::take_edit_state(&mut editor, &mut selection, focused, &marked, &dragging)
                    .err(),
                Some(error)
            );
            assert_eq!(editor.text().as_ptr(), pointer);
            assert_eq!(selection, Some(0..1));
            let mut state = Some(
                super::take_edit_state(&mut editor, &mut selection, false, &None, &None).unwrap(),
            );
            let mut target = Editor::new("ab".into());
            let mut target_selection = Some(1..2);
            assert_eq!(
                super::restore_edit_state(
                    &mut target,
                    &mut target_selection,
                    &mut state,
                    focused,
                    &marked,
                    &dragging
                ),
                Err(error)
            );
            assert_eq!(target.text(), "ab");
            assert_eq!(target_selection, Some(1..2));
            assert_eq!(state.as_ref().unwrap().engine.text().as_ptr(), pointer);
            super::restore_edit_state(
                &mut target,
                &mut target_selection,
                &mut state,
                false,
                &None,
                &None,
            )
            .unwrap();
            target.insert_text("c");
            target.undo();
            assert_eq!(target.text(), "a");
        }
        for error in [StaleText, EditedDestination, IncompatibleConfiguration] {
            let mut original = Editor::new("saved".into());
            let mut selection = Some(0..2);
            let mut state = Some(
                super::take_edit_state(&mut original, &mut selection, false, &None, &None).unwrap(),
            );
            let mut target = Editor::new("saved".into());
            match error {
                StaleText => target = Editor::new("newer".into()),
                EditedDestination => {
                    target.insert_text("x");
                    target.undo();
                }
                IncompatibleConfiguration => target.set_vim(true),
                _ => unreachable!(),
            }
            let before = target.text().to_owned();
            let revision = target.revision();
            let bytes = state.as_ref().unwrap().accounted_bytes();
            assert_eq!(
                super::restore_edit_state(
                    &mut target,
                    &mut selection,
                    &mut state,
                    false,
                    &None,
                    &None
                ),
                Err(error)
            );
            assert_eq!(target.text(), before);
            assert_eq!(target.revision(), revision);
            assert_eq!(state.as_ref().unwrap().accounted_bytes(), bytes);
            assert_eq!(state.as_ref().unwrap().selection, Some(0..2));
        }
    }

    #[test]
    fn retained_vim_register_and_pending_operator_survive() {
        for pending_delete in [false, true] {
            let mut editor = Editor::new("one two".into());
            editor.set_vim(true);
            editor.key(Key::Char('y'));
            editor.key(Key::Char('w'));
            if pending_delete {
                editor.key(Key::Char('d'));
            }
            let mut selection = None;
            let mut state = Some(
                super::take_edit_state(&mut editor, &mut selection, false, &None, &None).unwrap(),
            );
            let mut target = Editor::new("one two".into());
            target.set_vim(true);
            super::restore_edit_state(&mut target, &mut selection, &mut state, false, &None, &None)
                .unwrap();
            if pending_delete {
                target.key(Key::Char('w'));
                assert_eq!(target.text(), "two");
            }
            target.key(Key::Char('P'));
            assert_eq!(
                target.text(),
                if pending_delete {
                    "one two"
                } else {
                    "one one two"
                }
            );
        }
    }

    #[test]
    fn composition_owns_mode_until_commit_or_unmark() {
        for vim in [false, true] {
            let mut editor = Editor::new("😀に尾".into());
            if vim {
                editor.set_vim(true);
                editor.key(Key::Char('i'));
            }
            editor.set_cursor(7);
            let mut marked = Some(4..7);
            let mut selection = Some(7..7);
            for enabled in [!vim, vim] {
                assert!(!set_vim_unless_marked(
                    &mut editor,
                    &marked,
                    &mut selection,
                    enabled
                ));
                assert_eq!(editor.vim, vim);
                assert_eq!(editor.mode, Mode::Insert);
                assert_eq!(editor.cursor, 7);
                assert_eq!(marked, Some(4..7));
                assert_eq!(selection, Some(7..7));
            }
            assert!(replace_input_text(
                &mut editor,
                &mut marked,
                &mut selection,
                None,
                "日本"
            ));
            assert_eq!(editor.text(), "😀日本尾");
            assert_eq!(editor.cursor, 10);
            assert_eq!(marked, None);
            assert_eq!(selection, None);
            assert!(set_vim_unless_marked(
                &mut editor,
                &marked,
                &mut selection,
                !vim
            ));
            assert_eq!(editor.vim, !vim);
        }
        let mut editor = Editor::new("に".into());
        let mut selection = Some(0..3);
        // unmark_text clears only the marked range; the next toggle works.
        assert!(set_vim_unless_marked(
            &mut editor,
            &None,
            &mut selection,
            true
        ));
        assert_eq!(editor.mode, Mode::Normal);
        assert_eq!(selection, None);
    }

    #[test]
    fn rejected_commit_keeps_composition_selection_and_caret_for_retry() {
        for explicit in [false, true] {
            let mut editor = Editor::new("😀に尾".into());
            editor.set_cursor(7);
            editor.read_only = true;
            let mut marked = Some(4..7);
            let mut selection = Some(4..7);
            let range = explicit.then_some(4..7);
            let revision = editor.revision();
            assert!(!replace_input_text(
                &mut editor,
                &mut marked,
                &mut selection,
                range.clone(),
                "日本"
            ));
            assert_eq!(editor.text(), "😀に尾");
            assert_eq!(editor.revision(), revision);
            assert_eq!(editor.cursor, 7);
            assert_eq!(marked, Some(4..7));
            assert_eq!(selection, Some(4..7));
            editor.read_only = false;
            assert!(replace_input_text(
                &mut editor,
                &mut marked,
                &mut selection,
                range,
                "日本"
            ));
            assert_eq!(editor.text(), "😀日本尾");
            assert_eq!(marked, None);
            assert_eq!(selection, None);
            editor.undo();
            assert_eq!(editor.text(), "😀に尾");
            assert_eq!(editor.cursor, 7);
            editor.redo();
            assert_eq!(editor.text(), "😀日本尾");
        }
    }

    #[test]
    fn oversized_commit_preserves_mark_and_retry_replaces_it_once() {
        let mut editor = Editor::new("に".into());
        editor.set_cursor(3);
        let mut marked = Some(0..3);
        let mut selection = Some(3..3);
        assert!(!replace_input_text(
            &mut editor,
            &mut marked,
            &mut selection,
            None,
            &"x".repeat(MAX_EDIT_BYTES + 1)
        ));
        assert_eq!(editor.text(), "に");
        assert_eq!(editor.cursor, 3);
        assert_eq!(marked, Some(0..3));
        assert_eq!(selection, Some(3..3));
        assert!(replace_input_text(
            &mut editor,
            &mut marked,
            &mut selection,
            None,
            "日本"
        ));
        assert_eq!(editor.text(), "日本");
        assert_eq!(marked, None);
    }

    #[test]
    fn explicit_platform_range_wins_and_clears_mark_only_on_success() {
        let mut editor = Editor::new("😀に尾".into());
        editor.set_cursor(7);
        let mut marked = Some(4..7);
        let mut selection = Some(7..7);
        // UTF-16 3..4 addresses 尾 after the surrogate pair and に.
        let range = editor.buffer.byte_offset(3)..editor.buffer.byte_offset(4);
        assert!(replace_input_text(
            &mut editor,
            &mut marked,
            &mut selection,
            Some(range),
            "終"
        ));
        assert_eq!(editor.text(), "😀に終");
        assert_eq!(marked, None);
        assert_eq!(selection, None);
    }

    #[test]
    fn rejected_plain_selection_replacement_keeps_selection() {
        let mut editor = Editor::new("selected".into());
        editor.set_cursor(8);
        editor.read_only = true;
        let mut marked = None;
        let mut selection = Some(0..8);
        assert!(!replace_input_text(
            &mut editor,
            &mut marked,
            &mut selection,
            None,
            "replacement"
        ));
        assert_eq!(editor.cursor, 8);
        assert_eq!(selection, Some(0..8));
    }

    #[test]
    fn empty_commit_removes_only_marked_text_and_unblocks_mode_change() {
        let mut editor = Editor::new("😀に尾".into());
        editor.set_cursor(7);
        let mut marked = Some(4..7);
        let mut selection = Some(7..7);
        assert!(replace_input_text(
            &mut editor,
            &mut marked,
            &mut selection,
            None,
            ""
        ));
        assert_eq!(editor.text(), "😀尾");
        assert_eq!(editor.cursor, 4);
        assert_eq!(marked, None);
        assert_eq!(selection, None);
        assert!(set_vim_unless_marked(
            &mut editor,
            &marked,
            &mut selection,
            true
        ));
        editor.undo();
        assert_eq!(editor.text(), "😀に尾");
    }
    #[test]
    fn painted_width_wins_over_parent_estimate_without_reflow_ping_pong() {
        assert_eq!(measurement_width(500., None), 500.);
        for _ in 0..100 {
            assert_eq!(measurement_width(500., Some(420.)), 420.);
        }
        assert_eq!(measurement_width(500., Some(360.)), 360.);
    }
    #[test]
    fn enter_replaces_selection_instead_of_appending() {
        let mut editor = Editor::new("before selected after".into());
        replace_selection_with_newline(&mut editor, 7..15);
        assert_eq!(editor.text(), "before \n after");
        assert_eq!(editor.cursor, 8);
        editor.undo();
        assert_eq!(editor.text(), "before selected after");
    }
    #[test]
    fn enter_preserves_crlf_and_read_only() {
        let mut editor = Editor::new("first\r\nsecond".into());
        replace_selection_with_newline(&mut editor, 7..13);
        assert_eq!(editor.text(), "first\r\n\r\n");
        editor.read_only = true;
        replace_selection_with_newline(&mut editor, 0..5);
        assert_eq!(editor.text(), "first\r\n\r\n");
    }
}

#[cfg(test)]
#[path = "editor_presentation_tests.rs"]
mod presentation_tests;
