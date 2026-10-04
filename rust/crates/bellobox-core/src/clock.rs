use chrono::{DateTime, Datelike, Days, LocalResult, NaiveDateTime, TimeZone, Timelike, Utc};
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
    pub fn set_instant(&mut self, instant: DateTime<Utc>) -> Result<(), String> {
        if !(1..=9999).contains(&instant.year()) {
            return Err("Date must be within years 1–9999.".into());
        }
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
            .checked_add_signed(chrono::Duration::minutes(minutes))
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
        let target = date.and_time(local.time());
        let instant = resolve_local(self.reference, target)?;
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
pub fn resolve_local(zone: Tz, local: NaiveDateTime) -> Result<DateTime<Utc>, String> {
    for minute in 0..=180 {
        let next = local
            .checked_add_signed(chrono::Duration::minutes(minute))
            .ok_or("Date overflow")?;
        match zone.from_local_datetime(&next) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
                if !(1..=9999).contains(&dt.year()) {
                    return Err("Date must be within years 1–9999.".into());
                }
                return Ok(dt.with_timezone(&Utc));
            }
            LocalResult::None => {}
        }
    }
    Err("Unable to resolve local time across a timezone transition.".into())
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
mod tests {
    use super::*;
    #[test]
    fn unix_precision() {
        assert_eq!(
            parse_instant("1700000000").unwrap(),
            parse_instant("1700000000000").unwrap()
        );
        assert!(parse_instant("today").is_err());
        assert!(parse_instant("0000-01-01T00:00:00Z").is_err());
    }
    #[test]
    fn calendar_day_respects_dst() {
        let mut p = Planner {
            instant: parse_instant("2026-03-07T12:00:00-05:00").unwrap(),
            reference: chrono_tz::America::New_York,
            ..Default::default()
        };
        let before = p.instant;
        p.move_days(1).unwrap();
        assert_eq!((p.instant - before).num_hours(), 23);
        assert_eq!(p.instant.with_timezone(&p.reference).hour(), 12);
    }
    #[test]
    fn gap_resolves_and_duplicate_zones_removed() {
        let zone = chrono_tz::America::New_York;
        let dt = resolve_local(
            zone,
            NaiveDateTime::parse_from_str("2026-03-08 02:30:00", "%F %T").unwrap(),
        )
        .unwrap();
        assert_eq!(dt.with_timezone(&zone).hour(), 3);
        let mut p = Planner::default();
        p.set_zones(&["UTC".into(), "UTC".into()]).unwrap();
        assert_eq!(p.zones.len(), 1);
    }
    #[test]
    fn working_hours() {
        assert_eq!(
            Quality::at(
                parse_instant("2026-01-01T09:00:00Z").unwrap(),
                chrono_tz::UTC
            ),
            Quality::Working
        );
    }
}
