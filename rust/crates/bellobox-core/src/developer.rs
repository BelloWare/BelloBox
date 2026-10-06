//! Bounded, offline developer tools. No engine performs network requests, executes
//! generated code, touches the clipboard, or stores input (including HMAC keys).
//!
//! `second` is the secondary document/key, or a documented command/option string.
//! Unknown options fail explicitly. These portable text engines do not claim all
//! of the controls, visual previews, or dialects of the native Swift workbench.
use base64::{Engine as _, engine::general_purpose as b64};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use hmac::{Hmac, Mac};
use regex::RegexBuilder;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[path = "developer/certificate.rs"]
mod certificate;
#[path = "developer/json_schema.rs"]
mod json_schema;
#[path = "developer/list_set.rs"]
pub mod list_set;
#[path = "developer/plist.rs"]
mod plist_engine;
#[path = "developer/sql_formatter.rs"]
mod sql_formatter;

const MAX_INPUT: usize = 500_000;
const MAX_OUTPUT: usize = 4_000_000;
const MAX_NODES: usize = 20_000;
const MAX_DEPTH: usize = 64;
type R<T> = Result<T, String>;

#[derive(Clone, Copy, Debug)]
pub struct Tool {
    pub id: &'static str,
    pub title: &'static str,
    pub example: &'static str,
}

pub fn catalog() -> Vec<Tool> {
    [
        ("json", "JSON Formatter", r#"{"id":9007199254740993,"name":"Ada"}"#),
        ("compare", "Text Comparison", "alpha\nbeta\ngamma"),
        ("jwt", "JWT Inspector", "eyJhbGciOiJub25lIn0.eyJzdWIiOiJBZGEiLCJleHAiOjE3MDAwMDAwMDB9."),
        ("regex", "Regex Tester", "BOX-12 API-34"),
        ("url", "URL & Query Inspector", "https://example.com:8443/a?q=a+b&tag=one&tag=two&flag#hello"),
        ("time", "Timestamp Converter", "1700000000000"),
        ("cron", "Cron Inspector", "*/15 9-17 * * MON-FRI"),
        ("convert", "JSON / YAML / CSV Converter", "name,note\nAda,Hello\nLin,World"),
        ("snippets", "Snippet Template", "Hello {{name}}: {{selection}}"),
        ("http", "HTTP Request Inspector", "GET https://example.com/api HTTP/1.1\nAccept: application/json"),
        ("generate", "Local Generators", "uuid 3"),
        ("calculator", "Calculator", "sqrt(144) + 2^3"),
        ("units", "Unit Converter", "1024 KiB"),
        ("numberBase", "Number Base Converter", "9007199254740993"),
        ("color", "Color Converter", "#B84D16"),
        ("contrast", "Contrast Checker", "#B84D16"),
        ("gradient", "CSS Gradient Builder", "#B84D16"),
        ("markdown", "Safe Markdown to HTML", "# BelloBox\n\n**Offline** tools.\n\n- Local\n- Private"),
        ("jsonPointer", "JSON Pointer", r#"{"users":[{"name":"Ada"}]}"#),
        ("jsonFlatten", "Flatten & Unflatten JSON", r#"{"user":{"name":"Ada"},"empty":[],"active":true}"#),
        ("jsonCode", "JSON to TypeScript", r#"[{"id":1,"name":"Ada"},{"id":2,"active":true}]"#),
        ("sqlInsert", "SQL INSERT Builder", r#"[{"id":1,"name":"O'Reilly"},{"id":2,"name":"Ada"}]"#),
        ("xmlJSON", "XML to Ordered JSON", "<note priority=\"high\">Hello <b>Ada</b>!</note>"),
        ("unicode", "Unicode Inspector", "Hello 👩🏽‍💻 café e\u{301}"),
        ("stringEscape", "String Literal Escaper", "Hello \"BelloBox\"\nA second line"),
        ("extract", "Extract Links & Emails", "Visit https://example.com or email hello@example.com."),
        ("listSet", "List Set Operations", "Swift\nRust\nPython"),
        ("semver", "Semantic Versions", "1.0.0\n1.0.0-rc.2\n1.0.0-rc.10\n2.0.0"),
        ("subnet", "IPv4 Subnet Calculator", "192.168.1.42/24"),
        ("chmod", "Chmod Permissions", "755"),
        ("hmac", "HMAC-SHA256 Signer", "what do ya want for nothing?"),
        ("jsonSchema", "JSON Schema Validator (Unavailable)", r#"{"name":"Ada","age":37}"#),
        ("jsonMerge", "JSON Merge Patch", r#"{"name":"Ada","profile":{"city":"London","old":true}}"#),
        ("jsonRedact", "JSON Field Redactor", r#"{"user":{"email":"ada@example.com"},"token":"example","active":true}"#),
        ("jsonLines", "JSON Lines", "{\"id\":1}\n{\"id\":2}"),
        ("csvExplore", "CSV Explorer", "name,team\nAda,Platform\nLin,Design\nAda,Platform"),
        ("envFile", "Environment File", "APP_NAME=\"Bello Box\"\nPORT=8080\nGREETING='Hello ${USER}'"),
        ("plist", "Property List Converter (Unavailable)", "<plist version=\"1.0\"><dict><key>Name</key><string>BelloBox</string><key>Enabled</key><true/></dict></plist>"),
        ("sqlFormat", "SQL Formatter (Unavailable)", "select name, count(*) from users where active=true group by name;"),
        ("httpHeaders", "HTTP Header Inspector", "HTTP/1.1 200 OK\nContent-Type: application/json\nSet-Cookie: theme=dark\nSet-Cookie: locale=en"),
        ("cookies", "Cookie Inspector", "session=example; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=3600"),
        ("certificate", "Certificate Inspector", certificate::EXAMPLE),
        ("sshKey", "SSH Public Key Inspector", "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIIjHzQuE4mVVNZ33vbXM4iAWWOlxbTI01lJAojWyBLpg example@belloware"),
        ("uuidInspect", "UUID Inspector", "017f22e2-79b0-7cc3-98c4-dc0c0c07398f"),
        ("bitwise", "64-bit Bitwise Calculator", "0b10101010"),
        ("statistics", "Statistics Calculator", "12, 18, 18, 21, 24, 27, 32, 36, 42, 51"),
        ("dateMath", "Date Calculator", "2026-09-12"),
        ("aspectRatio", "Aspect Ratio Calculator", "1920 x 1080"),
        ("bezier", "CSS Bézier Curve", "0.25, 0.1, 0.25, 1"),
        ("boxShadow", "CSS Box Shadow", "#00000030"),
        ("textTable", "Text Table Builder", "Tool,Type,Local\nJSON,Validation,Yes\nColor,Design,Yes"),
    ].into_iter().map(|(id,title,example)| Tool{id,title,example}).collect()
}

/// Execute a tool locally. Input limits apply to both fields combined; results
/// above 4 MB are rejected rather than silently truncated.
pub fn execute(id: &str, input: &str, second: &str) -> R<String> {
    if input.len().saturating_add(second.len()) > MAX_INPUT {
        return Err("Inputs exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    let out = match id {
        "json" => {
            let v = parse_json(input)?;
            match second.trim() {
                "" | "pretty" => pretty(&v),
                "minify" => compact(&v),
                "validate" => Ok(
                    "Valid JSON · duplicate keys rejected · exact number lexemes preserved".into(),
                ),
                _ => Err("Use pretty, minify, or validate in the second field.".into()),
            }
        }
        "compare" => compare(input, second),
        "jwt" => jwt(input),
        "regex" => regex_tool(input, second),
        "url" => url_tool(input, second),
        "time" => time_tool(input, second),
        "cron" => cron(input, second),
        "convert" => convert(input, second),
        "snippets" => snippets(input, second),
        "http" => http(input),
        "generate" => generate(input),
        "calculator" => calculator(input),
        "units" => units(input, second),
        "numberBase" => number_base(input, second),
        "color" => color_tool(input),
        "contrast" => contrast(input, second),
        "gradient" => gradient(input, second),
        "markdown" => markdown(input),
        "jsonPointer" => {
            let v = parse_json(input)?;
            pretty(pointer(&v, second)?)
        }
        "jsonFlatten" => {
            let v = parse_json(input)?;
            match second.trim() {
                "" | "flatten" => pretty(&flatten(&v)?),
                "unflatten" => pretty(&unflatten(&v)?),
                _ => Err("Use flatten or unflatten in the second field.".into()),
            }
        }
        "jsonCode" => json_code(input, second),
        "sqlInsert" => sql_insert(input, second),
        "xmlJSON" => xml_json(input),
        "unicode" => unicode(input),
        "stringEscape" => string_escape(input, second),
        "extract" => extract(input, second),
        "listSet" => list_set::run(input, second, "Union", "Exact"),
        "semver" => semver_tool(input, second),
        "subnet" => subnet(input),
        "chmod" => chmod(input),
        "hmac" => {
            if second.is_empty() {
                return Err("Enter an ephemeral HMAC key in the second field.".into());
            }
            let mut mac =
                Hmac::<Sha256>::new_from_slice(second.as_bytes()).map_err(|e| e.to_string())?;
            mac.update(input.as_bytes());
            Ok(hex(&mac.finalize().into_bytes()))
        }
        "jsonSchema" => schema_tool(input, second),
        "jsonMerge" => {
            let mut v = parse_json(input)?;
            merge_patch(&mut v, &parse_json(second)?);
            pretty(&v)
        }
        "jsonRedact" => {
            let mut v = parse_json(input)?;
            let keys: HashSet<_> = if second.trim().is_empty() {
                "password,token,secret,api_key,email"
            } else {
                second
            }
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
            redact(&mut v, &keys);
            pretty(&v)
        }
        "jsonLines" => json_lines(input, second),
        "csvExplore" => csv_explore(input, second),
        "envFile" => env_file(input, second),
        "plist" => plist(input, second),
        "sqlFormat" => sql_format(input, second),
        "httpHeaders" => pretty(&headers(input)?),
        "cookies" => cookies(input, second),
        "certificate" => certificate::inspect(input),
        "sshKey" => ssh_key(input),
        "uuidInspect" => uuid_inspect(input),
        "bitwise" => bitwise(input, second),
        "statistics" => statistics(input, second),
        "dateMath" => date_math(input, second),
        "aspectRatio" => aspect_ratio(input, second),
        "bezier" => bezier(input),
        "boxShadow" => box_shadow(input, second),
        "textTable" => text_table(input, second),
        _ => Err(format!("Unknown developer tool: {id}")),
    }?;
    if out.len() > MAX_OUTPUT {
        Err("Result exceeds 4 MB; use a smaller input. Nothing was truncated.".into())
    } else {
        Ok(out)
    }
}
fn pretty(v: &Value) -> R<String> {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}
fn compact(v: &Value) -> R<String> {
    serde_json::to_string(v).map_err(|e| e.to_string())
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn finite(n: f64) -> R<f64> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err("A calculation has no finite real result.".into())
    }
}
fn number(s: &str) -> R<f64> {
    finite(
        s.trim()
            .parse::<f64>()
            .map_err(|_| "Enter a finite number.".to_string())?,
    )
}
fn scalar(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_string())
}
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// A small structural parser protects the serde value conversion against deep
// trees, duplicate object keys, and accidental f64 conversion of large IDs.
fn parse_json(s: &str) -> R<Value> {
    struct P<'a> {
        s: &'a str,
        i: usize,
        n: usize,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self
                .s
                .as_bytes()
                .get(self.i)
                .is_some_and(|b| b" \n\r\t".contains(b))
            {
                self.i += 1
            }
        }
        fn err(&self, msg: &str) -> String {
            format!(
                "{msg} at line {}, byte {}",
                self.s[..self.i].bytes().filter(|b| *b == b'\n').count() + 1,
                self.i + 1
            )
        }
        fn eat(&mut self, b: u8) -> bool {
            self.ws();
            if self.s.as_bytes().get(self.i) == Some(&b) {
                self.i += 1;
                true
            } else {
                false
            }
        }
        fn string(&mut self) -> R<String> {
            self.ws();
            let start = self.i;
            if !self.eat(b'"') {
                return Err(self.err("Expected a quoted string"));
            }
            let mut escape = false;
            while let Some(&b) = self.s.as_bytes().get(self.i) {
                self.i += 1;
                if b == b'"' && !escape {
                    return serde_json::from_str(&self.s[start..self.i]).map_err(|e| e.to_string());
                }
                escape = b == b'\\' && !escape;
            }
            Err(self.err("Unterminated string"))
        }
        fn value(&mut self, d: usize) -> R<Value> {
            self.ws();
            self.n += 1;
            if d >= MAX_DEPTH || self.n > MAX_NODES {
                return Err("JSON exceeds 64 levels or 20,000 values.".into());
            }
            match self.s.as_bytes().get(self.i) {
                Some(b'{') => {
                    self.i += 1;
                    let mut m = Map::new();
                    if self.eat(b'}') {
                        return Ok(Value::Object(m));
                    }
                    loop {
                        let k = self.string()?;
                        if m.contains_key(&k) {
                            return Err(self.err("Duplicate JSON property"));
                        }
                        if !self.eat(b':') {
                            return Err(self.err("Expected ':'"));
                        }
                        m.insert(k, self.value(d + 1)?);
                        if self.eat(b'}') {
                            break;
                        }
                        if !self.eat(b',') {
                            return Err(self.err("Expected ',' or '}'"));
                        }
                    }
                    Ok(Value::Object(m))
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = vec![];
                    if self.eat(b']') {
                        return Ok(Value::Array(a));
                    }
                    loop {
                        a.push(self.value(d + 1)?);
                        if self.eat(b']') {
                            break;
                        }
                        if !self.eat(b',') {
                            return Err(self.err("Expected ',' or ']'"));
                        }
                    }
                    Ok(Value::Array(a))
                }
                Some(b'"') => Ok(Value::String(self.string()?)),
                Some(_) => {
                    let start = self.i;
                    while self
                        .s
                        .as_bytes()
                        .get(self.i)
                        .is_some_and(|b| !b" \n\r\t,]}".contains(b))
                    {
                        self.i += 1
                    }
                    if start == self.i {
                        return Err(self.err("Expected a JSON value"));
                    }
                    serde_json::from_str::<Value>(&self.s[start..self.i])
                        .map_err(|e| self.err(&e.to_string()))
                }
                None => Err(self.err("Expected a JSON value")),
            }
        }
    }
    if s.len() > MAX_INPUT {
        return Err("JSON exceeds 500,000 UTF-8 bytes.".into());
    }
    let mut p = P { s, i: 0, n: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != s.len() {
        Err(p.err("Unexpected trailing content"))
    } else {
        Ok(v)
    }
}
fn pointer_tokens(s: &str) -> R<Vec<String>> {
    if s.len() > 8192 {
        return Err("JSON pointers are limited to 8 KB.".into());
    }
    let s = if let Some(s) = s.strip_prefix('#') {
        percent_decode(s)?
    } else {
        s.into()
    };
    if s.is_empty() {
        return Ok(vec![]);
    }
    if !s.starts_with('/') {
        return Err(
            "A JSON pointer must start with / (or #/ for a URI fragment); empty selects the root."
                .into(),
        );
    }
    s[1..]
        .split('/')
        .map(|t| {
            let mut out = String::new();
            let mut it = t.chars();
            while let Some(c) = it.next() {
                if c == '~' {
                    out.push(match it.next() {
                        Some('0') => '~',
                        Some('1') => '/',
                        _ => return Err("Invalid JSON pointer escape; use ~0 or ~1.".into()),
                    })
                } else {
                    out.push(c)
                }
            }
            Ok(out)
        })
        .collect()
}
fn array_index(t: &str) -> R<usize> {
    if t.is_empty() || (t.len() > 1 && t.starts_with('0')) || !t.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("Array indices must be unsigned digits without leading zeros.".into());
    }
    t.parse().map_err(|_| "Array index is too large.".into())
}
fn pointer<'a>(v: &'a Value, s: &str) -> R<&'a Value> {
    let mut v = v;
    for t in pointer_tokens(s)? {
        v = match v {
            Value::Object(m) => m
                .get(&t)
                .ok_or_else(|| format!("No property {t:?} at this path."))?,
            Value::Array(a) => a
                .get(array_index(&t)?)
                .ok_or("Array index is out of bounds.")?,
            _ => return Err("Pointer continues beyond a scalar value.".into()),
        }
    }
    Ok(v)
}
fn escape_pointer(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
fn flatten(v: &Value) -> R<Value> {
    fn walk(v: &Value, p: String, a: &mut Vec<Value>, total: &mut usize) -> R<()> {
        *total += p.len();
        if p.len() > 8192 || *total > 2_000_000 || a.len() >= MAX_NODES {
            return Err(
                "Flattened paths exceed the 8 KB path, 2 MB total-path, or 20,000-entry limit."
                    .into(),
            );
        }
        match v {
            Value::Object(m) => {
                a.push(json!({"path":p,"type":"object"}));
                for (k, v) in m {
                    walk(v, format!("{p}/{}", escape_pointer(k)), a, total)?
                }
            }
            Value::Array(values) => {
                a.push(json!({"path":p,"type":"array"}));
                for (i, v) in values.iter().enumerate() {
                    walk(v, format!("{p}/{i}"), a, total)?
                }
            }
            _ => a.push(json!({"path":p,"type":"value","value":v})),
        }
        Ok(())
    }
    let mut a = vec![];
    walk(v, "".into(), &mut a, &mut 0)?;
    Ok(Value::Array(a))
}
fn unflatten(v: &Value) -> R<Value> {
    #[derive(Default)]
    struct Node {
        v: Option<Value>,
        kind: Option<String>,
        children: BTreeMap<String, Node>,
    }
    fn build(n: Node) -> R<Value> {
        match n.kind.as_deref() {
            Some("value") => {
                let v = n.v.ok_or("A scalar entry needs value.")?;
                if !n.children.is_empty() || v.is_array() || v.is_object() {
                    return Err("Scalar entries cannot contain children or containers.".into());
                }
                Ok(v)
            }
            Some("object") => {
                if n.v.is_some() {
                    return Err("Container entries cannot have value.".into());
                }
                Ok(Value::Object(
                    n.children
                        .into_iter()
                        .map(|(k, v)| Ok((k, build(v)?)))
                        .collect::<R<Map<_, _>>>()?,
                ))
            }
            Some("array") => {
                if n.v.is_some() {
                    return Err("Container entries cannot have value.".into());
                }
                let mut c = n.children;
                let mut a = vec![];
                for i in 0..c.len() {
                    a.push(build(
                        c.remove(&i.to_string())
                            .ok_or("Array indices must be consecutive from 0.")?,
                    )?)
                }
                Ok(Value::Array(a))
            }
            _ => Err("Every parent, including the root, needs a typed entry.".into()),
        }
    }
    let a = v
        .as_array()
        .ok_or("Unflatten expects a typed entry array.")?;
    if a.is_empty() {
        return Err("The entry array must contain a root.".into());
    }
    let mut root = Node::default();
    let mut nodes = 1;
    for e in a {
        let m = e.as_object().ok_or("Each entry must be an object.")?;
        if m.keys()
            .any(|k| !matches!(k.as_str(), "path" | "type" | "value"))
        {
            return Err("Unexpected typed-entry property.".into());
        }
        let path = m
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Entry needs path.")?;
        if path.starts_with('#') {
            return Err("Typed entries use plain pointers.".into());
        }
        let ts = pointer_tokens(path)?;
        if ts.len() >= MAX_DEPTH {
            return Err("Path exceeds 64 levels.".into());
        }
        let mut n = &mut root;
        for t in ts {
            if !n.children.contains_key(&t) {
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err("Path tree exceeds 20,000 values.".into());
                }
            }
            n = n.children.entry(t).or_default()
        }
        if n.kind.is_some() {
            return Err("Duplicate typed-entry path.".into());
        }
        n.kind = Some(
            m.get("type")
                .and_then(Value::as_str)
                .ok_or("Entry needs type.")?
                .into(),
        );
        n.v = m.get("value").cloned();
    }
    build(root)
}
fn merge_patch(target: &mut Value, patch: &Value) {
    if let Value::Object(m) = patch {
        if !target.is_object() {
            *target = json!({})
        }
        let t = target.as_object_mut().unwrap();
        for (k, v) in m {
            if v.is_null() {
                t.remove(k);
            } else {
                merge_patch(t.entry(k.clone()).or_insert(Value::Null), v)
            }
        }
    } else {
        *target = patch.clone()
    }
}
fn redact(v: &mut Value, keys: &HashSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                if keys.contains(&k.to_lowercase()) {
                    *v = Value::String("[REDACTED]".into())
                } else {
                    redact(v, keys)
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                redact(v, keys)
            }
        }
        _ => (),
    }
}
fn json_lines(s: &str, mode: &str) -> R<String> {
    match mode.trim() {
        "array-to-lines" | "lines" => {
            let v = parse_json(s)?;
            let a = v.as_array().ok_or("Expected a JSON array.")?;
            a.iter()
                .map(compact)
                .collect::<R<Vec<_>>>()
                .map(|s| s.join("\n"))
        }
        "" | "lines-to-array" | "array" => {
            let mut a = vec![];
            for (i, line) in s.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                if a.len() >= 10_000 {
                    return Err("JSON Lines is limited to 10,000 records.".into());
                }
                a.push(parse_json(line).map_err(|e| format!("Record at line {}: {e}", i + 1))?)
            }
            pretty(&Value::Array(a))
        }
        _ => Err("Use lines-to-array or array-to-lines.".into()),
    }
}

fn compare(a: &str, b: &str) -> R<String> {
    let a: Vec<_> = a.lines().collect();
    let b: Vec<_> = b.lines().collect();
    if a.len().saturating_mul(b.len()) > 1_000_000 {
        return Err("Comparison is limited to one million line pairs; use fewer lines.".into());
    }
    let mut dp = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            }
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut lines = vec![];
    let (mut added, mut removed) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            lines.push(format!("  {}", a[i]));
            i += 1;
            j += 1
        } else if j < b.len() && (i == a.len() || dp[i][j + 1] >= dp[i + 1][j]) {
            lines.push(format!("+ {}", b[j]));
            added += 1;
            j += 1
        } else {
            lines.push(format!("- {}", a[i]));
            removed += 1;
            i += 1
        }
    }
    Ok(format!(
        "{added} added · {removed} removed (line comparison)\n{}",
        lines.join("\n")
    ))
}
fn jwt(s: &str) -> R<String> {
    let s = s.trim().strip_prefix("Bearer ").unwrap_or(s.trim());
    let p: Vec<_> = s.split('.').collect();
    if p.len() != 3 {
        return Err("Expected a three-part JWT. JWE encrypted tokens are not supported.".into());
    }
    let decode = |s: &str| -> R<Value> {
        if s.is_empty()
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("JWT segments must be unpadded base64url.".into());
        }
        let b = b64::URL_SAFE_NO_PAD.decode(s).map_err(|e| e.to_string())?;
        let t = std::str::from_utf8(&b).map_err(|_| "JWT JSON is not UTF-8.")?;
        let v = parse_json(t)?;
        if !v.is_object() {
            return Err("JWT header and claims must be JSON objects.".into());
        }
        Ok(v)
    };
    let h = decode(p[0])?;
    let claims = decode(p[1])?;
    let mut out = format!(
        "SIGNATURE NOT VERIFIED · decoded values are untrusted\n\nHeader\n{}\n\nClaims\n{}",
        pretty(&h)?,
        pretty(&claims)?
    );
    for k in ["iat", "nbf", "exp"] {
        if let Some(v) = claims.get(k)
            && let Some(n) = v.as_i64()
            && let Some(d) = DateTime::<Utc>::from_timestamp(n, 0)
        {
            out.push_str(&format!(
                "\n{k}: {}{}",
                d.to_rfc3339(),
                if k == "exp" && d < Utc::now() {
                    " (expired)"
                } else {
                    ""
                }
            ))
        }
    }
    Ok(out)
}
fn regex_tool(text: &str, options: &str) -> R<String> {
    // Plain second field is the pattern. JSON permits pattern/replacement/flags.
    let (pattern, replacement, flags) = if options.trim_start().starts_with('{') {
        let v = parse_json(options)?;
        let o = v.as_object().ok_or("Regex options must be an object.")?;
        if o.keys()
            .any(|k| !matches!(k.as_str(), "pattern" | "replacement" | "flags"))
        {
            return Err("Regex options: pattern, replacement, flags (i,m,s).".into());
        }
        (
            v.get("pattern")
                .and_then(Value::as_str)
                .ok_or("Regex options need pattern.")?
                .to_owned(),
            v.get("replacement")
                .and_then(Value::as_str)
                .map(str::to_owned),
            v.get("flags")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        )
    } else {
        (
            if options.is_empty() {
                r"([A-Z]+)-(\d+)"
            } else {
                options
            }
            .to_owned(),
            None,
            String::new(),
        )
    };
    if pattern.len() > 4096 || text.len() > 128_000 {
        return Err("Regex limits: 4 KB pattern and 128 KB text.".into());
    }
    if flags.chars().any(|c| !"ims".contains(c)) {
        return Err("Regex flags support only i, m, and s.".into());
    }
    let re=RegexBuilder::new(&pattern).case_insensitive(flags.contains('i')).multi_line(flags.contains('m')).dot_matches_new_line(flags.contains('s')).size_limit(2_000_000).dfa_size_limit(2_000_000).build().map_err(|e|format!("Regex error: {e}. Lookaround and backreferences are not supported by this bounded engine."))?;
    let mut matches = vec![];
    for c in re.captures_iter(text) {
        if matches.len() >= 1000 {
            return Err("More than 1,000 matches; narrow the expression.".into());
        }
        let m = c.get(0).unwrap();
        matches.push(json!({"text":m.as_str(),"startByte":m.start(),"endByte":m.end(),"groups":c.iter().skip(1).map(|m|m.map(|m|m.as_str())).collect::<Vec<_>>()}))
    }
    let mut out = json!({"count":matches.len(),"matches":matches,"engine":"Rust regex; UTF-8 byte offsets; no lookaround/backreferences"});
    if let Some(r) = replacement {
        let mut s = String::new();
        let mut end = 0;
        for c in re.captures_iter(text) {
            let m = c.get(0).unwrap();
            s.push_str(&text[end..m.start()]);
            c.expand(&r, &mut s);
            end = m.end();
            if s.len() > MAX_OUTPUT {
                return Err("Replacement exceeds 4 MB.".into());
            }
        }
        s.push_str(&text[end..]);
        out["replaced"] = Value::String(s)
    }
    pretty(&out)
}
fn percent_decode(s: &str) -> R<String> {
    let b = s.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'%'
            && (i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit())
        {
            return Err("Invalid percent escape.".into());
        }
    }
    percent_encoding::percent_decode_str(s)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| "Percent-decoded text is not UTF-8.".into())
}
fn url_tool(s: &str, second: &str) -> R<String> {
    let mut u = url::Url::parse(s.trim()).map_err(|e| e.to_string())?;
    if !second.trim().is_empty() {
        let v = parse_json(second)?;
        let a = v.as_array().ok_or(
            "URL query edits need an array of {name,value} records; omit value for flags.",
        )?;
        let mut parts = vec![];
        for v in a {
            let n = v
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Each query edit needs a string name.")?;
            let enc = |s: &str| {
                percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC)
                    .to_string()
            };
            let mut p = enc(n);
            if let Some(v) = v.get("value") {
                p.push('=');
                p.push_str(&enc(v.as_str().ok_or("Query values must be strings.")?))
            }
            parts.push(p)
        }
        u.set_query(Some(&parts.join("&")))
    }
    let mut params = vec![];
    if let Some(q) = u.query()
        && !q.is_empty()
    {
        for p in q.split('&') {
            let (n, v) = p
                .split_once('=')
                .map(|(n, v)| (n, Some(v)))
                .unwrap_or((p, None));
            params.push(json!({"name":percent_decode(n)?,"value":v.map(percent_decode).transpose()?,"hasValue":v.is_some()}))
        }
    }
    pretty(
        &json!({"url":u.as_str(),"scheme":u.scheme(),"host":u.host_str(),"port":u.port(),"path":percent_decode(u.path())?,"fragment":u.fragment().map(percent_decode).transpose()?,"query":params,"note":"Query plus signs are literal; repeated parameters and flags stay ordered."}),
    )
}
fn parse_date(s: &str) -> R<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(i) = s.parse::<i64>() {
        let millis = i.unsigned_abs() >= 100_000_000_000;
        return if millis {
            DateTime::from_timestamp_millis(i)
        } else {
            DateTime::from_timestamp(i, 0)
        }
        .ok_or("Timestamp is outside the supported calendar range.".into());
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Ok(d.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }
    Err("Enter Unix seconds/milliseconds, YYYY-MM-DD, or RFC 3339 with an offset.".into())
}
fn time_tool(s: &str, second: &str) -> R<String> {
    let d = parse_date(s)?;
    let mut out = format!(
        "UTC: {}\nUnix seconds: {}\nUnix milliseconds: {}",
        d.to_rfc3339(),
        d.timestamp(),
        d.timestamp_millis()
    );
    if !second.trim().is_empty() {
        if let Ok(tz) = second.trim().parse::<chrono_tz::Tz>() {
            out.push_str(&format!("\n{}: {}", tz, d.with_timezone(&tz).to_rfc3339()))
        } else {
            let other = parse_date(second)?;
            let ms = other.signed_duration_since(d).num_milliseconds();
            out.push_str(&format!(
                "\nDifference (second − input): {ms} ms\n{:.6} hours\n{:.6} days",
                ms as f64 / 3_600_000.,
                ms as f64 / 86_400_000.
            ))
        }
    }
    Ok(out)
}
fn cron(s: &str, second: &str) -> R<String> {
    fn field(s: &str, min: u32, max: u32, names: &[&str]) -> R<BTreeSet<u32>> {
        let val = |s: &str| -> R<u32> {
            if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
                Ok(i as u32 + min)
            } else {
                s.parse().map_err(|_| "Invalid cron field value.".into())
            }
        };
        let mut values = BTreeSet::new();
        for p in s.split(',') {
            let (range, step) = match p.split_once('/') {
                Some((a, b)) => (a, b.parse::<u32>().map_err(|_| "Invalid cron step.")?),
                None => (p, 1),
            };
            if step == 0 || step > max + 1 {
                return Err("Cron steps must be positive and within the field range.".into());
            }
            let (a, b) = if range == "*" {
                (min, max)
            } else if let Some((a, b)) = range.split_once('-') {
                (val(a)?, val(b)?)
            } else {
                let a = val(range)?;
                (a, if p.contains('/') { max } else { a })
            };
            if a < min || b > max || a > b {
                return Err("Cron field is out of range or reversed.".into());
            }
            for n in (a..=b).step_by(step as usize) {
                values.insert(n);
            }
        }
        Ok(values)
    }
    let f: Vec<_> = s.split_whitespace().collect();
    if f.len() != 5 {
        return Err("Use five cron fields: minute hour day month weekday.".into());
    }
    let minutes = field(f[0], 0, 59, &[])?;
    let hours = field(f[1], 0, 23, &[])?;
    let days = field(f[2], 1, 31, &[])?;
    let months = field(
        f[3],
        1,
        12,
        &[
            "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
        ],
    )?;
    let mut weekdays = field(
        f[4],
        0,
        7,
        &["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"],
    )?;
    if weekdays.remove(&7) {
        weekdays.insert(0);
    }
    let after = if second.trim().is_empty() {
        Utc::now()
    } else {
        parse_date(second)?
    };
    let mut d = after
        .with_second(0)
        .and_then(|x| x.with_nanosecond(0))
        .unwrap()
        .checked_add_signed(Duration::minutes(1))
        .ok_or("Date overflow.")?;
    let mut runs = vec![];
    for _ in 0..527_040 {
        let dom = days.contains(&d.day());
        let dow = weekdays.contains(&d.weekday().num_days_from_sunday());
        let day = if f[2] == "*" {
            dow
        } else if f[4] == "*" {
            dom
        } else {
            dom || dow
        };
        if minutes.contains(&d.minute())
            && hours.contains(&d.hour())
            && months.contains(&d.month())
            && day
        {
            runs.push(d.to_rfc3339());
            if runs.len() == 5 {
                break;
            }
        }
        d = d
            .checked_add_signed(Duration::minutes(1))
            .ok_or("Date overflow.")?;
    }
    if runs.is_empty() {
        return Err("No run found within the bounded 366-day UTC search window.".into());
    }
    Ok(format!(
        "UTC only · next {} run(s) within 366 days\n{}\nDay-of-month and weekday use cron OR semantics when both are restricted.",
        runs.len(),
        runs.join("\n")
    ))
}

// CSV parser is deliberately strict, keeping numeric-looking strings intact.
fn csv_rows(s: &str) -> R<Vec<Vec<String>>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut cell = String::new();
    let mut state = 0u8;
    let mut it = s.chars().peekable();
    let mut ended = false;
    while let Some(c) = it.next() {
        ended = false;
        match state {
            1 => {
                if c == '"' {
                    if it.peek() == Some(&'"') {
                        it.next();
                        cell.push('"')
                    } else {
                        state = 2
                    }
                } else {
                    cell.push(c)
                }
            }
            _ => {
                if c == ',' {
                    row.push(std::mem::take(&mut cell));
                    state = 0
                } else if c == '\n' || c == '\r' {
                    if c == '\r' && it.peek() == Some(&'\n') {
                        it.next();
                    }
                    row.push(std::mem::take(&mut cell));
                    rows.push(std::mem::take(&mut row));
                    state = 0;
                    ended = true
                } else if state == 2 {
                    return Err("Unexpected text after a CSV closing quote.".into());
                } else if c == '"' {
                    if !cell.is_empty() {
                        return Err("CSV quotes must begin at the start of a field.".into());
                    }
                    state = 1
                } else {
                    cell.push(c)
                }
            }
        }
        if row.len() > 256 || rows.len() > 10_000 {
            return Err("CSV is limited to 10,000 rows and 256 columns.".into());
        }
    }
    if state == 1 {
        return Err("Unterminated quoted CSV field.".into());
    }
    if !ended && (!s.is_empty() || !row.is_empty()) {
        row.push(cell);
        rows.push(row)
    }
    let columns = rows.first().ok_or("Enter CSV with a header row.")?.len();
    if columns > 256 {
        return Err("CSV is limited to 256 columns.".into());
    }
    if rows.iter().any(|r| r.len() != columns) {
        return Err("CSV rows must have the same number of fields as the header.".into());
    }
    let mut seen = HashSet::new();
    if rows[0].iter().any(|h| h.is_empty() || !seen.insert(h)) {
        return Err("CSV headers must be nonempty and unique.".into());
    }
    Ok(rows)
}
fn csv_encode(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|s| {
                    if s.contains([',', '"', '\r', '\n']) {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    } else {
                        s.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn csv_json(s: &str) -> R<Value> {
    let rows = csv_rows(s)?;
    Ok(Value::Array(
        rows[1..]
            .iter()
            .map(|r| {
                Value::Object(
                    rows[0]
                        .iter()
                        .cloned()
                        .zip(r.iter().cloned().map(Value::String))
                        .collect(),
                )
            })
            .collect(),
    ))
}
fn json_csv(v: &Value) -> R<String> {
    let a = v
        .as_array()
        .ok_or("CSV output needs an array of JSON objects.")?;
    if a.is_empty() {
        return Err("CSV output needs at least one record to establish headers.".into());
    }
    let mut keys = BTreeSet::new();
    for v in a {
        keys.extend(
            v.as_object()
                .ok_or("CSV records must be objects.")?
                .keys()
                .cloned(),
        );
    }
    if keys.is_empty() || keys.len() > 256 || a.len() > 10_000 {
        return Err("CSV output needs 1–256 columns and at most 10,000 records.".into());
    }
    let header: Vec<_> = keys.into_iter().collect();
    let mut rows = vec![header.clone()];
    for v in a {
        rows.push(
            header
                .iter()
                .map(|k| v.get(k).map(scalar).unwrap_or_default())
                .collect(),
        )
    }
    Ok(csv_encode(&rows))
}
fn yaml_json(s: &str) -> R<Value> {
    use yaml_rust2::{
        parser::{Event, Parser},
        scanner::TScalarStyle,
    };
    enum Frame {
        Array(Vec<Value>),
        Object(Map<String, Value>, Option<String>),
    }
    let mut parser = Parser::new_from_str(s);
    let mut stack: Vec<Frame> = vec![];
    let mut root = None;
    let mut count = 0;
    let mut docs = 0;
    fn attach(v: Value, stack: &mut [Frame], root: &mut Option<Value>) -> R<()> {
        match stack.last_mut() {
            Some(Frame::Array(a)) => a.push(v),
            Some(Frame::Object(m, key)) => {
                if let Some(k) = key.take() {
                    if m.insert(k, v).is_some() {
                        return Err("Duplicate YAML key.".into());
                    }
                } else {
                    let k = v
                        .as_str()
                        .ok_or("YAML mappings require string keys for JSON conversion.")?;
                    if k == "<<" {
                        return Err("YAML merge keys are not supported; expand them first.".into());
                    }
                    *key = Some(k.into())
                }
            }
            None => {
                if root.replace(v).is_some() {
                    return Err("Only one YAML document is supported.".into());
                }
            }
        }
        Ok(())
    }
    loop {
        let (event, _) = parser.next_token().map_err(|e| e.to_string())?;
        count += 1;
        if count > MAX_NODES * 3 || stack.len() >= MAX_DEPTH {
            return Err("YAML exceeds 64 levels or the bounded event budget.".into());
        }
        match event {
            Event::StreamEnd => break,
            Event::DocumentStart => {
                docs += 1;
                if docs > 1 {
                    return Err("Only one YAML document is supported.".into());
                }
            }
            Event::Alias(_) => return Err(
                "YAML aliases are not supported by this bounded converter; expand aliases first."
                    .into(),
            ),
            Event::SequenceStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err("YAML anchors and explicit tags are not supported.".into());
                }
                stack.push(Frame::Array(vec![]))
            }
            Event::MappingStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err("YAML anchors and explicit tags are not supported.".into());
                }
                stack.push(Frame::Object(Map::new(), None))
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let f = stack.pop().ok_or("Malformed YAML container.")?;
                let v = match f {
                    Frame::Array(a) => Value::Array(a),
                    Frame::Object(m, None) => Value::Object(m),
                    _ => return Err("YAML object is missing a value.".into()),
                };
                attach(v, &mut stack, &mut root)?
            }
            Event::Scalar(s, style, anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err("YAML anchors and explicit tags are not supported.".into());
                }
                let v = if style == TScalarStyle::Plain {
                    match s.as_str() {
                        "" | "~" | "null" | "Null" | "NULL" => Value::Null,
                        "true" | "True" | "TRUE" => Value::Bool(true),
                        "false" | "False" | "FALSE" => Value::Bool(false),
                        _ => {
                            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                                if v.is_number() { v } else { Value::String(s) }
                            } else {
                                Value::String(s)
                            }
                        }
                    }
                } else {
                    Value::String(s)
                };
                attach(v, &mut stack, &mut root)?
            }
            _ => (),
        }
    }
    root.ok_or("Enter a YAML document.".into())
}
fn json_yaml(v: &Value) -> R<String> {
    fn emit(v: &Value, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        match v {
            Value::Object(m) if !m.is_empty() => {
                for (k, v) in m {
                    out.push_str(&pad);
                    out.push_str(&Value::String(k.clone()).to_string());
                    out.push(':');
                    if (v.is_object() && !v.as_object().unwrap().is_empty())
                        || (v.is_array() && !v.as_array().unwrap().is_empty())
                    {
                        out.push('\n');
                        emit(v, depth + 1, out)
                    } else {
                        out.push(' ');
                        out.push_str(&v.to_string());
                        out.push('\n')
                    }
                }
            }
            Value::Array(a) if !a.is_empty() => {
                for v in a {
                    out.push_str(&pad);
                    out.push('-');
                    if (v.is_object() && !v.as_object().unwrap().is_empty())
                        || (v.is_array() && !v.as_array().unwrap().is_empty())
                    {
                        out.push('\n');
                        emit(v, depth + 1, out)
                    } else {
                        out.push(' ');
                        out.push_str(&v.to_string());
                        out.push('\n')
                    }
                }
            }
            _ => {
                out.push_str(&pad);
                out.push_str(&v.to_string());
                out.push('\n')
            }
        }
    }
    let mut out = String::new();
    emit(v, 0, &mut out);
    Ok(out)
}
fn convert(s: &str, mode: &str) -> R<String> {
    let mode = mode.trim().to_lowercase();
    let detected = if parse_json(s).is_ok() {
        "json"
    } else if s.lines().next().is_some_and(|s| s.contains(',')) {
        "csv"
    } else {
        "yaml"
    };
    let (from, to) = if mode.is_empty() {
        (detected, if detected == "json" { "yaml" } else { "json" })
    } else {
        mode.split_once('-')
            .ok_or("Use json-yaml, yaml-json, csv-json, json-csv, yaml-csv, or csv-yaml.")?
    };
    let v = match from {
        "json" => parse_json(s)?,
        "yaml" => yaml_json(s)?,
        "csv" => csv_json(s)?,
        _ => return Err("Unknown source format.".into()),
    };
    match to {
        "json" => pretty(&v),
        "yaml" => json_yaml(&v),
        "csv" => json_csv(&v),
        _ => Err("Unknown target format.".into()),
    }
}
fn csv_explore(s: &str, options: &str) -> R<String> {
    let rows = csv_rows(s)?;
    let mut selected = rows[1..].to_vec();
    let mut columns: Vec<usize> = (0..rows[0].len()).collect();
    if !options.trim().is_empty() {
        let v = parse_json(options)?;
        let o = v.as_object().ok_or(
            "Options need a JSON object: {unique:true,contains:\"text\",columns:[\"name\"]}.",
        )?;
        if o.keys()
            .any(|k| !matches!(k.as_str(), "unique" | "contains" | "columns"))
        {
            return Err("Supported CSV options: unique, contains, columns.".into());
        }
        if let Some(f) = v.get("contains").and_then(Value::as_str) {
            selected.retain(|r| r.iter().any(|s| s.contains(f)))
        }
        if v.get("unique").and_then(Value::as_bool).unwrap_or(false) {
            let mut seen = HashSet::new();
            selected.retain(|r| seen.insert(r.clone()));
        }
        if let Some(c) = v.get("columns") {
            columns = c
                .as_array()
                .ok_or("columns must be an array.")?
                .iter()
                .map(|v| {
                    rows[0]
                        .iter()
                        .position(|h| Some(h.as_str()) == v.as_str())
                        .ok_or("Unknown CSV column.".into())
                })
                .collect::<R<_>>()?;
            if columns.is_empty() {
                return Err("Choose at least one column.".into());
            }
        }
    }
    let mut out = vec![columns.iter().map(|i| rows[0][*i].clone()).collect()];
    out.extend(
        selected
            .iter()
            .map(|r| columns.iter().map(|i| r[*i].clone()).collect()),
    );
    Ok(csv_encode(&out))
}

fn calculator(s: &str) -> R<String> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
        depth: usize,
    }
    impl P<'_> {
        fn space(&mut self) {
            while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
                self.i += 1
            }
        }
        fn take(&mut self, b: u8) -> bool {
            self.space();
            if self.s.get(self.i) == Some(&b) {
                self.i += 1;
                true
            } else {
                false
            }
        }
        fn expr(&mut self) -> R<f64> {
            let mut n = self.term()?;
            loop {
                if self.take(b'+') {
                    n = finite(n + self.term()?)?
                } else if self.take(b'-') {
                    n = finite(n - self.term()?)?
                } else {
                    return Ok(n);
                }
            }
        }
        fn term(&mut self) -> R<f64> {
            let mut n = self.unary()?;
            loop {
                if self.take(b'*') {
                    n = finite(n * self.unary()?)?
                } else if self.take(b'/') {
                    n = finite(n / self.unary()?)?
                } else if self.take(b'%') {
                    n = finite(n % self.unary()?)?
                } else {
                    return Ok(n);
                }
            }
        }
        fn unary(&mut self) -> R<f64> {
            self.depth += 1;
            if self.depth > 64 {
                return Err("Expression exceeds 64 nesting levels.".into());
            }
            let n = if self.take(b'+') {
                self.unary()?
            } else if self.take(b'-') {
                -self.unary()?
            } else {
                let n = self.primary()?;
                if self.take(b'^') {
                    finite(n.powf(self.unary()?))?
                } else {
                    n
                }
            };
            self.depth -= 1;
            finite(n)
        }
        fn primary(&mut self) -> R<f64> {
            self.space();
            if self.take(b'(') {
                let n = self.expr()?;
                if !self.take(b')') {
                    return Err("Missing closing parenthesis.".into());
                }
                return Ok(n);
            }
            let start = self.i;
            while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
                self.i += 1
            }
            if self.i > start {
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1
                }
                let name = std::str::from_utf8(&self.s[start..self.i]).unwrap();
                if name == "pi" {
                    return Ok(std::f64::consts::PI);
                }
                if name == "e" {
                    return Ok(std::f64::consts::E);
                }
                if !self.take(b'(') {
                    return Err("Unknown constant; functions require parentheses.".into());
                }
                let n = self.expr()?;
                let second = if self.take(b',') {
                    Some(self.expr()?)
                } else {
                    None
                };
                if !self.take(b')') {
                    return Err("Missing closing function parenthesis.".into());
                }
                return finite(if let Some(b) = second {
                    match name {
                        "min" => n.min(b),
                        "max" => n.max(b),
                        "pow" => n.powf(b),
                        _ => return Err("Function does not support two arguments.".into()),
                    }
                } else {
                    match name{"sqrt"=>n.sqrt(),"abs"=>n.abs(),"sin"=>n.sin(),"cos"=>n.cos(),"tan"=>n.tan(),"ln"=>n.ln(),"log"|"log10"=>n.log10(),"exp"=>n.exp(),"floor"=>n.floor(),"ceil"=>n.ceil(),"round"=>n.round(),_=>return Err("Unknown function. Use sqrt, abs, sin, cos, tan, ln, log, exp, floor, ceil, round, min, max, pow.".into())}
                });
            }
            while self
                .s
                .get(self.i)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
            {
                self.i += 1
            }
            if self.s.get(self.i).is_some_and(|b| *b == b'e' || *b == b'E') {
                self.i += 1;
                if self.s.get(self.i).is_some_and(|b| *b == b'+' || *b == b'-') {
                    self.i += 1
                }
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1
                }
            }
            if self.i == start {
                return Err(format!("Expected a number at byte {}.", start + 1));
            }
            number(
                std::str::from_utf8(&self.s[start..self.i])
                    .map_err(|_| "Only ASCII arithmetic expressions are supported.")?,
            )
        }
    }
    if s.len() > 4096 {
        return Err("Expressions are limited to 4 KB.".into());
    }
    let s = s.to_ascii_lowercase();
    let mut p = P {
        s: s.as_bytes(),
        i: 0,
        depth: 0,
    };
    let n = p.expr()?;
    p.space();
    if p.i != s.len() {
        return Err(format!("Unexpected input at byte {}.", p.i + 1));
    }
    Ok(n.to_string())
}
fn units(input: &str, second: &str) -> R<String> {
    fn unit(s: &str) -> Option<(&'static str, f64, f64)> {
        Some(match s {
            "m" => ("length", 1., 0.),
            "km" => ("length", 1000., 0.),
            "cm" => ("length", 0.01, 0.),
            "mm" => ("length", 0.001, 0.),
            "in" => ("length", 0.0254, 0.),
            "ft" => ("length", 0.3048, 0.),
            "yd" => ("length", 0.9144, 0.),
            "mi" => ("length", 1609.344, 0.),
            "kg" => ("mass", 1., 0.),
            "g" => ("mass", 0.001, 0.),
            "lb" => ("mass", 0.45359237, 0.),
            "oz" => ("mass", 0.028349523125, 0.),
            "C" | "°C" => ("temperature", 1., 273.15),
            "F" | "°F" => ("temperature", 5. / 9., 273.15 - 32. * 5. / 9.),
            "K" => ("temperature", 1., 0.),
            "B" => ("data", 1., 0.),
            "bit" => ("data", 0.125, 0.),
            "kB" => ("data", 1e3, 0.),
            "MB" => ("data", 1e6, 0.),
            "GB" => ("data", 1e9, 0.),
            "TB" => ("data", 1e12, 0.),
            "KiB" => ("data", 1024., 0.),
            "MiB" => ("data", 1048576., 0.),
            "GiB" => ("data", 1073741824., 0.),
            "TiB" => ("data", 1099511627776., 0.),
            "s" => ("duration", 1., 0.),
            "ms" => ("duration", 0.001, 0.),
            "min" => ("duration", 60., 0.),
            "h" => ("duration", 3600., 0.),
            "day" => ("duration", 86400., 0.),
            "week" => ("duration", 604800., 0.),
            "m/s" => ("speed", 1., 0.),
            "km/h" => ("speed", 1. / 3.6, 0.),
            "mph" => ("speed", 0.44704, 0.),
            "knot" => ("speed", 1852. / 3600., 0.),
            _ => return None,
        })
    }
    let p: Vec<_> = input.split_whitespace().collect();
    let options: Vec<_> = second.split_whitespace().collect();
    let (value, from, to) = match (p.as_slice(), options.as_slice()) {
        ([n, u], []) => (*n, *u, "B"),
        ([n, u], [to]) => (*n, *u, *to),
        ([n], [from, to]) => (*n, *from, *to),
        _ => return Err(
            "Use input '1024 KiB' and target 'MiB'; or input '1024' and second field 'KiB MiB'."
                .into(),
        ),
    };
    let a = unit(from).ok_or("Unknown source unit.")?;
    let b = unit(to).ok_or("Unknown target unit.")?;
    if a.0 != b.0 {
        return Err("Units must have the same dimension.".into());
    }
    let base = finite(number(value)? * a.1 + a.2)?;
    if a.0 == "temperature" && base < -1e-10 {
        return Err("Temperature is below absolute zero.".into());
    }
    Ok(format!("{} {to}", finite((base - b.2) / b.1)?))
}
fn number_base(s: &str, second: &str) -> R<String> {
    let mut s = s.trim();
    let neg = s.starts_with('-');
    if s.starts_with(['-', '+']) {
        s = &s[1..]
    }
    let (mut base, mut digits) = (10, s);
    for (p, b) in [
        ("0x", 16),
        ("0X", 16),
        ("0b", 2),
        ("0B", 2),
        ("0o", 8),
        ("0O", 8),
    ] {
        if let Some(t) = s.strip_prefix(p) {
            base = b;
            digits = t;
            break;
        }
    }
    if !second.trim().is_empty() {
        let b = second
            .trim()
            .parse::<u32>()
            .map_err(|_| "Input base must be 2, 8, 10, or 16.")?;
        if ![2, 8, 10, 16].contains(&b) || base != 10 && base != b {
            return Err("Input prefix and chosen base disagree.".into());
        }
        base = b
    }
    if digits.is_empty() || digits.len() > 256 {
        return Err("Use 1–256 digits.".into());
    }
    let mut bytes = vec![0u32];
    for c in digits.chars() {
        let mut carry = c
            .to_digit(base)
            .ok_or("Digit is invalid for the input base.")?;
        for b in &mut bytes {
            let n = *b * base + carry;
            *b = n % 256;
            carry = n / 256
        }
        while carry > 0 {
            bytes.push(carry % 256);
            carry /= 256
        }
    }
    let render = |radix: u32| {
        let mut b = bytes.clone();
        let mut out = vec![];
        while b.iter().any(|n| *n > 0) {
            let mut r = 0;
            for n in b.iter_mut().rev() {
                let value = r * 256 + *n;
                *n = value / radix;
                r = value % radix
            }
            out.push(char::from_digit(r, radix).unwrap())
        }
        if out.is_empty() {
            out.push('0')
        }
        let mut s: String = out.into_iter().rev().collect();
        if neg && s != "0" {
            s.insert(0, '-')
        }
        s
    };
    Ok(format!(
        "Decimal: {}\nHex: {}\nOctal: {}\nBinary: {}",
        render(10),
        render(16),
        render(8),
        render(2)
    ))
}
#[derive(Clone, Copy)]
struct Color {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}
impl Color {
    fn css(self) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            (self.r * 255.).round(),
            (self.g * 255.).round(),
            (self.b * 255.).round(),
            self.a
        )
    }
    fn hex(self) -> String {
        let rgb = format!(
            "#{:02X}{:02X}{:02X}",
            (self.r * 255.).round() as u8,
            (self.g * 255.).round() as u8,
            (self.b * 255.).round() as u8
        );
        if self.a < 1. {
            format!("{rgb}{:02X}", (self.a * 255.).round() as u8)
        } else {
            rgb
        }
    }
    fn luminance(self) -> f64 {
        let f = |n: f64| {
            if n <= 0.04045 {
                n / 12.92
            } else {
                ((n + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(self.r) + 0.7152 * f(self.g) + 0.0722 * f(self.b)
    }
}
fn parse_color(s: &str) -> R<Color> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        if ![3, 4, 6, 8].contains(&h.len()) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("HEX colors need 3, 4, 6, or 8 hexadecimal digits.".into());
        }
        let h = if h.len() <= 4 {
            h.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            h.into()
        };
        let n: Vec<_> = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64 / 255.)
            .collect();
        return Ok(Color {
            r: n[0],
            g: n[1],
            b: n[2],
            a: n.get(3).copied().unwrap_or(1.),
        });
    }
    let (name, args) = s
        .split_once('(')
        .ok_or("Use HEX, rgb(r,g,b), rgba(r,g,b,a), hsl(h,s%,l%), or hsla().")?;
    let args = args
        .strip_suffix(')')
        .ok_or("Missing color closing parenthesis.")?;
    let p: Vec<_> = args.split(',').map(str::trim).collect();
    let count = if name == "rgba" || name == "hsla" {
        4
    } else {
        3
    };
    if p.len() != count {
        return Err("Unexpected number of color components.".into());
    }
    let a = if count == 4 { number(p[3])? } else { 1. };
    if !(0.0..=1.0).contains(&a) {
        return Err("Alpha must be between 0 and 1.".into());
    }
    let (r, g, b) = match name {
        "rgb" | "rgba" => {
            let mut n = vec![];
            for p in &p[..3] {
                let v = if let Some(p) = p.strip_suffix('%') {
                    number(p)? / 100.
                } else {
                    number(p)? / 255.
                };
                if !(0.0..=1.0).contains(&v) {
                    return Err("RGB components are out of range.".into());
                }
                n.push(v)
            }
            (n[0], n[1], n[2])
        }
        "hsl" | "hsla" => {
            let h = number(p[0])?.rem_euclid(360.) / 60.;
            let sat = number(p[1].strip_suffix('%').ok_or("HSL saturation needs %.")?)? / 100.;
            let l = number(p[2].strip_suffix('%').ok_or("HSL lightness needs %.")?)? / 100.;
            if !(0.0..=1.0).contains(&sat) || !(0.0..=1.0).contains(&l) {
                return Err("HSL percentages are out of range.".into());
            }
            let c = (1. - (2. * l - 1.).abs()) * sat;
            let x = c * (1. - (h % 2. - 1.).abs());
            let m = l - c / 2.;
            let (r, g, b) = match h as u8 {
                0 => (c, x, 0.),
                1 => (x, c, 0.),
                2 => (0., c, x),
                3 => (0., x, c),
                4 => (x, 0., c),
                _ => (c, 0., x),
            };
            (r + m, g + m, b + m)
        }
        _ => return Err("Unknown color function.".into()),
    };
    Ok(Color { r, g, b, a })
}
fn color_tool(s: &str) -> R<String> {
    let c = parse_color(s)?;
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let l = (max + min) / 2.;
    let sat = if d == 0. {
        0.
    } else {
        d / (1. - (2. * l - 1.).abs())
    };
    let h = if d == 0. {
        0.
    } else if max == c.r {
        60. * ((c.g - c.b) / d).rem_euclid(6.)
    } else if max == c.g {
        60. * ((c.b - c.r) / d + 2.)
    } else {
        60. * ((c.r - c.g) / d + 4.)
    };
    Ok(format!(
        "{}\n{}\nhsla({h:.3}, {:.3}%, {:.3}%, {})",
        c.hex(),
        c.css(),
        sat * 100.,
        l * 100.,
        c.a
    ))
}
fn contrast(a: &str, b: &str) -> R<String> {
    let a = parse_color(a)?;
    let b = parse_color(if b.trim().is_empty() { "#FFFFFF" } else { b })?;
    if a.a != 1. || b.a != 1. {
        return Err("Contrast requires opaque colors; composite transparent colors first.".into());
    }
    let x = a.luminance();
    let y = b.luminance();
    let ratio = (x.max(y) + 0.05) / (x.min(y) + 0.05);
    let pass = |n| if ratio >= n { "Pass" } else { "Fail" };
    Ok(format!(
        "Contrast: {ratio:.3}:1\nWCAG AA normal text: {}\nWCAG AAA normal text: {}\nWCAG AA large text: {}\nWCAG AAA large text: {}",
        pass(4.5),
        pass(7.),
        pass(3.),
        pass(4.5)
    ))
}
fn gradient(a: &str, b: &str) -> R<String> {
    let a = parse_color(a)?;
    let b = parse_color(if b.trim().is_empty() { "#F5BC89" } else { b })?;
    Ok(format!(
        "background: linear-gradient(90deg, {} 0%, {} 100%);",
        a.css(),
        b.css()
    ))
}
fn markdown(s: &str) -> R<String> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let events = pulldown_cmark::Parser::new(s).filter_map(|e| match e {
        Event::Html(h) | Event::InlineHtml(h) => Some(Event::Text(h)),
        Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => None,
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let safe = dest_url.starts_with("https://")
                || dest_url.starts_with("http://")
                || dest_url.starts_with("mailto:")
                || dest_url.starts_with('#');
            Some(Event::Start(Tag::Link {
                link_type,
                dest_url: if safe { dest_url } else { "#".into() },
                title,
                id,
            }))
        }
        _ => Some(e),
    });
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events);
    Ok(out)
}
fn unicode(s: &str) -> R<String> {
    if s.chars().count() > 5000 {
        return Err("Unicode inspection is limited to 5,000 code points.".into());
    }
    let mut rows = vec![];
    for (i, c) in s.char_indices() {
        let mut b = [0u8; 4];
        let utf8 = c.encode_utf8(&mut b).as_bytes();
        let mut u = [0u16; 2];
        let utf16 = c.encode_utf16(&mut u);
        rows.push(format!(
            "byte {i}: U+{:04X} {:?} · UTF-8 {} · UTF-16 {}",
            c as u32,
            c,
            hex(utf8),
            utf16
                .iter()
                .map(|n| format!("{n:04X}"))
                .collect::<Vec<_>>()
                .join(" ")
        ))
    }
    Ok(format!(
        "{} code points · {} UTF-8 bytes · {} UTF-16 units\n{}",
        s.chars().count(),
        s.len(),
        s.encode_utf16().count(),
        rows.join("\n")
    ))
}
fn string_escape(s: &str, mode: &str) -> R<String> {
    match mode.trim() {
        "" | "json" => compact(&Value::String(s.into())),
        "unescape" => {
            let v = parse_json(s)?;
            v.as_str()
                .map(str::to_owned)
                .ok_or("Expected a JSON string literal.".into())
        }
        "shell" => {
            if s.contains('\0') {
                return Err("Shell arguments cannot contain NUL.".into());
            }
            Ok(format!("'{}'", s.replace('\'', "'\\''")))
        }
        "html" => Ok(html_escape(s)),
        _ => Err("Supported modes: json, unescape, shell, html. No code is executed.".into()),
    }
}
fn extract(s: &str, mode: &str) -> R<String> {
    let pattern = match mode.trim() {
        "links" => r#"https?://[^\s<>\"']+"#,
        "emails" => {
            r"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?\.[A-Za-z]{2,}"
        }
        "" | "all" => {
            r#"https?://[^\s<>\"']+|[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?\.[A-Za-z]{2,}"#
        }
        _ => return Err("Use links, emails, or all.".into()),
    };
    let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
    let mut seen = HashSet::new();
    let mut out = vec![];
    for m in re.find_iter(s) {
        let m = m
            .as_str()
            .trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
        if seen.insert(m) {
            out.push(m)
        }
    }
    Ok(out.join("\n"))
}
fn semver_tool(s: &str, second: &str) -> R<String> {
    let mut versions = s
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(|s| semver::Version::parse(s.trim()).map_err(|e| e.to_string()))
        .collect::<R<Vec<_>>>()?;
    if versions.is_empty() {
        return Err("Enter one semantic version per line.".into());
    }
    if !second.trim().is_empty() {
        if versions.len() != 1 {
            return Err("Comparison expects one version in each field.".into());
        }
        let b = semver::Version::parse(second.trim()).map_err(|e| e.to_string())?;
        return Ok(format!(
            "{} {} {} (build metadata does not affect precedence)",
            versions[0],
            match versions[0].cmp_precedence(&b) {
                std::cmp::Ordering::Less => "<",
                std::cmp::Ordering::Equal => "=",
                std::cmp::Ordering::Greater => ">",
            },
            b
        ));
    }
    versions.sort_by(semver::Version::cmp_precedence);
    Ok(versions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n"))
}
fn subnet(s: &str) -> R<String> {
    let (ip, p) = s
        .trim()
        .split_once('/')
        .ok_or("Use IPv4/prefix, e.g. 192.168.1.42/24.")?;
    let ip = ip
        .parse::<std::net::Ipv4Addr>()
        .map_err(|_| "Invalid IPv4 address.")?;
    let p = p.parse::<u32>().map_err(|_| "Invalid prefix.")?;
    if p > 32 {
        return Err("IPv4 prefix must be 0–32.".into());
    }
    let mask = if p == 0 { 0 } else { u32::MAX << (32 - p) };
    let network = u32::from(ip) & mask;
    let broadcast = network | !mask;
    let count = 1u64 << (32 - p);
    let (first, last, hosts) = if p >= 31 {
        (network, broadcast, count)
    } else {
        (network + 1, broadcast - 1, count - 2)
    };
    let fmt = std::net::Ipv4Addr::from;
    Ok(format!(
        "Network: {}/{p}\nNetmask: {}\nBroadcast: {}\nRange: {} – {}\nUsable hosts: {hosts}\n/31 follows point-to-point semantics; /32 is one host.",
        fmt(network),
        fmt(mask),
        fmt(broadcast),
        fmt(first),
        fmt(last)
    ))
}
fn chmod(s: &str) -> R<String> {
    let s = s.trim();
    let n = if s.len() == 9 {
        let mut n = 0u32;
        for (i, c) in s.chars().enumerate() {
            let expected = ['r', 'w', 'x'][i % 3];
            if c == expected {
                n |= 1 << (8 - i)
            } else if c != '-' {
                return Err("Symbolic mode accepts exactly rwxrwxrwx positions with '-' for absent permissions; special bits use octal.".into());
            }
        }
        n
    } else {
        if s.is_empty() || s.len() > 4 || !s.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
            return Err("Use octal 0000–7777 or nine rwx characters.".into());
        }
        u32::from_str_radix(s, 8).map_err(|e| e.to_string())?
    };
    let mut p = String::new();
    for i in 0..9 {
        p.push(if n & (1 << (8 - i)) != 0 {
            ['r', 'w', 'x'][i % 3]
        } else {
            '-'
        })
    }
    let mut chars: Vec<_> = p.chars().collect();
    for (bit, i, low, high) in [
        (0o4000, 2, 's', 'S'),
        (0o2000, 5, 's', 'S'),
        (0o1000, 8, 't', 'T'),
    ] {
        if n & bit != 0 {
            chars[i] = if chars[i] == 'x' { low } else { high }
        }
    }
    Ok(format!(
        "Octal: {n:04o}\nSymbolic: {}\nOwner: {}{}{}\nGroup: {}{}{}\nOther: {}{}{}",
        chars.iter().collect::<String>(),
        chars[0],
        chars[1],
        chars[2],
        chars[3],
        chars[4],
        chars[5],
        chars[6],
        chars[7],
        chars[8]
    ))
}

fn schema_tool(input: &str, schema: &str) -> R<String> {
    json_schema::validate(input, schema)
}
fn plist(input: &str, options: &str) -> R<String> {
    plist_engine::convert(input, options)
}
fn sql_format(input: &str, options: &str) -> R<String> {
    sql_formatter::run(input, options)
}

fn snippets(s: &str, second: &str) -> R<String> {
    let values = if second.trim().is_empty() {
        json!({})
    } else {
        parse_json(second)?
    };
    let values = values
        .as_object()
        .ok_or("Second field needs a JSON object of placeholder values.")?;
    let values = values
        .iter()
        .map(|(key, value)| (key.clone(), scalar(value)))
        .collect::<BTreeMap<_, _>>();
    crate::snippets::render_at(
        s,
        values.get("selection").map(String::as_str).unwrap_or(""),
        &values,
        Utc::now(),
        &uuid::Uuid::new_v4().to_string(),
    )
}

fn generate(s: &str) -> R<String> {
    let p: Vec<_> = s.split_whitespace().collect();
    let kind = p.first().copied().unwrap_or("uuid");
    let count = p
        .get(1)
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| "Generator count must be an integer.")?
        .unwrap_or(1);
    if count == 0 || count > 1000 || p.len() > 2 {
        return Err("Use 'uuid N', 'hex N', or 'records N', with N from 1–1000.".into());
    }
    match kind {
        "uuid" => Ok((0..count)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect::<Vec<_>>()
            .join("\n")),
        "hex" => Ok((0..count)
            .map(|_| uuid::Uuid::new_v4().simple().to_string())
            .collect::<Vec<_>>()
            .join("\n")),
        "records" => pretty(&Value::Array(
            (0..count)
                .map(|i| json!({"id":i+1,"name":format!("Sample {}",i+1),"active":i%2==0}))
                .collect(),
        )),
        _ => Err(
            "Supported generators: uuid, hex (UUID-derived, not a password generator), records."
                .into(),
        ),
    }
}
fn json_code(s: &str, second: &str) -> R<String> {
    fn infer(v: &Value) -> String {
        match v {
            Value::Null => "null".into(),
            Value::Bool(_) => "boolean".into(),
            Value::Number(_) => "number".into(),
            Value::String(_) => "string".into(),
            Value::Array(a) => {
                let mut types = BTreeSet::new();
                for v in a {
                    types.insert(infer(v));
                }
                format!(
                    "Array<{}>",
                    if types.is_empty() {
                        "unknown".into()
                    } else {
                        types.into_iter().collect::<Vec<_>>().join(" | ")
                    }
                )
            }
            Value::Object(m) => format!(
                "{{ {} }}",
                m.iter()
                    .map(|(k, v)| format!("{}: {};", Value::String(k.clone()), infer(v)))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        }
    }
    let name = if second.trim().is_empty() {
        "Root"
    } else {
        second.trim()
    };
    if name.len() > 64
        || !name.chars().enumerate().all(|(i, c)| {
            c == '_' || c == '$' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()
        })
    {
        return Err("Second field is a TypeScript type name (default Root). Swift generation is not implemented.".into());
    }
    let v = parse_json(s)?;
    Ok(format!(
        "// Inferred sample types. Review optionality and large-number handling.\nexport type {name} = {};",
        infer(&v)
    ))
}
fn sql_insert(s: &str, second: &str) -> R<String> {
    let name = if second.trim().is_empty() {
        "records"
    } else {
        second.trim()
    };
    let ident = |s: &str| -> R<String> {
        if s.is_empty() || s.len() > 256 || s.chars().any(char::is_control) {
            return Err("SQL identifiers need 1–256 bytes with no controls.".into());
        }
        Ok(format!("\"{}\"", s.replace('"', "\"\"")))
    };
    let table = ident(name)?;
    let v = parse_json(s)?;
    let rows = if let Value::Array(a) = v { a } else { vec![v] };
    if rows.is_empty() || rows.len() > 5000 {
        return Err("Use 1–5,000 JSON objects.".into());
    }
    let mut columns = BTreeSet::new();
    for v in &rows {
        let m = v.as_object().ok_or("Each SQL record must be an object.")?;
        if m.is_empty() {
            return Err("SQL records cannot be empty.".into());
        }
        columns.extend(m.keys().cloned())
    }
    if columns.len() > 500 {
        return Err("Use at most 500 SQL columns.".into());
    }
    let columns: Vec<_> = columns.into_iter().collect();
    let mut out = format!(
        "-- PostgreSQL SQL generated locally; never executed.\nINSERT INTO {table} ({}) VALUES\n",
        columns
            .iter()
            .map(|s| ident(s))
            .collect::<R<Vec<_>>>()?
            .join(", ")
    );
    for (i, r) in rows.iter().enumerate() {
        let mut values = vec![];
        for k in &columns {
            values.push(match r.get(k).unwrap_or(&Value::Null) {
                Value::Null => "NULL".into(),
                Value::Bool(b) => {
                    if *b {
                        "TRUE".into()
                    } else {
                        "FALSE".into()
                    }
                }
                Value::Number(n) => n.to_string(),
                v => {
                    let s = scalar(v);
                    if s.contains('\0') {
                        return Err("SQL literals cannot contain NUL.".into());
                    }
                    format!("E'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
                }
            })
        }
        out.push_str(&format!(
            "({}){}",
            values.join(", "),
            if i + 1 == rows.len() { ";" } else { ",\n" }
        ));
        if out.len() > MAX_OUTPUT {
            return Err("SQL output exceeds 4 MB.".into());
        }
    }
    Ok(out)
}
fn xml_json(s: &str) -> R<String> {
    let upper = s.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err("XML DTDs and entity declarations are not accepted.".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        s,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES as u32,
        },
    )
    .map_err(|e| e.to_string())?;
    fn walk(n: roxmltree::Node<'_, '_>, depth: usize) -> R<Value> {
        if depth >= MAX_DEPTH {
            return Err("XML exceeds 64 nesting levels.".into());
        }
        if n.is_text() {
            return Ok(Value::String(n.text().unwrap_or_default().into()));
        }
        let mut children = vec![];
        for c in n.children().filter(|n| n.is_element() || n.is_text()) {
            children.push(walk(c, depth + 1)?)
        }
        let attrs: Vec<_> = n
            .attributes()
            .map(|a| json!({"name":a.name(),"namespace":a.namespace(),"value":a.value()}))
            .collect();
        Ok(
            json!({"name":n.tag_name().name(),"namespace":n.tag_name().namespace(),"attributes":attrs,"children":children}),
        )
    }
    pretty(&walk(doc.root_element(), 0)?)
}
fn env_file(s: &str, mode: &str) -> R<String> {
    fn valid(k: &str) -> bool {
        !k.is_empty()
            && k.bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
    }
    match mode.trim(){"json-to-env"=>{let v=parse_json(s)?;let m=v.as_object().ok_or("Expected a JSON object of string values.")?;let mut lines=vec![];for(k,v)in m{if !valid(k){return Err(format!("Invalid environment variable name: {k}"))}let v=v.as_str().ok_or("Environment JSON values must be strings; numbers/booleans are not silently coerced.")?;if v.contains(['\n','\r','\0','\'']){return Err("Literal .env export cannot represent newlines, NUL, or single quotes; edit these values explicitly.".into())}lines.push(format!("{k}='{v}'"))}Ok(lines.join("\n"))},""|"env-to-json"=>{let mut m=Map::new();for(i,l)in s.lines().enumerate(){let l=l.trim();if l.is_empty()||l.starts_with('#'){continue}let l=l.strip_prefix("export ").unwrap_or(l);let(k,v)=l.split_once('=').ok_or_else(||format!("Missing '=' at line {}",i+1))?;let k=k.trim();if !valid(k){return Err(format!("Invalid variable name at line {}",i+1))}let v=v.trim();let value=if v.starts_with(['\'','"']){let quote=v.chars().next().unwrap();if v.len()<2||!v.ends_with(quote){return Err(format!("Unterminated quote at line {}",i+1))}v[1..v.len()-1].to_owned()}else{v.to_owned()};if m.insert(k.into(),Value::String(value)).is_some(){return Err(format!("Duplicate variable: {k}"))}}pretty(&Value::Object(m))},_=>Err("Use env-to-json or json-to-env. Values remain literal; no interpolation or command execution.".into())}
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}
fn headers(s: &str) -> R<Value> {
    let mut start = None;
    let mut fields = vec![];
    for (i, line) in s.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        if line.contains('\r') || line.chars().any(|c| c.is_control() && c != '\t') {
            return Err("Header values cannot contain control characters.".into());
        }
        if line.starts_with([' ', '\t']) {
            return Err("Obsolete folded headers are not accepted.".into());
        }
        if i == 0
            && (line.starts_with("HTTP/")
                || line
                    .split_whitespace()
                    .last()
                    .is_some_and(|s| s.starts_with("HTTP/")))
        {
            start = Some(line);
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or("Headers need Name: value lines.")?;
        if !token(k) {
            return Err("Invalid HTTP header name.".into());
        }
        fields.push(json!({"name":k,"value":v.trim()}));
        if fields.len() > 1000 {
            return Err("Header inspection is limited to 1,000 fields.".into());
        }
    }
    Ok(
        json!({"startLine":start,"headers":fields,"note":"Ordered fields and duplicates preserved; no network request sent."}),
    )
}
fn http(s: &str) -> R<String> {
    if s.trim_start().starts_with("curl ") {
        return Err("cURL import is not implemented in this portable engine. Paste a raw HTTP request instead; no shell command or request was executed.".into());
    }
    let normalized = s.replace("\r\n", "\n");
    let (head, body) = normalized.split_once("\n\n").unwrap_or((&normalized, ""));
    let first = head.lines().next().ok_or("Enter a raw HTTP request.")?;
    let p: Vec<_> = first.split_whitespace().collect();
    if p.len() != 3 || !token(p[0]) || !p[2].starts_with("HTTP/") {
        return Err("First line must be METHOD target HTTP/1.1.".into());
    }
    let mut info = headers(head)?;
    info["method"] = json!(p[0]);
    info["target"] = json!(p[1]);
    info["body"] = json!(body);
    info["bodyBytes"] = json!(body.len());
    pretty(&info)
}
fn cookies(s: &str, mode: &str) -> R<String> {
    let mut out = vec![];
    if !matches!(mode.trim(), "" | "set-cookie" | "cookie") {
        return Err("Use set-cookie (default) or cookie.".into());
    }
    for line in s.lines().filter(|s| !s.trim().is_empty()) {
        if line.chars().any(char::is_control) {
            return Err("Cookie fields cannot contain control characters.".into());
        }
        let line = line
            .trim()
            .strip_prefix("Set-Cookie:")
            .or_else(|| line.trim().strip_prefix("Cookie:"))
            .unwrap_or(line)
            .trim();
        let parts: Vec<_> = line.split(';').map(str::trim).collect();
        if mode.trim() == "cookie" {
            for p in parts {
                let (k, v) = p.split_once('=').ok_or("Cookie pairs need name=value.")?;
                if !token(k) {
                    return Err("Invalid cookie name.".into());
                }
                out.push(json!({"name":k,"value":v}))
            }
        } else {
            let (k, v) = parts[0]
                .split_once('=')
                .ok_or("Set-Cookie begins with name=value.")?;
            if !token(k) {
                return Err("Invalid cookie name.".into());
            }
            let attrs: Vec<_> = parts[1..]
                .iter()
                .map(|p| {
                    let (k, v) = p
                        .split_once('=')
                        .map(|(k, v)| (k, Some(v)))
                        .unwrap_or((p, None));
                    json!({"name":k,"value":v})
                })
                .collect();
            out.push(json!({"name":k,"value":v,"attributes":attrs}))
        }
    }
    pretty(&Value::Array(out))
}
fn ssh_key(s: &str) -> R<String> {
    if s.contains("PRIVATE KEY") {
        return Err(
            "Only public OpenSSH keys are accepted. Private keys are not inspected.".into(),
        );
    }
    let p: Vec<_> = s.split_whitespace().collect();
    if p.len() < 2 {
        return Err("Expected 'ssh-ed25519 BASE64 comment' or an RSA/ECDSA public key.".into());
    }
    let bytes = b64::STANDARD.decode(p[1]).map_err(|e| e.to_string())?;
    fn field<'a>(b: &'a [u8], i: &mut usize) -> R<&'a [u8]> {
        if b.len().saturating_sub(*i) < 4 {
            return Err("Truncated SSH key length.".into());
        }
        let len = u32::from_be_bytes(b[*i..*i + 4].try_into().unwrap()) as usize;
        *i += 4;
        let end = i
            .checked_add(len)
            .filter(|n| *n <= b.len())
            .ok_or("Truncated SSH key field.")?;
        let v = &b[*i..end];
        *i = end;
        Ok(v)
    }
    let mut i = 0;
    let kind =
        std::str::from_utf8(field(&bytes, &mut i)?).map_err(|_| "SSH key type is not UTF-8.")?;
    if kind != p[0] {
        return Err("Outer and encoded SSH key types disagree.".into());
    }
    let detail=match kind{"ssh-ed25519"=>{let key=field(&bytes,&mut i)?;if key.len()!=32{return Err("Ed25519 public keys require exactly 32 bytes.".into())}json!({"keyBytes":32})},"ssh-rsa"=>{let exponent=field(&bytes,&mut i)?;let modulus=field(&bytes,&mut i)?;if exponent.is_empty()||modulus.is_empty()||exponent[0]&0x80!=0||modulus[0]&0x80!=0{return Err("RSA parameters must be positive SSH integers.".into())}let m=modulus.iter().position(|b|*b!=0).ok_or("RSA modulus is zero.")?;let bits=(modulus.len()-m-1)*8+(8-modulus[m].leading_zeros()as usize);json!({"modulusBits":bits,"exponentHex":hex(exponent)})},"ecdsa-sha2-nistp256"|"ecdsa-sha2-nistp384"|"ecdsa-sha2-nistp521"=>{let curve=std::str::from_utf8(field(&bytes,&mut i)?).map_err(|_|"Invalid curve name.")?;if kind!=format!("ecdsa-sha2-{curve}"){return Err("ECDSA curve and key type disagree.".into())}let key=field(&bytes,&mut i)?;let length=match curve{"nistp256"=>65,"nistp384"=>97,_=>133};if key.len()!=length||key.first()!=Some(&4){return Err("Expected an uncompressed EC public point of the correct length.".into())}json!({"curve":curve,"pointBytes":key.len(),"note":"Point encoding inspected; curve membership not verified."})},_=>return Err("Supported public keys: ssh-ed25519, ssh-rsa, ecdsa-sha2-nistp256/384/521. Certificates are not yet supported.".into())};
    if i != bytes.len() {
        return Err("Unexpected trailing bytes in SSH key.".into());
    }
    pretty(
        &json!({"type":kind,"fingerprint":format!("SHA256:{}",b64::STANDARD_NO_PAD.encode(Sha256::digest(&bytes))),"details":detail,"comment":p[2..].join(" "),"note":"Public-key inspection only; key ownership and signatures are not verified."}),
    )
}
fn uuid_inspect(s: &str) -> R<String> {
    let id = uuid::Uuid::parse_str(s.trim()).map_err(|e| e.to_string())?;
    let b = id.as_bytes();
    let v = id.get_version_num();
    let mut info = json!({"uuid":id.to_string(),"version":v,"variant":format!("{:?}",id.get_variant()),"bytes":hex(b),"nil":id.is_nil()});
    if v == 7 {
        let mut ms = 0i64;
        for b in &b[..6] {
            ms = (ms << 8) | *b as i64
        }
        if let Some(d) = DateTime::<Utc>::from_timestamp_millis(ms) {
            info["timestampUTC"] = json!(d.to_rfc3339())
        }
    } else if (v == 1 || v == 6)
        && let Some(ts) = id.get_timestamp()
    {
        let (secs, nanos) = ts.to_unix();
        if let Ok(secs) = i64::try_from(secs)
            && let Some(d) = DateTime::<Utc>::from_timestamp(secs, nanos)
        {
            info["timestampUTC"] = json!(d.to_rfc3339())
        }
    }
    pretty(&info)
}
fn parse_u64(s: &str) -> R<u64> {
    let s = s.trim();
    let (base, s) = if let Some(s) = s.strip_prefix("0x") {
        (16, s)
    } else if let Some(s) = s.strip_prefix("0b") {
        (2, s)
    } else if let Some(s) = s.strip_prefix("0o") {
        (8, s)
    } else {
        (10, s)
    };
    u64::from_str_radix(s, base)
        .map_err(|_| "Use an unsigned 64-bit decimal or prefixed 0x/0b/0o integer.".into())
}
fn bitwise(s: &str, second: &str) -> R<String> {
    let a = parse_u64(s)?;
    let p: Vec<_> = second.split_whitespace().collect();
    let op = p.first().copied().unwrap_or("NOT").to_ascii_uppercase();
    let b = p.get(1).map(|s| parse_u64(s)).transpose()?.unwrap_or(0);
    let width = p
        .get(2)
        .map(|s| s.parse::<u32>())
        .transpose()
        .map_err(|_| "Bit width must be 8, 16, 32, or 64.")?
        .unwrap_or(64);
    if ![8, 16, 32, 64].contains(&width) || p.len() > 3 {
        return Err("Use 'AND 0xCC 8', 'NOT', 'SHL 2 32', 'SHR', 'ROL', or 'ROR'.".into());
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    if a > mask || (matches!(op.as_str(), "AND" | "OR" | "XOR") && b > mask) {
        return Err("Operand exceeds selected word width.".into());
    }
    let n = match op.as_str() {
        "AND" => a & b,
        "OR" => a | b,
        "XOR" => a ^ b,
        "NOT" => !a,
        "SHL" | "SHR" | "ROL" | "ROR" => {
            if b >= width as u64 {
                return Err("Shift/rotation amount must be smaller than word width.".into());
            }
            let b = b as u32;
            match op.as_str() {
                "SHL" => a << b,
                "SHR" => a >> b,
                "ROL" => {
                    if b == 0 {
                        a
                    } else {
                        (a << b) | (a >> (width - b))
                    }
                }
                _ => {
                    if b == 0 {
                        a
                    } else {
                        (a >> b) | (a << (width - b))
                    }
                }
            }
        }
        _ => return Err("Unknown bitwise operation.".into()),
    } & mask;
    let signed = if width == 64 {
        n as i64 as i128
    } else if n & (1u64 << (width - 1)) != 0 {
        n as i128 - (1i128 << width)
    } else {
        n as i128
    };
    Ok(format!(
        "Unsigned: {n}\nSigned: {signed}\nHex: 0x{n:X}\nBinary: {n:0width$b}",
        width = width as usize
    ))
}
fn statistics(s: &str, mode: &str) -> R<String> {
    if !matches!(mode.trim(), "" | "population" | "sample") {
        return Err("Use population or sample deviation.".into());
    }
    let mut a = s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(number)
        .collect::<R<Vec<_>>>()?;
    if a.is_empty() || a.len() > 20_000 {
        return Err("Use 1–20,000 finite numbers.".into());
    }
    if mode.trim() == "sample" && a.len() < 2 {
        return Err("Sample deviation needs at least two values.".into());
    }
    let mut mean = 0.;
    let mut m2 = 0.;
    let mut sum = 0.;
    for (i, n) in a.iter().enumerate() {
        sum = finite(sum + n)?;
        let delta = finite(n - mean)?;
        mean = finite(mean + delta / (i + 1) as f64)?;
        m2 = finite(m2 + delta * (n - mean))?
    }
    a.sort_by(f64::total_cmp);
    let q = |p: f64| {
        let i = (a.len() - 1) as f64 * p;
        let lo = i.floor() as usize;
        let hi = i.ceil() as usize;
        a[lo] * (1. - i.fract()) + a[hi] * i.fract()
    };
    let variance = finite(m2 / (a.len() - usize::from(mode.trim() == "sample")) as f64)?;
    pretty(
        &json!({"count":a.len(),"sum":sum,"mean":mean,"min":a[0],"max":a[a.len()-1],"median":q(0.5),"p25":q(0.25),"p75":q(0.75),"p90":q(0.9),"variance":variance,"standardDeviation":variance.max(0.).sqrt(),"quantiles":"Linear interpolation between sorted values","deviation":if mode.trim()=="sample"{"sample"}else{"population"}}),
    )
}
fn date_math(s: &str, second: &str) -> R<String> {
    let d = parse_date(s)?;
    let p: Vec<_> = second.split_whitespace().collect();
    let amount = p
        .first()
        .map(|s| s.parse::<i64>())
        .transpose()
        .map_err(|_| "Date amount must be an integer.")?
        .unwrap_or(10);
    let unit = p.get(1).copied().unwrap_or("days");
    if p.len() > 2 || amount.unsigned_abs() > 100_000 {
        return Err("Use '10 days', '-3 months', '2 years', '5 weekdays', or '6 hours'; UTC only; amount limited to ±100,000.".into());
    }
    let result=match unit{"hours"=>d.checked_add_signed(Duration::hours(amount)),"days"=>d.checked_add_signed(Duration::days(amount)),"weeks"=>d.checked_add_signed(Duration::weeks(amount)),"months"|"years"=>{let months=amount.checked_mul(if unit=="years"{12}else{1}).ok_or("Date overflow.")?;let count=u32::try_from(months.unsigned_abs()).map_err(|_|"Month amount is too large.")?;if months<0{d.checked_sub_months(chrono::Months::new(count))}else{d.checked_add_months(chrono::Months::new(count))}},"weekdays"=>{let mut d=d;let mut left=amount.unsigned_abs();while left>0{d=d.checked_add_signed(Duration::days(amount.signum())).ok_or("Date overflow.")?;if d.weekday().num_days_from_monday()<5{left-=1}}Some(d)},_=>return Err("Supported UTC units: days, weeks, weekdays, months, years, hours. Named-zone/DST calendar arithmetic is not implemented.".into())}.ok_or("Date overflow.")?;
    Ok(format!(
        "{}\nUTC calendar arithmetic; weekdays exclude weekends only; month ends are clamped.",
        result.to_rfc3339()
    ))
}
fn aspect_ratio(s: &str, second: &str) -> R<String> {
    let s = s.replace(['×', ':'], "x");
    let (w, h) = s
        .split_once('x')
        .ok_or("Use width x height, e.g. 1920 x 1080.")?;
    let w = w
        .trim()
        .parse::<u64>()
        .map_err(|_| "Width must be an integer.")?;
    let h = h
        .trim()
        .parse::<u64>()
        .map_err(|_| "Height must be an integer.")?;
    if w == 0 || h == 0 || w > 1_000_000_000 || h > 1_000_000_000 {
        return Err("Dimensions must be 1–1,000,000,000.".into());
    }
    let (mut a, mut b) = (w, h);
    while b != 0 {
        (a, b) = (b, a % b)
    }
    let mut out = format!(
        "Reduced ratio: {}:{}\nDecimal ratio: {}",
        w / a,
        h / a,
        w as f64 / h as f64
    );
    if !second.trim().is_empty() {
        let p: Vec<_> = second.split_whitespace().collect();
        if p.len() != 2 {
            return Err("Target format: 'width 1280' or 'height 720'.".into());
        }
        let n = number(p[1])?;
        if n <= 0. || n > 1e9 {
            return Err("Target dimension must be positive and at most 1e9.".into());
        }
        let (w, h) = match p[0] {
            "width" => (n, n * h as f64 / w as f64),
            "height" => (n * w as f64 / h as f64, n),
            _ => return Err("Target dimension must be width or height.".into()),
        };
        out.push_str(&format!("\nTarget: {w} × {h}"))
    }
    Ok(out)
}
fn bezier(s: &str) -> R<String> {
    let s = match s.trim() {
        "linear" => "0,0,1,1",
        "ease" => "0.25,0.1,0.25,1",
        "ease-in" => "0.42,0,1,1",
        "ease-out" => "0,0,0.58,1",
        "ease-in-out" => "0.42,0,0.58,1",
        s => s,
    };
    let s = s
        .strip_prefix("cubic-bezier(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(s);
    let n = s.split(',').map(number).collect::<R<Vec<_>>>()?;
    if n.len() != 4
        || !(0.0..=1.0).contains(&n[0])
        || !(0.0..=1.0).contains(&n[2])
        || n.iter().any(|n| n.abs() > 1e6)
    {
        return Err(
            "Use x1,y1,x2,y2; x coordinates must be 0–1 and values bounded to ±1e6.".into(),
        );
    }
    Ok(format!(
        "cubic-bezier({}, {}, {}, {})",
        n[0], n[1], n[2], n[3]
    ))
}
fn box_shadow(s: &str, second: &str) -> R<String> {
    let c = parse_color(s)?;
    let n = if second.trim().is_empty() {
        vec![0., 8., 24., 0.]
    } else {
        second
            .split_whitespace()
            .map(number)
            .collect::<R<Vec<_>>>()?
    };
    if n.len() != 4 || n[2] < 0. || n.iter().any(|v| v.abs() > 10_000.) {
        return Err("Second field takes x y blur spread in pixels, e.g. '0 8 24 0'; blur nonnegative; values within ±10,000.".into());
    }
    Ok(format!(
        "box-shadow: {}px {}px {}px {}px {};",
        n[0],
        n[1],
        n[2],
        n[3],
        c.css()
    ))
}
fn text_table(s: &str, mode: &str) -> R<String> {
    let rows = csv_rows(s)?;
    match mode.trim() {
        "html" => {
            let mut out = "<table>\n<thead>\n".to_owned();
            for (i, r) in rows.iter().enumerate() {
                if i == 1 {
                    out.push_str("</thead>\n<tbody>\n")
                }
                let tag = if i == 0 { "th" } else { "td" };
                out.push_str("<tr>");
                for c in r {
                    out.push_str(&format!("<{tag}>{}</{tag}>", html_escape(c)))
                }
                out.push_str("</tr>\n")
            }
            out.push_str(if rows.len() > 1 {
                "</tbody>\n</table>"
            } else {
                "</thead>\n</table>"
            });
            Ok(out)
        }
        "plain" => {
            if rows.len() > 1000 || rows[0].len() > 30 {
                return Err("Aligned text is limited to 1,000 rows and 30 columns.".into());
            }
            let widths: Vec<_> = (0..rows[0].len())
                .map(|i| {
                    rows.iter()
                        .map(|r| r[i].chars().count())
                        .max()
                        .unwrap_or(0)
                        .min(1000)
                })
                .collect();
            if rows
                .iter()
                .flatten()
                .any(|s| s.contains(['\n', '\r']) || s.chars().count() > 1000)
            {
                return Err(
                    "Plain-text cells must be single-line and at most 1,000 code points.".into(),
                );
            }
            Ok(rows
                .iter()
                .map(|r| {
                    r.iter()
                        .enumerate()
                        .map(|(i, s)| format!("{}{}", s, " ".repeat(widths[i] - s.chars().count())))
                        .collect::<Vec<_>>()
                        .join(" | ")
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "" | "markdown" => {
            let mut out = vec![];
            for (i, r) in rows.iter().enumerate() {
                out.push(format!(
                    "| {} |",
                    r.iter()
                        .map(|s| s
                            .replace('\\', "\\\\")
                            .replace('|', "\\|")
                            .replace('\r', "")
                            .replace('\n', "<br>"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
                if i == 0 {
                    out.push(format!("| {} |", vec!["---"; r.len()].join(" | ")))
                }
            }
            Ok(out.join("\n"))
        }
        _ => Err("Use markdown, plain, or html.".into()),
    }
}

/// Useful default for the workbench's second input when loading a sample.
pub fn example_options(id: &str) -> &'static str {
    match id {
        "json" => "pretty",
        "compare" => "alpha\nnew\ngamma\nend",
        "regex" => r#"{"pattern":"([A-Z]+)-(\\d+)","replacement":"$2:$1"}"#,
        "time" => "America/New_York",
        "cron" => "2026-09-06T08:00:00Z",
        "convert" => "csv-json",
        "snippets" => r#"{"name":"Ada","selection":"welcome"}"#,
        "units" => "MiB",
        "contrast" => "#FFFFFF",
        "gradient" => "#F5BC89",
        "jsonPointer" => "/users/0/name",
        "jsonFlatten" => "flatten",
        "jsonCode" => "Root",
        "sqlInsert" => "users",
        "stringEscape" => "json",
        "extract" => "all",
        "listSet" => "Rust\nGo\nSwift",
        "hmac" => "Jefe",
        "jsonSchema" => {
            r#"{"type":"object","required":["name","age"],"properties":{"name":{"type":"string","minLength":1},"age":{"type":"integer","minimum":0}},"additionalProperties":false}"#
        }
        "jsonMerge" => r#"{"profile":{"city":"Singapore","old":null}}"#,
        "plist" => "plist-json",
        "sqlFormat" => r#"{"dialect":"PostgreSQL","keywords":"Uppercase"}"#,
        "jsonRedact" => "password,token,secret,api_key,email",
        "jsonLines" => "lines-to-array",
        "csvExplore" => r#"{"unique":true,"columns":["name","team"]}"#,
        "envFile" => "env-to-json",
        "cookies" => "set-cookie",
        "bitwise" => "AND 0b11001100 8",
        "statistics" => "population",
        "dateMath" => "10 days",
        "aspectRatio" => "width 1280",
        "boxShadow" => "0 8 24 0",
        "textTable" => "markdown",
        _ => "",
    }
}

/// Honest scope/option help; consumers should display this near the second input.
pub fn option_hint(id: &str) -> &'static str {
    match id {
        "json" => "pretty | minify | validate · rejects duplicate keys; exact numbers",
        "compare" => "Second text · exact line comparison, at most 1 million line pairs",
        "jwt" => "Inspection only · signature is never verified",
        "regex" => {
            "Pattern, or JSON {pattern,replacement,flags} · flags i/m/s · no lookaround/backreferences"
        }
        "url" => "Optional query edits: [{\"name\":\"q\",\"value\":\"hello\"},{\"name\":\"flag\"}]",
        "time" => {
            "Optional IANA zone or second timestamp · numeric input auto-detects seconds/milliseconds"
        }
        "cron" => {
            "Optional start timestamp · five fields, next five UTC runs within 366 days · UTC only"
        }
        "convert" => {
            "json-yaml | yaml-json | csv-json | json-csv | yaml-csv | csv-yaml · YAML aliases/tags unsupported"
        }
        "snippets" => {
            "JSON placeholder values · {{date}} and {{uuid}} built in; substitutions occur once"
        }
        "http" => "Raw HTTP request inspector only · no cURL import or request sending",
        "generate" => {
            "Input: uuid N | hex N | records N · N=1–1000 · hex is UUID-derived, not a password"
        }
        "calculator" => {
            "Arithmetic, powers, pi/e, sqrt/abs/sin/cos/tan/ln/log/exp/floor/ceil/round/min/max/pow · radians"
        }
        "units" => "Input: 1024 KiB; second: MiB · or input: 1024; second: KiB MiB",
        "numberBase" => {
            "Optional input base 2/8/10/16 · otherwise 0x/0b/0o prefixes auto-detected · exact 256-digit integers"
        }
        "color" => "HEX, rgb/rgba or hsl/hsla · comma-separated CSS components",
        "contrast" => "Second opaque color · default #FFFFFF · WCAG AA/AAA thresholds",
        "gradient" => "Second color · output is a two-stop 90-degree CSS linear gradient",
        "markdown" => {
            "Safe HTML output · raw HTML escaped, images omitted, unsafe link schemes removed"
        }
        "jsonPointer" => {
            "RFC 6901 pointer, e.g. /users/0/name · empty selects root; #/ fragments accepted"
        }
        "jsonFlatten" => {
            "flatten | unflatten · typed pointer entries preserve empty containers and arrays"
        }
        "jsonCode" => {
            "TypeScript type name, default Root · sample types only; Swift output unavailable"
        }
        "sqlInsert" => {
            "Table name, default records · quoted PostgreSQL output only; never executed"
        }
        "xmlJSON" => "Ordered XML tree with attributes/mixed text · DTDs/entities rejected",
        "unicode" => "Code points and UTF-8/UTF-16 encodings · names/normalization unavailable",
        "stringEscape" => "json | unescape (JSON) | shell | html · no code is executed",
        "extract" => "all | links | emails · unique matches; heuristic extraction",
        "listSet" => {
            "Second line list · stable-order union by default; blank lines and duplicates removed"
        }
        "semver" => {
            "Optional comparison version · otherwise sorts input lines by SemVer precedence"
        }
        "subnet" => "IPv4/prefix only · /31 point-to-point; /32 single host",
        "chmod" => {
            "Octal 0000–7777 or nine rwx characters · symbolic special-bit input unavailable"
        }
        "hmac" => "Ephemeral key · HMAC-SHA256 hex output · not stored by this engine",
        "jsonMerge" => "RFC 7396 merge patch JSON · null removes object fields",
        "jsonRedact" => {
            "Comma-separated field names · case-insensitive recursive replacement; review output before sharing"
        }
        "jsonLines" => "lines-to-array | array-to-lines · up to 10,000 records",
        "csvExplore" => {
            "Optional JSON {unique:true,contains:\"text\",columns:[\"name\"]} · complete CSV export"
        }
        "envFile" => {
            "env-to-json | json-to-env · strings remain literal, no interpolation; limited literal export"
        }
        "httpHeaders" => {
            "Raw header block · ordered duplicate fields preserved · no network access"
        }
        "cookies" => "set-cookie | cookie · values/attributes are inspected locally",
        "sshKey" => {
            "Public RSA/Ed25519/ECDSA only · fingerprint/encoding inspection, no signature verification"
        }
        "uuidInspect" => "UUID bytes, version/variant, and v1/v6/v7 timestamps",
        "bitwise" => {
            "AND/OR/XOR operand width | NOT | SHL/SHR/ROL/ROR amount width · width=8/16/32/64"
        }
        "statistics" => {
            "population | sample · linear-interpolated percentiles; finite numbers only"
        }
        "dateMath" => {
            "Amount unit, e.g. -3 months · days/weeks/weekdays/months/years/hours · UTC only"
        }
        "aspectRatio" => "Optional width 1280 or height 720 · input width x height",
        "bezier" => "Four comma-separated coordinates or linear/ease/ease-in/ease-out/ease-in-out",
        "boxShadow" => "x y blur spread in pixels · default 0 8 24 0 · outer shadow CSS",
        "textTable" => "markdown | plain | html · CSV input with header row",
        "certificate" => {
            "Up to 10 public PEM certificates, 64 KB DER each · offline inspection only; no trust, signature, hostname, revocation or chain validation"
        }
        "jsonSchema" => {
            "Second editor: bounded local JSON Schema 2020-12 subset · unsupported keywords/remote references rejected; exact decimal comparisons"
        }
        "plist" => {
            "Plist → JSON | JSON → Plist · typed tree preserves dates/data/integer/real; standard Apple DOCTYPE handled locally, custom DTDs/entities rejected"
        }
        "sqlFormat" => {
            "Options JSON: {dialect: PostgreSQL|SQLite|MySQL, keywords: Uppercase|Preserve} · formatting only, never validation/execution"
        }
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(id: &str, input: &str, second: &str) -> String {
        execute(id, input, second).unwrap_or_else(|e| panic!("{id}: {e}"))
    }
    #[test]
    fn catalog_has_51_unique_ids() {
        let tools = catalog();
        assert_eq!(tools.len(), 51);
        let ids: HashSet<_> = tools.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 51);
    }
    #[test]
    fn every_supported_example_runs() {
        for t in catalog() {
            let second = example_options(t.id);
            run(t.id, t.example, second);
        }
    }
    #[test]
    fn json_is_lossless_and_duplicate_strict() {
        let s = r#"{"id":900719925474099312345678901234567890,"tiny":1.234567890123456789e-100,"rocket":"🚀"}"#;
        let output = run("json", s, "minify");
        assert!(output.contains("900719925474099312345678901234567890"));
        assert!(output.contains("1.234567890123456789e-100"));
        assert_eq!(parse_json(&output).unwrap(), parse_json(s).unwrap());
        for s in [
            r#"{"x":1,"x":2}"#,
            r#"{"a":1,"\u0061":2}"#,
            "[1,]",
            "{\"x\":1,}",
            "01",
            "1.",
            "1e",
            "true false",
            "{\"x\":\"\\q\"}",
        ] {
            assert!(execute("json", s, "").is_err(), "{s}")
        }
    }
    #[test]
    fn json_depth_and_input_limits() {
        assert!(
            execute(
                "json",
                &format!("{}0{}", "[".repeat(70), "]".repeat(70)),
                ""
            )
            .is_err()
        );
        assert!(execute("unicode", &"é".repeat(250_001), "").is_err());
        assert!(execute("compare", &"a".repeat(300_000), &"b".repeat(300_000)).is_err());
        assert!(execute("json", &format!("[{}]", vec!["0"; 20_001].join(",")), "").is_err());
    }
    #[test]
    fn pointer_handles_fragments_and_escapes() {
        let s = r#"{"a/b":{"~x":["🚀"]},"é":1,"é":2}"#;
        assert_eq!(run("jsonPointer", s, "/a~1b/~0x/0"), "\"🚀\"");
        assert_eq!(run("jsonPointer", s, "#/%C3%A9"), "1");
        assert_eq!(run("jsonPointer", s, "/é"), "2");
        for p in ["/a~2b", "/a~1b/~0x/00", "#/%GG"] {
            assert!(execute("jsonPointer", s, p).is_err())
        }
    }
    #[test]
    fn flatten_roundtrips_empty_types_and_keys() {
        let s = r#"{"a/b":[{},[],null,{"~":"🚀"}],"0":{"1":false}}"#;
        let entries = run("jsonFlatten", s, "");
        let restored = run("jsonFlatten", &entries, "unflatten");
        assert_eq!(parse_json(s).unwrap(), parse_json(&restored).unwrap());
        assert!(
            execute(
                "jsonFlatten",
                r#"[{"path":"","type":"array"},{"path":"/2","type":"value","value":1}]"#,
                "unflatten"
            )
            .is_err()
        );
    }
    #[test]
    fn merge_patch_and_redaction() {
        assert_eq!(
            parse_json(&run(
                "jsonMerge",
                r#"{"a":{"b":1,"c":2},"d":3}"#,
                r#"{"a":{"b":null},"d":[1]}"#
            ))
            .unwrap(),
            json!({"a":{"c":2},"d":[1]})
        );
        assert_eq!(run("jsonMerge", r#"{"a":1}"#, "null"), "null");
        let s = run(
            "jsonRedact",
            r#"{"TOKEN":"secret","child":[{"email":"ada@example.com"}],"name":"Ada"}"#,
            "",
        );
        assert!(!s.contains("secret"));
        assert!(!s.contains("ada@example"));
        assert!(s.contains("Ada"));
    }
    #[test]
    fn ndjson_roundtrip() {
        let lines = "{\"id\":9007199254740993}\n\"🚀\"\nnull";
        let array = run("jsonLines", lines, "");
        assert_eq!(run("jsonLines", &array, "array-to-lines"), lines);
        assert!(execute("jsonLines", "{}\n{bad}", "").is_err());
    }
    #[test]
    fn csv_quoted_newlines_roundtrip() {
        let s = "name,note\r\n\"Ada, A\",\"line1\nline2\"\r\nBob,\"said \"\"hi\"\"\"\r\n";
        let rows = csv_rows(s).unwrap();
        assert_eq!(csv_rows(&csv_encode(&rows)).unwrap(), rows);
        let json = run("convert", s, "csv-json");
        assert_eq!(
            parse_json(&run(
                "convert",
                &run("convert", &json, "json-csv"),
                "csv-json"
            ))
            .unwrap(),
            parse_json(&json).unwrap()
        );
        for s in ["a,a\n1,2", "a,b\n1", "a\n\"bad", "a\n\"x\"q", "a\nb\"c"] {
            assert!(csv_rows(s).is_err(), "{s}")
        }
    }
    #[test]
    fn csv_explorer_preserves_complete_export() {
        let s = "name,team\nAda,P\nLin,D\nAda,P";
        assert_eq!(
            run(
                "csvExplore",
                s,
                r#"{"unique":true,"contains":"P","columns":["name"]}"#
            ),
            "name\nAda"
        );
    }
    #[test]
    fn yaml_exact_numbers_strings_and_bounds() {
        let s = r#"{"id":90071992547409931234567890,"word":"yes","on":"on","active":true,"nothing":null,"items":["001",2.5]}"#;
        let yaml = run("convert", s, "json-yaml");
        assert_eq!(
            parse_json(s).unwrap(),
            parse_json(&run("convert", &yaml, "yaml-json")).unwrap()
        );
        assert_eq!(
            yaml_json("yes: on\nflag: true\ndate: 2026-09-06").unwrap(),
            json!({"yes":"on","flag":true,"date":"2026-09-06"})
        );
        for s in [
            "a: 1\na: 2",
            "first: &x [1,2]\nsecond: *x",
            "---\na: 1\n---\nb: 2",
            "a: !!str hi",
        ] {
            assert!(yaml_json(s).is_err(), "{s}")
        }
    }
    #[test]
    fn regex_unicode_groups_replacement_and_limits() {
        let out = run(
            "regex",
            "🚀 BOX-12 API-34",
            r#"{"pattern":"([A-Z]+)-(\\d+)","replacement":"$2:$1"}"#,
        );
        let v = parse_json(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["matches"][0]["startByte"], 5);
        assert_eq!(v["replaced"], "🚀 12:BOX 34:API");
        assert!(execute("regex", "a", "(?=a)").is_err());
        assert!(execute("regex", "a", "[").is_err());
        assert!(execute("regex", &"a".repeat(2000), "a").is_err());
        assert_eq!(
            parse_json(&run("regex", &format!("{}!", "a".repeat(20_000)), "(a+)+$")).unwrap()["count"],
            0
        );
    }
    #[test]
    fn url_preserves_duplicate_flags_plus() {
        let v = parse_json(&run(
            "url",
            "https://example.com:8443/a?q=a+b&tag=one&tag=two&flag#hello",
            "",
        ))
        .unwrap();
        assert_eq!(v["query"][0]["value"], "a+b");
        assert_eq!(v["query"][3]["hasValue"], false);
        let v = parse_json(&run(
            "url",
            "https://example.com/",
            r#"[{"name":"x","value":"a & b"},{"name":"flag"}]"#,
        ))
        .unwrap();
        assert!(v["url"].as_str().unwrap().ends_with("?x=a%20%26%20b&flag"));
        assert!(execute("url", "https://example.com/%GG", "").is_err());
    }
    #[test]
    fn jwt_never_claims_verification() {
        let out = run(
            "jwt",
            catalog().iter().find(|t| t.id == "jwt").unwrap().example,
            "",
        );
        assert!(out.contains("SIGNATURE NOT VERIFIED"));
        assert!(out.contains("expired"));
        assert!(execute("jwt", "a.b.c.d.e", "").is_err());
    }
    #[test]
    fn time_and_cron() {
        assert!(run("time", "0", "").contains("1970-01-01"));
        assert!(run("time", "1700000000000", "").contains("Unix seconds: 1700000000"));
        assert!(run("time", "0", "3600").contains("1.000000 hours"));
        let out = run("cron", "0 9 7 * SUN", "2026-09-06T08:00:00Z");
        assert!(out.contains("2026-09-06T09:00:00+00:00\n2026-09-07T09:00:00+00:00"));
        for s in [
            "* * * * * *",
            "*/0 * * * *",
            "60 * * * *",
            "0 0 * BAD *",
            "0 0 * * FRI-MON",
        ] {
            assert!(execute("cron", s, "").is_err())
        }
    }
    #[test]
    fn calculator_precedence_domains_and_limits() {
        for (s, v) in [
            ("sqrt(144)+2^3", "20"),
            ("-2^2", "-4"),
            ("2^3^2", "512"),
            ("pow(2,3)+max(1,2)", "10"),
            ("1e3/4", "250"),
        ] {
            assert_eq!(run("calculator", s, ""), v)
        }
        for s in ["1/0", "sqrt(-1)", "pow(1)", "1+", "é", "1..2"] {
            assert!(execute("calculator", s, "").is_err(), "{s}")
        }
        assert!(
            execute(
                "calculator",
                &format!("{}1{}", "(".repeat(100), ")".repeat(100)),
                ""
            )
            .is_err()
        );
    }
    #[test]
    fn exact_integer_and_units() {
        assert!(run("numberBase", "9007199254740993", "").contains("Hex: 20000000000001"));
        assert!(
            run("numberBase", "-0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", "")
                .contains("-340282366920938463463374607431768211455")
        );
        assert!(execute("numberBase", "0x123", "2").is_err());
        assert_eq!(run("units", "1024 KiB", "MiB"), "1 MiB");
        assert_eq!(run("units", "32 F", "C"), "0 C");
        assert!(execute("units", "-274 C", "K").is_err());
        assert!(execute("units", "1 m", "kg").is_err());
    }
    #[test]
    fn color_contrast_and_css() {
        assert!(run("color", "hsl(0,100%,50%)", "").contains("#FF0000"));
        assert!(run("color", "#f008", "").contains("#FF000088"));
        assert!(run("contrast", "#000", "#fff").contains("21.000:1"));
        assert!(execute("color", "rgb(999,0,0)", "").is_err());
        assert!(execute("contrast", "#0008", "#fff").is_err());
        assert!(execute("bezier", "2,0,1,1", "").is_err());
        assert!(execute("boxShadow", "#000", "0 0 -1 0").is_err());
    }
    #[test]
    fn safe_markup_and_xml() {
        let out = run(
            "markdown",
            "<script>alert(1)</script>\n\n![x](https://example.com/x.png)\n\n[bad](javascript:alert(1))",
            "",
        );
        assert!(!out.contains("<script>"));
        assert!(!out.contains("<img"));
        assert!(!out.contains("href=\"javascript:"));
        let v = parse_json(&run("xmlJSON", "<a x=\"1\">before<b/>after</a>", "")).unwrap();
        assert_eq!(v["children"][0], "before");
        assert_eq!(v["children"][2], "after");
        assert!(
            execute(
                "xmlJSON",
                "<!DOCTYPE x [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><x>&x;</x>",
                ""
            )
            .is_err()
        );
    }
    #[test]
    fn snippets_substitute_once_and_generators_unique() {
        assert_eq!(run("snippets", "Hello {{name}}", ""), "Hello {{name}}");
        assert_eq!(
            run(
                "snippets",
                "Hello {{name}} {{selection}}",
                r#"{"name":"Ada","selection":"{{name}}"}"#
            ),
            "Hello Ada {{name}}"
        );
        let s = run("generate", "uuid 40", "");
        assert_eq!(s.lines().collect::<HashSet<_>>().len(), 40);
        assert!(s.lines().all(|s| uuid::Uuid::parse_str(s).is_ok()));
        assert!(execute("generate", "uuid 1001", "").is_err());
    }
    #[test]
    fn env_values_remain_literal() {
        let v = parse_json(&run(
            "envFile",
            "GREETING='Hello ${USER}'\nCMD=$(touch nope)",
            "",
        ))
        .unwrap();
        assert_eq!(v["GREETING"], "Hello ${USER}");
        assert_eq!(v["CMD"], "$(touch nope)");
        assert!(execute("envFile", "X=a\nX=b", "").is_err());
        assert_eq!(
            parse_json(&run(
                "envFile",
                &run("envFile", r#"{"X":"001","Y":"${USER}"}"#, "json-to-env"),
                ""
            ))
            .unwrap(),
            json!({"X":"001","Y":"${USER}"})
        );
    }
    #[test]
    fn sql_generation_quotes_identifiers_values() {
        let s = run(
            "sqlInsert",
            r#"[{"id":9007199254740993,"name":"O'Reilly"}]"#,
            "weird\"table",
        );
        assert!(s.contains("\"weird\"\"table\""));
        assert!(s.contains("E'O''Reilly'"));
        assert!(s.contains("9007199254740993"));
        assert!(execute("sqlInsert", r#"{"x":"\u0000"}"#, "").is_err());
    }
    #[test]
    fn protocol_inspection_preserves_order_and_duplicates() {
        let v = parse_json(&run(
            "httpHeaders",
            "HTTP/1.1 200 OK\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2",
            "",
        ))
        .unwrap();
        assert_eq!(v["headers"].as_array().unwrap().len(), 2);
        assert!(execute("httpHeaders", "Bad Name: x", "").is_err());
        assert!(execute("http", "curl https://example.com", "").is_err());
        assert_eq!(
            parse_json(&run("cookies", "a=1; a=2", "cookie"))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn ssh_and_uuid_are_local_inspectors() {
        let t = catalog().into_iter().find(|t| t.id == "sshKey").unwrap();
        assert!(run("sshKey", t.example, "").contains("SHA256:"));
        assert!(execute("sshKey", "-----BEGIN PRIVATE KEY-----", "").is_err());
        assert!(execute("sshKey", "ssh-ed25519 AAAA", "").is_err());
        let v = parse_json(&run(
            "uuidInspect",
            "017f22e2-79b0-7cc3-98c4-dc0c0c07398f",
            "",
        ))
        .unwrap();
        assert_eq!(v["version"], 7);
        assert!(
            v["timestampUTC"]
                .as_str()
                .unwrap()
                .starts_with("2022-02-22")
        );
    }
    #[test]
    fn hmac_known_rfc_vector() {
        assert_eq!(
            run("hmac", "what do ya want for nothing?", "Jefe"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }
    #[test]
    fn bitwise_statistics_dates_dimensions() {
        assert!(run("bitwise", "0b10101010", "AND 0b11001100 8").contains("Unsigned: 136"));
        assert!(run("bitwise", "0b10000001", "ROL 1 8").contains("Unsigned: 3"));
        assert!(execute("bitwise", "1", "SHL 64 64").is_err());
        let v = parse_json(&run("statistics", "1,2,3,4", "")).unwrap();
        assert_eq!(v["mean"], 2.5);
        assert_eq!(v["variance"], 1.25);
        assert!(run("dateMath", "2024-01-31", "1 months").contains("2024-02-29"));
        assert!(run("dateMath", "2026-09-11", "1 weekdays").contains("2026-09-14"));
        assert!(run("aspectRatio", "1920 x 1080", "width 1280").contains("1280 × 720"));
    }
    #[test]
    fn malformed_utf8_inputs_never_panic() {
        for id in catalog().iter().map(|t| t.id) {
            for s in ["🚀", "\0", "é\u{301}", "\"\\", ""] {
                let _ = execute(id, s, "🚀");
            }
        }
    }
}
