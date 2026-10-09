//! Text Tools opens a fresh owner from a value snapshot, never a transferred worker.
use super::*;
use crate::text_tool_state::{Category, Handoff};
use gpui::Focusable;
impl BelloBox {
    pub(super) fn sync_text(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.text_session.as_ref() else {
            return;
        };
        let s = session.read(cx);
        self.busy = s.busy;
        self.error = s.error.clone();
        self.status = s.status.clone();
        let output = s.output.clone();
        if self.output.read(cx).text() != output {
            self.output.update(cx, |e, cx| e.set_text(output, cx));
        }
        cx.notify();
    }
    pub(super) fn init_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.text_session.clone() else {
            return;
        };
        self._subscriptions
            .push(cx.observe(&session, |this, _, cx| this.sync_text(cx)));
        let owner = window.window_handle();
        self._subscriptions.push(cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner) {
                session.update(cx, |s, cx| s.retire(cx));
            }
        }));
        self.sync_text(cx);
    }
    pub(super) fn text_category(&mut self, category: Category, cx: &mut Context<Self>) {
        if let Some(s) = self.text_session.clone() {
            let mut choices = s.read(cx).choices;
            choices.select_category(category);
            s.update(cx, |s, cx| s.set_choices(choices, cx));
            self.sync_text(cx);
        }
    }
    pub(super) fn text_option(&mut self, argument: &str, cx: &mut Context<Self>) {
        if let Some(s) = self.text_session.clone() {
            let mut choices = s.read(cx).choices;
            choices.select_option(argument);
            s.update(cx, |s, cx| s.set_choices(choices, cx));
            self.sync_text(cx);
        }
    }
    pub(super) fn text_copy(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.text_session.as_ref()
            && self.input.read(cx).text() == s.read(cx).input()
            && let Some(text) = s.read(cx).copy_text()
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
            self.status = "Result copied.".into();
            cx.notify();
        }
    }
    pub(super) fn text_chain(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.text_session.as_ref()
            && self.input.read(cx).text() == s.read(cx).input()
            && s.read(cx).can_chain()
        {
            let output = s.read(cx).output.clone();
            self.input.update(cx, |e, cx| e.set_text(output, cx));
        }
    }
}
pub(super) fn open(handoff: Handoff, cx: &mut App) -> Result<(), String> {
    open_with(handoff, cx, create_window)
}
fn create_window(handoff: Handoff, cx: &mut App) -> Result<gpui::WindowHandle<BelloBox>, String> {
    let bounds = Bounds::centered(None, size(px(720.), px(660.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(720.), px(520.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Text Tools — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            crate::shutdown::guard_window(window, cx);
            cx.new(|cx| {
                BelloBox::new_for_with_sessions(
                    "textTools".into(),
                    handoff.input,
                    None,
                    Some(handoff.choices),
                    window,
                    cx,
                )
            })
        },
    )
    .map_err(|e| format!("Cannot open Text Tools: {e}"))
}
fn open_with(
    handoff: Handoff,
    cx: &mut App,
    create: impl FnOnce(Handoff, &mut App) -> Result<gpui::WindowHandle<BelloBox>, String>,
) -> Result<(), String> {
    if crate::shutdown::requested(cx) {
        return Err("The app is closing.".into());
    }
    bellobox_core::validate_input(&handoff.input)?;
    let category = launcher::category(&handoff.input);
    crate::screenshot_ui::area_navigation_changed(cx);
    let handle = create(handoff, cx)?;
    if crate::shutdown::requested(cx) || !cx.windows().contains(&handle.into()) {
        let _ = handle.update(cx, |_, w, _| w.remove_window());
        return Err("Text Tools closed before opening completed.".into());
    }
    if let Ok(mut settings) = Settings::load(&config_dir().join("settings.json")) {
        settings.explicit_open("textTools", category, now());
        let _ = settings.save(&config_dir().join("settings.json"));
    }
    Ok(())
}
#[cfg(test)]
mod tests;

impl BelloBox {
    pub(super) fn text_enabled(&self, index: usize, cx: &App) -> bool {
        let Some(session) = self.text_session.as_ref() else {
            return false;
        };
        let s = session.read(cx);
        match index {
            0..=6 => true,
            7..=15 => index - 7 < s.choices.options().len(),
            16 => self.input.read(cx).text() != self.initial_text,
            17 => s.can_chain() && self.input.read(cx).text() == s.input(),
            18 => s.can_copy() && self.input.read(cx).text() == s.input(),
            19 => true,
            20 => !self.input.read(cx).text().is_empty(),
            _ => false,
        }
    }
    pub(super) fn text_act(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.text_enabled(index, cx) {
            return;
        }
        match index {
            0..=6 => self.text_category(Category::ALL[index], cx),
            7..=15 => {
                let options = self
                    .text_session
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .choices
                    .options();
                if let Some((a, _)) = options.get(index - 7) {
                    self.text_option(a, cx);
                }
            }
            16 => {
                let text = self.initial_text.clone();
                self.input.update(cx, |e, cx| e.set_text(text, cx));
            }
            17 => self.text_chain(cx),
            18 => self.text_copy(cx),
            19 => self.text_paste(cx),
            20 => {
                self.input.update(cx, |e, cx| e.set_text(String::new(), cx));
                self.run_tool(cx);
            }
            _ => {}
        }
    }
    pub(super) fn text_button(
        &self,
        index: usize,
        label: &'static str,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let focus = self.text_focus[index].clone();
        let enabled = self.text_enabled(index, cx);
        button(("text-full-control", index), label, p)
            .track_focus(&focus)
            .when(!enabled, |s| s.opacity(0.45).cursor_default())
            .focus(move |s| s.border_color(p.accent))
            .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| {
                focus.focus(w);
                cx.stop_propagation();
            })
            .on_click(cx.listener(move |this, event, _, cx| {
                if matches!(event, gpui::ClickEvent::Mouse(_)) {
                    this.text_act(index, cx);
                }
                cx.stop_propagation();
            }))
    }
    pub(super) fn text_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.text_session.is_none() || self.input.read(cx).has_marked_text() {
            return false;
        }
        let k = event.keystroke.key.as_str();
        let m = event.keystroke.modifiers;
        if matches!(k, "enter" | "space") && self.text_consume.iter().any(|x| *x) {
            self.text_consume[usize::from(k == "space")] = true;
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if k == "tab" && !m.control && !m.platform && !m.alt {
            let mut order = vec![self.input.read(cx).focus_handle(cx)];
            order.extend(
                self.text_focus
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i < 16 && self.text_enabled(*i, cx))
                    .map(|(_, f)| f.clone()),
            );
            order.push(self.output.read(cx).focus_handle(cx));
            order.extend(
                (16..21)
                    .filter(|i| self.text_enabled(*i, cx))
                    .map(|i| self.text_focus[i].clone()),
            );
            let current = order.iter().position(|f| f.is_focused(window));
            let next = match (current, m.shift) {
                (Some(i), true) => (i + order.len() - 1) % order.len(),
                (Some(i), false) => (i + 1) % order.len(),
                (None, true) => order.len() - 1,
                _ => 0,
            };
            if !event.is_held {
                order[next].focus(window);
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if matches!(k, "enter" | "space")
            && !m.control
            && !m.platform
            && !m.alt
            && !m.shift
            && let Some(i) = self.text_focus.iter().position(|f| f.is_focused(window))
        {
            self.text_consume[usize::from(k == "space")] = true;
            if !event.is_held {
                self.text_act(i, cx);
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        false
    }
    pub(super) fn text_key_up(
        &mut self,
        event: &gpui::KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let i = match event.keystroke.key.as_str() {
            "enter" => 0,
            "space" => 1,
            _ => return false,
        };
        if std::mem::take(&mut self.text_consume[i]) {
            window.prevent_default();
            cx.stop_propagation();
            true
        } else {
            false
        }
    }
}

impl BelloBox {
    pub(super) fn text_paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.input.update(cx, |e, cx| e.set_text(text, cx));
            // The editor keeps a rejected draft; run_tool checks size before cloning.
            self.run_tool(cx);
        }
    }
    pub(super) fn text_input_actions(
        &self,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().text_size(px(12.)).child("Input"))
            .child(div().flex_1())
            .child(self.text_button(19, "Paste", p, cx))
            .child(self.text_button(20, "Clear", p, cx))
            .child(self.text_button(16, "Reset", p, cx))
    }
}
