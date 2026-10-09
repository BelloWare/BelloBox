//! Swift's compact JSON row: mode controls and complete output, no input editor.
use crate::json_session::{JsonSession, Mode};
use bello_workbench_ui::{EditorAppearance, EditorView};
use gpui::{prelude::*, *};

pub(crate) struct Open;
impl EventEmitter<Open> for LauncherJsonPreview {}

pub(crate) struct LauncherJsonPreview {
    pub session: Entity<JsonSession>,
    output: Entity<EditorView>,
    controls: Vec<FocusHandle>,
    active: bool,
    consume: [bool; 2],
    _subscription: Subscription,
}
impl LauncherJsonPreview {
    pub fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let session = cx.new(|cx| JsonSession::new(input, true, cx));
        let output = cx.new(|cx| {
            let mut e = EditorView::new(String::new(), window, cx);
            e.set_appearance(EditorAppearance::plain(), cx);
            e.set_read_only(true, cx);
            e
        });
        let subscription = cx.observe(&session, |this, _, cx| {
            this.sync(cx);
            cx.notify();
        });
        Self {
            session,
            output,
            controls: (0..7).map(|_| cx.focus_handle()).collect(),
            active: true,
            consume: [false; 2],
            _subscription: subscription,
        }
    }
    fn sync(&mut self, cx: &mut Context<Self>) {
        let s = self.session.read(cx);
        let text = s.error.as_ref().unwrap_or(&s.output).clone();
        if self.output.read(cx).text() != text {
            self.output.update(cx, |e, cx| e.set_text(text, cx));
        }
    }
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        self.active = false;
        self.session.update(cx, |s, cx| s.retire_palette(cx));
    }
    fn enabled(&self, index: usize, cx: &App) -> bool {
        if !self.active || !self.session.read(cx).can_transfer() {
            return false;
        }
        let s = self.session.read(cx);
        match index {
            0..=2 => !s.oversized_preview(),
            3 => s.busy,
            4 => s.can_chain(),
            5 => s.can_copy(),
            6 => s.oversized_preview(),
            _ => false,
        }
    }
    fn act(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.enabled(index, cx) {
            return;
        }
        match index {
            0..=2 => self
                .session
                .update(cx, |s, cx| s.set_mode(Mode::ALL[index], cx)),
            3 => self.session.update(cx, |s, cx| s.cancel(cx)),
            4 => {
                self.session.update(cx, |s, cx| s.chain(cx));
            }
            5 => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.session.read(cx).output.clone(),
                ));
            }
            6 => cx.emit(Open),
            _ => {}
        }
    }
    pub fn owns_focus(&self, window: &Window, cx: &App) -> bool {
        self.output.read(cx).focus_handle(cx).is_focused(window)
            || self.controls.iter().any(|f| f.is_focused(window))
    }
    #[cfg(test)]
    pub(crate) fn output_editor(&self) -> Entity<EditorView> {
        self.output.clone()
    }
    pub fn output_focused(&self, window: &Window, cx: &App) -> bool {
        self.output.read(cx).focus_handle(cx).is_focused(window)
    }
    pub fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        search: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.active {
            return false;
        }
        let key = event.keystroke.key.as_str();
        let m = event.keystroke.modifiers;
        if matches!(key, "enter" | "space") && self.consume.iter().any(|v| *v) {
            self.consume[usize::from(key == "space")] = true;
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if key == "tab" && !m.control && !m.platform && !m.alt {
            let mut order: Vec<_> = self
                .controls
                .iter()
                .enumerate()
                .filter(|(i, _)| self.enabled(*i, cx))
                .map(|(_, f)| f.clone())
                .collect();
            if !self.session.read(cx).oversized_preview() {
                order.insert(3.min(order.len()), self.output.read(cx).focus_handle(cx));
            }
            let current = order.iter().position(|f| f.is_focused(window));
            let next = match (current, m.shift) {
                (Some(0), true) => None,
                (Some(i), true) => Some(i - 1),
                (Some(i), false) if i + 1 < order.len() => Some(i + 1),
                (Some(_), false) => None,
                (None, true) => order.len().checked_sub(1),
                (None, false) => (!order.is_empty()).then_some(0),
            };
            if !event.is_held {
                if let Some(i) = next {
                    order[i].focus(window);
                } else {
                    search.focus(window);
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if matches!(key, "enter" | "space")
            && !m.control
            && !m.platform
            && !m.alt
            && !m.shift
            && let Some(index) = self.controls.iter().position(|f| f.is_focused(window))
        {
            self.consume[usize::from(key == "space")] = true;
            if !event.is_held {
                self.act(index, cx);
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        self.controls.iter().any(|f| f.is_focused(window))
    }
    pub fn handle_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let i = match event.keystroke.key.as_str() {
            "enter" => 0,
            "space" => 1,
            _ => return false,
        };
        if std::mem::take(&mut self.consume[i]) {
            window.prevent_default();
            cx.stop_propagation();
            true
        } else {
            false
        }
    }
    fn button(
        &self,
        index: usize,
        label: &'static str,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let enabled = self.enabled(index, cx);
        let focus = self.controls[index].clone();
        let selected = index < 3 && self.session.read(cx).mode == Mode::ALL[index];
        div()
            .id(("json-preview-control", index))
            .debug_selector(move || format!("json-preview-control-{index}"))
            .track_focus(&focus)
            .h(px(26.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(5.))
            .border_1()
            .border_color(if selected { p.accent } else { p.separator })
            .text_size(px(11.))
            .text_color(if enabled {
                p.primary
            } else {
                p.secondary.opacity(0.5)
            })
            .when(enabled, |s| {
                s.cursor_pointer()
                    .focus(move |s| s.border_color(p.accent))
                    .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                        focus.focus(w);
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, event, _, cx| {
                        if matches!(event, ClickEvent::Mouse(_)) {
                            this.act(index, cx);
                        }
                        cx.stop_propagation();
                    }))
            })
            .child(label)
    }
}
impl Render for LauncherJsonPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::for_window(window);
        self.output.update(cx, |e, cx| {
            let mut a = e.appearance().clone();
            if a.text != p.primary {
                a.text = p.primary;
                a.selection = p.accent.opacity(0.18);
                e.set_appearance(a, cx);
            }
        });
        let oversized = self.session.read(cx).oversized_preview();
        let busy = self.session.read(cx).busy;
        let status = if busy {
            "Working…".into()
        } else {
            self.session.read(cx).status.clone()
        };
        let empty = !busy
            && self.session.read(cx).output.is_empty()
            && self.session.read(cx).error.is_none();
        div()
            .debug_selector(|| "json-preview".into())
            .h(px(224.)).flex_none().px(px(10.)).pt(px(4.)).pb(px(12.)).flex().flex_col().gap(px(8.))
            .when(oversized, |s| s
                .child(div().text_size(px(12.)).child("Draft exceeds the 64 KB preview limit. Open JSON Tools to work with the complete draft; nothing was truncated."))
                .child(self.button(6, "Open JSON Tools", p, cx)))
            .when(!oversized, |s| s
                .child(div().flex().items_center().gap(px(4.))
                    .child(div().flex_1().text_size(px(11.)).child(if status.is_empty() { "JSON Tools".to_owned() } else { status }))
                    .children(Mode::ALL.into_iter().enumerate().map(|(i, m)| self.button(i, m.label(), p, cx))))
                .child(div().debug_selector(|| "json-preview-output".into()).relative().flex_1().min_h(px(0.)).p(px(8.)).rounded(px(7.)).bg(p.surface)
                    .child(self.output.clone())
                    .when(empty, |s| s.child(div().absolute().top(px(8.)).left(px(8.)).text_size(px(11.)).text_color(p.secondary).child("Paste text or use the clipboard to begin."))))
                .child(div().flex().gap(px(6.)).justify_end()
                    .when(busy, |s| s.child(self.button(3, "Cancel", p, cx)))
                    .when(self.session.read(cx).can_chain(), |s| s.child(self.button(4, "Use as Input", p, cx)))
                    .child(self.button(5, "Copy", p, cx))))
    }
}
