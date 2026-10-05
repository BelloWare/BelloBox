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

fn planner(at: &str, ids: &[&str], reference: &str) -> Planner {
    Planner::from_preferences(
        &ids.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        reference,
        "UTC",
        parse_instant(at).unwrap(),
        Some(parse_instant(at).unwrap()),
    )
    .unwrap()
}

#[test]
fn source_quality_boundaries_and_combined() {
    for (time, quality) in [
        ("06:59", Quality::Poor),
        ("07:00", Quality::Extended),
        ("08:59", Quality::Extended),
        ("09:00", Quality::Working),
        ("16:59", Quality::Working),
        ("17:00", Quality::Extended),
        ("20:59", Quality::Extended),
        ("21:00", Quality::Poor),
    ] {
        assert_eq!(
            Quality::at(
                parse_instant(&format!("2026-08-21T{time}:00+08:00")).unwrap(),
                chrono_tz::Asia::Singapore
            ),
            quality
        );
    }
    let zones = [chrono_tz::Asia::Singapore, chrono_tz::Europe::London];
    assert_eq!(
        Quality::combined(parse_instant("2026-08-21T08:00:00Z").unwrap(), &zones),
        Quality::Working
    );
    assert_eq!(
        Quality::combined(parse_instant("2026-08-21T09:00:00Z").unwrap(), &zones),
        Quality::Extended
    );
    assert_eq!(
        Quality::combined(parse_instant("2026-08-21T20:00:00Z").unwrap(), &zones),
        Quality::Poor
    );
    assert_eq!(Quality::combined(Utc::now(), &[]), Quality::Poor);
}

#[test]
fn fresh_defaults_preserve_explicit_utc_and_saved_order() {
    let now = parse_instant("2026-10-05T12:00:00Z").unwrap();
    let fresh = Planner::from_preferences(&[], "", "Asia/Tokyo", now, None).unwrap();
    assert_eq!(fresh.zones, vec![chrono_tz::Asia::Tokyo, chrono_tz::UTC]);
    assert_eq!(fresh.reference, chrono_tz::Asia::Tokyo);
    assert!(fresh.follows_now);
    let explicit =
        Planner::from_preferences(&["UTC".into()], "UTC", "Asia/Tokyo", now, None).unwrap();
    assert_eq!(explicit.zones, vec![chrono_tz::UTC]);
    let p = Planner::from_preferences(
        &[
            "Asia/Singapore".into(),
            "bad/zone".into(),
            "Asia/Singapore".into(),
            "Europe/London".into(),
        ],
        "bad/zone",
        "UTC",
        now,
        None,
    )
    .unwrap();
    assert_eq!(
        p.zones,
        vec![chrono_tz::Asia::Singapore, chrono_tz::Europe::London]
    );
    assert_eq!(p.reference, chrono_tz::Asia::Singapore);
    assert_eq!(normalized_zones(&[], "invalid"), vec![chrono_tz::UTC]);
}

#[test]
fn catalog_aliases_multiword_exclusion_and_suggestions() {
    assert_eq!(
        search_options(" San Francisco ", &[])[0].id,
        "America/Los_Angeles"
    );
    assert_eq!(search_options("bengaluru", &[])[0].name, "India");
    assert_eq!(search_options("london uk", &[])[0].id, "Europe/London");
    assert!(search_options("San Francisco", &["America/Los_Angeles".into()]).is_empty());
    assert_eq!(search_options("", &[])[0].id, "UTC");
    assert_eq!(
        search_options("", &["UTC".into()])[0].id,
        "America/Los_Angeles"
    );
    assert!(search_options("e", &[]).len() <= 10);
    assert!(search_options("does-not-exist", &[]).is_empty());
}

#[test]
fn timeline_dst_days_and_quality_band_midpoints() {
    for (at, hours, count) in [
        ("2024-03-10T12:00:00-07:00", 23, 92),
        ("2024-11-03T12:00:00-08:00", 25, 100),
        ("2024-05-03T12:00:00-07:00", 24, 96),
    ] {
        let p = planner(at, &["America/Los_Angeles"], "America/Los_Angeles");
        let t = p.timeline().unwrap();
        assert_eq!(t.duration_seconds(), hours * 3600);
        assert_eq!(t.start.with_timezone(&p.reference).hour(), 0);
        let bands = p.timeline_qualities().unwrap();
        assert_eq!(bands.len(), count);
        for (i, band) in bands.into_iter().enumerate() {
            assert_eq!(
                band,
                Quality::combined(
                    t.start + chrono::Duration::seconds(i as i64 * 900 + 450),
                    &p.zones
                )
            );
        }
    }
}

#[test]
fn local_edits_handle_gaps_folds_and_day_preservation() {
    let mut p = planner(
        "2024-03-09T02:30:00-08:00",
        &["America/Los_Angeles"],
        "America/Los_Angeles",
    );
    p.move_days(1).unwrap();
    assert_eq!(p.time_input(), "03:00");
    p.select_time("02:30").unwrap();
    assert_eq!(p.time_input(), "03:00");
    p.select_date("2024-11-03").unwrap();
    p.select_time("01:30").unwrap();
    assert_eq!(
        p.instant,
        parse_instant("2024-11-03T01:30:00-07:00").unwrap()
    );
    p.select_date("2024-11-04").unwrap();
    assert_eq!(p.time_input(), "01:30");
    p.select_time("16:30").unwrap();
    let before = p.instant;
    p.select_date("2024-11-03").unwrap();
    assert_eq!(p.time_input(), "16:30");
    assert_eq!((before - p.instant).num_hours(), 24);
}

#[test]
fn edits_are_atomic_for_bad_dates_times_and_overflow() {
    let mut p = planner("2026-10-05T12:00:00Z", &["UTC"], "UTC");
    p.follows_now = true;
    let before = p.instant;
    for date in ["2026-02-30", "2026-1-01", "", "0000-01-01", "10000-01-01"] {
        assert!(p.select_date(date).is_err(), "{date}");
        assert_eq!(p.instant, before);
        assert!(p.follows_now);
    }
    for time in ["24:00", "9:00", "12:60", "12:", "12:00:30"] {
        assert!(p.select_time(time).is_err(), "{time}");
        assert_eq!(p.instant, before);
    }
    assert!(p.nudge_minutes(i64::MAX).is_err());
    assert!(p.nudge_steps(i64::MAX).is_err());
    assert!(p.move_days(i64::MIN).is_err());
    assert!(p.set_offset(f64::NAN).is_err());
    assert_eq!(p.instant, before);
    assert!(p.follows_now);
}

#[test]
fn live_planning_now_and_midnight() {
    let now = parse_instant("2026-10-05T23:59:00Z").unwrap();
    let mut p = Planner::from_preferences(&[], "", "UTC", now, None).unwrap();
    assert!(!p.refresh_now(now + chrono::Duration::seconds(30)).unwrap());
    let midnight = now + chrono::Duration::minutes(1);
    assert!(p.refresh_now(midnight).unwrap());
    assert_eq!(p.timeline().unwrap().start, midnight);
    p.nudge_steps(-1).unwrap();
    let frozen = p.instant;
    assert!(!p.follows_now);
    assert!(!p.refresh_now(midnight).unwrap());
    assert_eq!(p.instant, frozen);
    p.go_to_now(midnight).unwrap();
    assert!(p.follows_now);
    assert_eq!(p.instant, midnight);
}

#[test]
fn captured_timeline_drag_is_continuous_and_stable_at_endpoint() {
    let mut p = planner("2026-10-05T12:00:00Z", &["UTC"], "UTC");
    let t = p.timeline().unwrap();
    p.set_instant(t.instant_at_offset(123.5).unwrap()).unwrap();
    assert_eq!(t.offset_seconds(p.instant), 123.5);
    for _ in 0..4 {
        p.set_instant(t.instant_at_offset(86400.).unwrap()).unwrap();
    }
    assert_eq!(p.instant, t.end);
    p.nudge_steps(1).unwrap();
    assert_eq!(p.time_input(), "00:15");
    assert_eq!(t.instant_at_offset(-1.).unwrap(), t.start);
    assert!(t.instant_at_offset(f64::INFINITY).is_err());
}

#[test]
fn location_actions_preserve_instant_and_guard_last_location() {
    let mut p = planner("2026-10-05T12:00:00Z", &["UTC"], "UTC");
    let before = p.instant;
    assert!(p.remove_zone("UTC").is_err());
    assert!(!p.remove_zone("missing").unwrap());
    assert!(p.add_zone("bad/zone").is_err());
    assert!(p.set_reference("Asia/Tokyo").is_err());
    assert!(p.add_zone("Asia/Tokyo").unwrap());
    assert!(!p.add_zone("Asia/Tokyo").unwrap());
    p.set_reference("Asia/Tokyo").unwrap();
    assert_eq!(p.instant, before);
    p.remove_zone("Asia/Tokyo").unwrap();
    assert_eq!(p.reference, chrono_tz::UTC);
    assert_eq!(p.instant, before);
}

#[test]
fn india_offset_dates_and_exact_copy_payload() {
    let p = planner(
        "2026-10-05T23:00:00Z",
        &["UTC", "Asia/Kolkata", "America/Los_Angeles"],
        "UTC",
    );
    let rows = p.presentations();
    assert_eq!(rows[1].name, "India");
    assert_eq!(rows[1].time_text, "04:30");
    assert_eq!(rows[1].day_difference, 1);
    assert!(rows[1].zone_text.ends_with("UTC+05:30"));
    assert_eq!(rows[2].day_difference, 0);
    assert_eq!(
        p.meeting_summary(),
        "Meeting time — Monday, October 5, 2026 at 23:00 (UTC)\nUTC: Mon, Oct 5, 23:00 (UTC - UTC+00:00)\nIndia: Tue, Oct 6, 04:30 (IST - UTC+05:30)\nLos Angeles: Mon, Oct 5, 16:00 (PDT - UTC-07:00)"
    );
    let q = planner(
        "2026-10-05T01:00:00Z",
        &["UTC", "America/Los_Angeles"],
        "UTC",
    );
    assert_eq!(q.presentations()[1].day_difference, -1);
}

#[test]
fn upper_year_boundary_rejects_next_day_atomically() {
    let mut p = planner("9999-12-31T23:59:00Z", &["UTC"], "UTC");
    assert_eq!(p.timeline().unwrap().duration_seconds(), 86400);
    let before = p.instant;
    assert!(p.move_days(1).is_err());
    assert!(p.nudge_steps(1).is_err());
    assert!(p.set_offset(86400.).is_err());
    assert_eq!(p.instant, before);
}

#[test]
fn gaps_drop_nonexistent_seconds_and_midnight_transitions_start_at_first_time() {
    let zone = chrono_tz::America::New_York;
    let local = NaiveDateTime::parse_from_str("2026-03-08 02:30:45", "%F %T").unwrap();
    assert_eq!(
        resolve_local(zone, local).unwrap(),
        parse_instant("2026-03-08T03:00:00-04:00").unwrap()
    );
    let p = planner(
        "2018-11-04T12:00:00-02:00",
        &["America/Sao_Paulo"],
        "America/Sao_Paulo",
    );
    let timeline = p.timeline().unwrap();
    assert_eq!(timeline.start.with_timezone(&p.reference).hour(), 1);
    assert_eq!(timeline.duration_seconds(), 23 * 3600);
}

#[test]
fn live_reference_changes_remain_live_and_invalid_seed_does_not_construct() {
    let now = parse_instant("2026-10-05T12:00:00Z").unwrap();
    let mut p = Planner::from_preferences(
        &["UTC".into(), "Asia/Tokyo".into()],
        "UTC",
        "UTC",
        now,
        None,
    )
    .unwrap();
    p.set_reference("Asia/Tokyo").unwrap();
    assert!(p.follows_now);
    assert_eq!(p.instant, now);
    let invalid = DateTime::from_timestamp(253_402_300_800, 0).unwrap();
    assert!(Planner::from_preferences(&[], "", "UTC", now, Some(invalid)).is_err());
    assert!(p.go_to_now(invalid).is_err());
    assert_eq!(p.instant, now);
}

#[test]
fn skipped_civil_day_preserves_wall_time_in_both_directions() {
    let mut p = planner(
        "2011-12-29T12:34:56-10:00",
        &["Pacific/Apia"],
        "Pacific/Apia",
    );
    p.move_days(1).unwrap();
    assert_eq!(
        p.instant,
        parse_instant("2011-12-31T12:34:56+14:00").unwrap()
    );
    p.move_days(-1).unwrap();
    assert_eq!(
        p.instant,
        parse_instant("2011-12-29T12:34:56-10:00").unwrap()
    );
    p.select_date("2011-12-30").unwrap();
    assert_eq!(
        p.instant,
        parse_instant("2011-12-31T12:34:56+14:00").unwrap()
    );
}

#[test]
fn historical_second_precision_gap_starts_at_exact_first_valid_time() {
    let zone = chrono_tz::Africa::Monrovia;
    let midnight = NaiveDateTime::parse_from_str("1972-01-07 00:00:00", "%F %T").unwrap();
    let first = parse_instant("1972-01-07T00:44:30Z").unwrap();
    assert_eq!(resolve_local(zone, midnight).unwrap(), first);
    let timeline =
        Timeline::containing(parse_instant("1972-01-07T12:00:00Z").unwrap(), zone).unwrap();
    assert_eq!(timeline.start, first);
    assert_eq!(timeline.duration_seconds(), 86400 - 44 * 60 - 30);
}

#[test]
fn india_search_preserves_source_alphabetic_region_matches() {
    let matches = search_options("India", &[]);
    assert_eq!(matches.first().unwrap().id, "Indian/Antananarivo");
    assert!(matches.iter().any(|option| option.id == "Asia/Kolkata"));
}

#[test]
fn lower_year_boundary_validates_final_instant_not_intermediate_midnight() {
    let original = planner("0001-01-02T12:00:00Z", &["Asia/Tokyo"], "Asia/Tokyo");
    let expected = parse_instant("0001-01-01T12:00:00Z").unwrap();
    let mut date_edit = original.clone();
    date_edit.select_date("0001-01-01").unwrap();
    assert_eq!(date_edit.instant, expected);
    let mut day_move = original;
    day_move.move_days(-1).unwrap();
    assert_eq!(day_move.instant, expected);
    assert!(day_move.move_days(-1).is_err());
    assert_eq!(day_move.instant, expected);
}
