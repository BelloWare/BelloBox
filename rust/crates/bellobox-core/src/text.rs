use base64::{Engine, engine::general_purpose::STANDARD};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
const URL_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseStyle {
    Upper,
    Lower,
    Title,
    Sentence,
    Camel,
    Pascal,
    Snake,
    Kebab,
    Constant,
}

pub fn words(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if ch.is_uppercase()
                && previous.is_some_and(|c| c.is_lowercase() || c.is_numeric())
                && !current.is_empty()
            {
                result.push(std::mem::take(&mut current));
            }
            current.push(ch);
        } else if !current.is_empty() {
            result.push(std::mem::take(&mut current));
        }
        previous = Some(ch);
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}
fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|first| first.to_uppercase().collect::<String>() + &c.as_str().to_lowercase())
        .unwrap_or_default()
}
pub fn case(text: &str, style: CaseStyle) -> String {
    match style {
        CaseStyle::Upper => text.to_uppercase(),
        CaseStyle::Lower => text.to_lowercase(),
        CaseStyle::Title => text
            .split(' ')
            .map(capitalized)
            .collect::<Vec<_>>()
            .join(" "),
        CaseStyle::Sentence => {
            let mut start = true;
            let mut out = String::new();
            for c in text.to_lowercase().chars() {
                if start && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    start = false;
                } else {
                    out.push(c);
                    if ".!?\n".contains(c) {
                        start = true;
                    }
                }
            }
            out
        }
        _ => {
            let w = words(text);
            match style {
                CaseStyle::Camel => w
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        if i == 0 {
                            s.to_lowercase()
                        } else {
                            capitalized(s)
                        }
                    })
                    .collect(),
                CaseStyle::Pascal => w.iter().map(|s| capitalized(s)).collect(),
                CaseStyle::Snake => w
                    .iter()
                    .map(|s| s.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("_"),
                CaseStyle::Kebab => w
                    .iter()
                    .map(|s| s.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("-"),
                CaseStyle::Constant => w
                    .iter()
                    .map(|s| s.to_uppercase())
                    .collect::<Vec<_>>()
                    .join("_"),
                _ => unreachable!(),
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Base64,
    Url,
    Html,
    Hex,
}
pub fn encode(text: &str, encoding: Encoding) -> String {
    match encoding {
        Encoding::Base64 => STANDARD.encode(text),
        Encoding::Url => utf8_percent_encode(text, URL_SET).to_string(),
        Encoding::Hex => text.as_bytes().iter().map(|b| format!("{b:02x}")).collect(),
        Encoding::Html => text
            .chars()
            .map(|c| match c {
                '&' => "&amp;".into(),
                '<' => "&lt;".into(),
                '>' => "&gt;".into(),
                '"' => "&quot;".into(),
                '\'' => "&#39;".into(),
                _ => c.to_string(),
            })
            .collect(),
    }
}
pub fn decode(text: &str, encoding: Encoding) -> Result<String, String> {
    match encoding {
        Encoding::Base64 => {
            String::from_utf8(STANDARD.decode(text.trim()).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())
        }
        Encoding::Url => {
            let bytes = text.as_bytes();
            for i in 0..bytes.len() {
                if bytes[i] == b'%'
                    && (i + 2 >= bytes.len()
                        || !bytes[i + 1].is_ascii_hexdigit()
                        || !bytes[i + 2].is_ascii_hexdigit())
                {
                    return Err("Invalid percent escape.".into());
                }
            }
            percent_decode_str(&text.replace('+', " "))
                .decode_utf8()
                .map(|s| s.into_owned())
                .map_err(|e| e.to_string())
        }
        Encoding::Hex => {
            let s: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if !s.is_ascii() || !s.len().is_multiple_of(2) {
                return Err("Hex requires pairs of ASCII hex digits.".into());
            }
            let bytes = (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            String::from_utf8(bytes).map_err(|e| e.to_string())
        }
        Encoding::Html => {
            // A single pass prevents recursively decoding e.g. &amp;lt;.
            let mut out = String::new();
            let mut rest = text;
            while let Some(i) = rest.find('&') {
                out.push_str(&rest[..i]);
                rest = &rest[i..];
                let Some(end) = rest.find(';').filter(|end| *end <= 16) else {
                    out.push('&');
                    rest = &rest[1..];
                    continue;
                };
                let entity = &rest[1..end];
                let c = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => entity
                        .strip_prefix("#x")
                        .and_then(|n| u32::from_str_radix(n, 16).ok())
                        .or_else(|| entity.strip_prefix('#').and_then(|n| n.parse().ok()))
                        .and_then(char::from_u32),
                };
                if let Some(c) = c {
                    out.push(c);
                } else {
                    out.push_str(&rest[..=end]);
                }
                rest = &rest[end + 1..];
            }
            out.push_str(rest);
            Ok(out)
        }
    }
}
pub fn auto_decode(text: &str) -> Result<(Encoding, String), String> {
    let text = text.trim();
    for encoding in [
        Encoding::Url,
        Encoding::Html,
        Encoding::Base64,
        Encoding::Hex,
    ] {
        let plausible = match encoding {
            Encoding::Url => text
                .as_bytes()
                .windows(3)
                .any(|w| w[0] == b'%' && w[1].is_ascii_hexdigit() && w[2].is_ascii_hexdigit()),
            Encoding::Html => text.contains('&') && text.contains(';'),
            Encoding::Base64 => !text.is_empty() && text.len().is_multiple_of(4),
            Encoding::Hex => !text.is_empty(),
        };
        if plausible
            && let Ok(output) = decode(text, encoding)
            && output != text
            && output.chars().all(|c| !c.is_control() || c.is_whitespace())
        {
            return Ok((encoding, output));
        }
    }
    Err("No supported text encoding detected; choose a format explicitly.".into())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOperation {
    Sort,
    SortReverse,
    Reverse,
    Unique,
    Nonempty,
    Trim,
}
pub fn lines(text: &str, operation: LineOperation) -> String {
    let mut rows: Vec<String> = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(str::to_owned)
        .collect();
    match operation {
        LineOperation::Sort => rows.sort(),
        LineOperation::SortReverse => rows.sort_by(|a, b| b.cmp(a)),
        LineOperation::Reverse => rows.reverse(),
        LineOperation::Unique => {
            let mut seen = HashSet::new();
            rows.retain(|s| seen.insert(s.clone()));
        }
        LineOperation::Nonempty => rows.retain(|s| !s.trim().is_empty()),
        LineOperation::Trim => rows = rows.iter().map(|s| s.trim().to_owned()).collect(),
    }
    rows.join("\n")
}
#[derive(Debug, PartialEq, Eq)]
pub struct Counts {
    pub characters: usize,
    pub without_whitespace: usize,
    pub words: usize,
    pub lines: usize,
    pub estimated_tokens: usize,
    pub tokenizer_family: &'static str,
}
pub fn counts(text: &str, model: &str) -> Counts {
    let lower = model.to_lowercase();
    let (family, multiplier) = if lower.contains("claude") {
        ("Claude heuristic", 1.08)
    } else if ["gpt-4o", "gpt-4.1", "gpt-5", "o1", "o3", "o4"]
        .iter()
        .any(|m| lower.contains(m))
    {
        ("o200k_base heuristic", 0.95)
    } else {
        ("generic heuristic", 1.0)
    };
    let base = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|w| w.chars().count().div_ceil(4))
        .sum::<usize>()
        + text
            .chars()
            .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
            .count();
    Counts {
        characters: text.chars().count(),
        without_whitespace: text.chars().filter(|c| !c.is_whitespace()).count(),
        words: text.split_whitespace().count(),
        lines: if text.is_empty() {
            0
        } else {
            text.split('\n').count()
        },
        estimated_tokens: if text.is_empty() {
            0
        } else {
            ((base as f64 * multiplier).round() as usize).max(1)
        },
        tokenizer_family: family,
    }
}
pub fn hashes(text: &str) -> String {
    format!(
        "MD5     {:x}\nSHA-1   {:x}\nSHA-256 {:x}\nSHA-512 {:x}\n\nMD5 and SHA-1 are for compatibility, not security.",
        Md5::digest(text),
        Sha1::digest(text),
        Sha256::digest(text),
        Sha512::digest(text)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_case_and_boundaries() {
        assert_eq!(
            case("helloWorld café", CaseStyle::Snake),
            "hello_world_café"
        );
        assert_eq!(case("ß", CaseStyle::Upper), "SS");
    }
    #[test]
    fn encodings_round_trip_utf8() {
        for e in [
            Encoding::Base64,
            Encoding::Url,
            Encoding::Html,
            Encoding::Hex,
        ] {
            assert_eq!(decode(&encode("<& café 界", e), e).unwrap(), "<& café 界");
        }
    }
    #[test]
    fn rejects_invalid_encodings() {
        assert!(decode("a", Encoding::Hex).is_err());
        assert!(decode("%xx", Encoding::Url).is_err());
        assert!(decode("ff", Encoding::Hex).is_err());
    }
    #[test]
    fn html_is_single_pass() {
        assert_eq!(
            decode("&amp;lt; &#x1f600;", Encoding::Html).unwrap(),
            "&lt; 😀"
        );
    }
    #[test]
    fn auto_decode_does_not_treat_plain_plus_as_url() {
        assert!(auto_decode("one+two").is_err());
        assert_eq!(
            auto_decode("aGVsbG8=").unwrap(),
            (Encoding::Base64, "hello".into())
        );
        assert_eq!(
            auto_decode("hello%20world").unwrap(),
            (Encoding::Url, "hello world".into())
        );
    }
    #[test]
    fn stable_unique_and_count() {
        assert_eq!(lines("b\na\nb", LineOperation::Unique), "b\na");
        assert_eq!(counts("é界", "").characters, 2);
        assert_eq!(counts("", "").estimated_tokens, 0);
    }
    #[test]
    fn known_hashes() {
        assert!(
            hashes("hello")
                .contains("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        );
    }
}
