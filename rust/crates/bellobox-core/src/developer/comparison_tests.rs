use super::*;

fn run(left: &str, right: &str, mode: Mode, ignore_whitespace: bool) -> ComparisonResult {
    compare(
        left,
        right,
        Options {
            mode,
            ignore_whitespace,
        },
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn row(kind: RowKind, text: &str) -> Row {
    Row {
        kind,
        text: text.into(),
    }
}
#[test]
fn source_example_has_separate_counts_and_exact_signed_copy() {
    let value = run(
        "alpha\nbeta\ngamma",
        "alpha\nnew\ngamma\nend",
        Mode::Lines,
        false,
    );
    assert_eq!((value.added, value.removed), (2, 1));
    assert_eq!(value.copy_text, "  alpha\n− beta\n+ new\n  gamma\n+ end");
    assert_eq!(
        value.rows,
        vec![
            row(RowKind::Same, "alpha"),
            row(RowKind::Removed, "beta"),
            row(RowKind::Added, "new"),
            row(RowKind::Same, "gamma"),
            row(RowKind::Added, "end")
        ]
    );
}
#[test]
fn empty_sides_line_endings_and_final_blank_are_literal() {
    assert_eq!(run("", "", Mode::Lines, false), ComparisonResult::default());
    assert_eq!(run("", "\n", Mode::Lines, false).copy_text, "+ \n+ ");
    assert_eq!(run("\n", "", Mode::Lines, false).copy_text, "− \n− ");
    assert_eq!(
        run("a\r\nb\rc\r", "a\nb\nc\n", Mode::Lines, false).copy_text,
        "  a\n  b\n  c\n  "
    );
    assert_eq!(run("a\n", "a", Mode::Lines, false).copy_text, "  a\n− ");
    assert_eq!(run("a", "a\n", Mode::Lines, false).copy_text, "  a\n+ ");
}
#[test]
fn canonical_matches_keep_left_spelling_and_do_not_use_compatibility() {
    assert_eq!(
        run("e\u{301}", "é", Mode::Lines, false).copy_text,
        "  e\u{301}"
    );
    assert_eq!(run("é", "e\u{301}", Mode::Words, false).copy_text, "  é");
    assert_eq!(run("ﬁ", "fi", Mode::Lines, false).copy_text, "− ﬁ\n+ fi");
}
#[test]
fn source_whitespace_collapses_tokens_and_uses_first_grapheme_scalar() {
    assert_eq!(
        run("  a\t b   ", "a b", Mode::Lines, true).copy_text,
        "  a b"
    );
    assert_eq!(run("  a\t b   ", "a b", Mode::Lines, false).removed, 1);
    for whitespace in [
        '\t', '\n', '\u{b}', '\u{c}', '\r', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2028}',
        '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
    ] {
        assert_eq!(
            run(
                &format!("one{whitespace}two"),
                "one two",
                Mode::Words,
                false
            )
            .copy_text,
            "  one\n  two"
        );
    }
    for separator in [" \u{301}", "\u{a0}\u{301}", " \u{200d}"] {
        assert_eq!(
            run(&format!("one{separator}two"), "one two", Mode::Words, false).copy_text,
            "  one\n  two"
        );
        assert_eq!(
            run(&format!("one{separator}two"), "one two", Mode::Lines, true).copy_text,
            "  one two"
        );
    }
    assert_eq!(
        run("one\u{200b}two", "one two", Mode::Words, false).removed,
        1
    );
    assert_eq!(
        run("one two", "one three", Mode::Words, true).copy_text,
        "  one\n− two\n+ three"
    );
}
#[test]
fn repeated_token_ties_follow_swift_6_3_3_suffix_shrinking() {
    assert_eq!(
        run("a\nb", "b\na", Mode::Lines, false).copy_text,
        "− a\n  b\n+ a"
    );
    assert_eq!(
        run("a\na\nb", "a\nb\na", Mode::Lines, false).copy_text,
        "  a\n− a\n  b\n+ a"
    );
    assert_eq!(
        run("a\nb\na", "b\na\na", Mode::Lines, false).copy_text,
        "− a\n  b\n+ a\n  a"
    );
}
#[test]
fn json_projection_is_sorted_lossless_quoted_and_retains_empty_containers() {
    let left = r#"{"a.b":1,"a":{"b":1.0},"z":[[],{},90071992547409931234567890,1E+02]}"#;
    let right = r#"{"z":[[],{},90071992547409931234567890,1E+02],"a":{"b":1.0},"a.b":1}"#;
    let value = run(left, right, Mode::Json, false);
    assert_eq!((value.added, value.removed), (0, 0));
    assert_eq!(
        value.copy_text,
        "  $[\"a\"][\"b\"] = 1.0\n  $[\"a.b\"] = 1\n  $[\"z\"][0] = []\n  $[\"z\"][1] = {}\n  $[\"z\"][2] = 90071992547409931234567890\n  $[\"z\"][3] = 1E+02"
    );
    assert_eq!(
        run("1", "1.0", Mode::Json, true).copy_text,
        "− $ = 1\n+ $ = 1.0"
    );
    assert_eq!(
        run("{}", "[]", Mode::Json, false).copy_text,
        "− $ = {}\n+ $ = []"
    );
    assert_eq!(
        run(
            r#"{"\"\\/\n": "\u0000"}"#,
            r#"{"\"\\/\n": "\u0000"}"#,
            Mode::Json,
            false
        )
        .copy_text,
        "  $[\"\\\"\\\\/\\n\"] = \"\\u0000\""
    );
    assert_eq!(
        run(
            "{\"e\u{301}\":\"e\u{301}\"}",
            "{\"é\":\"é\"}",
            Mode::Json,
            false
        )
        .copy_text,
        "  $[\"e\u{301}\"] = \"e\u{301}\""
    );
}
#[test]
fn malformed_and_canonical_duplicate_json_never_produce_partial_rows() {
    for input in [
        "",
        "{",
        "[1,]",
        "01",
        "{\"é\":1,\"e\u{301}\":2}",
        "{\"a\":1,\"\\u0061\":2}",
    ] {
        assert!(
            compare(
                input,
                "null",
                Options {
                    mode: Mode::Json,
                    ignore_whitespace: false
                },
                &AtomicBool::new(false)
            )
            .is_err(),
            "{input}"
        );
    }
    let deep = format!("{}0{}", "[".repeat(64), "]".repeat(64));
    assert!(
        compare(
            &deep,
            "null",
            Options {
                mode: Mode::Json,
                ignore_whitespace: false
            },
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
#[test]
fn precise_input_token_and_copy_output_limits_never_truncate() {
    let cancel = AtomicBool::new(false);
    assert!(compare(&"a".repeat(INPUT_LIMIT), "", Options::default(), &cancel).is_ok());
    assert!(
        compare(&"a".repeat(INPUT_LIMIT), "b", Options::default(), &cancel)
            .unwrap_err()
            .contains("500,000")
    );
    let lines = vec!["a"; TOKEN_LIMIT / 2].join("\n");
    assert_eq!(
        compare(&lines, &lines, Options::default(), &cancel)
            .unwrap()
            .rows
            .len(),
        TOKEN_LIMIT / 2
    );
    assert!(
        compare(&lines, &(lines.clone() + "\n"), Options::default(), &cancel)
            .unwrap_err()
            .contains("8,000")
    );
    let mut value = ComparisonResult::default();
    value
        .append(&"a".repeat(OUTPUT_LIMIT - 2), RowKind::Same)
        .unwrap();
    assert_eq!(value.copy_text.len(), OUTPUT_LIMIT);
    assert!(value.append("", RowKind::Same).is_err());
    assert_eq!(value.copy_text.len(), OUTPUT_LIMIT);
}
#[test]
fn json_flatten_bounds_long_paths_before_repeated_expansion() {
    let cancelled = AtomicBool::new(false);
    let fields =
        super::super::json_formatter::comparison_fields(r#"{"a":[1,{},[]]}"#, 3, 100, &cancelled)
            .unwrap();
    assert_eq!(
        fields,
        ["$[\"a\"][0] = 1", "$[\"a\"][1] = {}", "$[\"a\"][2] = []"]
    );
    let bytes: usize = fields.iter().map(String::len).sum();
    assert!(
        super::super::json_formatter::comparison_fields(r#"{"a":[1,{},[]]}"#, 3, bytes, &cancelled)
            .is_ok()
    );
    assert!(
        super::super::json_formatter::comparison_fields(
            r#"{"a":[1,{},[]]}"#,
            3,
            bytes - 1,
            &cancelled
        )
        .is_err()
    );
    assert!(
        super::super::json_formatter::comparison_fields(r#"{"a":[1,{},[]]}"#, 2, 100, &cancelled)
            .is_err()
    );
    let amplified = format!(
        "{{\"{}\":[{}]}}",
        "x".repeat(2000),
        vec!["0"; 2000].join(",")
    );
    assert!(
        compare(
            &amplified,
            "null",
            Options {
                mode: Mode::Json,
                ignore_whitespace: false
            },
            &cancelled
        )
        .is_err()
    );
}
#[test]
fn cancellation_is_enforced_at_every_public_mode_and_diff_entry() {
    let cancelled = AtomicBool::new(true);
    for mode in [Mode::Lines, Mode::Words, Mode::Json] {
        assert_eq!(
            compare(
                "a",
                "b",
                Options {
                    mode,
                    ignore_whitespace: false
                },
                &cancelled
            )
            .unwrap_err(),
            "Comparison cancelled."
        );
    }
    assert!(changes(&[0; 500], &[0; 500], &cancelled).is_err());
    assert!(super::super::json_formatter::comparison_fields("{}", 5, 100, &cancelled).is_err());
}

// Independent full-trace reference used only for short synthetic differential
// cases to verify edit minimality. Swift 6.3.3 chooses different ties, so exact
// row agreement is separately checked by source-shaped witnesses/native oracle.
fn reference(a: &[u16], b: &[u16]) -> (Vec<bool>, Vec<bool>) {
    use std::collections::HashMap;
    let mut v = HashMap::from([(1isize, 0usize)]);
    let mut trace = Vec::new();
    'outer: for d in 0..=a.len() + b.len() {
        trace.push(v.clone());
        let d = d as isize;
        for k in (-d..=d).step_by(2) {
            let mut x = if k == -d || (k != d && v[&(k - 1)] < v[&(k + 1)]) {
                v[&(k + 1)]
            } else {
                v[&(k - 1)] + 1
            };
            let mut y = (x as isize - k) as usize;
            while x < a.len() && y < b.len() && a[x] == b[y] {
                x += 1;
                y += 1;
            }
            v.insert(k, x);
            if x >= a.len() && y >= b.len() {
                break 'outer;
            }
        }
    }
    let (mut x, mut y) = (a.len(), b.len());
    let mut removed = vec![false; a.len()];
    let mut added = vec![false; b.len()];
    for d in (1..trace.len()).rev() {
        let k = x as isize - y as isize;
        let v = &trace[d];
        let depth = d as isize;
        let prior = if k == -depth || (k != depth && v[&(k - 1)] < v[&(k + 1)]) {
            k + 1
        } else {
            k - 1
        };
        let px = v[&prior];
        let py = (px as isize - prior) as usize;
        while x > px && y > py {
            x -= 1;
            y -= 1;
        }
        if y != py {
            added[py] = true;
        } else {
            removed[px] = true;
        }
        x = px;
        y = py;
    }
    (removed, added)
}
fn verify_minimal_reconstruction(a: &[u16], b: &[u16], cancelled: &AtomicBool) {
    let (removed, added) = changes(a, b, cancelled).unwrap();
    let (ref_removed, ref_added) = reference(a, b);
    let count = |items: &[bool]| items.iter().filter(|&&item| item).count();
    assert_eq!(
        (count(&removed), count(&added)),
        (count(&ref_removed), count(&ref_added)),
        "{a:?} / {b:?}"
    );
    let same_a: Vec<_> = a
        .iter()
        .zip(&removed)
        .filter_map(|(id, &changed)| (!changed).then_some(id))
        .collect();
    let same_b: Vec<_> = b
        .iter()
        .zip(&added)
        .filter_map(|(id, &changed)| (!changed).then_some(id))
        .collect();
    assert_eq!(same_a, same_b, "{a:?} / {b:?}");
}
#[test]
fn exhaustive_small_alphabet_and_large_regions_are_minimal_and_reconstruct() {
    let cancelled = AtomicBool::new(false);
    let mut sequences = vec![vec![]];
    for len in 1..=5 {
        for bits in 0..(1usize << len) {
            sequences.push((0..len).map(|index| ((bits >> index) & 1) as u16).collect());
        }
    }
    for a in &sequences {
        for b in &sequences {
            verify_minimal_reconstruction(a, b, &cancelled);
        }
    }
    for len in [31, 32, 33, 63, 64, 65, 95, 96, 97, 127, 128, 129] {
        let a = vec![0; len];
        let b = vec![1; len];
        verify_minimal_reconstruction(&a, &b, &cancelled);
        let a: Vec<_> = (0..len).map(|i| (i % 7) as u16).collect();
        let b: Vec<_> = (0..len).map(|i| ((i * 3 + 2) % 7) as u16).collect();
        verify_minimal_reconstruction(&a, &b, &cancelled);
    }
}
#[test]
fn maximum_disjoint_tokens_are_admitted_with_bounded_work_and_memory() {
    let a = vec![0; TOKEN_LIMIT / 2];
    let b = vec![1; TOKEN_LIMIT / 2];
    let (removed, added) = changes(&a, &b, &AtomicBool::new(false)).unwrap();
    assert!(removed.iter().all(|value| *value));
    assert!(added.iter().all(|value| *value));
}
