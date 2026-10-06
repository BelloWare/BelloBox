//! Source: TextUtilities.swift's listSet branch. Documents and options stay separate.
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

fn newline(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}
// CoreFoundation CFUniChar.c whitespace table includes ZERO WIDTH SPACE.
fn whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200b}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// Runs one source operation, retaining the first spelling of each matching item.
/// Swift String equality is canonically equivalent, including in Exact mode.
pub fn run(a: &str, b: &str, operation: &str, matching: &str) -> Result<String, String> {
    if a.len().saturating_add(b.len()) > super::MAX_INPUT {
        return Err("Inputs exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    if ![
        "Union",
        "Intersection",
        "A − B",
        "B − A",
        "Symmetric difference",
    ]
    .contains(&operation)
    {
        return Err("Choose a supported list operation.".into());
    }
    if !["Exact", "Trim", "Trim & ignore case"].contains(&matching) {
        return Err("Choose Exact, Trim, or Trim & ignore case matching.".into());
    }
    let list = |text: &str| {
        let mut seen = HashSet::new();
        text.split(newline)
            .filter_map(|line| {
                let value = if matching == "Exact" {
                    line
                } else {
                    line.trim_matches(whitespace)
                };
                if value.trim_matches(whitespace).is_empty() {
                    return None;
                }
                let key: String = if matching == "Trim & ignore case" {
                    value.to_lowercase().nfc().collect()
                } else {
                    value.nfc().collect()
                };
                seen.insert(key.clone()).then(|| (key, value.to_owned()))
            })
            .collect::<Vec<_>>()
    };
    let a = list(a);
    let b = list(b);
    let keys_a: HashSet<_> = a.iter().map(|(key, _)| key).collect();
    let keys_b: HashSet<_> = b.iter().map(|(key, _)| key).collect();
    let mut output = Vec::new();
    if operation != "B − A" {
        output.extend(
            a.iter()
                .filter(|(key, _)| match operation {
                    "Intersection" => keys_b.contains(key),
                    "A − B" | "Symmetric difference" => !keys_b.contains(key),
                    _ => true,
                })
                .map(|(_, value)| value.as_str()),
        );
    }
    if matches!(operation, "Union" | "B − A" | "Symmetric difference") {
        output.extend(
            b.iter()
                .filter(|(key, _)| !keys_a.contains(key))
                .map(|(_, value)| value.as_str()),
        );
    }
    let output = output.join("\n");
    if output.len() > super::MAX_OUTPUT {
        return Err("Output exceeds 4 MB; nothing was truncated.".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::run;
    #[test]
    fn all_operations_preserve_source_order_and_first_spelling() {
        for (mode, expected) in [
            ("Union", "Swift\nRust\nPython\nGo"),
            ("Intersection", "Swift\nRust"),
            ("A − B", "Python"),
            ("B − A", "Go"),
            ("Symmetric difference", "Python\nGo"),
        ] {
            assert_eq!(
                run(
                    "Swift\nRust\nPython\nRust\n\t",
                    "Rust\nGo\nSwift\nGo",
                    mode,
                    "Exact"
                )
                .unwrap(),
                expected
            );
        }
    }
    #[test]
    fn matching_modes_keep_documents_literal() {
        let a = " Rust \nRust\nrust\nSWIFT";
        let b = "rust\nswift\n Go ";
        for (matching, expected) in [
            ("Exact", " Rust \nRust\nrust\nSWIFT\nswift\n Go "),
            ("Trim", "Rust\nrust\nSWIFT\nswift\nGo"),
            ("Trim & ignore case", "Rust\nSWIFT\nGo"),
        ] {
            assert_eq!(run(a, b, "Union", matching).unwrap(), expected);
        }
    }
    #[test]
    fn foundation_line_separators_whitespace_and_unicode_equality() {
        assert_eq!(run("\u{200b}", "", "Union", "Exact").unwrap(), "");
        assert_eq!(
            run("\u{200b}A\u{200b}", "A", "Intersection", "Trim").unwrap(),
            "A"
        );
        assert_eq!(
            run(
                "a\rb\r\nc\u{b}d\u{c}e\u{85}f\u{2028}g\u{2029}h",
                "",
                "Union",
                "Exact"
            )
            .unwrap(),
            "a\nb\nc\nd\ne\nf\ng\nh"
        );
        assert_eq!(
            run(
                "\u{a0}é\u{3000}\n\u{2009}\n",
                "e\u{301}",
                "Intersection",
                "Trim"
            )
            .unwrap(),
            "é"
        );
        assert_eq!(
            run("e\u{301}\né", "é", "Intersection", "Exact").unwrap(),
            "e\u{301}"
        );
        assert_eq!(
            run(
                "É\nΟΣ\nİ\nß",
                "e\u{301}\nος\ni\u{307}\nSS",
                "Intersection",
                "Trim & ignore case"
            )
            .unwrap(),
            "É\nΟΣ\nİ"
        );
    }
    #[test]
    fn empty_inputs_bounds_and_bad_options() {
        assert_eq!(run("", "B", "Union", "Exact").unwrap(), "B");
        assert_eq!(run("A", "", "B − A", "Exact").unwrap(), "");
        assert_eq!(run("", "", "Union", "Exact").unwrap(), "");
        assert!(run("", "", "bad", "Exact").is_err());
        assert!(run("", "", "Union", "bad").is_err());
        assert!(run(&"a".repeat(super::super::MAX_INPUT), "b", "Union", "Exact").is_err());
        assert_eq!(
            run(&"a".repeat(super::super::MAX_INPUT), "", "Union", "Exact")
                .unwrap()
                .len(),
            super::super::MAX_INPUT
        );
    }
}
