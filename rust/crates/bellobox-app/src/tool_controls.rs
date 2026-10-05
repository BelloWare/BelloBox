//! State and labels mirrored from AdditionalUtility/ExtendedUtilityDefinitions.
//! Kept separate from the text document: choosing a mode never overwrites input.
#[derive(Clone, Copy)]
pub struct ChoiceSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub choices: &'static [&'static str],
}
#[derive(Clone, Debug, Default)]
pub struct ToolControls {
    values: std::collections::BTreeMap<&'static str, String>,
}
impl ToolControls {
    pub fn new(tool: &str, input: &str) -> Self {
        let mut state = Self::default();
        for spec in choices(tool) {
            state.values.insert(spec.id, spec.choices[0].into());
        }
        let trimmed = input.trim_start();
        let detected = match tool {
            "plist" if trimmed.starts_with('{') => Some("JSON → Plist"),
            "jsonLines" if trimmed.starts_with('[') => Some("Array → Lines"),
            "envFile" if trimmed.starts_with('{') => Some("JSON → Env"),
            "cookies" if trimmed.to_ascii_lowercase().starts_with("cookie:") => Some("Cookie"),
            _ => None,
        };
        if let Some(mode) = detected {
            state.values.insert("mode", mode.into());
        }
        state
    }
    pub fn value(&self, id: &str) -> &str {
        self.values.get(id).map(String::as_str).unwrap_or("")
    }
    pub fn select(&mut self, tool: &str, id: &str, value: &str) -> bool {
        let Some(spec) = choices(tool)
            .into_iter()
            .find(|s| s.id == id && s.choices.contains(&value))
        else {
            return false;
        };
        self.values.insert(spec.id, value.into());
        true
    }
    pub fn input_label(&self, tool: &str) -> &'static str {
        match (tool, self.value("mode")) {
            ("jsonLines", "Array → Lines") => "JSON array · one output record per item",
            ("envFile", "JSON → Env") => "JSON object · string values only",
            ("cookies", "Cookie") => "Cookie request header · name=value; name=value",
            ("cookies", _) => "One Set-Cookie field per line",
            ("plist", "JSON → Plist") => "Typed JSON · each node has type and value",
            ("plist", _) => "XML property list · standard plist declaration is handled locally",
            _ => input_label(tool),
        }
    }
    pub fn second(&self, tool: &str, text: &str) -> String {
        match tool {
            "plist" => self.value("mode").into(),
            "jsonLines" => match self.value("mode") {
                "Array → Lines" => "array-to-lines",
                _ => "lines-to-array",
            }
            .into(),
            "envFile" => match self.value("mode") {
                "JSON → Env" => "json-to-env",
                _ => "env-to-json",
            }
            .into(),
            "cookies" => match self.value("mode") {
                "Cookie" => "cookie",
                _ => "set-cookie",
            }
            .into(),
            "sqlFormat" => serde_json::json!({
                "keywords": self.value("keywords"), "dialect": self.value("dialect")
            })
            .to_string(),
            _ => text.into(),
        }
    }
    pub fn reverse_after_chaining(&mut self, tool: &str) {
        if matches!(tool, "plist" | "jsonLines" | "envFile")
            && let Some(spec) = choices(tool).into_iter().find(|spec| spec.id == "mode")
        {
            let next = if self.value("mode") == spec.choices[0] {
                spec.choices[1]
            } else {
                spec.choices[0]
            };
            self.values.insert("mode", next.into());
        }
    }
}
pub fn choices(tool: &str) -> Vec<ChoiceSpec> {
    match tool {
        "plist" => vec![ChoiceSpec {
            id: "mode",
            label: "Convert",
            choices: &["Plist → JSON", "JSON → Plist"],
        }],
        "jsonLines" => vec![ChoiceSpec {
            id: "mode",
            label: "Convert",
            choices: &["Lines → Array", "Array → Lines"],
        }],
        "envFile" => vec![ChoiceSpec {
            id: "mode",
            label: "Convert",
            choices: &["Env → JSON", "JSON → Env"],
        }],
        "cookies" => vec![ChoiceSpec {
            id: "mode",
            label: "Header",
            choices: &["Set-Cookie", "Cookie"],
        }],
        "sqlFormat" => vec![
            ChoiceSpec {
                id: "keywords",
                label: "Keywords",
                choices: &["Uppercase", "Preserve"],
            },
            ChoiceSpec {
                id: "dialect",
                label: "Dialect",
                choices: &["PostgreSQL", "SQLite", "MySQL"],
            },
        ],
        _ => vec![],
    }
}
pub fn input_label(tool: &str) -> &'static str {
    match tool {
        "jsonSchema" => "JSON document",
        "jsonMerge" => "Original JSON",
        "plist" => "XML plist or typed JSON",
        "jsonLines" => "One complete JSON value per line",
        "envFile" => "One KEY=value per line · values stay literal",
        "cookies" => "Cookie header or one Set-Cookie per line",
        "sqlFormat" => "SQL · formats text; does not execute or validate queries",
        "certificate" => "PEM certificate · inspection only, not trust validation",
        "compare" => "First text",
        "listSet" => "First list · one item per line",
        "snippets" => "Template",
        _ => "Input",
    }
}
pub fn second_label(tool: &str) -> &'static str {
    match tool {
        "jsonSchema" => "Schema · supported keywords only",
        "jsonMerge" => "Merge patch",
        "listSet" => "Second list · one item per line",
        _ => "Second text",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_sql_controls_and_literal_mappings() {
        let mut state = ToolControls::new("sqlFormat", "");
        assert_eq!(state.value("keywords"), "Uppercase");
        assert_eq!(state.value("dialect"), "PostgreSQL");
        assert!(state.select("sqlFormat", "dialect", "MySQL"));
        assert!(state.select("sqlFormat", "keywords", "Preserve"));
        assert!(!state.select("sqlFormat", "dialect", "remote"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&state.second("sqlFormat", "")).unwrap(),
            serde_json::json!({"dialect":"MySQL","keywords":"Preserve"})
        );
    }
    #[test]
    fn plist_direction_and_chaining_follow_source() {
        let mut state = ToolControls::new("plist", "<plist/>");
        assert_eq!(state.value("mode"), "Plist → JSON");
        state.reverse_after_chaining("plist");
        assert_eq!(state.value("mode"), "JSON → Plist");
        assert_eq!(
            ToolControls::new("plist", "{\"type\":\"dict\",\"value\":{}}").value("mode"),
            "JSON → Plist"
        );
    }
    #[test]
    fn independent_windows_do_not_share_control_state() {
        let mut first = ToolControls::new("sqlFormat", "");
        let second = ToolControls::new("sqlFormat", "");
        first.select("sqlFormat", "keywords", "Preserve");
        assert_eq!(second.value("keywords"), "Uppercase");
    }
}
