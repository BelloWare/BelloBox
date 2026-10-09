use super::*;

#[test]
fn numeric_lexemes_are_byte_exact() {
    let input =
        "[-0,-0.0,1E2,1e2,1E+02,1E-02,1.2300,900719925474099312345678901234567890,1e999999]";
    assert_eq!(execute(input, "minify").unwrap(), input);
    // Characterization contrast (not a source-mutant test): Value's arbitrary_precision is insufficient.
    let old: serde_json::Value = serde_json::from_str(input).unwrap();
    assert_ne!(serde_json::to_string(&old).unwrap(), input);
}
#[test]
fn nested_objects_sort_and_array_order_survives() {
    let input = r#"{"z":{"b":1,"a":2},"a":[{"y":3,"x":4},{}]}"#;
    let expected = r#"{"a":[{"x":4,"y":3},{}],"z":{"a":2,"b":1}}"#;
    assert_eq!(execute(input, "minify").unwrap(), expected);
    assert_eq!(execute("[{},[]]", "pretty").unwrap(), "[\n  {},\n  []\n]");
    assert_eq!(execute("{}", "pretty").unwrap(), "{}");
    assert_eq!(execute("[]", "").unwrap(), "[]");
    assert_eq!(execute("{\"x\":1}", "pretty").unwrap(), "{\n  \"x\": 1\n}");
}
#[test]
fn normalized_order_retains_original_key_spelling() {
    let input = "{\"e\u{301}\":1,\"f\":2}";
    assert_eq!(
        execute(input, "minify").unwrap(),
        "{\"f\":2,\"e\u{301}\":1}"
    );
    // Characterization contrast (not a source-mutant test): raw-byte recursive sorting still gets this wrong.
    let mut old: serde_json::Value = serde_json::from_str(input).unwrap();
    old.sort_all_objects();
    assert_ne!(old.to_string(), execute(input, "minify").unwrap());
}
#[test]
fn duplicates_use_nfc_but_not_compatibility_normalization() {
    for input in [
        r#"{"a":1,"a":2}"#,
        r#"{"a":1,"\u0061":2}"#,
        r#"{"\u00e9":1,"e\u0301":2}"#,
        r#"{"\u212b":1,"\u00c5":2}"#,
        r#"{"\uac00":1,"\u1100\u1161":2}"#,
    ] {
        for mode in ["pretty", "minify", "validate"] {
            assert!(
                execute(input, mode).unwrap_err().contains("Duplicate"),
                "{input}"
            );
        }
    }
    // Compatibility-equivalent and case-equivalent names remain distinct.
    assert!(execute(r#"{"\ufb01":1,"fi":2,"A":3,"a":4}"#, "validate").is_ok());
}
#[test]
fn ascii_sort_is_case_sensitive_and_not_natural_numeric_order() {
    assert_eq!(
        execute(r#"{"a10":1,"a2":2,"A":3,"a":4}"#, "minify").unwrap(),
        r#"{"A":3,"a":4,"a10":1,"a2":2}"#
    );
}
#[test]
fn supplementary_keys_follow_scalar_not_utf16_order() {
    assert_eq!(
        execute("{\"\u{10000}\":1,\"\u{e000}\":2}", "minify").unwrap(),
        "{\"\u{e000}\":2,\"\u{10000}\":1}"
    );
}
#[test]
fn strings_are_decoded_then_encoded_without_normalizing_values() {
    assert_eq!(
        execute(
            r#"["\/","\u0061","e\u0301","\ud83d\ude80","\u0000\u001f"]"#,
            "minify"
        )
        .unwrap(),
        "[\"/\",\"a\",\"e\u{301}\",\"🚀\",\"\\u0000\\u001f\"]"
    );
    assert_eq!(
        execute("\"\u{2028}\u{2029}\u{7f}\u{ffff}\"", "minify").unwrap(),
        "\"\u{2028}\u{2029}\u{7f}\u{ffff}\""
    );
    // These are Rust/source expectations. Foundation byte equivalence is pending.
}
#[test]
fn malformed_inputs_are_rejected_without_panics() {
    for input in [
        "",
        " ",
        "01",
        "-01",
        "1.",
        "1e",
        "1E+",
        "-",
        "+1",
        "NaN",
        "Infinity",
        "[1,]",
        "{\"a\":1,}",
        "true false",
        "nullx",
        "trué",
        "[é]",
        "{é:1}",
        "\"raw\nnewline\"",
        r#""\q""#,
        r#""\ud800""#,
        r#""\udc00""#,
        "\"unfinished\\",
    ] {
        assert!(execute(input, "minify").is_err(), "{input}");
    }
}
#[test]
fn root_scalars_and_json_whitespace_are_accepted() {
    for input in ["-0", "true", "false", "null", "\"hello\""] {
        assert_eq!(
            execute(&format!(" \t\r\n{input} \n"), "minify").unwrap(),
            input
        );
    }
    assert!(execute("\u{a0}null", "validate").is_err());
}
#[test]
fn depth_limit_counts_root_and_nested_values() {
    let valid = format!("{}0{}", "[".repeat(63), "]".repeat(63));
    assert!(execute(&valid, "validate").is_ok());
    let invalid = format!("{}0{}", "[".repeat(64), "]".repeat(64));
    assert!(execute(&invalid, "validate").is_err());
}
#[test]
fn exact_node_budget_includes_root() {
    let valid = format!("[{}]", vec!["0"; NODE_LIMIT - 1].join(","));
    assert!(execute(&valid, "validate").is_ok());
    let invalid = format!("[{}]", vec!["0"; NODE_LIMIT].join(","));
    assert!(execute(&invalid, "validate").is_err());
}
#[test]
fn input_budget_includes_untrimmed_option_bytes() {
    let valid = format!("\"{}\"", "x".repeat(INPUT_LIMIT - 2));
    assert!(execute(&valid, "").is_ok());
    assert!(execute(&valid, " ").is_err());
    let invalid = format!("\"{}\"", "é".repeat(INPUT_LIMIT / 2));
    assert!(execute(&invalid, "").is_err());
}
#[test]
fn output_writer_enforces_cap_before_appending() {
    let mut writer = BoundedOutput {
        bytes: Vec::new(),
        limit: 3,
    };
    writer.text("abc").unwrap();
    assert!(writer.text("d").is_err());
    assert_eq!(&writer.bytes, b"abc");
    let node = Parser::parse(r#"{"x":[1,2]}"#).unwrap();
    let full = render(&node, true, OUTPUT_LIMIT).unwrap();
    assert_eq!(render(&node, true, full.len()).unwrap(), full);
    assert!(render(&node, true, full.len() - 1).is_err());
    let string = Parser::parse(r#""\u0000""#).unwrap();
    assert!(render(&string, false, 7).is_err());
    assert_eq!(render(&string, false, 8).unwrap(), r#""\u0000""#);
}
#[test]
fn validate_parses_and_rejects_unknown_options() {
    assert_eq!(execute("{}", "validate").unwrap(), "Valid JSON.");
    assert!(execute("{", "validate").is_err());
    assert!(execute("{}", "unknown").is_err());
    assert_eq!(execute("[1]", "  minify  ").unwrap(), "[1]");
}
