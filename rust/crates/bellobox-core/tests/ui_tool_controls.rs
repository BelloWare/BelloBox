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
