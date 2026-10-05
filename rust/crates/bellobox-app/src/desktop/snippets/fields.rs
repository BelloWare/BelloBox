//! Source field rows, with a bounded viewport for exceptionally large templates.
use super::*;
use gpui::{EntityInputHandler, Focusable, UniformListScrollHandle, uniform_list};
use std::{collections::BTreeMap, ops::Range};

const ROW_HEIGHT: f32 = 38.;
const VIEWPORT_ROWS: usize = 8;
struct FieldSlot {
    editor: Entity<EditorView>,
    _subscription: Subscription,
}
pub(super) struct Fields {
    session: bellobox_core::snippets::Session,
    template: Option<String>,
    generation: u64,
    names: Vec<String>,
    row_height: f32,
    editors: BTreeMap<String, FieldSlot>,
    scroll: UniformListScrollHandle,
}
impl Fields {
    pub(super) fn new(selection: String) -> Self {
        Self {
            session: bellobox_core::snippets::Session::new(selection),
            template: None,
            generation: 0,
            names: Vec::new(),
            row_height: ROW_HEIGHT,
            editors: BTreeMap::new(),
            scroll: UniformListScrollHandle::new(),
        }
    }
    pub(super) fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.session.reset_fields();
        self.editors.clear();
        self.template = None;
        self.scroll = UniformListScrollHandle::new();
    }
}
fn viewport_height(count: usize) -> f32 {
    count.min(VIEWPORT_ROWS) as f32 * ROW_HEIGHT
}
fn keep_editor(visible: &[String], name: &str, focused: bool, composing: bool) -> bool {
    focused || composing || visible.iter().any(|field| field == name)
}
fn should_evict(
    measuring: bool,
    visible: &[String],
    name: &str,
    focused: bool,
    composing: bool,
) -> bool {
    !measuring && !keep_editor(visible, name, focused, composing)
}
fn block_outside_click(focused: bool, composing: bool) -> bool {
    focused && composing
}
fn pinned_row(names: &[String], active: &[(&str, bool, bool)]) -> Option<usize> {
    active
        .iter()
        .filter(|(_, focused, _)| *focused)
        .chain(
            active
                .iter()
                .filter(|(_, focused, composing)| !focused && *composing),
        )
        .find_map(|(name, _, _)| names.iter().position(|field| field == name))
}
impl BelloBox {
    pub(in crate::desktop) fn snippet_snapshot(&self) -> Option<bellobox_core::snippets::Session> {
        self.snippets.as_ref().map(|ui| ui.fields.session.clone())
    }
    pub(in crate::desktop) fn sync_snippet_fields(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ui) = self.snippets.as_mut() else {
            return;
        };
        let template = self.input.read(cx).text();
        if ui.fields.template.as_deref() != Some(template) {
            ui.fields.names = bellobox_core::snippets::custom_fields(template).unwrap_or_default();
            ui.fields.row_height = ROW_HEIGHT;
            if ui.fields.names.len() > VIEWPORT_ROWS {
                let mut style = window.text_style();
                style.font_family = crate::theme::ui_font().into();
                for name in &ui.fields.names {
                    let lines = window
                        .text_system()
                        .shape_text(
                            name.clone().into(),
                            px(12.),
                            &[style.to_run(name.len())],
                            Some(px(130.)),
                            None,
                        )
                        .map(|lines| {
                            lines
                                .iter()
                                .map(|line| line.wrap_boundaries.len() + 1)
                                .sum::<usize>()
                        })
                        .unwrap_or(name.len().div_ceil(10));
                    ui.fields.row_height = ui.fields.row_height.max(lines as f32 * 18. + 8.);
                }
            }
            ui.fields.template = Some(template.into());
        }
    }
    pub(in crate::desktop) fn snippet_fields(
        &mut self,
        p: crate::theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(ui) = self.snippets.as_ref() else {
            return div().into_any_element();
        };
        let count = ui.fields.names.len();
        if count == 0 {
            return div().into_any_element();
        }
        let content = if count <= VIEWPORT_ROWS {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(self.snippet_field_rows(0..count, false, p, window, cx))
                .into_any_element()
        } else {
            uniform_list(
                "snippet-template-fields",
                count,
                cx.processor(move |this, range: Range<usize>, window, cx| {
                    this.snippet_field_rows(range, true, p, window, cx)
                }),
            )
            .h(px(viewport_height(count)))
            .flex_none()
            .track_scroll(ui.fields.scroll.clone())
            .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(p.secondary)
                    .child("Fill template fields"),
            )
            .child(content)
            .into_any_element()
    }

    fn snippet_field_rows(
        &mut self,
        range: Range<usize>,
        virtualized: bool,
        p: crate::theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Div> {
        let Some(ui) = self.snippets.as_mut() else {
            return Vec::new();
        };
        let measuring = virtualized && range == (0..1);
        let visible = ui.fields.names.get(range).unwrap_or_default().to_vec();
        // GPUI measures row zero BEFORE applying deferred scrolling. Pin active
        // input here so its real row stays mounted/painted, including its IME handler.
        if measuring {
            let active = ui
                .fields
                .editors
                .iter()
                .map(|(name, slot)| {
                    let focused = slot.editor.read(cx).focus_handle(cx).is_focused(window);
                    let composing = slot.editor.update(cx, |editor, cx| {
                        editor.marked_text_range(window, cx).is_some()
                    });
                    (name.as_str(), focused, composing)
                })
                .collect::<Vec<_>>();
            if let Some(index) = pinned_row(&ui.fields.names, &active) {
                ui.fields
                    .scroll
                    .scroll_to_item(index, gpui::ScrollStrategy::Top);
            }
        }
        let mut remove = Vec::new();
        for (name, slot) in &ui.fields.editors {
            let focused = slot.editor.read(cx).focus_handle(cx).is_focused(window);
            let composing = slot.editor.update(cx, |editor, cx| {
                editor.marked_text_range(window, cx).is_some()
            });
            if should_evict(measuring, &visible, name, focused, composing) {
                // Synchronize before dropping an offscreen entity, even if its event is queued.
                let value = slot.editor.read(cx).text();
                if ui.fields.session.value(name).is_some() || !value.is_empty() {
                    ui.fields.session.set_value(name.clone(), value.into());
                }
                remove.push(name.clone());
            }
        }
        for name in remove {
            ui.fields.editors.remove(&name);
        }
        let mut rows = Vec::with_capacity(visible.len());
        for name in visible {
            if !ui.fields.editors.contains_key(&name) {
                let value = ui.fields.session.value(&name).unwrap_or("").to_owned();
                let editor = cx.new(|cx| {
                    let mut editor = EditorView::new(value, window, cx);
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
                let key = name.clone();
                let generation = ui.fields.generation;
                let subscription =
                    cx.subscribe(&editor, move |this, editor, event: &EditorEvent, cx| {
                        if matches!(event, EditorEvent::Changed) {
                            if let Some(ui) = this.snippets.as_mut() {
                                if ui.fields.generation != generation {
                                    return;
                                }
                                ui.fields
                                    .session
                                    .set_value(key.clone(), editor.read(cx).text().into());
                            }
                            this.run_tool(cx);
                        }
                    });
                ui.fields.editors.insert(
                    name.clone(),
                    FieldSlot {
                        editor,
                        _subscription: subscription,
                    },
                );
            }
            let editor = ui.fields.editors[&name].editor.clone();
            if editor.read(cx).appearance().text != p.primary {
                editor.update(cx, |e, cx| {
                    let mut style = e.appearance().clone();
                    style.text = p.primary;
                    style.caret = p.accent;
                    style.selection = p.accent.opacity(0.18);
                    e.set_appearance(style, cx);
                });
            }
            let outside_editor = editor.clone();
            let value_field =
                super::field(editor, "Value", p, cx).on_mouse_down_out(move |_, window, cx| {
                    let focused = outside_editor.read(cx).focus_handle(cx).is_focused(window);
                    let composing = outside_editor.update(cx, |editor, cx| {
                        editor.marked_text_range(window, cx).is_some()
                    });
                    // GPUI blur is not an IME commit/cancel. Keep the actual marked
                    // span and input handler until the platform completes composition.
                    if block_outside_click(focused, composing) {
                        cx.stop_propagation();
                    } else if focused && outside_editor.read(cx).focus_handle(cx).is_focused(window)
                    {
                        // Do not steal focus from a newer clicked input.
                        window.blur();
                    }
                });
            rows.push(
                div()
                    .min_h(px(30.))
                    .when(virtualized, |s| s.h(px(ui.fields.row_height)).pb(px(8.)))
                    .w_full()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .w(px(130.))
                            .flex_none()
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .child(name),
                    )
                    .child(div().flex_1().min_w_0().child(value_field)),
            );
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_viewport_has_zero_small_and_bounded_large_heights() {
        assert_eq!(viewport_height(0), 0.);
        assert_eq!(viewport_height(3), 114.);
        assert_eq!(viewport_height(10_000), viewport_height(VIEWPORT_ROWS));
    }
    #[test]
    fn measurement_never_evicts_actual_viewport_editors() {
        let measure = vec!["field0".into()];
        for _ in 0..10 {
            assert!(!should_evict(true, &measure, "field999", false, false));
        }
        assert!(!should_evict(
            false,
            &["field999".into()],
            "field999",
            false,
            false
        ));
        assert!(should_evict(false, &measure, "field999", false, false));
    }
    #[test]
    fn outside_click_preserves_live_composition_until_platform_finishes() {
        assert!(block_outside_click(true, true));
        assert!(!block_outside_click(true, false));
        assert!(!block_outside_click(false, true));
        assert!(!block_outside_click(false, false));
    }
    #[test]
    fn focused_and_composing_rows_pin_until_outside_click_releases_them() {
        let names = vec!["first".into(), "middle".into(), "last".into()];
        assert_eq!(pinned_row(&names, &[("last", true, false)]), Some(2));
        assert_eq!(pinned_row(&names, &[("last", false, true)]), Some(2));
        assert_eq!(
            pinned_row(&names, &[("first", false, true), ("last", true, false)]),
            Some(2)
        );
        assert_eq!(pinned_row(&names, &[("last", false, false)]), None);
        assert_eq!(pinned_row(&names, &[("removed", true, true)]), None);
    }
    #[test]
    fn field_cache_keeps_only_visible_focused_or_composing_rows() {
        let mut cache = BTreeMap::new();
        for start in (0..10_000).step_by(VIEWPORT_ROWS) {
            let visible = (start..(start + VIEWPORT_ROWS).min(10_000))
                .map(|i| format!("field{i}"))
                .collect::<Vec<_>>();
            cache.retain(|name: &String, _| {
                keep_editor(&visible, name, name == "field0", name == "field1")
            });
            for name in visible {
                cache.insert(name, ());
            }
            assert!(cache.len() <= VIEWPORT_ROWS + 2);
        }
        assert!(cache.contains_key("field0"));
        assert!(cache.contains_key("field1"));
        assert!(cache.contains_key("field9999"));
    }
    #[test]
    fn new_or_load_reset_rejects_old_events_without_changing_session_identity() {
        let mut fields = Fields::new("selected".into());
        let id = fields.session.render("{{uuid}}").unwrap();
        fields.session.set_value("name".into(), "old".into());
        fields.template = Some("{{name}}".into());
        let old_generation = fields.generation;
        fields.reset();
        assert_ne!(fields.generation, old_generation);
        assert!(fields.template.is_none());
        assert!(fields.editors.is_empty());
        assert_eq!(
            fields
                .session
                .render("{{name}} {{selection}} {{uuid}}")
                .unwrap(),
            format!("{{{{name}}}} selected {id}")
        );
    }
    #[test]
    fn value_state_survives_eviction_and_template_removal_then_resets() {
        let mut session = bellobox_core::snippets::Session::new("selection".into());
        session.set_value("name".into(), "draft 🦊".into());
        assert!(!keep_editor(&["other".into()], "name", false, false));
        assert!(
            bellobox_core::snippets::custom_fields("plain")
                .unwrap()
                .is_empty()
        );
        assert_eq!(session.value("name"), Some("draft 🦊"));
        assert_eq!(session.render("{{name}}").unwrap(), "draft 🦊");
        session.reset_fields();
        assert_eq!(session.value("name"), None);
    }
}
