//! Bounded, pure Swift WorldClockAIResolver protocol. No credential persistence,
//! network or planner mutation. Rust's IANA catalog remains the zone boundary.
//!
//! Questions match Swift's extended-grapheme and UTF-8 byte limits. History and
//! answers are bounded by Unicode scalar values, not bytes or graphemes. Swift's
//! strict typed envelope decoding and first-brace/last-brace extraction are
//! retained: a wrongly typed field falls back to plain text; a well-typed but
//! invalid date/zone drops only that proposal part and surfaces an issue.
//! Rust additionally bounds responses to 8 MB, prompts to 64 KB and contexts to
//! 24 rows with 512-byte labels/descriptions. Dates use chrono's strict RFC3339
//! parser and the inclusive UTC range 0001-01-01 through 9999-12-31T23:59:59Z;
//! Foundation's permissive ISO parser/catalog are not claimed as exact parity.
use super::{Suggestion, valid_identifiers};
use crate::{
    ai,
    clock::{Planner, Quality, zone_option},
};
use chrono::{DateTime, Datelike, SecondsFormat, Timelike, Utc};
use serde::Deserialize;
use unicode_segmentation::UnicodeSegmentation;

pub const QUESTION_LIMIT: usize = 2_000;
pub const QUESTION_BYTE_LIMIT: usize = 8_192;
pub const HISTORY_TURNS: usize = 6;
pub const HISTORY_TURN_LIMIT: usize = 600;
pub const ANSWER_LIMIT: usize = 4_000;
pub const MAX_PROMPT_BYTES: usize = 64_000;
pub const MAX_RESPONSE_BYTES: usize = 8_000_000;
pub const MAXIMUM_ZONES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}
impl Role {
    fn label(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Assistant => "Assistant",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub zone_id: String,
    pub name: String,
    pub local_description: String,
    pub quality: Quality,
    pub is_reference: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub selected_instant: DateTime<Utc>,
    pub reference_zone_id: String,
    pub locations: Vec<Location>,
    pub now: DateTime<Utc>,
    pub local_zone_id: String,
    pub is_following_now: bool,
}
impl Context {
    pub fn from_planner(
        planner: &Planner,
        now: DateTime<Utc>,
        local_zone: &str,
    ) -> Result<Self, String> {
        if planner.zones.is_empty() || planner.zones.len() > 24 {
            return Err("Choose 1–24 zones before asking the copilot.".into());
        }
        if local_zone.parse::<chrono_tz::Tz>().is_err()
            || !planner.zones.contains(&planner.reference)
        {
            return Err("The planner has an unsupported local or reference zone.".into());
        }
        if !supported_instant(planner.instant) || !supported_instant(now) {
            return Err(
                "The planner time is outside the supported calendar range (years 1–9999).".into(),
            );
        }
        planner.timeline()?;
        Ok(Self {
            selected_instant: planner.instant,
            reference_zone_id: planner.reference.name().into(),
            locations: planner
                .presentations()
                .into_iter()
                .map(|p| Location {
                    zone_id: p.id,
                    name: p.name,
                    local_description: format!(
                        "{}, {} ({})",
                        p.date_text, p.time_text, p.zone_text
                    ),
                    quality: p.quality,
                    is_reference: p.is_reference,
                })
                .collect(),
            now,
            local_zone_id: local_zone.into(),
            is_following_now: planner.follows_now,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotRequest {
    pub question: String,
    pub context: Context,
    pub history: Vec<Turn>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub answer: String,
    pub suggestion: Option<Suggestion>,
    pub suggestion_issue: Option<String>,
}

pub fn validate_question(question: &str) -> Result<String, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("Ask a question about the selected time or locations first.".into());
    }
    if question.len() > QUESTION_BYTE_LIMIT || question.graphemes(true).count() > QUESTION_LIMIT {
        return Err("Keep questions under 2,000 characters and 8 KB.".into());
    }
    Ok(question.into())
}
fn bounded(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result + "…"
    } else {
        result
    }
}
fn iso(date: DateTime<Utc>) -> String {
    date.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub const SYSTEM_PROMPT: &str = "You are the World Clock copilot inside Bello Box, a macOS utility for comparing times and planning meetings across time zones. Treat the planner context as ground truth and do time-zone arithmetic carefully, including daylight-saving offsets and date changes. Working hours are 09:00-17:00 local time, fringe hours are 07:00-09:00 and 17:00-21:00, and everything else is night. Be concise: at most three short sentences or a short list. Never invent locations that are not in the context or the question.\n\nReply with exactly one JSON object and nothing else, using this schema:\n{\"answer\":\"plain-text answer\",\"suggestion\":{\"referenceDate\":\"RFC3339 timestamp or null\",\"timeZoneIDs\":[\"IANA/Zone\"],\"replaceLocations\":false,\"anchorTimeZoneID\":\"IANA/Zone or null\"}}\nSet \"suggestion\" to null unless the user asks to move the meeting time, pick a slot, or add, remove, or replace locations. Use canonical IANA time-zone identifiers and put referenceDate at the proposed instant. To add locations, list only the new zones with replaceLocations false. To remove a location or replace the list, set replaceLocations to true and list every zone that should remain, keeping all of the user's other locations. Do not use Markdown or code fences.";

pub fn user_prompt(request: &CopilotRequest) -> Result<String, String> {
    let question = validate_question(&request.question)?;
    let c = &request.context;
    // Public context fields are checked too: callers cannot bypass bounds by
    // fabricating rows rather than using from_planner.
    if c.locations.is_empty()
        || c.locations.len() > 24
        || !supported_instant(c.selected_instant)
        || !supported_instant(c.now)
        || c.reference_zone_id.parse::<chrono_tz::Tz>().is_err()
        || c.local_zone_id.parse::<chrono_tz::Tz>().is_err()
        || !c.locations.iter().any(|l| l.zone_id == c.reference_zone_id)
        || c.locations.iter().any(|l| {
            l.zone_id.parse::<chrono_tz::Tz>().is_err()
                || l.name.len() > 512
                || l.local_description.len() > 512
        })
    {
        return Err("The planner context is invalid or exceeds the copilot limits.".into());
    }
    let mut lines = vec![
        "Planner context (ground truth):".to_owned(),
        format!(
            "- Current time: {} (user's local zone {})",
            iso(c.now),
            c.local_zone_id
        ),
        format!(
            "- Selected instant: {} ({})",
            iso(c.selected_instant),
            if c.is_following_now {
                "live time"
            } else {
                "planning time"
            }
        ),
        format!(
            "- Reference location: {} ({})",
            zone_option(&c.reference_zone_id).name,
            c.reference_zone_id
        ),
        "- Locations at the selected instant:".into(),
    ];
    for l in &c.locations {
        lines.push(format!(
            "  - {} ({}): {} — {}{}",
            l.name,
            l.zone_id,
            l.local_description,
            l.quality.label().to_lowercase(),
            if l.is_reference { " [reference]" } else { "" }
        ));
    }
    let history = &request.history[request.history.len().saturating_sub(HISTORY_TURNS)..];
    if !history.is_empty() {
        lines.extend([String::new(), "Conversation so far:".into()]);
        for turn in history {
            // Bound before replacing: replacing a huge supplied history turn
            // must not allocate an unbounded temporary.
            let text = bounded(&turn.text, HISTORY_TURN_LIMIT).replace('\n', " ");
            lines.push(format!("{}: {}", turn.role.label(), text));
        }
    }
    lines.extend([String::new(), format!("Question: {question}")]);
    let prompt = lines.join("\n");
    crate::validate_input(&prompt)?;
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err("The planner prompt exceeds 64 KB; nothing was sent.".into());
    }
    Ok(prompt)
}

/// Reuses every existing endpoint, credential and provider validation. Only the
/// validated raw user field replaces the writing assistant's selection wrapper.
pub fn provider_request(
    config: &ai::Config,
    key: &str,
    request: &CopilotRequest,
) -> Result<ai::Request, String> {
    let prompt = user_prompt(request)?;
    let mut config = config.clone();
    config.system_prompt = SYSTEM_PROMPT.into();
    let mut result = ai::request(&config, key, "World Clock question", "")?;
    let pointer = match config.provider {
        ai::Provider::OpenAIChat => "/messages/1/content",
        ai::Provider::OpenAIResponses => "/input",
        ai::Provider::Anthropic => "/messages/0/content",
    };
    let field = result
        .body
        .pointer_mut(pointer)
        .filter(|v| v.is_string())
        .ok_or("Unexpected provider request shape.")?;
    *field = serde_json::Value::String(prompt);
    Ok(result)
}

#[derive(Deserialize)]
struct Payload {
    answer: Option<String>,
    suggestion: Option<Proposal>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Proposal {
    reference_date: Option<String>,
    #[serde(rename = "timeZoneIDs")]
    time_zone_ids: Option<Vec<String>>,
    replace_locations: Option<bool>,
    #[serde(rename = "anchorTimeZoneID")]
    anchor_time_zone_id: Option<String>,
}
fn supported_instant(date: DateTime<Utc>) -> bool {
    (1..=9999).contains(&date.year())
        && !(date.year() == 9999
            && date.ordinal() == 365
            && date.hour() == 23
            && date.minute() == 59
            && date.second() == 59
            && date.nanosecond() > 0)
}
pub fn parse_response(response: &str) -> Result<Reply, String> {
    if response.len() > MAX_RESPONSE_BYTES {
        return Err("Provider response exceeds 8 MB limit.".into());
    }
    let response = response.trim();
    if response.is_empty() {
        return Err("The copilot returned an empty answer.".into());
    }
    let payload = response
        .find('{')
        .zip(response.rfind('}'))
        .filter(|(a, b)| a <= b)
        .and_then(|(a, b)| serde_json::from_str::<Payload>(&response[a..=b]).ok())
        .filter(|p| p.answer.is_some() || p.suggestion.is_some());
    let Some(payload) = payload else {
        return Ok(Reply {
            answer: bounded(response, ANSWER_LIMIT),
            suggestion: None,
            suggestion_issue: None,
        });
    };
    let mut issues = Vec::new();
    let mut suggestion = None;
    if let Some(proposal) = payload.suggestion {
        let mut candidate = Suggestion::default();
        if let Some(value) = proposal
            .reference_date
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            match DateTime::parse_from_rfc3339(value).map(|d| d.with_timezone(&Utc)) {
                Ok(date) if supported_instant(date) => candidate.instant = Some(date),
                Ok(_) => issues.push(format!("The suggested time {} is outside the supported calendar range (years 1–9999) and was ignored.", bounded(value,40))),
                Err(_) => issues.push(format!("The suggested time \"{}\" could not be read.", bounded(value,40))),
            }
        }
        let requested: Vec<String> = proposal
            .time_zone_ids
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        let invalid: Vec<_> = requested
            .iter()
            .filter(|s| s.parse::<chrono_tz::Tz>().is_err())
            .take(4)
            .map(|s| bounded(s, 40))
            .collect();
        if !invalid.is_empty() {
            issues.push(format!(
                "Unsupported time zones were ignored: {}.",
                invalid.join(", ")
            ));
        }
        candidate.zone_ids = valid_identifiers(&requested)
            .into_iter()
            .take(MAXIMUM_ZONES)
            .collect();
        candidate.replaces_locations = proposal.replace_locations.unwrap_or(false);
        if let Some(anchor) = proposal
            .anchor_time_zone_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if anchor.parse::<chrono_tz::Tz>().is_ok() {
                candidate.anchor_zone_id = Some(anchor.into());
            } else {
                issues.push(format!(
                    "The suggested reference zone \"{}\" is not supported.",
                    bounded(anchor, 40)
                ));
            }
        }
        if !candidate.is_empty() {
            suggestion = Some(candidate);
        }
    }
    let answer = bounded(payload.answer.as_deref().unwrap_or("").trim(), ANSWER_LIMIT);
    if answer.is_empty() && suggestion.is_none() {
        return Err("The copilot returned an empty answer.".into());
    }
    Ok(Reply {
        answer: if answer.is_empty() {
            "Here is a change you can apply.".into()
        } else {
            answer
        },
        suggestion,
        suggestion_issue: (!issues.is_empty()).then(|| issues.join(" ")),
    })
}

#[cfg(test)]
mod tests;
