use super::*;
use bellobox_core::ai::generation::{self, Preferences, ReasoningEffort, Thinking};

#[derive(Clone, Copy)]
pub(super) enum Edit {
    TemperatureMode(bool),
    Temperature(f64),
    Effort(Option<ReasoningEffort>),
    Thinking(Thinking),
    Budget(i32),
    Output(i32),
    Reset,
}
impl SettingsView {
    pub(super) fn generation_provider(&self) -> bellobox_core::ai::Provider {
        match self.provider {
            Provider::Anthropic => bellobox_core::ai::Provider::Anthropic,
            _ if self.responses => bellobox_core::ai::Provider::OpenAIResponses,
            _ => bellobox_core::ai::Provider::OpenAIChat,
        }
    }
    pub(super) fn generation_preferences(&self, cx: &App) -> Preferences {
        let provider = self.generation_provider();
        let key = generation::key(
            provider,
            self.endpoint.read(cx).text(),
            self.model.read(cx).text(),
        );
        self.generation
            .get(&key)
            .map(|e| e.options)
            .unwrap_or_default()
            .normalized(provider)
    }
    pub(super) fn edit_generation(&mut self, edit: Edit, cx: &mut Context<Self>) {
        if self.provider == Provider::Codex {
            return;
        }
        let endpoint = self.endpoint.read(cx).text().to_owned();
        let model = self.model.read(cx).text().to_owned();
        if let Err(error) = validate_provider(self.provider, &endpoint, &model) {
            self.status = Some(format!("Not saved: {error}"));
            cx.notify();
            return;
        }
        let provider = self.generation_provider();
        let modified_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .min(u64::MAX as u128) as u64;
        let mut saved = None;
        let success = self.persist(
            |settings| {
                // Resolve from the freshly loaded file, never overwrite another window's newer edit.
                let mut options = settings.generation_preferences(provider, &endpoint, &model);
                match edit {
                    Edit::TemperatureMode(v) => options.custom_temperature = v,
                    Edit::Temperature(delta) => options.temperature += delta,
                    Edit::Effort(v) => options.reasoning_effort = v,
                    Edit::Thinking(v) => options.thinking = v,
                    Edit::Budget(delta) => {
                        options.thinking_budget =
                            options.thinking_budget.saturating_add_signed(delta)
                    }
                    Edit::Output(delta) => {
                        options.output_token_limit = effective_output(options)
                            .saturating_add_signed(delta)
                            .max(minimum_output(options))
                    }
                    Edit::Reset => {}
                }
                settings.change_generation(
                    provider,
                    &endpoint,
                    &model,
                    (!matches!(edit, Edit::Reset)).then_some(options),
                    modified_at,
                )?;
                saved = Some(settings.model_generation.clone());
                Ok(())
            },
            cx,
        );
        if success {
            self.generation = saved.expect("successful mutation produces preferences");
            self.setup.invalidate(false);
        }
    }
    fn generation_button(
        &self,
        id: &'static str,
        label: &'static str,
        edit: Edit,
        selected: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        button(id, label, p)
            .when(selected, |s| {
                s.bg(opacity(p.accent, 0.12)).text_color(p.accent)
            })
            .on_click(cx.listener(move |this, _, _, cx| this.edit_generation(edit, cx)))
    }
    pub(super) fn model_behavior(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let model = self.model.read(cx).text().to_owned();
        let options = self.generation_preferences(cx);
        let mut panel=column(12.).p(px(14.)).rounded(px(12.)).border_1().border_color(p.separator).bg(p.well)
            .child(div().flex().items_center().gap(px(8.)).child(theme::tool_icon("slider.horizontal.3",16.,p)).child("Model behavior"))
            .child(help(if model.trim().is_empty() {"Choose a model above".into()} else {format!("{model} · Saved separately for this model and endpoint. Switching back restores your choices.")},p));
        if self.provider == Provider::Codex {
            return panel.child(help("Codex app-server behavior is not connected yet.", p));
        }
        if model.trim().is_empty() {
            return panel;
        }
        panel = panel
            .child(self.generation_button(
                "generation-reset",
                "Reset this model",
                Edit::Reset,
                false,
                p,
                cx,
            ))
            .child(separator(p));
        let thinking = self.provider == Provider::Anthropic
            && matches!(options.thinking, Thinking::Adaptive | Thinking::Budgeted(_));
        if thinking {
            panel=panel.child(help("Temperature: Not sent while thinking is enabled. Your temperature choice is kept for when you turn thinking off.",p));
        } else {
            panel = panel.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .child("Temperature")
                    .child(self.generation_button(
                        "temperature-default",
                        "Model default",
                        Edit::TemperatureMode(false),
                        !options.custom_temperature,
                        p,
                        cx,
                    ))
                    .child(self.generation_button(
                        "temperature-custom",
                        "Custom",
                        Edit::TemperatureMode(true),
                        options.custom_temperature,
                        p,
                        cx,
                    )),
            );
            if options.custom_temperature {
                panel = panel.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(self.generation_button(
                            "temperature-minus",
                            "−",
                            Edit::Temperature(-0.1),
                            false,
                            p,
                            cx,
                        ))
                        .child(format!("{:.1}", options.temperature))
                        .child(self.generation_button(
                            "temperature-plus",
                            "+",
                            Edit::Temperature(0.1),
                            false,
                            p,
                            cx,
                        )),
                );
            }
            panel = panel.child(help(
                "Model default omits temperature. Custom values depend on model support.",
                p,
            ));
        }
        let mut effort = div().flex().flex_wrap().gap(px(5.));
        for (id, label, value) in [
            ("effort-default", "Model default", None),
            ("effort-none", "None", Some(ReasoningEffort::None)),
            ("effort-minimal", "Minimal", Some(ReasoningEffort::Minimal)),
            ("effort-low", "Low", Some(ReasoningEffort::Low)),
            ("effort-medium", "Medium", Some(ReasoningEffort::Medium)),
            ("effort-high", "High", Some(ReasoningEffort::High)),
            ("effort-xhigh", "Extra high", Some(ReasoningEffort::XHigh)),
            ("effort-max", "Max", Some(ReasoningEffort::Max)),
        ] {
            if self.provider == Provider::Anthropic
                && matches!(
                    value,
                    Some(ReasoningEffort::None | ReasoningEffort::Minimal)
                )
            {
                continue;
            }
            effort = effort.child(self.generation_button(
                id,
                label,
                Edit::Effort(value),
                options.reasoning_effort == value,
                p,
                cx,
            ));
        }
        panel=panel.child("Reasoning effort").child(effort).child(help("Model default omits effort. Supported levels vary by model; higher effort can take longer and use more tokens.",p));
        if self.provider == Provider::Anthropic {
            let mut modes = div().flex().flex_wrap().gap(px(5.));
            for (id, label, value) in [
                (
                    "thinking-default",
                    "Model default",
                    Thinking::ProviderDefault,
                ),
                ("thinking-off", "Off", Thinking::Disabled),
                ("thinking-adaptive", "Adaptive", Thinking::Adaptive),
                (
                    "thinking-budgeted",
                    "Token budget",
                    Thinking::Budgeted(options.thinking_budget),
                ),
            ] {
                modes = modes.child(self.generation_button(
                    id,
                    label,
                    Edit::Thinking(value),
                    options.thinking == value,
                    p,
                    cx,
                ));
            }
            panel = panel
                .child(separator(p))
                .child("Thinking & token limits")
                .child(modes);
            if matches!(options.thinking, Thinking::Budgeted(_)) {
                panel = panel.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(format!("Thinking budget: {}", options.thinking_budget))
                        .child(self.generation_button(
                            "budget-minus",
                            "−",
                            Edit::Budget(-1024),
                            false,
                            p,
                            cx,
                        ))
                        .child(self.generation_button(
                            "budget-plus",
                            "+",
                            Edit::Budget(1024),
                            false,
                            p,
                            cx,
                        )),
                );
            }
            panel=panel.child(div().flex().gap(px(8.)).items_center().child(format!("Output token limit: {}",effective_output(options)))
                .child(self.generation_button("output-minus","−",Edit::Output(-1024),false,p,cx)).child(self.generation_button("output-plus","+",Edit::Output(1024),false,p,cx)))
                .child(help("Includes thinking and the answer. Adaptive uses at least 8,192 tokens; a manual budget reserves at least 2,048 answer tokens. Larger budgets can take longer and cost more.",p));
        }
        panel
    }
}
fn minimum_output(options: Preferences) -> u32 {
    match options.thinking {
        Thinking::Adaptive => 8192,
        Thinking::Budgeted(_) => options.thinking_budget + 2048,
        _ => 1024,
    }
}
fn effective_output(options: Preferences) -> u32 {
    options.output_token_limit.max(minimum_output(options))
}

#[cfg(test)]
mod tests {
    use super::{
        Edit, Preferences, ReasoningEffort, Settings, SettingsView, Thinking, effective_output,
    };
    use gpui::TestAppContext;
    #[gpui::test]
    fn edits_switch_reset_reopen_without_network_and_effective_route_snapshot(
        cx: &mut TestAppContext,
    ) {
        edits_switch_reset_reopen_without_network_and_effective_route_snapshot_flow(cx);
    }
    fn edits_switch_reset_reopen_without_network_and_effective_route_snapshot_flow(
        cx: &mut TestAppContext,
    ) {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/v1", server.local_addr().unwrap());
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("settings.json");
        Settings {
            provider_endpoint: endpoint.clone(),
            provider_model: "first".into(),
            ..Default::default()
        }
        .save(&path)
        .unwrap();
        let view = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
        cx.run_until_parked();
        view.update(cx, |view, _, cx| {
            view.select_model("second".into(), cx);
            view.select_model("first".into(), cx);
            assert!(Settings::load(&path).unwrap().model_generation.is_empty());
            view.edit_generation(Edit::TemperatureMode(true), cx);
            view.edit_generation(Edit::Temperature(-0.3), cx);
            view.edit_generation(Edit::Effort(Some(ReasoningEffort::High)), cx);
            view.select_model("second".into(), cx);
            assert_eq!(view.generation_preferences(cx), Preferences::default());
            view.select_model("first".into(), cx);
            assert_eq!(view.generation_preferences(cx).temperature, 0.7);
        })
        .unwrap();
        let saved = Settings::load(&path).unwrap();
        let snapshot =
            crate::transport::config_from_settings(saved.clone(), None, None, None).unwrap();
        assert_eq!(snapshot.generation_options.temperature, Some(0.7));
        let other =
            crate::transport::config_from_settings(saved.clone(), None, None, Some("second"))
                .unwrap();
        assert_eq!(other.generation_options, Default::default());
        let other_endpoint = crate::transport::config_from_settings(
            saved,
            None,
            Some("http://127.0.0.1:1/v1"),
            None,
        )
        .unwrap();
        assert_eq!(other_endpoint.generation_options, Default::default());
        view.update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let reopened = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
        reopened
            .update(cx, |view, _, cx| {
                assert_eq!(
                    view.generation_preferences(cx).reasoning_effort,
                    Some(ReasoningEffort::High)
                );
                view.edit_generation(Edit::Reset, cx);
                assert_eq!(view.generation_preferences(cx), Preferences::default());
            })
            .unwrap();
        assert!(Settings::load(&path).unwrap().model_generation.is_empty());
        assert_eq!(snapshot.generation_options.temperature, Some(0.7));
        assert!(matches!(server.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
    }
    #[gpui::test]
    fn thinking_preserves_temperature_and_output_stepper_uses_effective_allowance(
        cx: &mut TestAppContext,
    ) {
        thinking_preserves_temperature_and_output_stepper_uses_effective_allowance_flow(cx);
    }
    fn thinking_preserves_temperature_and_output_stepper_uses_effective_allowance_flow(
        cx: &mut TestAppContext,
    ) {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("settings.json");
        Settings {
            provider_kind: "anthropic".into(),
            provider_endpoint: "http://127.0.0.1:1/v1".into(),
            provider_model: "model".into(),
            ..Default::default()
        }
        .save(&path)
        .unwrap();
        let view = cx.add_window(|window, cx| SettingsView::new_at(path, window, cx));
        view.update(cx, |view, _, cx| {
            view.edit_generation(Edit::TemperatureMode(true), cx);
            view.edit_generation(Edit::Temperature(-0.4), cx);
            view.edit_generation(Edit::Thinking(Thinking::Adaptive), cx);
            assert_eq!(effective_output(view.generation_preferences(cx)), 8192);
            view.edit_generation(Edit::Output(-1024), cx);
            assert_eq!(effective_output(view.generation_preferences(cx)), 8192);
            view.edit_generation(Edit::Thinking(Thinking::Disabled), cx);
            assert_eq!(view.generation_preferences(cx).temperature, 0.6);
            view.edit_generation(Edit::Thinking(Thinking::Budgeted(4096)), cx);
            view.edit_generation(Edit::Budget(1024), cx);
            assert_eq!(
                view.generation_preferences(cx).thinking,
                Thinking::Budgeted(5120)
            );
        })
        .unwrap();
    }
}

#[cfg(test)]
mod failure_tests {
    use super::{Edit, Settings, SettingsView};
    use gpui::TestAppContext;
    #[gpui::test]
    fn failed_save_preserves_visible_options_and_invalid_file(cx: &mut TestAppContext) {
        failed_save_preserves_visible_options_and_invalid_file_flow(cx);
    }
    fn failed_save_preserves_visible_options_and_invalid_file_flow(cx: &mut TestAppContext) {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("settings.json");
        Settings {
            provider_model: "model".into(),
            ..Default::default()
        }
        .save(&path)
        .unwrap();
        let view = cx.add_window(|window, cx| SettingsView::new_at(path.clone(), window, cx));
        view.update(cx, |view, _, cx| {
            view.edit_generation(Edit::TemperatureMode(true), cx)
        })
        .unwrap();
        std::fs::write(&path, b"corrupt preferences").unwrap();
        view.update(cx, |view, _, cx| {
            let before = view.generation_preferences(cx);
            view.edit_generation(Edit::Temperature(-0.5), cx);
            assert_eq!(view.generation_preferences(cx), before);
            assert!(view.status.as_deref().unwrap().starts_with("Not saved:"));
            view.edit_generation(Edit::Reset, cx);
            assert_eq!(view.generation_preferences(cx), before);
        })
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt preferences");
    }
}
