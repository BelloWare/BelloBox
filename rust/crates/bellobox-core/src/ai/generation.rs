//! Source generation preferences shared by text, Clock Copilot and confirmed image requests.
use super::Provider;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}
impl ReasoningEffort {
    pub fn wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Thinking {
    #[default]
    ProviderDefault,
    Disabled,
    Adaptive,
    Budgeted(u32),
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GenerationOptions {
    pub temperature: Option<f64>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub thinking: Thinking,
}

impl GenerationOptions {
    pub fn apply(self, body: &mut Value, provider: Provider, output: u32) {
        let anthropic = provider == Provider::Anthropic;
        if let Some(t) = self.temperature.filter(|v| v.is_finite())
            && !(anthropic && matches!(self.thinking, Thinking::Adaptive | Thinking::Budgeted(_)))
        {
            body["temperature"] = json!(t.clamp(0.0, if anthropic { 1.0 } else { 2.0 }));
        }
        if let Some(effort) = self.reasoning_effort
            && !(anthropic && matches!(effort, ReasoningEffort::None | ReasoningEffort::Minimal))
        {
            match provider {
                Provider::OpenAIChat => body["reasoning_effort"] = json!(effort.wire()),
                Provider::OpenAIResponses => body["reasoning"] = json!({"effort": effort.wire()}),
                Provider::Anthropic => body["output_config"] = json!({"effort": effort.wire()}),
            }
        }
        if anthropic {
            match self.thinking {
                Thinking::ProviderDefault => {}
                Thinking::Disabled => body["thinking"] = json!({"type":"disabled"}),
                Thinking::Adaptive => {
                    body["thinking"] = json!({"type":"adaptive"});
                    body["max_tokens"] = json!(output.max(8192));
                }
                Thinking::Budgeted(v) => {
                    let budget = v.clamp(1024, 32768);
                    body["thinking"] = json!({"type":"enabled", "budget_tokens":budget});
                    body["max_tokens"] = json!(output.max(budget + 2048));
                }
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct Preferences {
    pub custom_temperature: bool,
    pub temperature: f64,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub thinking: Thinking,
    pub thinking_budget: u32,
    pub output_token_limit: u32,
}
impl<'de> Deserialize<'de> for Preferences {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let fields = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("Generation preferences must be an object"))?;
        fn field<T: serde::de::DeserializeOwned>(
            fields: &serde_json::Map<String, Value>,
            name: &str,
            fallback: T,
        ) -> T {
            fields
                .get(name)
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(fallback)
        }
        // Swift decodes signed Int then normalizes, so negative values clamp to the lower bound.
        fn tokens(
            fields: &serde_json::Map<String, Value>,
            name: &str,
            fallback: u32,
            upper: u32,
        ) -> u32 {
            fields
                .get(name)
                .and_then(Value::as_i64)
                .map(|v| v.clamp(1024, i64::from(upper)) as u32)
                .unwrap_or(fallback)
        }
        Ok(Self {
            custom_temperature: field(fields, "custom_temperature", false),
            temperature: field(fields, "temperature", 1.0),
            reasoning_effort: field(fields, "reasoning_effort", None),
            thinking: field(fields, "thinking", Thinking::ProviderDefault),
            thinking_budget: tokens(fields, "thinking_budget", 4096, 32768),
            output_token_limit: tokens(fields, "output_token_limit", 2048, 65536),
        })
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            custom_temperature: false,
            temperature: 1.0,
            reasoning_effort: None,
            thinking: Thinking::ProviderDefault,
            thinking_budget: 4096,
            output_token_limit: 2048,
        }
    }
}
impl Preferences {
    pub fn normalized(mut self, provider: Provider) -> Self {
        self.temperature = if self.temperature.is_finite() {
            (self.temperature.clamp(
                0.0,
                if provider == Provider::Anthropic {
                    1.0
                } else {
                    2.0
                },
            ) * 100.0)
                .round()
                / 100.0
        } else {
            1.0
        };
        if provider == Provider::Anthropic
            && matches!(
                self.reasoning_effort,
                Some(ReasoningEffort::None | ReasoningEffort::Minimal)
            )
        {
            self.reasoning_effort = None;
        }
        self.thinking_budget = self.thinking_budget.clamp(1024, 32768);
        self.output_token_limit = self.output_token_limit.clamp(1024, 65536);
        if matches!(self.thinking, Thinking::Budgeted(_)) {
            self.thinking = Thinking::Budgeted(self.thinking_budget);
        }
        self
    }
    pub fn options(self, provider: Provider) -> GenerationOptions {
        let p = self.normalized(provider);
        GenerationOptions {
            temperature: p.custom_temperature.then_some(p.temperature),
            reasoning_effort: p.reasoning_effort,
            thinking: if provider == Provider::Anthropic {
                p.thinking
            } else {
                Thinking::ProviderDefault
            },
        }
    }
}
/// OpenAI API format is intentionally not part of the source's provider identity.
pub fn key(provider: Provider, endpoint: &str, model: &str) -> String {
    let mut endpoint = endpoint.trim().to_owned();
    if let Some((scheme, rest)) = endpoint.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        endpoint = format!(
            "{}://{}{}",
            scheme.to_ascii_lowercase(),
            authority.to_ascii_lowercase(),
            if path.trim_end_matches('/').is_empty() {
                String::new()
            } else {
                format!("/{}", path.trim_end_matches('/'))
            }
        );
    }
    let provider = if provider == Provider::Anthropic {
        "anthropic"
    } else {
        "openAI"
    };
    let bytes =
        serde_json::to_vec(&[provider, &endpoint, model.trim()]).expect("strings serialize");
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub options: Preferences,
    pub modified_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_keys_share_openai_formats_but_isolate_routes_and_models() {
        let k = key(
            Provider::OpenAIChat,
            " HTTPS://EXAMPLE.TEST/v1/// ",
            " model ",
        );
        assert_eq!(
            k,
            key(
                Provider::OpenAIResponses,
                "https://example.test/v1",
                "model"
            )
        );
        assert_ne!(
            k,
            key(Provider::Anthropic, "https://example.test/v1", "model")
        );
        assert_ne!(
            k,
            key(Provider::OpenAIChat, "https://example.test/v2", "model")
        );
        assert_ne!(
            k,
            key(Provider::OpenAIChat, "https://example.test/v1", "other")
        );
    }
    #[test]
    fn preferences_bound_values_without_guessing_model_capabilities() {
        let p = Preferences {
            temperature: f64::NAN,
            reasoning_effort: Some(ReasoningEffort::Minimal),
            thinking: Thinking::Budgeted(0),
            thinking_budget: u32::MAX,
            output_token_limit: 0,
            ..Default::default()
        }
        .normalized(Provider::Anthropic);
        assert_eq!(p.temperature, 1.0);
        assert_eq!(p.reasoning_effort, None);
        assert_eq!(p.thinking, Thinking::Budgeted(32768));
        assert_eq!(p.output_token_limit, 1024);
        assert_eq!(
            Preferences {
                temperature: 1.236,
                ..Default::default()
            }
            .normalized(Provider::OpenAIChat)
            .temperature,
            1.24
        );
    }
    #[test]
    fn exact_optional_wire_fields_and_thinking_temperature_exclusion() {
        for provider in [
            Provider::OpenAIChat,
            Provider::OpenAIResponses,
            Provider::Anthropic,
        ] {
            let mut b = json!({});
            GenerationOptions::default().apply(&mut b, provider, 2048);
            assert_eq!(b, json!({}));
            let mut b = json!({});
            GenerationOptions {
                temperature: Some(0.7),
                reasoning_effort: Some(ReasoningEffort::High),
                thinking: Thinking::Budgeted(8192),
            }
            .apply(&mut b, provider, 2048);
            let expected = match provider {
                Provider::OpenAIChat => json!({"temperature":0.7,"reasoning_effort":"high"}),
                Provider::OpenAIResponses => {
                    json!({"temperature":0.7,"reasoning":{"effort":"high"}})
                }
                Provider::Anthropic => {
                    json!({"output_config":{"effort":"high"},"thinking":{"type":"enabled","budget_tokens":8192},"max_tokens":10240})
                }
            };
            assert_eq!(b, expected);
        }
    }
    #[test]
    fn explicit_mutation_persists_switches_reopens_resets_and_caps_recent_profiles() {
        use crate::settings::Settings;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = Settings::load(&path).unwrap();
        for i in 0..200 {
            let _ = s.generation_preferences(
                Provider::OpenAIChat,
                "https://example.test/v1",
                &format!("m{i}"),
            );
        }
        assert!(s.model_generation.is_empty());
        assert!(!path.exists());
        let p = Preferences {
            custom_temperature: true,
            temperature: 0.3,
            reasoning_effort: Some(ReasoningEffort::High),
            ..Default::default()
        };
        s.change_generation(
            Provider::OpenAIChat,
            "https://example.test/v1",
            "chosen",
            Some(p),
            1000,
        )
        .unwrap();
        s.save(&path).unwrap();
        let mut reopened = Settings::load(&path).unwrap();
        assert_eq!(
            reopened.generation_preferences(
                Provider::OpenAIResponses,
                "https://EXAMPLE.test/v1/",
                " chosen "
            ),
            p
        );
        assert_eq!(
            reopened.generation_preferences(
                Provider::OpenAIChat,
                "https://example.test/v1",
                "other"
            ),
            Preferences::default()
        );
        for i in 0..150 {
            reopened
                .change_generation(
                    Provider::OpenAIChat,
                    "https://example.test/v1",
                    &format!("m{i}"),
                    Some(p),
                    i,
                )
                .unwrap();
        }
        assert_eq!(reopened.model_generation.len(), 128);
        assert!(reopened.model_generation.contains_key(&key(
            Provider::OpenAIChat,
            "https://example.test/v1",
            "chosen"
        )));
        reopened
            .change_generation(
                Provider::OpenAIChat,
                "https://example.test/v1",
                "chosen",
                None,
                2000,
            )
            .unwrap();
        reopened.save(&path).unwrap();
        assert_eq!(
            Settings::load(&path).unwrap().generation_preferences(
                Provider::OpenAIChat,
                "https://example.test/v1",
                "chosen"
            ),
            Preferences::default()
        );
    }
}

#[cfg(test)]
mod decoding_tests {
    use super::*;
    #[test]
    fn settings_with_bad_optional_fields_keeps_valid_routing_and_siblings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let profile = key(Provider::Anthropic, "https://example.test/v1", "model");
        let value = json!({"provider_kind":"anthropic","provider_endpoint":"https://example.test/v1","provider_model":"model","model_generation":{profile:{"modified_at":1,"options":{"temperature":0.7,"custom_temperature":true,"reasoning_effort":"future","thinking_budget":-1,"output_token_limit":false}}}});
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let settings = crate::settings::Settings::load(&path).unwrap();
        assert_eq!(settings.provider_kind, "anthropic");
        assert_eq!(settings.provider_endpoint, "https://example.test/v1");
        assert_eq!(settings.provider_model, "model");
        let options = settings.generation_preferences(
            Provider::Anthropic,
            &settings.provider_endpoint,
            &settings.provider_model,
        );
        assert_eq!(options.temperature, 0.7);
        assert!(options.custom_temperature);
        assert_eq!(options.reasoning_effort, None);
        assert_eq!(options.thinking_budget, 1024);
        assert_eq!(options.output_token_limit, 2048);
        let mut invalid = value;
        invalid["provider_endpoint"] = json!([]);
        std::fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(crate::settings::Settings::load(&path).is_err());
    }
    #[test]
    fn malformed_fields_default_independently_and_signed_tokens_clamp() {
        let p:Preferences=serde_json::from_value(json!({"custom_temperature":"bad","temperature":false,"reasoning_effort":"future","thinking":"future","thinking_budget":-42,"output_token_limit":999999})).unwrap();
        assert!(!p.custom_temperature);
        assert_eq!(p.temperature, 1.0);
        assert_eq!(p.reasoning_effort, None);
        assert_eq!(p.thinking, Thinking::ProviderDefault);
        assert_eq!(p.thinking_budget, 1024);
        assert_eq!(p.output_token_limit, 65536);
        let p:Preferences=serde_json::from_value(json!({"custom_temperature":true,"temperature":0.7,"thinking_budget":"bad","output_token_limit":18446744073709551615u64})).unwrap();
        assert!(p.custom_temperature);
        assert_eq!(p.temperature, 0.7);
        assert_eq!(p.thinking_budget, 4096);
        assert_eq!(p.output_token_limit, 2048);
        assert!(serde_json::from_value::<Preferences>(json!([])).is_err());
    }
}
