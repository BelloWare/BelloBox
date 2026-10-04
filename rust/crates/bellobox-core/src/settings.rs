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
            appearance: Appearance::System,
            favorites: BTreeSet::new(),
            recents: Vec::new(),
            usage: Usage::default(),
            zone_ids: vec!["UTC".into()],
            anchor_zone_id: "UTC".into(),
            provider_endpoint: "https://api.openai.com/v1".into(),
            provider_model: String::new(),
            provider_kind: "openai".into(),
            system_prompt: crate::ai::DEFAULT_SYSTEM_PROMPT.into(),
        }
    }
}
impl Settings {
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
