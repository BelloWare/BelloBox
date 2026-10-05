//! Home preserves the navigation, hierarchy and dimensions of UI/MainView.swift.
use crate::theme::{self, Palette, opacity};
use bello_platform::{Permission, PermissionState, Platform};
use bello_workbench_ui::{EditorAppearance, EditorView};
use gpui::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Overview,
    Developer,
    Capture,
    Text,
}
impl Category {
    const ALL: [Self; 4] = [Self::Overview, Self::Developer, Self::Capture, Self::Text];
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Developer => "Developer",
            Self::Capture => "Capture",
            Self::Text => "Text & AI",
        }
    }
    fn subtitle(self) -> &'static str {
        match self {
            Self::Overview => "A focused space for the things you do every day.",
            Self::Developer => "Inspect, transform, and build. Your tools stay close.",
            Self::Capture => "Capture a moment, a longer page, or the whole workflow.",
            Self::Text => "Give words, ideas, and time a little more structure.",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Developer => "json",
            Self::Capture => "screenshot",
            Self::Text => "text",
        }
    }
    fn ids(self) -> &'static [&'static str] {
        match self {
            Self::Overview => &[
                "screenshot",
                "recording",
                "worldClock",
                "json",
                "textTools",
                "qr",
                "ai",
                "snippets",
                "compare",
            ],
            Self::Capture => &["screenshot", "scrollCapture", "recording", "videoToGIF"],
            Self::Text => &["ai", "textTools", "qr", "worldClock", "snippets", "compare"],
            Self::Developer => &[],
        }
    }
    fn commands(self) -> Vec<&'static Tool> {
        if self == Self::Developer {
            TOOLS.iter().filter(|tool| tool.group.is_some()).collect()
        } else {
            self.ids().iter().filter_map(|id| tool(id)).collect()
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Data,
    Text,
    Math,
    Design,
    Security,
}
impl Group {
    const DISPLAY_ORDER: [Self; 5] = [
        Self::Data,
        Self::Text,
        Self::Math,
        Self::Design,
        Self::Security,
    ];
    const MENU_ORDER: [Self; 5] = [
        Self::Math,
        Self::Design,
        Self::Data,
        Self::Text,
        Self::Security,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Data => "Data & code",
            Self::Text => "Text & debugging",
            Self::Math => "Math & numbers",
            Self::Design => "Color & design",
            Self::Security => "Network & security",
        }
    }
}
struct Tool {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    symbol: &'static str,
    group: Option<Group>,
    new: bool,
}
fn tool(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.id == id)
}
pub fn tool_title(id: &str) -> &'static str {
    tool(id).map_or("Bello Box", |tool| tool.title)
}
pub fn tool_subtitle(id: &str) -> &'static str {
    tool(id).map_or("Your workspace", |tool| tool.subtitle)
}

pub struct Home {
    category: Category,
    search: Entity<EditorView>,
    group: Option<Group>,
    group_menu: bool,
    new_only: bool,
    focus: FocusHandle,
    icon: Arc<Image>,
    trusted: bool,
    status: Option<String>,
    editor_ink: Hsla,
    card_labels: CardLabelCache,
    _subscriptions: Vec<Subscription>,
}
impl Home {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = theme::for_window(window);
        let search = cx.new(|cx| {
            let mut editor = EditorView::new(String::new(), window, cx);
            editor.set_compact(true, cx);
            editor.set_appearance(filter_appearance(p), cx);
            editor
        });
        let observe = cx.observe(&search, |_, _, cx| cx.notify());
        let appearance = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
        let focus = cx.focus_handle();
        focus.focus(window);
        let category = match std::env::var("BELLOBOX_HOME_CATEGORY").as_deref() {
            Ok("developer") => Category::Developer,
            Ok("capture") => Category::Capture,
            Ok("text") => Category::Text,
            _ => Category::Overview,
        };
        // Poll only a non-prompting permission preflight, as the Swift Home does.
        cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let trusted = selection_trusted();
                        if trusted != this.trusted {
                            this.trusted = trusted;
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
        Self {
            category,
            search,
            group: None,
            group_menu: false,
            new_only: false,
            focus,
            icon: Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!(
                    "../../../../BelloBox/Assets.xcassets/AppIcon.appiconset/icon_128x128@2x.png"
                )
                .to_vec(),
            )),
            trusted: selection_trusted(),
            status: None,
            editor_ink: p.primary,
            card_labels: Rc::default(),
            _subscriptions: vec![observe, appearance],
        }
    }
    fn filtered(&self, cx: &App) -> Vec<&'static Tool> {
        filtered_tools(self.search.read(cx).text(), self.group, self.new_only)
    }
    fn navigate(&mut self, category: Category, window: &mut Window, cx: &mut Context<Self>) {
        self.category = category;
        self.group_menu = false;
        self.focus.focus(window);
        cx.notify();
    }
    fn keyboard(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let command =
            key.modifiers.platform || (cfg!(target_os = "linux") && key.modifiers.control);
        if command {
            if key.key == "k" {
                crate::desktop::open_launcher(String::new(), cx);
                cx.stop_propagation();
            } else if let Some(category) = key
                .key
                .parse::<usize>()
                .ok()
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| Category::ALL.get(index))
                .copied()
            {
                self.navigate(category, window, cx);
                cx.stop_propagation();
            }
        } else if key.key == "escape" && self.group_menu {
            self.group_menu = false;
            cx.notify();
            cx.stop_propagation();
        } else if key.key == "enter"
            && self.category == Category::Developer
            && self.search.read(cx).focus_handle(cx).is_focused(window)
        {
            let matches = self.filtered(cx);
            if matches.len() == 1 {
                crate::desktop::open_tool(matches[0].id, String::new(), cx);
            }
            cx.stop_propagation();
        }
    }
    fn sidebar(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut sidebar = div()
            .w(px(200.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(6.))
            .bg(p.surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(16.))
                    .py(px(22.))
                    .child(img(self.icon.clone()).size(px(38.)).flex_none())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Bello Box"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(p.secondary)
                                    .child("Your workspace"),
                            ),
                    ),
            );
        for category in Category::ALL {
            let selected = category == self.category;
            sidebar = sidebar.child(
                div().px(px(10.)).child(
                    div()
                        .id(SharedString::from(format!(
                            "homeCategory_{}",
                            category.title()
                        )))
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .h(px(40.))
                        .px(px(12.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .text_color(if selected { p.accent } else { p.primary })
                        .text_size(px(13.))
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .when(selected, |view| {
                            view.bg(opacity(p.accent, 0.09)).child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top(px(12.))
                                    .w(px(3.))
                                    .h(px(16.))
                                    .rounded(px(2.))
                                    .bg(p.accent),
                            )
                        })
                        .hover(|s| s.bg(opacity(p.accent, 0.07)))
                        .child(div().w(px(20.)).flex().justify_center().child(glyph(
                            category.icon(),
                            15.,
                            if selected { p.accent } else { p.primary },
                        )))
                        .child(category.title())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.navigate(category, window, cx)
                        })),
                ),
            );
        }
        sidebar = sidebar
            .child(div().flex_1().min_h(px(20.)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(18.))
                    .text_size(px(11.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(if self.trusted { p.teal } else { p.secondary })
                            .child(glyph(
                                if self.trusted { "shield" } else { "lock" },
                                12.,
                                if self.trusted { p.teal } else { p.secondary },
                            ))
                            .child(if self.trusted {
                                "Selection tools ready"
                            } else {
                                "Selection access needed"
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(p.secondary)
                            .child(glyph("sparkles", 12., p.secondary))
                            .child(if ai_is_configured() {
                                "AI connected"
                            } else {
                                "AI is optional"
                            }),
                    ),
            )
            .child(div().mx(px(16.)).h(px(1.)).bg(p.separator));
        for (id, label, icon, route) in [
            ("homeSettings", "Settings", "settings", "settings"),
            ("homeSetup", "Setup guide", "help", "setup"),
            ("homeUpdates", "Check for updates", "updates", "updates"),
        ] {
            sidebar = sidebar.child(
                div().px(px(10.)).child(
                    div()
                        .id(id)
                        .h(px(40.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .text_size(px(12.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|s| s.bg(opacity(p.accent, 0.07)))
                        .child(glyph(icon, 13., p.secondary))
                        .child(label)
                        .on_click(move |_, _, cx| {
                            crate::desktop::open_tool(route, String::new(), cx)
                        }),
                ),
            );
        }
        sidebar
            .child(
                div()
                    .p(px(18.))
                    .text_size(px(10.))
                    .text_color(opacity(p.secondary, 0.55))
                    .child(format!("Version {}", source_version())),
            )
            .into_any_element()
    }
    fn search_card(&self, p: Palette) -> AnyElement {
        div()
            .id("mainLauncherButton")
            .w_full()
            .p(px(16.))
            .flex()
            .items_center()
            .gap(px(12.))
            .bg(p.surface)
            .border_1()
            .border_color(p.separator)
            .rounded(px(12.))
            .cursor_pointer()
            .shadow(source_shadow(0.035))
            .hover(|s| s.border_color(opacity(p.accent, 0.45)))
            .child(glyph("search", 16., p.accent))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Search all tools and commands"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(format!("{} commands. One place to start.", TOOLS.len())),
                    ),
            )
            .child(
                div()
                    .px(px(7.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.separator)
                    .text_size(px(11.))
                    .font_family("monospace")
                    .text_color(p.secondary)
                    .child("⌃⌥⌘B"),
            )
            .on_click(|_, _, cx| crate::desktop::open_launcher(String::new(), cx))
            .into_any_element()
    }
    fn permission_notice(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        div().p(px(14.)).flex().items_center().gap(px(12.)).rounded(px(12.)).bg(p.surface).border_1().border_color(p.separator)
            .child(glyph("unlock", 16., p.accent))
            .child(div().flex_1().flex().flex_col().gap(px(3.))
                .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("Connect to your selection"))
                .child(div().text_size(px(11.)).text_color(p.secondary).child(self.status.clone().unwrap_or_else(|| if cfg!(target_os = "macos") {
                    "Allow Accessibility to read and replace selected text. You can use the other tools now.".into()
                } else {"Selection access is unavailable on this platform. You can use the other tools now.".into()}))))
            .child(div().id("homeAllowAccess").px(px(11.)).h(px(28.)).flex().items_center().rounded(px(8.))
                .bg(p.surface).border_1().border_color(p.separator).text_size(px(12.)).font_weight(FontWeight::MEDIUM)
                .cursor_pointer().hover(|s| s.border_color(opacity(p.accent, 0.45))).child(if cfg!(target_os = "macos") {"Allow access"} else {"View status"})
                .on_click(cx.listener(|this, _, _, cx| {
                    if cfg!(target_os = "macos") {
                        if let Err(error) = Platform::new().open_permission_settings(Permission::Accessibility) {this.status = Some(error.to_string()); cx.notify();}
                    } else { crate::desktop::open_tool("settings", String::new(), cx); }
                }))).into_any_element()
    }
    fn filters(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let query_empty = self.search.read(cx).text().is_empty();
        let input = self.search.clone();
        let mut group_button = div()
            .id("homeGroupMenu")
            .relative()
            .px(px(10.))
            .h(px(39.))
            .min_w(px(110.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .text_size(px(12.))
            .bg(p.well)
            .rounded(px(9.))
            .cursor_pointer()
            .child(self.group.map_or("All groups", Group::title))
            .child(glyph("chevronDown", 9., p.secondary))
            .on_click(cx.listener(|this, _, _, cx| {
                this.group_menu = !this.group_menu;
                cx.notify();
            }));
        if self.group_menu {
            let mut menu = div()
                .id("homeGroupOptions")
                .absolute()
                .top(px(44.))
                .right_0()
                .w(px(185.))
                .p(px(4.))
                .flex()
                .flex_col()
                .bg(p.surface)
                .border_1()
                .border_color(p.border)
                .rounded(px(8.))
                .shadow_lg()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.group_menu = false;
                    cx.notify();
                }));
            for (i, group) in std::iter::once(None)
                .chain(Group::MENU_ORDER.map(Some))
                .enumerate()
            {
                menu = menu.child(
                    div()
                        .id(("homeGroupOption", i))
                        .h(px(30.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(5.))
                        .hover(|s| s.bg(opacity(p.accent, 0.09)))
                        .child(group.map_or("All groups", Group::title))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.group = group;
                            this.group_menu = false;
                            cx.notify();
                        })),
                );
            }
            group_button = group_button.child(deferred(menu).with_priority(1));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .id("homeDeveloperSearch")
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(10.))
                            .h(px(39.))
                            .bg(p.well)
                            .rounded(px(9.))
                            .border_1()
                            .border_color(p.separator)
                            .child(glyph("search", 13., p.secondary))
                            .child(
                                div()
                                    .relative()
                                    .h(px(24.))
                                    .flex_1()
                                    .min_w_0()
                                    .when(query_empty, |view| {
                                        view.child(
                                            div()
                                                .absolute()
                                                .left_0()
                                                .top(px(4.))
                                                .text_size(px(13.))
                                                .text_color(p.secondary)
                                                .child("Find a developer tool…"),
                                        )
                                    })
                                    .child(self.search.clone()),
                            )
                            .when(!query_empty, |view| {
                                view.child(
                                    div()
                                        .id("homeClearSearch")
                                        .cursor_pointer()
                                        .child(glyph("closeCircle", 13., p.secondary))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.search.update(cx, |editor, cx| {
                                                editor.set_text(String::new(), cx)
                                            })
                                        })),
                                )
                            })
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                input.read(cx).focus(window)
                            }),
                    )
                    .child(group_button),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.))
                    .child(
                        div()
                            .id("homeNewToolsOnly")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .child(
                                div()
                                    .size(px(12.))
                                    .rounded(px(2.))
                                    .border_1()
                                    .border_color(if self.new_only {
                                        p.accent_fill
                                    } else {
                                        p.border
                                    })
                                    .when(self.new_only, |view| {
                                        view.bg(p.accent_fill).child(glyph(
                                            "check",
                                            10.,
                                            gpui::white(),
                                        ))
                                    }),
                            )
                            .child("New in 0.0.74")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.new_only = !this.new_only;
                                cx.notify();
                            })),
                    )
                    .child(div().text_color(p.secondary).child(format!(
                        "{} tools · Enter opens a single match",
                        self.filtered(cx).len()
                    ))),
            )
            .into_any_element()
    }
    fn grid(&self, tools: Vec<&'static Tool>, columns: u16, p: Palette) -> AnyElement {
        div()
            .grid()
            .grid_cols(columns)
            .gap(px(12.))
            .children(tools.into_iter().map(|tool| {
                let id = tool.id;
                div()
                    .id(SharedString::from(format!("homeTool_{id}")))
                    .min_w_0()
                    .h(px(95.))
                    .p(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .bg(p.surface)
                    .rounded(px(12.))
                    .border_1()
                    .border_color(p.separator)
                    .cursor_pointer()
                    .shadow(source_shadow(0.035))
                    .hover(|s| s.border_color(opacity(p.accent, 0.45)))
                    .child(
                        div()
                            .flex_none()
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(10.08))
                            .bg(opacity(p.accent, 0.11))
                            .border_1()
                            .border_color(opacity(p.accent, 0.10))
                            .child(tool_icon(id, 15.48, p)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(card_label(
                                tool.title,
                                13.,
                                16.,
                                FontWeight::SEMIBOLD,
                                p.primary,
                                self.card_labels.clone(),
                            ))
                            .child(card_label(
                                tool.subtitle,
                                11.,
                                15.,
                                FontWeight::NORMAL,
                                p.secondary,
                                self.card_labels.clone(),
                            )),
                    )
                    .child(glyph("chevronRight", 10., opacity(p.secondary, 0.45)))
                    .on_click(move |_, _, cx| crate::desktop::open_tool(id, String::new(), cx))
            }))
            .into_any_element()
    }
}
// Swift HomeToolCard reserves two lines and truncates only the final line.
// GPUI 0.2.2's line_clamp + text_ellipsis truncates to 2 * width before word
// wrapping, so unused space on line one can push the ellipsis outside line two.
// Shape at the final width instead. The bounded cache keeps one layout per
// catalog label and invalidates on width or any inherited typography change.
type CardLabelCache = Rc<RefCell<HashMap<&'static str, CachedCardLabel>>>;
struct CachedCardLabel {
    width: Pixels,
    style: TextStyle,
    lines: Vec<ShapedLine>,
}
fn card_label(
    text: &'static str,
    font_size: f32,
    line_height: f32,
    weight: FontWeight,
    ink: Hsla,
    cache: CardLabelCache,
) -> Div {
    div()
        .h(px(2. * line_height))
        .overflow_hidden()
        .text_size(px(font_size))
        .line_height(px(line_height))
        .font_weight(weight)
        .text_color(ink)
        .child(
            canvas(
                move |bounds, window, _| {
                    let style = window.text_style();
                    let mut cache = cache.borrow_mut();
                    if let Some(cached) = cache.get(text)
                        && cached.width == bounds.size.width
                        && cached.style == style
                    {
                        return cached.lines.clone();
                    }
                    let shape = |value: &str| {
                        window.text_system().shape_line(
                            value.to_owned().into(),
                            px(font_size),
                            &[style.to_run(value.len())],
                            None,
                        )
                    };
                    let wrapped = window.text_system().shape_text(
                        text.into(),
                        px(font_size),
                        &[style.to_run(text.len())],
                        Some(bounds.size.width),
                        None,
                    );
                    // If wrapping fails, retain a visible, measured single-line
                    // tail-truncated fallback rather than terminating the app.
                    let first_end = wrapped
                        .as_ref()
                        .ok()
                        .and_then(|lines| {
                            lines
                                .first()
                                .and_then(|line| {
                                    line.wrap_boundaries.first().map(|boundary| {
                                        line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index
                                    })
                                })
                                .or(Some(text.len()))
                        })
                        .unwrap_or(0);
                    let lines = card_line_strings(text, first_end, |value| {
                        shape(value).width <= bounds.size.width
                    })
                    .iter()
                    .filter(|value| !value.is_empty())
                    .map(|value| shape(value))
                    .collect::<Vec<_>>();
                    cache.insert(
                        text,
                        CachedCardLabel {
                            width: bounds.size.width,
                            style,
                            lines: lines.clone(),
                        },
                    );
                    lines
                },
                move |bounds, lines, window, cx| {
                    for (index, line) in lines.iter().enumerate() {
                        let origin = bounds.origin + point(px(0.), px(index as f32 * line_height));
                        let _ = line.paint(origin, px(line_height), window, cx);
                    }
                },
            )
            .w_full()
            .h_full(),
        )
}

fn card_line_strings(text: &str, first_end: usize, fits: impl Fn(&str) -> bool) -> Vec<String> {
    // Font shaping gives a byte boundary; keep composed characters and emoji
    // intact even if a platform shaper offers a boundary inside a grapheme.
    let first_end = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .take_while(|index| *index <= first_end)
        .last()
        .unwrap_or(0);
    if first_end == text.len() {
        return vec![text.to_owned()];
    }
    let first = text[..first_end].trim_end().to_owned();
    let remainder = text[first_end..].trim_start();
    if fits(remainder) {
        return vec![first, remainder.to_owned()];
    }
    let mut last = String::new();
    // Catalog labels are short and this runs only when width/style changes.
    // Test actual shaped prefixes rather than additive per-character estimates.
    for (end, _) in remainder.grapheme_indices(true) {
        let candidate = format!("{}…", remainder[..end].trim_end());
        if fits(&candidate) {
            last = candidate;
        }
    }
    vec![first, last]
}

impl Render for Home {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        if self.editor_ink != p.primary {
            self.editor_ink = p.primary;
            self.search.update(cx, |editor, cx| {
                editor.set_appearance(filter_appearance(p), cx)
            });
        }
        let columns = grid_columns(f32::from(window.viewport_size().width));
        let commands = self.category.commands();
        let mut content = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(px(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if self.category == Category::Overview {
                                "Everything within reach."
                            } else {
                                self.category.title()
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(15.))
                            .text_color(p.secondary)
                            .child(self.category.subtitle()),
                    ),
            )
            .child(self.search_card(p));
        if !self.trusted {
            content = content.child(self.permission_notice(p, cx));
        }
        content = content.child(section_heading(
            if self.category == Category::Overview {
                "Quick access".into()
            } else {
                format!("{} tools", self.category.title())
            },
            format!("{} tools", commands.len()),
            p,
        ));
        if self.category == Category::Developer {
            content = content.child(self.filters(p, cx));
            let matches = self.filtered(cx);
            for group in Group::DISPLAY_ORDER {
                let tools: Vec<_> = matches
                    .iter()
                    .copied()
                    .filter(|tool| tool.group == Some(group))
                    .collect();
                if !tools.is_empty() {
                    content = content
                        .child(section_heading(
                            group.title().into(),
                            format!("{} tools", tools.len()),
                            p,
                        ))
                        .child(self.grid(tools, columns, p));
                }
            }
            if matches.is_empty() {
                content = content.child(
                    div()
                        .p(px(30.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(12.))
                        .rounded(px(12.))
                        .bg(p.surface)
                        .border_1()
                        .border_color(p.separator)
                        .child(glyph("search", 24., p.secondary))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("No tools match these filters"),
                        )
                        .child(
                            div()
                                .id("homeClearFilters")
                                .h(px(28.))
                                .px(px(11.))
                                .flex()
                                .items_center()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(p.separator)
                                .cursor_pointer()
                                .text_size(px(12.))
                                .child("Clear filters")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.group = None;
                                    this.new_only = false;
                                    this.search.update(cx, |editor, cx| {
                                        editor.set_text(String::new(), cx)
                                    });
                                    cx.notify();
                                })),
                        ),
                );
            }
        } else {
            content = content.child(self.grid(commands, columns, p));
        }
        content = content.child(div().p(px(14.)).flex().items_start().gap(px(10.)).bg(p.surface).rounded(px(12.)).border_1().border_color(p.separator)
            .child(glyph("cursor", 14., p.accent))
            .child(div().flex_1().text_size(px(11.)).line_height(px(15.)).text_color(p.secondary)
                .child("Select text in another app, then press ⌃⌥⌘B for tools that fit your selection.")));
        div()
            .id("home")
            .size_full()
            .overflow_hidden()
            .flex()
            .font_family(theme::ui_font())
            .text_color(p.primary)
            .bg(p.bg)
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::keyboard))
            .child(self.sidebar(p, cx))
            .child(div().w(px(1.)).h_full().flex_none().bg(p.separator))
            .child(
                div()
                    .id(SharedString::from(format!(
                        "homeContent_{}",
                        self.category.title()
                    )))
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .child(content),
            )
    }
}
fn selection_trusted() -> bool {
    Platform::new()
        .status()
        .permissions
        .iter()
        .any(|permission| {
            permission.permission == Permission::Accessibility
                && permission.state == PermissionState::Granted
        })
}
fn filter_appearance(p: Palette) -> EditorAppearance {
    EditorAppearance {
        font_family: theme::ui_font().into(),
        font_size: 13.,
        line_height: 19.,
        padding_x: 0.,
        padding_y: 3.,
        text: p.primary,
        caret: p.accent,
        selection: opacity(p.accent, 0.20),
        background: None,
        ..EditorAppearance::plain()
    }
}
fn section_heading(title: String, detail: String, p: Palette) -> AnyElement {
    div()
        .flex()
        .items_baseline()
        .justify_between()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(p.secondary)
                .child(detail),
        )
        .into_any_element()
}
fn grid_columns(window_width: f32) -> u16 {
    // Sidebar + divider + both content paddings; 240 pt adaptive minimum, 12 pt gaps.
    (((window_width - 200. - 1. - 48. + 12.) / 252.).floor() as u16).max(1)
}
fn filtered_tools(query: &str, group: Option<Group>, new_only: bool) -> Vec<&'static Tool> {
    let query = query.trim().to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    let catalog = bellobox_core::launcher::catalog();
    let mut tools: Vec<_> = TOOLS
        .iter()
        .filter(|tool| {
            if tool.group.is_none()
                || group.is_some_and(|group| tool.group != Some(group))
                || (new_only && !tool.new)
            {
                return false;
            }
            let extra = catalog
                .iter()
                .find(|command| command.id == tool.id)
                .map_or("", |command| command.keywords);
            let haystack =
                format!("{} {} {} {}", tool.title, tool.subtitle, tool.id, extra).to_lowercase();
            terms.iter().all(|term| haystack.contains(term))
        })
        .collect();
    // Match the source's explicit title preference while retaining catalog order for ties.
    tools.sort_by_key(|tool| {
        let title = tool.title.to_lowercase();
        if query.is_empty() {
            0
        } else if title.starts_with(&query) {
            -10000
        } else if terms
            .iter()
            .all(|term| title.split_whitespace().any(|word| word.starts_with(term)))
        {
            -6500
        } else if terms.iter().all(|term| title.contains(term)) {
            -5000
        } else {
            0
        }
    });
    tools
}

// Labels and grouping mirrored from LauncherCatalog.swift, AdditionalUtility.swift,
// and ExtendedUtilityDefinitions.swift (0.0.74). These are display-only metadata.
const TOOLS: &[Tool] = &[
    Tool {
        id: "json",
        title: "JSON Tools",
        subtitle: "Pretty-print, minify, and validate without rounding numbers",
        symbol: "curlybraces",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "compare",
        title: "Compare Text & JSON",
        subtitle: "Find changes between selections, clipboard text, and JSON fields",
        symbol: "arrow.left.arrow.right",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "jwt",
        title: "Inspect JWT",
        subtitle: "Read token claims and expiration times locally",
        symbol: "key.horizontal",
        group: Some(Group::Security),
        new: false,
    },
    Tool {
        id: "regex",
        title: "Regex Tester",
        subtitle: "Test patterns, inspect groups, extract, and replace",
        symbol: "asterisk",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "url",
        title: "URL & Query Editor",
        subtitle: "Edit URL components and repeated query parameters",
        symbol: "link",
        group: Some(Group::Security),
        new: false,
    },
    Tool {
        id: "time",
        title: "Timestamp Converter",
        subtitle: "Unix seconds, milliseconds, ISO dates, and time differences",
        symbol: "clock",
        group: Some(Group::Math),
        new: false,
    },
    Tool {
        id: "cron",
        title: "Cron Schedule",
        subtitle: "Explain five-field cron and preview upcoming runs",
        symbol: "calendar.badge.clock",
        group: Some(Group::Math),
        new: false,
    },
    Tool {
        id: "convert",
        title: "Convert JSON, YAML & CSV",
        subtitle: "Convert structured data and preview rows as a table",
        symbol: "tablecells",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "snippets",
        title: "Snippets & Templates",
        subtitle: "Save reusable text with fields you fill before inserting",
        symbol: "text.badge.plus",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "http",
        title: "HTTP & cURL",
        subtitle: "Import a cURL command, edit the request, then send",
        symbol: "network",
        group: Some(Group::Security),
        new: false,
    },
    Tool {
        id: "generate",
        title: "Developer Generators",
        subtitle: "UUIDs, random strings, timestamps, and sample records",
        symbol: "number",
        group: Some(Group::Math),
        new: false,
    },
    Tool {
        id: "calculator",
        title: "Calculator",
        subtitle: "Evaluate arithmetic, powers, constants, and math functions",
        symbol: "plus.forwardslash.minus",
        group: Some(Group::Math),
        new: false,
    },
    Tool {
        id: "units",
        title: "Unit Converter",
        subtitle: "Convert length, mass, temperature, data, duration, and speed",
        symbol: "ruler",
        group: Some(Group::Math),
        new: false,
    },
    Tool {
        id: "numberBase",
        title: "Number Base Converter",
        subtitle: "Convert exact integers between binary, octal, decimal, and hex",
        symbol: "number.square",
        group: Some(Group::Math),
        new: false,
    },
    Tool {
        id: "color",
        title: "Color Converter",
        subtitle: "Convert HEX, RGB, and HSL with a live color swatch",
        symbol: "paintpalette",
        group: Some(Group::Design),
        new: false,
    },
    Tool {
        id: "contrast",
        title: "Contrast Checker",
        subtitle: "Check two colors against WCAG AA and AAA contrast thresholds",
        symbol: "circle.lefthalf.filled",
        group: Some(Group::Design),
        new: false,
    },
    Tool {
        id: "gradient",
        title: "CSS Gradient Builder",
        subtitle: "Design a two-color linear gradient and copy its CSS",
        symbol: "rectangle.leadinghalf.inset.filled",
        group: Some(Group::Design),
        new: false,
    },
    Tool {
        id: "markdown",
        title: "Markdown Preview",
        subtitle: "Preview headings, lists, quotes, and code; export safe HTML",
        symbol: "doc.richtext",
        group: Some(Group::Design),
        new: false,
    },
    Tool {
        id: "jsonPointer",
        title: "JSON Pointer",
        subtitle: "Read a JSON value by its RFC 6901 pointer",
        symbol: "scope",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "jsonFlatten",
        title: "Flatten & Unflatten JSON",
        subtitle: "Round-trip nested JSON through typed path entries",
        symbol: "square.stack.3d.down.right",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "jsonCode",
        title: "JSON to Code",
        subtitle: "Infer TypeScript interfaces or Swift Codable models from JSON",
        symbol: "chevron.left.forwardslash.chevron.right",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "sqlInsert",
        title: "SQL INSERT Builder",
        subtitle: "Turn JSON records into quoted SQL statements, without executing",
        symbol: "externaldrive.badge.plus",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "xmlJSON",
        title: "XML to JSON",
        subtitle: "Convert XML to ordered JSON with attributes and mixed text intact",
        symbol: "doc.badge.gearshape",
        group: Some(Group::Data),
        new: false,
    },
    Tool {
        id: "unicode",
        title: "Unicode Inspector",
        subtitle: "Inspect code points, UTF encodings, names, and normalization",
        symbol: "character.cursor.ibeam",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "stringEscape",
        title: "String Literal Escaper",
        subtitle: "Quote JSON, Swift, or shell strings without running code",
        symbol: "text.quote",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "extract",
        title: "Extract Links & Emails",
        subtitle: "Collect unique links or email addresses from a block of text",
        symbol: "line.3.horizontal.decrease.circle",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "listSet",
        title: "List Set Operations",
        subtitle: "Find common, combined, or different items in two line lists",
        symbol: "circle.grid.2x1",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "semver",
        title: "Semantic Versions",
        subtitle: "Compare and sort strict semantic versions, including prereleases",
        symbol: "tag",
        group: Some(Group::Text),
        new: false,
    },
    Tool {
        id: "subnet",
        title: "IPv4 Subnet Calculator",
        subtitle: "Inspect a CIDR network, netmask, address range, and host count",
        symbol: "point.3.connected.trianglepath.dotted",
        group: Some(Group::Security),
        new: false,
    },
    Tool {
        id: "chmod",
        title: "Chmod Permissions",
        subtitle: "Build Unix permissions with an interactive read/write/execute grid",
        symbol: "lock.shield",
        group: Some(Group::Security),
        new: false,
    },
    Tool {
        id: "hmac",
        title: "HMAC Signer",
        subtitle: "Sign text locally with SHA-256 or SHA-512 and an ephemeral key",
        symbol: "signature",
        group: Some(Group::Security),
        new: false,
    },
    Tool {
        id: "jsonSchema",
        title: "JSON Schema Validator",
        subtitle: "Check types, required fields, bounds, and local schema references",
        symbol: "checkmark.seal",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "jsonMerge",
        title: "JSON Merge Patch",
        subtitle: "Apply an RFC 7396 patch locally; null removes object fields",
        symbol: "arrow.triangle.merge",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "jsonRedact",
        title: "JSON Field Redactor",
        subtitle: "Replace matching field names at every depth before sharing JSON",
        symbol: "eye.slash",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "jsonLines",
        title: "JSON Lines",
        subtitle: "Validate NDJSON records and convert to or from a JSON array",
        symbol: "list.bullet.rectangle",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "csvExplore",
        title: "CSV Explorer",
        subtitle: "Filter rows, choose columns, remove duplicates, and copy CSV",
        symbol: "tablecells.badge.ellipsis",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "envFile",
        title: "Environment File",
        subtitle: "Validate .env assignments and convert JSON without evaluating variables",
        symbol: "terminal",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "plist",
        title: "Property List Converter",
        subtitle: "Convert XML plist and typed JSON, preserving dates and binary data",
        symbol: "list.bullet.indent",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "sqlFormat",
        title: "SQL Formatter",
        subtitle: "Lay out SQL clauses while keeping literals, comments, and quoted names",
        symbol: "text.alignleft",
        group: Some(Group::Data),
        new: true,
    },
    Tool {
        id: "httpHeaders",
        title: "HTTP Header Inspector",
        subtitle: "Inspect raw request or response headers with duplicate fields intact",
        symbol: "network.badge.shield.half.filled",
        group: Some(Group::Security),
        new: true,
    },
    Tool {
        id: "cookies",
        title: "Cookie Inspector",
        subtitle: "Inspect Cookie or Set-Cookie fields without accessing browser storage",
        symbol: "circle.hexagongrid",
        group: Some(Group::Security),
        new: true,
    },
    Tool {
        id: "certificate",
        title: "Certificate Inspector",
        subtitle: "Read PEM certificate subjects, dates, extensions, and SHA-256 fingerprints",
        symbol: "checkmark.shield",
        group: Some(Group::Security),
        new: true,
    },
    Tool {
        id: "sshKey",
        title: "SSH Public Key Inspector",
        subtitle: "Inspect OpenSSH RSA, Ed25519, and ECDSA keys and their fingerprints",
        symbol: "key.viewfinder",
        group: Some(Group::Security),
        new: true,
    },
    Tool {
        id: "uuidInspect",
        title: "UUID Inspector",
        subtitle: "Read UUID versions, variants, bytes, and v1/v6/v7 timestamps",
        symbol: "number.square.fill",
        group: Some(Group::Security),
        new: true,
    },
    Tool {
        id: "bitwise",
        title: "Bitwise Calculator",
        subtitle: "Inspect fixed-width AND, OR, XOR, shifts, rotations, and signed results",
        symbol: "switch.2",
        group: Some(Group::Math),
        new: true,
    },
    Tool {
        id: "statistics",
        title: "Statistics Calculator",
        subtitle: "Summarize a number series with percentiles, spread, and a histogram",
        symbol: "chart.bar.xaxis",
        group: Some(Group::Math),
        new: true,
    },
    Tool {
        id: "dateMath",
        title: "Date Calculator",
        subtitle: "Add calendar days, weekdays, months, or years with an explicit time zone",
        symbol: "calendar.badge.plus",
        group: Some(Group::Math),
        new: true,
    },
    Tool {
        id: "aspectRatio",
        title: "Aspect Ratio Calculator",
        subtitle: "Reduce dimensions and calculate a proportional target width or height",
        symbol: "aspectratio",
        group: Some(Group::Design),
        new: true,
    },
    Tool {
        id: "bezier",
        title: "CSS Bézier Curve",
        subtitle: "Tune easing curves with a live graph and copy cubic-bezier CSS",
        symbol: "point.topleft.down.to.point.bottomright.curvepath",
        group: Some(Group::Design),
        new: true,
    },
    Tool {
        id: "boxShadow",
        title: "CSS Box Shadow",
        subtitle: "Build an outer shadow with a live card and ready-to-paste CSS",
        symbol: "square.on.square",
        group: Some(Group::Design),
        new: true,
    },
    Tool {
        id: "textTable",
        title: "Text Table Builder",
        subtitle: "Turn CSV into aligned Markdown, plain text, or escaped HTML tables",
        symbol: "tablecells",
        group: Some(Group::Text),
        new: true,
    },
    Tool {
        id: "ai",
        title: "Ask AI",
        subtitle: "Rewrite, explain, summarize, or ask about the selected text",
        symbol: "wand.and.stars",
        group: None,
        new: false,
    },
    Tool {
        id: "screenshot",
        title: "Screenshot",
        subtitle: "Capture an area, window, or screen and annotate",
        symbol: "camera.viewfinder",
        group: None,
        new: false,
    },
    Tool {
        id: "scrollCapture",
        title: "Scrolling Screenshot",
        subtitle: "Capture a scrolling page and stitch it into one tall image",
        symbol: "arrow.down.doc",
        group: None,
        new: false,
    },
    Tool {
        id: "recording",
        title: "Screen Recording",
        subtitle: "Record screen, audio, cursor, clicks, and keys as a movie or GIF",
        symbol: "record.circle",
        group: None,
        new: false,
    },
    Tool {
        id: "videoToGIF",
        title: "Video to GIF",
        subtitle: "Turn a movie on this Mac into a trimmed, resized GIF",
        symbol: "film.stack",
        group: None,
        new: false,
    },
    Tool {
        id: "worldClock",
        title: "World Clock",
        subtitle: "Compare live time or plan a meeting across locations",
        symbol: "globe",
        group: None,
        new: false,
    },
    Tool {
        id: "qr",
        title: "QR Code",
        subtitle: "Create a scannable code from text or a link",
        symbol: "qrcode",
        group: None,
        new: false,
    },
    Tool {
        id: "textTools",
        title: "Text Tools",
        subtitle: "Case, encode, decode, hashes, lines, and counts",
        symbol: "wrench.and.screwdriver",
        group: None,
        new: false,
    },
    Tool {
        id: "settings",
        title: "Settings",
        subtitle: "Shortcuts, permissions, providers, and defaults",
        symbol: "gearshape",
        group: None,
        new: false,
    },
    Tool {
        id: "home",
        title: "Home & Status",
        subtitle: "App status, setup guide, and updates",
        symbol: "house",
        group: None,
        new: false,
    },
];

/// Small source-style monochrome line symbols. All geometry is authored here;
/// no emoji fonts, image substitutions, or third-party icon assets are used.
pub fn tool_icon(id: &str, size: f32, p: Palette) -> AnyElement {
    glyph(id, size, p.accent)
}
fn glyph(id: &str, size: f32, ink: Hsla) -> AnyElement {
    let symbol = tool(id).map_or(id, |tool| tool.symbol).to_string();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let mut drawing = SymbolDrawing::new();
            match symbol.as_str() {
                "overview" => {
                    for x in [3., 14.] {
                        for y in [3., 14.] {
                            drawing.rect(x, y, 7., 7.);
                        }
                    }
                }
                "curlybraces" => {
                    drawing.line(&[
                        (9., 2.),
                        (6., 2.),
                        (5., 4.),
                        (5., 9.),
                        (3., 12.),
                        (5., 15.),
                        (5., 20.),
                        (6., 22.),
                        (9., 22.),
                    ]);
                    drawing.line(&[
                        (15., 2.),
                        (18., 2.),
                        (19., 4.),
                        (19., 9.),
                        (21., 12.),
                        (19., 15.),
                        (19., 20.),
                        (18., 22.),
                        (15., 22.),
                    ]);
                }
                "camera.viewfinder" => {
                    drawing.line(&[(7., 2.), (3., 2.), (2., 3.), (2., 7.)]);
                    drawing.line(&[(17., 2.), (21., 2.), (22., 3.), (22., 7.)]);
                    drawing.line(&[(2., 17.), (2., 21.), (3., 22.), (7., 22.)]);
                    drawing.line(&[(17., 22.), (21., 22.), (22., 21.), (22., 17.)]);
                    drawing.line(&[
                        (5., 8.),
                        (9., 8.),
                        (10., 6.),
                        (14., 6.),
                        (15., 8.),
                        (19., 8.),
                        (19., 17.),
                        (5., 17.),
                        (5., 8.),
                    ]);
                    drawing.circle(12., 12.5, 3.);
                }
                "record.circle" => {
                    drawing.circle(12., 12., 9.);
                    drawing.circle(12., 12., 4.);
                }
                "globe" => {
                    drawing.circle(12., 12., 10.);
                    drawing.ellipse(12., 12., 4.8, 10.);
                    drawing.line(&[(2., 12.), (22., 12.)]);
                    drawing.line(&[(4., 6.5), (20., 6.5)]);
                    drawing.line(&[(4., 17.5), (20., 17.5)]);
                }
                "arrow.left.arrow.right" => {
                    drawing.line(&[(3., 7.), (20., 7.), (16., 3.)]);
                    drawing.line(&[(20., 7.), (16., 11.)]);
                    drawing.line(&[(21., 17.), (4., 17.), (8., 13.)]);
                    drawing.line(&[(4., 17.), (8., 21.)]);
                }
                "wrench.and.screwdriver" => {
                    drawing.line(&[
                        (3., 4.),
                        (6., 2.),
                        (11., 7.),
                        (9., 9.),
                        (22., 20.),
                        (20., 22.),
                        (7., 11.),
                        (5., 12.),
                        (1., 7.),
                        (3., 4.),
                    ]);
                    drawing.line(&[
                        (15., 2.),
                        (13., 6.),
                        (14., 10.),
                        (3., 20.),
                        (5., 22.),
                        (16., 12.),
                        (20., 12.),
                        (23., 8.),
                        (19., 9.),
                        (16., 6.),
                        (17., 2.),
                        (15., 2.),
                    ]);
                }
                "qrcode" => {
                    for (x, y) in [(2., 2.), (15., 2.), (2., 15.)] {
                        drawing.rect(x, y, 7., 7.);
                        drawing.rect(x + 2., y + 2., 3., 3.);
                    }
                    drawing.line(&[
                        (15., 15.),
                        (18., 15.),
                        (18., 19.),
                        (22., 19.),
                        (22., 22.),
                        (15., 22.),
                        (15., 19.),
                    ]);
                    drawing.line(&[(22., 14.), (22., 16.)]);
                    drawing.line(&[(11.5, 3.), (11.5, 12.), (3., 12.)]);
                }
                "wand.and.stars" => {
                    drawing.line(&[(4., 20.), (16., 8.), (18., 10.), (6., 22.), (4., 20.)]);
                    drawing.line(&[(12., 12.), (14., 14.)]);
                    drawing.star(6., 6., 4.);
                    drawing.star(20., 4., 2.);
                    drawing.star(20., 16., 2.);
                }
                "sparkles" => {
                    drawing.star(12., 12., 7.);
                    drawing.star(4., 4., 3.);
                    drawing.star(21., 21., 2.);
                }
                "text.badge.plus" => {
                    drawing.circle(6., 6., 3.);
                    drawing.line(&[(12., 3.), (21., 3.)]);
                    drawing.line(&[(12., 8.), (20., 8.)]);
                    drawing.line(&[(3., 13.), (21., 13.)]);
                    drawing.line(&[(3., 18.), (16., 18.)]);
                    drawing.line(&[(20., 16.), (20., 23.)]);
                    drawing.line(&[(16.5, 19.5), (23.5, 19.5)]);
                }
                "text" | "text.alignleft" => {
                    for (y, end) in [(4., 21.), (9., 15.), (14., 21.), (19., 15.)] {
                        drawing.line(&[(3., y), (end, y)]);
                    }
                }
                "search" => {
                    drawing.circle(10., 10., 7.);
                    drawing.line(&[(15., 15.), (22., 22.)]);
                }
                "lock" | "lock.shield" => {
                    drawing.rect(5., 10., 14., 11.);
                    drawing.arc(
                        12.,
                        10.,
                        5.,
                        std::f32::consts::PI,
                        2. * std::f32::consts::PI,
                    );
                    drawing.circle(12., 15., 1.);
                    drawing.line(&[(12., 16.), (12., 18.)]);
                }
                "unlock" => {
                    drawing.rect(5., 10., 14., 11.);
                    drawing.arc(
                        12.,
                        10.,
                        5.,
                        std::f32::consts::PI,
                        1.9 * std::f32::consts::PI,
                    );
                }
                "shield" | "checkmark.shield" | "checkmark.seal" => {
                    drawing.line(&[
                        (12., 2.),
                        (21., 5.),
                        (20., 14.),
                        (17., 19.),
                        (12., 23.),
                        (7., 19.),
                        (4., 14.),
                        (3., 5.),
                        (12., 2.),
                    ]);
                    drawing.line(&[(7., 12.), (11., 16.), (17., 9.)]);
                }
                "settings" | "gearshape" => {
                    let pts: Vec<_> = (0..=32)
                        .map(|i| {
                            let angle = i as f32 * std::f32::consts::TAU / 32.;
                            let r = if i % 4 == 0 || i % 4 == 3 { 10. } else { 8. };
                            (12. + r * angle.cos(), 12. + r * angle.sin())
                        })
                        .collect();
                    drawing.line(&pts);
                    drawing.circle(12., 12., 4.);
                }
                "help" => {
                    drawing.circle(12., 12., 10.);
                    drawing.arc(
                        12.,
                        9.,
                        3.,
                        std::f32::consts::PI,
                        2.5 * std::f32::consts::PI,
                    );
                    drawing.line(&[(12., 12.), (12., 15.)]);
                    drawing.circle(12., 19., 0.5);
                }
                "updates" => {
                    drawing.arc(12., 12., 9., 0.4, 3.7);
                    drawing.arc(12., 12., 9., 3.6, 6.8);
                    drawing.line(&[(3., 3.), (3., 8.), (8., 8.)]);
                    drawing.line(&[(16., 16.), (21., 16.), (21., 21.)]);
                }
                "chevronRight" => drawing.line(&[(8., 4.), (16., 12.), (8., 20.)]),
                "chevronDown" => drawing.line(&[(4., 8.), (12., 16.), (20., 8.)]),
                "closeCircle" => {
                    drawing.circle(12., 12., 10.);
                    drawing.line(&[(8., 8.), (16., 16.)]);
                    drawing.line(&[(16., 8.), (8., 16.)]);
                }
                "check" => drawing.line(&[(4., 12.), (10., 18.), (21., 5.)]),
                "cursor" => {
                    drawing.line(&[
                        (6., 3.),
                        (6., 19.),
                        (10., 15.),
                        (14., 22.),
                        (17., 20.),
                        (13., 13.),
                        (19., 12.),
                        (6., 3.),
                    ]);
                    drawing.line(&[(2., 1.), (2., 5.)]);
                    drawing.line(&[(9., 0.), (11., 3.)]);
                    drawing.line(&[(0., 9.), (3., 9.)]);
                }
                "arrow.down.doc" => {
                    drawing.document();
                    drawing.line(&[(12., 8.), (12., 18.)]);
                    drawing.line(&[(8., 14.), (12., 18.), (16., 14.)]);
                }
                "film.stack" => {
                    drawing.rect(2., 5., 18., 16.);
                    drawing.line(&[(6., 2.), (23., 2.), (23., 17.)]);
                    drawing.line(&[(6., 5.), (6., 21.)]);
                    drawing.line(&[(16., 5.), (16., 21.)]);
                    for y in [9., 13., 17.] {
                        drawing.line(&[(2., y), (6., y)]);
                        drawing.line(&[(16., y), (20., y)]);
                    }
                }
                "key.horizontal" | "key.viewfinder" => {
                    drawing.circle(6., 10., 4.);
                    drawing.line(&[
                        (10., 10.),
                        (22., 10.),
                        (22., 15.),
                        (19., 15.),
                        (19., 12.),
                        (16., 12.),
                        (16., 15.),
                    ]);
                }
                "asterisk" => {
                    for a in [
                        0.,
                        std::f32::consts::PI / 3.,
                        2. * std::f32::consts::PI / 3.,
                    ] {
                        drawing.line(&[
                            (12. + 9. * a.cos(), 12. + 9. * a.sin()),
                            (12. - 9. * a.cos(), 12. - 9. * a.sin()),
                        ]);
                    }
                }
                "link" => {
                    drawing.line(&[
                        (9., 16.),
                        (7., 18.),
                        (4., 18.),
                        (2., 15.),
                        (2., 12.),
                        (8., 6.),
                        (11., 6.),
                        (14., 9.),
                    ]);
                    drawing.line(&[
                        (10., 15.),
                        (13., 18.),
                        (16., 18.),
                        (22., 12.),
                        (22., 9.),
                        (20., 6.),
                        (17., 6.),
                        (15., 8.),
                    ]);
                    drawing.line(&[(7., 13.), (17., 11.)]);
                }
                "clock" => {
                    drawing.circle(12., 12., 10.);
                    drawing.line(&[(12., 5.), (12., 12.), (17., 15.)]);
                }
                "calendar.badge.clock" | "calendar.badge.plus" => {
                    drawing.rect(3., 5., 18., 16.);
                    drawing.line(&[(3., 10.), (21., 10.)]);
                    drawing.line(&[(7., 2.), (7., 7.)]);
                    drawing.line(&[(17., 2.), (17., 7.)]);
                    drawing.line(&[(8., 15.), (16., 15.)]);
                    drawing.line(&[(12., 11.), (12., 19.)]);
                }
                "tablecells" | "tablecells.badge.ellipsis" | "externaldrive.badge.plus" => {
                    drawing.rect(2., 4., 20., 16.);
                    drawing.line(&[(2., 10.), (22., 10.)]);
                    drawing.line(&[(2., 15.), (22., 15.)]);
                    drawing.line(&[(9., 4.), (9., 20.)]);
                    drawing.line(&[(16., 4.), (16., 20.)]);
                }
                "network"
                | "network.badge.shield.half.filled"
                | "point.3.connected.trianglepath.dotted" => {
                    drawing.circle(12., 4., 3.);
                    drawing.circle(4., 20., 3.);
                    drawing.circle(20., 20., 3.);
                    drawing.line(&[(10.5, 7.), (5.5, 17.)]);
                    drawing.line(&[(13.5, 7.), (18.5, 17.)]);
                    drawing.line(&[(7., 20.), (17., 20.)]);
                }
                "number" | "number.square" | "number.square.fill" => {
                    if symbol != "number" {
                        drawing.rect(2., 2., 20., 20.);
                    }
                    drawing.line(&[(10., 5.), (7., 19.)]);
                    drawing.line(&[(17., 5.), (14., 19.)]);
                    drawing.line(&[(5., 10.), (20., 10.)]);
                    drawing.line(&[(4., 15.), (19., 15.)]);
                }
                "plus.forwardslash.minus" => {
                    drawing.line(&[(3., 7.), (11., 7.)]);
                    drawing.line(&[(7., 3.), (7., 11.)]);
                    drawing.line(&[(16., 3.), (8., 21.)]);
                    drawing.line(&[(15., 18.), (23., 18.)]);
                }
                "ruler" => {
                    drawing.rect(2., 6., 20., 12.);
                    for x in [6., 10., 14., 18.] {
                        drawing.line(&[(x, 6.), (x, if x == 10. || x == 18. { 13. } else { 10. })]);
                    }
                }
                "paintpalette" => {
                    drawing.circle(12., 12., 10.);
                    for (x, y) in [(8., 6.), (15., 6.), (6., 12.), (17., 11.)] {
                        drawing.circle(x, y, 1.5);
                    }
                    drawing.circle(12., 17., 2.);
                }
                "circle.lefthalf.filled" => {
                    drawing.circle(12., 12., 10.);
                    drawing.line(&[(12., 2.), (12., 22.)]);
                    for x in [4., 6., 8., 10.] {
                        let h = (100.0_f32 - (12. - x) * (12. - x)).sqrt();
                        drawing.line(&[(x, 12. - h), (x, 12. + h)]);
                    }
                }
                "rectangle.leadinghalf.inset.filled" => {
                    drawing.rect(2., 4., 20., 16.);
                    for x in [5., 8., 11., 14., 17.] {
                        drawing.line(&[(x, 7.), (x, 17.)]);
                    }
                }
                "doc.richtext" | "doc.badge.gearshape" | "list.bullet.rectangle" => {
                    drawing.document();
                    drawing.line(&[(7., 10.), (12., 10.)]);
                    drawing.line(&[(7., 14.), (17., 14.)]);
                    drawing.line(&[(7., 18.), (17., 18.)]);
                }
                "scope" => {
                    drawing.circle(12., 12., 6.);
                    drawing.line(&[(12., 1.), (12., 8.)]);
                    drawing.line(&[(12., 16.), (12., 23.)]);
                    drawing.line(&[(1., 12.), (8., 12.)]);
                    drawing.line(&[(16., 12.), (23., 12.)]);
                }
                "square.stack.3d.down.right" | "square.on.square" => {
                    drawing.rect(3., 2., 14., 14.);
                    drawing.line(&[(7., 19.), (20., 19.), (20., 6.)]);
                    drawing.line(&[(11., 22.), (23., 22.), (23., 10.)]);
                }
                "chevron.left.forwardslash.chevron.right" => {
                    drawing.line(&[(7., 6.), (1., 12.), (7., 18.)]);
                    drawing.line(&[(17., 6.), (23., 12.), (17., 18.)]);
                    drawing.line(&[(15., 3.), (9., 21.)]);
                }
                "character.cursor.ibeam" => {
                    drawing.line(&[(1., 20.), (7., 3.), (13., 20.)]);
                    drawing.line(&[(4., 13.), (10., 13.)]);
                    drawing.line(&[(16., 3.), (22., 3.)]);
                    drawing.line(&[(19., 3.), (19., 21.)]);
                    drawing.line(&[(16., 21.), (22., 21.)]);
                }
                "text.quote" => {
                    drawing.rect(3., 5., 6., 6.);
                    drawing.line(&[(9., 11.), (9., 15.), (5., 19.)]);
                    drawing.rect(15., 5., 6., 6.);
                    drawing.line(&[(21., 11.), (21., 15.), (17., 19.)]);
                }
                "line.3.horizontal.decrease.circle" => {
                    drawing.circle(12., 12., 10.);
                    for (y, x) in [(7., 6.), (12., 8.), (17., 10.)] {
                        drawing.line(&[(x, y), (24. - x, y)]);
                    }
                }
                "circle.grid.2x1" => {
                    drawing.circle(8., 12., 6.);
                    drawing.circle(16., 12., 6.);
                }
                "tag" => drawing.line(&[
                    (2., 3.),
                    (12., 3.),
                    (23., 14.),
                    (14., 23.),
                    (3., 12.),
                    (2., 3.),
                ]),
                "signature" => {
                    drawing.line(&[
                        (3., 18.),
                        (12., 3.),
                        (15., 3.),
                        (13., 9.),
                        (7., 15.),
                        (4., 14.),
                        (12., 14.),
                        (14., 18.),
                        (18., 13.),
                        (18., 18.),
                        (22., 16.),
                    ]);
                    drawing.line(&[(3., 22.), (22., 22.)]);
                }
                "arrow.triangle.merge" => {
                    drawing.line(&[(4., 3.), (4., 8.), (12., 14.), (12., 22.)]);
                    drawing.line(&[(20., 3.), (20., 8.), (12., 14.)]);
                    drawing.line(&[(8., 18.), (12., 22.), (16., 18.)]);
                }
                "eye.slash" => {
                    drawing.line(&[
                        (2., 12.),
                        (7., 6.),
                        (12., 4.),
                        (17., 6.),
                        (22., 12.),
                        (17., 18.),
                        (12., 20.),
                        (7., 18.),
                        (2., 12.),
                    ]);
                    drawing.circle(12., 12., 4.);
                    drawing.line(&[(2., 2.), (22., 22.)]);
                }
                "terminal" => {
                    drawing.rect(2., 3., 20., 18.);
                    drawing.line(&[(6., 8.), (10., 12.), (6., 16.)]);
                    drawing.line(&[(13., 16.), (18., 16.)]);
                }
                "list.bullet.indent" => {
                    for y in [5., 12., 19.] {
                        drawing.circle(3., y, 1.);
                        drawing.line(&[(8., y), (22., y)]);
                    }
                }
                "circle.hexagongrid" => {
                    for (x, y) in [
                        (8., 5.),
                        (17., 5.),
                        (4., 13.),
                        (13., 13.),
                        (21., 13.),
                        (8., 21.),
                        (17., 21.),
                    ] {
                        drawing.circle(x, y, 2.5);
                    }
                }
                "switch.2" => {
                    drawing.rect(2., 3., 20., 7.);
                    drawing.circle(6., 6.5, 2.);
                    drawing.rect(2., 14., 20., 7.);
                    drawing.circle(18., 17.5, 2.);
                }
                "chart.bar.xaxis" => {
                    drawing.line(&[(2., 22.), (23., 22.)]);
                    drawing.rect(4., 12., 4., 7.);
                    drawing.rect(10., 4., 4., 15.);
                    drawing.rect(16., 8., 4., 11.);
                }
                "aspectratio" => {
                    drawing.rect(2., 5., 20., 14.);
                    drawing.line(&[(5., 12.), (5., 8.), (10., 8.)]);
                    drawing.line(&[(19., 12.), (19., 16.), (14., 16.)]);
                }
                "point.topleft.down.to.point.bottomright.curvepath" => {
                    drawing.circle(3., 21., 2.);
                    drawing.circle(21., 3., 2.);
                    let points: Vec<_> = (0..=30)
                        .map(|i| {
                            let t = i as f32 / 30.;
                            (3. + 18. * t, 21. - 18. * (3. * t * t - 2. * t * t * t))
                        })
                        .collect();
                    drawing.line(&points);
                }
                "house" => {
                    drawing.line(&[
                        (1., 10.),
                        (12., 1.),
                        (23., 10.),
                        (20., 10.),
                        (20., 22.),
                        (4., 22.),
                        (4., 10.),
                        (1., 10.),
                    ]);
                    drawing.rect(9., 14., 6., 8.);
                }
                _ => {
                    drawing.document();
                    drawing.line(&[(7., 11.), (17., 11.)]);
                    drawing.line(&[(7., 16.), (15., 16.)]);
                }
            }
            for points in drawing.lines {
                let mut path = PathBuilder::stroke(px((size / 24. * 1.7).max(0.8)));
                for (i, (x, y)) in points.into_iter().enumerate() {
                    let at = point(
                        bounds.origin.x + px(x / 24. * size),
                        bounds.origin.y + px(y / 24. * size),
                    );
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
    .size(px(size))
    .flex_none()
    .into_any_element()
}
struct SymbolDrawing {
    lines: Vec<Vec<(f32, f32)>>,
}
impl SymbolDrawing {
    fn new() -> Self {
        Self { lines: vec![] }
    }
    fn line(&mut self, points: &[(f32, f32)]) {
        self.lines.push(points.to_vec());
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.line(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)]);
    }
    fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32) {
        self.lines.push(
            (0..=32)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 32.;
                    (x + rx * a.cos(), y + ry * a.sin())
                })
                .collect(),
        );
    }
    fn circle(&mut self, x: f32, y: f32, r: f32) {
        self.ellipse(x, y, r, r);
    }
    fn arc(&mut self, x: f32, y: f32, r: f32, start: f32, end: f32) {
        self.lines.push(
            (0..=24)
                .map(|i| {
                    let a = start + (end - start) * i as f32 / 24.;
                    (x + r * a.cos(), y + r * a.sin())
                })
                .collect(),
        );
    }
    fn star(&mut self, x: f32, y: f32, r: f32) {
        self.line(&[
            (x, y - r),
            (x + r * 0.25, y - r * 0.25),
            (x + r, y),
            (x + r * 0.25, y + r * 0.25),
            (x, y + r),
            (x - r * 0.25, y + r * 0.25),
            (x - r, y),
            (x - r * 0.25, y - r * 0.25),
            (x, y - r),
        ]);
    }
    fn document(&mut self) {
        self.line(&[
            (4., 2.),
            (14., 2.),
            (20., 8.),
            (20., 22.),
            (4., 22.),
            (4., 2.),
        ]);
        self.line(&[(14., 2.), (14., 8.), (20., 8.)]);
    }
}

fn source_version() -> &'static str {
    include_str!("../../../../project.yml")
        .lines()
        .find_map(|line| line.trim().strip_prefix("MARKETING_VERSION:"))
        .map(str::trim)
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

fn source_shadow(alpha: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: opacity(gpui::black(), alpha),
        offset: point(px(0.), px(1.)),
        blur_radius: px(3.),
        spread_radius: px(0.),
    }]
}

fn ai_is_configured() -> bool {
    crate::transport::provider_is_configured()
}

#[cfg(test)]
mod tests {
    use super::{Category, Group, TOOLS, filtered_tools, grid_columns, tool_title};
    #[test]
    fn card_labels_keep_source_typography_and_reservations() {
        use gpui::{Styled, px};
        for (font_size, line_height) in [(13., 16.), (11., 15.)] {
            let mut label = super::card_label(
                "label",
                font_size,
                line_height,
                gpui::FontWeight::NORMAL,
                gpui::black(),
                Default::default(),
            );
            let mut expected = gpui::div()
                .h(px(2. * line_height))
                .text_size(px(font_size))
                .line_height(px(line_height));
            assert_eq!(label.style().size.height, expected.style().size.height);
            assert_eq!(
                label.text_style().as_ref().unwrap().font_size,
                expected.text_style().as_ref().unwrap().font_size
            );
            assert_eq!(
                label.text_style().as_ref().unwrap().line_height,
                expected.text_style().as_ref().unwrap().line_height
            );
        }
    }
    #[test]
    fn card_label_tail_uses_measured_width_not_character_count() {
        let width = |text: &str| {
            text.chars()
                .map(|ch| match ch {
                    'W' => 9,
                    'i' => 2,
                    '…' => 6,
                    _ => 4,
                })
                .sum::<usize>()
        };
        for text in [
            "first WWWWWWWWWWWWW",
            "first iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii",
        ] {
            let lines = super::card_line_strings(text, 6, |candidate| width(candidate) <= 30);
            assert_eq!(lines[0], "first");
            assert!(lines[1].ends_with('…'));
            assert!(width(&lines[1]) <= 30);
        }
    }
    #[test]
    fn card_labels_truncate_only_the_last_line() {
        let fit = |value: &str| value.chars().count() <= 12;
        assert_eq!(
            super::card_line_strings(
                "Compare live time or plan a meeting across locations",
                13,
                fit
            ),
            ["Compare live", "time or pla…"]
        );
        assert_eq!(
            super::card_line_strings("short label", 11, fit),
            ["short label"]
        );
        assert_eq!(
            super::card_line_strings("first line second", 11, fit),
            ["first line", "second"]
        );
        assert_eq!(
            super::card_line_strings("abcdefghijklmnopqrstuv", 12, fit),
            ["abcdefghijkl", "mnopqrstuv"]
        );
    }
    #[test]
    fn card_labels_preserve_unicode_graphemes_and_ellipsis_width() {
        use unicode_segmentation::UnicodeSegmentation;
        let fit = |value: &str| value.graphemes(true).count() <= 3;
        assert_eq!(
            super::card_line_strings("abc 👩‍💻é中文", 4, fit),
            ["abc", "👩‍💻é…"]
        );
        assert_eq!(
            super::card_line_strings("abc def", 4, |_| false),
            ["abc", ""]
        );
        assert_eq!(
            super::card_line_strings("abc defghijklmnop", 4, fit),
            ["abc", "de…"]
        );
    }
    #[test]
    fn home_categories_match_source() {
        assert_eq!(Category::Overview.commands().len(), 9);
        assert_eq!(Category::Developer.commands().len(), 51);
        assert_eq!(Category::Capture.commands().len(), 4);
        assert_eq!(Category::Text.commands().len(), 6);
        assert_eq!(TOOLS.len(), 61);
    }
    #[test]
    fn adaptive_grid_matches_source_minimum() {
        assert_eq!(grid_columns(900.), 2);
        assert_eq!(grid_columns(1000.), 3);
        assert_eq!(grid_columns(1017.), 3);
    }
    #[test]
    fn developer_filters_preserve_source_labels_and_groups() {
        assert_eq!(filtered_tools("", None, true).len(), 20);
        assert_eq!(filtered_tools("", Some(Group::Data), false).len(), 15);
        assert_eq!(filtered_tools("HMAC", None, false)[0].title, "HMAC Signer");
        assert_eq!(tool_title("jsonFlatten"), "Flatten & Unflatten JSON");
        assert!(filtered_tools("nothing matches this phrase", None, false).is_empty());
    }
}
