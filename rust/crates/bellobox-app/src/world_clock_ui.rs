//! Dedicated offline planner, following WorldClockView and its window lifecycle.
//! Locale-native date controls, Copilot and launcher session handoff remain unported.
use crate::theme::Palette;
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::{
    clock::{self, Planner, Quality, Timeline, ZoneOption},
    settings::{Settings, config_dir},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, collections::HashMap, rc::Rc};

pub const WINDOW_SIZE: (f32, f32) = (920., 740.);
pub const MIN_SIZE: (f32, f32) = (780., 640.);
const PICKER_WIDTH: f32 = 440.;
const DATE_FIELD_WIDTH: f32 = 132.;
const TIME_FIELD_WIDTH: f32 = 88.;

#[derive(Default, Debug)]
struct WheelAccumulator {
    pending: f32,
}
impl WheelAccumulator {
    fn consume(&mut self, x: f32, y: f32, reset: bool) -> Option<i64> {
        if x == 0. || x.abs() < y.abs() {
            return None;
        }
        if reset {
            self.pending = 0.;
        }
        self.pending -= x;
        let steps = (self.pending / 14.).trunc() as i64;
        self.pending -= steps as f32 * 14.;
        Some(steps)
    }
}
fn moved_index(index: usize, direction: i32, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        (index as i64 + direction as i64).rem_euclid(count as i64) as usize
    }
}
fn scrub_fraction(x: f32, left: f32, width: f32) -> f64 {
    ((x - left - 5.) / (width - 10.).max(1.)).clamp(0., 1.) as f64
}
#[derive(Clone, PartialEq, Eq, Hash)]
enum Action {
    Now,
    Day(i64),
    Picker,
    CancelPicker,
    ClearSearch,
    AddSelected,
    Add(String),
    Remove(String),
    Reference(String),
    ReferenceMenu,
    Copy,
    ApplyDate,
    ApplyTime,
    Copilot,
}

pub struct WorldClock {
    planner: Planner,
    displayed_day: Timeline,
    action_focus: HashMap<Action, FocusHandle>,
    consume_enter_release: bool,
    date: Entity<EditorView>,
    time: Entity<EditorView>,
    query: Entity<EditorView>,
    focus: FocusHandle,
    timeline_focus: FocusHandle,
    restore_focus: Option<FocusHandle>,
    picker: bool,
    picker_index: usize,
    picker_scroll: ScrollHandle,
    reference_menu: bool,
    reference_index: usize,
    reference_scroll: ScrollHandle,
    reference_generation: u64,
    reference_reveal_pending: Rc<Cell<bool>>,
    reference_button_bounds: Rc<Cell<Bounds<Pixels>>>,
    status: Option<String>,
    error: Option<String>,
    bands: Vec<Quality>,
    bands_key: String,
    timeline_bounds: Rc<Cell<Bounds<Pixels>>>,
    slider_bounds: Rc<Cell<Bounds<Pixels>>>,
    drag_timeline: Option<Timeline>,
    drag_slider: bool,
    wheel: WheelAccumulator,
    _subscriptions: Vec<Subscription>,
}
impl WorldClock {
    fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = crate::theme::for_window(window);
        let loaded = Settings::load(&config_dir().join("settings.json"));
        let mut error = loaded.as_ref().err().cloned();
        let settings = loaded.unwrap_or_default();
        // Only an explicit routed seed is parsed. Never read clipboard or selection.
        let seed = if input.len() <= 256 {
            clock::parse_instant(&input).ok()
        } else {
            None
        };
        let local = local_zone();
        if local.is_none() && settings.zone_ids.is_empty() && error.is_none() {
            error =
                Some("System time zone unavailable; using UTC for the initial location.".into());
        }
        let local = local.unwrap_or_else(|| "UTC".into());
        let planner = Planner::from_preferences(
            &settings.zone_ids,
            &settings.anchor_zone_id,
            &local,
            clock::current_time(),
            seed,
        )
        .expect("current time and normalized zones are valid");
        let displayed_day = planner.timeline().expect("validated planner day");
        let date = plain_editor(String::new(), 13., p, window, cx);
        let time = plain_editor(String::new(), 13., p, window, cx);
        let query = plain_editor(String::new(), 16., p, window, cx);
        let subscription = cx.subscribe(&query, |this, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::Changed) {
                this.picker_index = 0;
                this.picker_scroll.scroll_to_item(0);
                cx.notify();
            }
        });
        let date_blur = cx.on_blur(
            &date.read(cx).focus_handle(cx),
            window,
            |this, window, cx| {
                if !this.date.read(cx).has_marked_text()
                    && this.date.read(cx).text() != this.displayed_date()
                {
                    this.act(Action::ApplyDate, window, cx);
                }
            },
        );
        let time_blur = cx.on_blur(
            &time.read(cx).focus_handle(cx),
            window,
            |this, window, cx| {
                if !this.time.read(cx).has_marked_text()
                    && this.time.read(cx).text() != this.planner.time_input()
                {
                    this.act(Action::ApplyTime, window, cx);
                }
            },
        );
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.reference_menu {
                this.reference_menu = false;
                // Restore logical keyboard focus without activating this window.
                if let Some(focus) = this.action_focus.get(&Action::ReferenceMenu) {
                    focus.focus(window);
                }
                cx.notify();
            }
        });
        let focus = cx.focus_handle();
        focus.focus(window);
        let mut result = Self {
            planner,
            displayed_day,
            action_focus: HashMap::new(),
            consume_enter_release: false,
            date,
            time,
            query,
            focus,
            timeline_focus: cx.focus_handle(),
            restore_focus: None,
            picker: false,
            picker_index: 0,
            picker_scroll: ScrollHandle::new(),
            reference_menu: false,
            reference_index: 0,
            reference_scroll: ScrollHandle::new(),
            reference_generation: 0,
            reference_reveal_pending: Rc::new(Cell::new(false)),
            reference_button_bounds: Rc::new(Cell::new(Bounds::default())),
            status: None,
            error,
            bands: vec![],
            bands_key: String::new(),
            timeline_bounds: Rc::new(Cell::new(Bounds::default())),
            slider_bounds: Rc::new(Cell::new(Bounds::default())),
            drag_timeline: None,
            drag_slider: false,
            wheel: WheelAccumulator::default(),
            _subscriptions: vec![subscription, date_blur, time_blur, activation],
        };
        result.sync_fields(cx);
        result.refresh_bands();
        cx.spawn_in(window, async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| {
                        let old_date = this.displayed_date();
                        let old_time = this.planner.time_input();
                        if this
                            .planner
                            .refresh_now(clock::current_time())
                            .unwrap_or(false)
                        {
                            this.displayed_day =
                                this.planner.timeline().expect("validated live day");
                            let date = this.displayed_date();
                            let time = this.planner.time_input();
                            if refresh_field(
                                this.date.read(cx).focus_handle(cx).is_focused(window),
                                this.date.read(cx).has_marked_text(),
                                this.date.read(cx).text(),
                                &old_date,
                                &date,
                            ) {
                                this.date.update(cx, |e, cx| e.set_text(date, cx));
                            }
                            if refresh_field(
                                this.time.read(cx).focus_handle(cx).is_focused(window),
                                this.time.read(cx).has_marked_text(),
                                this.time.read(cx).text(),
                                &old_time,
                                &time,
                            ) {
                                this.time.update(cx, |e, cx| e.set_text(time, cx));
                            }
                            this.refresh_bands();
                            this.status = None;
                            // Draft edits survive clock ticks until explicitly applied/cancelled.
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        result
    }
    fn displayed_date(&self) -> String {
        self.displayed_day
            .start
            .with_timezone(&self.planner.reference)
            .format("%Y-%m-%d")
            .to_string()
    }
    fn sync_fields(&mut self, cx: &mut Context<Self>) {
        let date = self.displayed_date();
        let time = self.planner.time_input();
        if !self.date.read(cx).has_marked_text() && self.date.read(cx).text() != date {
            self.date.update(cx, |e, cx| e.set_text(date, cx));
        }
        if !self.time.read(cx).has_marked_text() && self.time.read(cx).text() != time {
            self.time.update(cx, |e, cx| e.set_text(time, cx));
        }
    }
    fn refresh_bands(&mut self) {
        let key = format!(
            "{}:{:?}:{}",
            self.displayed_day.start.timestamp(),
            self.planner.zones,
            self.planner.reference
        );
        if self.bands_key != key {
            let mut band_planner = self.planner.clone();
            // Ask core for the displayed day's bands, including when selectedInstant
            // is its next-midnight endpoint. No new timezone calculation lives here.
            if let Ok(midpoint) = self
                .displayed_day
                .instant_at_offset(self.displayed_day.duration_seconds() as f64 / 2.)
            {
                band_planner.instant = midpoint;
            }
            self.bands = band_planner.timeline_qualities().unwrap_or_default();
            self.bands_key = key;
        }
    }
    fn changed_in_day(
        &mut self,
        result: Result<(), String>,
        preserve_day: bool,
        cx: &mut Context<Self>,
    ) {
        self.error = result.err();
        self.status = None;
        if self.error.is_none() {
            if !preserve_day && let Ok(day) = self.planner.timeline() {
                self.displayed_day = day;
            }
            self.sync_fields(cx);
            self.refresh_bands();
        }
        cx.notify();
    }
    fn persist(&mut self) {
        let ids: Vec<_> = self.planner.zones.iter().map(ToString::to_string).collect();
        if let Err(e) = Settings::save_clock_preferences(
            &config_dir().join("settings.json"),
            &ids,
            self.planner.reference.name(),
        ) {
            self.error = Some(e);
        }
    }
    fn results(&self, cx: &App) -> Vec<ZoneOption> {
        let ids: Vec<_> = self.planner.zones.iter().map(ToString::to_string).collect();
        clock::search_options(self.query.read(cx).text(), &ids)
    }
    fn close_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker = false;
        self.restore_focus
            .take()
            .unwrap_or_else(|| self.focus.clone())
            .focus(window);
        cx.notify();
    }
    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let preserve_day = matches!(&action, Action::Add(_))
            || matches!(&action,Action::Remove(id) if id != self.planner.reference.name());
        let mut save = false;
        let result = match action {
            Action::Now => self.planner.go_to_now(clock::current_time()),
            Action::Day(days) => move_displayed_day(&mut self.planner, &self.displayed_day, days),
            Action::ApplyDate => self.planner.select_date(self.date.read(cx).text()),
            Action::ApplyTime => self.planner.select_time(self.time.read(cx).text()),
            Action::Reference(id) => {
                if self.reference_menu {
                    self.action_focus[&Action::ReferenceMenu].focus(window);
                }
                self.reference_menu = false;
                save = true;
                self.planner.set_reference(&id)
            }
            Action::Remove(id) => {
                save = true;
                self.planner.remove_zone(&id).map(|_| ())
            }
            Action::Add(id) => {
                save = true;
                let result = self.planner.add_zone(&id).map(|_| ());
                if result.is_ok() {
                    self.close_picker(window, cx);
                }
                result
            }
            Action::Picker => {
                self.reference_menu = false;
                self.restore_focus = window.focused(cx);
                self.query.update(cx, |e, cx| e.set_text(String::new(), cx));
                self.picker_index = 0;
                self.picker = true;
                self.query.read(cx).focus(window);
                cx.notify();
                return;
            }
            Action::CancelPicker => {
                self.close_picker(window, cx);
                return;
            }
            Action::ClearSearch => {
                if !self.query.read(cx).text().is_empty() {
                    self.query.update(cx, |e, cx| e.set_text(String::new(), cx));
                }
                self.picker_index = 0;
                self.picker_scroll.scroll_to_item(0);
                self.query.read(cx).focus(window);
                cx.notify();
                return;
            }
            Action::AddSelected => {
                if let Some(id) = selected_location(&self.results(cx), self.picker_index) {
                    self.act(Action::Add(id), window, cx);
                }
                return;
            }
            Action::ReferenceMenu => {
                self.reference_menu = !self.reference_menu;
                self.reference_index = self
                    .planner
                    .zones
                    .iter()
                    .position(|z| *z == self.planner.reference)
                    .unwrap_or(0);
                if self.reference_menu {
                    self.reference_generation = self.reference_generation.wrapping_add(1);
                    self.reference_reveal_pending.set(true);
                    self.focus.focus(window);
                } else {
                    self.action_focus[&Action::ReferenceMenu].focus(window);
                }
                cx.notify();
                return;
            }
            Action::Copy => {
                cx.write_to_clipboard(ClipboardItem::new_string(self.planner.meeting_summary()));
                self.status = Some("Copied times".into());
                cx.notify();
                return;
            }
            Action::Copilot => {
                self.status =
                    Some("Copilot isn’t available in this offline Rust planner yet.".into());
                cx.notify();
                return;
            }
        };
        let success = result.is_ok();
        self.changed_in_day(result, preserve_day, cx);
        if save && success {
            self.persist();
        }
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if [&self.date, &self.time, &self.query]
            .iter()
            .any(|e| e.read(cx).focus_handle(cx).is_focused(window) && e.read(cx).has_marked_text())
        {
            return;
        }
        if key == "enter" {
            self.consume_enter_release = true;
        }
        // A held Return belongs to the control that received its first press,
        // never a button revealed/restored when that press dismisses a picker.
        if repeated_activation(key, event.is_held) {
            cx.stop_propagation();
            return;
        }
        if key == "tab" {
            cx.stop_propagation();
            // Commit before traversal, without depending on deferred focus events.
            // The blur callback remains for pointer/native focus changes.
            if self.date.read(cx).focus_handle(cx).is_focused(window)
                && self.date.read(cx).text() != self.displayed_date()
            {
                self.act(Action::ApplyDate, window, cx);
            } else if self.time.read(cx).focus_handle(cx).is_focused(window)
                && self.time.read(cx).text() != self.planner.time_input()
            {
                self.act(Action::ApplyTime, window, cx);
            }
            self.reference_menu = false;
            let order = self.tab_order(cx);
            let current = order.iter().position(|f| f.is_focused(window));
            let next = match current {
                Some(i) => moved_index(
                    i,
                    if event.keystroke.modifiers.shift {
                        -1
                    } else {
                        1
                    },
                    order.len(),
                ),
                None => {
                    if event.keystroke.modifiers.shift {
                        order.len().saturating_sub(1)
                    } else {
                        0
                    }
                }
            };
            if let Some(focus) = order.get(next) {
                focus.focus(window);
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.picker {
            if matches!(key, "escape" | "up" | "down" | "enter") {
                cx.stop_propagation();
            }
            match key {
                "escape" => self.close_picker(window, cx),
                "up" | "down" => {
                    let count = self.results(cx).len();
                    self.picker_index =
                        moved_index(self.picker_index, if key == "up" { -1 } else { 1 }, count);
                    self.picker_scroll.scroll_to_item(self.picker_index);
                    cx.notify();
                }
                "enter" => {
                    if self
                        .action_focus
                        .get(&Action::CancelPicker)
                        .is_some_and(|f| f.is_focused(window))
                    {
                        self.close_picker(window, cx);
                    } else if self
                        .action_focus
                        .get(&Action::ClearSearch)
                        .is_some_and(|f| f.is_focused(window))
                    {
                        self.act(Action::ClearSearch, window, cx);
                    } else {
                        self.act(Action::AddSelected, window, cx);
                    }
                }
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        if self.reference_menu {
            if matches!(key, "escape" | "up" | "down" | "enter") {
                cx.stop_propagation();
            }
            match key {
                "escape" => {
                    self.reference_menu = false;
                    self.action_focus[&Action::ReferenceMenu].focus(window);
                }
                "up" | "down" => {
                    self.reference_index = moved_index(
                        self.reference_index,
                        if key == "up" { -1 } else { 1 },
                        self.planner.zones.len(),
                    );
                    self.reference_scroll.scroll_to_item(self.reference_index);
                }
                "enter" => {
                    let id = self.planner.zones[self.reference_index].name().to_string();
                    self.act(Action::Reference(id), window, cx);
                }
                _ => return,
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if event.keystroke.modifiers.platform
            || cfg!(target_os = "linux") && event.keystroke.modifiers.control
        {
            let action = match key {
                "n" => Some(Action::Now),
                "l" => Some(Action::Picker),
                "c" if event.keystroke.modifiers.shift => Some(Action::Copy),
                _ => None,
            };
            if let Some(action) = action {
                cx.stop_propagation();
                self.act(action, window, cx);
                cx.stop_propagation();
                return;
            }
        }
        if key == "escape" {
            cx.stop_propagation();
            if self.reference_menu {
                self.reference_menu = false;
                cx.notify();
            } else if self.date.read(cx).focus_handle(cx).is_focused(window)
                || self.time.read(cx).focus_handle(cx).is_focused(window)
            {
                self.sync_fields(cx);
                self.error = None;
                self.focus.focus(window);
                cx.notify();
            }
            cx.stop_propagation();
        } else if key == "enter" {
            if self.date.read(cx).focus_handle(cx).is_focused(window) {
                cx.stop_propagation();
                self.act(Action::ApplyDate, window, cx);
                cx.stop_propagation();
            } else if self.time.read(cx).focus_handle(cx).is_focused(window) {
                cx.stop_propagation();
                self.act(Action::ApplyTime, window, cx);
                cx.stop_propagation();
            } else if let Some(action) = self
                .action_focus
                .iter()
                .find(|(_, f)| f.is_focused(window))
                .map(|(a, _)| a.clone())
            {
                cx.stop_propagation();
                self.act(action, window, cx);
                cx.stop_propagation();
            }
        } else if self.timeline_focus.is_focused(window) && ["left", "right"].contains(&key) {
            cx.stop_propagation();
            let result = step_displayed_day(
                &mut self.planner,
                &self.displayed_day,
                if key == "left" { -1 } else { 1 },
                false,
            );
            self.changed_in_day(result, true, cx);
            cx.stop_propagation();
        }
    }
    fn scrub(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(timeline) = self.drag_timeline.as_ref() else {
            return;
        };
        let b = if self.drag_slider {
            self.slider_bounds.get()
        } else {
            self.timeline_bounds.get()
        };
        let fraction = scrub_fraction(
            f32::from(position.x),
            f32::from(b.origin.x),
            f32::from(b.size.width),
        );
        let mut offset = fraction * timeline.duration_seconds() as f64;
        if self.drag_slider {
            offset = (offset / 900.).round() * 900.;
        }
        let result = timeline
            .instant_at_offset(offset)
            .and_then(|instant| self.planner.set_instant(instant));
        self.changed_in_day(result, true, cx);
    }
    fn prepare_focus(&mut self, cx: &mut Context<Self>) {
        let mut actions = vec![
            Action::Now,
            Action::ReferenceMenu,
            Action::Day(-1),
            Action::Day(1),
            Action::Picker,
            Action::CancelPicker,
            Action::ClearSearch,
            Action::AddSelected,
            Action::Copy,
            Action::Copilot,
        ];
        for z in &self.planner.zones {
            actions.push(Action::Reference(z.name().into()));
            actions.push(Action::Remove(z.name().into()));
        }
        actions.extend(self.results(cx).into_iter().map(|z| Action::Add(z.id)));
        self.action_focus.retain(|a, _| actions.contains(a));
        for action in actions {
            self.action_focus
                .entry(action)
                .or_insert_with(|| cx.focus_handle());
        }
    }
    fn tab_order(&self, cx: &App) -> Vec<FocusHandle> {
        let mut order = Vec::new();
        let add = |order: &mut Vec<FocusHandle>, action| {
            if let Some(f) = self.action_focus.get(&action) {
                order.push(f.clone());
            }
        };
        if self.picker {
            order.push(self.query.read(cx).focus_handle(cx));
            if !self.query.read(cx).text().is_empty() {
                add(&mut order, Action::ClearSearch);
            }
            add(&mut order, Action::CancelPicker);
            if selected_location(&self.results(cx), self.picker_index).is_some() {
                add(&mut order, Action::AddSelected);
            }
            return order;
        }
        add(&mut order, Action::Now);
        add(&mut order, Action::ReferenceMenu);
        add(&mut order, Action::Day(-1));
        order.push(self.date.read(cx).focus_handle(cx));
        add(&mut order, Action::Day(1));
        order.push(self.time.read(cx).focus_handle(cx));
        order.push(self.timeline_focus.clone());
        for z in &self.planner.zones {
            if *z != self.planner.reference {
                add(&mut order, Action::Reference(z.name().into()));
            }
            if self.planner.zones.len() > 1 {
                add(&mut order, Action::Remove(z.name().into()));
            }
        }
        for a in [Action::Picker, Action::Copilot, Action::Copy] {
            add(&mut order, a);
        }
        order
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        action: Action,
        primary: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let disabled_add = matches!(action, Action::AddSelected)
            && selected_location(&self.results(cx), self.picker_index).is_none();
        let enabled = !disabled_add
            && !matches!(&action, Action::Remove(_) if self.planner.zones.len() == 1)
            && !matches!(&action, Action::Reference(id) if id == self.planner.reference.name());
        let focus = self.action_focus[&action].clone();
        div()
            .id(id)
            .track_focus(&focus)
            .focus(move |s| s.border_color(p.accent))
            .when(enabled, |s| {
                s.on_mouse_down(MouseButton::Left, move |_, window, _| focus.focus(window))
            })
            .min_h(px(28.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(8.))
            .border_1()
            .border_color(if primary { p.accent_fill } else { p.separator })
            .bg(if primary { p.accent_fill } else { p.surface })
            .text_color(if primary {
                rgb(0xffffff).into()
            } else {
                p.primary
            })
            .text_size(px(12.))
            .when(enabled, |s| {
                s.cursor_pointer()
                    .hover(move |s| s.bg(if primary { p.brand } else { p.well }))
            })
            .when(disabled_add, |s| s.opacity(0.4))
            .child(label.into())
            .when(enabled, |s| {
                s.on_click(
                    cx.listener(move |this, _, window, cx| this.act(action.clone(), window, cx)),
                )
            })
    }
    fn icon_button(
        &self,
        id: impl Into<ElementId>,
        icon: ClockIcon,
        action: Action,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let enabled = !matches!(&action, Action::Remove(_) if self.planner.zones.len() == 1)
            && !matches!(&action, Action::Reference(id) if id == self.planner.reference.name());
        let ink = if matches!(&action, Action::Reference(id) if id == self.planner.reference.name())
        {
            p.accent
        } else {
            p.secondary
        };
        let focus = self.action_focus[&action].clone();
        div()
            .id(id)
            .track_focus(&focus)
            .focus(move |s| s.border_color(p.accent))
            .when(enabled, |s| {
                s.on_mouse_down(MouseButton::Left, move |_, window, _| focus.focus(window))
            })
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .border_1()
            .border_color(p.separator)
            .bg(p.surface)
            .child(clock_icon(icon, ink))
            .when(enabled, |s| {
                s.cursor_pointer().hover(move |s| s.bg(p.well)).on_click(
                    cx.listener(move |this, _, window, cx| this.act(action.clone(), window, cx)),
                )
            })
    }
    fn timeline(&self, slider: bool, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let bounds = if slider {
            self.slider_bounds.clone()
        } else {
            self.timeline_bounds.clone()
        };
        let timeline = Some(self.displayed_day.clone());
        let fraction = timeline
            .as_ref()
            .map(|t| t.offset_seconds(self.planner.instant) / t.duration_seconds() as f64)
            .unwrap_or(0.) as f32;
        let bands = self.bands.clone();
        div()
            .id(if slider {
                "clock-time-slider"
            } else {
                "clock-quality-timeline"
            })
            .h(px(if slider { 22. } else { 26. }))
            .w_full()
            .when(slider, |s| s.track_focus(&self.timeline_focus))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    this.timeline_focus.focus(window);
                    this.drag_timeline = Some(this.displayed_day.clone());
                    this.drag_slider = slider;
                    this.scrub(e.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                let delta = e.delta.pixel_delta(px(10.));
                if let Some(steps) = this.wheel.consume(
                    f32::from(delta.x),
                    f32::from(delta.y),
                    matches!(e.touch_phase, TouchPhase::Started),
                ) {
                    if steps != 0 {
                        let target = this.displayed_day.offset_seconds(this.planner.instant)
                            + steps as f64 * 900.;
                        let overflow =
                            target < 0. || target > this.displayed_day.duration_seconds() as f64;
                        let result =
                            step_displayed_day(&mut this.planner, &this.displayed_day, steps, true);
                        this.changed_in_day(result, !overflow, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .child(
                canvas(
                    move |b, _, _| bounds.set(b),
                    move |b, _, window, _| {
                        let center = b.origin.y + b.size.height / 2.;
                        let marker =
                            b.origin.x + px(5.) + (b.size.width - px(10.)) * fraction.clamp(0., 1.);
                        if slider {
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        point(b.origin.x, center - px(2.)),
                                        size(b.size.width, px(4.)),
                                    ),
                                    p.border,
                                )
                                .corner_radii(px(2.)),
                            );
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        point(b.origin.x, center - px(2.)),
                                        size(marker - b.origin.x, px(4.)),
                                    ),
                                    p.accent_fill,
                                )
                                .corner_radii(px(2.)),
                            );
                        } else {
                            for (i, q) in bands.iter().enumerate() {
                                let width = b.size.width / bands.len().max(1) as f32;
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(b.origin.x + width * i as f32, center - px(10.)),
                                        size((width - px(1.)).max(px(1.)), px(20.)),
                                    ),
                                    quality_color(*q, p).opacity(0.65),
                                ));
                            }
                        }
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(
                                        marker - px(5.),
                                        center - px(if slider { 7. } else { 13. }),
                                    ),
                                    size(px(10.), px(if slider { 14. } else { 26. })),
                                ),
                                p.accent,
                            )
                            .corner_radii(px(5.)),
                        );
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(
                                        marker - px(3.),
                                        center - px(if slider { 5. } else { 11. }),
                                    ),
                                    size(px(6.), px(if slider { 10. } else { 22. })),
                                ),
                                p.surface,
                            )
                            .corner_radii(px(3.)),
                        );
                    },
                )
                .size_full(),
            )
    }
    fn planner_card(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let reference = clock::zone_option(self.planner.reference.name()).name;
        let anchor_bounds = self.reference_button_bounds.clone();
        let reveal_pending = self.reference_reveal_pending.clone();
        let reference_generation = self.reference_generation;
        let entity = cx.weak_entity();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(16.))
            .rounded(px(12.))
            .bg(p.surface)
            .border_1()
            .border_color(p.separator)
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(crate::theme::tool_icon("worldClock", 16., p))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Meeting planner"),
                    )
                    .child(div().flex_1())
                    .child(
                        self.button(
                            "clock-reference",
                            format!("Reference: {reference} ▾"),
                            Action::ReferenceMenu,
                            false,
                            p,
                            cx,
                        )
                        .relative()
                        .child(
                            canvas(
                                move |bounds, _, _| anchor_bounds.set(bounds),
                                |_, _, _, _| (),
                            )
                            .absolute()
                            .inset_0(),
                        ),
                    )
                    .when(self.reference_menu, |s| {
                        s.child(deferred(
                            div()
                                .on_children_prepainted(move |_, window, _| {
                                    if reveal_pending.replace(false) {
                                        let entity = entity.clone();
                                        // GPUI initializes ScrollHandle overflow/bounds only
                                        // during the first prepaint. Reveal in the following
                                        // frame; a pre-layout FirstVisible request is discarded.
                                        window.on_next_frame(move |_, cx| {
                                            let _ = entity.update(cx, |this, cx| {
                                                if reveal_current_reference(
                                                    this.reference_menu,
                                                    this.reference_generation,
                                                    reference_generation,
                                                ) {
                                                    this.reference_scroll
                                                        .scroll_to_item(this.reference_index);
                                                    cx.notify();
                                                }
                                            });
                                        });
                                    }
                                })
                                .absolute()
                                .top(px(34.))
                                .right_0()
                                .id("clock-reference-menu")
                                .max_h(px(260.))
                                .overflow_y_scroll()
                                .track_scroll(&self.reference_scroll)
                                .on_mouse_down_out(cx.listener(
                                    |this, event: &MouseDownEvent, window, cx| {
                                        if dismiss_reference_for_pointer(
                                            this.reference_menu,
                                            this.reference_button_bounds
                                                .get()
                                                .contains(&event.position),
                                            false,
                                        ) {
                                            // Native menus consume the dismissing outside click;
                                            // it must not also activate Now or another planner control.
                                            this.reference_menu = false;
                                            window.prevent_default();
                                            cx.stop_propagation();
                                            this.action_focus[&Action::ReferenceMenu].focus(window);
                                            cx.notify();
                                        }
                                    },
                                ))
                                .w(px(250.))
                                .p(px(6.))
                                .rounded(px(8.))
                                .bg(p.surface)
                                .border_1()
                                .border_color(p.border)
                                .occlude()
                                .children(
                                    self.planner.presentations().into_iter().enumerate().map(
                                        |(index, zone)| {
                                            self.button(
                                                SharedString::from(format!(
                                                    "reference-{}",
                                                    zone.id
                                                )),
                                                format!(
                                                    "{}{}",
                                                    if zone.is_reference { "✓ " } else { "" },
                                                    zone.name
                                                ),
                                                Action::Reference(zone.id),
                                                false,
                                                p,
                                                cx,
                                            )
                                            .w_full()
                                            .when(index == self.reference_index, |s| {
                                                s.bg(p.accent.opacity(0.09))
                                            })
                                        },
                                    ),
                                ),
                        ))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(self.icon_button(
                        "clock-previous-day",
                        ClockIcon::Previous,
                        Action::Day(-1),
                        p,
                        cx,
                    ))
                    .child(field(self.date.clone(), DATE_FIELD_WIDTH, p).id("clock-date-field"))
                    .child(self.icon_button(
                        "clock-next-day",
                        ClockIcon::Next,
                        Action::Day(1),
                        p,
                        cx,
                    ))
                    .child(div().w(px(1.)).h(px(22.)).mx(px(3.)).bg(p.separator))
                    .child(div().text_size(px(12.)).child("Time"))
                    .child(
                        field(self.time.clone(), TIME_FIELD_WIDTH, p).id("clock-exact-time-field"),
                    )
                    .child(div().flex_1())
                    .child(quality_badge(self.planner.selected_quality(), p)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(self.timeline(false, p, cx))
                    .child(self.timeline(true, p, cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child("00:00")
                            .child(div().flex_1())
                            .children([Quality::Working, Quality::Extended, Quality::Poor].map(
                                |q| {
                                    div()
                                        .px(px(7.))
                                        .text_color(quality_color(q, p))
                                        .child(q.short_label())
                                },
                            ))
                            .child(div().flex_1())
                            .child("Next day"),
                    ),
            )
    }
    fn location_row(&self, z: clock::ZonePresentation, p: Palette, cx: &mut Context<Self>) -> Div {
        let day = if z.day_difference == 0 {
            String::new()
        } else {
            format!("  {:+}d", z.day_difference)
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(14.))
            .rounded(px(12.))
            .bg(p.surface)
            .border_1()
            .border_color(if z.is_reference {
                p.accent.opacity(0.25)
            } else {
                p.separator
            })
            .child(
                div()
                    .flex_none()
                    .size(px(38.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(11.))
                    .bg(quality_color(z.quality, p).opacity(0.09))
                    .child(quality_icon(z.quality, p)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(z.name.clone()),
                            )
                            .when(z.is_reference, |s| {
                                s.child(
                                    div()
                                        .px(px(6.))
                                        .py(px(3.))
                                        .rounded(px(4.))
                                        .bg(p.accent.opacity(0.09))
                                        .text_color(p.accent)
                                        .text_size(px(10.))
                                        .child("Reference"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(z.zone_text),
                    ),
            )
            .child(quality_badge(z.quality, p).w(px(86.)))
            .child(
                div()
                    .min_w(px(145.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(26.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(z.time_text),
                    )
                    .child(
                        div()
                            .flex()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(z.date_text)
                            .child(div().text_color(p.accent).child(day)),
                    ),
            )
            .child(
                self.icon_button(
                    SharedString::from(format!("anchor-{}", z.id)),
                    ClockIcon::Pin,
                    Action::Reference(z.id.clone()),
                    p,
                    cx,
                )
                .when(z.is_reference, |s| s.text_color(p.accent)),
            )
            .child(
                self.icon_button(
                    SharedString::from(format!("remove-{}", z.id)),
                    ClockIcon::Close,
                    Action::Remove(z.id),
                    p,
                    cx,
                )
                .when(self.planner.zones.len() == 1, |s| s.opacity(0.35)),
            )
    }
    fn clear_search_control(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let focus = self.action_focus[&Action::ClearSearch].clone();
        div()
            .id("clock-clear-search")
            .size(px(20.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&focus)
            .rounded(px(4.))
            .border_1()
            .border_color(transparent_black())
            .focus(move |s| s.border_color(p.accent))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, window, _| focus.focus(window))
            .child(clear_search_icon(p))
            .on_click(cx.listener(|this, _, window, cx| this.act(Action::ClearSearch, window, cx)))
    }
    fn picker_view(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let results = self.results(cx);
        let has_query = !self.query.read(cx).text().is_empty();
        div().absolute().inset_0().flex().items_center().justify_center().bg(rgba(0x00000038)).occlude()
            .on_mouse_down(MouseButton::Left,|_,_,cx|cx.stop_propagation())
            .child(div().w(px(PICKER_WIDTH)).flex().flex_col().rounded(px(12.)).bg(p.bg).border_1().border_color(p.border).overflow_hidden()
                .child(div().flex().items_center().gap(px(10.)).p(px(16.))
                    .child(crate::theme::tool_icon("worldClock",32.,p))
                    .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("Add a location"))
                    .child(div().flex_1()).child(self.button("clock-picker-close","×",Action::CancelPicker,false,p,cx)))
                .child(div().mx(px(16.)).p(px(12.)).rounded(px(10.)).border_1().border_color(p.separator).bg(p.well).flex().items_center().gap(px(10.))
                    .child(crate::theme::tool_icon("search",16.,Palette {accent:p.secondary,..p}))
                    .child(div().relative().flex_1().min_w_0().h(px(26.)).child(self.query.clone()).when(!has_query,|s|s.child(div().absolute().left(px(2.)).top(px(3.)).text_size(px(16.)).text_color(p.secondary.opacity(0.6)).child("Search city or time zone…"))))
                    .when(has_query,|s|s.child(self.clear_search_control(p,cx))))
                .child(div().id("clock-picker-results").h(px(280.)).overflow_y_scroll().track_scroll(&self.picker_scroll).p(px(10.))
                    .when(results.is_empty(),|s|s.child(div().p(px(24.)).flex().flex_col().gap(px(8.)).child("No matching locations").child(div().text_size(px(11.)).text_color(p.secondary).child("Try a nearby city or Asia/Tokyo. Locations already added are hidden."))))
                    .children(results.into_iter().enumerate().map(|(index,option)| {
                        let id = option.id.clone();
                        div().id(SharedString::from(format!("clock-result-{}",option.id))).p(px(10.)).mb(px(4.)).rounded(px(8.)).cursor_pointer()
                            .bg(if index==self.picker_index { p.accent.opacity(0.09) } else { gpui::transparent_black() })
                            .hover(move |s|s.bg(p.well)).flex().items_center().gap(px(10.))
                            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(option.name))
                                .child(div().text_size(px(11.)).text_color(p.secondary).truncate().child(option.subtitle)))
                            .child(div().text_color(p.accent).child(if index==self.picker_index { "↵" } else { "+" }))
                            .on_click(cx.listener(move |this,_,window,cx|this.act(Action::Add(id.clone()),window,cx)))
                    })))
                .child(div().p(px(12.)).border_t_1().border_color(p.separator).flex().items_center().gap(px(8.))
                    .child(div().text_size(px(11.)).text_color(p.secondary).child("↑↓ Navigate"))
                    .child(div().flex_1()).child(self.button("clock-picker-cancel","Cancel",Action::CancelPicker,false,p,cx))
                    .child(self.button("clock-picker-add","Add Location",Action::AddSelected,true,p,cx))))
    }
}
impl Render for WorldClock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_focus(cx);
        let p = crate::theme::for_window(window);
        for editor in [&self.date, &self.time, &self.query] {
            if editor.read(cx).appearance().text != p.primary {
                editor.update(cx, |e, cx| {
                    let mut a = e.appearance().clone();
                    a.text = p.primary;
                    a.caret = p.accent;
                    a.selection = p.accent.opacity(0.18);
                    e.set_appearance(a, cx);
                });
            }
        }
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::key))
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                // GPUI Div.on_click synthesizes an Enter click on key release.
                // We already performed this activation on key-down, possibly
                // restoring another focused control after dismissing a picker.
                if take_enter_release(&mut this.consume_enter_release, &event.keystroke.key) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.drag_timeline.is_some() && e.pressed_button == Some(MouseButton::Left) {
                    this.scrub(e.position, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    if this.drag_timeline.is_some() {
                        this.scrub(e.position, cx);
                        this.drag_timeline = None;
                        this.refresh_bands();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.drag_timeline.take().is_some() {
                        this.refresh_bands();
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .p(px(16.))
                    .border_b_1()
                    .border_color(p.separator)
                    .child(
                        div()
                            .size(px(34.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(10.))
                            .bg(p.accent.opacity(0.09))
                            .child(crate::theme::tool_icon("worldClock", 22., p)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("World Clock"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.secondary)
                                    .child("One moment. Every time zone."),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(20.))
                            .bg(p.surface)
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if self.planner.follows_now {
                                p.success
                            } else {
                                p.accent
                            })
                            .child(if self.planner.follows_now {
                                "Live time"
                            } else {
                                "Planning"
                            }),
                    )
                    .child(self.button("clock-now", "Now", Action::Now, false, p, cx)),
            )
            .child(
                div()
                    .id("clock-locations-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(self.planner_card(p, cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Your locations · {}", self.planner.zones.len()))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(p.secondary)
                                    .child("All times stay in sync"),
                            ),
                    )
                    .child(
                        div().flex().flex_col().gap(px(8.)).children(
                            self.planner
                                .presentations()
                                .into_iter()
                                .map(|z| self.location_row(z, p, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .p(px(16.))
                    .border_t_1()
                    .border_color(p.separator)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when_some(self.error.clone(), |s, e| {
                        s.child(div().text_size(px(11.)).text_color(p.danger).child(e))
                    })
                    .when_some(
                        self.status.clone().filter(|s| s != "Copied times"),
                        |s, e| s.child(div().text_size(px(11.)).text_color(p.secondary).child(e)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(self.button(
                                "clock-add-location",
                                "+ Add Location",
                                Action::Picker,
                                false,
                                p,
                                cx,
                            ))
                            .child(self.button(
                                "clock-copilot-unavailable",
                                "Copilot",
                                Action::Copilot,
                                false,
                                p,
                                cx,
                            ))
                            .child(div().flex_1())
                            .when(self.status.as_deref() == Some("Copied times"), |s| {
                                s.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.secondary)
                                        .child("Copied times"),
                                )
                            })
                            .child(self.button(
                                "clock-copy-times",
                                "Copy Times",
                                Action::Copy,
                                true,
                                p,
                                cx,
                            )),
                    ),
            )
            .when(self.picker, |s| s.child(self.picker_view(p, cx)))
    }
}
fn selected_location(results: &[ZoneOption], index: usize) -> Option<String> {
    results.get(index).map(|z| z.id.clone())
}
fn reveal_current_reference(open: bool, current: u64, requested: u64) -> bool {
    open && current == requested
}
fn dismiss_reference_for_pointer(open: bool, inside_anchor: bool, inside_menu: bool) -> bool {
    open && !inside_anchor && !inside_menu
}
fn clear_search_icon(p: Palette) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            window.paint_quad(fill(b, p.secondary).corner_radii(px(8.)));
            for line in [[(5., 5.), (11., 11.)], [(11., 5.), (5., 11.)]] {
                let mut path = PathBuilder::stroke(px(1.5));
                path.move_to(b.origin + point(px(line[0].0), px(line[0].1)));
                path.line_to(b.origin + point(px(line[1].0), px(line[1].1)));
                if let Ok(path) = path.build() {
                    window.paint_path(path, p.well);
                }
            }
        },
    )
    .size(px(16.))
}
fn take_enter_release(pending: &mut bool, key: &str) -> bool {
    key == "enter" && std::mem::take(pending)
}
fn repeated_activation(key: &str, held: bool) -> bool {
    held && matches!(key, "enter" | "escape")
}
fn refresh_field(focused: bool, marked: bool, current: &str, old: &str, new: &str) -> bool {
    !focused && !marked && current == old && current != new
}
fn move_displayed_day(planner: &mut Planner, day: &Timeline, days: i64) -> Result<(), String> {
    let mut target = planner.clone();
    target.set_instant(day.start)?;
    target.move_days(days)?;
    planner.select_date(&target.date_input())
}
fn step_displayed_day(
    planner: &mut Planner,
    day: &Timeline,
    steps: i64,
    wheel: bool,
) -> Result<(), String> {
    let target = day.offset_seconds(planner.instant) + steps as f64 * 900.;
    if wheel && (target < 0. || target > day.duration_seconds() as f64) {
        planner.nudge_steps(steps)
    } else {
        planner.set_instant(day.instant_at_offset(target)?)
    }
}
fn plain_editor(
    text: String,
    font_size: f32,
    p: Palette,
    window: &mut Window,
    cx: &mut Context<WorldClock>,
) -> Entity<EditorView> {
    cx.new(|cx| {
        let mut e = EditorView::new(text, window, cx);
        let mut a = EditorAppearance::plain();
        a.font_size = font_size;
        a.line_height = 22.;
        a.padding_y = 0.;
        a.padding_x = 0.;
        a.font_family = crate::theme::ui_font().into();
        a.text = p.primary;
        a.caret = p.accent;
        a.selection = p.accent.opacity(0.18);
        e.set_appearance(a, cx);
        e.set_compact(true, cx);
        e
    })
}
fn field(editor: Entity<EditorView>, width: f32, p: Palette) -> Div {
    div()
        .w(px(width))
        .h(px(30.))
        .px(px(6.))
        .py(px(3.))
        .bg(p.well)
        .rounded(px(6.))
        .border_1()
        .border_color(p.separator)
        .child(editor)
}
fn quality_color(quality: Quality, p: Palette) -> Hsla {
    // Exact semantic tokens from UI/Theme.swift. Decorative fixed hues do not
    // provide the source's readable small-text ink in both appearances.
    let dark = p.bg.l < 0.5;
    let (r, g, b) = match quality {
        Quality::Working => return p.success,
        Quality::Extended => {
            if dark {
                (1., 0.80, 0.42)
            } else {
                (0.50, 0.36, 0.)
            }
        }
        Quality::Poor => {
            if dark {
                (0.77, 0.63, 1.)
            } else {
                (0.46, 0.26, 0.70)
            }
        }
    };
    Rgba { r, g, b, a: 1. }.into()
}

fn quality_badge(quality: Quality, p: Palette) -> Div {
    div()
        .px(px(8.))
        .py(px(5.))
        .rounded(px(6.))
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(quality_color(quality, p))
        .bg(quality_color(quality, p).opacity(0.09))
        .child(quality.short_label())
}
#[derive(Clone, Copy)]
enum ClockIcon {
    Previous,
    Next,
    Pin,
    Close,
}
fn clock_icon(icon: ClockIcon, ink: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let mut lines: Vec<Vec<(f32, f32)>> = match icon {
                ClockIcon::Previous => vec![vec![(10., 3.), (5., 8.), (10., 13.)]],
                ClockIcon::Next => vec![vec![(6., 3.), (11., 8.), (6., 13.)]],
                ClockIcon::Close => vec![vec![(4., 4.), (12., 12.)], vec![(12., 4.), (4., 12.)]],
                ClockIcon::Pin => vec![vec![(8., 7.), (8., 14.)]],
            };
            if matches!(icon, ClockIcon::Pin) {
                lines.push(
                    (0..=24)
                        .map(|i| {
                            let a = i as f32 * std::f32::consts::TAU / 24.;
                            (8. + 3. * a.cos(), 4. + 3. * a.sin())
                        })
                        .collect(),
                );
            }
            for line in lines {
                let mut path = PathBuilder::stroke(px(1.4));
                for (i, (x, y)) in line.into_iter().enumerate() {
                    let p = b.origin + point(px(x), px(y));
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, ink);
                }
            }
        },
    )
    .size(px(16.))
}
fn quality_icon(quality: Quality, p: Palette) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let ink = quality_color(quality, p);
            let center = b.center();
            window.paint_quad(
                fill(
                    Bounds::new(center - point(px(6.), px(6.)), size(px(12.), px(12.))),
                    ink,
                )
                .corner_radii(px(6.)),
            );
            if quality == Quality::Poor {
                window.paint_quad(
                    fill(
                        Bounds::new(center - point(px(2.), px(8.)), size(px(12.), px(12.))),
                        p.surface,
                    )
                    .corner_radii(px(6.)),
                );
            } else {
                for (x, y, w, h) in [
                    (0., -11., 1., 3.),
                    (0., 8., 1., 3.),
                    (-11., 0., 3., 1.),
                    (8., 0., 3., 1.),
                ] {
                    window.paint_quad(fill(
                        Bounds::new(center + point(px(x), px(y)), size(px(w), px(h))),
                        ink,
                    ));
                }
            }
        },
    )
    .size(px(24.))
}
fn local_zone() -> Option<String> {
    bello_platform::system_time_zone_identifier()
        .filter(|id| clock::search_zones(id).iter().any(|z| z == id))
}

/// Plain repeat-open preserves the existing plan. Closing drops the entity; a new
/// open starts live from saved locations/reference, matching the Swift controller.
pub fn open(input: String, cx: &mut App) {
    for window in cx.windows() {
        if let Some(clock) = window.downcast::<WorldClock>()
            && clock
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            cx.activate(true);
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
    if let Err(e) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
            titlebar: Some(TitlebarOptions {
                title: Some("World Clock".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| WorldClock::new(input, window, cx)),
    ) {
        eprintln!("Cannot open World Clock: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_SIZE, PICKER_WIDTH, WINDOW_SIZE, WheelAccumulator, moved_index, scrub_fraction,
    };
    #[test]
    fn empty_or_stale_picker_selection_cannot_add_a_location() {
        let none: Vec<super::ZoneOption> = vec![];
        assert_eq!(super::selected_location(&none, 0), None);
        let results = bellobox_core::clock::search_options("bangalore", &[]);
        assert_eq!(
            super::selected_location(&results, 0).as_deref(),
            Some("Asia/Kolkata")
        );
        assert_eq!(super::selected_location(&results, results.len()), None);
    }
    #[test]
    fn delayed_reference_reveal_cannot_reopen_or_scroll_a_newer_menu() {
        assert!(super::reveal_current_reference(true, 4, 4));
        assert!(!super::reveal_current_reference(false, 4, 4));
        assert!(!super::reveal_current_reference(true, 5, 4));
    }
    #[test]
    fn reference_outside_dismissal_excludes_menu_and_toggle_anchor() {
        assert!(super::dismiss_reference_for_pointer(true, false, false));
        assert!(!super::dismiss_reference_for_pointer(true, true, false));
        assert!(!super::dismiss_reference_for_pointer(true, false, true));
        assert!(!super::dismiss_reference_for_pointer(false, false, false));
    }
    #[test]
    fn wrapped_reference_selection_keeps_scroll_target_in_range() {
        let count = 18;
        let mut selection = 0;
        selection = super::moved_index(selection, -1, count);
        assert_eq!(selection, 17);
        selection = super::moved_index(selection, 1, count);
        assert_eq!(selection, 0);
        for _ in 0..80 {
            selection = super::moved_index(selection, 1, count);
            assert!(selection < count);
        }
    }
    #[test]
    fn quality_ink_matches_source_tokens_in_both_appearances() {
        for (dark, warning, purple) in [
            (false, (0.50, 0.36, 0.), (0.46, 0.26, 0.70)),
            (true, (1., 0.80, 0.42), (0.77, 0.63, 1.)),
        ] {
            let palette = crate::theme::for_dark(dark);
            let color = |(r, g, b)| gpui::Hsla::from(gpui::Rgba { r, g, b, a: 1. });
            assert_eq!(
                super::quality_color(super::Quality::Extended, palette),
                color(warning)
            );
            assert_eq!(
                super::quality_color(super::Quality::Poor, palette),
                color(purple)
            );
            assert_eq!(
                super::quality_color(super::Quality::Working, palette),
                palette.success
            );
        }
    }
    #[test]
    fn consumed_enter_release_does_not_reactivate_restored_focus() {
        let mut pending = true; // A root-owned keydown handled Add and restored a button.
        assert!(!super::take_enter_release(&mut pending, "space"));
        assert!(pending);
        assert!(super::take_enter_release(&mut pending, "enter"));
        assert!(!pending);
        assert!(!super::take_enter_release(&mut pending, "enter"));
    }
    #[test]
    fn held_activation_cannot_fire_the_restored_control() {
        assert!(super::repeated_activation("enter", true));
        assert!(super::repeated_activation("escape", true));
        assert!(!super::repeated_activation("enter", false));
        assert!(!super::repeated_activation("right", true));
        assert!(!super::repeated_activation("up", true));
    }
    #[test]
    fn live_field_refresh_preserves_focus_composition_draft_and_identical_text() {
        assert!(!super::refresh_field(
            true, false, "09:00", "09:00", "09:01"
        ));
        assert!(!super::refresh_field(
            false, true, "09:00", "09:00", "09:01"
        ));
        assert!(!super::refresh_field(
            false, false, "draft", "09:00", "09:01"
        ));
        assert!(!super::refresh_field(
            false,
            false,
            "2026-10-05",
            "2026-10-05",
            "2026-10-05"
        ));
        assert!(super::refresh_field(
            false, false, "09:00", "09:00", "09:01"
        ));
    }
    #[test]
    fn compact_canonical_fields_leave_room_for_editor_caret_reserve() {
        // Shared EditorView currently reserves three estimated eight-pixel columns.
        for (width, characters) in [
            (super::DATE_FIELD_WIDTH, 10.),
            (super::TIME_FIELD_WIDTH, 5.),
        ] {
            assert!(((width - 14.) / 8.).floor() - 3. > characters);
        }
    }
    fn endpoint_planner() -> (
        bellobox_core::clock::Planner,
        bellobox_core::clock::Timeline,
    ) {
        let now = bellobox_core::clock::parse_instant("2026-10-05T12:00:00Z").unwrap();
        let planner = bellobox_core::clock::Planner::from_preferences(
            &["UTC".into()],
            "UTC",
            "UTC",
            now,
            Some(now),
        )
        .unwrap();
        let day = planner.timeline().unwrap();
        (planner, day)
    }
    #[test]
    fn keyboard_stays_on_displayed_day_but_wheel_overflows() {
        let (mut planner, day) = endpoint_planner();
        planner.set_instant(day.end).unwrap();
        super::step_displayed_day(&mut planner, &day, 1, false).unwrap();
        super::step_displayed_day(&mut planner, &day, 1, false).unwrap();
        assert_eq!(
            planner.instant, day.end,
            "right at endpoint must not advance displayed day"
        );
        super::step_displayed_day(&mut planner, &day, -1, false).unwrap();
        assert_eq!(planner.time_input(), "23:45");
        super::step_displayed_day(&mut planner, &day, 1, false).unwrap();
        super::step_displayed_day(&mut planner, &day, 1, true).unwrap();
        assert_eq!(planner.date_input(), "2026-10-06");
        assert_eq!(planner.time_input(), "00:15");
        planner.set_instant(day.start).unwrap();
        super::step_displayed_day(&mut planner, &day, -1, false).unwrap();
        assert_eq!(planner.instant, day.start);
        super::step_displayed_day(&mut planner, &day, -1, true).unwrap();
        assert_eq!(planner.date_input(), "2026-10-04");
        assert_eq!(planner.time_input(), "23:45");
    }
    #[test]
    fn day_arrow_uses_displayed_day_even_at_next_midnight_endpoint() {
        let (mut planner, day) = endpoint_planner();
        planner.set_instant(day.end).unwrap();
        super::move_displayed_day(&mut planner, &day, 1).unwrap();
        assert_eq!(planner.date_input(), "2026-10-06");
        assert_eq!(planner.time_input(), "00:00");
    }
    #[test]
    fn source_dimensions_and_picker_wrap() {
        assert_eq!(WINDOW_SIZE, (920., 740.));
        assert_eq!(MIN_SIZE, (780., 640.));
        assert_eq!(PICKER_WIDTH, 440.);
        assert_eq!(moved_index(0, -1, 10), 9);
        assert_eq!(moved_index(9, 1, 10), 0);
        assert_eq!(moved_index(0, 1, 0), 0);
    }
    #[test]
    fn source_wheel_direction_threshold_and_vertical_passthrough() {
        let mut a = WheelAccumulator::default();
        assert_eq!(a.consume(1., -8., false), None);
        assert_eq!(a.consume(-6., 2., false), Some(0));
        assert_eq!(a.consume(-6., 0., false), Some(0));
        assert_eq!(a.consume(-4., 0., false), Some(1));
        assert_eq!(a.consume(40., 0., false), Some(-2));
        assert_eq!(a.consume(-14., 0., true), Some(1));
    }
    #[test]
    fn continuous_pointer_mapping_preserves_source_inset() {
        assert_eq!(scrub_fraction(5., 0., 100.), 0.);
        assert_eq!(scrub_fraction(95., 0., 100.), 1.);
        assert_eq!(scrub_fraction(50., 0., 100.), 0.5);
        assert!(scrub_fraction(51., 0., 100.) > 0.5);
        assert_eq!(scrub_fraction(-20., 0., 100.), 0.);
    }
}
