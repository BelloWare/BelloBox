//! The seven source Settings pages, using SettingsView.swift's layout and labels.
//! Mounting and editing never sends a request or asks for system permission.
mod provider_setup;
use provider_setup::{Action as SetupAction, Setup};

use crate::theme::{self, Palette, opacity};
use bello_platform::{Permission, PermissionState, PermissionStatus, Platform};
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::{
    ai::DEFAULT_SYSTEM_PROMPT,
    settings::{Appearance, Settings, config_dir},
};
use gpui::{prelude::*, *};
use std::{collections::BTreeMap, path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    General,
    Ai,
    Capture,
    Recording,
    Ocr,
    Permissions,
    Prompt,
}
impl Category {
    const ALL: [Self; 7] = [
        Self::General,
        Self::Ai,
        Self::Capture,
        Self::Recording,
        Self::Ocr,
        Self::Permissions,
        Self::Prompt,
    ];
    fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Ai => "ai",
            Self::Capture => "capture",
            Self::Recording => "recording",
            Self::Ocr => "ocr",
            Self::Permissions => "permissions",
            Self::Prompt => "prompt",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Ai => "AI Provider",
            Self::Capture => "Screenshots",
            Self::Recording => "Recording",
            Self::Ocr => "OCR",
            Self::Permissions => "Permissions",
            Self::Prompt => "Prompt",
        }
    }
    fn explanation(self) -> &'static str {
        match self {
            Self::General => "Set how Bello Box starts, appears, and follows your system theme.",
            Self::Ai => "Connect your AI and tune how each model responds across Bello Box.",
            Self::Capture => {
                "Configure screenshot shortcuts and the capture behavior users see before editing."
            }
            Self::Recording => {
                "Set default recording options before choosing an area, window, or screen."
            }
            Self::Ocr => {
                "Tune OCR defaults. OCR runs only from the screenshot editor after you request it."
            }
            Self::Permissions => {
                "Review the macOS permissions needed for selection tools, screenshots, and recordings."
            }
            Self::Prompt => "Customize the system prompt used for text AI actions.",
        }
    }
    fn symbol(self) -> &'static str {
        match self {
            Self::General => "gearshape",
            Self::Ai => "sparkles",
            Self::Capture => "camera.viewfinder",
            Self::Recording => "record.circle",
            Self::Ocr => "text.viewfinder",
            Self::Permissions => "lock.shield",
            Self::Prompt => "text.alignleft",
        }
    }
    fn fixture(value: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|c| c.id() == value)
            .unwrap_or(Self::General)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Provider {
    OpenAi,
    Anthropic,
    Codex,
}
impl Provider {
    const ALL: [Self; 3] = [Self::OpenAi, Self::Anthropic, Self::Codex];
    fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Codex => "codex",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI-compatible",
            Self::Anthropic => "Anthropic-compatible",
            Self::Codex => "Codex app-server",
        }
    }
    fn endpoint(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Codex => "",
        }
    }
    fn from_kind(kind: &str) -> Self {
        match kind {
            "anthropic" => Self::Anthropic,
            "codex" | "codexCLI" => Self::Codex,
            _ => Self::OpenAi,
        }
    }
}

struct SettingsView {
    settings_path: std::path::PathBuf,
    category: Category,
    appearance: Appearance,
    provider: Provider,
    responses: bool,
    endpoint: Entity<EditorView>,
    model: Entity<EditorView>,
    prompt: Entity<EditorView>,
    provider_drafts: BTreeMap<&'static str, (String, String)>,
    switching_provider: bool,
    model_menu: bool,
    setup: Setup,
    focus: FocusHandle,
    scroll: ScrollHandle,
    icon: Arc<Image>,
    permissions: Vec<PermissionStatus>,
    permission_busy: bool,
    learned_reset: bool,
    status: Option<String>,
    editor_ink: Hsla,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_at(config_dir().join("settings.json"), window, cx)
    }
    fn new_at(
        settings_path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (settings, status) = match Settings::load(&settings_path) {
            Ok(settings) => (settings, None),
            Err(error) => (Settings::default(), Some(error)),
        };
        theme::set_saved_appearance(settings.appearance.clone());
        let p = theme::for_window(window);
        let endpoint = make_editor(settings.provider_endpoint, true, p, window, cx);
        let model = make_editor(settings.provider_model, true, p, window, cx);
        let prompt = make_editor(settings.system_prompt, false, p, window, cx);
        let endpoint_sub = cx.subscribe(&endpoint, |this, _, event, cx| {
            if matches!(event, EditorEvent::Changed) && !this.switching_provider {
                this.setup.invalidate(true);
                this.save_provider(cx);
            }
        });
        let model_sub = cx.subscribe(&model, |this, _, event, cx| {
            if matches!(event, EditorEvent::Changed) && !this.switching_provider {
                this.setup.invalidate(false);
                this.save_provider(cx);
            }
        });
        let prompt_sub = cx.subscribe(&prompt, |this, _, event, cx| {
            if matches!(event, EditorEvent::Changed) {
                this.setup.invalidate(false);
                let prompt = this.prompt.read(cx).text().to_owned();
                this.persist(
                    move |settings| {
                        settings.system_prompt = prompt;
                        Ok(())
                    },
                    cx,
                );
            }
        });
        let owner_window = window.window_handle();
        let weak = cx.weak_entity();
        let closed_sub = cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner_window) {
                let _ = weak.update(cx, |this, _| this.setup.close());
            }
        });
        let appearance_sub = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
        let focus = cx.focus_handle();
        focus.focus(window);
        cx.spawn(async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.category == Category::Permissions {
                            let permissions = Platform::new().status().permissions;
                            if permissions != this.permissions {
                                this.permissions = permissions;
                                cx.notify();
                            }
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
            settings_path,
            category: Category::fixture(
                &std::env::var("BELLOBOX_SETTINGS_CATEGORY").unwrap_or_default(),
            ),
            appearance: settings.appearance,
            provider: Provider::from_kind(&settings.provider_kind),
            responses: settings.provider_kind == "responses",
            endpoint,
            model,
            prompt,
            provider_drafts: BTreeMap::new(),
            switching_provider: false,
            model_menu: false,
            setup: Setup::default(),
            focus,
            scroll: ScrollHandle::new(),
            icon: Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!(
                    "../../../../BelloBox/Assets.xcassets/AppIcon.appiconset/icon_128x128@2x.png"
                )
                .to_vec(),
            )),
            permissions: Platform::new().status().permissions,
            permission_busy: false,
            learned_reset: false,
            status,
            editor_ink: p.primary,
            _subscriptions: vec![
                endpoint_sub,
                model_sub,
                prompt_sub,
                appearance_sub,
                closed_sub,
            ],
        }
    }
    fn persist(
        &mut self,
        change: impl FnOnce(&mut Settings) -> Result<(), String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match edit_saved(&self.settings_path, change) {
            Ok(()) => {
                self.status = None;
                crate::transport::settings_changed();
                cx.refresh_windows();
                cx.notify();
                true
            }
            Err(error) => {
                self.status = Some(format!("Not saved: {error}"));
                cx.notify();
                false
            }
        }
    }
    fn save_provider(&mut self, cx: &mut Context<Self>) {
        let endpoint = self.endpoint.read(cx).text().to_owned();
        let model = self.model.read(cx).text().to_owned();
        let kind = if self.provider == Provider::OpenAi && self.responses {
            "responses"
        } else {
            self.provider.id()
        }
        .to_string();
        let provider = self.provider;
        self.persist(
            move |settings| {
                validate_provider(provider, &endpoint, &model)?;
                settings.provider_kind = kind;
                settings.provider_endpoint = endpoint;
                settings.provider_model = model;
                Ok(())
            },
            cx,
        );
    }
    fn switch_provider(&mut self, provider: Provider, cx: &mut Context<Self>) {
        if provider == self.provider {
            return;
        }
        self.provider_drafts.insert(
            self.provider.id(),
            (
                self.endpoint.read(cx).text().into(),
                self.model.read(cx).text().into(),
            ),
        );
        let (endpoint, model) = self
            .provider_drafts
            .get(provider.id())
            .cloned()
            .unwrap_or_else(|| (provider.endpoint().into(), String::new()));
        self.setup.invalidate(true);
        self.provider = provider;
        self.model_menu = false;
        self.switching_provider = true;
        self.endpoint
            .update(cx, |editor, cx| editor.set_text(endpoint, cx));
        self.model
            .update(cx, |editor, cx| editor.set_text(model, cx));
        self.switching_provider = false;
        self.save_provider(cx);
    }
    fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        let saved = appearance.clone();
        if self.persist(
            move |settings| {
                settings.appearance = saved;
                Ok(())
            },
            cx,
        ) {
            self.appearance = appearance.clone();
            theme::set_saved_appearance(appearance);
            cx.refresh_windows();
        }
    }
    fn navigate(&mut self, category: Category, window: &mut Window, cx: &mut Context<Self>) {
        self.category = category;
        self.model_menu = false;
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.focus.focus(window);
        if category == Category::Permissions {
            self.permissions = Platform::new().status().permissions;
        }
        cx.notify();
    }
    fn open_permission(&mut self, permission: Permission, cx: &mut Context<Self>) {
        if self.permission_busy {
            return;
        }
        self.permission_busy = true;
        let work = cx
            .background_executor()
            .spawn(async move { Platform::new().open_permission_settings(permission) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.permission_busy = false;
                this.status = result.err().map(|error| error.to_string());
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn sidebar(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        div()
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
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Bello Box"),
                            )
                            .child(help("Settings", p)),
                    ),
            )
            .children(Category::ALL.into_iter().map(|category| {
                let selected = self.category == category;
                div().px(px(10.)).child(
                    div()
                        .id(SharedString::from(format!(
                            "settingsCategory_{}",
                            category.id()
                        )))
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(11.))
                        .h(px(40.))
                        .px(px(12.))
                        .rounded(px(8.))
                        .text_size(px(13.))
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if selected { p.accent } else { p.primary })
                        .cursor_pointer()
                        .when(selected, |s| {
                            s.bg(opacity(p.accent, 0.09)).child(
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
                        .child(div().w(px(20.)).flex_none().flex().justify_center().child(
                            theme::tool_icon(
                                category.symbol(),
                                15.,
                                Palette {
                                    accent: if selected { p.accent } else { p.primary },
                                    ..p
                                },
                            ),
                        ))
                        .child(category.title())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.navigate(category, window, cx)
                        })),
                )
            }))
    }
    fn general(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        column(14.)
            .child(section("Startup", "Choose whether Bello Box is ready as soon as you sign in.", "power", p,
                column(12.).child(disabled_toggle("Open Bello Box when I start my Mac", false, p)).child(help("Launch at login is not yet connected in this Rust build.", p))))
            .child(section("Selection & Shortcuts", "Control how the small selection toolbar appears.", "cursorarrow.rays", p,
                column(12.)
                    .child(disabled_toggle("Show auto hint after I select text", true, p))
                    .child(disabled_toggle("Enable global shortcut ⌃⌥⌘B", true, p))
                    .child(disabled_value("Shortcut", "⌃⌥⌘B", p))
                    .child(help("Auto hint appears after mouse selections. The shortcut opens searchable tools, with suggestions for your selection. It also works without a selection.", p))
                    .child(help("Selection hints and global shortcut recording are not yet connected.", p))))
            .child(section("Appearance", "Match the system or keep Bello Box fixed in one theme.", "circle.lefthalf.filled", p,
                column(12.)
                    .child(div().flex().gap(px(10.)).children([Appearance::System, Appearance::Light, Appearance::Dark].into_iter().enumerate().map(|(index, appearance)| {
                        let selected = appearance == self.appearance;
                        let (title, detail) = match appearance { Appearance::System => ("System", "Follow macOS"), Appearance::Light => ("Light", "Always light"), Appearance::Dark => ("Dark", "Always dark") };
                        let previews = div().h(px(62.)).flex_none().flex().rounded(px(6.)).overflow_hidden()
                            .when(appearance != Appearance::Dark, |s| s.child(appearance_preview(false)))
                            .when(appearance != Appearance::Light, |s| s.child(appearance_preview(true)));
                        div().id(("appearance", index)).flex_1().min_w_0().flex().flex_col().gap(px(10.)).p(px(10.)).rounded(px(10.))
                            .border_1().border_color(if selected { p.accent } else { p.border }).bg(if selected { opacity(p.accent, 0.07) } else { p.well }).cursor_pointer()
                            .child(previews)
                            .child(div().flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(title).child(div().flex_1()).when(selected, |s| s.child(div().text_color(p.accent).child("✓"))))
                            .child(div().text_size(px(10.)).text_color(p.secondary).child(detail))
                            .on_click(cx.listener(move |this, _, _, cx| this.set_appearance(appearance.clone(), cx)))
                    })))
                    .child(column(9.).mt(px(6.))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Window background"))
                        .child(div().flex().gap(px(10.))
                            .child(surface_choice("Glass", "See your desktop through windows and cards.", false, p))
                            .child(surface_choice("Solid", "Opaque backgrounds with the same colors.", true, p)))
                        .child(help("Glass is not yet available in this Rust build. Solid backgrounds are used.", p)))
                    .child(help("Theme changes apply immediately to every open window.", p))))
            .child(section("Tool Suggestions", "Your frequent choices rise to the top for similar text.", "sparkles", p,
                column(12.)
                    .child(help("Bello Box learns from the tools you open for JSON, dates, links, and other text. Only tool choices, text categories, and usage times are stored on this Mac.", p))
                    .child(div().flex().flex_wrap().items_center().gap(px(10.))
                        .child(button("reset-learned", "Reset learned tool order", p).on_click(cx.listener(|this, _, _, cx| {
                            if this.persist(|settings| { settings.usage = Default::default(); Ok(()) }, cx) { this.learned_reset = true; }
                        })))
                        .when(self.learned_reset, |s| s.child(help("Reset. Applies next time you open the palette.", p))))))
    }
    fn ai(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let codex = self.provider == Provider::Codex;
        let mut fields = column(12.).child(choice_bar(
            Provider::ALL
                .into_iter()
                .enumerate()
                .map(|(index, provider)| {
                    choice(
                        ("provider", index),
                        provider.title(),
                        self.provider == provider,
                        p,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.switch_provider(provider, cx)))
                }),
            p,
        ));
        fields = fields.child(labeled(
            if codex {
                "Codex command (optional)"
            } else {
                "Endpoint"
            },
            p,
            div()
                .flex()
                .gap(px(8.))
                .items_center()
                .child(div().flex_1().min_w_0().child(single_line_field(
                    self.endpoint.clone(),
                    if codex {
                        "codex (from your shell PATH)"
                    } else {
                        "Base URL"
                    },
                    p,
                    cx,
                )))
                .when(codex, |s| s.child(disabled_button("Detect", p)))
                .when(!codex, |s| {
                    s.child(
                        button("default-endpoint", "Default", p).on_click(cx.listener(
                            |this, _, _, cx| {
                                let endpoint = this.provider.endpoint().to_owned();
                                this.endpoint.update(cx, |e, cx| e.set_text(endpoint, cx));
                            },
                        )),
                    )
                }),
        ));
        if !codex {
            fields = fields.child(labeled("API key", p, disabled_field("Secure key storage is not yet connected", p)))
                .child(help("API keys cannot be entered or saved here yet. An explicitly configured process key stays in memory and is never shown.", p));
        } else {
            fields = fields.child(help("Codex app-server is not yet connected. These fields save configuration only; Detect and requests are unavailable.", p));
        }
        if self.provider == Provider::OpenAi {
            fields = fields.child(labeled(
                "Request API",
                p,
                choice_bar(
                    [false, true]
                        .into_iter()
                        .enumerate()
                        .map(|(index, responses)| {
                            choice(
                                ("request-api", index),
                                if responses {
                                    "Responses API"
                                } else {
                                    "Chat Completions"
                                },
                                self.responses == responses,
                                p,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_request_api(responses, cx);
                                },
                            ))
                        }),
                    p,
                ),
            ));
        }
        fields = fields
            .child(labeled(
                "Model",
                p,
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().flex_1().min_w_0().child(single_line_field(
                        self.model.clone(),
                        if codex { "gpt-5.5" } else { "Model name" },
                        p,
                        cx,
                    )))
                    .child(self.model_menu(p, cx))
                    .when(!codex, |s| {
                        s.child(self.setup_button(SetupAction::Load, p, cx))
                    }),
            ))
            .when_some(self.setup.load_message.clone(), |s, message| {
                s.child(help(message, p))
            })
            .child(self.model_behavior(p, cx));
        if codex {
            fields = fields.child(div().flex().gap(px(10.))
                .child(div().flex_1().child(labeled("Sandbox", p, disabled_field("Read only  ⌄", p))))
                .child(div().flex_1().child(labeled("Approvals", p, disabled_field("Never  ⌄", p)))))
                .child(help("Sandbox and approval controls are unavailable until the Codex transport is connected.", p));
        }
        fields = fields.child(help(if codex { "Used by Ask AI and World Clock copilot." } else { "Used by Ask AI, World Clock copilot, and AI screenshot text recognition." }, p))
            .child(div().flex().items_center().gap(px(10.)).child(if codex { disabled_button("Test connection", p).into_any_element() } else { self.setup_button(SetupAction::Test, p, cx) }).child(help("Sends a short hello with these settings.", p)))
            .when_some(self.setup.test_message.clone(), |s, message| s.child(help(message, p)))
            .when(self.setup.busy(), |s| s.child(button("cancel-provider-setup", "Cancel request", p).on_click(cx.listener(|this, _, _, cx| { this.setup.invalidate(false); cx.notify(); }))))
            .when(self.setup.busy() && self.setup.action.is_none(), |s| s.child(help("Stopping the previous request. Retry becomes available after the transport stops or times out.", p)))
            .child(help("Only Load and Test connection send requests. Editing settings never sends a request. Test checks the fields above; explicit process provider, endpoint or model overrides still take precedence in Ask AI.", p))
            .child(help(match self.provider {
                Provider::OpenAi if self.responses => "POST {endpoint}/responses with a Bearer token and Responses API streaming. Use this for OpenAI or compatible endpoints that implement the Responses API.",
                Provider::OpenAi => "POST {endpoint}/chat/completions with a Bearer token. Works with OpenAI, OpenRouter, Groq, Ollama, LM Studio, and other compatible servers.",
                Provider::Anthropic => "POST {endpoint}/messages with an x-api-key header.",
                Provider::Codex => "The original app runs codex app-server using your existing Codex login. That transport has not yet been ported.",
            }, p));
        section(
            "Provider",
            "Bring your own endpoint, API key, model, or local Codex app-server.",
            "antenna.radiowaves.left.and.right",
            p,
            fields,
        )
    }
    fn set_request_api(&mut self, responses: bool, cx: &mut Context<Self>) {
        self.setup.invalidate(true);
        self.responses = responses;
        self.save_provider(cx);
    }
    fn select_model(&mut self, model: String, cx: &mut Context<Self>) {
        self.model_menu = false;
        self.model
            .update(cx, |editor, cx| editor.set_text(model, cx));
        cx.notify();
    }
    fn setup_button(&self, action: SetupAction, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let label = match (action, self.setup.action) {
            (SetupAction::Load, Some(SetupAction::Load)) => "Loading…",
            (SetupAction::Test, Some(SetupAction::Test)) => "Testing model…",
            (SetupAction::Load, _) => "Load",
            (SetupAction::Test, _) => "Test connection",
        };
        if self.setup.busy() {
            return disabled_button(label, p).into_any_element();
        }
        button(
            if action == SetupAction::Load {
                "load-models"
            } else {
                "test-provider"
            },
            label,
            p,
        )
        .on_click(cx.listener(move |this, _, _, cx| this.start_setup(action, cx)))
        .into_any_element()
    }
    fn start_setup(&mut self, action: SetupAction, cx: &mut Context<Self>) {
        // Runtime-only credential boundary; only an explicit action reads it.
        let key = std::env::var("BELLOBOX_AI_KEY").unwrap_or_default();
        self.start_setup_with_key(action, &key, cx);
    }
    fn start_setup_with_key(&mut self, action: SetupAction, key: &str, cx: &mut Context<Self>) {
        if self.provider == Provider::Codex {
            return;
        }
        let endpoint = self.endpoint.read(cx).text().to_owned();
        let model = self.model.read(cx).text().to_owned();
        let result = validate_provider(self.provider, &endpoint, &model).and_then(|_| {
            let config = bellobox_core::ai::Config {
                provider: match self.provider {
                    Provider::Anthropic => bellobox_core::ai::Provider::Anthropic,
                    _ if self.responses => bellobox_core::ai::Provider::OpenAIResponses,
                    _ => bellobox_core::ai::Provider::OpenAIChat,
                },
                endpoint,
                model,
                system_prompt: self.prompt.read(cx).text().to_owned(),
                max_output_tokens: 4096,
            };
            self.setup.start(action, &config, key)
        });
        if let Err(error) = result {
            match action {
                SetupAction::Load => self.setup.load_message = Some(error),
                SetupAction::Test => self.setup.test_message = Some(error),
            }
        } else {
            cx.spawn(async |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(40))
                        .await;
                    let done = this.update(cx, |this, cx| {
                        let done = this.setup.poll();
                        if done {
                            cx.notify();
                        }
                        done || !this.setup.busy()
                    });
                    if !matches!(done, Ok(false)) {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.notify();
    }
    fn model_menu(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut button = button("model-menu", "⌄", p)
            .relative()
            .text_color(p.accent)
            .on_click(cx.listener(|this, _, _, cx| {
                this.model_menu = !this.model_menu;
                cx.notify();
            }));
        if self.model_menu {
            let models: Vec<String> = if self.setup.models.is_empty() {
                model_presets(self.provider)
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect()
            } else {
                self.setup.models.clone()
            };
            let menu = div()
                .id("model-presets")
                .absolute()
                .top(px(32.))
                .right_0()
                .w(px(240.))
                .p(px(4.))
                .max_h(px(280.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .bg(p.surface)
                .border_1()
                .border_color(p.border)
                .rounded(px(8.))
                .shadow_lg()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.model_menu = false;
                    cx.notify();
                }))
                .children(models.into_iter().enumerate().map(|(index, model)| {
                    div()
                        .id(("model-preset", index))
                        .h(px(28.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(5.))
                        .text_color(p.primary)
                        .cursor_pointer()
                        .hover(|s| s.bg(opacity(p.accent, 0.09)))
                        .child(model.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.select_model(model.clone(), cx);
                        }))
                }));
            button = button.child(deferred(menu).with_priority(1));
        }
        button
    }
    fn model_behavior(&self, p: Palette, cx: &App) -> Div {
        let model = self.model.read(cx).text();
        column(12.).p(px(14.)).rounded(px(12.)).border_1().border_color(p.separator).bg(p.well)
            .child(div().flex().items_center().gap(px(8.)).child(theme::tool_icon("slider.horizontal.3", 16., p)).child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Model behavior")))
            .child(column(3.)
                .child(div().text_size(px(11.)).font_family("monospace").text_color(p.accent).child(if model.is_empty() { "Choose a model above".to_owned() } else { model.to_owned() }))
                .child(help("Per-model behavior overrides are not yet connected; requests use the transport defaults.", p)))
            .child(separator(p))
            .when(self.provider != Provider::Codex, |s| s.child(column(7.)
                .child(div().flex().items_center().gap(px(12.)).child("Temperature").child(div().flex_1()).child(disabled_button("Model default", p)).child(disabled_button("Custom", p)))
                .child(help("No temperature is sent. Use this for models that don’t support temperature.", p))))
            .child(column(7.).child(disabled_value("Reasoning effort", "Model default  ⌄", p)).child(help("No effort is sent. Choose a level only if your model supports it.", p)))
            .when(self.provider == Provider::Anthropic, |s| s.child(separator(p)).child(disabled_value("Thinking & token limits", "Not yet connected", p)))
    }
    fn capture(&self, p: Palette) -> Div {
        column(14.)
            .child(section("Screenshot Shortcut", "Open the unified selector: hover a window, click blank space for the screen, or drag a rectangle.", "camera.viewfinder", p,
                column(12.)
                    .child(disabled_toggle("Enable screenshot shortcut ⌃⌥⌘S", false, p))
                    .child(disabled_value("Shortcut", "⌃⌥⌘S", p))
                    .child(disabled_toggle("Include cursor in screenshots", false, p))
                    .child(disabled_toggle("Auto-copy captured screenshot", false, p))
                    .child(help("Screenshot shortcuts and capture defaults are not yet connected in this Rust build.", p))))
            .child(section("Capture Behavior", "These controls affect screenshot capture and scrolling screenshots.", "rectangle.dashed", p,
                column(12.)
                    .child(column(8.).child(capture_hint("Hover", "Highlights the window under the pointer.", p)).child(capture_hint("Click", "Captures the highlighted window, or the whole screen on blank space.", p)).child(capture_hint("Drag", "Captures the rectangle you draw and keeps editing inline.", p)))
                    .child(separator(p))
                    .child(disabled_stepper("Scrolling max frames: 20", p))
                    .child(disabled_toggle("Remove repeated sticky headers/footers", true, p))
                    .child(separator(p))
                    .child(disabled_value("Advanced capture engine", "Automatic  ⌄", p))
                    .child(help("The unified selector and scrolling capture are not yet connected. OCR only runs when you ask for it.", p))))
            .child(section("Diagnostics", "Capture display metadata when screenshot behavior needs debugging.", "stethoscope", p,
                column(12.)
                    .child(disabled_toggle("Enable screenshot diagnostics logging", false, p))
                    .child(div().flex().child(disabled_button("Export Diagnostics Log…", p)))
                    .child(help("Capture diagnostics logging and export are not yet connected. No screenshot pixels, OCR text, image payloads, or API keys are logged here.", p))))
    }
    fn recording(&self, p: Palette) -> Div {
        section("Recording Defaults", "Set the options used when a recording target is selected.", "record.circle", p,
            column(12.)
                .child(disabled_toggle("Include cursor in recordings", true, p))
                .child(disabled_value("Audio", "None  ⌄", p))
                .child(disabled_value("Click overlays", "Click rings + labels  ⌄", p))
                .child(disabled_value("Keystroke overlays", "Shortcuts only  ⌄", p))
                .child(disabled_value("Secure-field protection", "Strict  ⌄", p))
                .child(disabled_value("Quality", "Balanced  ⌄", p))
                .child(disabled_stepper("Countdown: 3s", p))
                .child(disabled_value("Output", "Movie  ⌄", p))
                .child(separator(p))
                .child(disabled_toggle("Enable recording shortcut ⌃⌥⌘R", false, p))
                .child(disabled_value("Shortcut", "⌃⌥⌘R", p))
                .child(help("Default keystroke capture is shortcuts-only. The original app suppresses printable key overlays while typing into secure fields.", p))
                .child(help("Recording and its defaults are not yet connected in this Rust build. No recording or key monitoring starts here.", p)))
    }
    fn ocr(&self, p: Palette) -> Div {
        section("Screenshot OCR", "OCR is never automatic. These defaults apply only after you request OCR in the screenshot editor.", "text.viewfinder", p,
            column(12.)
                .child(disabled_value("OCR recognition", "Accurate  ⌄", p))
                .child(disabled_toggle("Use OCR language correction", true, p))
                .child(disabled_toggle("Show OCR text-region overlay by default", false, p))
                .child(disabled_field("OCR language hints, comma separated", p))
                .child(separator(p))
                .child(disabled_stepper("LLM OCR max long edge: 2200 px", p))
                .child(disabled_toggle("Include Mac OCR as LLM OCR hint", true, p))
                .child(help("LLM OCR still asks before uploading the edited screenshot image.", p))
                .child(help("These OCR defaults and LLM image uploads are not yet connected. No image is read or uploaded from Settings.", p)))
    }
    fn permissions(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let mut rows = column(12.);
        for (index, (permission, title, missing, granted)) in [
            (
                Permission::Accessibility,
                "Accessibility access",
                "Required to read selected text and replace it.",
                "Granted — selection reading is available; replacement is not yet connected.",
            ),
            (
                Permission::ScreenCapture,
                "Screen Recording",
                "Required for screenshots and recordings.",
                "Granted — screen capture is allowed; recording is not yet connected.",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let state = self
                .permissions
                .iter()
                .find(|s| s.permission == permission)
                .map(|s| s.state)
                .unwrap_or(PermissionState::NotApplicable);
            let trusted = state == PermissionState::Granted;
            let detail = if state == PermissionState::NotApplicable {
                "macOS permission status is unavailable on this platform."
            } else if trusted {
                granted
            } else {
                missing
            };
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(3.))
                    .child(div().w(px(24.)).flex_none().child(theme::tool_icon(
                        "lock.shield",
                        20.,
                        Palette {
                            accent: if trusted { p.success } else { p.secondary },
                            ..p
                        },
                    )))
                    .child(
                        column(2.)
                            .flex_1()
                            .min_w_0()
                            .child(title)
                            .child(help(detail, p)),
                    )
                    .when(!trusted && state != PermissionState::NotApplicable, |s| {
                        if self.permission_busy {
                            s.child(disabled_button("Opening…", p))
                        } else {
                            s.child(
                                button(("permission-settings", index), "Open Settings…", p)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_permission(permission, cx)
                                    })),
                            )
                        }
                    }),
            );
        }
        for (title, detail) in [
            ("Microphone", "Optional for recording microphone audio."),
            (
                "Input Monitoring",
                "Optional for click and keyboard overlays while recording.",
            ),
        ] {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(3.))
                    .child(div().w(px(24.)).flex_none().child(theme::tool_icon(
                        "lock.shield",
                        20.,
                        Palette {
                            accent: p.secondary,
                            ..p
                        },
                    )))
                    .child(
                        column(2.)
                            .flex_1()
                            .min_w_0()
                            .child(title)
                            .child(help(detail, p))
                            .child(help(
                                "Status and permission actions are not yet connected.",
                                p,
                            )),
                    )
                    .child(disabled_button("Grant…", p)),
            );
        }
        rows = rows.child(help("Status checks are read-only. Open Settings opens the native privacy pane; it never grants access or triggers a permission prompt.", p));
        section(
            "macOS Permissions",
            "Bello Box asks lazily, but granting here makes setup predictable.",
            "lock.shield",
            p,
            rows,
        )
    }
    fn prompt(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        section(
            "System Prompt",
            "This instruction is sent with text AI actions.",
            "text.alignleft",
            p,
            column(12.)
                .child(editor_field(self.prompt.clone(), 240., p))
                .child(
                    div().flex().child(
                        button("reset-system-prompt", "Reset to default", p)
                            .text_color(p.accent)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.prompt.update(cx, |editor, cx| {
                                    editor.set_text(DEFAULT_SYSTEM_PROMPT.into(), cx)
                                });
                            })),
                    ),
                )
                .child(help(
                    "Saved locally. Editing this instruction does not send it to a provider.",
                    p,
                )),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        if p.primary != self.editor_ink {
            self.editor_ink = p.primary;
            for (editor, compact) in [
                (&self.endpoint, true),
                (&self.model, true),
                (&self.prompt, false),
            ] {
                editor.update(cx, |editor, cx| {
                    editor.set_appearance(editor_appearance(p, compact), cx)
                });
            }
        }
        let content = match self.category {
            Category::General => self.general(p, cx),
            Category::Ai => self.ai(p, cx),
            Category::Capture => self.capture(p),
            Category::Recording => self.recording(p),
            Category::Ocr => self.ocr(p),
            Category::Permissions => self.permissions(p, cx),
            Category::Prompt => self.prompt(p, cx),
        };
        div()
            .size_full()
            .flex()
            .bg(p.bg)
            .text_color(p.primary)
            .text_size(px(13.))
            .line_height(px(18.))
            .font_family(theme::ui_font())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if (event.keystroke.modifiers.platform
                    || cfg!(target_os = "linux") && event.keystroke.modifiers.control)
                    && event.keystroke.key == "w"
                {
                    crate::shutdown::close_window(window, cx);
                    cx.stop_propagation();
                } else if event.keystroke.key == "escape" && this.model_menu {
                    this.model_menu = false;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(self.sidebar(p, cx))
            .child(div().w(px(1.)).h_full().flex_none().bg(p.separator))
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        column(18.)
                            .p(px(24.))
                            .w_full()
                            .child(
                                column(5.)
                                    .child(
                                        div()
                                            .text_size(px(28.))
                                            .line_height(px(34.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(self.category.title()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .line_height(px(17.))
                                            .text_color(p.secondary)
                                            .child(self.category.explanation()),
                                    ),
                            )
                            .when_some(self.status.clone(), |s, message| {
                                s.child(
                                    div()
                                        .p(px(10.))
                                        .rounded(px(8.))
                                        .bg(opacity(p.danger, 0.08))
                                        .text_color(p.danger)
                                        .text_size(px(11.))
                                        .child(message),
                                )
                            })
                            .child(content),
                    ),
            )
    }
}

/// Like SettingsWindowController, reopening focuses the same ordinary window.
pub fn open(cx: &mut App) {
    for window in cx.windows() {
        if let Some(settings) = window.downcast::<SettingsView>()
            && settings
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            cx.activate(true);
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(900.), px(720.)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(900.), px(680.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Bello Box Settings".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            crate::shutdown::guard_window(window, cx);
            cx.new(|cx| SettingsView::new(window, cx))
        },
    ) {
        eprintln!("Cannot open Settings: {error}");
    }
}

fn edit_saved(
    path: &Path,
    change: impl FnOnce(&mut Settings) -> Result<(), String>,
) -> Result<(), String> {
    // Reload before a narrow edit so a tool's latest favorites/recents survive.
    let mut settings = Settings::load(path)?;
    change(&mut settings)?;
    settings.save(path)
}
/// The source app's offline choices, never a claim that an endpoint offers them.
fn model_presets(provider: Provider) -> &'static [&'static str] {
    match provider {
        Provider::OpenAi => &[
            "gpt-4o-mini",
            "gpt-4o",
            "gpt-4.1-mini",
            "gpt-4.1",
            "o3-mini",
            "gpt-3.5-turbo",
        ],
        Provider::Anthropic => &[
            "claude-3-5-haiku-latest",
            "claude-3-5-sonnet-latest",
            "claude-3-7-sonnet-latest",
            "claude-3-opus-latest",
        ],
        Provider::Codex => &[
            "gpt-5.5",
            "gpt-5-codex",
            "gpt-5",
            "o4-mini",
            "o3",
            "gpt-4.1",
        ],
    }
}
fn validate_provider(provider: Provider, endpoint: &str, model: &str) -> Result<(), String> {
    if model.len() > 256 || model.contains(['\n', '\r']) {
        return Err("Model names must be a single line of at most 256 bytes.".into());
    }
    if endpoint.len() > 4096 || endpoint.contains(['\n', '\r']) {
        return Err("Endpoint or command must be a single line of at most 4,096 bytes.".into());
    }
    if provider != Provider::Codex {
        let url = reqwest::Url::parse(endpoint.trim())
            .map_err(|_| "Enter a complete HTTP(S) endpoint.")?;
        if !["https", "http"].contains(&url.scheme())
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("Use an HTTP(S) endpoint without credentials, a query, or a fragment. API keys are never saved in endpoints.".into());
        }
    }
    Ok(())
}
fn column(gap: f32) -> Div {
    div().flex().flex_col().gap(px(gap))
}
fn help(text: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .text_size(px(11.))
        .line_height(px(16.))
        .text_color(p.secondary)
        .child(text.into())
}
fn separator(p: Palette) -> Div {
    div().h(px(1.)).w_full().flex_none().bg(p.separator)
}
fn section(
    title: &'static str,
    subtitle: &'static str,
    symbol: &'static str,
    p: Palette,
    content: Div,
) -> Div {
    column(14.)
        .p(px(16.))
        .rounded(px(12.))
        .border_1()
        .border_color(p.separator)
        .bg(p.surface)
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .rounded(px(9.))
                        .bg(opacity(p.accent, 0.10))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(theme::tool_icon(symbol, 16., p)),
                )
                .child(
                    column(3.)
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(help(subtitle, p)),
                ),
        )
        .child(content)
}
fn button(id: impl Into<ElementId>, label: &'static str, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(28.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .text_size(px(12.))
        .cursor_pointer()
        .hover(|s| s.bg(opacity(p.accent, 0.08)))
        .child(label)
}
fn disabled_button(label: &'static str, p: Palette) -> Div {
    div()
        .flex_none()
        .h(px(28.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .text_size(px(12.))
        .text_color(opacity(p.secondary, 0.65))
        .child(label)
}
fn disabled_toggle(label: &'static str, checked: bool, p: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .text_color(opacity(p.primary, 0.62))
        .child(
            div()
                .size(px(14.))
                .flex_none()
                .rounded(px(3.))
                .border_1()
                .border_color(p.border)
                .bg(if checked {
                    opacity(p.accent, 0.28)
                } else {
                    p.well
                })
                .text_size(px(11.))
                .flex()
                .items_center()
                .justify_center()
                .child(if checked { "✓" } else { "" }),
        )
        .child(label)
}
fn disabled_value(label: &'static str, value: &'static str, p: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .text_color(opacity(p.primary, 0.62))
        .child(label)
        .child(div().flex_1())
        .child(disabled_button(value, p))
}
fn disabled_stepper(label: &'static str, p: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .text_color(opacity(p.primary, 0.62))
        .child(label)
        .child(div().flex_1())
        .child(disabled_button("−   +", p))
}
fn disabled_field(label: &'static str, p: Palette) -> Div {
    div()
        .h(px(30.))
        .flex()
        .items_center()
        .px(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .text_color(opacity(p.secondary, 0.65))
        .text_size(px(12.))
        .child(label)
}
fn labeled(label: &'static str, p: Palette, child: impl IntoElement) -> Div {
    column(3.)
        .child(
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .text_color(p.secondary)
                .child(label),
        )
        .child(child)
}
fn choice(
    id: impl Into<ElementId>,
    label: &'static str,
    selected: bool,
    p: Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .h(px(27.))
        .flex()
        .items_center()
        .justify_center()
        .px(px(6.))
        .rounded(px(5.))
        .text_size(px(12.))
        .text_color(if selected { p.accent } else { p.secondary })
        .cursor_pointer()
        .when(selected, |s| {
            s.bg(p.surface).border_1().border_color(p.separator)
        })
        .child(label)
}
fn choice_bar(choices: impl IntoIterator<Item = Stateful<Div>>, p: Palette) -> Div {
    div()
        .flex()
        .p(px(2.))
        .gap(px(2.))
        .rounded(px(7.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .children(choices)
}
fn capture_hint(title: &'static str, detail: &'static str, p: Palette) -> Div {
    div()
        .flex()
        .gap(px(10.))
        .child(
            div()
                .w(px(54.))
                .flex_none()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(p.accent)
                .child(title),
        )
        .child(help(detail, p).flex_1().min_w_0())
}
fn surface_choice(title: &'static str, detail: &'static str, selected: bool, p: Palette) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(10.))
        .p(px(12.))
        .min_h(px(68.))
        .rounded(px(10.))
        .border_1()
        .border_color(if selected { p.accent } else { p.separator })
        .bg(p.well)
        .child(theme::tool_icon(
            if selected {
                "square.fill"
            } else {
                "square.on.square"
            },
            18.,
            p,
        ))
        .child(
            column(4.)
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(help(detail, p)),
        )
        .child(
            div()
                .text_color(if selected { p.accent } else { p.secondary })
                .child(if selected { "✓" } else { "○" }),
        )
}
fn appearance_preview(dark: bool) -> Div {
    let p = theme::for_dark(dark);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .gap(px(5.))
        .p(px(8.))
        .bg(p.bg)
        .child(
            div()
                .w(px(14.))
                .h_full()
                .flex_none()
                .rounded(px(3.))
                .bg(opacity(p.brand, if dark { 0.7 } else { 0.35 })),
        )
        .child(
            column(5.)
                .flex_1()
                .child(
                    div()
                        .w(px(25.))
                        .h(px(3.))
                        .rounded(px(2.))
                        .bg(opacity(p.primary, 0.55)),
                )
                .child(div().flex_1().rounded(px(3.)).bg(p.surface))
                .child(div().flex_1().rounded(px(3.)).bg(p.surface)),
        )
}
fn editor_appearance(p: Palette, compact: bool) -> EditorAppearance {
    EditorAppearance {
        font_family: if compact {
            theme::ui_font().into()
        } else {
            "monospace".into()
        },
        font_size: 13.,
        line_height: 19.,
        padding_x: 2.,
        padding_y: 4.,
        text: p.primary,
        caret: p.accent,
        normal_caret: opacity(p.accent, 0.55),
        selection: opacity(p.accent, 0.2),
        background: None,
        wrap_lines: !compact,
        ..EditorAppearance::plain()
    }
}
fn make_editor(
    text: String,
    compact: bool,
    p: Palette,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
) -> Entity<EditorView> {
    cx.new(|cx| {
        let mut editor = EditorView::new(text, window, cx);
        editor.set_compact(compact, cx);
        editor.set_appearance(editor_appearance(p, compact), cx);
        editor
    })
}
fn single_line_field(
    editor: Entity<EditorView>,
    placeholder: &'static str,
    p: Palette,
    cx: &App,
) -> Div {
    let empty = editor.read(cx).text().is_empty();
    let focus = editor.clone();
    editor_field(editor, 30., p)
        .relative()
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
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            focus.read(cx).focus(window)
        })
}
fn editor_field(editor: Entity<EditorView>, height: f32, p: Palette) -> Div {
    div()
        .h(px(height))
        .flex_none()
        .px(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .overflow_hidden()
        .child(editor)
}

#[cfg(test)]
mod tests {
    use super::{Category, Provider, edit_saved, editor_appearance, validate_provider};
    use crate::theme;
    use bellobox_core::settings::{Appearance, Settings};
    #[test]
    fn categories_and_order_match_swift_source() {
        assert_eq!(
            Category::ALL.map(Category::title),
            [
                "General",
                "AI Provider",
                "Screenshots",
                "Recording",
                "OCR",
                "Permissions",
                "Prompt"
            ]
        );
        assert_eq!(Category::fixture("ocr"), Category::Ocr);
        assert_eq!(Category::fixture("unknown"), Category::General);
    }
    #[test]
    fn provider_fields_reject_embedded_credentials_and_unbounded_values() {
        for endpoint in [
            "https://user:secret@example.com/v1",
            "https://example.com?api_key=secret",
            "https://example.com#secret",
            "file:///tmp/key",
        ] {
            assert!(validate_provider(Provider::OpenAi, endpoint, "model").is_err());
        }
        assert!(validate_provider(Provider::OpenAi, "http://localhost:11434/v1", "model").is_ok());
        assert!(
            validate_provider(
                Provider::OpenAi,
                "https://api.openai.com/v1",
                &"m".repeat(257)
            )
            .is_err()
        );
        assert!(validate_provider(Provider::Codex, "codex\ncommand", "model").is_err());
    }
    #[test]
    fn narrow_edits_preserve_recent_activity_and_do_not_clobber_bad_files() {
        let folder = std::env::temp_dir().join(format!(
            "bellobox-settings-ui-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("settings.json");
        let mut settings = Settings::default();
        settings.favorites.insert("json".into());
        settings.explicit_open("qr", "text", 1.);
        settings.save(&path).unwrap();
        edit_saved(&path, |s| {
            s.appearance = Appearance::Dark;
            Ok(())
        })
        .unwrap();
        let saved = Settings::load(&path).unwrap();
        assert_eq!(saved.appearance, Appearance::Dark);
        assert!(saved.favorites.contains("json"));
        assert_eq!(saved.recents, ["qr"]);
        std::fs::write(&path, "malformed").unwrap();
        assert!(
            edit_saved(&path, |s| {
                s.appearance = Appearance::Light;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "malformed");
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn editor_theme_keeps_plain_chrome_and_prompt_wrap() {
        let p = theme::for_dark(true);
        let field = editor_appearance(p, true);
        let prompt = editor_appearance(p, false);
        assert!(
            !field.show_gutter && !field.show_status && !field.show_vim_toggle && !field.wrap_lines
        );
        assert!(prompt.wrap_lines);
        assert_eq!(field.text, p.primary);
        assert_eq!(prompt.text, p.primary);
    }
}

#[cfg(test)]
mod setup_tests;
