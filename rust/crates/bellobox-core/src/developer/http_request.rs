//! Pure, bounded HTTP editor/import and response presentation. No I/O, shell,
//! environment expansion, cookie storage or request-history persistence occurs.
//!
//! This is a portable model of HTTPRequestTool.swift, not Foundation/wire parity.
//! Rust retains its 500,000-byte admission rather than Swift's 512,000 bytes and
//! additionally bounds aggregate drafts, headers, method length and token count.
//! URL parsing uses `url`; raw whitespace, backslashes, invalid percent escapes
//! and URL userinfo are rejected rather than silently rewritten/authenticated.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{fmt, time::Duration};

pub const MAX_DRAFT_BYTES: usize = 500_000;
pub const MAX_HEADER_BYTES: usize = 64_000;
pub const MAX_HEADER_COUNT: usize = 256;
pub const MAX_METHOD_BYTES: usize = 64;
pub const MAX_IMPORT_TOKENS: usize = 4_096;
pub const MAX_RESPONSE_BYTES: usize = 2_000_000;
pub const MAX_RESPONSE_TEXT_BYTES: usize = MAX_RESPONSE_BYTES + MAX_HEADER_BYTES + 512;
type Result<T> = std::result::Result<T, String>;

/// Editable, memory-only fields. Debug deliberately omits all request content.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequestDraft {
    pub method: String,
    pub url: String,
    pub headers: String,
    pub body: String,
}
impl Default for HttpRequestDraft {
    fn default() -> Self {
        Self {
            method: "GET".into(),
            url: String::new(),
            headers: String::new(),
            body: String::new(),
        }
    }
}
impl fmt::Debug for HttpRequestDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpRequestDraft")
            .field("bytes", &self.byte_len())
            .finish_non_exhaustive()
    }
}
impl HttpRequestDraft {
    pub fn byte_len(&self) -> usize {
        self.method
            .len()
            .saturating_add(self.url.len())
            .saturating_add(self.headers.len())
            .saturating_add(self.body.len())
    }

    /// Validate once and freeze the exact edited fields for an explicit Send.
    /// The transport must use these fields without adding implicit credentials,
    /// following redirects or accepting framing from the original draft.
    pub fn prepare(&self) -> Result<PreparedHttpRequest> {
        if self.byte_len() > MAX_DRAFT_BYTES {
            return Err("Request fields exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
        }
        let method = self.method.trim().to_uppercase();
        if method.is_empty()
            || method.len() > MAX_METHOD_BYTES
            || !method.bytes().all(|b| b.is_ascii_uppercase() || b == b'-')
        {
            return Err("Enter a valid HTTP method (up to 64 ASCII letters or hyphens).".into());
        }
        let url = validate_url(&self.url)?;
        if self.headers.len() > MAX_HEADER_BYTES {
            return Err("Request headers exceed 64,000 UTF-8 bytes.".into());
        }
        let mut headers = Vec::new();
        let mut count = 0;
        for line in self.headers.split('\n') {
            if line.trim_matches([' ', '\t']).is_empty() {
                continue;
            }
            count += 1;
            if count > MAX_HEADER_COUNT {
                return Err("Use at most 256 request headers.".into());
            }
            if line.contains('\r') {
                return Err("Use one header per line: Name: value.".into());
            }
            let (name, value) = line
                .split_once(':')
                .ok_or("Use one header per line: Name: value.")?;
            let name = name.trim_matches([' ', '\t']);
            let value = value.trim_matches([' ', '\t']);
            validate_header(name, value)?;
            // The body editor is authoritative; never replay pasted framing.
            if name.eq_ignore_ascii_case("content-length")
                || name.eq_ignore_ascii_case("transfer-encoding")
            {
                continue;
            }
            headers.push((name.to_owned(), value.to_owned()));
        }
        let prepared_bytes = headers.iter().fold(
            method
                .len()
                .saturating_add(url.len())
                .saturating_add(self.body.len()),
            |n, (k, v)| {
                n.saturating_add(k.len())
                    .saturating_add(v.len())
                    .saturating_add(3)
            },
        );
        if prepared_bytes > MAX_DRAFT_BYTES {
            return Err(
                "Prepared request exceeds 500,000 UTF-8 bytes after URL/header normalization."
                    .into(),
            );
        }
        Ok(PreparedHttpRequest {
            method,
            url,
            headers,
            body: self.body.as_bytes().to_vec(),
        })
    }
}

/// An immutable owned snapshot; no Debug or serialization exposes its contents.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedHttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
impl PreparedHttpRequest {
    pub fn method(&self) -> &str {
        &self.method
    }
    /// Canonical URL from the portable URL parser, including any fragment.
    /// Fragments are never sent on the wire by the transport.
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}
impl fmt::Debug for PreparedHttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedHttpRequest")
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

fn validate_header(name: &str, value: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
    {
        return Err("Invalid HTTP header name.".into());
    }
    if !value
        .chars()
        .all(|c| (c >= ' ' && c != '\u{7f}') || c == '\t')
    {
        return Err("HTTP header values cannot contain control characters.".into());
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<String> {
    const ERROR: &str = "Enter a complete http:// or https:// request URL.";
    if value
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
        || !valid_percent_escapes(value)
    {
        return Err("Request URLs cannot contain whitespace, controls, backslashes or invalid percent escapes.".into());
    }
    let parsed = url::Url::parse(value).map_err(|_| ERROR)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none_or(str::is_empty)
        || !value
            .split_once(':')
            .is_some_and(|(_, tail)| tail.starts_with("//"))
    {
        return Err(ERROR.into());
    }
    // Even empty userinfo is rejected; there is no implicit URL authentication.
    let authority = value
        .split_once("://")
        .map(|(_, tail)| tail.split(['/', '?', '#']).next().unwrap_or(""))
        .unwrap_or("");
    if authority.is_empty() {
        return Err(ERROR.into());
    }
    if authority.contains('@') || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(
            "URL credentials are not imported. Use an explicit Authorization header.".into(),
        );
    }
    Ok(parsed.into())
}

fn valid_percent_escapes(value: &str) -> bool {
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%'
            && (!bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|b| b.is_ascii_hexdigit()))
        {
            return false;
        }
    }
    true
}

/// Import a literal URL or cURL command. Importing never sends anything.
pub fn import_request(input: &str) -> Result<HttpRequestDraft> {
    if input.len() > MAX_DRAFT_BYTES {
        return Err("Import exceeds 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    let input = input.trim();
    if input.starts_with("curl") {
        import_curl(input)
    } else {
        let draft = HttpRequestDraft {
            url: input.into(),
            ..Default::default()
        };
        draft.prepare()?;
        Ok(draft)
    }
}

fn take_value(tokens: &[String], index: &mut usize, attached: Option<String>) -> Result<String> {
    if let Some(value) = attached {
        return Ok(value);
    }
    let value = tokens
        .get(*index)
        .ok_or("Missing value for a cURL option.")?
        .clone();
    *index += 1;
    Ok(value)
}

fn import_curl(command: &str) -> Result<HttpRequestDraft> {
    let tokens = tokenize_curl(command)?;
    if tokens.first().map(String::as_str) != Some("curl") {
        return Err("Paste a cURL command beginning with curl.".into());
    }
    let mut draft = HttpRequestDraft::default();
    let (mut index, mut explicit_method, mut get, mut json) = (1, false, false, false);
    let (mut target, mut headers, mut bodies) = (None, Vec::<String>::new(), Vec::<String>::new());
    while index < tokens.len() {
        let token = &tokens[index];
        index += 1;
        // Attached options are decoded only at option position. Values such as
        // '-dhello' or '--json=literal' remain unchanged when consumed as data.
        let (option, attached) = split_option(token);
        match option {
            "-X" | "--request" => {
                draft.method = take_value(&tokens, &mut index, attached)?.to_uppercase();
                explicit_method = true;
            }
            "-H" | "--header" => {
                let value = take_value(&tokens, &mut index, attached)?;
                if value.contains(['\r', '\n']) {
                    return Err("A header cannot contain a newline.".into());
                }
                headers.push(value);
            }
            "-d" | "--data" | "--data-binary" | "--data-raw" | "--json" => {
                let value = take_value(&tokens, &mut index, attached)?;
                if option != "--data-raw" && value.starts_with('@') {
                    return Err(
                        "File uploads are not imported. Paste the body into the request editor."
                            .into(),
                    );
                }
                if (json && option != "--json")
                    || (option == "--json" && !json && !bodies.is_empty())
                {
                    return Err(
                        "Use either JSON data or form data in a single imported request.".into(),
                    );
                }
                bodies.push(value);
                if option == "--json" {
                    json = true;
                }
            }
            "--url" => {
                if target.is_some() {
                    return Err("Import one request URL at a time.".into());
                }
                target = Some(take_value(&tokens, &mut index, attached)?);
            }
            "-G" | "--get" => get = true,
            "-I" | "--head" => {
                draft.method = "HEAD".into();
                explicit_method = true;
            }
            "--compressed" | "-s" | "--silent" | "-S" | "--show-error" | "--globoff" | "-g" => {}
            "-b" | "--cookie" => {
                let value = take_value(&tokens, &mut index, attached)?;
                if !value.contains('=') || value.contains(['\r', '\n']) {
                    return Err(
                        "Import inline cookies as name=value; cookie files are not read.".into(),
                    );
                }
                headers.push(format!("Cookie: {value}"));
            }
            "-u" | "--user" => {
                let value = take_value(&tokens, &mut index, attached)?;
                headers.push(format!(
                    "Authorization: Basic {}",
                    STANDARD.encode(value.as_bytes())
                ));
            }
            _ => {
                // Do not echo an unsupported token: it could itself be a secret.
                if token.starts_with('-') {
                    return Err("Unsupported cURL option. Remove it or configure the request in the editor.".into());
                }
                if target.is_some() {
                    return Err("Import one request URL at a time.".into());
                }
                target = Some(token.clone());
            }
        }
    }
    draft.url = target.ok_or("The cURL command has no URL.")?;
    draft.body = bodies.join(if json { "" } else { "&" });
    if !bodies.is_empty() {
        if get {
            draft.url = append_get_data(&draft.url, &draft.body)?;
            draft.body.clear();
        } else if !explicit_method {
            draft.method = "POST".into();
        }
        if !get && !has_imported_header(&headers, "content-type:") {
            headers.push(
                if json {
                    "Content-Type: application/json"
                } else {
                    "Content-Type: application/x-www-form-urlencoded"
                }
                .into(),
            );
        }
    }
    if json && !has_imported_header(&headers, "accept:") {
        headers.push("Accept: application/json".into());
    }
    draft.headers = headers.join("\n");
    draft.prepare()?;
    Ok(draft)
}

fn has_imported_header(headers: &[String], prefix: &str) -> bool {
    // Match the source's literal prefix check, before edited-header trimming.
    headers.iter().any(|line| {
        line.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    })
}

fn split_option(token: &str) -> (&str, Option<String>) {
    const LONG: &[&str] = &[
        "--request",
        "--header",
        "--data",
        "--data-binary",
        "--data-raw",
        "--json",
        "--url",
        "--user",
        "--cookie",
    ];
    if let Some((name, value)) = token.split_once('=')
        && LONG.contains(&name)
    {
        return (name, Some(value.into()));
    }
    for short in ["-X", "-H", "-d", "-u", "-b"] {
        if let Some(value) = token.strip_prefix(short)
            && !value.is_empty()
        {
            return (short, Some(value.into()));
        }
    }
    (token, None)
}

fn append_get_data(url: &str, body: &str) -> Result<String> {
    validate_url(url)?;
    let (before_fragment, fragment) = url
        .split_once('#')
        .map_or((url, None), |(a, b)| (a, Some(b)));
    let (base, existing) = before_fragment
        .split_once('?')
        .unwrap_or((before_fragment, ""));
    let query = [existing, body]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("&");
    if !valid_percent_escapes(&query)
        || percent_encoding::percent_decode_str(&query)
            .decode_utf8()
            .is_err()
        || !query
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?%".contains(&b))
    {
        return Err("GET data must be URL-encoded before importing.".into());
    }
    Ok(format!(
        "{base}?{query}{}",
        fragment.map_or(String::new(), |f| format!("#{f}"))
    ))
}

/// A small literal shell tokenizer, not a shell interpreter. Single quotes and
/// explicitly escaped dollars/backticks stay literal; unescaped expansion and
/// shell operators outside quotes are rejected. No @file is ever opened.
pub fn tokenize_curl(command: &str) -> Result<Vec<String>> {
    if command.len() > MAX_DRAFT_BYTES {
        return Err("Import exceeds 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    if command
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err("cURL import contains an unsupported control character.".into());
    }
    let (mut tokens, mut token, mut quote, mut started) = (Vec::new(), String::new(), None, false);
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && quote != Some('\'') {
            let next = chars
                .next()
                .ok_or("The cURL command has an unclosed quote or trailing escape.")?;
            if next == '\n' {
                continue;
            }
            if next == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
                continue;
            }
            if quote == Some('"') && !"$`\"\\".contains(next) {
                token.push('\\');
            }
            token.push(next);
            started = true;
            continue;
        }
        if let Some(q) = quote {
            if q == '"' && matches!(c, '$' | '`') {
                return Err("Shell expansion is not imported. Use single quotes or escape the literal character.".into());
            }
            if c == q {
                quote = None;
            } else {
                token.push(c);
            }
            started = true;
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
            started = true;
        } else if c.is_whitespace() {
            if started {
                push_token(&mut tokens, &mut token)?;
                started = false;
            }
        } else {
            if ";|&<>`$".contains(c) {
                return Err("Shell operators and expansion are not imported. Quote URLs and use literal request values.".into());
            }
            token.push(c);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("The cURL command has an unclosed quote or trailing escape.".into());
    }
    if started {
        push_token(&mut tokens, &mut token)?;
    }
    Ok(tokens)
}
fn push_token(tokens: &mut Vec<String>, token: &mut String) -> Result<()> {
    if tokens.len() >= MAX_IMPORT_TOKENS {
        return Err("cURL import exceeds 4,096 literal tokens.".into());
    }
    tokens.push(std::mem::take(token));
    Ok(())
}

/// Retains only the response prefix. `truncated` becomes true only after data
/// beyond the exact two-million-byte cap is observed; retained_len is never a
/// claim about the full response length. Transport should stop after truncation.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ResponseBodyPreview {
    bytes: Vec<u8>,
    truncated: bool,
}
impl ResponseBodyPreview {
    pub fn new() -> Self {
        Self::default()
    }
    /// Returns true once the caller should stop reading (an extra byte arrived).
    pub fn push(&mut self, chunk: &[u8]) -> bool {
        let keep = chunk.len().min(MAX_RESPONSE_BYTES - self.bytes.len());
        if self.bytes.len() + keep > self.bytes.capacity() {
            // Geometric growth avoids reallocating for each small stream chunk;
            // clamp the requested capacity so Vec's usual doubling cannot keep
            // a four-megabyte allocation for a two-megabyte retained prefix.
            let capacity = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(64)
                .max(self.bytes.len() + keep)
                .min(MAX_RESPONSE_BYTES);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(&chunk[..keep]);
        self.truncated |= keep < chunk.len();
        self.truncated
    }
    pub fn retained_len(&self) -> usize {
        self.bytes.len()
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
impl fmt::Debug for ResponseBodyPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseBodyPreview")
            .field("retained_bytes", &self.bytes.len())
            .field("truncated", &self.truncated)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpInspectionResult {
    status: String,
    headers: String,
    body: String,
    retained_len: usize,
    truncated: bool,
}
impl HttpInspectionResult {
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn headers(&self) -> &str {
        &self.headers
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    pub fn retained_len(&self) -> usize {
        self.retained_len
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    /// Complete bounded copy text, not a rendered viewport prefix.
    pub fn text(&self) -> String {
        format!("{}\n\n{}\n\n{}", self.status, self.headers, self.body)
    }
}
impl fmt::Debug for HttpInspectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpInspectionResult")
            .field("retained_bytes", &self.retained_len)
            .field("truncated", &self.truncated)
            .finish_non_exhaustive()
    }
}

pub fn render_response(
    status: u16,
    elapsed: Duration,
    headers: &[(String, String)],
    preview: &ResponseBodyPreview,
) -> Result<HttpInspectionResult> {
    if !(100..=999).contains(&status) {
        return Err("The server did not return a valid three-digit HTTP status.".into());
    }
    if headers.len() > MAX_HEADER_COUNT {
        return Err("Response exceeds 256 headers.".into());
    }
    let mut header_bytes = 0usize;
    for (name, value) in headers {
        validate_header(name, value)?;
        header_bytes = header_bytes
            .saturating_add(name.len())
            .saturating_add(value.len())
            .saturating_add(3);
        if header_bytes > MAX_HEADER_BYTES {
            return Err("Response headers exceed 64,000 UTF-8 bytes.".into());
        }
    }
    let mut lines = headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>();
    lines.sort();
    let headers = lines.join("\n");
    let body = match std::str::from_utf8(preview.bytes()) {
        Ok(original) if !preview.truncated() => super::json_formatter::execute(original, "")
            .ok()
            .filter(|s| s.len() <= MAX_RESPONSE_BYTES)
            .unwrap_or_else(|| original.to_owned()),
        Ok(original) => original.to_owned(),
        Err(_) => format!(
            "Binary response ({} bytes).",
            grouped(preview.retained_len())
        ),
    };
    // Deterministic English grouping and nearest whole millisecond. This is not
    // a claim of every native locale or Foundation rounding-mode equivalence.
    let millis = elapsed
        .as_millis()
        .saturating_add(u128::from(elapsed.subsec_nanos() % 1_000_000 >= 500_000));
    let status = format!(
        "HTTP {status} · {millis} ms · {} bytes{}",
        grouped(preview.retained_len()),
        if preview.truncated() {
            " · preview limited to 2 MB"
        } else {
            ""
        }
    );
    if status
        .len()
        .saturating_add(headers.len())
        .saturating_add(body.len())
        .saturating_add(4)
        > MAX_RESPONSE_TEXT_BYTES
    {
        return Err("Response presentation exceeds its bounded text budget.".into());
    }
    Ok(HttpInspectionResult {
        status,
        headers,
        body,
        retained_len: preview.retained_len(),
        truncated: preview.truncated(),
    })
}
fn grouped(value: usize) -> String {
    let raw = value.to_string();
    let mut out = String::new();
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && (raw.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "http_request_tests.rs"]
mod tests;
