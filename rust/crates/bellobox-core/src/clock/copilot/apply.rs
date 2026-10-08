//! Explicit, transactional suggestion application. Preview is always read-only.
use super::{Parts, Plan, Suggestion};
use crate::clock::{Planner, validate_instant};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signature {
    instant: DateTime<Utc>,
    zones: Vec<Tz>,
    reference: Tz,
    follows_now: bool,
}
impl From<&Planner> for Signature {
    fn from(p: &Planner) -> Self {
        Self {
            instant: p.instant,
            zones: p.zones.clone(),
            reference: p.reference,
            follows_now: p.follows_now,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PreparedApply {
    pub plan: Plan,
    pub issue: Option<String>,
    original_plan: Plan,
    base: Signature,
    staged: Planner,
}
impl PreparedApply {
    /// Rejects stale previews rather than overwriting intervening planner edits.
    pub fn commit(self, planner: &mut Planner) -> Result<Parts, String> {
        if Signature::from(&*planner) != self.base {
            return Err(
                "The planner changed. Review the updated suggestion before applying it.".into(),
            );
        }
        if self.plan != self.original_plan {
            return Err("The prepared suggestion changed. Review it again before applying.".into());
        }
        validate(&self.staged)?;
        *planner = self.staged;
        Ok(self.plan.parts)
    }
}
/// Recalculates applicable parts against the current planner. Applied-part history
/// informs host status only; an intervening edit can make a part actionable again. Location overflow
/// removes the entire location part, never silently truncating the user's list.
/// A valid time part can still be offered independently with a visible warning.
pub fn prepare(
    planner: &Planner,
    suggestion: &Suggestion,
    _already_applied: Parts,
    allow_time: bool,
    allow_locations: bool,
) -> Result<Option<PreparedApply>, String> {
    validate(planner)?;
    let ids: Vec<String> = planner.zones.iter().map(|z| z.name().into()).collect();
    let time = allow_time;
    let locations = allow_locations;
    let make = |locations| {
        suggestion.plan(
            &ids,
            planner.reference.name(),
            planner.instant,
            time,
            locations,
            |instant| instant.to_rfc3339(),
        )
    };
    let Some(mut plan) = make(locations) else {
        return Ok(None);
    };
    let mut issue = None;
    if plan
        .zone_ids
        .as_ref()
        .is_some_and(|zones| zones.is_empty() || zones.len() > 24)
    {
        let warning = "The location proposal exceeds the 24-zone limit. Remove locations or ask for a replacement list.".to_string();
        let Some(time_only) = make(false) else {
            return Err(warning);
        };
        plan = time_only;
        issue = Some(warning);
    }
    let mut staged = planner.clone();
    // Set the final state on the clone, then validate it as one transaction.
    // No intermediate reference/day combination can mutate the live planner.
    if let Some(ids) = &plan.zone_ids {
        staged.set_zones(ids)?;
    }
    if let Some(anchor) = &plan.anchor_zone_id {
        staged.reference = anchor
            .parse()
            .map_err(|_| "Unknown IANA reference timezone.")?;
    }
    if let Some(instant) = plan.instant {
        staged.instant = instant;
        staged.follows_now = false;
    }
    validate(&staged)?;
    Ok(Some(PreparedApply {
        original_plan: plan.clone(),
        plan,
        issue,
        base: Signature::from(planner),
        staged,
    }))
}
fn validate(planner: &Planner) -> Result<(), String> {
    if planner.zones.is_empty() || planner.zones.len() > 24 {
        return Err("Choose 1–24 zones.".into());
    }
    if !planner.zones.contains(&planner.reference) {
        return Err("Reference must be one of your locations.".into());
    }
    validate_instant(planner.instant)?;
    planner.timeline()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};
    fn planner() -> Planner {
        Planner {
            instant: Utc.with_ymd_and_hms(2026, 10, 8, 12, 0, 0).unwrap(),
            zones: vec![chrono_tz::UTC],
            reference: chrono_tz::UTC,
            follows_now: true,
        }
    }
    fn mixed(p: &Planner) -> Suggestion {
        Suggestion {
            instant: Some(p.instant + Duration::hours(1)),
            zone_ids: vec!["Asia/Tokyo".into()],
            ..Default::default()
        }
    }
    #[test]
    fn preview_is_read_only_and_commit_adopts_both_parts() {
        let mut p = planner();
        let before = Signature::from(&p);
        let prepared = prepare(&p, &mixed(&p), Parts::empty(), true, true)
            .unwrap()
            .unwrap();
        assert_eq!(Signature::from(&p), before);
        assert_eq!(
            prepared.commit(&mut p).unwrap(),
            Parts::TIME | Parts::LOCATIONS
        );
        assert_eq!(p.instant, before.instant + Duration::hours(1));
        assert_eq!(p.zones.len(), 2);
        assert!(!p.follows_now);
    }
    #[test]
    fn every_current_planner_field_fences_stale_apply() {
        for change in 0..4 {
            let mut p = planner();
            p.zones.push(chrono_tz::Asia::Tokyo);
            let prepared = prepare(&p, &mixed(&p), Parts::empty(), true, true)
                .unwrap()
                .unwrap();
            match change {
                0 => p.instant += Duration::seconds(1),
                1 => p.zones.reverse(),
                2 => p.reference = chrono_tz::Asia::Tokyo,
                _ => p.follows_now = false,
            }
            let changed = Signature::from(&p);
            assert!(prepared.commit(&mut p).is_err());
            assert_eq!(Signature::from(&p), changed);
        }
    }
    #[test]
    fn remaining_parts_allow_location_after_time_only_apply() {
        let mut p = planner();
        let suggestion = mixed(&p);
        let first = prepare(&p, &suggestion, Parts::empty(), true, false)
            .unwrap()
            .unwrap();
        assert_eq!(first.commit(&mut p).unwrap(), Parts::TIME);
        let second = prepare(&p, &suggestion, Parts::TIME, true, true)
            .unwrap()
            .unwrap();
        assert_eq!(second.commit(&mut p).unwrap(), Parts::LOCATIONS);
        assert!(
            prepare(&p, &suggestion, Parts::TIME | Parts::LOCATIONS, true, true)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn current_applicability_wins_over_old_applied_markers() {
        let mut p = planner();
        let suggestion = mixed(&p);
        prepare(&p, &suggestion, Parts::empty(), true, true)
            .unwrap()
            .unwrap()
            .commit(&mut p)
            .unwrap();
        p.instant += Duration::hours(2);
        let prepared = prepare(&p, &suggestion, Parts::TIME | Parts::LOCATIONS, true, true)
            .unwrap()
            .unwrap();
        assert_eq!(prepared.plan.parts, Parts::TIME);
        prepared.commit(&mut p).unwrap();
        assert_eq!(p.instant, suggestion.instant.unwrap());
    }
    #[test]
    fn overflow_preserves_locations_but_offers_valid_time() {
        let mut p = planner();
        p.zones = chrono_tz::TZ_VARIANTS
            .iter()
            .copied()
            .filter(|z| *z != chrono_tz::Asia::Tokyo)
            .take(24)
            .collect();
        p.reference = p.zones[0];
        let original_zones = p.zones.clone();
        let suggestion = mixed(&p);
        let prepared = prepare(&p, &suggestion, Parts::empty(), true, true)
            .unwrap()
            .unwrap();
        assert!(prepared.issue.is_some());
        assert_eq!(prepared.plan.parts, Parts::TIME);
        prepared.commit(&mut p).unwrap();
        assert_eq!(p.zones, original_zones);
        let location_only = Suggestion {
            instant: None,
            ..suggestion
        };
        assert!(prepare(&p, &location_only, Parts::empty(), true, true).is_err());
    }
    #[test]
    fn invalid_instant_and_tampered_plan_never_mutate() {
        let mut p = planner();
        let before = Signature::from(&p);
        let bad = Suggestion {
            instant: Some(Utc.with_ymd_and_hms(10000, 1, 1, 0, 0, 0).unwrap()),
            ..mixed(&p)
        };
        assert!(prepare(&p, &bad, Parts::empty(), true, true).is_err());
        assert_eq!(Signature::from(&p), before);
        let mut prepared = prepare(&p, &mixed(&p), Parts::empty(), true, true)
            .unwrap()
            .unwrap();
        prepared.plan.instant = None;
        assert!(prepared.commit(&mut p).is_err());
        assert_eq!(Signature::from(&p), before);
    }
    #[test]
    fn replacement_selects_valid_reference_and_noop_has_no_apply() {
        let mut p = planner();
        let s = Suggestion {
            zone_ids: vec!["Asia/Tokyo".into(), "Europe/London".into()],
            replaces_locations: true,
            anchor_zone_id: Some("Europe/London".into()),
            ..Default::default()
        };
        prepare(&p, &s, Parts::empty(), true, true)
            .unwrap()
            .unwrap()
            .commit(&mut p)
            .unwrap();
        assert_eq!(p.reference, chrono_tz::Europe::London);
        assert!(
            prepare(&p, &s, Parts::empty(), true, true)
                .unwrap()
                .is_none()
        );
    }
}
