//! Separate source-sized launcher surface; never the application's Home sidebar.
use crate::clock_copilot_worker::RetirementGuard;
use crate::launcher_clock_ui::LauncherClockPreview;
use crate::launcher_qr_ui::LauncherQrPreview;
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::{
    launcher,
    settings::{Settings, config_dir},
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};

struct Launcher {
    query: Entity<EditorView>,
    preview: Entity<EditorView>,
    input: String,
    selected: usize,
    settings: Settings,
    settings_writable: bool,
    notice: Option<String>,
    jobs: crate::session::SessionJobs,
    qr: Option<Entity<LauncherQrPreview>>,
    qr_active: bool,
    qr_sized: bool,
    qr_subscriptions: Vec<Subscription>,
    clock: Option<Entity<LauncherClockPreview>>,
    clock_guards: Rc<RefCell<Vec<RetirementGuard>>>,
    clock_transferred: bool,
    suppress_escape: bool,
    closed: bool,
    clock_active: bool,
    clock_sized: bool,
    was_active: bool,
    _subscriptions: Vec<Subscription>,
}
impl Launcher {
    fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = crate::theme::for_window(window);
        let query = cx.new(|cx| {
            let mut e = EditorView::new(String::new(), window, cx);
            let mut a = EditorAppearance::plain();
            a.font_size = 18.;
            a.line_height = 26.;
            a.text = p.primary;
            a.caret = p.accent;
            e.set_appearance(a, cx);
            e.set_compact(true, cx);
            e
        });
        let preview = cx.new(|cx| {
            let mut e = EditorView::new(String::new(), window, cx);
            let mut a = EditorAppearance::plain();
            a.font_family = "monospace".into();
            a.font_size = 11.;
            a.line_height = 16.;
            a.text = p.primary;
            e.set_appearance(a, cx);
            e.set_read_only(true, cx);
            e
        });
        query.read(cx).focus(window);
        let subscription = cx.subscribe_in(
            &query,
            window,
            |this, _, event: &EditorEvent, window, cx| {
                if matches!(event, EditorEvent::Changed) {
                    this.selected = 0;
                    this.refresh_preview(window, cx);
                    cx.notify();
                }
            },
        );
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.was_active = true;
            } else if this.was_active && this.clock_active && !this.qr_save_pending(cx) {
                // The compact source palette dismisses on deactivation. Limit
                // this addition to the clock slice; no background preview survives.
                this.close(window, cx);
                cx.notify();
            }
        });
        let (notice, input) = match bellobox_core::validate_input(&input) {
            Ok(()) => (None, input),
            Err(e) => (Some(e), String::new()),
        };
        let loaded = Settings::load(&config_dir().join("settings.json"));
        let settings_writable = loaded.is_ok();
        let mut app = Self {
            query,
            preview,
            input,
            selected: 0,
            settings: loaded.unwrap_or_default(),
            settings_writable,
            notice,
            jobs: crate::session::SessionJobs::default(),
            qr: None,
            qr_active: false,
            qr_sized: false,
            qr_subscriptions: Vec::new(),
            clock: None,
            clock_guards: Default::default(),
            clock_transferred: false,
            suppress_escape: false,
            closed: false,
            clock_active: false,
            clock_sized: false,
            was_active: window.is_window_active(),
            _subscriptions: vec![subscription, activation],
        };
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |this, cx| this.allow_close(cx))
                .unwrap_or(true)
                && crate::shutdown::allow_close(window, cx)
        });
        let owner = window.window_handle();
        let weak = cx.weak_entity();
        app._subscriptions.push(cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner) {
                let _ = weak.update(cx, |this, cx| {
                    this.closed = true;
                    this.discard_qr(cx);
                    this.discard_clock(cx)
                });
            }
        }));
        cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.notice.as_deref()
                            == Some("Stopping the Copilot request. Close again after it finishes.")
                            && !this
                                .clock_guards
                                .borrow()
                                .iter()
                                .any(RetirementGuard::physically_active)
                        {
                            this.notice = None;
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
        app.refresh_preview(window, cx);
        app
    }
    fn clock_search_shortcut(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        if self.clock_active
            && event.keystroke.key == "k"
            && (modifiers.platform || modifiers.control)
            && !modifiers.alt
            && !modifiers.shift
        {
            self.query
                .update(cx, |editor, cx| editor.set_text(String::new(), cx));
            self.query.read(cx).focus(window);
            window.prevent_default();
            cx.stop_propagation();
            true
        } else {
            false
        }
    }
    fn route_clock_editor_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.key == "escape" && self.suppress_escape {
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if !self.clock_active {
            return false;
        }
        let Some(clock) = &self.clock else {
            return false;
        };
        if clock.read(cx).draft_composing(window, cx) {
            return true;
        }
        if event.keystroke.key == "escape" && clock.read(cx).draft_focused(window, cx) {
            self.suppress_escape = true;
            self.query.read(cx).focus(window);
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        false
    }
    fn qr_save_pending(&self, cx: &App) -> bool {
        self.qr
            .as_ref()
            .is_some_and(|qr| qr.read(cx).pending_dialog())
    }
    fn discard_qr(&mut self, cx: &mut Context<Self>) {
        self.qr_subscriptions.clear();
        if let Some(qr) = self.qr.take() {
            qr.update(cx, |qr, cx| qr.retire(cx));
        }
        self.qr_active = false;
    }
    fn route_qr_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.qr_save_pending(cx) {
            return true;
        }
        if !self.qr_active {
            return false;
        }
        let Some(qr) = self.qr.clone() else {
            return false;
        };
        if qr.read(cx).composing(window, cx) {
            return true;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if key == "escape"
            && (self.suppress_escape || qr.read(cx).owns_focus(window, cx))
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
        {
            self.suppress_escape = true;
            self.query.read(cx).focus(window);
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if key == "k"
            && (modifiers.platform || modifiers.control)
            && !modifiers.alt
            && !modifiers.shift
        {
            self.query.update(cx, |e, cx| e.set_text(String::new(), cx));
            self.query.read(cx).focus(window);
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        let search = self.query.read(cx).focus_handle(cx);
        qr.update(cx, |qr, cx| qr.handle_key(event, &search, window, cx))
    }
    fn tool_input(&self, id: &str, cx: &App) -> Result<String, String> {
        if id == "qr"
            && let Some(qr) = &self.qr
        {
            qr.read(cx).handoff(cx)
        } else {
            Ok(self.input.clone())
        }
    }
    fn discard_clock(&mut self, cx: &mut Context<Self>) {
        if let Some(clock) = self.clock.take() {
            clock.update(cx, |clock, cx| clock.retire_copilot(cx));
        }
        self.clock_guards
            .borrow_mut()
            .retain(RetirementGuard::physically_active);
    }
    fn allow_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.clock_transferred {
            return true;
        }
        let active = self
            .clock_guards
            .borrow()
            .iter()
            .any(RetirementGuard::physically_active);
        if active {
            for guard in self.clock_guards.borrow().iter() {
                guard.cancel();
            }
            if let Some(clock) = &self.clock {
                clock.update(cx, |clock, cx| clock.cancel_copilot(cx));
            }
            self.notice =
                Some("Stopping the Copilot request. Close again after it finishes.".into());
            cx.notify();
        }
        !active && !self.qr_save_pending(cx)
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.allow_close(cx) {
            self.jobs.cancel();
            self.discard_qr(cx);
            self.discard_clock(cx);
            crate::shutdown::close_window(window, cx);
        }
    }
    fn commands(&self, cx: &App) -> Vec<launcher::Command> {
        launcher::search(
            self.query.read(cx).text(),
            &self.input,
            &self.settings.favorites,
            &self.settings.recents,
            &self.settings.usage,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        )
    }
    fn refresh_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closed || crate::shutdown::requested(cx) {
            return;
        }
        self.clock_active = false;
        self.qr_active = false;
        if let Some(qr) = &self.qr {
            qr.update(cx, |qr, cx| qr.set_active(false, cx));
        }
        if let Some(clock) = &self.clock {
            clock.update(cx, |clock, cx| clock.set_active(false, cx));
        }
        let revision = self.jobs.begin();
        let Some(command) = self.commands(cx).get(self.selected).cloned() else {
            self.preview
                .update(cx, |e, cx| e.set_text(String::new(), cx));
            return;
        };
        if self.input.len() > bellobox_core::MAX_PREVIEW_BYTES {
            self.preview.update(cx,|e,cx|e.set_text("Selection exceeds the 64 KB preview limit. Open to work with the complete text.".into(),cx));
            return;
        }
        if command.id == "worldClock" {
            if self.clock.is_none() {
                self.clock = Some(cx.new(|cx| {
                    let mut preview =
                        LauncherClockPreview::new(self.input.clone(), &self.settings, window, cx);
                    preview.retain_guards(self.clock_guards.clone());
                    preview
                }));
                let preview = self.clock.as_ref().expect("created preview").clone();
                self._subscriptions
                    .push(cx.observe(&preview, |_, _, cx| cx.notify()));
            }
            self.clock_active = true;
            if let Some(clock) = &self.clock {
                clock.update(cx, |clock, cx| clock.set_active(true, cx));
            }
            cx.notify();
            return;
        }
        if command.id == "qr" {
            if self.qr.is_none() {
                let qr = cx.new(|cx| LauncherQrPreview::new(self.input.clone(), window, cx));
                self.qr_subscriptions
                    .push(cx.observe(&qr, |_, _, cx| cx.notify()));
                self.qr_subscriptions.push(cx.subscribe_in(
                    &qr,
                    window,
                    |this, _, _: &crate::launcher_qr_ui::Open, window, cx| {
                        if this.qr_active {
                            this.launch(window, cx);
                        }
                    },
                ));
                self.qr = Some(qr);
            }
            self.qr_active = true;
            if let Some(qr) = &self.qr {
                qr.update(cx, |qr, cx| qr.set_active(true, cx));
            }
            cx.notify();
            return;
        }
        if self.input.is_empty() {
            self.preview.update(cx, |e, cx| {
                e.set_text(
                    "Open this tool to begin, or choose Use Clipboard below.".into(),
                    cx,
                )
            });
            return;
        }
        let input = self.input.clone();
        let id = command.id.to_string();
        let task = cx.background_executor().spawn(async move {
            #[cfg(feature = "developer-tools")]
            if id == "numberBase" {
                return crate::tool_controls::number_base_preview(&input);
            }
            crate::execute(&id, &input, "")
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.jobs.accepts(revision) {
                    return;
                }
                let text = result.unwrap_or_else(|error| error);
                this.preview.update(cx, |e, cx| e.set_text(text, cx));
                cx.notify();
            });
        })
        .detach();
    }
    fn launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closed || crate::shutdown::requested(cx) || self.qr_save_pending(cx) {
            return;
        }
        if let Some(c) = self.commands(cx).get(self.selected) {
            let handoff = if c.id == "worldClock" && self.clock_active {
                match self
                    .clock
                    .as_ref()
                    .map(|clock| clock.read(cx).handoff(cx))
                    .transpose()
                {
                    Ok(handoff) => handoff,
                    Err(error) => {
                        self.notice = Some(error);
                        cx.notify();
                        return;
                    }
                }
            } else {
                None
            };
            // The source clock transfers a value snapshot. Closing the palette
            // releases its preview; the full window owns independent state.
            self.jobs.cancel();
            if let Some(handoff) = handoff {
                let handoff = handoff.with_retirement_guard(
                    self.clock_guards
                        .borrow()
                        .iter()
                        .find(|guard| guard.physically_active())
                        .cloned(),
                );
                if let Err(error) =
                    crate::desktop::open_clock_handoff(self.input.clone(), handoff, cx)
                {
                    self.notice = Some(error);
                    cx.notify();
                    return;
                }
                self.clock_transferred = true;
            } else {
                let input = match self.tool_input(c.id, cx) {
                    Ok(input) => input,
                    Err(error) => {
                        self.notice = Some(error);
                        cx.notify();
                        return;
                    }
                };
                crate::desktop::open_tool(c.id, input, cx);
            }
            self.close(window, cx);
        }
    }
}
impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::for_window(window);
        if self.query.read(cx).appearance().text != p.primary {
            for editor in [&self.query, &self.preview] {
                editor.update(cx, |e, cx| {
                    let mut style = e.appearance().clone();
                    style.text = p.primary;
                    style.caret = p.accent;
                    style.selection = p.accent.opacity(0.18);
                    e.set_appearance(style, cx);
                });
            }
        }
        let commands = self.commands(cx);
        let count = commands.len();
        if self.clock_active || self.clock_sized || self.qr_active || self.qr_sized {
            let natural = if self.clock_active {
                clock_palette_height(count, !self.input.is_empty() || self.notice.is_some())
                    + self
                        .clock
                        .as_ref()
                        .map(|c| c.read(cx).height() - crate::launcher_clock_ui::PREVIEW_HEIGHT)
                        .unwrap_or(0.)
            } else if self.qr_active {
                64. + if !self.input.is_empty() || self.notice.is_some() {
                    48.
                } else {
                    0.
                } + 26.
                    + count.min(5) as f32 * 42.
                    + 12.
                    + 42.
                    + self
                        .qr
                        .as_ref()
                        .map(|qr| qr.read(cx).height())
                        .unwrap_or(232.)
            } else {
                620.
            };
            let available = window
                .display(cx)
                .map(|display| f32::from(display.bounds().size.height) - 24.)
                .unwrap_or(natural);
            let height = natural.min(available.max(320.));
            if (f32::from(window.bounds().size.height) - height).abs() > 1. {
                window.resize(size(px(680.), px(height)));
            }
            self.clock_sized = self.clock_active;
            self.qr_sized = self.qr_active;
        }
        self.selected = self.selected.min(count.saturating_sub(1));
        let best = launcher::suggestions(&self.input).first().copied();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .rounded(px(16.))
            .border_1()
            .border_color(p.separator)
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.suppress_escape = false;
                }
                if this.query.read(cx).has_marked_text() {
                    return;
                }
                if let Some(qr) = &this.qr
                    && qr.update(cx, |qr, cx| qr.handle_key_up(event, window, cx))
                {
                    return;
                }
                if this.clock_active
                    && let Some(clock) = &this.clock
                {
                    clock.update(cx, |clock, cx| {
                        clock.handle_key_up(event, window, cx);
                    });
                }
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.query.read(cx).has_marked_text() {
                    return;
                }
                if this.route_qr_key(event, window, cx) {
                    return;
                }
                if this.route_clock_editor_key(event, window, cx) {
                    return;
                }
                if this.clock_active
                    && let Some(clock) = &this.clock
                    && clock.update(cx, |clock, cx| clock.handle_key(event, window, cx))
                {
                    return;
                }
                let key = event.keystroke.key.as_str();
                let modifiers = event.keystroke.modifiers;
                if this.clock_search_shortcut(event, window, cx) {
                    return;
                }
                if this.clock_active && clock_returns_to_search(key) {
                    this.query.read(cx).focus(window);
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                if this.clock_active
                    && this.query.read(cx).text().is_empty()
                    && matches!(key, "left" | "right")
                    && !modifiers.control
                    && !modifiers.platform
                {
                    let direction = if key == "right" { 1 } else { -1 };
                    let steps = direction * if modifiers.alt { 4 } else { 1 };
                    if let Some(clock) = &this.clock {
                        clock.update(cx, |clock, cx| {
                            clock.nudge(
                                if modifiers.shift { direction } else { steps },
                                modifiers.shift,
                                cx,
                            )
                        });
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                if modifiers.control || modifiers.platform || modifiers.alt || modifiers.shift {
                    return;
                }
                match key {
                    "escape" => {
                        this.close(window, cx);
                    }
                    "up" => {
                        this.selected = this.selected.saturating_sub(1);
                        this.refresh_preview(window, cx);
                        this.query.read(cx).focus(window);
                        cx.notify();
                    }
                    "down" => {
                        this.selected =
                            (this.selected + 1).min(this.commands(cx).len().saturating_sub(1));
                        this.refresh_preview(window, cx);
                        this.query.read(cx).focus(window);
                        cx.notify();
                    }
                    "enter" if !event.is_held => this.launch(window, cx),
                    _ => return,
                }
                window.prevent_default();
                cx.stop_propagation();
            }))
            .child(
                div()
                    .h(px(64.))
                    .flex_none()
                    .px(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(13.))
                    .child(crate::theme::tool_icon("search", 18., p))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h(px(30.))
                            .child(self.query.clone())
                            .when(self.query.read(cx).text().is_empty(), |s| {
                                s.child(
                                    div()
                                        .absolute()
                                        .left(px(2.))
                                        .top(px(3.))
                                        .text_size(px(18.))
                                        .text_color(p.secondary.opacity(0.55))
                                        .child("Search tools and commands…"),
                                )
                            }),
                    )
                    .when(!self.query.read(cx).text().is_empty(), |s| {
                        s.child(
                            div()
                                .id("clear-query")
                                .cursor_pointer()
                                .text_color(p.secondary)
                                .child("×")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.query.update(cx, |e, cx| e.set_text(String::new(), cx))
                                })),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(10.))
                            .px(px(5.))
                            .py(px(3.))
                            .rounded(px(4.))
                            .bg(p.primary.opacity(0.045))
                            .child("esc"),
                    ),
            )
            .when(!self.input.is_empty() || self.notice.is_some(), |s| {
                let context = div()
                    .h(px(48.))
                    .flex_none()
                    .px(px(18.))
                    .pb(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(crate::theme::tool_icon("textTools", 20., p))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .text_color(p.secondary)
                            .child(div().text_size(px(10.)).child(format!(
                                "Selected text · {} characters",
                                self.input.chars().count()
                            )))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(self.notice.clone().unwrap_or_else(|| {
                                        self.input
                                            .lines()
                                            .next()
                                            .unwrap_or_default()
                                            .chars()
                                            .take(180)
                                            .collect()
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("clear-selection")
                            .cursor_pointer()
                            .text_size(px(12.))
                            .child("×")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.qr_save_pending(cx) {
                                    return;
                                }
                                this.discard_clock(cx);
                                this.discard_qr(cx);
                                this.selected = 0;
                                this.query
                                    .update(cx, |editor, cx| editor.set_text(String::new(), cx));
                                this.input.clear();
                                this.notice = None;
                                this.refresh_preview(window, cx);
                                cx.notify();
                            })),
                    );
                s.child(context)
            })
            .child(
                div()
                    .border_t_1()
                    .border_color(p.separator)
                    .h(px(25.))
                    .flex_none()
                    .px(px(18.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(p.secondary)
                    .child(if self.query.read(cx).text().is_empty() {
                        if self.input.is_empty() {
                            "Your tools"
                        } else {
                            "Suggested for your selection"
                        }
                    } else {
                        "Results"
                    })
                    .child(count.to_string()),
            )
            .child(
                div()
                    .id("command-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(8.))
                    .py(px(6.))
                    .children(commands.into_iter().enumerate().map(|(i, c)| {
                        let selected = i == self.selected;
                        let id = c.id;
                        let favorite = self.settings.favorites.contains(id);
                        div()
                            .id(id)
                            .rounded(px(10.))
                            .mb(px(3.))
                            .when(selected, |s| {
                                s.bg(p.accent.opacity(0.09))
                                    .border_1()
                                    .border_color(p.accent.opacity(0.25))
                            })
                            .child(
                                div()
                                    .id(("row-header", i))
                                    .h(px(42.))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .gap(px(11.))
                                    .cursor_pointer()
                                    .child(crate::theme::tool_icon(id, 18., p))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(if selected {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .child(crate::theme::tool_title(id)),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(if best == Some(id) {
                                                p.accent
                                            } else {
                                                p.secondary
                                            })
                                            .child(if best == Some(id) {
                                                "Best match"
                                            } else if ![
                                                "ai",
                                                "screenshot",
                                                "scrollCapture",
                                                "recording",
                                                "videoToGIF",
                                                "worldClock",
                                                "qr",
                                                "textTools",
                                                "settings",
                                                "home",
                                            ]
                                            .contains(&id)
                                            {
                                                "Developer"
                                            } else {
                                                "Utility"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id(("favorite", i))
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .child(if favorite { "★" } else { "☆" })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if !this.settings.favorites.remove(id) {
                                                    this.settings.favorites.insert(id.into());
                                                }
                                                if this.settings_writable
                                                    && let Err(e) = this
                                                        .settings
                                                        .save(&config_dir().join("settings.json"))
                                                {
                                                    this.notice = Some(e);
                                                }
                                                cx.stop_propagation();
                                                cx.notify();
                                            })),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if this.qr_save_pending(cx) {
                                            return;
                                        }
                                        this.selected = i;
                                        this.refresh_preview(window, cx);
                                        this.launch(window, cx);
                                    })),
                            )
                            .when(selected && self.clock_active, |s| {
                                s.child(self.clock.as_ref().expect("active clock session").clone())
                            })
                            .when(selected && self.qr_active, |s| {
                                s.child(self.qr.as_ref().expect("active QR session").clone())
                            })
                            .when(selected && !self.clock_active && !self.qr_active, |s| {
                                s.child(
                                    div()
                                        .px(px(10.))
                                        .pb(px(10.))
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .h(px(130.))
                                                .p(px(8.))
                                                .rounded(px(7.))
                                                .bg(p.surface)
                                                .overflow_hidden()
                                                .child(self.preview.clone()),
                                        )
                                        .child(
                                            div().flex().justify_end().child(
                                                div()
                                                    .id("open-selected")
                                                    .px(px(10.))
                                                    .py(px(5.))
                                                    .rounded(px(6.))
                                                    .bg(p.surface)
                                                    .text_size(px(11.))
                                                    .text_color(p.accent)
                                                    .cursor_pointer()
                                                    .child("Open")
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.launch(window, cx)
                                                        },
                                                    )),
                                            ),
                                        ),
                                )
                            })
                    })),
            )
            .child(
                div()
                    .h(px(41.))
                    .flex_none()
                    .border_t_1()
                    .border_color(p.separator)
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .bg(p.surface)
                    .text_size(px(10.))
                    .text_color(p.secondary)
                    .child("Bello Box")
                    .child("│")
                    .child(
                        div()
                            .id("use-clipboard")
                            .cursor_pointer()
                            .child("Use Clipboard")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.qr_save_pending(cx) {
                                    return;
                                }
                                if let Some(text) = cx.read_from_clipboard().and_then(|v| v.text())
                                {
                                    match bellobox_core::validate_input(&text) {
                                        Ok(()) => {
                                            this.discard_clock(cx);
                                            this.discard_qr(cx);
                                            this.selected = 0;
                                            this.query.update(cx, |editor, cx| {
                                                editor.set_text(String::new(), cx)
                                            });
                                            this.input = text;
                                            this.notice = None;
                                        }
                                        Err(e) => {
                                            this.discard_clock(cx);
                                            this.discard_qr(cx);
                                            this.notice = Some(e);
                                            this.input.clear();
                                        }
                                    }
                                }
                                this.refresh_preview(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child("↑  ↓  Navigate")
                    .child("│")
                    .child("Open  ↵"),
            )
    }
}
pub fn open(input: String, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(680.), px(620.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            crate::shutdown::guard_window(window, cx);
            cx.new(|cx| Launcher::new(input, window, cx))
        },
    );
}

// This offline preview has no secondary text editor. Escape from a button or
// timeline belongs to launcher close; only an unhandled Tab returns to search.
fn clock_returns_to_search(key: &str) -> bool {
    key == "tab"
}
fn clock_palette_height(commands: usize, has_context: bool) -> f32 {
    64. + if has_context { 48. } else { 0. } + 26. + commands.min(5) as f32 * 42. + 12. + 261. + 42.
}
#[cfg(test)]
mod clock_tests {
    use super::{clock_palette_height, clock_returns_to_search};
    #[test]
    fn ordinary_clock_control_escape_belongs_to_launcher_close() {
        assert!(clock_returns_to_search("tab"));
        assert!(!clock_returns_to_search("escape"));
        assert!(!clock_returns_to_search("enter"));
    }
    #[test]
    fn clock_height_reserves_source_interactive_row_synchronously() {
        assert_eq!(clock_palette_height(5, false), 615.);
        assert_eq!(clock_palette_height(8, true), 663.);
        assert_eq!(clock_palette_height(1, true), 495.);
    }
}

#[cfg(test)]
mod copilot_lifecycle_tests;

#[cfg(test)]
mod qr_lifecycle_tests {
    use super::Launcher;
    use gpui::{
        AppContext, EntityInputHandler, Focusable, KeyDownEvent, Keystroke, TestAppContext,
    };
    fn key(name: &str, held: bool) -> KeyDownEvent {
        KeyDownEvent {
            keystroke: Keystroke::parse(name).unwrap(),
            is_held: held,
        }
    }
    #[gpui::test]
    fn retained_qr_draft_is_exact_and_other_tools_keep_original_selection(cx: &mut TestAppContext) {
        let view = cx.add_window(|w, cx| Launcher::new("original".into(), w, cx));
        view.update(cx, |v, w, cx| {
            v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
            v.refresh_preview(w, cx);
            let qr = v.qr.as_ref().unwrap().clone();
            let editor = qr.read(cx).draft_editor();
            editor.update(cx, |e, cx| e.set_text("edited\n界".into(), cx));
            v.query.update(cx, |e, cx| e.set_text("json".into(), cx));
            v.refresh_preview(w, cx);
            assert!(!v.qr_active);
            assert_eq!(v.tool_input("json", cx).unwrap(), "original");
            v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
            v.refresh_preview(w, cx);
            assert_eq!(v.qr.as_ref().unwrap().entity_id(), qr.entity_id());
            assert_eq!(v.tool_input("qr", cx).unwrap(), "edited\n界");
            editor.update(cx, |e, cx| e.set_text(String::new(), cx));
            assert_eq!(v.tool_input("qr", cx).unwrap(), "");
            v.discard_qr(cx);
            v.input = "replacement".into();
            v.refresh_preview(w, cx);
            assert_ne!(v.qr.as_ref().unwrap().entity_id(), qr.entity_id());
            assert_eq!(v.tool_input("qr", cx).unwrap(), "replacement");
        })
        .unwrap();
    }
    #[gpui::test]
    fn empty_selection_creates_qr_and_ime_escape_return_route_without_open(
        cx: &mut TestAppContext,
    ) {
        let view = cx.add_window(|w, cx| Launcher::new(String::new(), w, cx));
        view.update(cx, |v, w, cx| {
            v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
            v.refresh_preview(w, cx);
            assert!(v.qr_active);
            let editor = v.qr.as_ref().unwrap().read(cx).draft_editor();
            editor.read(cx).focus(w);
            assert!(v.route_qr_key(&key("enter", false), w, cx));
            assert!(v.route_qr_key(&key("down", false), w, cx));
            editor.update(cx, |e, cx| {
                e.replace_and_mark_text_in_range(None, "に", Some(1..1), w, cx)
            });
            assert!(v.route_qr_key(&key("escape", false), w, cx));
            assert!(editor.read(cx).has_marked_text());
            assert!(editor.read(cx).focus_handle(cx).is_focused(w));
            editor.update(cx, |e, cx| e.replace_text_in_range(None, "日", w, cx));
            assert!(v.route_qr_key(&key("escape", false), w, cx));
            assert!(v.query.read(cx).focus_handle(cx).is_focused(w));
            assert!(v.route_qr_key(&key("escape", true), w, cx));
            assert!(!v.closed);
            v.suppress_escape = false;
            assert!(!v.route_qr_key(&key("enter", false), w, cx));
            v.close(w, cx);
            assert!(v.qr.is_none());
        })
        .unwrap();
    }
    #[gpui::test]
    fn actual_open_popup_receives_edited_and_explicitly_empty_qr(cx: &mut TestAppContext) {
        for draft in ["edited payload", ""] {
            let view = cx.add_window(|w, cx| Launcher::new("original must not leak".into(), w, cx));
            cx.run_until_parked();
            view.update(cx, |v, w, cx| {
                v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
                v.refresh_preview(w, cx);
                let editor = v.qr.as_ref().unwrap().read(cx).draft_editor();
                editor.update(cx, |e, cx| e.set_text(draft.into(), cx));
                v.launch(w, cx);
            })
            .unwrap();
            cx.run_until_parked();
            assert!(view.root(cx).is_err());
            assert_eq!(cx.windows().len(), 1);
            let popup = cx.windows()[0];
            // Typing into the actual popup input makes an empty draft observable
            // through the editor's real select-all/copy route as well.
            let mut visual = gpui::VisualTestContext::from_window(popup, cx);
            visual.simulate_click(
                gpui::point(gpui::px(150.), gpui::px(450.)),
                gpui::Modifiers::default(),
            );
            cx.simulate_input(popup, "probe");
            cx.simulate_keystrokes(popup, "ctrl-a ctrl-c");
            let text = cx.read_from_clipboard().and_then(|c| c.text()).unwrap();
            assert!(text.contains("probe"));
            assert!(!text.contains("original must not leak"));
            assert_eq!(text.replace("probe", ""), draft);
            cx.update_window(popup, |_, w, cx| crate::shutdown::close_window(w, cx))
                .unwrap();
            cx.run_until_parked();
        }
    }
    #[gpui::test]
    fn dispatched_clear_held_return_and_release_preserve_empty_then_allow_newline(
        cx: &mut TestAppContext,
    ) {
        let view = cx.add_window(|w, cx| Launcher::new("draft".into(), w, cx));
        view.update(cx, |v, w, cx| {
            v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
            v.refresh_preview(w, cx);
            v.query.read(cx).focus(w);
        })
        .unwrap();
        cx.run_until_parked();
        // Search -> editor -> Enlarge -> Paste -> Clear.
        cx.simulate_keystrokes(view.into(), "tab tab tab tab");
        let mut visual = gpui::VisualTestContext::from_window(view.into(), cx);
        visual.simulate_event(key("enter", false));
        visual.simulate_event(key("enter", true));
        visual.simulate_event(gpui::KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
        cx.run_until_parked();
        view.update(cx, |v, _, cx| {
            assert_eq!(v.tool_input("qr", cx).unwrap(), "")
        })
        .unwrap();
        cx.simulate_keystrokes(view.into(), "enter");
        view.update(cx, |v, _, cx| {
            assert_eq!(v.tool_input("qr", cx).unwrap(), "\n")
        })
        .unwrap();
        assert_eq!(cx.windows().len(), 1);
    }
    #[gpui::test]
    fn actual_save_pending_blocks_launcher_open_navigation_and_close(cx: &mut TestAppContext) {
        let view = cx.add_window(|w, cx| Launcher::new("save owner".into(), w, cx));
        view.update(cx, |v, w, cx| {
            v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
            v.refresh_preview(w, cx);
            v.query.read(cx).focus(w);
        })
        .unwrap();
        cx.run_until_parked();
        // Search -> editor -> Enlarge -> Paste -> Clear -> Save.
        cx.simulate_keystrokes(view.into(), "tab tab tab tab tab enter");
        view.update(cx, |v, w, cx| {
            assert!(v.qr_save_pending(cx));
            let selected = v.selected;
            assert!(v.route_qr_key(&key("down", false), w, cx));
            v.launch(w, cx);
            v.close(w, cx);
            assert_eq!(v.selected, selected);
        })
        .unwrap();
        assert_eq!(cx.windows().len(), 1);
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        view.update(cx, |v, w, cx| {
            assert!(!v.qr_save_pending(cx));
            v.close(w, cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(view.root(cx).is_err());
    }
}
