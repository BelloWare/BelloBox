//! Pure permissions calculator from SecurityUtilities.swift and its checkbox grid.
//! Generated commands are text only: this module never reads a path, executes a
//! command, or changes filesystem permissions.

const INPUT_ERROR: &str =
    "Enter 3–4 octal digits (755, 4755) or nine permission characters (rwxr-xr-x).";

// Foundation's whitespacesAndNewlines includes ZERO WIDTH SPACE, unlike trim().
fn source_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200b}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// Parse three/four ASCII octal digits or nine symbolic permission characters.
/// Symbolic input may have a leading `-`, `d`, or `l` file-type character.
///
/// Every accepted form and ASCII error matches the Swift source. For rejected
/// non-ASCII input, positions/counts use Unicode scalars rather than Swift's
/// extended grapheme clusters, so the error wording can differ.
pub fn parse(input: &str) -> Result<u16, String> {
    let raw = input.trim_matches(source_whitespace);
    if (3..=4).contains(&raw.len()) && raw.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return Ok(raw
            .bytes()
            .fold(0, |bits, b| bits * 8 + u16::from(b - b'0')));
    }

    // Eleven scalars are enough to reject an overlong input without allocating
    // proportionally to a draft. Valid permissions contain only ASCII.
    let chars: Vec<_> = raw.chars().take(11).collect();
    let chars = if chars.len() == 10 && matches!(chars[0], '-' | 'd' | 'l') {
        &chars[1..]
    } else {
        &chars[..]
    };
    if chars.len() != 9 {
        return Err(INPUT_ERROR.into());
    }

    let mut bits = 0;
    for (index, &character) in chars.iter().enumerate() {
        let expected = ['r', 'w', 'x'][index % 3];
        if character == expected {
            bits |= 1 << (8 - index);
        } else if index % 3 == 2
            && ((index < 8 && matches!(character, 's' | 'S'))
                || (index == 8 && matches!(character, 't' | 'T')))
        {
            bits |= match index {
                2 => 0o4000,
                5 => 0o2000,
                _ => 0o1000,
            };
            if matches!(character, 's' | 't') {
                bits |= 1 << (8 - index);
            }
        } else if character != '-' {
            return Err(format!(
                "Invalid permission character at position {}.",
                index + 1
            ));
        }
    }
    Ok(bits)
}

/// Format parsed permission bits with three digits, or four for special bits.
pub fn octal(bits: u16) -> String {
    format!("{bits:03o}")
}

/// Format the twelve permission bits, including set UID, set GID, and sticky.
pub fn symbolic(bits: u16) -> String {
    let mut chars = ['-'; 9];
    for (index, character) in chars.iter_mut().enumerate() {
        if bits & (1 << (8 - index)) != 0 {
            *character = ['r', 'w', 'x'][index % 3];
        }
    }
    for (index, special, lower, upper) in [
        (2, 0o4000, 's', 'S'),
        (5, 0o2000, 's', 'S'),
        (8, 0o1000, 't', 'T'),
    ] {
        if bits & special != 0 {
            chars[index] = if chars[index] == 'x' { lower } else { upper };
        }
    }
    chars.into_iter().collect()
}

/// Return the source's literal command preview without executing it.
pub fn run(input: &str) -> Result<String, String> {
    Ok(format!("chmod {} 'path/to/file'", octal(parse(input)?)))
}

/// Set one checkbox bit while preserving every other bit. Like the Swift grid,
/// invalid input starts from zero and the resulting draft is canonical octal.
pub fn toggle(input: &str, mask: u16, enabled: bool) -> Result<String, String> {
    if !mask.is_power_of_two() || mask & !0x0fff != 0 {
        return Err("Choose exactly one permission bit between 0001 and 4000.".into());
    }
    let old = parse(input).unwrap_or(0);
    Ok(octal(if enabled { old | mask } else { old & !mask }))
}

#[cfg(test)]
mod tests {
    use super::{INPUT_ERROR, octal, parse, run, symbolic, toggle};

    #[test]
    fn all_4096_masks_round_trip_every_source_form() {
        for bits in 0..=0o7777 {
            let numeric = octal(bits);
            let text = symbolic(bits);
            assert_eq!(numeric.len(), if bits > 0o777 { 4 } else { 3 });
            assert_eq!(text.len(), 9);
            assert_eq!(parse(&numeric).unwrap(), bits);
            assert_eq!(parse(&format!("{bits:04o}")).unwrap(), bits);
            assert_eq!(parse(&text).unwrap(), bits);
            for prefix in ['-', 'd', 'l'] {
                assert_eq!(parse(&format!("{prefix}{text}")).unwrap(), bits);
            }
            assert_eq!(
                run(&text).unwrap(),
                format!("chmod {numeric} 'path/to/file'")
            );
        }
    }

    #[test]
    fn all_4096_masks_toggle_each_bit_without_changing_the_others() {
        for bits in 0..=0o7777 {
            for input in [octal(bits), symbolic(bits), format!("d{}", symbolic(bits))] {
                for shift in 0..12 {
                    let mask = 1 << shift;
                    for enabled in [false, true] {
                        let output = toggle(&input, mask, enabled).unwrap();
                        let actual = parse(&output).unwrap();
                        let expected = if enabled { bits | mask } else { bits & !mask };
                        assert_eq!(actual, expected, "{input}, bit {shift}, {enabled}");
                        assert_eq!(actual & !mask, bits & !mask);
                        assert_eq!(output, octal(expected));
                        assert_eq!(toggle(&output, mask, enabled).unwrap(), output);
                    }
                }
            }
        }
    }

    #[test]
    fn source_special_bits_and_execute_cases() {
        for (input, bits, text) in [
            ("0000", 0, "---------"),
            ("001", 0o001, "--------x"),
            ("0755", 0o755, "rwxr-xr-x"),
            ("rwsr-xr-x", 0o4755, "rwsr-xr-x"),
            ("rwSr-xr-x", 0o4655, "rwSr-xr-x"),
            ("rwxr-sr-x", 0o2755, "rwxr-sr-x"),
            ("rwxr-Sr-x", 0o2745, "rwxr-Sr-x"),
            ("rwxr-xr-t", 0o1755, "rwxr-xr-t"),
            ("rwxr-xr-T", 0o1754, "rwxr-xr-T"),
            ("--S--S--T", 0o7000, "--S--S--T"),
            ("--s--s--t", 0o7111, "--s--s--t"),
            ("7777", 0o7777, "rwsrwsrwt"),
        ] {
            assert_eq!(parse(input).unwrap(), bits, "{input}");
            assert_eq!(symbolic(bits), text, "{input}");
        }
        assert_eq!(toggle("--S--S--T", 0o100, true).unwrap(), "7100");
        assert_eq!(symbolic(parse("7100").unwrap()), "--s--S--T");
        assert_eq!(toggle("7777", 0o4000, false).unwrap(), "3777");
    }

    #[test]
    fn source_requires_exact_octal_width_or_symbolic_shape() {
        for input in [
            "",
            "0",
            "00",
            "7",
            "77",
            "00000",
            "07777",
            "888",
            "+755",
            "-755",
            "0o755",
            "0x755",
            "７５５",
            "٧٥٥",
            "rwxr-xr-",
            "rwxr-xr-x-",
            "brwxr-xr-x",
            "crwxr-xr-x",
            "prwxr-xr-x",
            "srwxr-xr-x",
            "Drwxr-xr-x",
            "-755",
            "d755",
            "u+rwx",
            "755 file",
            "chmod 755 'path/to/file'",
            "755\n644",
            "rwx r-xr-x",
            "rw xr-xr-x",
        ] {
            assert_eq!(parse(input).unwrap_err(), INPUT_ERROR, "{input:?}");
            assert_eq!(run(input).unwrap_err(), INPUT_ERROR, "{input:?}");
        }
        assert_eq!(parse(&"r".repeat(500_001)).unwrap_err(), INPUT_ERROR);
    }

    #[test]
    fn source_errors_identify_permission_position_after_optional_prefix() {
        for index in 0..9 {
            let mut chars = ['-'; 9];
            chars[index] = if index % 3 == 2 { 'r' } else { 'x' };
            let input: String = chars.into_iter().collect();
            let expected = format!("Invalid permission character at position {}.", index + 1);
            for prefix in ["", "-", "d", "l"] {
                assert_eq!(parse(&format!("{prefix}{input}")).unwrap_err(), expected);
            }
        }
        for (input, position) in [
            ("rwt------", 3),
            ("rwT------", 3),
            ("-----t---", 6),
            ("-----T---", 6),
            ("--------s", 9),
            ("--------S", 9),
            ("s--------", 1),
            ("-S-------", 2),
            ("rwxR-xr-x", 4),
            ("rwxr-Wr-x", 6),
        ] {
            assert_eq!(
                parse(input).unwrap_err(),
                format!("Invalid permission character at position {position}.")
            );
        }
    }

    #[test]
    fn source_foundation_whitespace_trims_only_the_edges() {
        let whitespace = [
            '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}',
            '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
            '\u{2008}', '\u{2009}', '\u{200a}', '\u{200b}', '\u{2028}', '\u{2029}', '\u{202f}',
            '\u{205f}', '\u{3000}',
        ];
        for c in whitespace {
            assert_eq!(parse(&format!("{c}4755{c}")).unwrap(), 0o4755);
            assert_eq!(parse(&format!("{c}drwsr-xr-x{c}")).unwrap(), 0o4755);
            assert!(parse(&format!("7{c}55")).is_err());
            assert!(parse(&format!("rwxr{c}xr-x")).is_err());
            assert_eq!(parse(&c.to_string()).unwrap_err(), INPUT_ERROR);
        }
        for c in ['\0', '\u{1c}', '\u{180e}', '\u{2060}', '\u{feff}'] {
            assert!(parse(&format!("{c}755{c}")).is_err());
        }
    }

    #[test]
    fn non_ascii_permission_characters_are_rejected() {
        // These inputs cannot be valid permissions. The source uses grapheme
        // clusters for diagnostics; this dependency-free parser uses scalars.
        for input in [
            "rwxr-xr-é",
            "rwxr-xr-e\u{301}",
            "rwxr-xr-👩🏽‍💻",
            "ｒwxr-xr-x",
            "r\u{301}wxr-xr-x",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn source_command_placeholder_and_dispatch_remain_text_only() {
        for (input, expected) in [
            ("0000", "chmod 000 'path/to/file'"),
            ("0755", "chmod 755 'path/to/file'"),
            ("drwsr-xr-x", "chmod 4755 'path/to/file'"),
            ("lrwxr-xr-T", "chmod 1754 'path/to/file'"),
            ("-rwsrwsrwt", "chmod 7777 'path/to/file'"),
        ] {
            assert_eq!(run(input).unwrap(), expected);
            assert_eq!(super::super::execute("chmod", input, "").unwrap(), expected);
            // A secondary draft must never become a target path or command.
            assert_eq!(
                super::super::execute("chmod", input, "/do/not/read/or/change").unwrap(),
                expected
            );
        }
        assert!(
            super::super::execute("chmod", &" ".repeat(500_001), "")
                .unwrap_err()
                .contains("500,000")
        );
    }

    #[test]
    fn toggles_validate_one_bit_and_recover_invalid_input_from_zero() {
        for mask in 0u16..=u16::MAX {
            let valid = mask.is_power_of_two() && mask <= 0o4000;
            for enabled in [false, true] {
                let result = toggle("invalid", mask, enabled);
                assert_eq!(result.is_ok(), valid, "mask {mask}, {enabled}");
                if valid {
                    assert_eq!(result.unwrap(), octal(if enabled { mask } else { 0 }));
                }
            }
        }
        for input in ["", "not permissions", "77", "888", "rwxr-xr-é"] {
            assert_eq!(toggle(input, 0o400, true).unwrap(), "400");
            assert_eq!(toggle(input, 0o4000, true).unwrap(), "4000");
            assert_eq!(toggle(input, 0o1000, false).unwrap(), "000");
        }
    }
}
