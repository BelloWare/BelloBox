//! Integrator-level malformed-input checks; these do not establish Swift parity.
#![cfg(feature = "developer-tools")]
use bellobox_core::developer;

#[test]
fn every_tool_rejects_oversized_input_before_processing() {
    let oversized = "a".repeat(500_001);
    for tool in developer::catalog() {
        assert!(
            developer::execute(tool.id, &oversized, "").is_err(),
            "{}",
            tool.id
        );
        assert!(
            developer::execute(tool.id, "", &oversized).is_err(),
            "{}",
            tool.id
        );
    }
}

#[test]
fn malformed_unicode_and_syntax_never_panic() {
    let cases = [
        "",
        "\0",
        "😀é\u{301}中",
        "[{",
        "'\"\\",
        "-9999999999999999999999999999999999999",
        "NaN Infinity",
        "a\r\nb\rc\n",
        "../;$(echo nope)",
        "<!DOCTYPE x [<!ENTITY x SYSTEM 'file:///nonexistent-bello-test'>]><x>&x;</x>",
    ];
    for tool in developer::catalog() {
        for input in cases {
            for second in ["", "invalid option 😀"] {
                let outcome =
                    std::panic::catch_unwind(|| developer::execute(tool.id, input, second));
                assert!(outcome.is_ok(), "{} panicked for malformed input", tool.id);
                if let Ok(Ok(output)) = outcome {
                    assert!(output.len() <= 4_000_000, "{} output cap", tool.id);
                }
            }
        }
    }
}

#[test]
fn catalog_ids_are_unique_and_unknown_tools_fail() {
    let tools = developer::catalog();
    let ids: std::collections::HashSet<_> = tools.iter().map(|t| t.id).collect();
    assert_eq!(tools.len(), 51);
    assert_eq!(ids.len(), tools.len());
    assert!(developer::execute("not-a-tool", "", "").is_err());
}
