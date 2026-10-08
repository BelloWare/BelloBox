pub mod image_ocr;
pub mod provider_setup;

use bellobox_core::ai::{Config, Provider, Request, SseDecoder, StreamEvent};
use std::io::Read;

/// Explicit runtime overrides take precedence over the user's saved settings.
/// No network access or credential persistence occurs while resolving settings.
pub fn config_from_env() -> Result<Config, String> {
    let settings = bellobox_core::settings::Settings::load(
        &bellobox_core::settings::config_dir().join("settings.json"),
    )?;
    config_from_settings(
        settings,
        std::env::var("BELLOBOX_AI_PROVIDER").ok().as_deref(),
        std::env::var("BELLOBOX_AI_ENDPOINT").ok().as_deref(),
        std::env::var("BELLOBOX_AI_MODEL").ok().as_deref(),
    )
}
/// Pure resolution: explicit runtime overrides win; persisted choices otherwise survive.
pub fn config_from_settings(
    settings: bellobox_core::settings::Settings,
    provider_override: Option<&str>,
    endpoint_override: Option<&str>,
    model_override: Option<&str>,
) -> Result<Config, String> {
    let kind = provider_override.unwrap_or(&settings.provider_kind);
    let provider = match kind {
        "openai" => Provider::OpenAIChat,
        "responses" => Provider::OpenAIResponses,
        "anthropic" => Provider::Anthropic,
        "codex" | "codexCLI" => {
            return Err(
                "Codex app-server is not ported yet. Choose a configured HTTP provider.".into(),
            );
        }
        _ => return Err("AI provider must be openai, responses, anthropic, or codex.".into()),
    };
    let endpoint = endpoint_override.map(str::to_owned).unwrap_or_else(|| {
        if kind != settings.provider_kind {
            if provider == Provider::Anthropic {
                "https://api.anthropic.com/v1".into()
            } else {
                "https://api.openai.com/v1".into()
            }
        } else {
            settings.provider_endpoint.clone()
        }
    });
    let model = model_override
        .unwrap_or(&settings.provider_model)
        .to_owned();
    if model.trim().is_empty() {
        return Err("Choose a model in Settings → AI Provider before sending.".into());
    }
    Ok(Config {
        provider,
        endpoint,
        model,
        system_prompt: settings.system_prompt,
        max_output_tokens: 4096,
    })
}
// UI status must not read a preferences file on every paint. Settings invalidates
// this bounded boolean cache after successful saves; actual sends always reload.
static READY_CACHE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);
pub fn settings_changed() {
    if let Ok(mut cached) = READY_CACHE.lock() {
        *cached = None;
    }
}
pub fn provider_is_configured() -> bool {
    let Ok(mut cached) = READY_CACHE.lock() else {
        return false;
    };
    if let Some(ready) = *cached {
        return ready;
    }
    let ready = config_from_env().is_ok_and(|config| {
        bellobox_core::ai::request(
            &config,
            &std::env::var("BELLOBOX_AI_KEY").unwrap_or_default(),
            "Configuration validation",
            "",
        )
        .is_ok()
    });
    *cached = Some(ready);
    ready
}
/// Explicit caller-owned network action. No redirects: credentials never follow a redirect.
/// Provider error bodies are not echoed because they can contain private request content.
pub fn send(request: Request, on_text: impl FnMut(&str)) -> Result<(), String> {
    send_cancellable(
        request,
        on_text,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
}
pub fn send_cancellable(
    request: Request,
    mut on_text: impl FnMut(&str),
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Request cancelled.".into());
    }
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "Cannot initialize HTTPS transport")?;
    let mut builder = client.post(request.url).json(&request.body);
    for (name, value) in request.headers {
        builder = builder.header(name, value);
    }
    let mut response = builder
        .send()
        .map_err(|_| "Provider connection failed or timed out. Check endpoint and network.")?;
    if !response.status().is_success() {
        return Err(format!(
            "Provider returned HTTP {}. No redirect was followed.",
            response.status().as_u16()
        ));
    }
    let mut decoder = SseDecoder::default();
    let mut buffer = [0_u8; 8192];
    let mut bytes = 0;
    let mut output = 0;
    let mut done = false;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Request cancelled.".into());
        }
        let n = response
            .read(&mut buffer)
            .map_err(|_| "Provider stream interrupted")?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Request cancelled.".into());
        }
        if n == 0 {
            break;
        }
        bytes += n;
        if bytes > 8_000_000 {
            return Err("Provider response exceeds 8 MB limit.".into());
        }
        for event in decoder.push(&buffer[..n])? {
            match event {
                StreamEvent::Text(text) => {
                    output += text.len();
                    on_text(&text);
                }
                StreamEvent::Done => done = true,
            }
        }
        if done {
            break;
        }
    }
    if !done {
        for event in decoder.finish()? {
            if let StreamEvent::Text(text) = event {
                output += text.len();
                on_text(&text);
            }
        }
    }
    if output == 0 {
        return Err("Provider returned no visible text.".into());
    }
    Ok(())
}
