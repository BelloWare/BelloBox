//! Pure, bounded requests for explicit provider setup actions. No environment or I/O.
use super::{Config, Provider, Request};
use std::collections::BTreeSet;
use url::Url;

pub const MAX_MODELS_BYTES: usize = 1_048_576;
pub const MAX_MODELS: usize = 4096;
pub const MAX_MODEL_ID_BYTES: usize = 256;
pub const TEST_USER_TEXT: &str = "Reply with a short, friendly hello.";

/// No Debug implementation: headers can contain credentials.
pub struct ModelsRequest {
    pub url: Url,
    pub headers: Vec<(String, String)>,
}

pub fn models_request(
    provider: Provider,
    endpoint: &str,
    key: &str,
) -> Result<ModelsRequest, String> {
    if endpoint.len() > 8192 {
        return Err("AI endpoint exceeds 8,192 bytes.".into());
    }
    if key.chars().any(char::is_control) {
        return Err("Credential contains invalid header characters.".into());
    }
    // Keep the same URL security policy as the ordinary text request builder.
    let mut url = Url::parse(endpoint.trim()).map_err(|_| "Invalid AI endpoint.")?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Endpoint must be an HTTP(S) URL without embedded credentials, query, or fragment."
                .into(),
        );
    }
    let path = url.path().trim_end_matches('/');
    if !path.ends_with("/models") {
        url.set_path(&format!("{path}/models"));
    }
    let mut headers = Vec::new();
    match provider {
        Provider::OpenAIChat | Provider::OpenAIResponses => {
            let key = key.trim();
            if !key.is_empty() {
                headers.push(("authorization".into(), format!("Bearer {key}")));
            }
        }
        Provider::Anthropic => {
            if key.trim().is_empty() {
                return Err("Anthropic requires an API key in the runtime environment.".into());
            }
            headers.push(("x-api-key".into(), key.into()));
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
        }
    }
    Ok(ModelsRequest { url, headers })
}

pub fn parse_models(bytes: &[u8]) -> Result<Vec<String>, String> {
    if bytes.len() > MAX_MODELS_BYTES {
        return Err("Model list exceeds 1 MiB. Enter a model manually.".into());
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "Invalid model list JSON. Enter a model manually.")?;
    let entries = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or("Unsupported model list format. Enter a model manually.")?;
    if entries.len() > MAX_MODELS {
        return Err("Model list exceeds 4,096 entries. Enter a model manually.".into());
    }
    let mut ids = BTreeSet::new();
    for entry in entries {
        if let Some(id) = entry.get("id").and_then(serde_json::Value::as_str) {
            if id.len() > MAX_MODEL_ID_BYTES || id.chars().any(char::is_control) {
                return Err(
                    "Model list contains an invalid model ID. Enter a model manually.".into(),
                );
            }
            if !id.trim().is_empty() {
                ids.insert(id.to_owned());
            }
        }
    }
    Ok(ids.into_iter().collect())
}

/// Reuse ordinary provider defaults and validation, replacing only the user input.
/// The connection probe must never send the selected-text transformation wrapper.
pub fn test_request(config: &Config, key: &str) -> Result<Request, String> {
    if config.endpoint.len() > 8192
        || config.model.len() > MAX_MODEL_ID_BYTES
        || config.system_prompt.len() > 32_000
        || key.chars().any(char::is_control)
    {
        return Err(
            "Provider configuration exceeds text limits or has an invalid credential.".into(),
        );
    }
    let mut request = super::request(config, key, TEST_USER_TEXT, "")?;
    match config.provider {
        Provider::OpenAIChat => request.body["messages"][1]["content"] = TEST_USER_TEXT.into(),
        Provider::OpenAIResponses => request.body["input"] = TEST_USER_TEXT.into(),
        Provider::Anthropic => request.body["messages"][0]["content"] = TEST_USER_TEXT.into(),
    }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_paths_and_headers() {
        for provider in [Provider::OpenAIChat, Provider::OpenAIResponses] {
            let r = models_request(provider, " http://127.0.0.1:1/v1/// ", "").unwrap();
            assert_eq!(r.url.as_str(), "http://127.0.0.1:1/v1/models");
            assert!(r.headers.is_empty());
            let r = models_request(provider, "http://127.0.0.1:1/v1/models/", "synthetic").unwrap();
            assert_eq!(r.url.path(), "/v1/models/");
            assert_eq!(
                r.headers,
                [("authorization".into(), "Bearer synthetic".into())]
            );
        }
        let r = models_request(Provider::Anthropic, "http://127.0.0.1:1/v1", "synthetic").unwrap();
        assert_eq!(
            r.headers,
            [
                ("x-api-key".into(), "synthetic".into()),
                ("anthropic-version".into(), "2023-06-01".into())
            ]
        );
        assert!(models_request(Provider::Anthropic, "http://127.0.0.1:1", "").is_err());
    }
    #[test]
    fn rejects_unsafe_urls_and_headers() {
        for endpoint in [
            "file:///tmp/x",
            "https://user:pass@127.0.0.1",
            "https://127.0.0.1?q=x",
            "https://127.0.0.1#x",
            "bad",
        ] {
            assert!(models_request(Provider::OpenAIChat, endpoint, "").is_err());
        }
        for key in ["synthetic\rfoo", "synthetic\nfoo", "synthetic\0foo"] {
            assert!(models_request(Provider::OpenAIChat, "http://127.0.0.1", key).is_err());
        }
    }
    #[test]
    fn parses_sorted_unique_ids_and_bounds() {
        assert_eq!(
            parse_models(
                br#"{"data":[{"id":"z"},{"id":"a"},{"id":"z"},{"name":"skip"},{"id":""}]}"#
            )
            .unwrap(),
            ["a", "z"]
        );
        for data in [b"{".as_slice(), b"{}"] {
            assert!(parse_models(data).unwrap_err().contains("manually"));
        }
        assert!(parse_models(br#"{"data":[]}"#).unwrap().is_empty());
        assert!(parse_models(&vec![b' '; MAX_MODELS_BYTES + 1]).is_err());
        assert!(
            parse_models(
                &serde_json::to_vec(
                    &serde_json::json!({"data":vec![serde_json::json!({"id":"x"});MAX_MODELS+1]})
                )
                .unwrap()
            )
            .is_err()
        );
        assert!(
            parse_models(
                &serde_json::to_vec(
                    &serde_json::json!({"data":[{"id":"x".repeat(MAX_MODEL_ID_BYTES+1)}]})
                )
                .unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn test_uses_exact_input_and_existing_defaults_for_all_providers() {
        for provider in [
            Provider::OpenAIChat,
            Provider::OpenAIResponses,
            Provider::Anthropic,
        ] {
            let c = Config {
                provider,
                endpoint: "http://127.0.0.1:1/v1".into(),
                model: "synthetic-model".into(),
                system_prompt: "test system".into(),
                max_output_tokens: 4096,
            };
            let r = test_request(&c, "synthetic").unwrap();
            assert_eq!(r.body["stream"], true);
            assert!(!r.body.to_string().contains("selected_text"));
            assert!(r.body.get("temperature").is_none());
            assert!(r.body.get("reasoning").is_none());
            match provider {
                Provider::OpenAIChat => {
                    assert_eq!(r.body["messages"][1]["content"], TEST_USER_TEXT);
                    assert_eq!(r.body["max_completion_tokens"], 4096);
                }
                Provider::OpenAIResponses => {
                    assert_eq!(r.body["input"], TEST_USER_TEXT);
                    assert_eq!(r.body["max_output_tokens"], 4096);
                }
                Provider::Anthropic => {
                    assert_eq!(r.body["messages"][0]["content"], TEST_USER_TEXT);
                    assert_eq!(r.body["max_tokens"], 4096);
                }
            }
        }
    }
}
