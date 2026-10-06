//! Headless control-state/engine integration, not desktop interaction evidence.
//! Uses the production GPUI window's option state without linking a renderer.
#![cfg(feature = "developer-tools")]
#[path = "../../bellobox-app/src/tool_controls.rs"]
#[allow(dead_code)]
mod controls;
use bellobox_core::developer::execute;
use controls::ToolControls;

#[test]
fn source_converter_defaults_follow_selection_and_chain_both_directions() {
    for (tool, input, initial, reversed) in [
        (
            "jsonLines",
            "  [1,{\"id\":2}]",
            "Array → Lines",
            "Lines → Array",
        ),
        (
            "envFile",
            " {\"NAME\":\"${USER}\"}",
            "JSON → Env",
            "Env → JSON",
        ),
    ] {
        let mut state = ToolControls::new(tool, input);
        assert_eq!(state.value("mode"), initial);
        let output = execute(
            tool,
            input,
            &state.second(tool, "ignored stale option draft"),
        )
        .unwrap();
        state.reverse_after_chaining(tool);
        assert_eq!(state.value("mode"), reversed);
        let roundtrip = execute(tool, &output, &state.second(tool, "")).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&roundtrip).unwrap(),
            serde_json::from_str::<serde_json::Value>(input).unwrap()
        );
        state.reverse_after_chaining(tool);
        assert_eq!(state.value("mode"), initial);
    }
}

#[test]
fn source_menus_validate_choices_without_mutating_other_windows() {
    for (tool, expected) in [
        ("jsonLines", "Lines → Array"),
        ("envFile", "Env → JSON"),
        ("cookies", "Set-Cookie"),
    ] {
        let mut state = ToolControls::new(tool, "");
        let other = state.clone();
        assert_eq!(state.value("mode"), expected);
        assert!(!state.select(tool, "mode", "unknown"));
        assert!(!state.select(tool, "unknown", expected));
        assert_eq!(state.value("mode"), expected);
        let spec = controls::choices(tool).remove(0);
        assert!(state.select(tool, "mode", spec.choices[1]));
        assert_eq!(other.value("mode"), expected);
    }
}

#[test]
fn cookie_header_detection_and_manual_menu_route_to_engine() {
    let input = "Cookie: theme=dark; language=en";
    let mut state = ToolControls::new("cookies", input);
    assert_eq!(state.value("mode"), "Cookie");
    assert_eq!(
        ToolControls::new("cookies", "  COOKIE: theme=dark").value("mode"),
        "Cookie"
    );
    let cookie = execute("cookies", input, &state.second("cookies", "")).unwrap();
    assert!(cookie.contains("theme") && cookie.contains("language"));
    state.reverse_after_chaining("cookies");
    assert_eq!(
        state.value("mode"),
        "Cookie",
        "inspection is not a converter"
    );
    assert!(state.select("cookies", "mode", "Set-Cookie"));
    let set_cookie = execute(
        "cookies",
        "theme=dark; Path=/; Secure",
        &state.second("cookies", ""),
    )
    .unwrap();
    assert!(set_cookie.contains("Secure"));
}

#[test]
fn source_labels_follow_mode_changes_and_chaining() {
    for (tool, first, second) in [
        (
            "jsonLines",
            "One complete JSON value per line",
            "JSON array · one output record per item",
        ),
        (
            "envFile",
            "One KEY=value per line · values stay literal",
            "JSON object · string values only",
        ),
        (
            "plist",
            "XML property list · standard plist declaration is handled locally",
            "Typed JSON · each node has type and value",
        ),
    ] {
        let mut state = ToolControls::new(tool, "");
        assert_eq!(state.input_label(tool), first);
        state.reverse_after_chaining(tool);
        assert_eq!(state.input_label(tool), second);
        state.reverse_after_chaining(tool);
        assert_eq!(state.input_label(tool), first);
    }
    let mut state = ToolControls::new("cookies", "");
    assert_eq!(
        state.input_label("cookies"),
        "One Set-Cookie field per line"
    );
    state.select("cookies", "mode", "Cookie");
    assert_eq!(
        state.input_label("cookies"),
        "Cookie request header · name=value; name=value"
    );
}

#[test]
fn list_set_source_controls_select_each_operation_without_overwriting_second_document() {
    let mut state = ToolControls::new("listSet", "Swift\nRust\nPython");
    let independent = state.clone();
    let second = "Rust\nGo\nSwift";
    assert_eq!(state.value("mode"), "Union");
    assert_eq!(state.value("matching"), "Exact");
    for (mode, expected) in [
        ("Union", "Swift\nRust\nPython\nGo"),
        ("Intersection", "Swift\nRust"),
        ("A − B", "Python"),
        ("B − A", "Go"),
        ("Symmetric difference", "Python\nGo"),
    ] {
        assert!(state.select("listSet", "mode", mode));
        assert_eq!(state.second("listSet", second), second);
        assert_eq!(
            bellobox_core::developer::list_set::run(
                "Swift\nRust\nPython",
                &state.second("listSet", second),
                state.value("mode"),
                state.value("matching")
            )
            .unwrap(),
            expected
        );
    }
    assert!(!state.select("listSet", "mode", "invented"));
    assert!(!state.select("listSet", "matching", "Locale sort"));
    assert!(state.select("listSet", "matching", "Trim & ignore case"));
    assert_eq!(independent.value("mode"), "Union");
    assert_eq!(independent.value("matching"), "Exact");
    let reopened = ToolControls::new("listSet", "anything");
    assert_eq!(reopened.value("mode"), "Union");
    assert_eq!(reopened.value("matching"), "Exact");
    assert_eq!(execute("listSet", "B\nA\nB", "C\nA").unwrap(), "B\nA\nC");
}

#[test]
fn paired_list_editor_heights_follow_source_bounded_utf16_measurement() {
    assert_eq!(controls::list_set_editor_height("A", "B"), 80.);
    assert_eq!(
        controls::list_set_editor_height("A", &"B\n".repeat(20)),
        156.
    );
    assert_eq!(controls::list_set_editor_height(&"😀".repeat(72), "B"), 96.);
    assert_eq!(controls::list_set_editor_height("A\r\nB\rC", "B"), 80.);
    assert_eq!(
        controls::list_set_editor_height("", &"x".repeat(500_000)),
        156.
    );
}
