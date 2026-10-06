//! Source GUI behavior from MathUtilities.swift's NumberBaseTool.
//!
//! This API keeps explicit input/output choices separate from the legacy CLI's
//! prefix autodetection and colon-labeled output. Invalid non-ASCII diagnostics
//! identify Unicode scalars; Swift identifies extended grapheme clusters.

const BASES: [u32; 4] = [2, 8, 10, 16];

// Foundation's whitespacesAndNewlines includes ZERO WIDTH SPACE, unlike Rust's
// standard whitespace predicate. Keep the source trimming behavior at the edges.
fn whitespace_and_newlines(c: char) -> bool {
    matches!(
        c,
        '\t'..='\r' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200b}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// Convert an exact integer with source GUI choices (defaults: `10`, `None`).
/// Only the prefix matching the selected input base is removed. `None` returns
/// all four labeled bases; a selected output base returns lowercase digits only.
pub fn run(input: &str, input_base: u32, output_base: Option<u32>) -> Result<String, String> {
    if input.len() > super::MAX_INPUT {
        return Err("Inputs exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    if !BASES.contains(&input_base) {
        return Err("Choose base 2, 8, 10, or 16.".into());
    }
    let raw = input.trim_matches(whitespace_and_newlines).to_lowercase();
    let negative = raw.starts_with('-');
    let unsigned = raw.strip_prefix(['-', '+']).unwrap_or(&raw);
    let prefix = match input_base {
        2 => "0b",
        8 => "0o",
        16 => "0x",
        _ => "",
    };
    let digits = unsigned.strip_prefix(prefix).unwrap_or(unsigned);
    if digits.is_empty() || digits.len() > 256 {
        return Err("Enter 1–256 digits in the chosen base.".into());
    }
    let mut value = ExactInteger::zero();
    for c in digits.chars() {
        let digit = c
            .to_digit(input_base)
            .ok_or_else(|| format!("‘{c}’ is not a base-{input_base} digit."))?;
        value.push_digit(input_base, digit);
    }
    let output = if let Some(radix) = output_base {
        if !BASES.contains(&radix) {
            return Err("Choose output base 2, 8, 10, or 16.".into());
        }
        value.render(radix, negative)
    } else {
        format!(
            "Decimal  {}\nHex      {}\nOctal    {}\nBinary   {}",
            value.render(10, negative),
            value.render(16, negative).to_uppercase(),
            value.render(8, negative),
            value.render(2, negative)
        )
    };
    if output.len() > super::MAX_OUTPUT {
        return Err("Result exceeds 4 MB; use a smaller input. Nothing was truncated.".into());
    }
    Ok(output)
}

/// Shared exact byte-array arithmetic only; GUI and CLI parsers remain separate.
pub(super) struct ExactInteger {
    bytes: Vec<u32>,
}

impl ExactInteger {
    pub(super) fn zero() -> Self {
        Self { bytes: vec![0] }
    }

    pub(super) fn push_digit(&mut self, radix: u32, digit: u32) {
        let mut carry = digit;
        for byte in &mut self.bytes {
            let value = *byte * radix + carry;
            *byte = value % 256;
            carry = value / 256;
        }
        while carry > 0 {
            self.bytes.push(carry % 256);
            carry /= 256;
        }
    }

    pub(super) fn render(&self, radix: u32, negative: bool) -> String {
        let mut remaining = self.bytes.clone();
        let mut digits = Vec::new();
        while remaining.iter().any(|n| *n > 0) {
            let mut remainder = 0;
            for byte in remaining.iter_mut().rev() {
                let value = remainder * 256 + *byte;
                *byte = value / radix;
                remainder = value % radix;
            }
            digits.push(char::from_digit(remainder, radix).expect("a supported radix"));
        }
        if digits.is_empty() {
            digits.push('0');
        }
        let mut output: String = digits.into_iter().rev().collect();
        if negative && output != "0" {
            output.insert(0, '-');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::developer::execute;

    #[test]
    fn source_all_bases_labels_spacing_order_and_hex_case() {
        assert_eq!(
            run("255", 10, None).unwrap(),
            "Decimal  255\nHex      FF\nOctal    377\nBinary   11111111"
        );
        assert_eq!(
            run("-0xFf", 16, None).unwrap(),
            "Decimal  -255\nHex      -FF\nOctal    -377\nBinary   -11111111"
        );
        for (base, expected) in [(2, "11111111"), (8, "377"), (10, "255"), (16, "ff")] {
            assert_eq!(run("255", 10, Some(base)).unwrap(), expected);
            assert_eq!(run("-255", 10, Some(base)).unwrap(), format!("-{expected}"));
        }
    }

    #[test]
    fn exact_beyond_64_bits_and_at_maximum_digit_length() {
        assert_eq!(
            run("340282366920938463463374607431768211455", 10, Some(16)).unwrap(),
            "ffffffffffffffffffffffffffffffff"
        );
        assert_eq!(
            run("100000000000000000000000000000000", 16, Some(10)).unwrap(),
            "340282366920938463463374607431768211456"
        );
        let hex = "F".repeat(256);
        let binary = "1".repeat(1024);
        assert_eq!(run(&hex, 16, Some(2)).unwrap(), binary);
        assert_eq!(
            run(&format!("-0x{hex}"), 16, Some(2)).unwrap(),
            format!("-{binary}")
        );
        let all = run(&hex, 16, None).unwrap();
        assert!(all.contains(&format!("\nHex      {hex}\n")));
        assert!(all.ends_with(&format!("\nBinary   {binary}")));
        assert!(all.len() < super::super::MAX_OUTPUT);
        assert_eq!(
            run(&"F".repeat(257), 16, None).unwrap_err(),
            "Enter 1–256 digits in the chosen base."
        );
    }

    #[test]
    fn signs_matching_prefixes_and_negative_zero() {
        for (text, base, expected) in [
            ("+0B1010", 2, "10"),
            ("-0O17", 8, "-15"),
            ("+0XfF", 16, "255"),
            ("00042", 10, "42"),
            ("0b10", 16, "2832"),
            ("-0B10", 16, "-2832"),
            ("-0", 10, "0"),
            ("-0b000", 2, "0"),
            ("-0o000", 8, "0"),
            ("-0x000", 16, "0"),
        ] {
            assert_eq!(run(text, base, Some(10)).unwrap(), expected, "{text}");
        }
        assert_eq!(
            run("-0", 10, None).unwrap(),
            "Decimal  0\nHex      0\nOctal    0\nBinary   0"
        );
        for text in ["++1", "--1", "+-1", "-+1", "- 1", "1 0", "1_0", "1.0"] {
            assert!(run(text, 10, None).is_err(), "{text}");
        }
        for (text, base, digit) in [("0xFF", 10, 'x'), ("0b10", 8, 'b'), ("0o10", 16, 'o')] {
            assert_eq!(
                run(text, base, None).unwrap_err(),
                format!("‘{digit}’ is not a base-{base} digit.")
            );
        }
    }

    #[test]
    fn foundation_trim_includes_zero_width_space_but_not_internal_space() {
        for space in [
            '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}',
            '\u{200b}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
        ] {
            assert_eq!(
                run(&format!("{space}+0xFf{space}"), 16, Some(10)).unwrap(),
                "255"
            );
        }
        assert!(run("1\u{200b}0", 10, None).is_err());
        for text in ["\u{feff}1", "\u{180e}1", "１２", "١٢", "é", "💡"] {
            assert!(run(text, 10, None).is_err(), "{text}");
        }
        // Swift groups this combining mark with the preceding ASCII letter;
        // Rust's invalid-input diagnostic identifies the mark's scalar alone.
        assert_eq!(
            run("a\u{301}", 16, None).unwrap_err(),
            "‘\u{301}’ is not a base-16 digit."
        );
    }

    #[test]
    fn validation_order_is_input_base_then_length_then_digit_then_output_base() {
        assert_eq!(
            run("", 3, Some(3)).unwrap_err(),
            "Choose base 2, 8, 10, or 16."
        );
        for text in ["", "+", "-", "0x", "+0X", "-0x"] {
            assert_eq!(
                run(text, 16, Some(3)).unwrap_err(),
                "Enter 1–256 digits in the chosen base."
            );
        }
        assert_eq!(
            run(&"g".repeat(257), 16, Some(3)).unwrap_err(),
            "Enter 1–256 digits in the chosen base."
        );
        assert_eq!(
            run("G", 16, Some(3)).unwrap_err(),
            "‘g’ is not a base-16 digit."
        );
        assert_eq!(
            run("2", 2, Some(3)).unwrap_err(),
            "‘2’ is not a base-2 digit."
        );
        assert_eq!(
            run("1", 10, Some(3)).unwrap_err(),
            "Choose output base 2, 8, 10, or 16."
        );
        assert_eq!(
            run(&"é".repeat(129), 16, None).unwrap_err(),
            "Enter 1–256 digits in the chosen base."
        );
        assert_eq!(
            run(&"é".repeat(128), 16, None).unwrap_err(),
            "‘é’ is not a base-16 digit."
        );
    }

    #[test]
    fn direct_gui_api_enforces_full_input_byte_bound_before_trimming() {
        let input = format!("{}1", " ".repeat(super::super::MAX_INPUT - 1));
        assert_eq!(run(&input, 10, Some(10)).unwrap(), "1");
        assert_eq!(
            run(&format!("{input} "), 10, None).unwrap_err(),
            "Inputs exceed 500,000 UTF-8 bytes; nothing was truncated."
        );
        assert_eq!(
            run(&"\u{200b}".repeat(166_667), 10, None).unwrap_err(),
            "Inputs exceed 500,000 UTF-8 bytes; nothing was truncated."
        );
    }

    #[test]
    fn legacy_cli_keeps_autodetection_numeric_options_and_colon_labels() {
        assert_eq!(
            execute("numberBase", "0xFF", "").unwrap(),
            "Decimal: 255\nHex: ff\nOctal: 377\nBinary: 11111111"
        );
        assert_eq!(
            run("0xFF", 10, None).unwrap_err(),
            "‘x’ is not a base-10 digit."
        );
        assert_eq!(
            execute("numberBase", "0b10", "").unwrap(),
            "Decimal: 2\nHex: 2\nOctal: 2\nBinary: 10"
        );
        assert_eq!(
            execute("numberBase", "0b10", "16").unwrap_err(),
            "Input prefix and chosen base disagree."
        );
        assert_eq!(run("0b10", 16, Some(10)).unwrap(), "2832");
        for (text, option) in [("ff", "16"), ("377", "8"), ("11111111", "2"), ("255", "10")] {
            assert_eq!(
                execute("numberBase", text, option).unwrap(),
                "Decimal: 255\nHex: ff\nOctal: 377\nBinary: 11111111"
            );
        }
        assert_eq!(
            execute("numberBase", "1", "3").unwrap_err(),
            "Input prefix and chosen base disagree."
        );
        assert_eq!(
            execute("numberBase", "1", "hex").unwrap_err(),
            "Input base must be 2, 8, 10, or 16."
        );
        assert_eq!(
            execute("numberBase", "-0x000", "").unwrap(),
            "Decimal: 0\nHex: 0\nOctal: 0\nBinary: 0"
        );
        assert!(execute("numberBase", "\u{200b}1\u{200b}", "").is_err());
    }
}
