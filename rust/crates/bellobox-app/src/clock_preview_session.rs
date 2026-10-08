//! Ephemeral launcher clock state. Constructing, editing, and handing off a
//! preview never writes preferences or replaces the full window's locations.

use bellobox_core::clock::{self, Planner, Quality, Timeline};

const MAX_SEED_BYTES: usize = 256;
const MAX_PREVIEW_ZONES: usize = 4;

#[derive(Debug, Clone)]
pub struct ClockPreviewSession {
    pub planner: Planner,
    /// Kept independently of the instant so dragging to the next-midnight
    /// endpoint does not move the scrubber's origin on the next pointer event.
    pub displayed_day: Timeline,
    seed: Option<Planner>,
}

impl ClockPreviewSession {
    pub fn new(saved: &[String], anchor: &str, local: &str, input: &str) -> Result<Self, String> {
        let ids = preview_zone_ids(saved, local);
        // Gate before trimming or parsing: arbitrary launcher selections remain
        // a live clock, including selections too large for a timestamp preview.
        let instant = (input.len() <= MAX_SEED_BYTES)
            .then(|| clock::parse_instant(input).ok())
            .flatten();
        let planner = Planner::from_preferences(
            &ids,
            canonical_zone_id(anchor),
            canonical_zone_id(local),
            clock::current_time(),
            instant,
        )?;
        let displayed_day = planner.timeline()?;
        let seed = instant.map(|_| planner.clone());
        Ok(Self {
            planner,
            displayed_day,
            seed,
        })
    }

    pub fn handoff(&self) -> ClockHandoff {
        ClockHandoff {
            snapshot: self.planner.clone(),
            copilot: Default::default(),
            draft: String::new(),
            retirement: None,
        }
    }

    pub fn is_live(&self) -> bool {
        self.planner.follows_now
    }

    pub fn reference(&self) -> &str {
        self.planner.reference.name()
    }

    pub fn has_seed(&self) -> bool {
        self.seed.is_some()
    }

    pub fn timeline_qualities(&self) -> Result<Vec<Quality>, String> {
        let mut displayed = self.planner.clone();
        displayed.set_instant(
            self.displayed_day
                .instant_at_offset(self.displayed_day.duration_seconds() as f64 / 2.)?,
        )?;
        displayed.timeline_qualities()
    }

    pub fn has_moved_from_seed(&self) -> bool {
        self.seed.as_ref().is_some_and(|seed| {
            (self.planner.instant - seed.instant)
                .num_milliseconds()
                .unsigned_abs()
                >= 60_000
        })
    }

    pub fn return_to_seed(&mut self) -> Result<(), String> {
        let Some(instant) = self.seed.as_ref().map(|seed| seed.instant) else {
            return Ok(());
        };
        // A reset restores the input instant, retaining the chosen reference.
        self.update_planner(|planner| planner.set_instant(instant))
    }

    pub fn go_to_now(&mut self) -> Result<(), String> {
        self.update_planner(|planner| planner.go_to_now(clock::current_time()))
    }

    /// Returns whether a live tick changed the visible minute. Planned previews
    /// remain untouched; the host can skip notification when this returns false.
    pub fn refresh_now(&mut self) -> Result<bool, String> {
        let mut planner = self.planner.clone();
        if !planner.refresh_now(clock::current_time())? {
            return Ok(false);
        }
        self.displayed_day = planner.timeline()?;
        self.planner = planner;
        Ok(true)
    }

    pub fn move_day(&mut self, days: i64) -> Result<(), String> {
        let start = self.displayed_day.start;
        self.update_planner(|planner| {
            // At the endpoint, the selected instant belongs to tomorrow but
            // the arrows still move relative to the day the scrubber displays.
            let mut target = planner.clone();
            target.set_instant(start)?;
            target.move_days(days)?;
            planner.select_date(&target.date_input())
        })
    }

    /// Keyboard and horizontal-wheel steps roll across midnight in either
    /// direction. A pointer drag instead uses the stable, clamped set_offset.
    pub fn nudge(&mut self, steps: i64) -> Result<(), String> {
        self.update_planner(|planner| planner.nudge_steps(steps))
    }

    pub fn set_reference(&mut self, id: &str) -> Result<(), String> {
        let id = canonical_zone_id(id);
        if self.reference() == id {
            return Ok(());
        }
        self.update_planner(|planner| planner.set_reference(id))
    }

    pub fn set_offset(&mut self, seconds: f64) -> Result<(), String> {
        self.planner
            .set_instant(self.displayed_day.instant_at_offset(seconds)?)
    }

    fn update_planner(
        &mut self,
        update: impl FnOnce(&mut Planner) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut planner = self.planner.clone();
        update(&mut planner)?;
        let displayed_day = planner.timeline()?;
        self.planner = planner;
        self.displayed_day = displayed_day;
        Ok(())
    }
}

/// An explicit launcher transfer. A plain window reopen has no handoff and
/// therefore leaves its existing planner alone. Live intent is resolved when
/// applied, rather than freezing the instant when Enter was pressed.
#[derive(Debug, Clone)]
pub struct ClockHandoff {
    snapshot: Planner,
    pub(crate) copilot: bellobox_core::clock::copilot::session::Snapshot,
    pub(crate) draft: String,
    pub(crate) retirement: Option<crate::clock_copilot_worker::RetirementGuard>,
}

impl ClockHandoff {
    pub fn with_retirement_guard(
        mut self,
        guard: Option<crate::clock_copilot_worker::RetirementGuard>,
    ) -> Self {
        self.retirement = guard;
        self
    }
    pub fn with_copilot(
        mut self,
        snapshot: bellobox_core::clock::copilot::session::Snapshot,
        draft: String,
        guard: Option<crate::clock_copilot_worker::RetirementGuard>,
    ) -> Self {
        self.copilot = snapshot;
        self.draft = draft;
        self.retirement = guard;
        self
    }

    pub fn is_live(&self) -> bool {
        self.snapshot.follows_now
    }

    pub fn reference(&self) -> &str {
        self.snapshot.reference.name()
    }

    pub fn apply(&self, planner: &mut Planner) -> Result<(), String> {
        let mut target = planner.clone();
        if self.is_live() {
            target.go_to_now(clock::current_time())?;
        } else {
            target.set_instant(self.snapshot.instant)?;
        }
        // Do not copy the preview's capped list over the full planner. Only its
        // chosen reference may need to join that list, in memory.
        target.add_zone(self.reference())?;
        target.set_reference(self.reference())?;
        *planner = target;
        Ok(())
    }
}

fn canonical_zone_id(id: &str) -> &str {
    match id {
        "GMT" | "Etc/GMT" | "Etc/UTC" | "UTC" => "UTC",
        _ => id,
    }
}

fn preview_zone_ids(saved: &[String], local: &str) -> Vec<String> {
    let candidates: Vec<_> = saved
        .iter()
        .map(String::as_str)
        .chain([local, "UTC"])
        .map(|id| canonical_zone_id(id).to_string())
        .collect();
    clock::normalized_zones(&candidates, "UTC")
        .iter()
        .take(MAX_PREVIEW_ZONES)
        .map(|zone| zone.name().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "2026-10-05T12:34:56.789Z";

    fn strings(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).into()).collect()
    }

    fn session(input: &str) -> ClockPreviewSession {
        ClockPreviewSession::new(&strings(&["UTC", "Asia/Tokyo"]), "UTC", "UTC", input).unwrap()
    }

    fn zone_ids(planner: &Planner) -> Vec<&str> {
        planner.zones.iter().map(|zone| zone.name()).collect()
    }

    #[test]
    fn supported_seeds_are_exact_and_do_not_follow_now() {
        for input in [
            SEED,
            "1791203696",
            "1791203696789",
            "  2026-10-05T12:34:56Z  ",
        ] {
            let mut preview = session(input);
            let instant = clock::parse_instant(input).unwrap();
            assert_eq!(preview.planner.instant, instant);
            assert!(!preview.is_live());
            assert!(preview.has_seed());
            assert!(!preview.has_moved_from_seed());
            assert!(!preview.refresh_now().unwrap());
            assert_eq!(preview.planner.instant, instant);
        }
    }

    #[test]
    fn nonseeds_and_oversized_inputs_are_live_and_have_no_reset() {
        let oversized = format!("{}{}", " ".repeat(MAX_SEED_BYTES), SEED);
        for input in [
            "",
            "ordinary text",
            "tomorrow at noon",
            "2026-10-05",
            &oversized,
        ] {
            let mut preview = session(input);
            assert!(preview.is_live());
            assert!(!preview.has_seed());
            assert!(!preview.has_moved_from_seed());
            preview.nudge(1).unwrap();
            let instant = preview.planner.instant;
            preview.return_to_seed().unwrap();
            assert_eq!(preview.planner.instant, instant);
            assert!(!preview.is_live());
            assert!(!preview.has_moved_from_seed());
        }
    }

    #[test]
    fn seed_bound_checks_original_utf8_bytes_before_parsing() {
        let at_limit = format!("{}{}", " ".repeat(MAX_SEED_BYTES - SEED.len()), SEED);
        assert!(!session(&at_limit).is_live());
        assert!(session(&format!(" {at_limit}")).is_live());
        let unicode_padding = "\u{2003}".repeat(MAX_SEED_BYTES / 3);
        assert!(session(&format!("{unicode_padding}{SEED}")).is_live());
    }

    #[test]
    fn seed_reset_preserves_reference_and_one_minute_threshold() {
        let mut preview = session(SEED);
        let offset = preview
            .displayed_day
            .offset_seconds(preview.planner.instant);
        preview.set_offset(offset + 59.999).unwrap();
        assert!(!preview.has_moved_from_seed());
        preview.set_offset(offset + 60.).unwrap();
        assert!(preview.has_moved_from_seed());
        preview.set_offset(offset - 60.).unwrap();
        assert!(preview.has_moved_from_seed());
        preview.set_reference("Asia/Tokyo").unwrap();
        preview.return_to_seed().unwrap();
        assert_eq!(preview.planner.instant, clock::parse_instant(SEED).unwrap());
        assert_eq!(preview.reference(), "Asia/Tokyo");
        assert!(!preview.has_moved_from_seed());
        assert!(!preview.is_live());
        assert_eq!(preview.displayed_day, preview.planner.timeline().unwrap());
    }

    #[test]
    fn now_and_live_refresh_keep_the_seed_available() {
        let mut preview = session("2000-01-01T12:00:00Z");
        let before = clock::current_time();
        preview.go_to_now().unwrap();
        let after = clock::current_time();
        assert!(preview.is_live());
        assert!(preview.planner.instant >= before && preview.planner.instant <= after);
        assert!(preview.has_moved_from_seed());
        // Simulate a stale live row without sleeping or depending on a minute edge.
        preview.planner.instant = clock::parse_instant("2000-01-01T12:00:00Z").unwrap();
        assert!(preview.refresh_now().unwrap());
        assert!(preview.is_live());
        assert_eq!(preview.displayed_day, preview.planner.timeline().unwrap());
        preview.return_to_seed().unwrap();
        assert_eq!(preview.planner.date_input(), "2000-01-01");
        assert!(!preview.is_live());
    }

    #[test]
    fn preview_keeps_saved_order_then_local_and_utc_with_four_zone_cap() {
        let saved = strings(&["bad/zone", "Asia/Tokyo", "Europe/London", "Asia/Tokyo"]);
        let preview =
            ClockPreviewSession::new(&saved, "Europe/London", "America/New_York", SEED).unwrap();
        assert_eq!(
            zone_ids(&preview.planner),
            ["Asia/Tokyo", "Europe/London", "America/New_York", "UTC",]
        );
        assert_eq!(preview.reference(), "Europe/London");

        let saved = strings(&[
            "Asia/Tokyo",
            "Europe/London",
            "Asia/Kolkata",
            "Europe/Berlin",
            "UTC",
        ]);
        let preview = ClockPreviewSession::new(&saved, "UTC", "America/New_York", SEED).unwrap();
        assert_eq!(
            zone_ids(&preview.planner),
            [
                "Asia/Tokyo",
                "Europe/London",
                "Asia/Kolkata",
                "Europe/Berlin",
            ]
        );
        assert_eq!(
            preview.reference(),
            "Asia/Tokyo",
            "capped-out anchor must fall back"
        );
    }

    #[test]
    fn utc_aliases_canonicalize_before_deduplication_and_capping() {
        let saved = strings(&["GMT", "Etc/GMT", "Etc/UTC", "UTC", "Asia/Tokyo"]);
        let preview = ClockPreviewSession::new(&saved, "Etc/UTC", "Europe/London", SEED).unwrap();
        assert_eq!(
            zone_ids(&preview.planner),
            ["UTC", "Asia/Tokyo", "Europe/London"]
        );
        assert_eq!(preview.reference(), "UTC");
        assert_eq!(preview_zone_ids(&[], "invalid"), ["UTC"]);
        assert_eq!(preview_zone_ids(&[], "Asia/Tokyo"), ["Asia/Tokyo", "UTC"]);
    }

    #[test]
    fn pointer_endpoint_is_stable_but_nudges_roll_the_day() {
        let mut preview = session(SEED);
        let day = preview.displayed_day.clone();
        preview.set_offset(day.duration_seconds() as f64).unwrap();
        preview.set_offset(day.duration_seconds() as f64).unwrap();
        assert_eq!(preview.planner.instant, day.end);
        assert_eq!(preview.displayed_day, day);
        preview.set_reference("UTC").unwrap();
        assert_eq!(preview.displayed_day, day);
        preview.set_offset(23.75 * 3600.).unwrap();
        preview.nudge(1).unwrap();
        assert_eq!(preview.planner.instant, day.end);
        assert_eq!(preview.displayed_day.start, day.end);
        preview.nudge(-1).unwrap();
        assert_eq!(preview.planner.time_input(), "23:45");
        assert_eq!(preview.displayed_day, day);
        preview.set_offset(0.).unwrap();
        preview.nudge(-1).unwrap();
        assert_eq!(preview.planner.date_input(), "2026-10-04");
        assert_eq!(preview.planner.time_input(), "23:45");
    }

    #[test]
    fn day_arrows_use_displayed_day_even_at_endpoint() {
        let mut preview = session(SEED);
        preview
            .set_offset(preview.displayed_day.duration_seconds() as f64)
            .unwrap();
        preview.move_day(1).unwrap();
        assert_eq!(preview.planner.date_input(), "2026-10-06");
        assert_eq!(preview.planner.time_input(), "00:00");
        preview.move_day(-1).unwrap();
        assert_eq!(preview.planner.date_input(), "2026-10-05");
    }

    #[test]
    fn calendar_moves_and_timeline_offsets_respect_dst() {
        let mut preview = ClockPreviewSession::new(
            &strings(&["America/New_York"]),
            "America/New_York",
            "UTC",
            "2026-03-07T17:00:00Z",
        )
        .unwrap();
        preview.move_day(1).unwrap();
        assert_eq!(preview.planner.time_input(), "12:00");
        assert_eq!(preview.displayed_day.duration_seconds(), 23 * 3600);
        assert_eq!(
            preview.planner.instant,
            clock::parse_instant("2026-03-08T16:00:00Z").unwrap()
        );
        preview.set_offset(23. * 3600.).unwrap();
        assert_eq!(preview.planner.time_input(), "00:00");
        assert_eq!(preview.planner.date_input(), "2026-03-09");
        assert_eq!(preview.displayed_day.duration_seconds(), 23 * 3600);
        assert_eq!(preview.timeline_qualities().unwrap().len(), 92);
        preview.nudge(1).unwrap();
        assert_eq!(preview.timeline_qualities().unwrap().len(), 96);
    }

    #[test]
    fn rejected_edits_leave_the_whole_preview_unchanged() {
        let mut preview = session(SEED);
        let instant = preview.planner.instant;
        let day = preview.displayed_day.clone();
        for result in [
            preview.nudge(i64::MAX),
            preview.move_day(i64::MIN),
            preview.set_reference("bad/zone"),
            preview.set_reference("Europe/London"),
            preview.set_offset(f64::NAN),
        ] {
            assert!(result.is_err());
        }
        assert_eq!(preview.planner.instant, instant);
        assert_eq!(preview.reference(), "UTC");
        assert_eq!(preview.displayed_day, day);
    }

    #[test]
    fn displayed_day_bands_use_core_sampling_even_at_next_midnight() {
        let mut preview = ClockPreviewSession::new(
            &strings(&["America/New_York", "Asia/Tokyo"]),
            "America/New_York",
            "UTC",
            "2026-11-01T17:00:00Z",
        )
        .unwrap();
        let expected = preview.planner.timeline_qualities().unwrap();
        assert_eq!(expected.len(), 100);
        preview
            .set_offset(preview.displayed_day.duration_seconds() as f64)
            .unwrap();
        assert_eq!(preview.timeline_qualities().unwrap(), expected);
        assert_eq!(preview.planner.timeline_qualities().unwrap().len(), 96);
    }

    #[test]
    fn handoff_freezes_the_exact_plan_and_preserves_full_window_zones() {
        let mut preview = session(SEED);
        preview.nudge(1).unwrap();
        preview.set_reference("Asia/Tokyo").unwrap();
        let handoff = preview.handoff();
        let selected = preview.planner.instant;
        preview.nudge(5).unwrap();
        let mut full = Planner::from_preferences(
            &strings(&[
                "Europe/London",
                "UTC",
                "America/New_York",
                "Europe/Berlin",
                "Asia/Kolkata",
            ]),
            "Europe/London",
            "UTC",
            clock::current_time(),
            None,
        )
        .unwrap();
        handoff.apply(&mut full).unwrap();
        assert_eq!(full.instant, selected);
        assert!(!full.follows_now);
        assert_eq!(full.reference.name(), "Asia/Tokyo");
        assert_eq!(
            zone_ids(&full),
            [
                "Europe/London",
                "UTC",
                "America/New_York",
                "Europe/Berlin",
                "Asia/Kolkata",
                "Asia/Tokyo",
            ]
        );
        handoff.apply(&mut full).unwrap();
        assert_eq!(
            full.zones.len(),
            6,
            "repeated adoption does not duplicate a reference"
        );
    }

    #[test]
    fn live_handoff_takes_over_an_existing_plan_at_current_now() {
        let mut preview = session("");
        preview.set_reference("Asia/Tokyo").unwrap();
        preview.planner.instant = clock::parse_instant("2000-01-01T00:00:00Z").unwrap();
        let handoff = preview.handoff();
        assert!(handoff.is_live());
        let mut full = session(SEED).planner;
        let zones = full.zones.clone();
        let before = clock::current_time();
        handoff.apply(&mut full).unwrap();
        let after = clock::current_time();
        assert!(full.follows_now);
        assert!(full.instant >= before && full.instant <= after);
        assert_eq!(full.reference.name(), "Asia/Tokyo");
        assert_eq!(full.zones, zones);
    }

    #[test]
    fn handoff_adds_only_missing_reference_not_other_preview_locations() {
        let mut preview = session(SEED);
        preview.set_reference("Asia/Tokyo").unwrap();
        let mut full = Planner::from_preferences(
            &strings(&["Europe/London"]),
            "Europe/London",
            "UTC",
            clock::current_time(),
            None,
        )
        .unwrap();
        preview.handoff().apply(&mut full).unwrap();
        assert_eq!(zone_ids(&full), ["Europe/London", "Asia/Tokyo"]);
    }
}
