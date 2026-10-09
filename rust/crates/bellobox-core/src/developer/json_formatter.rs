//! Formatter-local JSON semantics. Numbers retain their source spelling; object
//! comparisons use NFC keys without changing the spelling emitted to the user.
//! This models the inspected Swift source, not a universal runtime parity claim.
use std::collections::BTreeMap;
use std::io::{self, Write};
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
}
impl<'a> Parser<'a> {
    fn parse(input: &'a str) -> Result<Node<'a>> {
        if input.len() > INPUT_LIMIT {
            return Err("JSON exceeds 500,000 UTF-8 bytes.".into());
        }
        let mut p = Self {
            input,
            pos: 0,
            nodes: 0,
        };
        let node = p.value(0)?;
        p.ws();
        if p.pos != input.len() {
            return Err(p.error("Unexpected trailing content"));
        }
        Ok(node)
    }
    fn byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }
    fn ws(&mut self) {
        while self.byte().is_some_and(|b| b" \t\r\n".contains(&b)) {
            self.pos += 1;
        }
    }
    fn eat(&mut self, b: u8) -> bool {
        self.ws();
        if self.byte() == Some(b) {
            self.pos += 1;
            true
        } else {
            false
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
        self.ws();
        let start = self.pos;
        if self.byte() != Some(b'"') {
            return Err(self.error("Expected a quoted property name"));
        }
        self.pos += 1;
        let mut escaped = false;
        while let Some(b) = self.byte() {
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
        self.ws();
        self.nodes += 1;
        if depth >= DEPTH_LIMIT || self.nodes > NODE_LIMIT {
            return Err("JSON exceeds 64 levels or 20,000 values.".into());
        }
        match self.byte() {
            Some(b'"') => self.string().map(Node::String),
            Some(b'{') => {
                self.pos += 1;
                let mut fields = BTreeMap::new();
                if self.eat(b'}') {
                    return Ok(Node::Object(fields));
                }
                loop {
                    let key = self.string()?;
                    let normalized: String = key.nfc().collect();
                    if fields.contains_key(&normalized) {
                        return Err(self.error("Duplicate JSON property"));
                    }
                    if !self.eat(b':') {
                        return Err(self.error("Expected ':'"));
                    }
                    let value = self.value(depth + 1)?;
                    fields.insert(normalized, (key, value));
                    if self.eat(b'}') {
                        return Ok(Node::Object(fields));
                    }
                    if !self.eat(b',') {
                        return Err(self.error("Expected ',' or '}'"));
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                if self.eat(b']') {
                    return Ok(Node::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.eat(b']') {
                        return Ok(Node::Array(items));
                    }
                    if !self.eat(b',') {
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
