//! Independent full-window Regex Tester; drafts remain in memory only.
use crate::regex_session::RegexSession;
use bello_workbench_ui::{
    EditorAppearance, EditorEvent, EditorView, TextDecoration, TextPresentation,
};
use bellobox_core::developer::regex_inspection::{self, Mode};
use gpui::{
    App, Bounds, ClipboardItem, Context, Div, Entity, FocusHandle, Focusable, FontWeight,
    KeyDownEvent, SharedString, Stateful, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use std::sync::Arc;
type EditToken = (u64, u64, bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Search,
    New,
    Matches,
    Extract,
    Replace,
    IgnoreCase,
    Multiline,
    Paste(usize),
    Clear(usize),
    Refresh,
    Cancel,
    Copy,
    Chain,
}
struct RegexWindow {
    inputs: [Entity<EditorView>; 3],
    output: Entity<EditorView>,
    highlighted: Entity<EditorView>,
    session: Entity<RegexSession>,
    observed: [EditToken; 3],
    observed_composing: bool,
    rejected: [bool; 3],
    displayed_revision: Option<u64>,
    displayed_result: bool,
    decorated_revision: Option<u64>,
    decorated_color: Option<gpui::Hsla>,
    notice: String,
    closed: bool,
    focus: FocusHandle,
    controls: Vec<(Control, FocusHandle)>,
    subscriptions: Vec<Subscription>,
}
fn editor(
    text: String,
    readonly: bool,
    limit: usize,
    window: &mut Window,
    cx: &mut App,
) -> Entity<EditorView> {
    let p = crate::theme::for_window(window);
    cx.new(|cx| {
        let mut e = EditorView::new(text, window, cx);
        assert!(e.set_edit_byte_limit(limit));
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
impl RegexWindow {
    fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let admission = regex_inspection::validate(&input, "", "").err();
        let input = if admission.is_some() {
            String::new()
        } else {
            input
        };
        let inputs = [
            editor(input, false, regex_inspection::INPUT_LIMIT, window, cx),
            editor(
                String::new(),
                false,
                regex_inspection::PATTERN_LIMIT,
                window,
                cx,
            ),
            editor(
                String::new(),
                false,
                regex_inspection::REPLACEMENT_LIMIT,
                window,
                cx,
            ),
        ];
        let observed = inputs.each_ref().map(|e| e.read(cx).edit_token());
        let session = cx.new(RegexSession::new);
        let mut s = Self {
            inputs,
            output: editor(
                String::new(),
                true,
                regex_inspection::OUTPUT_LIMIT,
                window,
                cx,
            ),
            highlighted: editor(
                String::new(),
                true,
                regex_inspection::INPUT_LIMIT,
                window,
                cx,
            ),
            session,
            observed,
            observed_composing: false,
            rejected: [false; 3],
            displayed_revision: None,
            displayed_result: false,
            decorated_revision: None,
            decorated_color: None,
            notice: String::new(),
            closed: false,
            focus: cx.focus_handle(),
            controls: [
                Control::Search,
                Control::New,
                Control::Paste(1),
                Control::Clear(1),
                Control::IgnoreCase,
                Control::Multiline,
                Control::Matches,
                Control::Extract,
                Control::Replace,
                Control::Paste(2),
                Control::Clear(2),
                Control::Paste(0),
                Control::Clear(0),
                Control::Refresh,
                Control::Cancel,
                Control::Copy,
                Control::Chain,
            ]
            .into_iter()
            .map(|c| (c, cx.focus_handle()))
            .collect(),
            subscriptions: Vec::new(),
        };
        for e in s.inputs.clone() {
            // Mark/unmark can notify without EditorEvent::Changed, including
            // same-byte IME commits. Observe that lifecycle independently.
            s.subscriptions
                .push(cx.observe(&e, |s, _, cx| s.sync_inputs(false, cx)));
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
        let owner_window = window.window_handle();
        let weak = cx.weak_entity();
        s.subscriptions.push(cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner_window) {
                let _ = weak.update(cx, |s, cx| {
                    if !s.closed {
                        s.closed = true;
                        s.session.update(cx, |s, cx| s.retire(cx));
                        s.clear_output(cx);
                    }
                });
            }
        }));
        cx.on_release(|s: &mut Self, cx| s.session.update(cx, |s, cx| s.retire(cx)))
            .detach();
        s.sync_inputs(true, cx);
        if let Some(error) = admission {
            s.rejected[0] = true;
            s.session.update(cx, |s, cx| s.reject(error, cx));
        }
        s.inputs[1].read(cx).focus(window);
        s
    }
    fn tokens(&self, cx: &App) -> [EditToken; 3] {
        self.inputs.each_ref().map(|e| e.read(cx).edit_token())
    }
    fn composing(&self, cx: &App) -> bool {
        self.inputs.iter().any(|e| e.read(cx).has_marked_text())
    }
    fn current(&self, cx: &App) -> bool {
        !self.closed
            && !self.composing(cx)
            && self.tokens(cx) == self.observed
            && !self.rejected.iter().any(|v| *v)
            && self.observed.iter().all(|t| t.2)
            && self.session.read(cx).has_current_result()
    }
    fn ready(&self, cx: &App) -> bool {
        self.current(cx) && self.session.read(cx).can_copy()
    }
    fn clear_output(&mut self, cx: &mut Context<Self>) {
        if self.displayed_revision.is_none()
            && !self.displayed_result
            && self.decorated_revision.is_none()
            && self.output.read(cx).text().is_empty()
            && self.highlighted.read(cx).text().is_empty()
        {
            return;
        }
        self.displayed_revision = None;
        self.displayed_result = false;
        self.decorated_revision = None;
        self.output.update(cx, |e, cx| {
            if !e.text().is_empty() {
                e.set_text(String::new(), cx);
            }
            let _ = e.set_text_presentation(None, cx);
        });
        self.highlighted.update(cx, |e, cx| {
            e.set_text(String::new(), cx);
            let _ = e.set_text_presentation(None, cx);
        });
    }
    fn sync_inputs(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        let tokens = self.tokens(cx);
        let composing = self.composing(cx);
        if !force && tokens == self.observed && composing == self.observed_composing {
            return;
        }
        self.observed_composing = composing;
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
        if composing {
            self.session.update(cx, |s, cx| s.suspend_composition(cx));
        } else if self.rejected.iter().any(|v| *v) {
            self.session.update(cx, |s, cx| s.reject("An edit was rejected. Edit, Clear, or Paste that field to continue. No text was truncated.".into(), cx));
        } else {
            // Per-editor limits and rejection tokens are checked before cloning.
            let [text, pattern, replacement] =
                self.inputs.each_ref().map(|e| e.read(cx).text().to_owned());
            self.session
                .update(cx, |s, cx| s.set_drafts(text, pattern, replacement, cx));
        }
        cx.notify();
    }
    fn sync_output(&mut self, cx: &mut Context<Self>) {
        if self.closed
            || self.composing(cx)
            || self.tokens(cx) != self.observed
            || self.rejected.iter().any(|r| *r)
        {
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
        let mode = session.mode;
        let output = session
            .result
            .as_ref()
            .map(|r| r.output(mode).to_owned())
            .unwrap_or_default();
        let input = if has_result {
            self.inputs[0].read(cx).text().to_owned()
        } else {
            String::new()
        };
        self.output.update(cx, |e, cx| e.set_text(output, cx));
        self.highlighted.update(cx, |e, cx| e.set_text(input, cx));
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
        if text.len() > field_limit(side) {
            let error = format!(
                "Field exceeds {} UTF-8 bytes. Nothing was truncated.",
                field_limit(side)
            );
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
            .output(self.session.read(cx).mode)
            .to_owned();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.notice = "Complete result copied.".into();
        cx.notify();
    }
    fn enabled(&self, control: Control, cx: &App) -> bool {
        let s = self.session.read(cx);
        match control {
            Control::Copy => self.ready(cx),
            Control::Chain => self.ready(cx) && s.can_chain(),
            Control::Cancel => s.busy,
            Control::Refresh => !s.busy,
            Control::Paste(2) | Control::Clear(2) => s.mode == Mode::Replace,
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
            Control::Matches | Control::Extract | Control::Replace => {
                let mode = match control {
                    Control::Matches => Mode::Matches,
                    Control::Extract => Mode::Extract,
                    _ => Mode::Replace,
                };
                self.clear_output(cx);
                self.notice.clear();
                self.session.update(cx, |s, cx| s.set_mode(mode, cx));
                self.sync_output(cx);
            }
            Control::IgnoreCase | Control::Multiline => {
                let mut options = self.session.read(cx).options;
                if control == Control::IgnoreCase {
                    options.ignore_case = !options.ignore_case;
                } else {
                    options.multiline = !options.multiline;
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
            Control::Chain => {
                let text = self
                    .session
                    .read(cx)
                    .result
                    .as_ref()
                    .unwrap()
                    .output(self.session.read(cx).mode)
                    .to_owned();
                self.replace(0, text, cx);
                self.inputs[0].read(cx).focus(window);
            }
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
            .id(("regex-control", index))
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
            if let Control::Clear(side) = control
                && (*side != 2 || self.session.read(cx).mode == Mode::Replace)
            {
                handles.push(self.inputs[*side].read(cx).focus_handle(cx));
            }
            if *control == Control::Clear(0) && self.session.read(cx).result.is_some() {
                handles.push(self.highlighted.read(cx).focus_handle(cx));
                handles.push(self.output.read(cx).focus_handle(cx));
            }
        }
        handles
    }
    fn key(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let command = e.keystroke.modifiers.platform
            || (cfg!(target_os = "linux") && e.keystroke.modifiers.control);
        if self.composing(cx) {
            // Do not let an old read-only selection copy while another field
            // owns a marked composition, even before observers have run.
            if command
                && matches!(e.keystroke.key.as_str(), "c" | "x")
                && (self.output.read(cx).focus_handle(cx).is_focused(w)
                    || self.highlighted.read(cx).focus_handle(cx).is_focused(w))
            {
                cx.stop_propagation();
            }
            return;
        }
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
                    if !self.current(cx)
                        && (self.output.read(cx).focus_handle(cx).is_focused(w)
                            || self.highlighted.read(cx).focus_handle(cx).is_focused(w)) => {}
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
        } else if e.keystroke.key == "enter"
            && (self.inputs[1].read(cx).focus_handle(cx).is_focused(w)
                || self.inputs[2].read(cx).focus_handle(cx).is_focused(w))
        {
            // Keep pattern/replacement single-row keyboard behavior. Pasted
            // literal newlines are still preserved without normalization.
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

fn field_limit(side: usize) -> usize {
    match side {
        1 => regex_inspection::PATTERN_LIMIT,
        2 => regex_inspection::REPLACEMENT_LIMIT,
        _ => regex_inspection::INPUT_LIMIT,
    }
}
impl Render for RegexWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_inputs(false, cx);
        self.sync_output(cx);
        let p = crate::theme::for_window(window);
        for e in self.inputs.iter().chain([&self.output, &self.highlighted]) {
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
        let mode = session.mode;
        let options = session.options;
        let error = session.error.clone();
        let status = session.status.clone();
        let busy = session.busy;
        let has_result = session.result.is_some();
        if (self.decorated_revision != Some(session.revision)
            || self.decorated_color != Some(p.primary))
            && let Some(result) = &session.result
        {
            let presentation = TextPresentation {
                token: session.revision,
                text: Arc::from(self.inputs[0].read(cx).text()),
                // Zero-width matches remain in details/count/replacement; the
                // shared editor correctly rejects empty highlight spans.
                decorations: result
                    .matches
                    .iter()
                    .filter(|m| !m.bytes.is_empty())
                    .map(|m| TextDecoration {
                        range: m.bytes.clone(),
                        color: p.accent.opacity(0.22),
                    })
                    .collect(),
                emphasized: None,
            };
            self.decorated_revision = Some(session.revision);
            self.decorated_color = Some(p.primary);
            self.highlighted.update(cx, |e, cx| {
                let _ = e.set_text_presentation(Some(presentation), cx);
            });
        }
        let mut tabs = div().flex().gap(px(7.));
        for (control, label, choice) in [
            (Control::Matches, "Matches", Mode::Matches),
            (Control::Extract, "Extract", Mode::Extract),
            (Control::Replace, "Replace", Mode::Replace),
        ] {
            tabs = tabs.child(
                self.button(control, label, p, cx)
                    .when(mode == choice, |b| {
                        b.text_color(p.accent).border_color(p.accent)
                    }),
            );
        }
        let field = |side: usize, label: &'static str, height: f32| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex_1()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(12.))
                                .child(label),
                        )
                        .child(self.button(Control::Paste(side), "Paste", p, cx))
                        .child(self.button(Control::Clear(side), "Clear", p, cx)),
                )
                .child(card(self.inputs[side].clone(), height, p))
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(field(1, "Regular expression", 40.))
            .child(
                div()
                    .flex()
                    .gap(px(7.))
                    .child(self.button(
                        Control::IgnoreCase,
                        if options.ignore_case {
                            "☑ Ignore case"
                        } else {
                            "☐ Ignore case"
                        },
                        p,
                        cx,
                    ))
                    .child(self.button(
                        Control::Multiline,
                        if options.multiline {
                            "☑ Multiline anchors"
                        } else {
                            "☐ Multiline anchors"
                        },
                        p,
                        cx,
                    )),
            )
            .child(div().text_size(px(11.)).text_color(p.secondary).child(
                "Portable Rust regex · no lookaround/backreferences · detail ranges use UTF-16",
            ))
            .child(tabs);
        if mode == Mode::Replace {
            content = content.child(field(2, "Replacement ($0, $1…; backslash escapes)", 40.));
        }
        content = content.child(field(0, "Input", 145.));
        if let Some(error) = error {
            content = content.child(div().text_size(px(12.)).text_color(p.danger).child(error));
        }
        if has_result {
            content = content
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Matches in your text"),
                )
                .child(card(self.highlighted.clone(), 120., p))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Result"),
                )
                .child(card(self.output.clone(), 180., p));
        } else if !busy && self.session.read(cx).error.is_none() {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(p.secondary)
                    .child("Enter text and a pattern to see live matches."),
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
                            .child("Regex Tester"),
                    )
                    .child(self.button(Control::Search, "Search", p, cx))
                    .child(self.button(Control::New, "New Window", p, cx)),
            )
            .child(
                div()
                    .id("regex-content")
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
                    .child(self.button(Control::Chain, "Use as Input", p, cx))
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
                    title: Some("Regex Tester — Bello Box".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |w, cx| {
                crate::shutdown::guard_window(w, cx);
                cx.new(|cx| RegexWindow::new(input, w, cx))
            },
        )
        .is_err()
    {
        eprintln!("Cannot open Regex Tester window.");
    }
}
#[cfg(test)]
mod tests;
