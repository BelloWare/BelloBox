//! SnippetTemplate / UtilityWorkbenchModel semantics from the Swift source.
use chrono::{DateTime, Utc};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const MAX_OUTPUT_BYTES: usize = 4_000_000;
fn pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\{\{([A-Za-z][A-Za-z0-9_ -]{0,59})\}\}").unwrap())
}
pub fn placeholders(template: &str) -> Result<Vec<String>, String> {
    crate::validate_input(template)?;
    let mut seen = BTreeSet::new();
    Ok(pattern()
        .captures_iter(template)
        .filter_map(|capture| {
            let name = capture[1].to_owned();
            seen.insert(name.clone()).then_some(name)
        })
        .collect())
}
pub fn custom_fields(template: &str) -> Result<Vec<String>, String> {
    Ok(placeholders(template)?
        .into_iter()
        .filter(|name| !matches!(name.as_str(), "selection" | "date" | "timestamp" | "uuid"))
        .collect())
}

/// Per-tool context: selection and UUID are captured once; values are never saved.
#[derive(Clone, Debug)]
pub struct Session {
    selection: String,
    uuid: String,
    values: BTreeMap<String, String>,
}
impl Session {
    pub fn new(selection: String) -> Self {
        Self {
            selection,
            uuid: uuid::Uuid::new_v4().to_string(),
            values: BTreeMap::new(),
        }
    }
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
    pub fn set_value(&mut self, name: String, value: String) {
        self.values.insert(name, value);
    }
    pub fn reset_fields(&mut self) {
        self.values.clear();
    }
    pub fn render(&self, template: &str) -> Result<String, String> {
        render_at(
            template,
            &self.selection,
            &self.values,
            Utc::now(),
            &self.uuid,
        )
    }
}
/// Matches only the original template. Inserted text is never evaluated again.
/// Time/UUID arguments make regression tests deterministic; GUI UUID comes from Session.
pub fn render_at(
    template: &str,
    selection: &str,
    values: &BTreeMap<String, String>,
    now: DateTime<Utc>,
    uuid: &str,
) -> Result<String, String> {
    crate::validate_input(template)?;
    let date = now.format("%Y-%m-%d").to_string();
    // Swift Int64(TimeInterval) truncates toward zero, including before the epoch.
    let timestamp = (now.timestamp()
        + i64::from(now.timestamp() < 0 && now.timestamp_subsec_nanos() > 0))
    .to_string();
    let mut output = String::new();
    let mut end = 0;
    for captures in pattern().captures_iter(template) {
        let matched = captures.get(0).unwrap();
        append(&mut output, &template[end..matched.start()])?;
        let value = match &captures[1] {
            "selection" => selection,
            "date" => &date,
            "timestamp" => &timestamp,
            "uuid" => uuid,
            name => values
                .get(name)
                .map(String::as_str)
                .unwrap_or(matched.as_str()),
        };
        append(&mut output, value)?;
        end = matched.end();
    }
    append(&mut output, &template[end..])?;
    Ok(output)
}
fn append(output: &mut String, value: &str) -> Result<(), String> {
    if value.len() > MAX_OUTPUT_BYTES.saturating_sub(output.len()) {
        return Err("Rendered snippet exceeds 4 MB; nothing was truncated.".into());
    }
    output.push_str(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-06T00:45:06+02:00")
            .unwrap()
            .with_timezone(&Utc)
    }
    #[test]
    fn source_grammar_order_dedup_and_reserved_case() {
        assert_eq!(custom_fields("{{name}} {{full name}} {{first-name}} {{name}} {{Name}} {{a_2}} {{_bad}} {{2bad}} {{date}} {{Date}} {{uuid}} {{timestamp}} {{selection}}").unwrap(), vec!["name", "full name", "first-name", "Name", "a_2", "Date"]);
        assert_eq!(
            placeholders(&format!(
                "{{{{{}}}}} {{{{{}}}}}",
                "a".repeat(60),
                "b".repeat(61)
            ))
            .unwrap(),
            vec!["a".repeat(60)]
        );
    }
    #[test]
    fn missing_empty_and_literal_substitution_are_distinct() {
        let values = BTreeMap::from([
            ("name".into(), "{{date}} 🦊".into()),
            ("empty".into(), String::new()),
        ]);
        assert_eq!(
            render_at(
                "{{name}}|{{missing}}|{{empty}}|{{ name }}|{{selection}}",
                "{{name}}",
                &values,
                time(),
                "fixed"
            )
            .unwrap(),
            "{{date}} 🦊|{{missing}}||{{ name }}|{{name}}"
        );
    }
    #[test]
    fn builtins_override_values_and_use_utc() {
        let values = BTreeMap::from([
            ("date".into(), "fake".into()),
            ("uuid".into(), "fake".into()),
        ]);
        assert_eq!(
            render_at(
                "{{date}} {{timestamp}} {{uuid}}",
                "",
                &values,
                time(),
                "fixed"
            )
            .unwrap(),
            format!("2026-10-05 {} fixed", time().timestamp())
        );
        let before = DateTime::parse_from_rfc3339("1969-12-31T23:59:59.500Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            render_at("{{timestamp}}", "", &values, before, "fixed").unwrap(),
            "0"
        );
    }
    #[test]
    fn session_uuid_selection_and_values_survive_edits_but_fields_reset() {
        let mut session = Session::new("{{name}} selected".into());
        let uuid = session.render("{{uuid}}").unwrap();
        assert!(uuid::Uuid::parse_str(&uuid).is_ok());
        session.set_value("name".into(), "Ada".into());
        assert_eq!(session.render("plain").unwrap(), "plain");
        assert_eq!(
            session.render("{{name}} {{uuid}} {{selection}}").unwrap(),
            format!("Ada {uuid} {{{{name}}}} selected")
        );
        session.reset_fields();
        assert_eq!(
            session.render("{{name}} {{uuid}}").unwrap(),
            format!("{{{{name}}}} {uuid}")
        );
        assert_ne!(
            Session::new(String::new()).render("{{uuid}}").unwrap(),
            uuid
        );
    }
    #[test]
    fn template_and_expansion_bounds_are_explicit() {
        assert!(placeholders(&"x".repeat(crate::MAX_INPUT_BYTES + 1)).is_err());
        let values = BTreeMap::from([("x".into(), "z".repeat(500_000))]);
        assert_eq!(
            render_at(&"{{x}}".repeat(8), "", &values, time(), "")
                .unwrap()
                .len(),
            MAX_OUTPUT_BYTES
        );
        assert!(render_at(&"{{x}}".repeat(9), "", &values, time(), "").is_err());
    }
    #[test]
    fn zero_and_many_custom_fields_are_complete() {
        assert!(custom_fields("{{date}} plain").unwrap().is_empty());
        let template = (0..10_000)
            .map(|i| format!("{{{{field{i}}}}}"))
            .collect::<String>();
        let names = custom_fields(&template).unwrap();
        assert_eq!(names.len(), 10_000);
        assert_eq!(names.last().unwrap(), "field9999");
        assert_eq!(
            Session::new(String::new())
                .render("Hello {{name}}")
                .unwrap(),
            "Hello {{name}}"
        );
    }
}
