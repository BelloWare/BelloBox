//! Independent, in-memory full URL windows. No implicit clipboard or transport.
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::url_editor::Draft;
use gpui::{
    App, Bounds, ClipboardItem, Context, Div, ElementId, Entity, FocusHandle, Focusable,
    FontWeight, KeyDownEvent, SharedString, Stateful, Subscription, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
const PAGE: usize = 8;
const EXAMPLE: &str = "https://example.com:8443/a?q=a+b&tag=one&tag=two&flag#hello";
struct Row {
    id: u64,
    name: Entity<EditorView>,
    value: Entity<EditorView>,
}
struct UrlWindow {
    input: Entity<EditorView>,
    fields: Vec<Entity<EditorView>>,
    rows: Vec<Row>,
    output: Entity<EditorView>,
    draft: Option<Draft>,
    source: String,
    page: usize,
    status: String,
    built: Option<Draft>,
    built_tokens: Vec<(u64, u64, bool)>,
    allowed_rejections: Vec<u64>,
    source_token: (u64, u64, bool),
    rejected: bool,
    subscriptions: Vec<Subscription>,
    row_subscriptions: Vec<Subscription>,
    focus: FocusHandle,
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
        e.set_compact(true, cx);
        e.set_vim(false, cx);
        e.set_read_only(readonly, cx);
        let mut a = EditorAppearance::plain();
        a.font_size = 12.;
        a.line_height = 18.;
        a.padding_x = 4.;
        a.padding_y = 3.;
        a.text = p.primary;
        a.caret = p.accent;
        a.selection = p.accent.opacity(0.18);
        e.set_appearance(a, cx);
        e
    })
}
fn card(e: Entity<EditorView>, h: f32, p: crate::theme::Palette) -> Div {
    div()
        .h(px(h))
        .min_w_0()
        .flex_1()
        .border_1()
        .border_color(p.separator)
        .rounded(px(5.))
        .bg(p.well)
        .child(e)
}
fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    p: crate::theme::Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(30.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.))
        .border_1()
        .border_color(p.separator)
        .rounded(px(6.))
        .bg(p.surface)
        .cursor_pointer()
        .child(text.into())
}
impl UrlWindow {
    fn new(text: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (text, status) = match bellobox_core::validate_input(&text) {
            Ok(()) => (text, String::new()),
            Err(e) => (String::new(), e),
        };
        let admission_error = (!status.is_empty()).then(|| status.clone());
        let input = editor(text, false, window, cx);
        let output = editor(String::new(), true, window, cx);
        let fields = (0..5)
            .map(|_| editor(String::new(), false, window, cx))
            .collect::<Vec<_>>();
        let mut s = Self {
            input: input.clone(),
            fields,
            rows: Vec::new(),
            output,
            draft: None,
            source: String::new(),
            page: 0,
            status,
            built: None,
            built_tokens: Vec::new(),
            allowed_rejections: Vec::new(),
            source_token: (0, 0, true),
            rejected: false,
            subscriptions: Vec::new(),
            row_subscriptions: Vec::new(),
            focus: cx.focus_handle(),
        };
        s.subscriptions.push(cx.subscribe_in(
            &input,
            window,
            |this, _, event: &EditorEvent, w, cx| {
                if matches!(event, EditorEvent::Changed)
                    && this.input.read(cx).edit_token() != this.source_token
                {
                    this.invalidate(cx);
                    this.status = "Source changed. Inspect URL to load its fields.".into();
                    let _ = w;
                }
            },
        ));
        for f in s.fields.clone() {
            s.subscriptions
                .push(cx.subscribe(&f, |this, _, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Changed) {
                        this.edited(cx);
                    }
                }));
        }
        s.inspect(window, cx);
        if let Some(error) = admission_error {
            s.rejected = true;
            s.status = error;
        }
        input.read(cx).focus(window);
        s
    }
    fn composing(&self, cx: &App) -> bool {
        std::iter::once(&self.input)
            .chain(self.fields.iter())
            .chain(self.rows.iter().flat_map(|r| [&r.name, &r.value]))
            .any(|e| e.read(cx).has_marked_text())
    }
    fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.built = None;
        if !self.output.read(cx).text().is_empty() {
            self.output
                .update(cx, |e, cx| e.set_text(String::new(), cx));
        }
        cx.notify();
    }
    fn inspect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing(cx) {
            self.status = "Finish the current text composition first.".into();
            return;
        }
        self.invalidate(cx);
        self.draft = None;
        self.page = 0;
        let text = self.input.read(cx).text();
        if let Err(e) = bellobox_core::validate_input(text) {
            self.status = e;
            self.source.clear();
            self.mount(window, cx);
            return;
        }
        self.source = text.to_owned();
        self.source_token = self.input.read(cx).edit_token();
        match Draft::inspect(&self.source) {
            Ok(d) => {
                self.draft = Some(d);
                self.status = "Inspected locally. Edit fields, then Build URL.".into();
            }
            Err(e) => {
                self.status = if self.source.is_empty() {
                    "Paste or enter an HTTP(S) URL.".into()
                } else {
                    e
                }
            }
        }
        if let Some(d) = &self.draft {
            for (e, value) in
                self.fields
                    .iter()
                    .zip([&d.scheme, &d.host, &d.port, &d.path, &d.fragment])
            {
                e.update(cx, |e, cx| e.set_text(value.clone(), cx));
            }
        }
        self.mount(window, cx);
    }
    fn tokens(&self, cx: &App) -> Vec<(u64, u64, bool)> {
        std::iter::once(&self.input)
            .chain(self.fields.iter())
            .chain(self.rows.iter().flat_map(|r| [&r.name, &r.value]))
            .map(|e| e.read(cx).edit_token())
            .collect()
    }
    fn rejection_pending(&self, cx: &App) -> bool {
        self.rejected
            || self.tokens(cx).iter().any(|t| !t.2)
            || self
                .tokens(cx)
                .iter()
                .map(|t| t.1)
                .ne(self.allowed_rejections.iter().copied())
    }
    fn snapshot(&self, cx: &App) -> Result<Draft, String> {
        if self.rejection_pending(cx) {
            return Err(
                "An edit was rejected. Clear or replace the source URL to continue.".into(),
            );
        }
        if self.input.read(cx).edit_token() != self.source_token {
            return Err("Source changed; inspect the current URL first.".into());
        }
        let mut d = self
            .draft
            .clone()
            .ok_or("Enter a valid HTTP(S) URL first.")?;
        let values = self
            .fields
            .iter()
            .map(|e| e.read(cx).text())
            .collect::<Vec<_>>();
        if values.iter().map(|s| s.len()).sum::<usize>() > bellobox_core::MAX_INPUT_BYTES {
            return Err("Edited fields exceed 500000 UTF-8 bytes.".into());
        }
        d.scheme = values[0].into();
        d.host = values[1].into();
        d.port = values[2].into();
        d.path = values[3].into();
        d.fragment = values[4].into();
        for r in &self.rows {
            if let Some(p) = d.parameters.iter_mut().find(|p| p.id == r.id) {
                let n = r.name.read(cx).text();
                let v = r.value.read(cx).text();
                if n.len() + v.len() > bellobox_core::MAX_INPUT_BYTES {
                    return Err("Edited query exceeds 500000 UTF-8 bytes.".into());
                }
                p.name = n.into();
                p.value = v.into();
            }
        }
        d.validate()?;
        Ok(d)
    }
    fn edited(&mut self, cx: &mut Context<Self>) {
        if self.built.is_some() && self.tokens(cx) == self.built_tokens {
            return;
        }
        self.invalidate(cx);
        self.status = "URL edited. Build URL to update the result.".into();
    }
    fn mount(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rows.clear();
        self.row_subscriptions.clear();
        if let Some(d) = &self.draft {
            for p in d.parameters.iter().skip(self.page * PAGE).take(PAGE) {
                self.rows.push(Row {
                    id: p.id,
                    name: editor(p.name.clone(), false, window, cx),
                    value: editor(p.value.clone(), false, window, cx),
                });
            }
        }
        for e in self
            .rows
            .iter()
            .flat_map(|r| [r.name.clone(), r.value.clone()])
        {
            self.row_subscriptions
                .push(cx.subscribe(&e, |this, _, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Changed) {
                        this.edited(cx);
                    }
                }));
        }
        self.allowed_rejections = self.tokens(cx).iter().map(|t| t.1).collect();
        if self.built.is_some() && self.built == self.draft {
            self.built_tokens = self.tokens(cx);
        }
        cx.notify();
    }
    fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.composing(cx) {
            self.status = "Finish the current text composition first.".into();
            cx.notify();
            return false;
        }
        match self.snapshot(cx) {
            Ok(d) => {
                if self.built.as_ref().is_some_and(|built| built != &d) {
                    self.invalidate(cx);
                }
                self.draft = Some(d);
                window.focus(&self.focus);
                true
            }
            Err(e) => {
                self.invalidate(cx);
                self.status = e;
                false
            }
        }
    }
    fn navigate(&mut self, page: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.flush(window, cx) {
            let pages = self
                .draft
                .as_ref()
                .map(|d| d.parameters.len().saturating_sub(1) / PAGE)
                .unwrap_or(0);
            self.page = page.min(pages);
            self.mount(window, cx);
        }
    }
    fn build(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.flush(window, cx) {
            return;
        }
        self.invalidate(cx);
        let d = self.draft.as_ref().unwrap();
        match d.build() {
            Ok(s) => {
                self.built = Some(d.clone());
                self.built_tokens = self.tokens(cx);
                self.output.update(cx, |e, cx| e.set_text(s, cx));
                self.status = "URL rebuilt locally.".into();
            }
            Err(e) => self.status = e,
        }
        cx.notify();
    }
    fn ready(&self, cx: &App) -> bool {
        !self.composing(cx)
            && !self.rejection_pending(cx)
            && self.built.is_some()
            && self.tokens(cx) == self.built_tokens
            && !self.output.read(cx).text().is_empty()
    }
    fn copy(&mut self, cx: &mut Context<Self>) {
        if self.ready(cx) {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.output.read(cx).text().into(),
            ));
            self.status = "Complete URL copied.".into();
            cx.notify();
        }
    }
    fn replace_source(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing(cx) {
            self.status = "Finish the current text composition first.".into();
            cx.notify();
            return;
        }
        if let Err(e) = bellobox_core::validate_input(&text) {
            self.invalidate(cx);
            self.rejected = true;
            self.status = e;
            return;
        }
        self.rejected = false;
        self.input.update(cx, |e, cx| e.set_text(text, cx));
        self.inspect(window, cx);
    }
}
impl Render for UrlWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::for_window(window);
        for e in std::iter::once(&self.input)
            .chain(self.fields.iter())
            .chain(self.rows.iter().flat_map(|r| [&r.name, &r.value]))
            .chain(std::iter::once(&self.output))
        {
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
        let ready = self.ready(cx);
        if self.built.is_some() && !ready {
            self.invalidate(cx);
        }
        let count = self.draft.as_ref().map(|d| d.parameters.len()).unwrap_or(0);
        let pages = count.max(1).div_ceil(PAGE);
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(9.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().child("Input URL"))
                    .child(
                        button("paste", "Paste", p).on_click(cx.listener(|s, _, w, cx| {
                            if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                                s.replace_source(text, w, cx);
                            }
                        })),
                    )
                    .child(button("example", "Example", p).on_click(
                        cx.listener(|s, _, w, cx| s.replace_source(EXAMPLE.into(), w, cx)),
                    ))
                    .child(button("clear", "Clear", p).on_click(
                        cx.listener(|s, _, w, cx| s.replace_source(String::new(), w, cx)),
                    )),
            )
            .child(card(self.input.clone(), 54., p))
            .child(
                button("inspect", "Inspect URL", p).on_click(cx.listener(|s, _, w, cx| {
                    if !s.rejection_pending(cx) {
                        s.inspect(w, cx);
                    } else {
                        s.status = "Clear or replace the source URL after a rejected edit.".into();
                        cx.notify();
                    }
                })),
            );
        if let Some(draft) = &self.draft {
            for (i, label) in [
                "Scheme (http / https)",
                "Host",
                "Port (optional)",
                "Path",
                "Fragment",
            ]
            .iter()
            .enumerate()
            {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().w(px(140.)).text_size(px(12.)).child(*label))
                        .child(card(self.fields[i].clone(), 30., p)),
                );
            }
            content = content.child(div().text_size(px(12.)).child(format!(
                "Query parameters · {count} total · page {} of {pages} · all rows retained",
                self.page + 1
            )));
            for (offset, r) in self.rows.iter().enumerate() {
                let id = r.id;
                let has = draft
                    .parameters
                    .get(self.page * PAGE + offset)
                    .is_some_and(|p| p.id == id && p.has_value);
                content = content.child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .items_center()
                        .child(card(r.name.clone(), 32., p))
                        .child(card(r.value.clone(), 32., p))
                        .child(
                            button(("equals", id), if has { "☑ =" } else { "☐ =" }, p).on_click(
                                cx.listener(move |s, _, w, cx| {
                                    if s.flush(w, cx) {
                                        if let Some(p) = s
                                            .draft
                                            .as_mut()
                                            .unwrap()
                                            .parameters
                                            .iter_mut()
                                            .find(|p| p.id == id)
                                        {
                                            p.has_value = !p.has_value;
                                        }
                                        s.invalidate(cx);
                                    }
                                }),
                            ),
                        )
                        .child(button(("remove", id), "Remove", p).on_click(cx.listener(
                            move |s, _, w, cx| {
                                if s.flush(w, cx) {
                                    s.draft.as_mut().unwrap().remove(id);
                                    s.invalidate(cx);
                                    s.page = s.page.min(
                                        s.draft
                                            .as_ref()
                                            .unwrap()
                                            .parameters
                                            .len()
                                            .saturating_sub(1)
                                            / PAGE,
                                    );
                                    s.mount(w, cx);
                                }
                            },
                        ))),
                );
            }
            content = content.child(
                div()
                    .flex()
                    .gap(px(7.))
                    .child(
                        button("first", "First", p)
                            .on_click(cx.listener(|s, _, w, cx| s.navigate(0, w, cx))),
                    )
                    .child(button("prev", "Previous", p).on_click(
                        cx.listener(|s, _, w, cx| s.navigate(s.page.saturating_sub(1), w, cx)),
                    ))
                    .child(
                        button("next", "Next", p)
                            .on_click(cx.listener(|s, _, w, cx| s.navigate(s.page + 1, w, cx))),
                    )
                    .child(
                        button("last", "Last", p)
                            .on_click(cx.listener(move |s, _, w, cx| s.navigate(pages - 1, w, cx))),
                    )
                    .child(div().flex_1())
                    .child(button("add", "Add Parameter", p).on_click(cx.listener(
                        |s, _, w, cx| {
                            if s.flush(w, cx) {
                                match s.draft.as_mut().unwrap().add() {
                                    Ok(_) => {
                                        s.invalidate(cx);
                                        s.page = s
                                            .draft
                                            .as_ref()
                                            .unwrap()
                                            .parameters
                                            .len()
                                            .saturating_sub(1)
                                            / PAGE;
                                        s.mount(w, cx);
                                    }
                                    Err(e) => s.status = e,
                                }
                            }
                        },
                    ))),
            );
        }
        content=content.child(div().text_size(px(11.)).text_color(p.secondary).child("Names and values are literal decoded text. Turn off = for a bare flag. Nothing is opened or sent."))
            .child(div().flex().justify_between().items_center().child("Result").child(button("build","Build URL",p).on_click(cx.listener(|s,_,w,cx|s.build(w,cx)))))
            .child(card(self.output.clone(),84.,p));
        div()
            .size_full()
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key != "tab" || s.composing(cx) {
                    return;
                }
                let handles = std::iter::once(&s.input)
                    .chain(s.fields.iter().filter(|_| s.draft.is_some()))
                    .chain(s.rows.iter().flat_map(|r| [&r.name, &r.value]))
                    .map(|e| e.read(cx).focus_handle(cx))
                    .collect::<Vec<_>>();
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
                cx.stop_propagation();
            }))
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if s.composing(cx) {
                    return;
                }
                let command = e.keystroke.modifiers.platform
                    || (cfg!(target_os = "linux") && e.keystroke.modifiers.control);
                if command {
                    match e.keystroke.key.as_str() {
                        "w" => crate::shutdown::close_window(w, cx),
                        "n" => open(String::new(), cx),
                        "enter" => s.build(w, cx),
                        "c" if e.keystroke.modifiers.shift => s.copy(cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("URL & Query Inspector"),
                    )
                    .child(
                        button("new-window", "New Window", p)
                            .on_click(|_, _, cx| open(String::new(), cx)),
                    ),
            )
            .child(
                div()
                    .id("url-content")
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
                            .child(self.status.clone()),
                    )
                    .child(
                        button("chain", "Use as Input", p)
                            .when(!ready, |b| b.opacity(0.4))
                            .on_click(cx.listener(|s, _, w, cx| {
                                if s.ready(cx) {
                                    let text = s.output.read(cx).text().to_owned();
                                    s.replace_source(text, w, cx);
                                }
                            })),
                    )
                    .child(
                        button("copy", "Copy Result", p)
                            .when(!ready, |b| b.opacity(0.4))
                            .on_click(cx.listener(|s, _, _, cx| s.copy(cx))),
                    ),
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
                    title: Some("URL & Query Inspector — Bello Box".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |w, cx| {
                crate::shutdown::guard_window(w, cx);
                cx.new(|cx| UrlWindow::new(input, w, cx))
            },
        )
        .is_err()
    {
        eprintln!("Cannot open URL inspector window.");
    }
}

#[cfg(test)]
mod tests;
