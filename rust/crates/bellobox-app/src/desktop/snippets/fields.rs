//! Source field rows, with a bounded viewport for exceptionally large templates.
use super::*;
use bello_workbench_ui::{EditStateError, EditorEditState};
use gpui::{EntityInputHandler, Focusable, UniformListScrollHandle, uniform_list};
use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
};

const ROW_HEIGHT: f32 = 38.;
const VIEWPORT_ROWS: usize = 8;
const COLD_STATE_BYTES: usize = 16 * 1024 * 1024;
const COLD_STATE_COUNT: usize = 64;

// Only inactive model state is retained here, never GPUI entities/platform handles.
// The byte bound is EditorEditState accounting, not an allocator/RSS claim.
struct ColdEntry<T> {
    generation: u64,
    value: T,
    bytes: usize,
}
struct ColdStates<T> {
    entries: BTreeMap<String, ColdEntry<T>>,
    order: VecDeque<String>,
    bytes: usize,
    byte_limit: usize,
    count_limit: usize,
}
impl<T> ColdStates<T> {
    fn new(byte_limit: usize, count_limit: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            byte_limit,
            count_limit,
        }
    }
    fn remove(&mut self, name: &str) -> Option<ColdEntry<T>> {
        let entry = self.entries.remove(name)?;
        self.order.retain(|key| key != name);
        self.bytes -= entry.bytes;
        Some(entry)
    }
    fn take(&mut self, name: &str, generation: u64) -> Option<T> {
        self.remove(name)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.value)
    }
    fn insert(&mut self, name: String, generation: u64, value: T, bytes: usize) -> bool {
        self.remove(&name);
        if bytes > self.byte_limit || self.count_limit == 0 {
            return false;
        }
        while self.bytes > self.byte_limit - bytes || self.entries.len() >= self.count_limit {
            let oldest = self
                .order
                .front()
                .expect("entries have recency keys")
                .clone();
            self.remove(&oldest);
        }
        self.bytes += bytes;
        self.order.push_back(name.clone());
        self.entries.insert(
            name,
            ColdEntry {
                generation,
                value,
                bytes,
            },
        );
        true
    }
    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }
}
fn current_mount(
    generation: u64,
    mount: u64,
    current_generation: u64,
    current: Option<u64>,
) -> bool {
    generation == current_generation && current == Some(mount)
}
struct FieldSlot {
    editor: Entity<EditorView>,
    mount: u64,
    _subscription: Subscription,
}
pub(super) struct Fields {
    session: bellobox_core::snippets::Session,
    template: Option<String>,
    generation: u64,
    names: Vec<String>,
    row_height: f32,
    editors: BTreeMap<String, FieldSlot>,
    cold: ColdStates<EditorEditState>,
    next_mount: u64,
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
            cold: ColdStates::new(COLD_STATE_BYTES, COLD_STATE_COUNT),
            next_mount: 0,
            scroll: UniformListScrollHandle::new(),
        }
    }
    pub(super) fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.session.reset_fields();
        self.editors.clear();
        self.cold.clear();
        self.next_mount = 0;
        self.template = None;
        self.scroll = UniformListScrollHandle::new();
    }
}
fn viewport_height(count: usize) -> f32 {
    count.min(VIEWPORT_ROWS) as f32 * ROW_HEIGHT
}
fn virtual_viewport_height(count: usize, row_height: f32) -> f32 {
    // Virtual lists have >8 items. Keeping at least two rows in their layout
    // bounds means an actual viewport cannot be the ambiguous range 0..1.
    viewport_height(count).max(row_height * 2.)
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
            .h(px(virtual_viewport_height(count, ui.fields.row_height)))
            .min_h(px(ui.fields.row_height * 2.))
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
        // Height + min-height >= two row heights, with no list padding/border,
        // makes 0..1 measurement-only even when the outer scroll clips the list.
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
                    let blocker = slot.editor.read(cx).suspension_blocker(window);
                    (
                        name.as_str(),
                        blocker == Some(EditStateError::Focused),
                        blocker.is_some(),
                    )
                })
                .collect::<Vec<_>>();
            if let Some(index) = pinned_row(&ui.fields.names, &active) {
                ui.fields
                    .scroll
                    .scroll_to_item(index, gpui::ScrollStrategy::Top);
            }
            // The row height is known. Measurement must not create a view, consume
            // a cold state or refresh its recency merely because row zero is sampled.
            return vec![div().w_full().h(px(ui.fields.row_height))];
        }
        let remove = ui
            .fields
            .editors
            .iter()
            .filter_map(|(name, slot)| {
                let blocker = slot.editor.read(cx).suspension_blocker(window);
                should_evict(
                    measuring,
                    &visible,
                    name,
                    blocker == Some(EditStateError::Focused),
                    blocker.is_some(),
                )
                .then(|| name.clone())
            })
            .collect::<Vec<_>>();
        for name in remove {
            // Removing the slot first invalidates its mount token before any queued event.
            let slot = ui
                .fields
                .editors
                .remove(&name)
                .expect("selected mounted field");
            let view = slot.editor.read(cx);
            let edited = ui.fields.session.value(&name).is_some() || view.engine.revision() != 0;
            let value = view.text().to_owned();
            if edited || !value.is_empty() {
                ui.fields.session.set_value(name.clone(), value);
            }
            match slot
                .editor
                .update(cx, |editor, cx| editor.take_edit_state(window, cx))
            {
                Ok(state) => {
                    // The detached shell/subscription are dropped; only plain state survives.
                    drop(slot);
                    if edited {
                        let bytes = state.accounted_bytes();
                        ui.fields
                            .cold
                            .insert(name, ui.fields.generation, state, bytes);
                    }
                }
                Err(_) => {
                    // Atomic API failure leaves the original view intact. Keep it live and
                    // let the next measurement pin its newly active focus/IME/drag state.
                    ui.fields.editors.insert(name, slot);
                    cx.notify();
                }
            }
        }
        let mut rows = Vec::with_capacity(visible.len());
        for name in visible {
            if !ui.fields.editors.contains_key(&name) {
                let value = ui.fields.session.value(&name).unwrap_or("").to_owned();
                let mut retained = ui.fields.cold.take(&name, ui.fields.generation);
                let mut restore_failed = false;
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
                    // Configure before restore: mode setters can finish pending undo groups.
                    if retained.is_some()
                        && editor
                            .restore_edit_state(&mut retained, window, cx)
                            .is_err()
                    {
                        restore_failed = true;
                    }
                    editor
                });
                if restore_failed {
                    ui.error = Some("This field's undo history could not be restored. Its current value was kept.".into());
                }
                let key = name.clone();
                let generation = ui.fields.generation;
                ui.fields.next_mount = ui.fields.next_mount.wrapping_add(1);
                let mount = ui.fields.next_mount;
                let subscription =
                    cx.subscribe(&editor, move |this, editor, event: &EditorEvent, cx| {
                        if matches!(event, EditorEvent::Changed) {
                            if let Some(ui) = this.snippets.as_mut() {
                                if !current_mount(
                                    generation,
                                    mount,
                                    ui.fields.generation,
                                    ui.fields.editors.get(&key).map(|slot| slot.mount),
                                ) {
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
                        mount,
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
    fn virtual_layout_always_contains_multiple_rows_even_for_tall_labels() {
        for row_height in [38., 152., 304., 400., 1088.] {
            let height = virtual_viewport_height(12, row_height);
            assert!(height >= row_height * 2.);
            assert!((height / row_height).ceil() >= 2.);
        }
        assert_eq!(virtual_viewport_height(12, ROW_HEIGHT), 304.);
        assert_eq!(virtual_viewport_height(12, 400.), 800.);
    }
    #[test]
    fn cold_state_limits_are_exact_and_replace_accounting_stays_consistent() {
        let mut cold = ColdStates::new(10, 2);
        assert!(cold.insert("a".into(), 1, 10, 5));
        assert!(cold.insert("b".into(), 1, 20, 5));
        assert_eq!(cold.bytes, 10);
        assert!(cold.insert("c".into(), 1, 30, 5));
        assert!(cold.take("a", 1).is_none());
        assert_eq!(cold.entries.len(), 2);
        assert!(cold.insert("b".into(), 1, 21, 1));
        assert_eq!(cold.bytes, 6);
        assert_eq!(
            cold.order.iter().map(String::as_str).collect::<Vec<_>>(),
            vec!["c", "b"]
        );
        assert_eq!(cold.take("b", 1), Some(21));
        assert_eq!(cold.bytes, 5);
        assert!(!cold.insert("huge".into(), 1, 99, usize::MAX));
        assert_eq!(cold.bytes, 5);
        assert!(cold.insert("edge".into(), 1, 100, 10));
        assert_eq!(cold.bytes, 10);
        assert_eq!(cold.entries.len(), 1);
        assert_eq!(cold.take("edge", 1), Some(100));
        assert_eq!(cold.bytes, 0);
    }
    #[test]
    fn cold_models_move_without_cloning_and_old_epochs_are_rejected() {
        // Opaque non-Clone payload represents the move-only shared edit state.
        struct Payload(Box<[u8]>);
        let state = Payload(vec![1, 2, 3].into_boxed_slice());
        let pointer = state.0.as_ptr();
        let mut cold = ColdStates::new(100, 4);
        cold.insert("field".into(), 7, state, 3);
        let state = cold.take("field", 7).unwrap();
        assert_eq!(state.0.as_ptr(), pointer);
        assert!(cold.entries.is_empty());
        cold.insert("field".into(), 7, state, 3);
        assert!(cold.take("field", 8).is_none());
        assert_eq!(cold.bytes, 0);
        assert!(cold.order.is_empty());
    }
    #[test]
    fn ten_thousand_evictions_bound_history_without_dropping_values() {
        let mut cold = ColdStates::new(16, 8);
        let mut session = bellobox_core::snippets::Session::new(String::new());
        for i in 0..10_000 {
            let name = format!("field{i}");
            session.set_value(name.clone(), format!("value{i}"));
            cold.insert(name, 1, i, 2);
            assert!(cold.bytes <= 16);
            assert!(cold.entries.len() <= 8);
            assert_eq!(cold.entries.len(), cold.order.len());
        }
        assert!(cold.take("field0", 1).is_none());
        assert_eq!(session.value("field0"), Some("value0"));
        assert_eq!(cold.take("field9999", 1), Some(9999));
        assert_eq!(session.value("field9999"), Some("value9999"));
        cold.clear();
        assert_eq!(cold.bytes, 0);
        assert!(cold.entries.is_empty() && cold.order.is_empty());
        assert_eq!(session.value("field0"), Some("value0"));
    }
    #[test]
    fn mount_tokens_fence_evicted_views_even_when_field_and_session_match() {
        assert!(current_mount(3, 8, 3, Some(8)));
        assert!(!current_mount(3, 8, 3, None));
        assert!(!current_mount(3, 8, 3, Some(9)));
        assert!(!current_mount(3, 8, 4, Some(8)));
    }
    #[test]
    fn template_removal_readd_can_retain_state_but_new_session_cannot() {
        let mut cold = ColdStates::new(100, 4);
        let mut session = bellobox_core::snippets::Session::new(String::new());
        session.set_value("name".into(), "value".into());
        cold.insert("name".into(), 2, "retained state", 10);
        assert!(
            bellobox_core::snippets::custom_fields("plain")
                .unwrap()
                .is_empty()
        );
        assert_eq!(session.value("name"), Some("value"));
        assert_eq!(cold.take("name", 2), Some("retained state"));
        cold.insert("name".into(), 2, "retained state", 10);
        session.reset_fields();
        cold.clear();
        assert_eq!(session.value("name"), None);
        assert!(cold.take("name", 3).is_none());
    }
    #[test]
    fn drag_blocker_pins_and_rejects_eviction_like_other_active_input() {
        let blocker = Some(EditStateError::Dragging);
        let focused = blocker == Some(EditStateError::Focused);
        assert!(!should_evict(
            false,
            &["other".into()],
            "drag",
            focused,
            blocker.is_some()
        ));
        assert_eq!(
            pinned_row(
                &["other".into(), "drag".into()],
                &[("drag", focused, blocker.is_some())]
            ),
            Some(1)
        );
    }
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
