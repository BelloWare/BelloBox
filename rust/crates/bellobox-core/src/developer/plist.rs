//! Source-compatible typed XML plist conversion. Never resolves a DTD or entity.
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{Datelike, Timelike};
use serde_json::{Map, Value, json};
const DOCTYPE: &str = "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">";
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 20_000;
const MAX_OUTPUT: usize = 4_000_000;
type R<T> = Result<T, String>;
fn typed(kind: &str, value: Value) -> Value {
    json!({"type":kind,"value":value})
}
fn step(depth: usize, budget: &mut usize) -> R<()> {
    if depth >= MAX_DEPTH || *budget == 0 {
        return Err("Plist exceeds 64 levels or its bounded value budget.".into());
    }
    *budget -= 1;
    Ok(())
}
fn validate_text(s: &str) -> R<()> {
    if s.chars()
        .all(|c| matches!(c as u32,9|10|13|0x20..=0xd7ff|0xe000..=0xfffd|0x10000..=0x10ffff))
    {
        Ok(())
    } else {
        Err("XML plist strings and keys cannot contain XML 1.0 control characters.".into())
    }
}
fn escape(s: &str) -> R<String> {
    validate_text(s)?;
    Ok(s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('\r', "&#13;"))
}
fn valid_date(s: &str) -> bool {
    s.len() == 20
        && s.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        && chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ").is_ok_and(|d| {
            (1..=9999).contains(&d.year())
                && d.second() < 60
                && d.nanosecond() < 1_000_000_000
                && d.format("%Y-%m-%dT%H:%M:%SZ").to_string() == s
        })
}
fn integer(s: &str) -> R<Value> {
    let text = if let Ok(v) = s.parse::<i64>() {
        v.to_string()
    } else if let Ok(v) = s.parse::<u64>() {
        v.to_string()
    } else {
        return Err("Plist integers must fit signed or unsigned 64 bits.".into());
    };
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
fn real(s: &str) -> R<Value> {
    let number = s
        .parse::<f64>()
        .map_err(|_| "Plist real must be a finite 64-bit floating-point value.")?;
    if !number.is_finite() {
        return Err("Plist real must be finite.".into());
    }
    let text = if number.fract() == 0.0 {
        format!("{number:.1}")
    } else {
        number.to_string()
    };
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
fn read(node: roxmltree::Node<'_, '_>, depth: usize, budget: &mut usize) -> R<Value> {
    step(depth, budget)?;
    let kind = node.tag_name().name();
    if node.tag_name().namespace().is_some()
        || ![
            "plist", "dict", "array", "key", "string", "integer", "real", "date", "data", "true",
            "false",
        ]
        .contains(&kind)
    {
        return Err(format!(
            "Unsupported plist element: {}.",
            kind.chars().take(40).collect::<String>()
        ));
    }
    if node.attributes().len() != 0
        && !(kind == "plist"
            && node.attributes().len() == 1
            && node.attribute("version") == Some("1.0")
            && node.attributes().all(|a| a.namespace().is_none()))
    {
        return Err("Unsupported plist attributes.".into());
    }
    let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
    let text = node
        .children()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<String>();
    let trimmed = text.trim();
    match kind {
        "plist" => {
            if depth != 0
                || children.len() != 1
                || !trimmed.is_empty()
                || children[0].tag_name().name() == "key"
            {
                return Err("A plist root needs exactly one value and no direct text.".into());
            }
            read(children[0], depth + 1, budget)
        }
        "array" => {
            if !trimmed.is_empty() {
                return Err("Text is not allowed directly inside a plist container.".into());
            }
            let mut values = Vec::new();
            for child in children {
                if child.tag_name().name() == "key" {
                    return Err("A key must be inside a dictionary.".into());
                }
                values.push(read(child, depth + 1, budget)?);
            }
            Ok(typed("array", Value::Array(values)))
        }
        "dict" => {
            if !trimmed.is_empty() || !children.len().is_multiple_of(2) {
                return Err(
                    "Each dictionary key needs a value, with no direct container text.".into(),
                );
            }
            let mut values = Map::new();
            for pair in children.as_chunks::<2>().0 {
                if pair[0].tag_name().name() != "key" || pair[1].tag_name().name() == "key" {
                    return Err("Plist dictionary keys must be followed by a value.".into());
                }
                let key = read(pair[0], depth + 1, budget)?
                    .as_str()
                    .ok_or("Invalid plist dictionary key.")?
                    .to_owned();
                if values.contains_key(&key) {
                    return Err("Plist dictionary keys must be unique.".into());
                }
                values.insert(key, read(pair[1], depth + 1, budget)?);
            }
            let mut sorted: Vec<_> = values.into_iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(typed("dict", Value::Object(sorted.into_iter().collect())))
        }
        _ => {
            if !children.is_empty() {
                return Err("A plist scalar cannot contain elements.".into());
            }
            match kind {
                "key" => {
                    validate_text(&text)?;
                    Ok(Value::String(text))
                }
                "string" => {
                    validate_text(&text)?;
                    Ok(typed(kind, Value::String(text)))
                }
                "true" | "false" => {
                    if !trimmed.is_empty() {
                        return Err("Boolean elements must be empty.".into());
                    }
                    Ok(typed("bool", Value::Bool(kind == "true")))
                }
                "integer" => Ok(typed(kind, integer(trimmed)?)),
                "real" => Ok(typed(kind, real(trimmed)?)),
                "date" => {
                    if !valid_date(trimmed) {
                        return Err("Plist dates use YYYY-MM-DDTHH:mm:ssZ in UTC.".into());
                    }
                    Ok(typed(kind, Value::String(trimmed.into())))
                }
                "data" => {
                    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
                    let data = STANDARD
                        .decode(compact)
                        .map_err(|_| "Invalid Base64 plist data.")?;
                    Ok(typed(kind, Value::String(STANDARD.encode(data))))
                }
                _ => Err("Invalid plist element.".into()),
            }
        }
    }
}
fn push(out: &mut String, text: &str) -> R<()> {
    if out.len().saturating_add(text.len()) > MAX_OUTPUT {
        return Err("Plist output exceeds 4 MB; nothing was truncated.".into());
    }
    out.push_str(text);
    Ok(())
}
fn write(node: &Value, depth: usize, budget: &mut usize, out: &mut String) -> R<()> {
    step(depth, budget)?;
    let fields = node
        .as_object()
        .ok_or("Each typed plist node must contain type and value.")?;
    if fields.len() != 2 || !fields.contains_key("type") || !fields.contains_key("value") {
        return Err("Use typed JSON from this tool: each node has only type and value.".into());
    }
    let kind = fields["type"]
        .as_str()
        .ok_or("Plist type must be a string.")?;
    let value = &fields["value"];
    match kind {
        "dict" => {
            let fields = value
                .as_object()
                .ok_or("Plist dict needs an object value.")?;
            push(out, "<dict>")?;
            let mut keys: Vec<_> = fields.keys().collect();
            keys.sort();
            for key in keys {
                push(out, &format!("<key>{}</key>", escape(key)?))?;
                write(&fields[key], depth + 1, budget, out)?;
            }
            push(out, "</dict>")
        }
        "array" => {
            let values = value
                .as_array()
                .ok_or("Plist array needs an array value.")?;
            push(out, "<array>")?;
            for value in values {
                write(value, depth + 1, budget, out)?;
            }
            push(out, "</array>")
        }
        "string" => {
            let text = value.as_str().ok_or("Plist string needs a string value.")?;
            push(out, &format!("<string>{}</string>", escape(text)?))
        }
        "bool" => {
            let flag = value.as_bool().ok_or("Plist bool needs a boolean value.")?;
            push(out, if flag { "<true/>" } else { "<false/>" })
        }
        "integer" => {
            let number = value
                .as_number()
                .ok_or("Plist integer needs a numeric value.")?
                .to_string();
            integer(&number)?;
            push(out, &format!("<integer>{number}</integer>"))
        }
        "real" => {
            let number = value
                .as_number()
                .ok_or("Plist real needs a numeric value.")?
                .to_string();
            let normalized = real(&number)?.to_string();
            push(out, &format!("<real>{normalized}</real>"))
        }
        "date" => {
            let date = value
                .as_str()
                .filter(|s| valid_date(s))
                .ok_or("Plist dates use YYYY-MM-DDTHH:mm:ssZ in UTC.")?;
            push(out, &format!("<date>{date}</date>"))
        }
        "data" => {
            let data = STANDARD
                .decode(value.as_str().ok_or("Plist data needs a Base64 string.")?)
                .map_err(|_| "Plist data must be valid Base64.")?;
            push(out, &format!("<data>{}</data>", STANDARD.encode(data)))
        }
        _ => Err(format!(
            "Unsupported plist type or mismatched value: {}.",
            kind.chars().take(40).collect::<String>()
        )),
    }
}
pub(super) fn convert(input: &str, option: &str) -> R<String> {
    if input.len() > super::MAX_INPUT {
        return Err("Plist input exceeds 500,000 UTF-8 bytes.".into());
    }
    let reverse = match option.trim() {
        "" => input.trim_start().starts_with('{'),
        "plist-json" | "Plist → JSON" => false,
        "json-plist" | "JSON → Plist" => true,
        _ => return Err("Use Plist → JSON or JSON → Plist.".into()),
    };
    if reverse {
        let tree = super::parse_json(input)?;
        let mut out = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{DOCTYPE}\n<plist version=\"1.0\">\n"
        );
        write(&tree, 0, &mut 50_000, &mut out)?;
        push(&mut out, "\n</plist>")?;
        Ok(out)
    } else {
        let clean = input.replace(DOCTYPE, "");
        let upper = clean.to_ascii_uppercase();
        if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
            return Err("Custom DTDs and entities are not accepted. The standard plist declaration is handled locally.".into());
        }
        let doc = roxmltree::Document::parse_with_options(
            &clean,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                nodes_limit: 80_001,
            },
        )
        .map_err(|e| format!("Invalid XML property list: {e}"))?;
        if doc.root_element().tag_name().name() != "plist" {
            return Err("XML needs a plist root element.".into());
        }
        let mut budget = MAX_NODES;
        let value = read(doc.root_element(), 0, &mut budget)?;
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_roundtrip_preserves_dates_data_scalars_and_reserved_keys() {
        let input = "<plist version=\"1.0\"><dict><key>type</key><string>data</string><key>empty</key><array/><key>data</key><data>AAH/</data><key>date</key><date>2026-09-12T00:00:00Z</date><key>int</key><integer>18446744073709551615</integer><key>real</key><real>2.5</real><key>flag</key><false/></dict></plist>";
        let json = convert(input, "").unwrap();
        let xml = convert(&json, "json-plist").unwrap();
        assert_eq!(convert(&xml, "").unwrap(), json);
        assert!(json.contains("18446744073709551615"));
        assert!(json.contains("AAH/"));
    }
    #[test]
    fn rejects_entities_duplicates_controls_and_wrong_types() {
        for input in [
            "<!DOCTYPE plist [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><plist><string>&x;</string></plist>",
            "<plist><dict><key>a</key><true/><key>a</key><false/></dict></plist>",
            "<plist><string><true/></string></plist>",
            "<plist><date>2026-02-30T00:00:00Z</date></plist>",
            "<plist><array><key>x</key></array></plist>",
            "<plist><integer>18446744073709551616</integer></plist>",
            "<plist><real>NaN</real></plist>",
            "<plist><string a='x'>x</string></plist>",
        ] {
            assert!(convert(input, "").is_err(), "{input}");
        }
        assert!(convert(r#"{"type":"string","value":"\u0000"}"#, "json-plist").is_err());
        assert!(convert(r#"{"type":"integer","value":"2"}"#, "json-plist").is_err());
    }
    #[test]
    fn depth_dates_and_namespace_are_strict() {
        let deep = format!(
            "<plist>{}{}</plist>",
            "<array>".repeat(65),
            "</array>".repeat(65)
        );
        assert!(convert(&deep, "").is_err());
        assert!(
            convert(
                "<plist xmlns='https://invalid.example'><string>x</string></plist>",
                ""
            )
            .is_err()
        );
        for date in [
            "0000-01-01T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-12-31T23:59:60Z",
        ] {
            assert!(!valid_date(date));
        }
    }
    #[test]
    fn standard_doctype_and_literal_carriage_return() {
        let json = r#"{"type":"dict","value":{"a\rb":{"type":"string","value":"<&>\r\n😀"}}}"#;
        let xml = convert(json, "json-plist").unwrap();
        assert!(xml.contains(DOCTYPE));
        assert!(xml.contains("&#13;"));
        assert_eq!(
            super::super::parse_json(&convert(&xml, "").unwrap()).unwrap(),
            super::super::parse_json(json).unwrap()
        );
    }
}
