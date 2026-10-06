//! TextUtilities.swift String Literal Escaper. Generates text; never executes it.
use super::{R, compact, html_escape, parse_json};
use serde_json::Value;
use std::fmt::Write;

pub(super) fn run(input: &str, mode: &str) -> R<String> {
    match mode.trim() {
        "" | "json" | "JSON quote" => compact(&Value::String(input.into())),
        "unescape" | "JSON unquote" => {
            let value = parse_json(input)?;
            value.as_str().map(str::to_owned).ok_or_else(|| {
                "Unquote expects one JSON string, including its double quotes.".into()
            })
        }
        "Swift literal" => {
            let mut output = String::from("\"");
            for scalar in input.chars() {
                match scalar {
                    '"' => output.push_str("\\\""),
                    '\\' => output.push_str("\\\\"),
                    '\n' => output.push_str("\\n"),
                    '\r' => output.push_str("\\r"),
                    '\t' => output.push_str("\\t"),
                    '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2028}' | '\u{2029}' => {
                        write!(output, "\\u{{{:x}}}", scalar as u32)
                            .expect("String writes cannot fail");
                    }
                    _ => output.push(scalar),
                }
            }
            output.push('"');
            Ok(output)
        }
        "shell" | "Shell quote" => {
            if input.contains('\0') {
                return Err("A shell argument cannot contain a NUL character.".into());
            }
            Ok(format!("'{}'", input.replace('\'', "'\"'\"'")))
        }
        // Compatibility extension for existing CLI callers; not a source UI choice.
        "html" => Ok(html_escape(input)),
        _ => Err(
            "Choose JSON quote, JSON unquote, Swift literal, or Shell quote. No code is executed."
                .into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::super::execute;
    fn run(input: &str, mode: &str) -> Result<String, String> {
        execute("stringEscape", input, mode)
    }
    #[test]
    fn string_literal_swift_scalar_boundaries_and_interpolation_are_literal() {
        assert_eq!(run("\0\u{8}\t\n\u{b}\u{c}\r\u{1f} ~\u{7f}\u{80}\u{9f}\u{a0}\u{2028}\u{2029}é😀e\u{301}", "Swift literal").unwrap(), "\"\\u{0}\\u{8}\\t\\n\\u{b}\\u{c}\\r\\u{1f} ~\\u{7f}\\u{80}\\u{9f}\u{a0}\\u{2028}\\u{2029}é😀e\u{301}\"");
        assert_eq!(
            run("say \"Hi\" \\(danger) \\path", "Swift literal").unwrap(),
            "\"say \\\"Hi\\\" \\\\(danger) \\\\path\""
        );
    }
    #[test]
    fn string_literal_json_roundtrip_exact_encoding_and_surrogates() {
        let input = "\"/\\\n\t\0😀e\u{301}";
        let quoted = run(input, "JSON quote").unwrap();
        assert_eq!(quoted, "\"\\\"/\\\\\\n\\t\\u0000😀e\u{301}\"");
        assert_eq!(run(&quoted, "JSON unquote").unwrap(), input);
        assert_eq!(run(r#""\ud83d\ude00""#, "JSON unquote").unwrap(), "😀");
        for input in [
            "[]",
            "{}",
            "1",
            "null",
            "true",
            r#""\x00""#,
            r#""\ud800""#,
            r#""\udc00""#,
            r#""x" "y""#,
            "\"unterminated",
        ] {
            assert!(run(input, "JSON unquote").is_err(), "{input}");
        }
    }
    #[test]
    fn string_literal_shell_spelling_nul_and_cli_aliases() {
        assert_eq!(run("O'Reilly", "Shell quote").unwrap(), "'O'\"'\"'Reilly'");
        assert_eq!(
            run("$(touch file);\n`echo hi`\\x", "Shell quote").unwrap(),
            "'$(touch file);\n`echo hi`\\x'"
        );
        assert!(run("a\0b", "Shell quote").is_err());
        for (alias, label, input) in [
            ("json", "JSON quote", "hello"),
            ("unescape", "JSON unquote", "\"hello\""),
            ("shell", "Shell quote", "O'Reilly"),
        ] {
            assert_eq!(run(input, alias).unwrap(), run(input, label).unwrap());
        }
        assert_eq!(run("<&>", "html").unwrap(), "&lt;&amp;&gt;");
    }
    #[test]
    fn string_literal_empty_unknown_and_bounded_expansion() {
        for mode in ["JSON quote", "Swift literal"] {
            assert_eq!(run("", mode).unwrap(), "\"\"");
        }
        assert_eq!(run("", "Shell quote").unwrap(), "''");
        assert_eq!(run("\"\"", "JSON unquote").unwrap(), "");
        assert!(run("anything", "bad").is_err());
        assert!(run(&"x".repeat(500_001), "JSON quote").is_err());
        // Option text also counts toward the existing combined-input limit.
        let input = "\u{1f}".repeat(500_000 - "Swift literal".len());
        let out = run(&input, "Swift literal").unwrap();
        assert_eq!(out.len(), input.len() * 6 + 2);
    }
}
