//! The same deliberately bounded, offline JSON Schema 2020-12 subset as the
//! Swift workbench. This is not a general-purpose or remotely resolving validator.
//! Unknown constraints fail schema preflight, including in unused branches.
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

type Result<T> = std::result::Result<T, String>;
const MAX_DEPTH: usize = 64;
const MAX_WORK: usize = 50_000;
const MAX_ISSUES: usize = 100;
const MAX_NUMERIC_BYTES: usize = 4_096;
const MAX_EXPONENT: i64 = 1_000_000;
const MAX_OUTPUT: usize = 4_000_000;
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const SUPPORTED: &[&str] = &[
    "$schema",
    "$defs",
    "$ref",
    "$comment",
    "title",
    "description",
    "default",
    "examples",
    "readOnly",
    "writeOnly",
    "deprecated",
    "type",
    "enum",
    "const",
    "properties",
    "required",
    "additionalProperties",
    "minProperties",
    "maxProperties",
    "items",
    "prefixItems",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
];
const TYPES: &[&str] = &[
    "null", "boolean", "object", "array", "number", "integer", "string",
];
const COUNTS: &[&str] = &[
    "minProperties",
    "maxProperties",
    "minItems",
    "maxItems",
    "minLength",
    "maxLength",
];
const NUMERIC_BOUNDS: &[&str] = &["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"];
const VALID: &str = "Valid against this schema.\n\nSupported: types, enum/const, required/properties, object/array/string bounds, numeric bounds, prefixItems/items, uniqueItems, allOf/anyOf/oneOf/not, and local $defs/$ref.\n\nUnsupported keywords are rejected, never ignored. No remote resources are fetched.";

pub(super) fn validate(input: &str, schema: &str) -> Result<String> {
    if schema.trim().is_empty() {
        return Err("Add a JSON schema in the second editor, or load the example. Remote references and unsupported keywords are reported.".into());
    }
    if input.len().saturating_add(schema.len()) > super::MAX_INPUT {
        return Err("Inputs exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    let value = super::parse_json(input)?;
    let schema = super::parse_json(schema)?;
    let mut checker = Checker::new(&schema);
    checker.lint(&schema, 0)?;
    let issues = checker.check(&value, &schema, "", 0)?;
    let output = if issues.is_empty() {
        VALID.into()
    } else {
        let mut output = issues.join("\n");
        if issues.len() == MAX_ISSUES {
            output.push_str("\nShowing the first 100 issues.");
        }
        output
    };
    if output.len() > MAX_OUTPUT {
        Err("Result exceeds 4 MB; use a smaller input. Nothing was truncated.".into())
    } else {
        Ok(output)
    }
}

struct Checker<'a> {
    root: &'a Value,
    remaining: usize,
    references: HashMap<&'a str, &'a Value>,
    // References can form a graph. Visit each target during preflight, without
    // recursively expanding cycles. Actual evaluation still has a depth limit.
    linted: HashSet<*const Value>,
}

impl<'a> Checker<'a> {
    fn new(root: &'a Value) -> Self {
        Self {
            root,
            remaining: MAX_WORK,
            references: HashMap::new(),
            linted: HashSet::new(),
        }
    }

    fn spend(&mut self) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("This operation exceeds its 50,000-value work limit. Use a smaller input.")?;
        Ok(())
    }

    fn step(&mut self, depth: usize) -> Result<()> {
        self.spend()?;
        if depth >= MAX_DEPTH {
            Err("Schema recursion exceeds 64 levels. Check for a reference cycle.".into())
        } else {
            Ok(())
        }
    }

    fn resolve(&mut self, reference: &'a str) -> Result<&'a Value> {
        if let Some(target) = self.references.get(reference) {
            return Ok(*target);
        }
        let target = super::pointer(self.root, reference)?;
        self.references.insert(reference, target);
        Ok(target)
    }

    fn lint(&mut self, schema: &'a Value, depth: usize) -> Result<()> {
        self.step(depth)?;
        if schema.is_boolean() || !self.linted.insert(schema as *const Value) {
            return Ok(());
        }
        let fields = schema
            .as_object()
            .ok_or("A schema must be a JSON object or boolean.")?;
        if let Some(unknown) = fields
            .keys()
            .filter(|key| !SUPPORTED.contains(&key.as_str()))
            .min()
        {
            return Err(format!(
                "Unsupported schema keyword: {}. This validator checks a local 2020-12 subset; it will not ignore constraints.",
                prefix(unknown, 80),
            ));
        }
        if fields
            .get("$schema")
            .is_some_and(|dialect| dialect.as_str() != Some(DIALECT))
        {
            return Err("Use the 2020-12 schema dialect, or omit $schema.".into());
        }
        let referenced = if let Some(reference) = fields.get("$ref") {
            let reference = reference
                .as_str()
                .filter(|r| r.starts_with('#'))
                .ok_or("Only local JSON Pointer references (# or #/…) are supported.")?;
            Some(self.resolve(reference)?)
        } else {
            None
        };
        if let Some(kind) = fields.get("type") {
            let names = type_names(kind)?;
            if names.is_empty()
                || names.iter().any(|name| !TYPES.contains(name))
                || names.iter().collect::<HashSet<_>>().len() != names.len()
            {
                return Err("Use unique JSON type names in type.".into());
            }
        }
        if let Some(enumeration) = fields.get("enum")
            && enumeration.as_array().is_none_or(Vec::is_empty)
        {
            return Err("Schema enum needs a non-empty array.".into());
        }
        if let Some(required) = fields.get("required") {
            let values = required
                .as_array()
                .ok_or("Schema required must be an array of unique strings.")?;
            let mut names = HashSet::new();
            for value in values {
                self.spend()?;
                let name = value
                    .as_str()
                    .ok_or("Schema required must contain strings.")?;
                if !names.insert(name) {
                    return Err("Schema required contains duplicate names.".into());
                }
            }
        }
        for key in COUNTS {
            if let Some(value) = fields.get(*key) {
                let message = format!(
                    "{key} must be a non-negative whole number within the 64-bit integer range."
                );
                let number = value.as_number().ok_or_else(|| message.clone())?;
                let count = ExactNumber::new(&number.to_string())?
                    .integer_value()
                    .ok_or_else(|| message.clone())?;
                if count < 0 {
                    return Err(message);
                }
            }
        }
        for key in NUMERIC_BOUNDS {
            if let Some(value) = fields.get(*key) {
                let number = value
                    .as_number()
                    .ok_or_else(|| format!("{key} must be a number."))?;
                ExactNumber::new(&number.to_string())?;
            }
        }
        if fields
            .get("uniqueItems")
            .is_some_and(|value| !value.is_boolean())
        {
            return Err("uniqueItems must be a boolean.".into());
        }
        for key in ["properties", "$defs"] {
            if let Some(container) = fields.get(key) {
                let schemas = container
                    .as_object()
                    .ok_or_else(|| format!("{key} must be an object of schemas."))?;
                for child in schemas.values() {
                    self.lint(child, depth + 1)?;
                }
            }
        }
        for key in ["items", "additionalProperties", "not"] {
            if let Some(child) = fields.get(key) {
                self.lint(child, depth + 1)?;
            }
        }
        for key in ["prefixItems", "allOf", "anyOf", "oneOf"] {
            if let Some(container) = fields.get(key) {
                let schemas = container
                    .as_array()
                    .filter(|values| !values.is_empty())
                    .ok_or_else(|| format!("{key} must be a non-empty array of schemas."))?;
                for child in schemas {
                    self.lint(child, depth + 1)?;
                }
            }
        }
        // A local ref may enter an annotation such as default or examples.
        // Preflight these targets even when their containing branch is unused.
        if let Some(target) = referenced {
            self.lint(target, depth + 1)?;
        }
        Ok(())
    }

    fn canonical(&mut self, value: &Value) -> Result<String> {
        let mut encoded = String::new();
        self.encode_canonical(value, &mut encoded, 0)?;
        Ok(encoded)
    }

    fn encode_canonical(&mut self, value: &Value, output: &mut String, depth: usize) -> Result<()> {
        self.step(depth)?;
        match value {
            Value::Number(number) => {
                push(output, "n")?;
                push(output, &ExactNumber::new(&number.to_string())?.canonical())?;
            }
            Value::Array(children) => {
                push(output, "[")?;
                for (index, child) in children.iter().enumerate() {
                    if index > 0 {
                        push(output, ",")?;
                    }
                    self.encode_canonical(child, output, depth + 1)?;
                }
                push(output, "]")?;
            }
            Value::Object(fields) => {
                push(output, "{")?;
                let mut keys: Vec<_> = fields.keys().collect();
                keys.sort_unstable();
                for (index, key) in keys.iter().enumerate() {
                    if index > 0 {
                        push(output, ",")?;
                    }
                    push(output, &quoted(key))?;
                    push(output, ":")?;
                    self.encode_canonical(&fields[*key], output, depth + 1)?;
                }
                push(output, "}")?;
            }
            Value::String(text) => push(output, &quoted(text))?,
            Value::Bool(true) => push(output, "true")?,
            Value::Bool(false) => push(output, "false")?,
            Value::Null => push(output, "null")?,
        }
        Ok(())
    }

    fn check(
        &mut self,
        value: &Value,
        schema: &'a Value,
        path: &str,
        depth: usize,
    ) -> Result<Vec<String>> {
        self.step(depth)?;
        if let Some(allowed) = schema.as_bool() {
            return Ok(if allowed {
                vec![]
            } else {
                vec![diagnostic(path, "value is not allowed")]
            });
        }
        let fields = schema
            .as_object()
            .ok_or("A referenced schema must be an object or boolean.")?;
        let mut issues = Vec::new();
        if let Some(reference) = fields.get("$ref").and_then(Value::as_str) {
            let target = self.resolve(reference)?;
            append_issues(&mut issues, self.check(value, target, path, depth + 1)?);
        }
        if let Some(kind) = fields.get("type") {
            let names = type_names(kind)?;
            let mut matched = false;
            for name in &names {
                if matches_type(value, name)? {
                    matched = true;
                    break;
                }
            }
            if !matched {
                issue(
                    &mut issues,
                    path,
                    &format!("expected {}", names.join(" or ")),
                );
                return Ok(issues);
            }
        }
        if let Some(constant) = fields.get("const")
            && self.canonical(value)? != self.canonical(constant)?
        {
            issue(&mut issues, path, "does not match const");
        }
        if let Some(candidates) = fields.get("enum").and_then(Value::as_array) {
            let actual = self.canonical(value)?;
            let mut matched = false;
            for candidate in candidates {
                if self.canonical(candidate)? == actual {
                    matched = true;
                    break;
                }
            }
            if !matched {
                issue(&mut issues, path, "not one of the allowed enum values");
            }
        }
        match value {
            Value::Object(object) => {
                if let Some(required) = fields.get("required").and_then(Value::as_array) {
                    for name in required {
                        self.spend()?;
                        // Preflight checked every required entry's type.
                        let name = name
                            .as_str()
                            .ok_or("Schema required must contain strings.")?;
                        if !object.contains_key(name) {
                            issue(
                                &mut issues,
                                path,
                                &format!("missing required field {}", quoted(&prefix(name, 80))),
                            );
                        }
                    }
                }
                let properties = fields.get("properties").and_then(Value::as_object);
                let mut keys: Vec<_> = object.keys().collect();
                keys.sort_unstable();
                for key in keys {
                    self.spend()?;
                    let rule = properties
                        .and_then(|p| p.get(key))
                        .or_else(|| fields.get("additionalProperties"));
                    if let Some(rule) = rule {
                        let child_path = child_path(path, key);
                        append_issues(
                            &mut issues,
                            self.check(&object[key], rule, &child_path, depth + 1)?,
                        );
                        if issues.len() == MAX_ISSUES {
                            break;
                        }
                    }
                }
                bounds(
                    object.len(),
                    fields,
                    "minProperties",
                    "maxProperties",
                    "properties",
                    path,
                    &mut issues,
                )?;
            }
            Value::Array(array) => {
                let prefix = fields.get("prefixItems").and_then(Value::as_array);
                let items = fields.get("items");
                // No per-item work exists beyond the prefix without `items`.
                // Skipping it also prevents large irrelevant arrays from being
                // scanned repeatedly through composition branches.
                let checked_items = if items.is_some() {
                    array.len()
                } else {
                    prefix.map_or(0, Vec::len)
                };
                for (index, child) in array.iter().take(checked_items).enumerate() {
                    let rule = prefix.and_then(|rules| rules.get(index)).or(items);
                    if let Some(rule) = rule {
                        let child_path = child_path(path, &index.to_string());
                        append_issues(
                            &mut issues,
                            self.check(child, rule, &child_path, depth + 1)?,
                        );
                        if issues.len() == MAX_ISSUES {
                            break;
                        }
                    }
                }
                if fields.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
                    let mut seen = HashSet::new();
                    for child in array {
                        if !seen.insert(self.canonical(child)?) {
                            issue(&mut issues, path, "array items must be unique");
                            break;
                        }
                    }
                }
                bounds(
                    array.len(),
                    fields,
                    "minItems",
                    "maxItems",
                    "items",
                    path,
                    &mut issues,
                )?;
            }
            Value::String(text) => bounds(
                text.chars().count(),
                fields,
                "minLength",
                "maxLength",
                "Unicode code points",
                path,
                &mut issues,
            )?,
            Value::Number(number) => {
                for key in NUMERIC_BOUNDS {
                    if let Some(threshold) = fields.get(*key).and_then(Value::as_number) {
                        let threshold_text = threshold.to_string();
                        let comparison = ExactNumber::new(&number.to_string())?
                            .compare(&ExactNumber::new(&threshold_text)?);
                        let failed = match *key {
                            "minimum" => comparison == Ordering::Less,
                            "maximum" => comparison == Ordering::Greater,
                            "exclusiveMinimum" => comparison != Ordering::Greater,
                            "exclusiveMaximum" => comparison != Ordering::Less,
                            _ => unreachable!(),
                        };
                        if failed {
                            issue(
                                &mut issues,
                                path,
                                &format!("does not satisfy {key} {}", prefix(&threshold_text, 80)),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        for key in ["allOf", "anyOf", "oneOf"] {
            if let Some(branches) = fields.get(key).and_then(Value::as_array) {
                let mut passes = 0;
                // Do not short-circuit: every evaluated branch shares the budget.
                for branch in branches {
                    if self.check(value, branch, path, depth + 1)?.is_empty() {
                        passes += 1;
                    }
                }
                if (key == "allOf" && passes != branches.len())
                    || (key == "anyOf" && passes == 0)
                    || (key == "oneOf" && passes != 1)
                {
                    issue(
                        &mut issues,
                        path,
                        &format!("{key}: matched {passes} of {} branches", branches.len()),
                    );
                }
            }
        }
        if let Some(negative) = fields.get("not")
            && self.check(value, negative, path, depth + 1)?.is_empty()
        {
            issue(&mut issues, path, "matches a disallowed not schema");
        }
        Ok(issues)
    }
}

fn type_names(value: &Value) -> Result<Vec<&str>> {
    match value {
        Value::String(name) => Ok(vec![name.as_str()]),
        Value::Array(values) => values
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| "Schema types must be strings.".into())
            })
            .collect(),
        _ => Err("Schema type must be a name or list of names.".into()),
    }
}

fn matches_type(value: &Value, kind: &str) -> Result<bool> {
    Ok(match kind {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => match value.as_number() {
            Some(number) => ExactNumber::new(&number.to_string())?.is_integer(),
            None => false,
        },
        _ => false,
    })
}

fn bounds(
    count: usize,
    fields: &Map<String, Value>,
    minimum: &str,
    maximum: &str,
    label: &str,
    path: &str,
    issues: &mut Vec<String>,
) -> Result<()> {
    for (key, is_minimum) in [(minimum, true), (maximum, false)] {
        if let Some(number) = fields.get(key).and_then(Value::as_number) {
            let threshold = ExactNumber::new(&number.to_string())?
                .integer_value()
                .ok_or("Invalid count bound.")?;
            if (is_minimum && (count as u64) < threshold as u64)
                || (!is_minimum && count as u64 > threshold as u64)
            {
                let relation = if is_minimum {
                    "needs at least"
                } else {
                    "allows at most"
                };
                issue(issues, path, &format!("{relation} {threshold} {label}"));
            }
        }
    }
    Ok(())
}

fn prefix(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}
fn quoted(text: &str) -> String {
    Value::String(text.into()).to_string()
}
fn diagnostic(path: &str, message: &str) -> String {
    format!(
        "{}: {message}",
        if path.is_empty() {
            "(root)".into()
        } else {
            prefix(path, 512)
        }
    )
}
fn child_path(path: &str, key: &str) -> String {
    // Paths only feed diagnostics; carry the same bounded prefix through deep
    // trees instead of repeatedly allocating potentially enormous object keys.
    path.chars()
        .chain(std::iter::once('/'))
        .chain(
            key.chars()
                .flat_map(|c| match c {
                    '~' => [Some('~'), Some('0')],
                    '/' => [Some('~'), Some('1')],
                    _ => [Some(c), None],
                })
                .flatten(),
        )
        .take(512)
        .collect()
}
fn issue(issues: &mut Vec<String>, path: &str, message: &str) {
    if issues.len() < MAX_ISSUES {
        issues.push(diagnostic(path, message));
    }
}
fn append_issues(issues: &mut Vec<String>, children: Vec<String>) {
    issues.extend(children.into_iter().take(MAX_ISSUES - issues.len()));
}
fn push(output: &mut String, text: &str) -> Result<()> {
    if output.len().saturating_add(text.len()) > MAX_OUTPUT {
        return Err("Schema canonical comparison exceeds the 4 MB output limit.".into());
    }
    output.push_str(text);
    Ok(())
}

/// Decimal normalization without floating point or expansion of large powers.
/// All inputs originate from the strict JSON parser, so their grammar is valid.
#[derive(Debug)]
struct ExactNumber {
    negative: bool,
    digits: String,
    exponent: i64,
}
impl ExactNumber {
    fn new(text: &str) -> Result<Self> {
        if text.len() > MAX_NUMERIC_BYTES {
            return Err("Schema numeric comparisons support up to 4,096 digits.".into());
        }
        let mut parts = text.split(['e', 'E']);
        let mantissa = parts.next().unwrap_or_default();
        let power = match parts.next() {
            Some(power) => power
                .parse::<i64>()
                .map_err(|_| "Schema numeric exponents must be within ±1,000,000.")?,
            None => 0,
        };
        if parts.next().is_some() || !(-MAX_EXPONENT..=MAX_EXPONENT).contains(&power) {
            return Err("Schema numeric exponents must be within ±1,000,000.".into());
        }
        let negative = mantissa.starts_with('-');
        let unsigned = mantissa.strip_prefix('-').unwrap_or(mantissa);
        let fraction_length = unsigned
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        let digits: String = unsigned.chars().filter(|c| *c != '.').collect();
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            return Ok(Self {
                negative: false,
                digits: "0".into(),
                exponent: 0,
            });
        }
        let normalized = digits.trim_end_matches('0');
        let exponent = power - fraction_length as i64 + (digits.len() - normalized.len()) as i64;
        Ok(Self {
            negative,
            digits: normalized.into(),
            exponent,
        })
    }
    fn canonical(&self) -> String {
        format!(
            "{}{}e{}",
            if self.negative { "-" } else { "" },
            self.digits,
            self.exponent
        )
    }
    fn is_integer(&self) -> bool {
        self.digits == "0" || self.exponent >= 0
    }
    fn integer_value(&self) -> Option<i64> {
        if !self.is_integer() || self.exponent < 0 || self.digits.len() as i64 + self.exponent > 19
        {
            return None;
        }
        format!(
            "{}{}{}",
            if self.negative { "-" } else { "" },
            self.digits,
            "0".repeat(self.exponent as usize)
        )
        .parse()
        .ok()
    }
    fn compare(&self, other: &Self) -> Ordering {
        if self.negative != other.negative {
            return if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let order = match (self.digits == "0", other.digits == "0") {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                let magnitude = (self.digits.len() as i64 + self.exponent)
                    .cmp(&(other.digits.len() as i64 + other.exponent));
                if magnitude != Ordering::Equal {
                    magnitude
                } else {
                    let length = self.digits.len().max(other.digits.len());
                    self.digits
                        .bytes()
                        .chain(std::iter::repeat(b'0'))
                        .take(length)
                        .cmp(
                            other
                                .digits
                                .bytes()
                                .chain(std::iter::repeat(b'0'))
                                .take(length),
                        )
                }
            }
        };
        if self.negative {
            order.reverse()
        } else {
            order
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(input: &str, schema: &str) {
        let result =
            validate(input, schema).unwrap_or_else(|e| panic!("{input} against {schema}: {e}"));
        assert!(result.starts_with("Valid against this schema."), "{result}");
    }
    fn invalid(input: &str, schema: &str) -> String {
        let result =
            validate(input, schema).unwrap_or_else(|e| panic!("{input} against {schema}: {e}"));
        assert!(
            !result.starts_with("Valid against this schema."),
            "Unexpected valid result"
        );
        result
    }

    #[test]
    fn source_types_required_escaped_paths_and_booleans() {
        let schema = r#"{"type":"object","required":["name"],"properties":{"a/b":{"type":"array","items":{"type":"integer","minimum":0}}},"additionalProperties":false}"#;
        let result = invalid(r#"{"a/b":[-1,"bad"]}"#, schema);
        assert!(result.contains("missing required field \"name\""));
        assert!(result.contains("/a~1b/0: does not satisfy minimum 0"));
        assert!(result.contains("/a~1b/1: expected integer"));
        assert!(invalid(r#"{"extra":1}"#, schema).contains("/extra: value is not allowed"));
        valid(
            r#"{"name":"Ada","age":37}"#,
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"age":{"type":"integer","minimum":0}}}"#,
        );
        assert_eq!(invalid("null", "false"), "(root): value is not allowed");
        valid("null", "true");
        valid("null", "{}");
    }

    #[test]
    fn source_exact_numbers_and_code_point_lengths() {
        invalid("9007199254740993", r#"{"maximum":9007199254740992}"#);
        valid("1e3", r#"{"type":"integer","const":1000.00}"#);
        invalid("1.01", r#"{"type":"integer"}"#);
        invalid(r#""e\u0301""#, r#"{"maxLength":1}"#);
        valid(r#""é""#, r#"{"maxLength":1.0}"#);
        invalid(r#""é""#, r#"{"const":"e\u0301"}"#);
        invalid("[1,1.0]", r#"{"uniqueItems":true}"#);
        valid(r#"["é","e\u0301"]"#, r#"{"uniqueItems":true}"#);
        assert!(validate("1", r#"{"minimum":1e-9223372036854775808}"#).is_err());
        valid(r#""💡""#, r#"{"minLength":1,"maxLength":1}"#);
        valid(r#""👩🏽‍💻""#, r#"{"minLength":4,"maxLength":4}"#);
        invalid(r#""👩🏽‍💻""#, r#"{"maxLength":1}"#);
        for (a, b, expected) in [
            ("-1e20", "-99999999999999999999", Ordering::Less),
            ("0", "-0.001", Ordering::Greater),
            ("0.0100", "1e-2", Ordering::Equal),
            ("1e1000000", "9e999999", Ordering::Greater),
        ] {
            assert_eq!(
                ExactNumber::new(a)
                    .unwrap()
                    .compare(&ExactNumber::new(b).unwrap()),
                expected
            );
        }
    }

    #[test]
    fn source_local_references_composition_and_tuple_items() {
        let schema = r##"{"$defs":{"count":{"type":"integer","minimum":1}},"type":"array","items":{"$ref":"#/$defs/count"}}"##;
        valid("[1,2,3]", schema);
        invalid("[0]", schema);
        assert!(
            invalid("1", r#"{"oneOf":[{"type":"number"},{"type":"integer"}]}"#)
                .contains("oneOf: matched 2 of 2 branches")
        );
        valid(
            r#""x""#,
            r#"{"anyOf":[{"type":"number"},{"type":"string"}],"not":{"const":"y"}}"#,
        );
        invalid("3", r#"{"allOf":[{"minimum":1},{"maximum":2}]}"#);
        assert_eq!(
            invalid(
                "[1,2]",
                r#"{"prefixItems":[{"type":"integer"}],"items":false}"#
            ),
            "/1: value is not allowed"
        );
        for schema in [
            r#"{"$ref":"https://example.com/schema"}"#,
            r#"{"properties":{"unused":{"pattern":"evil"}}}"#,
            r##"{"$ref":"#"}"##,
            r#"{"items":{},"unexpected":true}"#,
            r##"{"$defs":{"n":{"type":"number"}},"$ref":"#/$defs/n","minimum":"0"}"##,
            r##"{"default":{"pattern":"x"},"$ref":"#/default"}"##,
        ] {
            assert!(validate("{}", schema).is_err(), "{schema}");
        }
    }

    #[test]
    fn all_supported_type_names_and_unions_are_exact() {
        for (input, kind) in [
            ("null", "null"),
            ("true", "boolean"),
            ("{}", "object"),
            ("[]", "array"),
            ("1.1", "number"),
            ("1e2", "integer"),
            (r#""1""#, "string"),
        ] {
            valid(input, &format!(r#"{{"type":"{kind}"}}"#));
            for wrong in TYPES
                .iter()
                .filter(|t| **t != kind && !(kind == "integer" && **t == "number"))
            {
                invalid(input, &format!(r#"{{"type":"{wrong}"}}"#));
            }
        }
        valid("null", r#"{"type":["null","integer"]}"#);
        valid("12", r#"{"type":["null","integer"]}"#);
        invalid("true", r#"{"type":["null","integer"]}"#);
        for kind in [
            r#"[]"#,
            r#"["string","string"]"#,
            r#"[1]"#,
            r#"true"#,
            r#""int""#,
        ] {
            assert!(validate("null", &format!(r#"{{"type":{kind}}}"#)).is_err());
        }
    }

    #[test]
    fn canonical_equality_is_typed_order_independent_and_decimal_exact() {
        valid(
            r#"{"b":[1000,0],"a":true}"#,
            r#"{"const":{"a":true,"b":[1.00e3,-0.0e20]}}"#,
        );
        valid(
            "1e-1000000",
            r#"{"enum":[false,null,"1",2e-1000000,0.1e-999999]}"#,
        );
        invalid("true", r#"{"const":1}"#);
        invalid("1", r#"{"const":"1"}"#);
        invalid("null", r#"{"const":"null"}"#);
        invalid("[]", r#"{"const":{}}"#);
        invalid("9007199254740993", r#"{"enum":[9007199254740992]}"#);
        invalid(
            r#"[{"b":2,"a":1},{"a":1.0,"b":2e0}]"#,
            r#"{"uniqueItems":true}"#,
        );
        valid(r#"[1,"1",true,null,[],{}]"#, r#"{"uniqueItems":true}"#);
        invalid("[-0,0.00]", r#"{"uniqueItems":true}"#);
        valid(
            r#"{"é":1,"e\u0301":2}"#,
            r#"{"required":["é","e\u0301"],"minProperties":2}"#,
        );
        invalid(r#"{"é":1}"#, r#"{"required":["e\u0301"]}"#);
    }

    #[test]
    fn exact_number_signs_exponents_and_boundaries() {
        for (a, b, expected) in [
            ("0", "0.000e-99", Ordering::Equal),
            ("-0", "0", Ordering::Equal),
            ("-1", "0", Ordering::Less),
            ("0", "1e-1000000", Ordering::Less),
            ("-9", "-10", Ordering::Greater),
            ("9.9", "9.91", Ordering::Less),
            ("1.200", "12e-1", Ordering::Equal),
            ("100", "1E+2", Ordering::Equal),
            ("1e-1000000", "2e-1000000", Ordering::Less),
            (
                "123456789012345678901",
                "123456789012345678900",
                Ordering::Greater,
            ),
        ] {
            let a = ExactNumber::new(a).unwrap();
            let b = ExactNumber::new(b).unwrap();
            assert_eq!(a.compare(&b), expected);
            assert_eq!(b.compare(&a), expected.reverse());
            assert_eq!(a.canonical() == b.canonical(), expected == Ordering::Equal);
        }
        for number in [
            "1e1000001",
            "1e-1000001",
            "1e999999999999999999999",
            "-0e1000001",
        ] {
            assert!(ExactNumber::new(number).is_err(), "{number}");
        }
        assert!(ExactNumber::new(&"1".repeat(MAX_NUMERIC_BYTES)).is_ok());
        assert!(ExactNumber::new(&"1".repeat(MAX_NUMERIC_BYTES + 1)).is_err());
        assert_eq!(
            ExactNumber::new("9223372036854775807")
                .unwrap()
                .integer_value(),
            Some(i64::MAX)
        );
        assert_eq!(
            ExactNumber::new("9223372036854775808")
                .unwrap()
                .integer_value(),
            None
        );
        assert_eq!(
            ExactNumber::new("-9223372036854775808")
                .unwrap()
                .integer_value(),
            Some(i64::MIN)
        );
        valid(
            "1e1000000",
            r#"{"maximum":10e1000000,"exclusiveMinimum":9e999999}"#,
        );
        invalid("1e-1000000", r#"{"exclusiveMaximum":0.1e-999999}"#);
        valid(
            "0",
            r#"{"type":"integer","minimum":-0,"maximum":0,"const":0.000e-999999}"#,
        );
    }

    #[test]
    fn exact_number_order_matches_small_rational_arithmetic() {
        let mut samples = Vec::new();
        for coefficient in -16_i128..=16 {
            for exponent in -3_i32..=3 {
                let text = format!("{coefficient}e{exponent}");
                let scaled_integer = coefficient * 10_i128.pow((exponent + 3) as u32);
                samples.push((ExactNumber::new(&text).unwrap(), scaled_integer));
            }
        }
        for (a, a_scaled) in &samples {
            for (b, b_scaled) in &samples {
                assert_eq!(a.compare(b), a_scaled.cmp(b_scaled), "{a:?}, {b:?}");
            }
        }
    }

    #[test]
    fn scalar_object_and_array_bounds_match_the_subset() {
        invalid(r#"{"a":1}"#, r#"{"minProperties":2}"#);
        invalid(r#"{"a":1,"b":2}"#, r#"{"maxProperties":1}"#);
        valid("[]", r#"{"maxItems":0,"minItems":-0.0}"#);
        invalid("[1]", r#"{"minItems":2}"#);
        invalid("[1,2]", r#"{"maxItems":1}"#);
        valid(
            "[1,2.5]",
            r#"{"prefixItems":[{"type":"integer"}],"items":{"type":"number"},"minItems":2,"maxItems":2}"#,
        );
        valid(
            r#"{"a":1,"x":"yes"}"#,
            r#"{"properties":{"a":{"const":1}},"additionalProperties":{"type":"string"}}"#,
        );
        invalid(
            r#"{"a":1,"x":2}"#,
            r#"{"properties":{"a":{"const":1}},"additionalProperties":{"type":"string"}}"#,
        );
        valid("5", r#"{"minimum":5,"maximum":5}"#);
        invalid("5", r#"{"exclusiveMinimum":5}"#);
        invalid("5", r#"{"exclusiveMaximum":5}"#);
        valid(
            "null",
            r#"{"minimum":5,"minLength":10,"minItems":10,"required":["ignored-for-null"]}"#,
        );
        valid("null", r#"{"minItems":9223372036854775807}"#);
        for value in [
            "-1",
            "1.5",
            "9223372036854775808",
            "1e19",
            "true",
            "null",
            r#""1""#,
        ] {
            assert!(
                validate("null", &format!(r#"{{"minItems":{value}}}"#)).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn preflight_rejects_constraints_and_malformed_schemas_in_unused_locations() {
        for location in [
            r#"{"properties":{"unused":REPLACE}}"#,
            r#"{"$defs":{"unused":REPLACE}}"#,
            r#"{"items":REPLACE}"#,
            r#"{"additionalProperties":REPLACE}"#,
            r#"{"prefixItems":[true,REPLACE]}"#,
            r#"{"allOf":[false,REPLACE]}"#,
            r#"{"anyOf":[true,REPLACE]}"#,
            r#"{"oneOf":[true,REPLACE]}"#,
            r#"{"not":REPLACE}"#,
        ] {
            for child in [
                r#"{"format":"email"}"#,
                r#"{"minimum":"0"}"#,
                "null",
                "1",
                "[]",
            ] {
                assert!(
                    validate("null", &location.replace("REPLACE", child)).is_err(),
                    "{location}: {child}"
                );
            }
        }
        for (keyword, value) in [
            ("properties", "[]"),
            ("$defs", "[]"),
            ("enum", "[]"),
            ("enum", "null"),
            ("required", "{}"),
            ("required", "[1]"),
            ("required", r#"["a","a"]"#),
            ("uniqueItems", "1"),
            ("prefixItems", "[]"),
            ("allOf", "[]"),
            ("anyOf", "{}"),
            ("oneOf", "false"),
            ("maximum", "true"),
        ] {
            assert!(
                validate("null", &format!(r#"{{"{keyword}":{value}}}"#)).is_err(),
                "{keyword}"
            );
        }
        for unknown in [
            "pattern",
            "format",
            "$id",
            "$anchor",
            "$dynamicRef",
            "contains",
            "multipleOf",
            "dependentRequired",
            "unevaluatedProperties",
            "definitions",
        ] {
            assert!(
                validate("null", &format!(r#"{{"{unknown}":true}}"#))
                    .unwrap_err()
                    .contains("Unsupported schema keyword")
            );
        }
    }

    #[test]
    fn annotations_are_opaque_until_referenced_and_all_ref_targets_are_linted() {
        valid(
            "null",
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$comment":"ok","title":null,"description":[],"default":{"pattern":"annotation"},"examples":[{"format":"annotation"}],"readOnly":null,"writeOnly":2,"deprecated":"annotation"}"#,
        );
        for schema in [
            r##"{"default":{"pattern":"x"},"$ref":"#/default"}"##,
            r##"{"default":{"pattern":"x"},"properties":{"unused":{"$ref":"#/default"}}}"##,
            r##"{"examples":[{"format":"email"}],"$defs":{"unused":{"$ref":"#/examples/0"}}}"##,
            r##"{"default":null,"anyOf":[true,{"$ref":"#/default"}]}"##,
            r##"{"default":{"$ref":"#/examples/0"},"examples":[{"pattern":"x"}],"$ref":"#/default"}"##,
        ] {
            assert!(validate("null", schema).is_err(), "{schema}");
        }
        valid("1", r##"{"default":{"minimum":0},"$ref":"#/default"}"##);
        invalid("-1", r##"{"default":{"minimum":0},"$ref":"#/default"}"##);
        for dialect in [
            "http://json-schema.org/draft/2020-12/schema",
            "https://json-schema.org/draft/2019-09/schema",
            "2020-12",
        ] {
            assert!(validate("null", &format!(r#"{{"$schema":"{dialect}"}}"#)).is_err());
        }
    }

    #[test]
    fn local_pointer_escaping_percent_decoding_and_reference_siblings() {
        valid(
            "1",
            r##"{"$defs":{"a/b~c":{"const":1}},"$ref":"#/$defs/a~1b~0c"}"##,
        );
        valid(
            "1",
            r##"{"$defs":{"a b":{"const":1}},"$ref":"#/%24defs/a%20b"}"##,
        );
        valid("1", r##"{"$defs":{"":{"const":1}},"$ref":"#/$defs/"}"##);
        valid("1", r##"{"examples":[{"const":1}],"$ref":"#/examples/0"}"##);
        invalid(
            "0",
            r##"{"$defs":{"n":{"type":"integer"}},"$ref":"#/$defs/n","minimum":1}"##,
        );
        for reference in [
            "https://example.com/schema",
            "/$defs/a",
            "#anchor",
            "#/$defs/missing",
            "#/$defs/a~2",
            "#/%GG",
            "#/examples/00",
            "#/examples/-",
            "#/examples/+0",
            "#/examples/1",
            "#/%FF",
            "#/type/child",
        ] {
            let schema = serde_json::json!({"$defs":{"a":true},"examples":[true],"type":"number","$ref":reference}).to_string();
            assert!(validate("1", &schema).is_err(), "{reference}");
        }
        let schema = serde_json::json!({"$ref":format!("#/{}", "a".repeat(8192))}).to_string();
        assert!(validate("null", &schema).is_err());
        assert!(validate("1", r#"{"$ref":true}"#).is_err());
    }

    #[test]
    fn cycles_and_excessive_recursion_fail_but_finite_recursive_data_works() {
        for schema in [
            r##"{"$ref":"#"}"##,
            r##"{"$defs":{"a":{"$ref":"#/$defs/b"},"b":{"$ref":"#/$defs/a"}},"$ref":"#/$defs/a"}"##,
            r##"{"default":{"$ref":"#/default"},"$ref":"#/default"}"##,
        ] {
            assert!(
                validate("null", schema).unwrap_err().contains("64 levels"),
                "{schema}"
            );
        }
        valid(
            "null",
            r##"{"$defs":{"unused":{"$ref":"#/$defs/unused"}}}"##,
        );
        valid(
            r#"{"next":{"next":null}}"#,
            r##"{"anyOf":[{"type":"null"},{"type":"object","properties":{"next":{"$ref":"#"}},"required":["next"],"additionalProperties":false}]}"##,
        );
        let deep_input = format!("{}null{}", "[".repeat(64), "]".repeat(64));
        assert!(validate(&deep_input, "true").is_err());
        let deep_schema = format!("{}true{}", "{\"not\":".repeat(64), "}".repeat(64));
        assert!(validate("null", &deep_schema).is_err());
    }

    #[test]
    fn work_diagnostics_input_and_output_are_bounded() {
        let input = format!("[{}]", vec!["0"; 10_000].join(","));
        let schema = r##"{"$defs":{"n":{"type":"integer"}},"items":{"allOf":[{"$ref":"#/$defs/n"},{"$ref":"#/$defs/n"},{"$ref":"#/$defs/n"}]}}"##;
        assert!(
            validate(&input, schema)
                .unwrap_err()
                .contains("50,000-value work limit")
        );
        let input = format!("[{}]", vec!["null"; 200].join(","));
        let output = invalid(&input, r#"{"items":false}"#);
        assert_eq!(output.lines().count(), MAX_ISSUES + 1);
        assert!(output.contains("/99: value is not allowed"));
        assert!(!output.contains("/100:"));
        assert!(output.ends_with("Showing the first 100 issues."));
        let input = serde_json::json!({"é".repeat(10_000): null}).to_string();
        let output = invalid(&input, r#"{"additionalProperties":false}"#);
        assert_eq!(output.split(':').next().unwrap().chars().count(), 512);
        assert!(
            validate(
                &format!("\"{}\"", "a".repeat(super::super::MAX_INPUT)),
                "true"
            )
            .is_err()
        );
        let mut full = "a".repeat(MAX_OUTPUT);
        assert!(push(&mut full, "b").is_err());
        assert_eq!(full.len(), MAX_OUTPUT);
    }

    #[test]
    fn strict_parser_rejects_duplicate_keys_and_empty_documents() {
        for (input, schema) in [
            ("null", ""),
            ("null", "   "),
            ("", "true"),
            (r#"{"a":1,"a":2}"#, "true"),
            ("null", r#"{"type":"null","type":"string"}"#),
            ("NaN", "true"),
            ("1 trailing", "true"),
            ("null", "null"),
        ] {
            assert!(validate(input, schema).is_err(), "{input} against {schema}");
        }
    }
}
