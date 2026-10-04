use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Command {
    pub id: &'static str,
    pub title: &'static str,
    pub keywords: &'static str,
}
pub fn catalog() -> Vec<Command> {
    let entries = [
        ("json", "JSON Tools", "pretty minify validate lossless"),
        ("compare", "Compare Text & JSON", "diff difference"),
        ("jwt", "Inspect JWT", "bearer claims token expiration"),
        ("regex", "Regex Tester", "regular expression matches"),
        ("url", "URL & Query Editor", "parameters encode decode"),
        ("time", "Timestamp Converter", "epoch unix iso date"),
        ("cron", "Cron Schedule", "crontab next runs"),
        ("convert", "Convert JSON, YAML & CSV", "tsv table"),
        ("snippets", "Snippets & Templates", "saved fields"),
        ("http", "HTTP & cURL", "request rest network"),
        ("generate", "Developer Generators", "uuid random"),
        ("calculator", "Calculator", "math expression"),
        ("units", "Unit Converter", "length mass temperature"),
        ("numberBase", "Number Base", "binary decimal hex octal"),
        ("color", "Color Converter", "hex rgb hsl"),
        ("contrast", "Contrast Checker", "wcag accessible"),
        ("gradient", "Gradient Builder", "css stops"),
        ("markdown", "Markdown Preview", "html"),
        ("jsonPointer", "JSON Pointer", "rfc6901"),
        ("jsonFlatten", "JSON Flatten", "typed entries unflatten"),
        ("jsonCode", "JSON to Code", "typescript swift rust"),
        ("sqlInsert", "SQL INSERT", "data generation"),
        ("xmlJSON", "XML & JSON", "xml converter"),
        ("unicode", "Unicode Inspector", "codepoints utf8"),
        ("stringEscape", "String Escape", "json literal"),
        ("extract", "Extract from Text", "email url ip"),
        (
            "listSet",
            "List & Set Operations",
            "union intersection difference",
        ),
        ("semver", "Semantic Version", "version sort"),
        ("subnet", "Subnet Calculator", "ipv4 cidr"),
        ("chmod", "Chmod Calculator", "permissions octal"),
        ("hmac", "HMAC", "sha256 key"),
        ("jsonSchema", "JSON Schema", "validate draft2020"),
        ("jsonMerge", "JSON Merge Patch", "rfc7396"),
        ("jsonRedact", "JSON Redact", "field names secrets"),
        ("jsonLines", "JSON Lines", "ndjson"),
        ("csvExplore", "CSV Explorer", "rows columns"),
        ("envFile", "Environment File", "dotenv values"),
        ("plist", "Property Lists", "typed xml"),
        ("sqlFormat", "SQL Formatter", "dialect indent"),
        ("httpHeaders", "HTTP Headers", "duplicates ordered"),
        ("cookies", "Cookies", "set-cookie fields"),
        ("certificate", "Certificate Inspector", "pem x509"),
        ("sshKey", "SSH Public Key", "fingerprint"),
        ("uuidInspect", "UUID Inspector", "guid version"),
        ("bitwise", "Bitwise Calculator", "64 bit shift"),
        ("statistics", "Statistics", "mean median deviation"),
        ("dateMath", "Date Math", "duration date difference"),
        ("aspectRatio", "Aspect Ratio", "dimensions scale"),
        ("bezier", "Cubic Bezier", "css easing"),
        ("boxShadow", "CSS Box Shadow", "blur spread"),
        ("textTable", "Text Table", "markdown ascii"),
        ("ai", "Ask AI", "rewrite explain summarize translate"),
        ("screenshot", "Screenshot", "capture screen area window"),
        ("scrollCapture", "Scrolling Screenshot", "stitch page"),
        ("recording", "Screen Recording", "movie audio"),
        ("videoToGIF", "Video to GIF", "convert movie frames"),
        ("worldClock", "World Clock", "timezone meeting planner"),
        ("qr", "QR Code", "scannable link"),
        (
            "textTools",
            "Text Tools",
            "case encode decode hashes lines count",
        ),
        ("settings", "Settings", "preferences provider appearance"),
        ("home", "Home & Status", "permissions capabilities"),
    ];
    entries
        .into_iter()
        .map(|(id, title, keywords)| Command {
            id,
            title,
            keywords,
        })
        .collect()
}
pub fn suggestions(input: &str) -> Vec<&'static str> {
    if input.is_empty() || input.len() > crate::MAX_INPUT_BYTES {
        return vec![];
    }
    let text = input.trim();
    if text.starts_with('{') || text.starts_with('[') {
        return vec!["json", "compare", "convert"];
    }
    if text.starts_with("https://") || text.starts_with("http://") {
        return vec!["url", "qr", "http"];
    }
    if text.starts_with("curl ") {
        return vec!["http"];
    }
    if text.starts_with("eyJ") && text.split('.').count() == 3 {
        return vec!["jwt"];
    }
    if text.len() <= 256 && crate::clock::parse_instant(text).is_ok() {
        return vec!["worldClock", "time"];
    }
    if text.contains('\n') && (text.contains(",") || text.contains(": ")) {
        return vec!["convert", "compare"];
    }
    vec!["textTools", "compare", "snippets"]
}
pub fn category(input: &str) -> &'static str {
    if input.is_empty() {
        return "empty";
    }
    match suggestions(input).first().copied() {
        Some("json") => "json",
        Some("url") => "url",
        Some("jwt") => "jwt",
        Some("worldClock") => "timestamp",
        Some("convert") => "data",
        Some("http") => "curl",
        _ => "text",
    }
}
/// Learning stores only category/tool IDs, bounded weights, and timestamps.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    entries: BTreeMap<String, BTreeMap<String, Use>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Use {
    weight: f64,
    last_used: f64,
}
impl Use {
    fn decayed(&self, now: f64) -> f64 {
        self.weight * 0.5_f64.powf((now - self.last_used).max(0.) / (30. * 86400.))
    }
}
impl Usage {
    pub fn record(&mut self, category: &str, command: &str, now: f64) {
        if !now.is_finite()
            || !valid_category(category)
            || !valid_command(command)
            || ["home", "settings"].contains(&command)
        {
            return;
        }
        self.entries.retain(|category, values| {
            values.retain(|id, value| {
                valid_command(id)
                    && value.weight.is_finite()
                    && value.last_used.is_finite()
                    && value.decayed(now) >= 0.05
            });
            valid_category(category) && !values.is_empty()
        });
        let values = self.entries.entry(category.into()).or_default();
        let old = values.get(command).map(|v| v.decayed(now)).unwrap_or(0.);
        values.insert(
            command.into(),
            Use {
                weight: (old + 1.).min(4.),
                last_used: now,
            },
        );
    }
    pub fn score(&self, category: &str, command: &str, now: f64) -> i64 {
        if !now.is_finite() {
            return 0;
        }
        self.entries
            .get(category)
            .and_then(|v| v.get(command))
            .map(|v| (v.decayed(now) * 350.).round().clamp(0., 1400.) as i64)
            .unwrap_or(0)
    }
    pub fn reset(&mut self) {
        self.entries.clear();
    }
}
fn valid_category(value: &str) -> bool {
    [
        "empty",
        "text",
        "json",
        "url",
        "jwt",
        "timestamp",
        "cron",
        "data",
        "curl",
    ]
    .contains(&value)
}
fn valid_command(value: &str) -> bool {
    catalog().iter().any(|c| c.id == value)
}
pub fn search(
    query: &str,
    input: &str,
    favorites: &BTreeSet<String>,
    recents: &[String],
    usage: &Usage,
    now: f64,
) -> Vec<Command> {
    let query = query.trim().to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    let suggested = suggestions(input);
    let category = category(input);
    let mut ranked: Vec<_> = catalog()
        .into_iter()
        .enumerate()
        .filter_map(|(index, c)| {
            let title = c.title.to_lowercase();
            let haystack = format!("{title} {}", c.keywords);
            if !terms.iter().all(|t| haystack.contains(t)) {
                return None;
            }
            let mut score = if favorites.contains(c.id) { 100 } else { 0 };
            if let Some(i) = suggested.iter().position(|id| *id == c.id) {
                score += 1000 - i as i64 * 200;
            }
            if let Some(i) = recents.iter().position(|id| id == c.id) {
                score += (40 - i as i64).max(0);
            }
            score += usage.score(category, c.id, now);
            if !query.is_empty() && title.starts_with(&query) {
                score += 10000;
            } else if !terms.is_empty()
                && terms
                    .iter()
                    .all(|term| title.split_whitespace().any(|word| word.starts_with(term)))
            {
                score += 6500;
            } else if !terms.is_empty() && terms.iter().all(|term| title.contains(term)) {
                score += 5000;
            }
            Some((score, index, c))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    ranked.into_iter().map(|(_, _, c)| c).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_matches_baseline_size() {
        let c = catalog();
        assert_eq!(c.len(), 61);
        assert_eq!(c.iter().map(|c| c.id).collect::<BTreeSet<_>>().len(), 61);
    }
    #[test]
    fn explicit_title_overrides_learning() {
        let mut u = Usage::default();
        for _ in 0..4 {
            u.record("text", "http", 0.);
        }
        let hits = search("ai", "hello", &BTreeSet::new(), &[], &u, 0.);
        assert_eq!(hits[0].id, "ai");
    }
    #[test]
    fn learning_decays_and_does_not_accept_text() {
        let mut u = Usage::default();
        for _ in 0..10 {
            u.record("json", "qr", 0.);
        }
        assert_eq!(u.score("json", "qr", 0.), 1400);
        assert_eq!(u.score("json", "qr", 30. * 86400.), 700);
        u.record("secret selection", "qr", 0.);
        assert!(
            !serde_json::to_string(&u)
                .unwrap()
                .contains("secret selection")
        );
    }
    #[test]
    fn oversized_input_never_suggests() {
        assert!(suggestions(&"x".repeat(500001)).is_empty());
    }
}
