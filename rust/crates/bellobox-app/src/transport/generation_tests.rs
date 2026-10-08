use super::*;
use bellobox_core::{
    ai::{
        self,
        generation::{Preferences, ReasoningEffort, Thinking},
    },
    settings::Settings,
};
#[test]
fn runtime_route_selects_matching_saved_profile_and_snapshots_it() {
    let mut settings = Settings {
        provider_model: "visible".into(),
        ..Default::default()
    };
    settings
        .change_generation(
            Provider::OpenAIChat,
            "https://api.openai.com/v1",
            "visible",
            Some(Preferences {
                custom_temperature: true,
                temperature: 0.4,
                ..Default::default()
            }),
            1,
        )
        .unwrap();
    settings
        .change_generation(
            Provider::Anthropic,
            "http://127.0.0.1:2/v1",
            "effective",
            Some(Preferences {
                thinking: Thinking::Adaptive,
                reasoning_effort: Some(ReasoningEffort::Max),
                output_token_limit: 12288,
                ..Default::default()
            }),
            2,
        )
        .unwrap();
    let snapshot = config_from_settings(
        settings.clone(),
        Some("anthropic"),
        Some("http://127.0.0.1:2/v1"),
        Some("effective"),
    )
    .unwrap();
    settings
        .change_generation(
            Provider::Anthropic,
            "http://127.0.0.1:2/v1",
            "effective",
            None,
            3,
        )
        .unwrap();
    let body = ai::request(&snapshot, "fixture-key", "hello", "")
        .unwrap()
        .body;
    assert_eq!(body["model"], "effective");
    assert_eq!(body["max_tokens"], 12288);
    assert_eq!(body["thinking"]["type"], "adaptive");
    assert_eq!(body["output_config"]["effort"], "max");
    assert!(body.get("temperature").is_none());
    let reset = config_from_settings(
        settings,
        Some("anthropic"),
        Some("http://127.0.0.1:2/v1"),
        Some("effective"),
    )
    .unwrap();
    assert_eq!(reset.generation_options, Default::default());
    assert_eq!(reset.max_output_tokens, 2048);
}
