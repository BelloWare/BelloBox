use crate::telemetry::{InputKind, Tracker};
use crate::{
    EditorAppearance,
    wrapping::{DisplayRow, WrappedRows},
};
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
        }
    }
    pub fn set_appearance(&mut self, mut appearance: EditorAppearance, cx: &mut Context<Self>) {
        appearance.validate();
        self.appearance = appearance;
        self.measured_viewport_width = None;
        self.style_revision = self.style_revision.wrapping_add(1);
        self.wrapped.clear();
        self.reveal_cursor = true;
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
        self.compact = enabled;
        if enabled {
            self.engine.set_vim(false);
        }
        cx.notify();
    }
    /// Reveal an explicit 1-based line without replacing text or undo history.
    pub fn reveal_line(&mut self, line: usize, window: &mut Window, cx: &mut Context<Self>) {
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
        if self.engine.revision() != before {
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
        self.record_input(InputKind::Scroll, Instant::now());
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
                    self.selection = Some(0..self.text().len());
                    cx.notify();
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
        self.editor.update(cx, |ed, cx| {
            let width = f32::from(bounds.size.width).max(1.);
            ed.measured_viewport_width = Some(width);
            if ed.wraps() && (ed.viewport_width - width).abs() > 1. {
                ed.viewport_width = width;
                cx.notify();
            }
            ed.layouts
                .insert(self.row, (state.line.clone(), bounds, state.byte_start));
        });
    }
}
impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let previous_rows = self.display_count();
        self.prepare_wrap(window);
        if previous_rows != self.display_count() {
            cx.emit(EditorEvent::LayoutChanged);
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
        } else if cursor_column < self.horizontal_column {
            self.horizontal_column = cursor_column;
        } else if cursor_column >= self.horizontal_column + visible_columns {
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
