use chrono::{
    DateTime, Datelike, Days, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone,
    Timelike, Utc,
};
use chrono_tz::Tz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Working,
    Extended,
    Poor,
}
impl Quality {
    pub fn at(instant: DateTime<Utc>, zone: Tz) -> Self {
        let hour = instant.with_timezone(&zone).hour();
        match hour {
            9..=16 => Self::Working,
            7..=8 | 17..=20 => Self::Extended,
            _ => Self::Poor,
        }
    }
    pub fn combined(instant: DateTime<Utc>, zones: &[Tz]) -> Self {
        if zones.is_empty() || zones.iter().any(|z| Self::at(instant, *z) == Self::Poor) {
            Self::Poor
        } else if zones.iter().all(|z| Self::at(instant, *z) == Self::Working) {
            Self::Working
        } else {
            Self::Extended
        }
    }
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Extended => "Fringe",
            Self::Poor => "Night",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "Working hours",
            Self::Extended => "Outside core hours",
            Self::Poor => "Late night / early morning",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Planner {
    pub instant: DateTime<Utc>,
    pub zones: Vec<Tz>,
    pub reference: Tz,
    pub follows_now: bool,
}
impl Default for Planner {
    fn default() -> Self {
        Self {
            instant: Utc::now(),
            zones: vec![
                chrono_tz::UTC,
                chrono_tz::America::Los_Angeles,
                chrono_tz::Europe::London,
                chrono_tz::Asia::Tokyo,
            ],
            reference: chrono_tz::UTC,
            follows_now: true,
        }
    }
}
impl Planner {
    /// Constructs a window session without writing preferences. An explicit UTC-only
    /// saved list stays UTC-only; only an empty/invalid list uses local then UTC.
    pub fn from_preferences(
        ids: &[String],
        anchor: &str,
        local_zone: &str,
        now: DateTime<Utc>,
        seed: Option<DateTime<Utc>>,
    ) -> Result<Self, String> {
        let zones = normalized_zones(ids, local_zone);
        let reference = anchor
            .parse::<Tz>()
            .ok()
            .filter(|z| zones.contains(z))
            .unwrap_or(zones[0]);
        let instant = seed.unwrap_or(now);
        validate_instant(instant)?;
        let planner = Self {
            instant,
            zones,
            reference,
            follows_now: seed.is_none(),
        };
        planner.timeline()?;
        Ok(planner)
    }
    pub fn date_input(&self) -> String {
        self.instant
            .with_timezone(&self.reference)
            .format("%Y-%m-%d")
            .to_string()
    }
    pub fn time_input(&self) -> String {
        self.instant
            .with_timezone(&self.reference)
            .format("%H:%M")
            .to_string()
    }
    pub fn go_to_now(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        self.set_instant(now)?;
        self.follows_now = true;
        Ok(())
    }
    pub fn refresh_now(&mut self, now: DateTime<Utc>) -> Result<bool, String> {
        if !self.follows_now {
            return Ok(false);
        }
        validate_instant(now)?;
        Timeline::containing(now, self.reference)?;
        if now.timestamp().div_euclid(60) == self.instant.timestamp().div_euclid(60) {
            return Ok(false);
        }
        self.instant = now;
        Ok(true)
    }
    pub fn set_reference(&mut self, id: &str) -> Result<(), String> {
        let zone = id.parse::<Tz>().map_err(|_| "Unknown IANA timezone")?;
        if !self.zones.contains(&zone) {
            return Err("Reference must be one of your locations.".into());
        }
        Timeline::containing(self.instant, zone)?;
        self.reference = zone;
        Ok(())
    }
    pub fn add_zone(&mut self, id: &str) -> Result<bool, String> {
        let zone = id.parse::<Tz>().map_err(|_| "Unknown IANA timezone")?;
        if self.zones.contains(&zone) {
            return Ok(false);
        }
        self.zones.push(zone);
        Ok(true)
    }
    pub fn remove_zone(&mut self, id: &str) -> Result<bool, String> {
        let Some(index) = self.zones.iter().position(|z| z.name() == id) else {
            return Ok(false);
        };
        if self.zones.len() == 1 {
            return Err("Keep at least one location.".into());
        }
        let reference = if self.zones[index] == self.reference {
            self.zones[if index == 0 { 1 } else { 0 }]
        } else {
            self.reference
        };
        Timeline::containing(self.instant, reference)?;
        self.zones.remove(index);
        self.reference = reference;
        Ok(true)
    }
    pub fn select_date(&mut self, text: &str) -> Result<(), String> {
        let date = NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map_err(|_| "Use a date in YYYY-MM-DD format.")?;
        if date.format("%Y-%m-%d").to_string() != text {
            return Err("Use a date in YYYY-MM-DD format.".into());
        }
        let local = self.instant.with_timezone(&self.reference).naive_local();
        let midnight = date.and_hms_opt(0, 0, 0).ok_or("Date overflow")?;
        let resolved_day = resolve_local_unbounded(self.reference, midnight)?
            .with_timezone(&self.reference)
            .date_naive();
        self.set_instant(resolve_local(
            self.reference,
            resolved_day.and_time(local.time()),
        )?)
    }
    pub fn select_time(&mut self, text: &str) -> Result<(), String> {
        let time =
            NaiveTime::parse_from_str(text, "%H:%M").map_err(|_| "Use a time in HH:MM format.")?;
        if time.format("%H:%M").to_string() != text {
            return Err("Use a time in HH:MM format.".into());
        }
        let date = self.instant.with_timezone(&self.reference).date_naive();
        self.set_instant(resolve_local(self.reference, date.and_time(time))?)
    }
    pub fn timeline(&self) -> Result<Timeline, String> {
        Timeline::containing(self.instant, self.reference)
    }
    pub fn timeline_qualities(&self) -> Result<Vec<Quality>, String> {
        let timeline = self.timeline()?;
        Ok((0..((timeline.duration_seconds() + 899) / 900))
            .map(|i| {
                let offset = (i * 900 + 450).min(timeline.duration_seconds() - 1);
                Quality::combined(
                    timeline.start + chrono::Duration::seconds(offset),
                    &self.zones,
                )
            })
            .collect())
    }
    pub fn set_offset(&mut self, seconds: f64) -> Result<(), String> {
        let timeline = self.timeline()?;
        self.set_instant(timeline.instant_at_offset(seconds)?)
    }

    pub fn nudge_steps(&mut self, steps: i64) -> Result<(), String> {
        self.nudge_minutes(steps.checked_mul(15).ok_or("Date overflow")?)
    }
    pub fn selected_quality(&self) -> Quality {
        Quality::combined(self.instant, &self.zones)
    }
    pub fn presentations(&self) -> Vec<ZonePresentation> {
        let reference_day = self.instant.with_timezone(&self.reference).date_naive();
        self.zones
            .iter()
            .map(|zone| {
                let local = self.instant.with_timezone(zone);
                let offset = local.offset().fix().local_minus_utc();
                let sign = if offset < 0 { '-' } else { '+' };
                let magnitude = offset.unsigned_abs();
                let utc = format!(
                    "UTC{sign}{:02}:{:02}",
                    magnitude / 3600,
                    magnitude % 3600 / 60
                );
                let abbreviation = local.format("%Z").to_string();
                let zone_text = if abbreviation.starts_with("GMT")
                    || abbreviation.starts_with('+')
                    || abbreviation.starts_with('-')
                {
                    utc
                } else {
                    format!("{abbreviation} - {utc}")
                };
                ZonePresentation {
                    id: zone.name().into(),
                    name: zone_option(zone.name()).name,
                    date_text: local.format("%a, %b %-d").to_string(),
                    time_text: local.format("%H:%M").to_string(),
                    zone_text,
                    day_difference: (local.date_naive() - reference_day).num_days(),
                    quality: Quality::at(self.instant, *zone),
                    is_reference: *zone == self.reference,
                }
            })
            .collect()
    }
    /// Source-shaped copy payload. Formatting is deliberately deterministic English
    /// and 24-hour until native locale formatting is implemented.
    pub fn meeting_summary(&self) -> String {
        let local = self.instant.with_timezone(&self.reference);
        let mut lines = vec![format!(
            "Meeting time — {} at {} ({})",
            local.format("%A, %B %-d, %Y"),
            local.format("%H:%M"),
            zone_option(self.reference.name()).name
        )];
        lines.extend(self.presentations().iter().map(|z| {
            format!(
                "{}: {}, {} ({})",
                z.name, z.date_text, z.time_text, z.zone_text
            )
        }));
        lines.join("\n")
    }
    pub fn set_instant(&mut self, instant: DateTime<Utc>) -> Result<(), String> {
        validate_instant(instant)?;
        Timeline::containing(instant, self.reference)?;
        self.instant = instant;
        self.follows_now = false;
        Ok(())
    }
    pub fn set_zones(&mut self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() || ids.len() > 24 {
            return Err("Choose 1–24 zones.".into());
        }
        let mut zones = Vec::new();
        for id in ids {
            let zone = id
                .parse::<Tz>()
                .map_err(|_| format!("Unknown IANA timezone: {id}"))?;
            if !zones.contains(&zone) {
                zones.push(zone);
            }
        }
        if !zones.contains(&self.reference) {
            self.reference = zones[0];
        }
        self.zones = zones;
        Ok(())
    }
    pub fn nudge_minutes(&mut self, minutes: i64) -> Result<(), String> {
        let next = self
            .instant
            .checked_add_signed(chrono::Duration::try_minutes(minutes).ok_or("Date overflow")?)
            .ok_or("Date overflow")?;
        self.set_instant(next)
    }
    /// Calendar days in the reference zone, not 24-hour durations. Ambiguous fall-back
    /// times pick the first occurrence; missing spring-forward times move forward.
    pub fn move_days(&mut self, days: i64) -> Result<(), String> {
        let local = self.instant.with_timezone(&self.reference).naive_local();
        let date = if days >= 0 {
            local
                .date()
                .checked_add_days(Days::new(days.unsigned_abs()))
        } else {
            local
                .date()
                .checked_sub_days(Days::new(days.unsigned_abs()))
        }
        .ok_or("Date overflow")?;
        // Resolve the calendar day first: a skipped civil date (e.g. Apia)
        // must not replace the requested wall time with the transition midnight.
        let midnight = date.and_hms_opt(0, 0, 0).ok_or("Date overflow")?;
        let resolved = resolve_local_unbounded(self.reference, midnight)?;
        let mut target_date = resolved.with_timezone(&self.reference).date_naive();
        if days < 0 && target_date > date {
            target_date = date.pred_opt().ok_or("Date overflow")?;
        }
        let instant = resolve_local(self.reference, target_date.and_time(local.time()))?;
        self.set_instant(instant)
    }
    pub fn summary(&self) -> String {
        self.zones
            .iter()
            .map(|zone| {
                format!(
                    "{}  {}  ({})",
                    zone,
                    self.instant
                        .with_timezone(zone)
                        .format("%a %Y-%m-%d %H:%M %Z %:z"),
                    Quality::at(self.instant, *zone).label()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}
impl Timeline {
    pub fn containing(instant: DateTime<Utc>, zone: Tz) -> Result<Self, String> {
        validate_instant(instant)?;
        let date = instant.with_timezone(&zone).date_naive();
        let next = date.succ_opt().ok_or("Date overflow")?;
        let boundary = |date: NaiveDate| -> Result<DateTime<Utc>, String> {
            resolve_local_unbounded(zone, date.and_hms_opt(0, 0, 0).ok_or("Date overflow")?)
        };
        let start = boundary(date)?;
        let end = boundary(next)?;
        if end <= start {
            return Err("Reference day has no duration.".into());
        }
        Ok(Self { start, end })
    }
    /// Use a captured timeline throughout a pointer drag so the next-midnight
    /// endpoint cannot shift the drag's origin on subsequent pointer events.
    pub fn instant_at_offset(&self, seconds: f64) -> Result<DateTime<Utc>, String> {
        if !seconds.is_finite() {
            return Err("Timeline offset must be finite.".into());
        }
        let millis = (seconds.clamp(0., self.duration_seconds() as f64) * 1000.).round() as i64;
        let instant = self
            .start
            .checked_add_signed(chrono::Duration::milliseconds(millis))
            .ok_or("Date overflow")?;
        validate_instant(instant)?;
        Ok(instant)
    }
    pub fn duration_seconds(&self) -> i64 {
        (self.end - self.start).num_seconds()
    }
    pub fn offset_seconds(&self, instant: DateTime<Utc>) -> f64 {
        ((instant - self.start).num_milliseconds() as f64 / 1000.)
            .clamp(0., self.duration_seconds() as f64)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZonePresentation {
    pub id: String,
    pub name: String,
    pub date_text: String,
    pub time_text: String,
    pub zone_text: String,
    pub day_difference: i64,
    pub quality: Quality,
    pub is_reference: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneOption {
    pub id: String,
    pub name: String,
    pub region: String,
    pub subtitle: String,
}
pub fn zone_option(id: &str) -> ZoneOption {
    let mut components = id.rsplitn(2, '/');
    let fallback = components.next().unwrap_or(id).replace('_', " ");
    let region = components
        .next()
        .unwrap_or("")
        .replace('/', " / ")
        .replace('_', " ");
    let name = match id {
        "Asia/Kolkata" => "India".into(),
        _ => fallback,
    };
    let subtitle = if region.is_empty() {
        id.into()
    } else {
        format!("{region} - {id}")
    };
    ZoneOption {
        id: id.into(),
        name,
        region,
        subtitle,
    }
}
fn aliases(id: &str) -> &'static str {
    match id {
        "America/Los_Angeles" => "san francisco bay area pacific time pst pdt",
        "America/New_York" => "eastern time est edt",
        "America/Chicago" => "central time cst cdt",
        "America/Denver" => "mountain time mst mdt",
        "Asia/Kolkata" => "bengaluru bangalore mumbai delhi india ist",
        "Asia/Shanghai" => "beijing china cst",
        "Europe/London" => "uk britain england gmt bst",
        "UTC" => "gmt universal coordinated",
        _ => "",
    }
}
pub fn search_options(query: &str, excluded: &[String]) -> Vec<ZoneOption> {
    let query = query.to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    if words.is_empty() {
        return [
            "UTC",
            "America/Los_Angeles",
            "America/New_York",
            "Europe/London",
            "Europe/Berlin",
            "Asia/Singapore",
            "Asia/Tokyo",
            "Asia/Kolkata",
            "Australia/Sydney",
        ]
        .iter()
        .filter(|id| !excluded.iter().any(|x| x == **id))
        .take(10)
        .map(|id| zone_option(id))
        .collect();
    }
    let mut matches: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .filter(|z| !excluded.iter().any(|id| id == z.name()))
        .map(|z| zone_option(z.name()))
        .filter(|option| {
            let terms = format!(
                "{} {} {} {}",
                option.name,
                option.region,
                option.id,
                aliases(&option.id)
            )
            .to_lowercase();
            words.iter().all(|word| terms.contains(word))
        })
        .collect();
    matches.sort_by_cached_key(|z| (z.name.to_lowercase(), z.id.clone()));
    matches.truncate(10);
    matches
}
pub fn normalized_zones(ids: &[String], local_zone: &str) -> Vec<Tz> {
    let mut zones = Vec::new();
    for id in ids {
        if let Ok(zone) = id.parse::<Tz>()
            && !zones.contains(&zone)
        {
            zones.push(zone);
        }
    }
    if zones.is_empty() {
        if let Ok(zone) = local_zone.parse::<Tz>() {
            zones.push(zone);
        }
        if !zones.contains(&chrono_tz::UTC) {
            zones.push(chrono_tz::UTC);
        }
    }
    zones
}
pub fn current_time() -> DateTime<Utc> {
    Utc::now()
}
fn validate_instant(instant: DateTime<Utc>) -> Result<(), String> {
    if !(1..=9999).contains(&instant.year()) {
        return Err("Date must be within years 1–9999.".into());
    }
    Ok(())
}
fn resolve_local_unbounded(zone: Tz, local: NaiveDateTime) -> Result<DateTime<Utc>, String> {
    // Ordinary DST gaps and historical skipped days, without guessing an offset.
    for minute in 0..=1440 {
        let base = if minute == 0 {
            local
        } else {
            local
                .with_second(0)
                .and_then(|value| value.with_nanosecond(0))
                .ok_or("Date overflow")?
        };
        let next = base
            .checked_add_signed(chrono::Duration::minutes(minute))
            .ok_or("Date overflow")?;
        let first = match zone.from_local_datetime(&next) {
            LocalResult::Single(dt) => Some(dt),
            LocalResult::Ambiguous(a, b) => Some(a.min(b)),
            LocalResult::None => None,
        };
        if let Some(first) = first {
            if minute == 0 {
                return Ok(first.with_timezone(&Utc));
            }
            // IANA historical transitions need not land on minute boundaries.
            // Refine the one-minute crossing interval to its first valid second.
            for seconds_back in (1..60).rev() {
                let candidate = next
                    .checked_sub_signed(chrono::Duration::seconds(seconds_back))
                    .ok_or("Date overflow")?;
                match zone.from_local_datetime(&candidate) {
                    LocalResult::Single(dt) => return Ok(dt.with_timezone(&Utc)),
                    LocalResult::Ambiguous(a, b) => return Ok(a.min(b).with_timezone(&Utc)),
                    LocalResult::None => {}
                }
            }
            return Ok(first.with_timezone(&Utc));
        }
    }
    Err("Unable to resolve local time across a timezone transition.".into())
}
pub fn resolve_local(zone: Tz, local: NaiveDateTime) -> Result<DateTime<Utc>, String> {
    let instant = resolve_local_unbounded(zone, local)?;
    validate_instant(instant)?;
    Ok(instant)
}
pub fn parse_instant(input: &str) -> Result<DateTime<Utc>, String> {
    let input = input.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        let dt = dt.with_timezone(&Utc);
        if !(1..=9999).contains(&dt.year()) {
            return Err("Date must be within years 1–9999.".into());
        }
        return Ok(dt);
    }
    if let Ok(value) = input.parse::<i64>() {
        let dt = if input.trim_start_matches('-').len() > 10 {
            DateTime::from_timestamp_millis(value)
        } else {
            DateTime::from_timestamp(value, 0)
        };
        return dt
            .filter(|dt| (1..=9999).contains(&dt.year()))
            .ok_or("Timestamp is outside supported years 1–9999.".into());
    }
    Err("Use ISO 8601 with a timezone, Unix seconds, or Unix milliseconds.".into())
}
pub fn search_zones(query: &str) -> Vec<String> {
    let query = query.to_lowercase().replace(' ', "_");
    chrono_tz::TZ_VARIANTS
        .iter()
        .filter(|z| z.name().to_lowercase().contains(&query))
        .take(100)
        .map(ToString::to_string)
        .collect()
}
#[cfg(test)]
mod tests;
