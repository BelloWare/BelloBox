//! Separate source-sized launcher surface; never the application's Home sidebar.
use crate::launcher_clock_ui::LauncherClockPreview;
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::{
    launcher,
    settings::{Settings, config_dir},
};
use gpui::{prelude::*, *};
use std::sync::Arc;

struct Launcher {
    query: Entity<EditorView>,
    preview: Entity<EditorView>,
    input: String,
    selected: usize,
    settings: Settings,
    settings_writable: bool,
    notice: Option<String>,
    jobs: crate::session::SessionJobs,
    qr: Option<Arc<Image>>,
    clock: Option<Entity<LauncherClockPreview>>,
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
            } else if this.was_active && this.clock_active {
                // The compact source palette dismisses on deactivation. Limit
                // this addition to the clock slice; no background preview survives.
                this.jobs.cancel();
                this.clock = None;
                window.remove_window();
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
            clock: None,
            clock_active: false,
            clock_sized: false,
            was_active: window.is_window_active(),
            _subscriptions: vec![subscription, activation],
        };
        app.refresh_preview(window, cx);
        app
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
        self.clock_active = false;
        if let Some(clock) = &self.clock {
            clock.update(cx, |clock, cx| clock.set_active(false, cx));
        }
        let revision = self.jobs.begin();
        self.qr = None;
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
                    LauncherClockPreview::new(self.input.clone(), &self.settings, window, cx)
                }));
            }
            self.clock_active = true;
            if let Some(clock) = &self.clock {
                clock.update(cx, |clock, cx| clock.set_active(true, cx));
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
        if command.id == "qr" {
            if let Err(error) = crate::desktop::qr_jobs::validate(&self.input) {
                self.preview.update(cx, |e, cx| e.set_text(error, cx));
                return;
            }
            self.preview
                .update(cx, |e, cx| e.set_text("Generating QR code…".into(), cx));
            let input = self.input.clone();
            let cancellation = self.jobs.cancellation();
            let task = cx.background_executor().spawn(async move {
                crate::desktop::qr_jobs::generate(&input, true, &cancellation)
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if !this.jobs.accepts(revision) {
                        return;
                    }
                    let Some(result) = result else {
                        return;
                    };
                    let text = match result {
                        Ok(result) => {
                            this.qr =
                                Some(Arc::new(Image::from_bytes(ImageFormat::Png, result.png)));
                            result.terminal.unwrap_or_default()
                        }
                        Err(error) => error,
                    };
                    this.preview.update(cx, |e, cx| e.set_text(text, cx));
                    cx.notify();
                });
            })
            .detach();
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
        if let Some(c) = self.commands(cx).get(self.selected) {
            let handoff = if c.id == "worldClock" && self.clock_active {
                self.clock.as_ref().map(|clock| clock.read(cx).handoff())
            } else {
                None
            };
            // The source clock transfers a value snapshot. Closing the palette
            // releases its preview; the full window owns independent state.
            self.jobs.cancel();
            window.remove_window();
            if let Some(handoff) = handoff {
                crate::desktop::open_clock_handoff(self.input.clone(), handoff, cx);
            } else {
                crate::desktop::open_tool(c.id, self.input.clone(), cx);
            }
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
        if self.clock_active || self.clock_sized {
            let natural = if self.clock_active {
                clock_palette_height(count, !self.input.is_empty() || self.notice.is_some())
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
                if this.query.read(cx).has_marked_text() {
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
                if this.clock_active
                    && let Some(clock) = &this.clock
                    && clock.update(cx, |clock, cx| clock.handle_key(event, window, cx))
                {
                    return;
                }
                let key = event.keystroke.key.as_str();
                let modifiers = event.keystroke.modifiers;
                if this.clock_active
                    && key == "k"
                    && (modifiers.platform || modifiers.control)
                    && !modifiers.alt
                    && !modifiers.shift
                {
                    this.query
                        .update(cx, |editor, cx| editor.set_text(String::new(), cx));
                    this.query.read(cx).focus(window);
                    window.prevent_default();
                    cx.stop_propagation();
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
                        this.jobs.cancel();
                        window.remove_window();
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
                                this.clock = None;
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
                                        this.selected = i;
                                        this.refresh_preview(window, cx);
                                        this.launch(window, cx);
                                    })),
                            )
                            .when(selected && self.clock_active, |s| {
                                s.child(self.clock.as_ref().expect("active clock session").clone())
                            })
                            .when(selected && !self.clock_active, |s| {
                                s.child(
                                    div()
                                        .px(px(10.))
                                        .pb(px(10.))
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .when_some(self.qr.clone(), |s, image| {
                                            s.child(
                                                div()
                                                    .flex()
                                                    .justify_center()
                                                    .child(img(image).size(px(128.))),
                                            )
                                        })
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
                                if let Some(text) = cx.read_from_clipboard().and_then(|v| v.text())
                                {
                                    match bellobox_core::validate_input(&text) {
                                        Ok(()) => {
                                            this.clock = None;
                                            this.selected = 0;
                                            this.query.update(cx, |editor, cx| {
                                                editor.set_text(String::new(), cx)
                                            });
                                            this.input = text;
                                            this.notice = None;
                                        }
                                        Err(e) => {
                                            this.clock = None;
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
        move |window, cx| cx.new(|cx| Launcher::new(input, window, cx)),
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
