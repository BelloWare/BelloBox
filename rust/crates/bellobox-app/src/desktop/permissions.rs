//! Source permission grid and read-only visual. No filesystem/process operations.
use super::*;
use crate::tool_controls::{PERMISSION_TOGGLES, PermissionToggleSpec};
use gpui::Focusable;

pub(super) struct PermissionUi {
    value: Entity<EditorView>,
    help: Entity<EditorView>,
    focus: Vec<gpui::FocusHandle>,
}
const HELP: &str = "Changes apply to this preview only. Copy the command when ready.";
struct PermissionTip {
    text: &'static str,
    palette: crate::theme::Palette,
}
impl Render for PermissionTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p(px(8.))
            .max_w(px(300.))
            .rounded(px(6.))
            .bg(self.palette.surface)
            .border_1()
            .border_color(self.palette.separator)
            .text_color(self.palette.primary)
            .text_size(px(12.))
            .child(self.text)
    }
}
impl BelloBox {
    pub(super) fn init_permissions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != "chmod" {
            return;
        }
        let p = crate::theme::for_window(window);
        let make = |text: String, large: bool, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut editor = EditorView::new(text, window, cx);
                editor.set_read_only(true, cx);
                editor.set_compact(large, cx);
                let mut style = EditorAppearance::plain();
                style.font_family = if large {
                    "monospace".into()
                } else {
                    crate::theme::ui_font().into()
                };
                style.font_size = if large { 25. } else { 12. };
                style.line_height = if large { 32. } else { 18. };
                style.padding_x = 0.;
                style.padding_y = 0.;
                style.text = if large { p.primary } else { p.secondary };
                style.selection = p.accent.opacity(0.18);
                style.caret = p.accent;
                editor.set_appearance(style, cx);
                editor
            })
        };
        self.permissions = Some(PermissionUi {
            value: make(String::new(), true, window, cx),
            help: make(HELP.into(), false, window, cx),
            focus: (0..12)
                .map(|index| cx.focus_handle().tab_stop(true).tab_index(index + 1))
                .collect(),
        });
    }
    pub(super) fn clear_permission_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = &self.permissions {
            ui.value.update(cx, |e, cx| e.set_text(String::new(), cx));
        }
    }
    pub(super) fn sync_permission_preview(&mut self, cx: &mut Context<Self>) {
        #[cfg(feature = "developer-tools")]
        if self.selected == "chmod" && self.error.is_none() {
            use bellobox_core::developer::permissions;
            if let (Some(ui), Ok(bits)) = (
                &self.permissions,
                permissions::parse(self.input.read(cx).text()),
            ) {
                ui.value.update(cx, |e, cx| {
                    e.set_text(
                        format!(
                            "{}  {}",
                            permissions::octal(bits),
                            permissions::symbolic(bits)
                        ),
                        cx,
                    )
                });
            }
        }
        #[cfg(not(feature = "developer-tools"))]
        let _ = cx;
    }
    pub(super) fn permission_appearance(
        &mut self,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) {
        let Some(ui) = &self.permissions else {
            return;
        };
        for (editor, ink) in [(&ui.value, p.primary), (&ui.help, p.secondary)] {
            if editor.read(cx).appearance().text != ink {
                editor.update(cx, |e, cx| {
                    let mut style = e.appearance().clone();
                    style.text = ink;
                    style.caret = p.accent;
                    style.selection = p.accent.opacity(0.18);
                    e.set_appearance(style, cx);
                });
            }
        }
    }
    fn permission_bits(&self, cx: &App) -> u16 {
        #[cfg(feature = "developer-tools")]
        {
            bellobox_core::developer::permissions::parse(self.input.read(cx).text()).unwrap_or(0)
        }
        #[cfg(not(feature = "developer-tools"))]
        {
            let _ = cx;
            0
        }
    }
    fn toggle_permission(&mut self, mask: u16, cx: &mut Context<Self>) {
        #[cfg(feature = "developer-tools")]
        {
            let input = self.input.read(cx).text();
            let enabled = self.permission_bits(cx) & mask == 0;
            if let Ok(value) = bellobox_core::developer::permissions::toggle(input, mask, enabled) {
                self.input.update(cx, |e, cx| e.set_text(value, cx));
            }
        }
        #[cfg(not(feature = "developer-tools"))]
        let _ = (mask, cx);
    }
    fn permission_checkbox(
        &self,
        index: usize,
        spec: PermissionToggleSpec,
        bits: u16,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let checked = bits & spec.mask != 0;
        let focus = self
            .permissions
            .as_ref()
            .expect("chmod grid initialized")
            .focus[index]
            .clone();
        let click_focus = focus.clone();
        div()
            .id(("permission-bit", index))
            .track_focus(&focus)
            .key_context("PermissionToggle")
            .rounded(px(4.))
            .border_1()
            .border_color(p.separator.opacity(0.))
            .focus(move |style| style.border_color(p.accent))
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(12.))
            .cursor_pointer()
            .child(
                div()
                    .size(px(14.))
                    .flex_none()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(p.separator)
                    .bg(if checked { p.accent_fill } else { p.well })
                    .text_color(if checked { gpui::white() } else { p.primary })
                    .text_size(px(11.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if checked { "✓" } else { "" }),
            )
            .child(spec.label)
            .tooltip(move |_, cx| {
                cx.new(|_| PermissionTip {
                    text: spec.help,
                    palette: p,
                })
                .into()
            })
            // GPUI also sends KeyboardClickEvent here on Space/Return release.
            // Do not add a key-down toggle: that would apply the same bit twice.
            .on_click(cx.listener(move |this, _, window, cx| {
                click_focus.focus(window);
                this.toggle_permission(spec.mask, cx);
                cx.stop_propagation();
            }))
    }
    pub(super) fn permission_tab(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.selected != "chmod"
            || !cfg!(feature = "developer-tools")
            || event.keystroke.key != "tab"
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.alt
        {
            return false;
        }
        let Some(ui) = &self.permissions else {
            return false;
        };
        let input_focus = self.input.read(cx).focus_handle(cx);
        if input_focus.is_focused(window) && self.input.read(cx).has_marked_text() {
            return false;
        }
        let current = if input_focus.is_focused(window) {
            Some(0)
        } else {
            ui.focus
                .iter()
                .position(|focus| focus.is_focused(window))
                .map(|index| index + 1)
        };
        let next =
            crate::tool_controls::permission_tab_target(current, event.keystroke.modifiers.shift);
        if next == 0 {
            input_focus.focus(window);
        } else {
            ui.focus[next - 1].focus(window);
        }
        cx.stop_propagation();
        cx.notify();
        true
    }
    pub(super) fn permission_grid(
        &self,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        if !cfg!(feature = "developer-tools") || self.permissions.is_none() {
            return div();
        }
        let bits = self.permission_bits(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div().flex().gap(px(12.)).children(
                    ["Owner", "Group", "Others"]
                        .into_iter()
                        .enumerate()
                        .map(|(group, title)| {
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(p.secondary)
                                        .child(title),
                                )
                                .child(div().flex().gap(px(8.)).children(
                                    (group * 3..group * 3 + 3).map(|index| {
                                        self.permission_checkbox(
                                            index,
                                            PERMISSION_TOGGLES[index],
                                            bits,
                                            p,
                                            cx,
                                        )
                                    }),
                                ))
                        }),
                ),
            )
            .child(div().flex().gap(px(14.)).children((9..12).map(|index| {
                self.permission_checkbox(index, PERMISSION_TOGGLES[index], bits, p, cx)
            })))
    }
    pub(super) fn permission_preview(&self, p: crate::theme::Palette) -> gpui::Div {
        let Some(ui) = &self.permissions else {
            return div();
        };
        div()
            .h(px(140.))
            .flex_none()
            .p(px(10.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.separator)
            .bg(p.well)
            .flex()
            .gap(px(10.))
            .child(
                div()
                    .size(px(25.))
                    .flex_none()
                    .child(crate::theme::tool_icon("chmod", 25., p)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().h(px(32.)).flex_none().child(ui.value.clone()))
                    .child(div().h(px(40.)).flex_none().child(ui.help.clone())),
            )
    }
}
