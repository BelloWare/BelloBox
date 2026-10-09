//! Independent full-window HTTP & cURL inspector. Importing never sends.
use crate::http_session::HttpSession;
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::developer::http_request::{self, HttpRequestDraft};
use gpui::{
    App, Bounds, ClipboardItem, Context, Div, Entity, FocusHandle, Focusable, FontWeight,
    KeyDownEvent, SharedString, Stateful, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
const METHODS: [&str; 7] = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"];
type EditToken = (u64, u64, bool);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    MethodMenu,
    Method(usize),
    CustomMethod,
    Search,
    New,
    Import,
    Example,
    Paste(usize),
    Clear(usize),
    Send,
    Cancel,
    Copy,
}
struct HttpWindow {
    // Import, method, URL, headers, body. These are literal, memory-only editors.
    inputs: [Entity<EditorView>; 5],
    output: Entity<EditorView>,
    session: Entity<HttpSession>,
    observed: [EditToken; 5],
    observed_composing: bool,
    rejected: [bool; 5],
    import_dirty: bool,
    method_menu: bool,
    displayed_revision: Option<u64>,
    notice: String,
    closed: bool,
    focus: FocusHandle,
    controls: Vec<(Control, FocusHandle)>,
    subscriptions: Vec<Subscription>,
}
fn field_limit(side: usize) -> usize {
    match side {
        1 => http_request::MAX_METHOD_BYTES,
        3 => http_request::MAX_HEADER_BYTES,
        _ => http_request::MAX_DRAFT_BYTES,
    }
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
        appearance.font_family = "monospace".into();
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
impl HttpWindow {
    fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let oversized = input.len() > http_request::MAX_DRAFT_BYTES;
        let inputs = [
            if oversized { String::new() } else { input },
            "GET".into(),
            String::new(),
            String::new(),
            String::new(),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, text)| editor(text, false, field_limit(i), window, cx))
        .collect::<Vec<_>>();
        let inputs: [Entity<EditorView>; 5] = inputs.try_into().ok().unwrap();
        let observed = inputs.each_ref().map(|e| e.read(cx).edit_token());
        let session = cx.new(HttpSession::new);
        let mut s = Self {
            inputs,
            output: editor(
                String::new(),
                true,
                http_request::MAX_RESPONSE_TEXT_BYTES,
                window,
                cx,
            ),
            session,
            observed,
            observed_composing: false,
            rejected: [false; 5],
            import_dirty: false,
            method_menu: false,
            displayed_revision: None,
            notice: String::new(),
            closed: false,
            focus: cx.focus_handle(),
            controls: [
                Control::Search,
                Control::New,
                Control::Paste(0),
                Control::Clear(0),
                Control::Example,
                Control::Import,
                Control::MethodMenu,
                Control::Method(0),
                Control::Method(1),
                Control::Method(2),
                Control::Method(3),
                Control::Method(4),
                Control::Method(5),
                Control::Method(6),
                Control::CustomMethod,
                Control::Paste(2),
                Control::Clear(2),
                Control::Paste(3),
                Control::Clear(3),
                Control::Paste(4),
                Control::Clear(4),
                Control::Send,
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
                .push(cx.observe(&e, |s, _, cx| s.sync_inputs(false, cx)));
            s.subscriptions
                .push(cx.subscribe(&e, |s, _, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Changed) {
                        s.sync_inputs(false, cx);
                    }
                }));
        }
        s.subscriptions.push(cx.observe(&s.session, |s, _, cx| {
            s.sync_inputs(false, cx);
            s.sync_output(cx);
            cx.notify();
        }));
        // Native close, last-window shutdown, retained entities and app-controlled
        // quit all invalidate before asynchronous completion can publish.
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |w, cx| {
            let _ = weak.update(cx, |s, cx| s.close(cx));
            crate::shutdown::allow_close(w, cx)
        });
        let owner_window = window.window_handle();
        let weak = cx.weak_entity();
        s.subscriptions.push(cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner_window) {
                let _ = weak.update(cx, |s, cx| s.close(cx));
            }
        }));
        s.subscriptions
            .push(cx.observe_global::<crate::shutdown::Shutdown>(|s, cx| {
                if crate::shutdown::requested(cx) {
                    s.close(cx);
                }
            }));
        s.subscriptions.push(cx.on_app_quit(|s, cx| {
            s.close(cx);
            async {}
        }));
        cx.on_release(|s: &mut Self, cx| s.session.update(cx, |s, cx| s.retire(cx)))
            .detach();
        if oversized {
            s.rejected[0] = true;
            s.session.update(cx, |s, cx| {
                s.reject(
                    "Import exceeds 500,000 UTF-8 bytes. Nothing was truncated.".into(),
                    cx,
                )
            });
        } else if !s.inputs[0].read(cx).text().trim().is_empty() {
            s.import(cx);
        }
        s.inputs[0].read(cx).focus(window);
        s
    }
    fn close(&mut self, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.session.update(cx, |s, cx| s.retire(cx));
        self.clear_output(cx);
        for input in &self.inputs {
            input.update(cx, |e, cx| e.set_text(String::new(), cx));
        }
    }
    fn tokens(&self, cx: &App) -> [EditToken; 5] {
        self.inputs.each_ref().map(|e| e.read(cx).edit_token())
    }
    fn composing(&self, cx: &App) -> bool {
        self.inputs.iter().any(|e| e.read(cx).has_marked_text())
    }
    fn fields_valid(&self, cx: &App) -> bool {
        !self.closed
            && !self.composing(cx)
            && self.tokens(cx) == self.observed
            && !self.rejected.iter().any(|r| *r)
            && self.observed.iter().all(|t| t.2)
            && !self.import_dirty
    }
    fn ready(&self, cx: &App) -> bool {
        self.fields_valid(cx) && self.session.read(cx).can_copy()
    }
    fn clear_output(&mut self, cx: &mut Context<Self>) {
        self.displayed_revision = None;
        if !self.output.read(cx).text().is_empty() {
            self.output
                .update(cx, |e, cx| e.set_text(String::new(), cx));
        }
    }
    fn draft(&self, cx: &App) -> HttpRequestDraft {
        HttpRequestDraft {
            method: self.inputs[1].read(cx).text().into(),
            url: self.inputs[2].read(cx).text().into(),
            headers: self.inputs[3].read(cx).text().into(),
            body: self.inputs[4].read(cx).text().into(),
        }
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
        self.clear_output(cx);
        self.notice.clear();
        for (i, token) in tokens.iter().enumerate() {
            if !token.2 || token.1 != self.observed[i].1 {
                self.rejected[i] = true;
            } else if token.0 != self.observed[i].0 {
                self.rejected[i] = false;
            }
        }
        if tokens[0] != self.observed[0] {
            self.import_dirty = !self.inputs[0].read(cx).text().trim().is_empty();
        }
        self.observed = tokens;
        self.observed_composing = composing;
        if composing {
            self.session.update(cx, |s, cx| {
                s.reject(
                    "Finish the current text composition before sending.".into(),
                    cx,
                )
            });
        } else if self.rejected.iter().any(|r| *r) {
            self.session.update(cx, |s, cx| s.reject("An edit was rejected. Edit, Clear, or Paste that field to continue. Nothing was truncated.".into(), cx));
        } else if self.import_dirty {
            self.session.update(cx, |s, cx| {
                s.reject(
                    "Import changed. Choose Import to review its request fields.".into(),
                    cx,
                )
            });
        } else {
            let draft = self.draft(cx);
            self.session.update(cx, |s, cx| s.set_draft(draft, cx));
        }
        cx.notify();
    }
    fn import(&mut self, cx: &mut Context<Self>) {
        if self.closed || self.composing(cx) || self.rejected[0] {
            return;
        }
        self.clear_output(cx);
        match http_request::import_request(self.inputs[0].read(cx).text()) {
            Ok(draft) => {
                for (i, text) in [
                    draft.method.clone(),
                    draft.url.clone(),
                    draft.headers.clone(),
                    draft.body.clone(),
                ]
                .into_iter()
                .enumerate()
                {
                    self.inputs[i + 1].update(cx, |e, cx| e.set_text(text, cx));
                }
                self.observed = self.tokens(cx);
                self.rejected = [false; 5];
                self.import_dirty = false;
                self.session.update(cx, |s, cx| s.set_draft(draft, cx));
                self.notice = "Request imported. Review the fields and choose Send.".into();
            }
            Err(error) => {
                self.import_dirty = true;
                self.session.update(cx, |s, cx| s.reject(error, cx));
            }
        }
        cx.notify();
    }
    fn sync_output(&mut self, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            self.clear_output(cx);
            return;
        }
        let session = self.session.read(cx);
        if self.displayed_revision == Some(session.revision) {
            return;
        }
        let revision = session.revision;
        let text = session.result.as_ref().unwrap().text();
        self.output.update(cx, |e, cx| e.set_text(text, cx));
        self.displayed_revision = Some(revision);
    }
    fn replace(&mut self, side: usize, text: String, cx: &mut Context<Self>) {
        if self.closed || self.composing(cx) {
            return;
        }
        self.clear_output(cx);
        if text.len() > field_limit(side) {
            self.rejected[side] = true;
            self.session.update(cx, |s, cx| {
                s.reject(
                    format!(
                        "Field exceeds {} UTF-8 bytes. Nothing was truncated.",
                        field_limit(side)
                    ),
                    cx,
                )
            });
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
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.session.read(cx).result.as_ref().unwrap().text(),
        ));
        self.notice = "Complete response copied.".into();
        cx.notify();
    }
    fn enabled(&self, control: Control, cx: &App) -> bool {
        if self.closed {
            return false;
        }
        match control {
            Control::Method(_) => self.method_menu && !self.composing(cx),
            Control::CustomMethod => {
                self.method_menu
                    && !METHODS.contains(&self.inputs[1].read(cx).text())
                    && !self.composing(cx)
            }
            Control::Copy => self.ready(cx),
            Control::Send => {
                !self.method_menu && self.fields_valid(cx) && self.session.read(cx).can_send()
            }
            Control::Cancel => self.session.read(cx).busy,
            Control::Import => {
                !self.composing(cx)
                    && !self.rejected[0]
                    && !self.inputs[0].read(cx).text().trim().is_empty()
            }
            _ => !self.composing(cx),
        }
    }
    fn activate(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing(cx) {
            return;
        }
        self.sync_inputs(false, cx);
        if !self.enabled(control, cx) {
            return;
        }
        match control {
            Control::Search => crate::launcher_ui::open(String::new(), cx),
            Control::MethodMenu => self.method_menu = !self.method_menu,
            Control::Method(i) => {
                self.replace(1, METHODS[i].into(), cx);
                self.method_menu = false;
                self.focus_method(window);
            }
            Control::CustomMethod => {
                self.method_menu = false;
                self.focus_method(window);
            }
            Control::New => open(String::new(), cx),
            Control::Import => self.import(cx),
            Control::Example => {
                self.replace(
                    0,
                    "curl 'https://example.com' -H 'Accept: text/html'".into(),
                    cx,
                );
                self.import(cx);
            }
            Control::Paste(side) => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.replace(side, text, cx);
                } else {
                    self.notice = "The clipboard has no text.".into();
                }
            }
            Control::Clear(side) => self.replace(side, String::new(), cx),
            Control::Send => {
                self.clear_output(cx);
                self.notice.clear();
                self.session.update(cx, |s, cx| s.send(cx));
            }
            Control::Cancel => {
                self.clear_output(cx);
                self.notice.clear();
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
            .id(("http-control", index))
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
            .when(control == Control::Send, |b| {
                b.bg(p.accent_fill).text_color(gpui::white())
            })
            .when(!enabled, |b| b.opacity(0.4))
            .when(enabled, |b| b.cursor_pointer())
            .when(control == Control::MethodMenu, |b| b.w_full().min_w_0())
            .child(div().min_w_0().truncate().child(label.into()))
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
            if let Control::Clear(side) = control {
                handles.push(self.inputs[*side].read(cx).focus_handle(cx));
            }
        }
        if self.ready(cx) {
            handles.push(self.output.read(cx).focus_handle(cx));
        }
        handles
    }
    fn focus_method(&self, window: &mut Window) {
        let handle = &self
            .controls
            .iter()
            .find(|(c, _)| *c == Control::MethodMenu)
            .unwrap()
            .1;
        window.focus(handle);
    }
    fn key(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let command = e.keystroke.modifiers.platform
            || (cfg!(target_os = "linux") && e.keystroke.modifiers.control);
        if self.composing(cx) {
            if command
                && matches!(e.keystroke.key.as_str(), "c" | "x")
                && self.output.read(cx).focus_handle(cx).is_focused(w)
            {
                cx.stop_propagation();
            }
            return;
        }
        if command && self.method_menu {
            cx.stop_propagation();
            return;
        }
        if command {
            let m = e.keystroke.modifiers;
            let exact = if cfg!(target_os = "macos") {
                m.platform && !m.control
            } else {
                m.control && !m.platform
            };
            if !exact || m.alt || m.function || (m.shift && e.keystroke.key != "c") {
                return;
            }
            if e.is_held && matches!(e.keystroke.key.as_str(), "n" | "k" | "w" | "enter") {
                cx.stop_propagation();
                return;
            }
            match e.keystroke.key.as_str() {
                "n" => self.activate(Control::New, w, cx),
                "k" => self.activate(Control::Search, w, cx),
                "w" => {
                    self.close(cx);
                    crate::shutdown::close_window(w, cx);
                }
                "enter" => self.activate(Control::Send, w, cx),
                "c" if e.keystroke.modifiers.shift => self.copy(cx),
                "c" | "x"
                    if !self.ready(cx) && self.output.read(cx).focus_handle(cx).is_focused(w) => {}
                _ => return,
            }
        } else if self.method_menu && matches!(e.keystroke.key.as_str(), "escape" | "up" | "down") {
            if e.keystroke.key == "escape" {
                self.method_menu = false;
                self.focus_method(w);
            } else {
                let current = self.controls.iter().find_map(|(c, h)| match c {
                    Control::Method(i) if h.is_focused(w) => Some(*i),
                    _ => None,
                });
                let next = if e.keystroke.key == "up" {
                    current
                        .unwrap_or(0)
                        .checked_sub(1)
                        .unwrap_or(METHODS.len() - 1)
                } else {
                    current.map(|i| (i + 1) % METHODS.len()).unwrap_or(0)
                };
                let handle = &self
                    .controls
                    .iter()
                    .find(|(c, _)| *c == Control::Method(next))
                    .unwrap()
                    .1;
                w.focus(handle);
            }
            cx.notify();
        } else if e.keystroke.key == "tab" {
            let handles = self.tab_handles(cx);
            if handles.is_empty() {
                return;
            }
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
            && [1, 2]
                .iter()
                .any(|i| self.inputs[*i].read(cx).focus_handle(cx).is_focused(w))
        {
            // Literal single-row fields never send on plain Return.
        } else if matches!(e.keystroke.key.as_str(), "enter" | "space") {
            if let Some(control) = self
                .controls
                .iter()
                .find(|(_, h)| h.is_focused(w))
                .map(|(c, _)| *c)
            {
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
impl Render for HttpWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_inputs(false, cx);
        self.sync_output(cx);
        let p = crate::theme::for_window(window);
        for e in self.inputs.iter().chain([&self.output]) {
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
        let error = session.error.clone();
        let status = session.status.clone();
        let busy = session.busy;
        let running = session.physical_running();
        let has_result = self.ready(cx);
        let field = |side: usize, label: &'static str, height: f32| {
            div()
                .flex_1()
                .min_w_0()
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
            .child(field(0, "Import cURL or URL", 64.))
            .child(
                div()
                    .flex()
                    .gap(px(7.))
                    .child(self.button(Control::Import, "Import", p, cx))
                    .child(self.button(Control::Example, "Example", p, cx)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .w(px(160.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().h(px(30.)).text_size(px(12.)).child("Method"))
                            .child(
                                self.button(
                                    Control::MethodMenu,
                                    format!("{} ▾", self.inputs[1].read(cx).text()),
                                    p,
                                    cx,
                                )
                                .h(px(40.)),
                            ),
                    )
                    .child(field(2, "Request URL", 40.)),
            );
        if self.method_menu {
            let mut menu = div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .p(px(8.))
                .border_1()
                .border_color(p.separator)
                .rounded(px(7.))
                .bg(p.surface);
            for (i, method) in METHODS.iter().enumerate() {
                menu = menu.child(self.button(Control::Method(i), *method, p, cx));
            }
            let current = self.inputs[1].read(cx).text();
            if !METHODS.contains(&current) {
                menu = menu.child(self.button(Control::CustomMethod, current.to_owned(), p, cx));
            }
            content = content.child(menu);
        }
        content = content.child(div().flex().gap(px(12.)).child(field(3, "Headers · one per line", 140.)).child(field(4, "Body", 140.)))
            .child(div().text_size(px(11.)).text_color(p.secondary).child("Only Send makes a request. Redirects are shown for inspection. Requests and responses are not saved to history."));
        if let Some(error) = error {
            content = content.child(div().text_size(px(12.)).text_color(p.danger).child(error));
        }
        if has_result {
            content = content
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Response"),
                )
                .child(card(self.output.clone(), 220., p));
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
                            .child("HTTP & cURL"),
                    )
                    .child(self.button(Control::Search, "Search", p, cx))
                    .child(self.button(Control::New, "New Window", p, cx)),
            )
            .child(
                div()
                    .id("http-content")
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
                        Control::Send,
                        if running && !busy {
                            "Stopping…"
                        } else {
                            "Send"
                        },
                        p,
                        cx,
                    ))
                    .child(self.button(Control::Cancel, "Cancel", p, cx))
                    .child(self.button(Control::Copy, "Copy Response", p, cx)),
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
                    title: Some("HTTP & cURL — Bello Box".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |w, cx| {
                crate::shutdown::guard_window(w, cx);
                cx.new(|cx| HttpWindow::new(input, w, cx))
            },
        )
        .is_err()
    {
        eprintln!("Cannot open HTTP & cURL window.");
    }
}
#[cfg(test)]
mod tests;
