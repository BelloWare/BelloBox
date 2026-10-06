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

#[test]
fn string_literal_source_format_menu_routes_without_an_option_document() {
    let specs = controls::choices("stringEscape");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].label, "Format");
    assert_eq!(
        specs[0].choices,
        ["JSON quote", "JSON unquote", "Swift literal", "Shell quote"]
    );
    let mut state = ToolControls::new("stringEscape", "\"selected\"");
    let other = state.clone();
    assert_eq!(state.value("mode"), "JSON quote");
    for (mode, input, expected) in [
        ("JSON quote", "a/b", "\"a/b\""),
        ("JSON unquote", "\"a/b\"", "a/b"),
        ("Swift literal", "\0", "\"\\u{0}\""),
        ("Shell quote", "O'Reilly", "'O'\"'\"'Reilly'"),
    ] {
        assert!(state.select("stringEscape", "mode", mode));
        assert_eq!(
            execute("stringEscape", input, &state.second("stringEscape", "html")).unwrap(),
            expected
        );
    }
    assert!(!state.select("stringEscape", "mode", "html"));
    assert_eq!(state.value("mode"), "Shell quote");
    assert_eq!(other.value("mode"), "JSON quote");
    assert_eq!(
        ToolControls::new("stringEscape", "new document").value("mode"),
        "JSON quote"
    );
}

#[test]
fn string_literal_source_idle_copy_and_bounded_editor_height() {
    for value in ["", " \t\r\n", "\u{200b}\u{85}\u{2028}"] {
        assert!(controls::source_input_is_idle(value));
    }
    for value in [" a ", "\0", "\"\"", "\u{feff}"] {
        assert!(!controls::source_input_is_idle(value));
    }
    for (busy, error, output, enabled) in [
        (false, false, "ok", true),
        (true, false, "old", false),
        (false, true, "old", false),
        (false, false, "", false),
    ] {
        assert_eq!(controls::source_copy_enabled(busy, error, output), enabled);
    }
    assert_eq!(controls::string_literal_editor_height("short"), 80.);
    assert_eq!(
        controls::string_literal_editor_height(&"😀".repeat(152)),
        96.
    );
    assert_eq!(controls::string_literal_editor_height("a\r\nb\rc"), 80.);
    assert_eq!(
        controls::string_literal_editor_height(&"long\n".repeat(100_000)),
        156.
    );
}

#[test]
fn string_literal_empty_success_differs_from_idle() {
    assert_eq!(
        controls::string_literal_empty_message("\"\"", "JSON unquote"),
        "JSON unquote · literal text only"
    );
    assert_eq!(
        controls::string_literal_empty_message(" ", "JSON unquote"),
        "Paste text or use an example to begin."
    );
    assert!(!controls::source_copy_enabled(false, false, ""));
}

#[test]
fn subnet_source_label_example_no_options_and_error_recovery() {
    let mut state = ToolControls::new("subnet", "192.168.1.42/24");
    assert!(controls::choices("subnet").is_empty());
    assert!(!state.select("subnet", "mode", "IPv6"));
    assert_eq!(
        state.input_label("subnet"),
        "IPv4 / prefix · e.g. 192.168.1.42/24"
    );
    let example = bellobox_core::developer::catalog()
        .into_iter()
        .find(|tool| tool.id == "subnet")
        .unwrap()
        .example;
    assert_eq!(example, "192.168.1.42/24");
    assert!(execute("subnet", "192.168.1.42/024", &state.second("subnet", "")).is_err());
    assert!(
        execute("subnet", example, &state.second("subnet", ""))
            .unwrap()
            .contains("Broadcast    192.168.1.255")
    );
    for input in ["", " \r\n\t", "\u{200b}\u{2029}"] {
        assert!(controls::source_input_is_idle(input));
    }
    assert!(!controls::source_input_is_idle("0.0.0.0/0"));
    assert!(!controls::source_copy_enabled(true, false, "old result"));
    assert!(!controls::source_copy_enabled(false, true, "old result"));
}

#[test]
fn permission_source_grid_has_twelve_distinct_bits_and_no_options() {
    let state = ToolControls::new("chmod", "755");
    assert!(controls::choices("chmod").is_empty());
    assert_eq!(
        state.input_label("chmod"),
        "Octal or rwx permissions · e.g. 755 or rwxr-xr-x"
    );
    let specs = controls::PERMISSION_TOGGLES;
    assert_eq!(
        specs.map(|s| s.mask),
        [
            0o400, 0o200, 0o100, 0o40, 0o20, 0o10, 0o4, 0o2, 0o1, 0o4000, 0o2000, 0o1000
        ]
    );
    assert_eq!(
        specs.map(|s| s.label),
        [
            "Read", "Write", "Exec", "Read", "Write", "Exec", "Read", "Write", "Exec", "Set UID",
            "Set GID", "Sticky"
        ]
    );
    assert!(specs.iter().all(|s| !s.help.is_empty()));
    let mut input = "invalid".to_owned();
    for spec in specs {
        input = bellobox_core::developer::permissions::toggle(&input, spec.mask, true).unwrap();
    }
    assert_eq!(input, "7777");
    assert_eq!(
        execute("chmod", &input, &state.second("chmod", "")).unwrap(),
        "chmod 7777 'path/to/file'"
    );
}

#[test]
fn permission_tab_reaches_every_checkbox_and_returns_without_editing_input() {
    let mut current = 0;
    for expected in (1..13).chain(std::iter::once(0)) {
        current = controls::permission_tab_target(Some(current), false);
        assert_eq!(current, expected);
    }
    for expected in (0..13).rev() {
        current = controls::permission_tab_target(Some(current), true);
        assert_eq!(current, expected);
    }
    assert_eq!(controls::permission_tab_target(None, false), 0);
    assert_eq!(controls::permission_tab_target(None, true), 12);
    assert_eq!(controls::permission_tab_target(Some(usize::MAX), true), 12);
}

#[test]
fn number_base_source_menus_defaults_and_gui_cli_boundary() {
    let specs = controls::choices("numberBase");
    assert_eq!(specs.len(), 2);
    assert_eq!(
        (specs[0].label, specs[0].choices),
        ("Input base", &["10", "2", "8", "16"][..])
    );
    assert_eq!(
        (specs[1].label, specs[1].choices),
        ("Output", &["All bases", "10", "16", "2", "8"][..])
    );
    let mut state = ToolControls::new("numberBase", "0xFF");
    let independent = state.clone();
    assert_eq!(state.number_base_options(), (10, None));
    assert_eq!(
        state.number_base_status(),
        "Exact integer · no floating-point rounding"
    );
    assert_eq!(
        state.input_label("numberBase"),
        "Integer · optional sign and matching 0x / 0b / 0o prefix"
    );
    assert!(bellobox_core::developer::number_base::run("0xFF", 10, None).is_err());
    assert!(
        execute("numberBase", "0xFF", "")
            .unwrap()
            .contains("Decimal: 255")
    );
    assert!(state.select("numberBase", "base", "16"));
    for (output, expected) in [
        ("10", "2832"),
        ("16", "b10"),
        ("2", "101100010000"),
        ("8", "5420"),
    ] {
        assert!(state.select("numberBase", "outputBase", output));
        let (base, target) = state.number_base_options();
        assert_eq!(
            bellobox_core::developer::number_base::run("0b10", base, target).unwrap(),
            expected
        );
        assert_eq!(
            state.number_base_status(),
            format!("Exact integer · base 16 → {output}")
        );
    }
    assert!(execute("numberBase", "0b10", "16").is_err());
    assert!(!state.select("numberBase", "base", "Auto"));
    assert!(!state.select("numberBase", "outputBase", "36"));
    assert_eq!(independent.number_base_options(), (10, None));
    assert_eq!(
        ToolControls::new("numberBase", "9007199254740993").number_base_options(),
        (10, None)
    );
    // Clear does not reconstruct controls, so selecting a format survives an idle draft.
    assert!(controls::source_input_is_idle(" \u{200b}"));
    assert_eq!(state.number_base_options(), (16, Some(8)));
}

#[test]
fn number_base_launcher_preview_matches_gui_defaults_not_cli_autodetection() {
    assert_eq!(
        controls::number_base_preview("255").unwrap(),
        "Decimal  255\nHex      FF\nOctal    377\nBinary   11111111"
    );
    for input in ["0xFF", "0b10", "0o77"] {
        let (base, output) = ToolControls::new("numberBase", input).number_base_options();
        assert_eq!(
            controls::number_base_preview(input),
            bellobox_core::developer::number_base::run(input, base, output)
        );
        assert!(controls::number_base_preview(input).is_err());
        assert!(execute("numberBase", input, "").is_ok());
    }
}
