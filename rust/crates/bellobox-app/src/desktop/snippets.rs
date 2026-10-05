//! Library controls follow UtilityWorkbenchView.snippetControls / SnippetLibraryMenu.
mod fields;
use super::*;
use crate::snippet_library::Library;

pub(super) struct SnippetUi {
    library: Library,
    fields: fields::Fields,
    name: Entity<EditorView>,
    query: Entity<EditorView>,
    notice: Option<String>,
    error: Option<String>,
    deleting: bool,
    menu_focus: gpui::FocusHandle,
    previous_focus: Option<gpui::FocusHandle>,
}
impl BelloBox {
    pub(super) fn init_snippets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != "snippets" {
            return;
        }
        let p = crate::theme::for_window(window);
        let mut fields = Vec::new();
        for _ in 0..2 {
            let field = cx.new(|cx| {
                let mut editor = EditorView::new(String::new(), window, cx);
                editor.set_compact(true, cx);
                let mut style = EditorAppearance::plain();
                style.font_family = crate::theme::ui_font().into();
                style.font_size = 13.;
                style.line_height = 20.;
                style.wrap_lines = false;
                style.text = p.primary;
                style.caret = p.accent;
                style.selection = p.accent.opacity(0.18);
                editor.set_appearance(style, cx);
                editor
            });
            self._subscriptions
                .push(cx.subscribe(&field, |this, _, _: &EditorEvent, cx| {
                    this.menu_index = 0;
                    cx.notify();
                }));
            fields.push(field);
        }
        let mut library = Library::new(config_dir().join("snippets.json"));
        let error = library.refresh().err();
        self.snippets = Some(SnippetUi {
            library,
            fields: fields::Fields::new(self.initial_text.clone()),
            name: fields.remove(0),
            query: fields.remove(0),
            notice: None,
            error,
            deleting: false,
            menu_focus: cx.focus_handle(),
            previous_focus: None,
        });
        if self.input.read(cx).text().is_empty() {
            self.input.update(cx, |e, cx| {
                e.set_text("Hello {{name}},\n\n{{selection}}".into(), cx)
            });
        }
    }
    fn new_snippet(&mut self, cx: &mut Context<Self>) {
        let Some(ui) = self.snippets.as_mut() else {
            return;
        };
        ui.library.new_draft();
        ui.fields.reset();
        ui.notice = None;
        ui.error = None;
        ui.name.update(cx, |e, cx| e.set_text(String::new(), cx));
        self.second
            .update(cx, |e, cx| e.set_text(String::new(), cx));
        self.input.update(cx, |e, cx| e.set_text(String::new(), cx));
        self.open_menu = None;
        self.run_tool(cx);
    }
    fn load_snippet(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(ui) = self.snippets.as_mut() else {
            return;
        };
        match ui.library.load(id) {
            Ok(item) => {
                ui.fields.reset();
                ui.name.update(cx, |e, cx| e.set_text(item.name, cx));
                ui.notice = None;
                ui.error = None;
                self.second
                    .update(cx, |e, cx| e.set_text(String::new(), cx));
                self.input.update(cx, |e, cx| e.set_text(item.body, cx));
                self.run_tool(cx);
            }
            Err(error) => ui.error = Some(error),
        }
        self.open_menu = None;
        cx.notify();
    }
    fn save_snippet(&mut self, cx: &mut Context<Self>) {
        let Some(ui) = self.snippets.as_mut() else {
            return;
        };
        match ui
            .library
            .save(ui.name.read(cx).text(), self.input.read(cx).text())
        {
            Ok(()) => {
                ui.notice = Some(
                    if cfg!(target_os = "macos") {
                        "Snippet saved on this Mac."
                    } else {
                        "Snippet saved on this computer."
                    }
                    .into(),
                );
                ui.error = None;
            }
            Err(error) => {
                ui.error = Some(error);
                ui.notice = None;
            }
        }
        cx.notify();
    }
    fn confirm_delete_snippet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.snippets.as_mut() else {
            return;
        };
        if ui.deleting {
            return;
        }
        let Some(id) = ui.library.selected.clone() else {
            return;
        };
        ui.deleting = true;
        let answer = window.prompt(
            gpui::PromptLevel::Warning,
            "Delete this snippet?",
            None,
            &["Cancel", "Delete"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            let confirmed = matches!(answer.await, Ok(1));
            let _ = this.update(cx, |this, cx| {
                let Some(ui) = this.snippets.as_mut() else {
                    return;
                };
                ui.deleting = false;
                if confirmed {
                    let still_selected = ui.library.selected.as_deref() == Some(&id);
                    match ui.library.delete(&id) {
                        Ok(()) => {
                            if still_selected {
                                this.new_snippet(cx);
                            }
                            let ui = this.snippets.as_mut().unwrap();
                            ui.notice = Some("Snippet deleted.".into());
                            ui.error = None;
                        }
                        Err(error) => {
                            ui.error = Some(error);
                            ui.notice = None;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn close_snippet_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu = None;
        if let Some(ui) = self.snippets.as_mut() {
            let previous_focus = ui.previous_focus.take();
            // Outside clicks may have already focused another input. Never steal it.
            if ui.menu_focus.is_focused(window) {
                if let Some(focus) = previous_focus {
                    focus.focus(window);
                } else {
                    window.blur();
                }
            }
        }
        cx.notify();
    }
    pub(super) fn snippet_menu_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ui) = self.snippets.as_ref() else {
            return;
        };
        let ids: Vec<String> = ui
            .library
            .filtered(ui.query.read(cx).text())
            .iter()
            .map(|s| s.id.clone())
            .collect();
        match key {
            "escape" => self.close_snippet_menu(window, cx),
            "up" => self.menu_index = self.menu_index.saturating_sub(1),
            "down" => self.menu_index = (self.menu_index + 1).min(ids.len().saturating_sub(1)),
            "enter" => {
                if let Some(id) = ids.get(self.menu_index) {
                    self.load_snippet(id, cx);
                    self.close_snippet_menu(window, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }
    pub(super) fn snippet_appearance(&mut self, p: crate::theme::Palette, cx: &mut Context<Self>) {
        if let Some(ui) = &self.snippets {
            for field in [&ui.name, &ui.query] {
                if field.read(cx).appearance().text != p.primary {
                    field.update(cx, |e, cx| {
                        let mut style = e.appearance().clone();
                        style.text = p.primary;
                        style.caret = p.accent;
                        style.selection = p.accent.opacity(0.18);
                        e.set_appearance(style, cx);
                    });
                }
            }
        }
    }
    pub(super) fn snippet_controls(
        &self,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(ui) = &self.snippets else {
            return div().into_any_element();
        };
        let open = self.open_menu == Some("snippets-library");
        let popup = div()
            .id("snippet-library-popup")
            .track_focus(&ui.menu_focus)
            .max_h(px(250.))
            .min_w(px(220.))
            .overflow_y_scroll()
            .p(px(4.))
            .rounded(px(7.))
            .bg(p.surface)
            .border_1()
            .border_color(p.separator)
            .shadow_md()
            .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                this.close_snippet_menu(window, cx);
            }))
            .when(ui.library.items.is_empty(), |s| {
                s.child(
                    div()
                        .p(px(8.))
                        .text_color(p.secondary)
                        .child("Save your first snippet below"),
                )
            })
            .children(
                ui.library
                    .filtered(ui.query.read(cx).text())
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let id = item.id.clone();
                        div()
                            .id(("saved-snippet", index))
                            .px(px(8.))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .when(index == self.menu_index, |s| s.bg(p.accent.opacity(0.1)))
                            .hover(move |s| s.bg(p.accent.opacity(0.12)))
                            .child(item.name.clone())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.load_snippet(&id, cx);
                                this.close_snippet_menu(window, cx);
                                cx.stop_propagation();
                            }))
                    }),
            );
        let can_save = Library::can_save(ui.name.read(cx).text(), self.input.read(cx).text());
        div().flex().flex_col().gap(px(10.)).text_size(px(12.))
            .child(div().flex().gap(px(8.)).items_center()
                .child(div().flex_1().min_w_0().child(field(ui.query.clone(), "Find a saved snippet…", p, cx)))
                .child(div().relative().child(button("saved-snippets", format!("Saved Snippets ({})  ⌄", ui.library.items.len()), p)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if open { this.close_snippet_menu(window, cx); }
                        else if let Some(ui) = this.snippets.as_mut() {
                            ui.error = ui.library.refresh().err();
                            ui.previous_focus = window.focused(cx);
                            ui.menu_focus.focus(window);
                            this.open_menu = Some("snippets-library");
                            this.menu_index = 0;
                        }
                        cx.notify();
                    })))
                    .when(open, |s| s.child(gpui::deferred(gpui::anchored().position_mode(gpui::AnchoredPositionMode::Local)
                        .offset(gpui::point(px(0.), px(34.))).snap_to_window().child(popup)).with_priority(20)))))
            .child(div().flex().gap(px(8.)).items_center()
                .child(div().flex_1().min_w_0().child(field(ui.name.clone(), "Snippet name", p, cx)))
                .child(button("new-snippet", "New", p).on_click(cx.listener(|this, _, _, cx| this.new_snippet(cx))))
                .child(button("save-snippet", "Save Snippet", p).bg(p.accent_fill).text_color(gpui::white())
                    .when(!can_save, |s| s.opacity(0.45).cursor_default())
                    .on_click(cx.listener(move |this, _, _, cx| { if can_save { this.save_snippet(cx); } })))
                .when(ui.library.selected.is_some(), |s| s.child(button("delete-snippet", "Delete…", p)
                    .on_click(cx.listener(|this, _, window, cx| this.confirm_delete_snippet(window, cx))))))
            .child(div().text_size(px(11.)).text_color(p.secondary).child("Use {{name}} for a field, or {{selection}}, {{date}}, {{timestamp}}, and {{uuid}}. Fill fields below and copy or replace with the preview."))
            .when_some(ui.error.clone().or(ui.notice.clone()), |s, message| s.child(div().text_size(px(11.)).text_color(if ui.error.is_some() { p.danger } else { p.secondary }).child(message)))
            .into_any_element()
    }
}
fn field(
    editor: Entity<EditorView>,
    placeholder: &'static str,
    p: crate::theme::Palette,
    cx: &App,
) -> gpui::Div {
    let empty = editor.read(cx).text().is_empty();
    let focus = editor.clone();
    div()
        .relative()
        .h(px(30.))
        .px(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .overflow_hidden()
        .child(editor)
        .when(empty, |s| {
            s.child(
                div()
                    .absolute()
                    .left(px(8.))
                    .top(px(4.))
                    .text_size(px(13.))
                    .text_color(p.secondary)
                    .child(placeholder),
            )
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            focus.read(cx).focus(window)
        })
}
