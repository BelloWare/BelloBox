use crate::ai::Provider;
use serde_json::Value;

pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_OCR_TEXT_BYTES: usize = 1024 * 1024;
const MAX_WARNING_COUNT: usize = 32;
const MAX_WARNING_BYTES: usize = 4 * 1024;
const MAX_TOTAL_WARNING_BYTES: usize = 32 * 1024;
const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_CONTAINERS: usize = 16_384;

/// Provider text is always literal data. Never render it as HTML, execute it,
/// follow its URLs, or include it in diagnostics. No Debug implementation.
pub struct OcrResult {
    pub plain_text: String,
    pub markdown_text: Option<String>,
    pub warnings: Vec<String>,
}
fn validate_json_structure(bytes: &[u8]) -> Result<(), String> {
    // Check nesting *before* recursive parsing/destruction. The enclosing byte
    // cap bounds scalars; this pass also bounds arrays/objects, including ignored
    // fields. It is deliberately independent of serde's larger recursion limit.
    let mut depth = 0usize;
    let mut containers = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for &b in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    containers += 1;
                    if depth > MAX_JSON_DEPTH || containers > MAX_JSON_CONTAINERS {
                        return Err("AI OCR JSON exceeds its structure limit.".into());
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}
fn checked_json(bytes: &[u8]) -> Result<Value, String> {
    validate_json_structure(bytes)?;
    serde_json::from_slice(bytes).map_err(|_| "Provider returned malformed AI OCR JSON.".into())
}
fn push_text(output: &mut String, text: &str) -> Result<(), String> {
    let separator = usize::from(!output.is_empty());
    if output
        .len()
        .saturating_add(text.len())
        .saturating_add(separator)
        > MAX_OCR_TEXT_BYTES
    {
        return Err("AI OCR response text exceeds its limit.".into());
    }
    if separator != 0 {
        output.push('\n');
    }
    output.push_str(text);
    Ok(())
}
/// Parse the provider-specific envelope first. Thinking/reasoning blocks and
/// unrelated text fields can never become OCR output. HTTP error bodies are
/// rejected by transport without reaching this parser.
pub fn parse_response(provider: Provider, bytes: &[u8]) -> Result<OcrResult, String> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("AI OCR response exceeds 2 MiB.".into());
    }
    let envelope = checked_json(bytes)?;
    if envelope.get("error").is_some() {
        return Err("Provider reported an AI OCR error.".into());
    }
    let mut text = String::new();
    match provider {
        Provider::OpenAIChat => {
            if let Some(choice) = envelope
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|v| v.first())
            {
                if choice["finish_reason"] == "length"
                    || choice["finish_reason"] == "content_filter"
                {
                    return Err("AI OCR response was incomplete or blocked.".into());
                }
                if let Some(value) = choice["message"]["content"].as_str() {
                    push_text(&mut text, value)?;
                }
            }
        }
        Provider::OpenAIResponses => {
            if envelope["status"] == "incomplete" || envelope["status"] == "failed" {
                return Err("AI OCR response was incomplete or failed.".into());
            }
            if let Some(value) = envelope["output_text"].as_str() {
                push_text(&mut text, value)?;
            } else if let Some(items) = envelope["output"].as_array() {
                for item in items {
                    if item["type"] != "message" {
                        continue;
                    }
                    if let Some(parts) = item["content"].as_array() {
                        for part in parts {
                            if part["type"] == "output_text"
                                && let Some(value) = part["text"].as_str()
                            {
                                push_text(&mut text, value)?;
                            }
                        }
                    }
                }
            }
        }
        Provider::Anthropic => {
            if envelope["stop_reason"] == "max_tokens" {
                return Err(
                    "AI OCR output limit was reached. Increase the output limit and retry.".into(),
                );
            }
            if let Some(parts) = envelope["content"].as_array() {
                for part in parts {
                    if part["type"] == "text"
                        && let Some(value) = part["text"].as_str()
                    {
                        push_text(&mut text, value)?;
                    }
                }
            }
        }
    }
    parse_ocr_text(&text)
}
fn optional_text(object: &Value, key: &str) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.len() <= MAX_OUTPUT_BYTES => Ok(Some(value.clone())),
        Some(Value::String(_)) => Err("AI OCR output exceeds its text limit.".into()),
        _ => Err("AI OCR returned an invalid text field.".into()),
    }
}
pub fn parse_ocr_text(text: &str) -> Result<OcrResult, String> {
    if text.len() > MAX_OCR_TEXT_BYTES {
        return Err("AI OCR response text exceeds its limit.".into());
    }
    let mut text = text.trim();
    if text.is_empty() {
        return Err("Provider returned no visible AI OCR text.".into());
    }
    // A single fenced JSON wrapper is supported without recursive reparsing.
    if text.starts_with("```")
        && let Some((_, body)) = text.split_once('\n')
    {
        text = body.trim();
        if let Some(body) = text.strip_suffix("```") {
            text = body.trim();
        }
    }
    if text.is_empty() {
        return Err("Provider returned no visible AI OCR text.".into());
    }
    // Budget failures remain errors. Ordinary malformed/nonobject JSON falls
    // back to literal text, including source-shaped “[unclear]” transcriptions.
    let object = if text.starts_with('{') || text.starts_with('[') {
        validate_json_structure(text.as_bytes())?;
        serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_object)
    } else {
        None
    };
    let result = if let Some(object) = object {
        let plain = optional_text(&object, "plainText")?
            .or(optional_text(&object, "text")?)
            .unwrap_or_default();
        let markdown = optional_text(&object, "markdownText")?;
        let mut warnings = Vec::new();
        let mut total = 0usize;
        match object.get("warnings") {
            None | Some(Value::Null) => {}
            Some(Value::Array(values)) if values.len() <= MAX_WARNING_COUNT => {
                for value in values {
                    let warning = value
                        .as_str()
                        .ok_or("AI OCR returned an invalid warning field.")?;
                    total = total.saturating_add(warning.len());
                    if warning.len() > MAX_WARNING_BYTES || total > MAX_TOTAL_WARNING_BYTES {
                        return Err("AI OCR warnings exceed their size limit.".into());
                    }
                    warnings.push(warning.into());
                }
            }
            _ => {
                return Err(
                    "AI OCR warnings exceed their count limit or have invalid structure.".into(),
                );
            }
        }
        let plain = if plain.trim().is_empty() {
            markdown.clone().unwrap_or_default()
        } else {
            plain
        };
        OcrResult {
            plain_text: plain,
            markdown_text: markdown,
            warnings,
        }
    } else {
        if text.len() > MAX_OUTPUT_BYTES {
            return Err("AI OCR output exceeds its text limit.".into());
        }
        OcrResult {
            plain_text: text.into(),
            markdown_text: None,
            warnings: vec!["Provider returned non-JSON OCR text.".into()],
        }
    };
    if result.plain_text.trim().is_empty() {
        return Err("Provider returned no visible AI OCR text.".into());
    }
    Ok(result)
}
