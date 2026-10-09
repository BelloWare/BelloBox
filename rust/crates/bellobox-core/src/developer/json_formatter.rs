//! Formatter-local JSON semantics. Numbers retain their source spelling; object
//! comparisons use NFC keys without changing the spelling emitted to the user.
//! This models the inspected Swift source, not a universal runtime parity claim.
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use unicode_normalization::UnicodeNormalization;

const INPUT_LIMIT: usize = 500_000;
const OUTPUT_LIMIT: usize = 4_000_000;
const NODE_LIMIT: usize = 20_000;
const DEPTH_LIMIT: usize = 64;
type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
enum Node<'a> {
    Literal(&'a str),
    String(String),
    Array(Vec<Node<'a>>),
    // Normalized comparison key -> (original decoded key, value).
    Object(BTreeMap<String, (String, Node<'a>)>),
}

pub(super) fn execute(input: &str, option: &str) -> Result<String> {
    if input.len().saturating_add(option.len()) > INPUT_LIMIT {
        return Err("Inputs exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    let pretty = match option.trim() {
        "" | "pretty" => true,
        "minify" | "validate" => false,
        _ => return Err("Use pretty, minify, or validate in the second field.".into()),
    };
    let value = Parser::parse(input)?;
    if option.trim() == "validate" {
        return Ok("Valid JSON.".into());
    }
    render(&value, pretty, OUTPUT_LIMIT)
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    nodes: usize,
    cancelled: Option<&'a AtomicBool>,
}
impl<'a> Parser<'a> {
    fn parse(input: &'a str) -> Result<Node<'a>> {
        Self::parse_cancellable(input, None)
    }
    fn parse_cancellable(input: &'a str, cancelled: Option<&'a AtomicBool>) -> Result<Node<'a>> {
        if input.len() > INPUT_LIMIT {
            return Err("JSON exceeds 500,000 UTF-8 bytes.".into());
        }
        let mut p = Self {
            input,
            pos: 0,
            nodes: 0,
            cancelled,
        };
        let node = p.value(0)?;
        p.ws()?;
        p.check_cancelled()?;
        if p.pos != input.len() {
            return Err(p.error("Unexpected trailing content"));
        }
        Ok(node)
    }
    fn check_cancelled(&self) -> Result<()> {
        if let Some(cancelled) = self.cancelled {
            check_comparison_cancelled(cancelled)?;
        }
        Ok(())
    }
    fn byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }
    fn ws(&mut self) -> Result<()> {
        while self.byte().is_some_and(|b| b" \t\r\n".contains(&b)) {
            if self.pos.is_multiple_of(1024) {
                self.check_cancelled()?;
            }
            self.pos += 1;
        }
        Ok(())
    }
    fn eat(&mut self, b: u8) -> Result<bool> {
        self.ws()?;
        if self.byte() == Some(b) {
            self.pos += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn error(&self, message: &str) -> String {
        // Byte slicing, rather than str slicing, remains safe even when a
        // malformed token leaves the cursor inside a multibyte scalar.
        let prefix = &self.input.as_bytes()[..self.pos];
        let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
        let column = prefix
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(self.pos + 1, |i| self.pos - i);
        format!("{message} at line {line}, byte column {column}.")
    }
    fn string(&mut self) -> Result<String> {
        self.ws()?;
        let start = self.pos;
        if self.byte() != Some(b'"') {
            return Err(self.error("Expected a quoted property name"));
        }
        self.pos += 1;
        let mut escaped = false;
        while let Some(b) = self.byte() {
            if self.pos.is_multiple_of(1024) {
                self.check_cancelled()?;
            }
            self.pos += 1;
            if b < 32 {
                return Err(self.error("Unescaped control character"));
            }
            if !escaped && b == b'"' {
                // Quote boundaries are necessarily UTF-8 scalar boundaries.
                return serde_json::from_str(&self.input[start..self.pos])
                    .map_err(|_| self.error("Invalid JSON string or escape"));
            }
            if escaped {
                escaped = false;
            } else {
                escaped = b == b'\\';
            }
        }
        Err(self.error("Unclosed string"))
    }
    fn digits(&mut self) -> Result<()> {
        let start = self.pos;
        while self.byte().is_some_and(|b| b.is_ascii_digit()) {
            if self.pos.is_multiple_of(1024) {
                self.check_cancelled()?;
            }
            self.pos += 1;
        }
        if self.pos == start {
            Err(self.error("Expected a digit"))
        } else {
            Ok(())
        }
    }
    fn number(&mut self) -> Result<Node<'a>> {
        let start = self.pos;
        if self.byte() == Some(b'-') {
            self.pos += 1;
        }
        if self.byte() == Some(b'0') {
            self.pos += 1;
        } else {
            self.digits()?;
        }
        if self.byte() == Some(b'.') {
            self.pos += 1;
            self.digits()?;
        }
        if matches!(self.byte(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.byte(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.digits()?;
        }
        Ok(Node::Literal(&self.input[start..self.pos]))
    }
    fn value(&mut self, depth: usize) -> Result<Node<'a>> {
        self.check_cancelled()?;
        self.ws()?;
        self.nodes += 1;
        if depth >= DEPTH_LIMIT || self.nodes > NODE_LIMIT {
            return Err("JSON exceeds 64 levels or 20,000 values.".into());
        }
        match self.byte() {
            Some(b'"') => self.string().map(Node::String),
            Some(b'{') => {
                self.pos += 1;
                let mut fields = BTreeMap::new();
                if self.eat(b'}')? {
                    return Ok(Node::Object(fields));
                }
                loop {
                    let key = self.string()?;
                    let mut normalized = String::new();
                    for (index, scalar) in key.nfc().enumerate() {
                        if index.is_multiple_of(1024) {
                            self.check_cancelled()?;
                        }
                        normalized.push(scalar);
                    }
                    if fields.contains_key(&normalized) {
                        return Err(self.error("Duplicate JSON property"));
                    }
                    if !self.eat(b':')? {
                        return Err(self.error("Expected ':'"));
                    }
                    let value = self.value(depth + 1)?;
                    fields.insert(normalized, (key, value));
                    if self.eat(b'}')? {
                        return Ok(Node::Object(fields));
                    }
                    if !self.eat(b',')? {
                        return Err(self.error("Expected ',' or '}'"));
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                if self.eat(b']')? {
                    return Ok(Node::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.eat(b']')? {
                        return Ok(Node::Array(items));
                    }
                    if !self.eat(b',')? {
                        return Err(self.error("Expected ',' or ']'"));
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b't' | b'f' | b'n') => {
                let word = match self.byte() {
                    Some(b't') => "true",
                    Some(b'f') => "false",
                    _ => "null",
                };
                if !self.input.as_bytes()[self.pos..].starts_with(word.as_bytes()) {
                    return Err(self.error("Expected a JSON value"));
                }
                self.pos += word.len();
                Ok(Node::Literal(word))
            }
            _ => Err(self.error("Expected a JSON value")),
        }
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other(
                "Result exceeds output limit; nothing was truncated.",
            ));
        }
        // Grow geometrically, capped before allocation. No full unbounded
        // intermediate serialization is constructed.
        let needed = self.bytes.len() + bytes.len();
        if needed > self.bytes.capacity() {
            let capacity = needed
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl BoundedOutput {
    fn text(&mut self, s: &str) -> io::Result<()> {
        self.write_all(s.as_bytes())
    }
    fn quote(&mut self, s: &str) -> io::Result<()> {
        serde_json::to_writer(self, s).map_err(io::Error::other)
    }
    fn indent(&mut self, depth: usize) -> io::Result<()> {
        for _ in 0..depth {
            self.text("  ")?;
        }
        Ok(())
    }
    fn node(&mut self, node: &Node<'_>, pretty: bool, depth: usize) -> io::Result<()> {
        match node {
            Node::Literal(s) => self.text(s),
            Node::String(s) => self.quote(s),
            Node::Array(items) => {
                self.text("[")?;
                for (i, item) in items.iter().enumerate() {
                    self.entry(i, pretty, depth + 1)?;
                    self.node(item, pretty, depth + 1)?;
                }
                self.close("]", !items.is_empty(), pretty, depth)
            }
            Node::Object(fields) => {
                self.text("{")?;
                for (i, (key, value)) in fields.values().enumerate() {
                    self.entry(i, pretty, depth + 1)?;
                    self.quote(key)?;
                    self.text(if pretty { ": " } else { ":" })?;
                    self.node(value, pretty, depth + 1)?;
                }
                self.close("}", !fields.is_empty(), pretty, depth)
            }
        }
    }
    fn entry(&mut self, i: usize, pretty: bool, depth: usize) -> io::Result<()> {
        if i != 0 {
            self.text(",")?;
        }
        if pretty {
            self.text("\n")?;
            self.indent(depth)?;
        }
        Ok(())
    }
    fn close(&mut self, s: &str, nonempty: bool, pretty: bool, depth: usize) -> io::Result<()> {
        if nonempty && pretty {
            self.text("\n")?;
            self.indent(depth)?;
        }
        self.text(s)
    }
}
fn render(value: &Node<'_>, pretty: bool, limit: usize) -> Result<String> {
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        limit,
    };
    output.node(value, pretty, 0).map_err(|e| e.to_string())?;
    String::from_utf8(output.bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "json_formatter_prototype_tests.rs"]
mod tests;

fn check_comparison_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err("Comparison cancelled.".into())
    } else {
        Ok(())
    }
}

/// Narrow comparison-only projection. The formatter and other JSON routes retain
/// their existing parse/render contracts. Each side's leaf bytes and path are
/// bounded before append; repeated long paths cannot expand without a limit.
pub(super) fn comparison_fields(
    input: &str,
    token_limit: usize,
    output_limit: usize,
    cancelled: &AtomicBool,
) -> Result<Vec<String>> {
    check_comparison_cancelled(cancelled)?;
    let node = Parser::parse_cancellable(input, Some(cancelled))?;
    let mut fields = Vec::new();
    let mut remaining = output_limit;
    let mut path = BoundedOutput {
        bytes: Vec::new(),
        limit: output_limit,
    };
    path.text("$").map_err(|e| e.to_string())?;
    flatten_fields(
        &node,
        &mut path,
        &mut fields,
        token_limit,
        &mut remaining,
        cancelled,
    )?;
    Ok(fields)
}
fn flatten_fields(
    node: &Node<'_>,
    path: &mut BoundedOutput,
    fields: &mut Vec<String>,
    token_limit: usize,
    remaining: &mut usize,
    cancelled: &AtomicBool,
) -> Result<()> {
    check_comparison_cancelled(cancelled)?;
    let path_length = path.bytes.len();
    match node {
        Node::Object(values) if !values.is_empty() => {
            for (key, value) in values.values() {
                path.text("[")
                    .and_then(|()| path.quote(key))
                    .and_then(|()| path.text("]"))
                    .map_err(|e| e.to_string())?;
                flatten_fields(value, path, fields, token_limit, remaining, cancelled)?;
                path.bytes.truncate(path_length);
            }
        }
        Node::Array(values) if !values.is_empty() => {
            for (index, value) in values.iter().enumerate() {
                write!(path, "[{index}]").map_err(|e| e.to_string())?;
                flatten_fields(value, path, fields, token_limit, remaining, cancelled)?;
                path.bytes.truncate(path_length);
            }
        }
        _ => {
            if fields.len() == token_limit {
                return Err("Compare up to 8,000 combined lines, words, or JSON fields. Narrow the selection first.".into());
            }
            let mut output = BoundedOutput {
                bytes: Vec::new(),
                limit: *remaining,
            };
            output
                .write_all(&path.bytes)
                .and_then(|()| output.text(" = "))
                .and_then(|()| output.node(node, false, 0))
                .map_err(|e| e.to_string())?;
            *remaining -= output.bytes.len();
            fields.push(String::from_utf8(output.bytes).map_err(|e| e.to_string())?);
        }
    }
    check_comparison_cancelled(cancelled)
}

#[cfg(test)]
mod comparison_projection_tests {
    use super::*;

    #[test]
    fn projection_is_available_to_standalone_formatter_regression_harness() {
        let cancel = AtomicBool::new(false);
        assert_eq!(
            comparison_fields(r#"{"a.b":1E+02,"a":{"b":[]}}"#, 2, 100, &cancel).unwrap(),
            [r#"$["a"]["b"] = []"#, r#"$["a.b"] = 1E+02"#]
        );
        assert!(comparison_fields("[1,2]", 1, 100, &cancel).is_err());
        assert!(comparison_fields("[1,2]", 2, 1, &cancel).is_err());
    }

    #[test]
    fn parser_scan_entries_honor_cancellation() {
        let cancel = AtomicBool::new(true);
        for input in [" ", "123"] {
            let mut parser = Parser {
                input,
                pos: 0,
                nodes: 0,
                cancelled: Some(&cancel),
            };
            let result = if input == " " {
                parser.ws()
            } else {
                parser.digits()
            };
            assert_eq!(result.unwrap_err(), "Comparison cancelled.");
        }
        let input = format!("\"{}\"", "a".repeat(2048));
        let mut parser = Parser {
            input: &input,
            pos: 0,
            nodes: 0,
            cancelled: Some(&cancel),
        };
        assert_eq!(parser.string().unwrap_err(), "Comparison cancelled.");
    }
}
