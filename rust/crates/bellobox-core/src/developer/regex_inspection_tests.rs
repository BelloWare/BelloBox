use super::*;
fn run(text: &str, pattern: &str, replacement: &str) -> Inspection {
    inspect(
        text,
        pattern,
        replacement,
        Options::default(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn unicode_ranges_keep_utf16_details_and_utf8_highlights_separate() {
    let r = run("é😀中 ab", "(😀)(中)?|(ab)(z)?", "$0/$1/$2/$3/$4");
    assert_eq!(r.matches[0].bytes, 2..9);
    assert_eq!(r.matches[0].utf16, 1..4);
    assert_eq!(
        r.details,
        "1. [1..<4] 😀中\n   $1: 😀\n   $2: 中\n   $3: (not matched)\n   $4: (not matched)\n2. [5..<7] ab\n   $1: (not matched)\n   $2: (not matched)\n   $3: ab\n   $4: (not matched)"
    );
    assert_eq!(r.extracted, "😀中\nab");
    assert_eq!(r.replaced, "é😀中/😀/中// ab///ab/");
}
#[test]
fn zero_length_unicode_end_and_empty_capture_are_retained() {
    let r = run("é😀", "()", "X");
    assert_eq!(
        r.matches
            .iter()
            .map(|m| m.bytes.clone())
            .collect::<Vec<_>>(),
        vec![0..0, 2..2, 6..6]
    );
    assert_eq!(
        r.matches
            .iter()
            .map(|m| m.utf16.clone())
            .collect::<Vec<_>>(),
        vec![0..0, 1..1, 3..3]
    );
    assert_eq!(r.extracted, "\n\n");
    assert_eq!(r.replaced, "XéX😀X");
    assert!(!r.details.contains("not matched"));
}
#[test]
fn source_options_no_match_and_invalid_pattern() {
    let text = "AA\naa";
    assert_eq!(run(text, "^aa$", "!").matches.len(), 1);
    let r = inspect(
        text,
        "^aa$",
        "!",
        Options {
            ignore_case: true,
            multiline: false,
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(r.matches.is_empty());
    assert_eq!(r.details, "No matches.");
    assert_eq!(r.replaced, text);
    assert_eq!(
        inspect(
            text,
            "^aa$",
            "!",
            Options {
                ignore_case: true,
                multiline: true
            },
            &AtomicBool::new(false)
        )
        .unwrap()
        .matches
        .len(),
        2
    );
    for pattern in ["", "[", "(?=a)", r"(a)\1"] {
        assert!(
            inspect(
                "a",
                pattern,
                "",
                Options::default(),
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
}
#[test]
fn numbered_templates_backslashes_and_unmatched_groups() {
    assert_eq!(
        run(
            "ab",
            "(a)(b)?(c)?",
            r"$0|$1|$2|$3|$9|$10|$$1|\$1|\\|\x|${1}|$"
        )
        .replaced,
        r"ab|a|b|||a0|$a|$1|\|x|${1}|$"
    );
    assert_eq!(run("a", "(a)", "end\\").replaced, "end");
    assert_eq!(
        run(
            "abcdefghij",
            "(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)",
            "$10-$11-$01-$100"
        )
        .replaced,
        "j--a-j0"
    );
}
#[test]
fn admissions_precede_work_and_match_capture_output_bounds_are_exact() {
    let flag = AtomicBool::new(false);
    assert!(
        inspect(
            &"x".repeat(INPUT_LIMIT + 1),
            ".",
            "",
            Options::default(),
            &flag
        )
        .unwrap_err()
        .contains("100000")
    );
    assert!(
        inspect(
            "x",
            &"a".repeat(PATTERN_LIMIT + 1),
            "",
            Options::default(),
            &flag
        )
        .is_err()
    );
    assert!(
        inspect(
            "x",
            "x",
            &"r".repeat(REPLACEMENT_LIMIT + 1),
            Options::default(),
            &flag
        )
        .is_err()
    );
    assert_eq!(
        run(&"x".repeat(MATCH_LIMIT), "x", "").matches.len(),
        MATCH_LIMIT
    );
    assert!(
        inspect(
            &"x".repeat(MATCH_LIMIT + 1),
            "x",
            "",
            Options::default(),
            &flag
        )
        .unwrap_err()
        .contains("1000")
    );
    assert!(
        inspect(
            "x",
            &"()".repeat(GROUP_LIMIT + 1),
            "",
            Options::default(),
            &flag
        )
        .unwrap_err()
        .contains("128")
    );
    // Template expansion is checked slice-by-slice before appending, never by
    // building an unchecked Captures.expand intermediate.
    assert!(
        inspect(
            &"a".repeat(INPUT_LIMIT),
            "(a+)",
            &"$1".repeat(50),
            Options::default(),
            &flag
        )
        .unwrap_err()
        .contains("4000000")
    );
    let mut remaining = 3;
    let mut output = String::new();
    append(&mut output, "éx", &mut remaining).unwrap();
    assert_eq!(remaining, 0);
    assert!(append(&mut output, "x", &mut remaining).is_err());
    assert_eq!(output, "éx");
}
#[test]
fn cancelled_and_expired_return_no_partial_result() {
    assert!(
        inspect("a", "a", "", Options::default(), &AtomicBool::new(true))
            .unwrap_err()
            .contains("cancelled")
    );
    assert!(
        inspect_with_budget(
            "a",
            "a",
            "",
            Options::default(),
            &AtomicBool::new(false),
            Duration::ZERO
        )
        .unwrap_err()
        .contains("time budget")
    );
}
#[test]
fn legacy_cli_json_schema_flags_and_utf8_offsets_remain_unchanged() {
    let output = super::super::execute(
        "regex",
        "é😀a\na",
        r#"{"pattern":"(a)","flags":"ims","replacement":"${1}x"}"#,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["count"], 2);
    assert_eq!(value["matches"][0]["startByte"], 6);
    assert_eq!(value["matches"][0]["endByte"], 7);
    assert_eq!(value["replaced"], "é😀ax\nax");
}

#[test]
fn source_zero_width_match_after_nonempty_is_not_suppressed() {
    let r = run("a", "a*", "<$0>");
    assert_eq!(
        r.matches
            .iter()
            .map(|m| m.bytes.clone())
            .collect::<Vec<_>>(),
        vec![0..1, 1..1]
    );
    assert_eq!(r.replaced, "<a><>");
}

#[test]
fn match_cap_and_zero_width_end_boundaries() {
    for count in [999, 1000, 1001] {
        for zero in [false, true] {
            let text = "a".repeat(count - usize::from(zero));
            let result = inspect(
                &text,
                if zero { "()" } else { "a" },
                "",
                Options::default(),
                &AtomicBool::new(false),
            );
            if count <= MATCH_LIMIT {
                assert_eq!(result.unwrap().matches.len(), count);
            } else {
                assert!(result.unwrap_err().contains("1000"));
            }
        }
    }
}
#[test]
fn exact_input_pattern_replacement_and_capture_limits_are_admitted() {
    let no_match = inspect(
        &"a".repeat(INPUT_LIMIT),
        &"z".repeat(PATTERN_LIMIT),
        &"r".repeat(REPLACEMENT_LIMIT),
        Options::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(no_match.matches.is_empty());
    assert_eq!(no_match.replaced.len(), INPUT_LIMIT);
    let groups = run("x", &"()".repeat(GROUP_LIMIT), "");
    assert_eq!(groups.matches[0].groups.len(), GROUP_LIMIT + 1);
}
