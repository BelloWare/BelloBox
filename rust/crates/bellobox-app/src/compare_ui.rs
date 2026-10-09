//! Independent full-window comparison. Drafts and the explicitly pinned snapshot
//! remain in memory only; no clipboard read occurs when this window is created.
use crate::comparison_session::ComparisonSession;
use bello_workbench_ui::{
    EditorAppearance, EditorEvent, EditorView, TextDecoration, TextPresentation,
};
use bellobox_core::developer::comparison::{Mode, RowKind};
use gpui::{
    App, Bounds, ClipboardItem, Context, Div, Entity, FocusHandle, Focusable, FontWeight, Global,
    KeyDownEvent, SharedString, Stateful, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use std::sync::Arc;
mod words;
use words::WordsView;

#[derive(Default)]
struct PinnedText(Option<Arc<str>>);
impl Global for PinnedText {}
type EditToken = (u64, u64, bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Search,
    New,
    Lines,
    Words,
    Json,
    Whitespace,
    Paste(usize),
    Clear(usize),
    Pin,
    UsePin,
    Previous,
    Next,
    Refresh,
    Cancel,
    Copy,
}
struct CompareWindow {
    inputs: [Entity<EditorView>; 2],
    output: Entity<EditorView>,
    words: Entity<WordsView>,
    session: Entity<ComparisonSession>,
    observed: [EditToken; 2],
    rejected: [bool; 2],
    displayed_revision: Option<u64>,
    displayed_result: bool,
    decorated_revision: Option<u64>,
    decorated_color: Option<gpui::Hsla>,
    notice: String,
    focus: FocusHandle,
    controls: Vec<(Control, FocusHandle)>,
    subscriptions: Vec<Subscription>,
}
fn editor(text: String, readonly: bool, window: &mut Window, cx: &mut App) -> Entity<EditorView> {
    let p = crate::theme::for_window(window);
    cx.new(|cx| {
        let mut e = EditorView::new(text, window, cx);
        assert!(e.set_edit_byte_limit(if readonly {
            4_000_000
        } else {
            bellobox_core::MAX_INPUT_BYTES
        }));
        e.set_compact(readonly, cx);
        e.set_vim(false, cx);
        e.set_read_only(readonly, cx);
        let mut appearance = EditorAppearance::plain();
        appearance.font_size = 12.;
        appearance.line_height = 18.;
        appearance.padding_x = 8.;
        appearance.padding_y = 8.;
        appearance.text = p.primary;
        appearance.caret = p.accent;
        appearance.selection = p.accent.opacity(0.18);
        e.set_appearance(appearance, cx);
        e
    })
}
fn card(view: Entity<EditorView>, height: f32, p: crate::theme::Palette) -> Div {
    div()
        .h(px(height))
        .min_w_0()
        .w_full()
        .border_1()
        .border_color(p.separator)
        .rounded(px(7.))
        .bg(p.well)
        .child(view)
}
impl CompareWindow {
    fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let admission = bellobox_core::validate_input(&input).err();
        let input = if admission.is_some() {
            String::new()
        } else {
            input
        };
        let inputs = [
            editor(input, false, window, cx),
            editor(String::new(), false, window, cx),
        ];
        let observed = inputs.each_ref().map(|e| e.read(cx).edit_token());
        let session = cx.new(ComparisonSession::new);
        let mut s = Self {
            inputs,
            output: editor(String::new(), true, window, cx),
            words: cx.new(WordsView::new),
            session,
            observed,
            rejected: [false; 2],
            displayed_revision: None,
            displayed_result: false,
            decorated_revision: None,
            decorated_color: None,
            notice: String::new(),
            focus: cx.focus_handle(),
            controls: [
                Control::Search,
                Control::New,
                Control::Lines,
                Control::Words,
                Control::Json,
                Control::Whitespace,
                Control::Paste(0),
                Control::Clear(0),
                Control::Pin,
                Control::Paste(1),
                Control::Clear(1),
                Control::UsePin,
                Control::Previous,
                Control::Next,
                Control::Refresh,
                Control::Cancel,
                Control::Copy,
            ]
            .into_iter()
            .map(|c| (c, cx.focus_handle()))
            .collect(),
            subscriptions: Vec::new(),
        };
        for e in s.inputs.clone() {
            s.subscriptions
                .push(cx.subscribe(&e, |s, _, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Changed) {
                        s.sync_inputs(false, cx);
                    }
                }));
        }
        s.subscriptions.push(cx.observe(&s.session, |s, _, cx| {
            // Editors can change before their queued Changed notification. Never
            // let a session completion reintroduce output for those old tokens.
            s.sync_inputs(false, cx);
            s.sync_output(cx);
            cx.notify();
        }));
        cx.on_release(|s: &mut Self, cx| s.session.update(cx, |s, cx| s.retire(cx)))
            .detach();
        s.sync_inputs(true, cx);
        if let Some(error) = admission {
            s.rejected[0] = true;
            s.session.update(cx, |s, cx| s.reject(error, cx));
        }
        s.inputs[0].read(cx).focus(window);
        s
    }
    fn tokens(&self, cx: &App) -> [EditToken; 2] {
        self.inputs.each_ref().map(|e| e.read(cx).edit_token())
    }
    fn composing(&self, cx: &App) -> bool {
        self.inputs.iter().any(|e| e.read(cx).has_marked_text())
    }
    fn ready(&self, cx: &App) -> bool {
        !self.composing(cx)
            && self.tokens(cx) == self.observed
            && !self.rejected.iter().any(|v| *v)
            && self.observed.iter().all(|t| t.2)
            && self.session.read(cx).can_copy()
    }
    fn clear_output(&mut self, cx: &mut Context<Self>) {
        self.displayed_revision = None;
        self.displayed_result = false;
        self.decorated_revision = None;
        self.output.update(cx, |e, cx| {
            if !e.text().is_empty() {
                e.set_text(String::new(), cx);
            }
            let _ = e.set_text_presentation(None, cx);
        });
        self.words.update(cx, |w, cx| w.clear(cx));
    }
    fn sync_inputs(&mut self, force: bool, cx: &mut Context<Self>) {
        let tokens = self.tokens(cx);
        if !force && tokens == self.observed {
            return;
        }
        self.clear_output(cx);
        self.notice.clear();
        for (i, token) in tokens.iter().enumerate() {
            if !token.2 || token.1 != self.observed[i].1 {
                self.rejected[i] = true;
            } else if token.0 != self.observed[i].0 {
                self.rejected[i] = false;
            }
        }
        self.observed = tokens;
        if self.rejected.iter().any(|v| *v) {
            self.session.update(cx, |s, cx| s.reject("An edit was rejected. Edit, Clear, or Paste that field to continue. No text was truncated.".into(), cx));
        } else {
            let bytes = self
                .inputs
                .iter()
                .map(|e| e.read(cx).text().len())
                .sum::<usize>();
            if bytes > bellobox_core::MAX_INPUT_BYTES {
                self.session.update(cx, |s, cx| s.reject("Compare up to 500000 combined UTF-8 input bytes. The drafts have not been truncated.".into(), cx));
            } else {
                let [a, b] = self.inputs.each_ref().map(|e| e.read(cx).text().to_owned());
                self.session.update(cx, |s, cx| s.set_drafts(a, b, cx));
            }
        }
        cx.notify();
    }
    fn sync_output(&mut self, cx: &mut Context<Self>) {
        if self.tokens(cx) != self.observed || self.rejected.iter().any(|r| *r) {
            self.clear_output(cx);
            return;
        }
        let session = self.session.read(cx);
        if self.displayed_revision == Some(session.revision)
            && self.displayed_result == session.result.is_some()
        {
            return;
        }
        let revision = session.revision;
        let has_result = session.result.is_some();
        let mode = session.options.mode;
        let output = session
            .result
            .as_ref()
            .map(|r| r.copy_text.clone())
            .unwrap_or_default();
        let rows = session
            .result
            .as_ref()
            .map(|r| r.rows.clone())
            .unwrap_or_default();
        self.output.update(cx, |e, cx| e.set_text(output, cx));
        self.words.update(cx, |w, cx| {
            w.set_rows(
                if mode == Mode::Words {
                    rows
                } else {
                    Vec::new()
                },
                cx,
            )
        });
        self.displayed_revision = Some(revision);
        self.displayed_result = has_result;
    }
    fn replace(&mut self, side: usize, text: String, cx: &mut Context<Self>) {
        if self.composing(cx) {
            self.notice = "Finish the current text composition first.".into();
            cx.notify();
            return;
        }
        self.clear_output(cx);
        if let Err(error) = bellobox_core::validate_input(&text) {
            self.rejected[side] = true;
            self.session.update(cx, |s, cx| s.reject(error, cx));
        } else {
            self.rejected[side] = false;
            self.inputs[side].update(cx, |e, cx| e.set_text(text, cx));
            self.sync_inputs(true, cx);
        }
        cx.notify();
    }
    fn copy(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let text = self
            .session
            .read(cx)
            .result
            .as_ref()
            .unwrap()
            .copy_text
            .clone();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.notice = "Complete result copied.".into();
        cx.notify();
    }
    fn enabled(&self, control: Control, cx: &App) -> bool {
        let s = self.session.read(cx);
        match control {
            Control::Whitespace => s.options.mode == Mode::Lines,
            Control::Copy => self.ready(cx),
            Control::Cancel => s.busy,
            Control::Refresh => !s.busy,
            Control::Previous => self.words.read(cx).has_previous(),
            Control::Next => self.words.read(cx).has_next(),
            Control::Pin => {
                !self.rejected[0] && self.tokens(cx)[0].2 && self.tokens(cx)[0] == self.observed[0]
            }
            _ => true,
        }
    }
    fn activate(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing(cx) || !self.enabled(control, cx) {
            return;
        }
        self.sync_inputs(false, cx);
        match control {
            Control::Search => crate::launcher_ui::open(String::new(), cx),
            Control::New => open(String::new(), cx),
            Control::Lines | Control::Words | Control::Json | Control::Whitespace => {
                let mut options = self.session.read(cx).options;
                match control {
                    Control::Lines => options.mode = Mode::Lines,
                    Control::Words => options.mode = Mode::Words,
                    Control::Json => options.mode = Mode::Json,
                    _ => options.ignore_whitespace = !options.ignore_whitespace,
                }
                self.clear_output(cx);
                self.notice.clear();
                self.session.update(cx, |s, cx| s.set_options(options, cx));
            }
            Control::Paste(side) => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.replace(side, text, cx);
                } else {
                    self.notice = "The clipboard has no text.".into();
                }
            }
            Control::Clear(side) => self.replace(side, String::new(), cx),
            Control::Pin => {
                let text = Arc::from(self.inputs[0].read(cx).text());
                cx.default_global::<PinnedText>().0 = Some(text);
                self.notice = "Pinned for comparison until Bello Box quits.".into();
            }
            Control::UsePin => {
                let pin = cx.default_global::<PinnedText>().0.clone();
                if let Some(text) = pin {
                    self.replace(1, text.to_string(), cx);
                } else {
                    self.notice =
                        "Pin a selection first, then open Compare with another selection.".into();
                }
            }
            Control::Previous => self.words.update(cx, |w, cx| w.previous(cx)),
            Control::Next => self.words.update(cx, |w, cx| w.next(cx)),
            Control::Refresh => {
                self.clear_output(cx);
                self.session.update(cx, |s, cx| s.refresh(cx));
            }
            Control::Cancel => {
                self.clear_output(cx);
                self.session.update(cx, |s, cx| s.cancel(cx));
            }
            Control::Copy => self.copy(cx),
        }
        let _ = window;
        cx.notify();
    }
    fn button(
        &self,
        control: Control,
        label: impl Into<SharedString>,
        p: crate::theme::Palette,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let index = self
            .controls
            .iter()
            .position(|(c, _)| *c == control)
            .unwrap();
        let handle = self.controls[index].1.clone();
        let enabled = self.enabled(control, cx);
        div()
            .id(("compare-control", index))
            .track_focus(&handle)
            .h(px(30.))
            .px(px(9.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .border_1()
            .border_color(p.separator)
            .rounded(px(6.))
            .bg(p.surface)
            .focus(|s| s.border_color(p.accent))
            .when(!enabled, |b| b.opacity(0.4))
            .when(enabled, |b| b.cursor_pointer())
            .child(label.into())
            .on_click(cx.listener(move |s, _, w, cx| {
                if s.enabled(control, cx) {
                    w.focus(&handle);
                    s.activate(control, w, cx);
                }
            }))
    }
    fn tab_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles = Vec::new();
        for (control, focus) in &self.controls {
            if self.enabled(*control, cx) {
                handles.push(focus.clone());
            }
            if *control == Control::Clear(0) {
                handles.push(self.inputs[0].read(cx).focus_handle(cx));
            }
            if *control == Control::Clear(1) {
                handles.push(self.inputs[1].read(cx).focus_handle(cx));
            }
            if *control == Control::UsePin && self.session.read(cx).result.is_some() {
                handles.push(if self.session.read(cx).options.mode == Mode::Words {
                    self.words.read(cx).focus_handle(cx)
                } else {
                    self.output.read(cx).focus_handle(cx)
                });
            }
        }
        handles
    }
    fn key(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing(cx) {
            return;
        }
        let command = e.keystroke.modifiers.platform
            || (cfg!(target_os = "linux") && e.keystroke.modifiers.control);
        if command {
            if e.is_held && matches!(e.keystroke.key.as_str(), "n" | "k" | "w") {
                cx.stop_propagation();
                return;
            }
            match e.keystroke.key.as_str() {
                "n" => self.activate(Control::New, w, cx),
                "k" => self.activate(Control::Search, w, cx),
                "w" => {
                    self.session.update(cx, |s, cx| s.retire(cx));
                    crate::shutdown::close_window(w, cx);
                }
                "c" if e.keystroke.modifiers.shift => self.copy(cx),
                "c" | "x"
                    if !self.ready(cx)
                        && (self.output.read(cx).focus_handle(cx).is_focused(w)
                            || self.words.read(cx).focus_handle(cx).is_focused(w)) => {}
                _ => return,
            }
        } else if e.keystroke.key == "tab" {
            let handles = self.tab_handles(cx);
            let current = handles.iter().position(|h| h.is_focused(w));
            let next = if e.keystroke.modifiers.shift {
                current
                    .unwrap_or(0)
                    .checked_sub(1)
                    .unwrap_or(handles.len() - 1)
            } else {
                current.map(|i| (i + 1) % handles.len()).unwrap_or(0)
            };
            w.focus(&handles[next]);
        } else if matches!(e.keystroke.key.as_str(), "enter" | "space") {
            let control = self
                .controls
                .iter()
                .find(|(_, h)| h.is_focused(w))
                .map(|(c, _)| *c);
            if let Some(control) = control {
                if !e.is_held {
                    self.activate(control, w, cx);
                }
            } else {
                return;
            }
        } else {
            return;
        }
        cx.stop_propagation();
    }
}
impl Render for CompareWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_inputs(false, cx);
        self.sync_output(cx);
        let p = crate::theme::for_window(window);
        for e in self.inputs.iter().chain(std::iter::once(&self.output)) {
            if e.read(cx).appearance().text != p.primary {
                e.update(cx, |e, cx| {
                    let mut a = e.appearance().clone();
                    a.text = p.primary;
                    a.caret = p.accent;
                    a.selection = p.accent.opacity(0.18);
                    e.set_appearance(a, cx);
                });
            }
        }
        let session = self.session.read(cx);
        let mode = session.options.mode;
        let ignore = session.options.ignore_whitespace;
        let error = session.error.clone();
        let status = session.status.clone();
        let busy = session.busy;
        let has_result = session.result.is_some();
        if (self.decorated_revision != Some(session.revision)
            || self.decorated_color != Some(p.primary))
            && let Some(result) = &session.result
        {
            let text: Arc<str> = Arc::from(result.copy_text.as_str());
            let mut offset = 0;
            let decorations = result
                .rows
                .iter()
                .filter_map(|row| {
                    let length = match row.kind {
                        RowKind::Removed => 4,
                        _ => 2,
                    } + row.text.len();
                    let range = offset..offset + length;
                    offset += length + 1;
                    match row.kind {
                        RowKind::Added => Some(TextDecoration {
                            range,
                            color: p.success.opacity(0.10),
                        }),
                        RowKind::Removed => Some(TextDecoration {
                            range,
                            color: p.danger.opacity(0.10),
                        }),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            let presentation = TextPresentation {
                token: session.revision,
                text,
                decorations: decorations.into(),
                emphasized: None,
            };
            self.decorated_revision = Some(session.revision);
            self.decorated_color = Some(p.primary);
            self.output.update(cx, |e, cx| {
                let _ = e.set_text_presentation(Some(presentation), cx);
            });
        }
        let mut tabs = div().flex().items_center().gap(px(7.));
        for (control, label, tab_mode) in [
            (Control::Lines, "Lines", Mode::Lines),
            (Control::Words, "Words", Mode::Words),
            (Control::Json, "JSON fields", Mode::Json),
        ] {
            tabs = tabs.child(
                self.button(control, label, p, cx)
                    .when(mode == tab_mode, |b| {
                        b.text_color(p.accent).border_color(p.accent)
                    }),
            );
        }
        tabs = tabs.child(div().flex_1()).child(self.button(
            Control::Whitespace,
            if ignore {
                "☑ Ignore whitespace"
            } else {
                "☐ Ignore whitespace"
            },
            p,
            cx,
        ));
        let mut inputs = div().flex().gap(px(14.));
        for (side, label) in ["First text", "Second text"].iter().enumerate() {
            inputs = inputs.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(*label),
                            )
                            .child(self.button(Control::Paste(side), "Paste", p, cx))
                            .child(self.button(Control::Clear(side), "Clear", p, cx)),
                    )
                    .child(card(self.inputs[side].clone(), 190., p))
                    .child(self.button(
                        if side == 0 {
                            Control::Pin
                        } else {
                            Control::UsePin
                        },
                        if side == 0 {
                            "Pin this text"
                        } else {
                            "Use pinned text"
                        },
                        p,
                        cx,
                    )),
            );
        }
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(tabs)
            .child(inputs);
        if let Some(error) = error {
            content = content.child(
                div()
                    .p(px(10.))
                    .rounded(px(7.))
                    .bg(p.danger.opacity(0.07))
                    .text_color(p.danger)
                    .text_size(px(12.))
                    .child(error),
            );
        }
        if has_result {
            content = content.child(
                div().flex().justify_between().child("Result").child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.secondary)
                        .child(status.clone()),
                ),
            );
            if mode == Mode::Words {
                content = content.child(self.words.clone()).child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(self.button(Control::Previous, "Previous", p, cx))
                        .child(self.button(Control::Next, "Next", p, cx))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(p.secondary)
                                .child(self.words.read(cx).page_label()),
                        ),
                );
            } else {
                content = content.child(card(self.output.clone(), 210., p));
            }
        } else if !busy && self.session.read(cx).error.is_none() {
            content = content.child(
                div()
                    .p(px(24.))
                    .text_color(p.secondary)
                    .text_size(px(12.))
                    .child("Paste or enter text on either side to begin."),
            );
        }
        div()
            .size_full()
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::key))
            .child(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Text Comparison"),
                    )
                    .child(self.button(Control::Search, "Search", p, cx))
                    .child(self.button(Control::New, "New Window", p, cx)),
            )
            .child(
                div()
                    .id("compare-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .child(content),
            )
            .child(
                div()
                    .p(px(12.))
                    .border_t_1()
                    .border_color(p.separator)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(if self.notice.is_empty() {
                                status
                            } else {
                                self.notice.clone()
                            }),
                    )
                    .child(self.button(
                        if busy {
                            Control::Cancel
                        } else {
                            Control::Refresh
                        },
                        if busy { "Cancel" } else { "Refresh" },
                        p,
                        cx,
                    ))
                    .child(self.button(Control::Copy, "Copy Result", p, cx)),
            )
    }
}
pub(crate) fn open(input: String, cx: &mut App) {
    if crate::shutdown::requested(cx) {
        return;
    }
    let bounds = Bounds::centered(None, size(px(820.), px(660.)), cx);
    if cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(740.), px(560.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Text Comparison — Bello Box".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |w, cx| {
                crate::shutdown::guard_window(w, cx);
                cx.new(|cx| CompareWindow::new(input, w, cx))
            },
        )
        .is_err()
    {
        eprintln!("Cannot open Text Comparison window.");
    }
}
#[cfg(test)]
mod tests;
