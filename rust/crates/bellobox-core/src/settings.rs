use crate::launcher::Usage;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}
/// A separate Rust preferences file. Legacy Swift UserDefaults/Keychain are never modified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    pub appearance: Appearance,
    pub favorites: BTreeSet<String>,
    pub recents: Vec<String>,
    pub usage: Usage,
    pub zone_ids: Vec<String>,
    pub anchor_zone_id: String,
    pub provider_endpoint: String,
    pub provider_model: String,
    pub provider_kind: String,
    pub system_prompt: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            extra: Default::default(),
            appearance: Appearance::System,
            favorites: BTreeSet::new(),
            recents: Vec::new(),
            usage: Usage::default(),
            zone_ids: Vec::new(),
            anchor_zone_id: String::new(),
            provider_endpoint: "https://api.openai.com/v1".into(),
            provider_model: String::new(),
            provider_kind: "openai".into(),
            system_prompt: crate::ai::DEFAULT_SYSTEM_PROMPT.into(),
        }
    }
}
impl Settings {
    /// Explicit clock-location action only. Reload first so unrelated settings,
    /// including unknown forward-compatible keys, survive. Corruption blocks writes.
    pub fn save_clock_preferences(path: &Path, ids: &[String], anchor: &str) -> Result<(), String> {
        if ids.is_empty() {
            return Err("Keep at least one location.".into());
        }
        let mut unique = Vec::new();
        for id in ids {
            id.parse::<chrono_tz::Tz>()
                .map_err(|_| format!("Unknown IANA timezone: {id}"))?;
            if !unique.contains(id) {
                unique.push(id.clone());
            }
        }
        if !unique.iter().any(|id| id == anchor) {
            return Err("Reference must be one of your locations.".into());
        }
        // Validate the same bounded snapshot we patch. A typed serialization would
        // discard nested future fields and normalize unrelated collections.
        let mut value = if path.exists() {
            let bytes = read_bounded(path, 128_000)?;
            let settings: Self = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Preferences were not changed: {e}"))?;
            if settings.schema_version != 1 {
                return Err("Unknown preferences version; file left unchanged.".into());
            }
            if settings.system_prompt.len() > 32_000
                || settings.provider_endpoint.len() > 8_192
                || settings.provider_model.len() > 1_024
            {
                return Err("Settings exceed their bounded text limits; nothing was saved.".into());
            }
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())?
        } else {
            serde_json::to_value(Self::default()).map_err(|e| e.to_string())?
        };
        let object = value
            .as_object_mut()
            .ok_or("Preferences must be a JSON object; file left unchanged.")?;
        object.insert("zone_ids".into(), serde_json::json!(unique));
        object.insert("anchor_zone_id".into(), serde_json::json!(anchor));
        if serde_json::to_vec_pretty(&value)
            .map_err(|e| e.to_string())?
            .len()
            > 128_000
        {
            return Err("Settings exceed128 KB; nothing was saved.".into());
        }
        atomic_json(path, &value)
    }
    pub fn explicit_open(&mut self, id: &str, category: &str, now: f64) {
        self.recents.retain(|x| x != id);
        self.recents.insert(0, id.to_owned());
        self.recents.truncate(20);
        self.usage.record(category, id, now);
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = read_bounded(path, 128_000)?;
        let settings: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Preferences were not changed: {e}"))?;
        if settings.schema_version != 1 {
            return Err("Unknown preferences version; file left unchanged.".into());
        }
        Ok(settings)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.system_prompt.len() > 32_000
            || self.provider_endpoint.len() > 8_192
            || self.provider_model.len() > 1_024
        {
            return Err("Settings exceed their bounded text limits; nothing was saved.".into());
        }
        if serde_json::to_vec_pretty(self)
            .map_err(|e| e.to_string())?
            .len()
            > 128_000
        {
            return Err("Settings exceed128 KB; nothing was saved.".into());
        }
        atomic_json(path, self)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub body: String,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Snippets {
    pub snippets: Vec<Snippet>,
}
impl Snippets {
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&read_bounded(path, 5_000_000)?).map_err(|e| e.to_string())
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.snippets.len() > 500
            || self
                .snippets
                .iter()
                .any(|s| s.body.len() > crate::MAX_INPUT_BYTES || s.name.len() > 512)
        {
            return Err("Snippet limits exceeded.".into());
        }
        atomic_json(path, self)
    }
    pub fn render(
        snippet: &str,
        values: &std::collections::BTreeMap<String, String>,
    ) -> Result<String, String> {
        let mut out = String::new();
        let mut rest = snippet;
        while let Some(i) = rest.find("{{") {
            out.push_str(&rest[..i]);
            rest = &rest[i + 2..];
            let end = rest.find("}}").ok_or("Unclosed template field")?;
            let name = rest[..end].trim();
            out.push_str(
                values
                    .get(name)
                    .ok_or_else(|| format!("Missing template field: {name}"))?,
            );
            rest = &rest[end + 2..];
            if out.len() > crate::MAX_INPUT_BYTES {
                return Err("Rendered snippet exceeds input limit.".into());
            }
        }
        out.push_str(rest);
        crate::validate_input(&out)?;
        Ok(out)
    }
}
pub fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("BELLOBOX_CONFIG_DIR") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "macos")]
    {
        return PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Library/Application Support/BelloBox/Rust");
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
            })
            .join("bellobox-rust")
    }
}
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("File exceeds its size limit; left unchanged.".into());
    }
    Ok(bytes)
}
fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("File needs a parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let staged = parent.join(format!(
        ".bellobox-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&staged).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&staged, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(staged);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_never_include_text_or_secrets() {
        let mut s = Settings::default();
        s.explicit_open("qr", "url", 10.);
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("selected_text"));
    }
    #[test]
    fn roundtrip_and_corrupt_file_preservation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        Settings::default().save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap().schema_version, 1);
        fs::write(&path, b"corrupt").unwrap();
        assert!(Settings::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"corrupt");
    }
    #[test]
    fn old_settings_receive_source_prompt_default() {
        let settings: Settings =
            serde_json::from_str(r#"{"schema_version":1,"provider_model":"local-model"}"#).unwrap();
        assert_eq!(settings.system_prompt, crate::ai::DEFAULT_SYSTEM_PROMPT);
        assert_eq!(settings.provider_model, "local-model");
    }
    #[test]
    fn oversized_prompt_never_replaces_existing_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut settings = Settings::default();
        settings.save(&path).unwrap();
        let before = fs::read(&path).unwrap();
        settings.system_prompt = "x".repeat(32_001);
        assert!(settings.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    #[test]
    fn template_substitution_is_literal() {
        let mut v = std::collections::BTreeMap::new();
        v.insert("name".into(), "{{other}}".into());
        assert_eq!(
            Snippets::render("Hi {{ name }}", &v).unwrap(),
            "Hi {{other}}"
        );
        assert!(Snippets::render("{{missing}}", &v).is_err());
    }
}

#[cfg(test)]
mod clock_preference_tests {
    use super::*;
    #[test]
    fn fresh_unset_and_explicit_utc_are_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(Settings::load(&path).unwrap().zone_ids.is_empty());
        assert!(!path.exists());
        fs::write(&path, r#"{"zone_ids":["UTC"],"anchor_zone_id":"UTC"}"#).unwrap();
        assert_eq!(Settings::load(&path).unwrap().zone_ids, vec!["UTC"]);
        fs::write(&path, r#"{"provider_model":"test"}"#).unwrap();
        let settings = Settings::load(&path).unwrap();
        assert!(settings.zone_ids.is_empty());
        assert!(settings.anchor_zone_id.is_empty());
    }
    #[test]
    fn clock_patch_preserves_unknown_and_unrelated_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"provider_model":"new-model","appearance":"Dark","future_option":{"nested":[1,"two"]},"zone_ids":["UTC"]}"#).unwrap();
        Settings::save_clock_preferences(
            &path,
            &["Asia/Tokyo".into(), "UTC".into(), "UTC".into()],
            "Asia/Tokyo",
        )
        .unwrap();
        let settings = Settings::load(&path).unwrap();
        assert_eq!(settings.provider_model, "new-model");
        assert_eq!(settings.appearance, Appearance::Dark);
        assert_eq!(
            settings.extra["future_option"],
            serde_json::json!({"nested":[1,"two"]})
        );
        assert_eq!(settings.zone_ids, vec!["Asia/Tokyo", "UTC"]);
        assert_eq!(settings.anchor_zone_id, "Asia/Tokyo");
    }
    #[test]
    fn invalid_clock_patch_and_corrupt_preferences_are_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for bytes in [
            b"broken".as_slice(),
            br#"{"schema_version":2}"#,
            br#"{"zone_ids":["UTC"]}"#,
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(Settings::save_clock_preferences(&path, &[], "UTC").is_err());
            assert!(
                Settings::save_clock_preferences(&path, &["bad/zone".into()], "bad/zone").is_err()
            );
            assert!(
                Settings::save_clock_preferences(&path, &["UTC".into()], "Asia/Tokyo").is_err()
            );
            if bytes != br#"{"zone_ids":["UTC"]}"# {
                assert!(Settings::save_clock_preferences(&path, &["UTC".into()], "UTC").is_err());
            }
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
    #[test]
    fn session_time_actions_do_not_write_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings::load(&path).unwrap();
        let now = crate::clock::parse_instant("2026-10-05T12:00:00Z").unwrap();
        let mut p = crate::clock::Planner::from_preferences(
            &settings.zone_ids,
            &settings.anchor_zone_id,
            "Asia/Tokyo",
            now,
            None,
        )
        .unwrap();
        p.refresh_now(now + chrono::Duration::minutes(1)).unwrap();
        p.select_time("10:30").unwrap();
        p.select_date("2026-10-06").unwrap();
        let _ = p.meeting_summary();
        p.go_to_now(now).unwrap();
        assert!(!path.exists());
        assert_eq!(p.zones.len(), 2);
    }
}

#[cfg(test)]
mod clock_raw_patch_tests {
    use super::*;
    #[test]
    fn clock_patch_changes_only_two_keys_in_original_ordered_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = r#"{"favorites":["z","a","z"],"usage":{"entries":{},"future_nested":{"z":[3,2,1],"a":true}},"future":{"z":1,"a":2},"provider_model":"model","zone_ids":["UTC"],"anchor_zone_id":"UTC"}"#;
        fs::write(&path, original).unwrap();
        let mut expected: serde_json::Value = serde_json::from_str(original).unwrap();
        expected["zone_ids"] = serde_json::json!(["Asia/Tokyo", "UTC"]);
        expected["anchor_zone_id"] = serde_json::json!("Asia/Tokyo");
        Settings::save_clock_preferences(&path, &["Asia/Tokyo".into(), "UTC".into()], "Asia/Tokyo")
            .unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(actual, expected);
        // Value equality alone does not verify object insertion order.
        assert_eq!(
            serde_json::to_string(&actual).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
    }
}
