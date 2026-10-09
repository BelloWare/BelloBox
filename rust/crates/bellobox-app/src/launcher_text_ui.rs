//! Selection-only Text Tools preview; opening snapshots choices into a fresh popup.
use crate::text_tool_state::{Category, Choices, TextSession};
use bello_workbench_ui::{EditorAppearance, EditorView};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

pub(crate) struct Open;
impl EventEmitter<Open> for LauncherTextPreview {}

pub(crate) struct LauncherTextPreview {
    pub session: Entity<TextSession>,
    output: Entity<EditorView>,
    controls: Vec<FocusHandle>,
    menu: bool,
    menu_index: usize,
    menu_bounds: Rc<Cell<Bounds<Pixels>>>,
    active: bool,
    consume: [bool; 2],
    _subscription: Subscription,
}
impl LauncherTextPreview {
    pub fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let session = cx.new(|cx| TextSession::new(input, Choices::default(), true, cx));
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
            menu: false,
            menu_index: 0,
            menu_bounds: Rc::new(Cell::new(Bounds::default())),
            controls: (0..19).map(|_| cx.focus_handle()).collect(),
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
        if !active {
            self.menu = false;
        }
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        self.active = false;
        self.session.update(cx, |s, cx| s.retire(cx));
    }
    fn enabled(&self, index: usize, cx: &App) -> bool {
        if !self.active || !self.session.read(cx).can_snapshot() {
            return false;
        }
        let s = self.session.read(cx);
        match index {
            0..=6 => !s.oversized_preview(),
            7 => {
                !s.oversized_preview()
                    && matches!(s.choices.category, Category::Case | Category::Lines)
            }
            8..=12 => {
                !s.oversized_preview()
                    && matches!(s.choices.category, Category::Encode | Category::Decode)
                    && index - 8 < s.choices.options().len()
            }
            13 => s.can_copy(),
            14 => s.oversized_preview(),
            15..=18 => s.can_copy() && index - 15 < s.digest_rows.len(),
            _ => false,
        }
    }
    fn act(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.enabled(index, cx) {
            return;
        }
        match index {
            0..=6 => {
                let mut choices = self.session.read(cx).choices;
                choices.select_category(Category::ALL[index]);
                self.menu = false;
                self.session.update(cx, |s, cx| s.set_choices(choices, cx));
            }
            7 => {
                self.menu = !self.menu;
                self.menu_index = self
                    .session
                    .read(cx)
                    .choices
                    .options()
                    .iter()
                    .position(|(a, _)| *a == self.session.read(cx).choices.argument())
                    .unwrap_or(0);
                cx.notify();
            }
            8..=12 => self.select_option(index - 8, cx),
            13 => {
                if let Some(text) = self.session.read(cx).copy_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
                }
            }
            14 => cx.emit(Open),
            15..=18 => {
                if let Some(text) = self.session.read(cx).hash_copy(index - 15) {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
                }
            }
            _ => {}
        }
    }
    fn select_option(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut choices = self.session.read(cx).choices;
        if let Some((argument, _)) = choices.options().get(index) {
            choices.select_option(argument);
            self.session.update(cx, |s, cx| s.set_choices(choices, cx));
        }
        self.menu = false;
        cx.notify();
    }
    pub fn menu_open(&self) -> bool {
        self.menu
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
        if self.menu {
            if !event.is_held {
                match key {
                    "escape" | "tab" => {
                        self.menu = false;
                    }
                    "up" => self.menu_index = self.menu_index.saturating_sub(1),
                    "down" => {
                        self.menu_index = (self.menu_index + 1).min(
                            self.session
                                .read(cx)
                                .choices
                                .options()
                                .len()
                                .saturating_sub(1),
                        )
                    }
                    "enter" | "space" => {
                        self.consume[usize::from(key == "space")] = true;
                        self.select_option(self.menu_index, cx);
                    }
                    _ => {}
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
            return true;
        }
        if matches!(key, "enter" | "space") && self.consume.iter().any(|v| *v) {
            self.consume[usize::from(key == "space")] = true;
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if key == "tab" && !m.control && !m.platform && !m.alt {
            let mut order: Vec<_> = (0..13)
                .filter(|i| self.enabled(*i, cx))
                .map(|i| self.controls[i].clone())
                .collect();
            let state = self.session.read(cx);
            if !state.oversized_preview()
                && (!matches!(state.choices.category, Category::Hash | Category::Count)
                    || state.error.is_some())
            {
                order.push(self.output.read(cx).focus_handle(cx));
            }
            order.extend(
                (15..19)
                    .filter(|i| self.enabled(*i, cx))
                    .map(|i| self.controls[i].clone()),
            );
            order.extend(
                (13..15)
                    .filter(|i| self.enabled(*i, cx))
                    .map(|i| self.controls[i].clone()),
            );
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
        label: impl Into<SharedString>,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let bounds = self.menu_bounds.clone();
        let enabled = self.enabled(index, cx);
        let focus = self.controls[index].clone();
        let choices = self.session.read(cx).choices;
        let selected = (index < 7 && choices.category == Category::ALL[index])
            || ((8..=12).contains(&index)
                && choices
                    .options()
                    .get(index - 8)
                    .is_some_and(|(a, _)| *a == choices.argument()));
        div()
            .id(("text-preview-control", index))
            .debug_selector(move || format!("text-preview-control-{index}"))
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
            .relative()
            .when(index == 7, |s| {
                s.child(
                    canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                )
            })
            .child(label.into())
    }
}

impl Render for LauncherTextPreview {
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
        let state = self.session.read(cx);
        let oversized = state.oversized_preview();
        let choices = state.choices;
        let status = state.error.clone().unwrap_or_else(|| {
            if state.busy {
                "Working locally…".into()
            } else {
                choices.scope_note().into()
            }
        });
        let rows = state.digest_rows.clone();
        let counts = state.count_rows.clone();
        let text_output =
            !matches!(choices.category, Category::Hash | Category::Count) || state.error.is_some();
        div().debug_selector(||"text-tools-preview".into()).relative().h(px(224.)).flex_none().px(px(10.)).pt(px(4.)).pb(px(12.)).flex().flex_col().gap(px(6.))
        .when(oversized,|s|s.child(div().text_size(px(12.)).child("Selection exceeds 64 KB. Open Text Tools for the complete text; nothing was truncated.")).child(self.button(14,"Open Text Tools",p,cx)))
        .when(!oversized,|s|s
            .child(div().flex().gap(px(2.)).children(Category::ALL.into_iter().enumerate().map(|(i,c)|self.button(i,c.label(),p,cx))))
            .child(div().relative().flex().gap(px(3.)).items_center()
                .when(matches!(choices.category,Category::Case|Category::Lines),|s|s.child(self.button(7,format!("{} ▾",choices.options().iter().find(|(a,_)|*a==choices.argument()).map(|(_,l)|*l).unwrap_or("Option")),p,cx)))
                .when(matches!(choices.category,Category::Encode|Category::Decode),|s|s.children(choices.options().iter().enumerate().map(|(i,(_,label))|self.button(i+8,*label,p,cx))))
                .when(matches!(choices.category,Category::Pretty|Category::Hash|Category::Count),|s|s.child(div().text_size(px(10.)).text_color(p.secondary).child(choices.scope_note()))))
            .child(div().flex_1().min_h_0().p(px(6.)).rounded(px(7.)).bg(p.surface)
                .when(text_output,|s|s.child(self.output.clone()))
                .when(!text_output&&choices.category==Category::Hash,|s|s.flex().flex_col().gap(px(2.)).children(rows.iter().enumerate().map(|(i,(label,value))|div().flex().items_center().gap(px(6.)).child(div().w(px(48.)).text_size(px(10.)).child(*label)).child(div().flex_1().min_w_0().truncate().text_size(px(10.)).font_family("monospace").child(value.clone())).child(self.button(i+15,"Copy",p,cx).h(px(20.))))))
                .when(!text_output&&choices.category==Category::Count,|s|s.flex().gap(px(5.)).children(counts.iter().map(|(label,value)|div().flex_1().min_w_0().flex().flex_col().justify_center().gap(px(6.)).child(div().text_size(px(9.)).child(*label)).child(div().text_size(px(18.)).child(value.clone()))))))
            .child(div().flex().items_center().gap(px(6.)).child(div().flex_1().min_w_0().text_size(px(10.)).text_color(p.secondary).child(status)).child(self.button(13,"Copy",p,cx))))
        .when(self.menu,|s|s.child(deferred(div().id("text-option-menu").absolute().top(px(59.)).left(px(10.)).w(px(210.)).p(px(4.)).rounded(px(7.)).border_1().border_color(p.separator).bg(p.surface).occlude()
            .on_mouse_down_out(cx.listener(|this,event:&MouseDownEvent,w,cx|{if !this.menu_bounds.get().contains(&event.position){this.menu=false;w.prevent_default();cx.stop_propagation();cx.notify();}}))
            .children(choices.options().iter().enumerate().map(|(i,(_,label))|div().id(("text-menu-choice",i)).h(px(22.)).px(px(7.)).flex().items_center().text_size(px(11.)).when(i==self.menu_index,|s|s.bg(p.well)).child(*label).on_mouse_down(MouseButton::Left,|_,_,cx|cx.stop_propagation()).on_click(cx.listener(move|this,event,_,cx|{if matches!(event,ClickEvent::Mouse(_)){this.select_option(i,cx);}cx.stop_propagation();})))))))
    }
}
