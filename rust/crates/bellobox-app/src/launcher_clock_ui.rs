//! The bounded World Clock row in the launcher. Its planner is ephemeral: only
//! an explicit launcher handoff can carry this state into the full clock window.
use crate::{
    clock_preview_session::{ClockHandoff, ClockPreviewSession},
    theme::Palette,
    world_clock_ui::{WheelAccumulator, local_zone, quality_badge, quality_color},
};
use bellobox_core::{
    clock::{self, Quality, Timeline, ZonePresentation},
    settings::Settings,
};
use gpui::{prelude::*, *};
use std::{cell::Cell, collections::HashMap, rc::Rc};

pub const PREVIEW_HEIGHT: f32 = 261.;
const HEADER_HEIGHT: f32 = 18.;
const PLANNER_HEIGHT: f32 = 78.;
const CARDS_HEIGHT: f32 = 88.;
const COPILOT_HEIGHT: f32 = 37.;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Control {
    Reference,
    Previous,
    Next,
    Reset,
}

struct CapturedScrub {
    day: Timeline,
    bounds: Bounds<Pixels>,
}

pub struct LauncherClockPreview {
    session: ClockPreviewSession,
    focus: FocusHandle,
    timeline_focus: FocusHandle,
    controls: HashMap<Control, FocusHandle>,
    reference_menu: bool,
    reference_index: usize,
    reference_bounds: Rc<Cell<Bounds<Pixels>>>,
    timeline_bounds: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<CapturedScrub>,
    wheel: WheelAccumulator,
    consume_enter_release: bool,
    bands: Vec<Quality>,
    bands_key: String,
    error: Option<String>,
    active: bool,
    ticker: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl LauncherClockPreview {
    pub fn new(
        input: String,
        settings: &Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let local = local_zone().unwrap_or_else(|| "UTC".into());
        let result =
            ClockPreviewSession::new(&settings.zone_ids, &settings.anchor_zone_id, &local, &input);
        let error = result.as_ref().err().cloned();
        let session = result.unwrap_or_else(|_| {
            ClockPreviewSession::new(&[], "UTC", "UTC", "")
                .expect("the current UTC day is a valid clock preview")
        });
        let controls = [
            Control::Reference,
            Control::Previous,
            Control::Next,
            Control::Reset,
        ]
        .into_iter()
        .map(|action| (action, cx.focus_handle()))
        .collect();
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.drag = None;
                if this.reference_menu {
                    this.close_reference(window, cx);
                }
            }
        });
        let mut this = Self {
            session,
            focus: cx.focus_handle(),
            timeline_focus: cx.focus_handle(),
            controls,
            reference_menu: false,
            reference_index: 0,
            reference_bounds: Rc::new(Cell::new(Bounds::default())),
            timeline_bounds: Rc::new(Cell::new(Bounds::default())),
            drag: None,
            wheel: WheelAccumulator::default(),
            consume_enter_release: false,
            bands: Vec::new(),
            bands_key: String::new(),
            error,
            active: false,
            ticker: None,
            _subscriptions: vec![activation],
        };
        this.refresh_bands();
        // Mounting a preview never takes focus from the launcher's search field.
        this
    }

    pub fn handoff(&self) -> ClockHandoff {
        self.session.handoff()
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        if !active {
            // Dropping the task cancels the timer, rather than leaving every
            // previously visited row polling invisibly for the palette session.
            self.ticker = None;
            self.reference_menu = false;
            self.drag = None;
            self.wheel = WheelAccumulator::default();
            self.consume_enter_release = false;
            return;
        }
        self.tick(cx);
        self.ticker = Some(cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.drag.is_some() {
            return;
        }
        match self.session.refresh_now() {
            Ok(true) => {
                self.refresh_bands();
                cx.notify();
            }
            Ok(false) => {}
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }

    pub fn nudge(&mut self, steps: i64, day: bool, cx: &mut Context<Self>) {
        self.drag = None;
        let result = if day {
            self.session.move_day(steps)
        } else {
            self.session.nudge(steps)
        };
        self.changed(result, cx);
    }

    fn reset_visible(&self) -> bool {
        self.session.has_moved_from_seed() || !self.session.has_seed() && !self.session.is_live()
    }

    fn tab_order(&self) -> Vec<FocusHandle> {
        let mut order = vec![
            self.controls[&Control::Reference].clone(),
            self.controls[&Control::Previous].clone(),
            self.controls[&Control::Next].clone(),
        ];
        if self.reset_visible() {
            order.push(self.controls[&Control::Reset].clone());
        }
        order.push(self.timeline_focus.clone());
        order
    }

    /// The launcher calls this only after checking its query's IME marked text.
    /// The reference menu owns every key while open. Ordinary search arrows and
    /// Enter remain with the launcher unless an actual preview button owns focus.
    pub fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if self.reference_menu {
            if key == "enter" {
                self.consume_enter_release = true;
            }
            if !event.is_held || !matches!(key, "enter" | "escape") {
                match key {
                    "escape" => self.close_reference(window, cx),
                    "up" | "down" => {
                        self.reference_index = moved_index(
                            self.reference_index,
                            if key == "up" { -1 } else { 1 },
                            self.session.planner.zones.len(),
                        );
                        cx.notify();
                    }
                    "enter" => {
                        if let Some(zone) = self.session.planner.zones.get(self.reference_index) {
                            let id = zone.name().to_string();
                            self.choose_reference(&id, window, cx);
                        }
                    }
                    "tab" => {
                        self.close_reference(window, cx);
                        self.traverse(modifiers.shift, window);
                    }
                    _ => {}
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if modifiers.platform || modifiers.control || modifiers.alt {
            return false;
        }
        if key == "tab" && self.traverse(modifiers.shift, window) {
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if key == "enter"
            && let Some(action) = self
                .controls
                .iter()
                .find(|(_, focus)| focus.is_focused(window))
                .map(|(action, _)| *action)
        {
            self.consume_enter_release = true;
            if !event.is_held {
                self.act(action, window, cx);
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        false
    }

    /// Captured at the launcher root as focus can move when a menu closes.
    pub fn handle_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.key == "enter" && std::mem::take(&mut self.consume_enter_release) {
            // Div.on_click synthesizes Enter clicks on release. This key has
            // already activated the old control, so it must not hit a new one.
            window.prevent_default();
            cx.stop_propagation();
            true
        } else {
            false
        }
    }

    fn traverse(&self, backwards: bool, window: &mut Window) -> bool {
        let order = self.tab_order();
        let current = order.iter().position(|focus| focus.is_focused(window));
        let next = next_tab_index(current, backwards, order.len());
        if let Some(next) = next {
            order[next].focus(window);
            true
        } else {
            // The launcher owns the search field and restores it at either edge.
            false
        }
    }

    fn act(&mut self, action: Control, window: &mut Window, cx: &mut Context<Self>) {
        self.drag = None;
        let result = match action {
            Control::Previous => self.session.move_day(-1),
            Control::Next => self.session.move_day(1),
            Control::Reset => {
                let result = if self.session.has_moved_from_seed() {
                    self.session.return_to_seed()
                } else {
                    self.session.go_to_now()
                };
                // The reset chip disappears after activation. Keep keyboard focus
                // on a visible control instead of a removed element.
                self.timeline_focus.focus(window);
                result
            }
            Control::Reference => {
                if self.reference_menu {
                    self.close_reference(window, cx);
                } else {
                    self.reference_index = self
                        .session
                        .planner
                        .zones
                        .iter()
                        .position(|zone| *zone == self.session.planner.reference)
                        .unwrap_or(0);
                    self.reference_menu = true;
                    self.focus.focus(window);
                    cx.notify();
                }
                return;
            }
        };
        self.changed(result, cx);
    }

    fn close_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reference_menu = false;
        self.controls[&Control::Reference].focus(window);
        cx.notify();
    }

    fn choose_reference(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.close_reference(window, cx);
        let result = self.session.set_reference(id);
        self.changed(result, cx);
    }

    fn changed(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.error = result.err();
        if self.error.is_none() {
            self.refresh_bands();
        }
        cx.notify();
    }

    fn refresh_bands(&mut self) {
        let key = format!(
            "{}:{}:{:?}",
            self.session.displayed_day.start.timestamp(),
            self.session.reference(),
            self.session.planner.zones,
        );
        if self.bands_key != key {
            self.bands = self.session.timeline_qualities().unwrap_or_default();
            self.bands_key = key;
        }
    }

    fn scrub(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        let fraction = scrub_fraction(
            f32::from(position.x),
            f32::from(drag.bounds.origin.x),
            f32::from(drag.bounds.size.width),
        );
        let offset = fraction * drag.day.duration_seconds() as f64;
        // Preserve the day captured on mouse-down. The pure session's set_offset
        // keeps it stable even when selecting the next-midnight endpoint.
        self.session.displayed_day = drag.day.clone();
        let result = self.session.set_offset(offset);
        self.changed(result, cx);
    }

    fn button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        action: Control,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let focus = self.controls[&action].clone();
        div()
            .id(id)
            .track_focus(&focus)
            .h(px(22.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .px(px(6.))
            .rounded(px(6.))
            .border_1()
            .border_color(transparent_black())
            .focus(move |style| style.border_color(p.accent))
            .hover(move |style| style.bg(p.well))
            .cursor_pointer()
            .text_size(px(10.))
            .line_height(px(12.))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                focus.focus(window);
                cx.stop_propagation();
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.act(action, window, cx);
                cx.stop_propagation();
            }))
            .child(label.into())
    }

    fn header(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let reference = clock::zone_option(self.session.reference()).name;
        let title = if self.session.has_seed() {
            "Timestamp recognized"
        } else if self.session.is_live() {
            "Current time"
        } else {
            "Planning"
        };
        let subtitle = if self.session.is_live() {
            "Live · scrub to plan a meeting"
        } else {
            "↵ opens World Clock at this time"
        };
        let bounds = self.reference_bounds.clone();
        div()
            .relative()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(10.))
            .line_height(px(12.))
            .child(div().text_color(p.accent).child("ϟ"))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(if self.error.is_some() {
                        p.danger
                    } else {
                        p.secondary
                    })
                    .child(
                        self.error
                            .clone()
                            .unwrap_or_else(|| format!("· {subtitle}")),
                    ),
            )
            .child(
                self.button(
                    "launcher-clock-reference",
                    format!("Reference: {reference} ▾"),
                    Control::Reference,
                    p,
                    cx,
                )
                .h(px(18.))
                .relative()
                .child(
                    canvas(move |b, _, _| bounds.set(b), |_, _, _, _| ())
                        .absolute()
                        .inset_0(),
                ),
            )
            .when(self.reference_menu, |style| {
                style.child(deferred(self.reference_dropdown(p, cx)))
            })
    }

    fn reference_dropdown(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("launcher-clock-reference-menu")
            .absolute()
            .top(px(22.))
            .right_0()
            .w(px(230.))
            .p(px(5.))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if this.reference_menu && !this.reference_bounds.get().contains(&event.position) {
                    this.close_reference(window, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .children(
                self.session
                    .planner
                    .presentations()
                    .into_iter()
                    .enumerate()
                    .map(|(index, zone)| {
                        let id = zone.id.clone();
                        div()
                            .id(SharedString::from(format!(
                                "launcher-reference-{}",
                                zone.id
                            )))
                            .h(px(28.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(7.))
                            .rounded(px(5.))
                            .text_size(px(11.))
                            .bg(if index == self.reference_index {
                                p.accent.opacity(0.09)
                            } else {
                                transparent_black()
                            })
                            .hover(move |style| style.bg(p.well))
                            .cursor_pointer()
                            .child(
                                div()
                                    .w(px(12.))
                                    .text_color(p.accent)
                                    .child(if zone.is_reference { "✓" } else { "" }),
                            )
                            .child(zone.name)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.choose_reference(&id, window, cx);
                                cx.stop_propagation();
                            }))
                    }),
            )
    }

    fn planner(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let selected = self
            .session
            .planner
            .instant
            .with_timezone(&self.session.planner.reference)
            .format("%a, %b %-d · %H:%M")
            .to_string();
        let start = self
            .session
            .displayed_day
            .start
            .with_timezone(&self.session.planner.reference)
            .format("%H:%M")
            .to_string();
        div()
            .h(px(PLANNER_HEIGHT))
            .flex_none()
            .px(px(10.))
            .py(px(8.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .rounded(px(9.))
            .border_1()
            .border_color(p.separator)
            .bg(p.surface)
            .child(
                div()
                    .h(px(22.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(
                        self.button("launcher-clock-previous-day", "‹", Control::Previous, p, cx)
                            .w(px(22.))
                            .px_0()
                            .text_size(px(20.)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(selected),
                    )
                    .child(
                        self.button("launcher-clock-next-day", "›", Control::Next, p, cx)
                            .w(px(22.))
                            .px_0()
                            .text_size(px(20.)),
                    )
                    .when(self.reset_visible(), |style| {
                        style.child(
                            self.button(
                                "launcher-clock-reset",
                                if self.session.has_moved_from_seed() {
                                    "↶ Selected time"
                                } else {
                                    "↶ Now"
                                },
                                Control::Reset,
                                p,
                                cx,
                            )
                            .text_color(p.accent)
                            .bg(p.accent.opacity(0.09))
                            .rounded(px(11.)),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        quality_badge(self.session.planner.selected_quality(), p)
                            .py(px(3.))
                            .flex_none(),
                    ),
            )
            .child(self.timeline(p, cx))
            .child(
                div()
                    .h(px(12.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(9.))
                    .line_height(px(10.))
                    .text_color(p.secondary)
                    .child(start)
                    .child(div().flex_1())
                    .children(
                        [Quality::Working, Quality::Extended, Quality::Poor].map(|quality| {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.))
                                .text_color(quality_color(quality, p))
                                .child(quality_symbol(quality, p))
                                .child(quality.short_label())
                        }),
                    )
                    .child(div().flex_1())
                    .child("Next day"),
            )
    }

    fn timeline(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let bounds = self.timeline_bounds.clone();
        let day = &self.session.displayed_day;
        let fraction = (day.offset_seconds(self.session.planner.instant)
            / day.duration_seconds() as f64) as f32;
        let bands = self.bands.clone();
        let entity = cx.weak_entity();
        div()
            .id("launcher-clock-timeline")
            .h(px(14.))
            .w_full()
            .flex_none()
            .track_focus(&self.timeline_focus)
            .rounded(px(7.))
            .border_1()
            .border_color(transparent_black())
            .focus(move |style| style.border_color(p.accent))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.timeline_focus.focus(window);
                    this.drag = Some(CapturedScrub {
                        day: this.session.displayed_day.clone(),
                        bounds: this.timeline_bounds.get(),
                    });
                    this.scrub(event.position, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(10.));
                if let Some(steps) = this.wheel.consume(
                    f32::from(delta.x),
                    f32::from(delta.y),
                    matches!(event.touch_phase, TouchPhase::Started),
                ) {
                    if steps != 0 {
                        this.nudge(steps, false, cx);
                    }
                    cx.stop_propagation();
                }
                // Vertical wheel events deliberately continue to the launcher list.
            }))
            .child(
                canvas(
                    move |b, _, _| bounds.set(b),
                    move |b, _, window, _| {
                        let center = b.center().y;
                        for (index, quality) in bands.iter().enumerate() {
                            let width = b.size.width / bands.len().max(1) as f32;
                            // No segment gaps: this is one continuous 14-point strip.
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(b.origin.x + width * index as f32, center - px(4.)),
                                    size(width + px(0.5), px(8.)),
                                ),
                                quality_color(*quality, p).opacity(0.60),
                            ));
                        }
                        let marker =
                            b.origin.x + px(4.) + (b.size.width - px(8.)) * fraction.clamp(0., 1.);
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(marker - px(4.), center - px(6.)),
                                    size(px(8.), px(12.)),
                                ),
                                p.accent,
                            )
                            .corner_radii(px(4.)),
                        );
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(marker - px(2.), center - px(4.)),
                                    size(px(4.), px(8.)),
                                ),
                                p.surface,
                            )
                            .corner_radii(px(2.)),
                        );

                        // Register during paint as required by GPUI. These captured
                        // listeners keep scrubbing beyond the preview's bounds.
                        let move_entity = entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture
                                && event.pressed_button == Some(MouseButton::Left)
                            {
                                let _ = move_entity.update(cx, |this, cx| {
                                    if this.drag.is_some() {
                                        this.scrub(event.position, cx);
                                        cx.stop_propagation();
                                    }
                                });
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                let _ = entity.update(cx, |this, cx| {
                                    if this.drag.is_some() {
                                        this.scrub(event.position, cx);
                                        this.drag = None;
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    }
                                });
                            }
                        });
                    },
                )
                .size_full(),
            )
    }

    fn card(&self, zone: ZonePresentation, p: Palette) -> Stateful<Div> {
        let compact_zone = compact_zone_text(&zone.zone_text);
        div()
            .id(SharedString::from(format!(
                "launcher-clock-card-{}",
                zone.id
            )))
            .h_full()
            .flex_1()
            .min_w_0()
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(3.))
            .rounded(px(9.))
            .border_1()
            .border_color(if zone.is_reference {
                p.accent.opacity(0.35)
            } else {
                p.border
            })
            .bg(p.surface)
            .child(
                div()
                    .h(px(13.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .line_height(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(zone.name),
                    )
                    .when(zone.is_reference, |style| style.child(reference_symbol(p)))
                    .child(quality_symbol(zone.quality, p)),
            )
            .child(
                div()
                    .h(px(24.))
                    .flex_none()
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(zone.time_text),
            )
            .child(
                div()
                    .h(px(12.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(10.))
                    .line_height(px(12.))
                    .text_color(p.secondary)
                    .child(div().min_w_0().truncate().child(zone.date_text))
                    .when(zone.day_difference != 0, |style| {
                        style.child(
                            div()
                                .flex_none()
                                .text_color(p.accent)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("{:+}d", zone.day_difference)),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(11.))
                    .flex_none()
                    .truncate()
                    .text_size(px(9.))
                    .line_height(px(11.))
                    .text_color(p.secondary)
                    .child(compact_zone),
            )
    }
}

impl Render for LauncherClockPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::for_window(window);
        div()
            .id("launcher-clock-preview")
            .relative()
            .w_full()
            .h(px(PREVIEW_HEIGHT))
            .flex_none()
            .px(px(10.))
            .pt(px(4.))
            .pb(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .track_focus(&self.focus)
            .child(self.header(p, cx))
            .child(self.planner(p, cx))
            .child(
                div()
                    .h(px(CARDS_HEIGHT))
                    .flex_none()
                    .flex()
                    .gap(px(8.))
                    .children(
                        self.session
                            .planner
                            .presentations()
                            .into_iter()
                            .take(4)
                            .map(|zone| self.card(zone, p)),
                    ),
            )
            .child(
                div()
                    .id("launcher-clock-copilot-unavailable")
                    .h(px(COPILOT_HEIGHT))
                    .flex_none()
                    .px(px(8.))
                    .py(px(4.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.separator)
                    .bg(p.well)
                    .child(
                        div()
                            .text_size(px(11.))
                            .line_height(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Copilot"),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .line_height(px(12.))
                            .text_color(p.secondary)
                            .child("Unavailable in this offline Rust preview."),
                    ),
            )
    }
}

/// Compact vector equivalents of the source sun.max, sun.horizon and moon.stars.
fn quality_symbol(quality: Quality, p: Palette) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let ink = quality_color(quality, p);
            let mut lines: Vec<Vec<(f32, f32)>> = Vec::new();
            match quality {
                Quality::Working | Quality::Extended => {
                    let extended = quality == Quality::Extended;
                    let count = if extended { 12 } else { 24 };
                    lines.push(
                        (0..=count)
                            .map(|i| {
                                let a = if extended {
                                    std::f32::consts::PI
                                        + i as f32 * std::f32::consts::PI / count as f32
                                } else {
                                    i as f32 * std::f32::consts::TAU / count as f32
                                };
                                (
                                    5. + 2.2 * a.cos(),
                                    if extended { 6. } else { 5. } + 2.2 * a.sin(),
                                )
                            })
                            .collect(),
                    );
                    for (x, y, x2, y2) in [
                        (5., 0.5, 5., 1.5),
                        (0.5, 5., 1.5, 5.),
                        (8.5, 5., 9.5, 5.),
                        (1.7, 1.7, 2.4, 2.4),
                        (7.6, 2.4, 8.3, 1.7),
                    ] {
                        lines.push(vec![(x, y), (x2, y2)]);
                    }
                    if extended {
                        lines.push(vec![(0.5, 6.5), (9.5, 6.5)]);
                        lines.push(vec![(2., 8.5), (8., 8.5)]);
                    } else {
                        lines.extend([
                            vec![(5., 8.5), (5., 9.5)],
                            vec![(1.7, 8.3), (2.4, 7.6)],
                            vec![(7.6, 7.6), (8.3, 8.3)],
                        ]);
                    }
                }
                Quality::Poor => {
                    lines.push(vec![
                        (5.5, 1.),
                        (3., 1.5),
                        (1.5, 3.5),
                        (1.5, 6.),
                        (3., 8.),
                        (5.5, 8.5),
                        (7.5, 7.3),
                        (5., 7.),
                        (3.8, 5.),
                        (4., 2.7),
                        (5.5, 1.),
                    ]);
                    lines.push(vec![(8., 0.5), (8., 3.5)]);
                    lines.push(vec![(6.5, 2.), (9.5, 2.)]);
                }
            }
            for line in lines {
                let mut path = PathBuilder::stroke(px(0.8));
                for (i, (x, y)) in line.into_iter().enumerate() {
                    let at = bounds.origin + point(px(x), px(y));
                    if i == 0 {
                        path.move_to(at);
                    } else {
                        path.line_to(at);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, ink);
                }
            }
        },
    )
    .size(px(10.))
    .flex_none()
}
fn reference_symbol(p: Palette) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(fill(bounds, p.accent).corner_radii(px(4.5)));
            window.paint_quad(
                fill(
                    Bounds::new(bounds.origin + point(px(3.), px(1.5)), size(px(3.), px(3.))),
                    p.surface,
                )
                .corner_radii(px(1.5)),
            );
            window.paint_quad(fill(
                Bounds::new(bounds.origin + point(px(4.), px(4.)), size(px(1.), px(3.5))),
                p.surface,
            ));
        },
    )
    .size(px(9.))
    .flex_none()
}

fn moved_index(index: usize, direction: i32, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        (index as i64 + direction as i64).rem_euclid(count as i64) as usize
    }
}

fn next_tab_index(current: Option<usize>, backwards: bool, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match (current, backwards) {
        (None, false) => Some(0),
        (None, true) => Some(count - 1),
        (Some(index), true) => index.checked_sub(1),
        (Some(index), false) => (index + 1 < count).then_some(index + 1),
    }
}

fn scrub_fraction(x: f32, left: f32, width: f32) -> f64 {
    ((x - left - 4.) / (width - 8.).max(1.)).clamp(0., 1.) as f64
}

/// Format the core's already-resolved offset rather than calculating timezone
/// offsets in the UI. Keep quarter/half-hour offsets intact in narrow cards.
fn compact_zone_text(text: &str) -> String {
    let (abbreviation, offset) = text.split_once(" - ").unwrap_or(("", text));
    let Some(offset) = offset.strip_prefix("UTC") else {
        return text.to_string();
    };
    let Some((hours, minutes)) = offset.get(1..).and_then(|offset| offset.split_once(':')) else {
        return text.to_string();
    };
    let Ok(hours) = hours.parse::<u32>() else {
        return text.to_string();
    };
    let sign = if offset.starts_with('-') { "−" } else { "+" };
    let compact = if minutes == "00" {
        format!("UTC{sign}{hours}")
    } else {
        format!("UTC{sign}{hours}:{minutes}")
    };
    if abbreviation.is_empty() || abbreviation == "UTC" {
        compact
    } else {
        format!("{abbreviation} · {compact}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CARDS_HEIGHT, COPILOT_HEIGHT, HEADER_HEIGHT, PLANNER_HEIGHT, PREVIEW_HEIGHT,
        compact_zone_text, moved_index, next_tab_index, scrub_fraction,
    };

    #[test]
    fn preview_regions_fit_the_original_reserved_height() {
        assert_eq!(
            HEADER_HEIGHT + PLANNER_HEIGHT + CARDS_HEIGHT + COPILOT_HEIGHT + 3. * 8. + 4. + 12.,
            PREVIEW_HEIGHT
        );
    }

    #[test]
    fn scrubber_clamps_both_endpoints_and_keeps_fractional_positions() {
        assert_eq!(scrub_fraction(-100., 10., 608.), 0.);
        assert_eq!(scrub_fraction(14., 10., 608.), 0.);
        assert_eq!(scrub_fraction(614., 10., 608.), 1.);
        assert_eq!(scrub_fraction(1_000., 10., 608.), 1.);
        assert!((scrub_fraction(164., 10., 608.) - 0.25).abs() < 0.0001);
    }

    #[test]
    fn tab_enters_from_search_and_releases_to_search_at_both_ends() {
        assert_eq!(next_tab_index(None, false, 4), Some(0));
        assert_eq!(next_tab_index(None, true, 4), Some(3));
        assert_eq!(next_tab_index(Some(0), true, 4), None);
        assert_eq!(next_tab_index(Some(3), false, 4), None);
        assert_eq!(next_tab_index(Some(1), false, 4), Some(2));
        assert_eq!(next_tab_index(None, false, 0), None);
    }

    #[test]
    fn menu_navigation_wraps_at_the_bounded_locations() {
        assert_eq!(moved_index(0, -1, 4), 3);
        assert_eq!(moved_index(3, 1, 4), 0);
        assert_eq!(moved_index(0, -1, 0), 0);
    }

    #[test]
    fn compact_offsets_preserve_non_hour_zones() {
        assert_eq!(compact_zone_text("EDT - UTC-04:00"), "EDT · UTC−4");
        assert_eq!(compact_zone_text("UTC - UTC+00:00"), "UTC+0");
        assert_eq!(compact_zone_text("UTC+05:30"), "UTC+5:30");
        assert_eq!(compact_zone_text("+0545 - UTC+05:45"), "+0545 · UTC+5:45");
        assert_eq!(compact_zone_text("unexpected"), "unexpected");
    }
}
