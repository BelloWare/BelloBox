use super::*;

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn now() -> DateTime<Utc> {
    super::super::parse_instant("2026-08-21T12:00:00Z").unwrap()
}

fn location_plan(suggestion: &Suggestion, current: &[&str], anchor: &str) -> Option<Plan> {
    suggestion.plan(&ids(current), anchor, now(), false, true, |_| {
        panic!("Location-only planning must not describe an instant")
    })
}

fn mixed_suggestion() -> Suggestion {
    Suggestion {
        instant: Some(now() + Duration::hours(1)),
        zone_ids: ids(&["Asia/Tokyo", "UTC", "Nope/Zone"]),
        replaces_locations: false,
        anchor_zone_id: Some("Asia/Tokyo".into()),
    }
}

#[test]
fn source_mixed_plan_fixture_has_time_locations_reference_summary_order() {
    let suggestion = mixed_suggestion();
    let current = ids(&["UTC"]);
    let original = suggestion.clone();
    let plan = suggestion
        .plan(&current, "UTC", now(), true, true, |instant| {
            assert_eq!(instant, now() + Duration::hours(1));
            "1:00 PM".into()
        })
        .unwrap();
    assert_eq!(
        plan,
        Plan {
            instant: suggestion.instant,
            zone_ids: Some(ids(&["UTC", "Asia/Tokyo"])),
            anchor_zone_id: Some("Asia/Tokyo".into()),
            summary: "Set time to 1:00 PM · Add Tokyo · Reference: Tokyo".into(),
            parts: Parts::TIME | Parts::LOCATIONS,
        }
    );
    assert_eq!(suggestion, original);
    assert_eq!(current, ids(&["UTC"]));
}

#[test]
fn time_change_uses_absolute_inclusive_minute_boundary_without_rounding() {
    for nanoseconds in [
        -60_000_000_001,
        -60_000_000_000,
        -59_999_999_999,
        -1,
        0,
        1,
        59_999_999_999,
        60_000_000_000,
        60_000_000_001,
    ] {
        let instant = now() + Duration::nanoseconds(nanoseconds);
        let suggestion = Suggestion {
            instant: Some(instant),
            ..Default::default()
        };
        let qualifies = nanoseconds.abs() >= 60_000_000_000;
        let mut calls = 0;
        let plan = suggestion.plan(&ids(&["UTC"]), "UTC", now(), true, true, |actual| {
            calls += 1;
            assert_eq!(actual, instant);
            "host description".into()
        });
        assert_eq!(plan.is_some(), qualifies, "{nanoseconds} ns");
        assert_eq!(calls, usize::from(qualifies));
        if let Some(plan) = plan {
            assert_eq!(plan.instant, Some(instant));
            assert_eq!(plan.parts, Parts::TIME);
            assert_eq!(plan.summary, "Set time to host description");
            assert_eq!(plan.zone_ids, None);
            assert_eq!(plan.anchor_zone_id, None);
        }
    }
}

#[test]
fn capability_flags_independently_gate_time_and_all_location_changes() {
    let suggestion = mixed_suggestion();
    for (allows_time, allows_locations) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut calls = 0;
        let plan = suggestion.plan(
            &ids(&["UTC"]),
            "UTC",
            now(),
            allows_time,
            allows_locations,
            |_| {
                calls += 1;
                "1:00 PM".into()
            },
        );
        assert_eq!(calls, usize::from(allows_time));
        assert_eq!(plan.is_some(), allows_time || allows_locations);
        if let Some(plan) = plan {
            assert_eq!(plan.instant.is_some(), allows_time);
            assert_eq!(plan.zone_ids.is_some(), allows_locations);
            assert_eq!(plan.anchor_zone_id.is_some(), allows_locations);
            assert_eq!(plan.parts.contains(Parts::TIME), allows_time);
            assert_eq!(plan.parts.contains(Parts::LOCATIONS), allows_locations);
            assert_eq!(
                plan.summary,
                match (allows_time, allows_locations) {
                    (true, true) => "Set time to 1:00 PM · Add Tokyo · Reference: Tokyo",
                    (true, false) => "Set time to 1:00 PM",
                    (false, true) => "Add Tokyo · Reference: Tokyo",
                    (false, false) => unreachable!(),
                }
            );
        }
        assert_eq!(suggestion.parts(), Parts::TIME | Parts::LOCATIONS);
    }
}

#[test]
fn unchanged_suggestion_and_empty_suggestion_do_not_invoke_formatter() {
    for suggestion in [
        Suggestion::default(),
        Suggestion {
            instant: Some(now() + Duration::seconds(20)),
            zone_ids: ids(&["UTC"]),
            anchor_zone_id: Some("UTC".into()),
            ..Default::default()
        },
    ] {
        assert!(
            suggestion
                .plan(&ids(&["UTC"]), "UTC", now(), true, true, |_| {
                    panic!("No time change is planned")
                })
                .is_none()
        );
    }
}

#[test]
fn validation_keeps_first_occurrence_and_exact_alias_spelling_without_defaults() {
    assert_eq!(
        valid_identifiers(&ids(&[
            "Asia/Singapore",
            "invalid",
            "Asia/Singapore",
            "UTC",
            "Etc/UTC",
            "US/Eastern",
            "America/New_York",
            "US/Eastern",
            " UTC",
            "UTC ",
            "utc",
            "",
        ])),
        ids(&[
            "Asia/Singapore",
            "UTC",
            "Etc/UTC",
            "US/Eastern",
            "America/New_York",
        ])
    );
    assert!(valid_identifiers(&[]).is_empty());
    assert!(valid_identifiers(&ids(&["invalid", ""])).is_empty());
}

#[test]
fn validation_is_bounded_to_existing_rust_catalog() {
    // Foundation additionally parses GMT/UTC offset identifiers. This port does
    // not invent support for them or silently normalize them to IANA Etc names.
    assert!(valid_identifiers(&ids(&["GMT+8", "GMT+05:30", "UTC+05:30"])).is_empty());
    assert_eq!(
        valid_identifiers(&ids(&["Etc/GMT-8", "Asia/Kolkata"])),
        ids(&["Etc/GMT-8", "Asia/Kolkata"])
    );
    let catalog: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .map(|zone| zone.name().to_owned())
        .collect();
    assert_eq!(valid_identifiers(&catalog), catalog);
}

#[test]
fn additions_keep_existing_order_and_summarize_only_new_valid_locations() {
    let suggestion = Suggestion {
        zone_ids: ids(&["Asia/Tokyo", "invalid", "UTC", "Asia/Tokyo", "Asia/Kolkata"]),
        ..Default::default()
    };
    let plan = location_plan(&suggestion, &["Europe/London", "UTC"], "UTC").unwrap();
    assert_eq!(
        plan.zone_ids,
        Some(ids(&["Europe/London", "UTC", "Asia/Tokyo", "Asia/Kolkata"]))
    );
    assert_eq!(plan.anchor_zone_id, None);
    assert_eq!(plan.summary, "Add Tokyo, India");
    assert_eq!(plan.parts, Parts::LOCATIONS);
}

#[test]
fn additions_preserve_current_duplicates_unknown_ids_and_aliases_literally() {
    let suggestion = Suggestion {
        zone_ids: ids(&["Etc/UTC", "UTC", "Asia/Tokyo"]),
        ..Default::default()
    };
    let current = ["not/a/zone", "UTC", "UTC", ""];
    let plan = location_plan(&suggestion, &current, "not/a/zone").unwrap();
    assert_eq!(
        plan.zone_ids,
        Some(ids(&[
            "not/a/zone",
            "UTC",
            "UTC",
            "",
            "Etc/UTC",
            "Asia/Tokyo"
        ]))
    );
    assert_eq!(plan.anchor_zone_id, None);
    assert_eq!(plan.summary, "Add UTC, Tokyo");
}

#[test]
fn replacement_deduplicates_valid_proposals_and_falls_back_when_reference_disappears() {
    let suggestion = Suggestion {
        zone_ids: ids(&["Europe/Berlin", "invalid", "Europe/Berlin", "Asia/Kolkata"]),
        replaces_locations: true,
        ..Default::default()
    };
    let plan = location_plan(&suggestion, &["UTC", "Asia/Tokyo"], "UTC").unwrap();
    assert_eq!(plan.zone_ids, Some(ids(&["Europe/Berlin", "Asia/Kolkata"])));
    assert_eq!(plan.anchor_zone_id.as_deref(), Some("Europe/Berlin"));
    assert_eq!(
        plan.summary,
        "Replace locations with Berlin, India · Reference: Berlin"
    );
    assert_eq!(plan.parts, Parts::LOCATIONS);
}

#[test]
fn reordered_replacement_is_a_change_but_identical_order_is_not() {
    let suggestion = Suggestion {
        zone_ids: ids(&["Asia/Tokyo", "UTC"]),
        replaces_locations: true,
        ..Default::default()
    };
    let plan = location_plan(&suggestion, &["UTC", "Asia/Tokyo"], "UTC").unwrap();
    assert_eq!(plan.zone_ids, Some(ids(&["Asia/Tokyo", "UTC"])));
    assert_eq!(plan.anchor_zone_id, None);
    assert_eq!(plan.summary, "Replace locations with Tokyo, UTC");
    assert!(location_plan(&suggestion, &["Asia/Tokyo", "UTC"], "UTC").is_none());
}

#[test]
fn empty_or_invalid_replacements_do_not_erase_locations_or_repair_reference() {
    for zone_ids in [Vec::new(), ids(&["invalid", "UTC ", "invalid"])] {
        let suggestion = Suggestion {
            zone_ids,
            replaces_locations: true,
            ..Default::default()
        };
        assert!(location_plan(&suggestion, &["UTC", "Asia/Tokyo"], "missing").is_none());
        assert!(location_plan(&suggestion, &[], "missing").is_none());
        let with_anchor = Suggestion {
            anchor_zone_id: Some("Asia/Tokyo".into()),
            ..suggestion
        };
        let plan = location_plan(&with_anchor, &["UTC", "Asia/Tokyo"], "UTC").unwrap();
        assert_eq!(plan.zone_ids, None);
        assert_eq!(plan.anchor_zone_id.as_deref(), Some("Asia/Tokyo"));
        assert_eq!(plan.summary, "Reference: Tokyo");
    }
}

#[test]
fn anchor_only_change_requires_result_membership_and_different_current_anchor() {
    let suggestion = Suggestion {
        anchor_zone_id: Some("Asia/Tokyo".into()),
        ..Default::default()
    };
    let plan = location_plan(&suggestion, &["UTC", "Asia/Tokyo"], "UTC").unwrap();
    assert_eq!(plan.zone_ids, None);
    assert_eq!(plan.anchor_zone_id.as_deref(), Some("Asia/Tokyo"));
    assert_eq!(plan.summary, "Reference: Tokyo");
    assert_eq!(plan.parts, Parts::LOCATIONS);
    assert!(location_plan(&suggestion, &["UTC"], "UTC").is_none());
    assert!(location_plan(&suggestion, &["UTC", "Asia/Tokyo"], "Asia/Tokyo").is_none());

    // The source checks resulting membership, not separate anchor validity.
    let literal = Suggestion {
        anchor_zone_id: Some("old/Unsupported_Name".into()),
        ..Default::default()
    };
    let plan = location_plan(&literal, &["UTC", "old/Unsupported_Name"], "UTC").unwrap();
    assert_eq!(plan.anchor_zone_id.as_deref(), Some("old/Unsupported_Name"));
    assert_eq!(plan.summary, "Reference: Unsupported Name");
}

#[test]
fn literal_anchor_names_match_swift_empty_path_component_fallback() {
    for (anchor, name) in [
        ("Asia/Kolkata/", "Kolkata"),
        ("old//Unsupported_Name/", "Unsupported Name"),
        ("///", "///"),
        ("", ""),
    ] {
        let suggestion = Suggestion {
            anchor_zone_id: Some(anchor.into()),
            ..Default::default()
        };
        let plan = location_plan(&suggestion, &["UTC", anchor], "UTC").unwrap();
        assert_eq!(plan.anchor_zone_id.as_deref(), Some(anchor));
        assert_eq!(plan.summary, format!("Reference: {name}"));
        assert_eq!(plan.zone_ids, None);
    }
}

#[test]
fn reference_choice_prioritizes_requested_member_then_current_member_then_first() {
    for (requested, current_anchor, expected_anchor, expected_summary) in [
        (
            Some("Asia/Tokyo"),
            "UTC",
            Some("Asia/Tokyo"),
            " · Reference: Tokyo",
        ),
        (Some("invalid"), "Europe/Berlin", None, ""),
        (None, "Europe/Berlin", None, ""),
        (
            Some("UTC"),
            "UTC",
            Some("Europe/Berlin"),
            " · Reference: Berlin",
        ),
        (
            Some("invalid"),
            "UTC",
            Some("Europe/Berlin"),
            " · Reference: Berlin",
        ),
        (None, "UTC", Some("Europe/Berlin"), " · Reference: Berlin"),
    ] {
        let suggestion = Suggestion {
            zone_ids: ids(&["Europe/Berlin", "Asia/Tokyo"]),
            replaces_locations: true,
            anchor_zone_id: requested.map(str::to_owned),
            ..Default::default()
        };
        let plan = location_plan(&suggestion, &["UTC", "Europe/Berlin"], current_anchor).unwrap();
        assert_eq!(plan.anchor_zone_id.as_deref(), expected_anchor);
        assert_eq!(
            plan.summary,
            format!("Replace locations with Berlin, Tokyo{expected_summary}")
        );
    }
}

#[test]
fn reference_fallback_requires_an_actual_list_change() {
    let noop = Suggestion {
        zone_ids: ids(&["UTC"]),
        ..Default::default()
    };
    assert!(location_plan(&noop, &["UTC"], "missing").is_none());
    let added = Suggestion {
        zone_ids: ids(&["Asia/Tokyo"]),
        ..Default::default()
    };
    let plan = location_plan(&added, &["UTC"], "missing").unwrap();
    assert_eq!(plan.anchor_zone_id.as_deref(), Some("UTC"));
    assert_eq!(plan.summary, "Add Tokyo · Reference: UTC");
    let from_empty = location_plan(&added, &[], "missing").unwrap();
    assert_eq!(from_empty.zone_ids, Some(ids(&["Asia/Tokyo"])));
    assert_eq!(from_empty.anchor_zone_id.as_deref(), Some("Asia/Tokyo"));
    assert_eq!(from_empty.summary, "Add Tokyo · Reference: Tokyo");
}

#[test]
fn calculation_does_not_apply_parser_or_planner_setter_location_caps() {
    let many: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .filter(|zone| **zone != chrono_tz::UTC)
        .take(30)
        .map(|zone| zone.name().to_owned())
        .collect();
    assert_eq!(many.len(), 30);
    for replacing in [false, true] {
        let suggestion = Suggestion {
            zone_ids: many.clone(),
            replaces_locations: replacing,
            ..Default::default()
        };
        let plan = location_plan(&suggestion, &["UTC"], "UTC").unwrap();
        let expected = if replacing {
            many.clone()
        } else {
            let mut expected = ids(&["UTC"]);
            expected.extend_from_slice(&many);
            expected
        };
        assert_eq!(plan.zone_ids, Some(expected));
    }
}

#[test]
fn proposed_parts_include_invalid_and_noop_proposals_but_not_replace_flag_alone() {
    for (suggestion, expected_parts) in [
        (Suggestion::default(), Parts::empty()),
        (
            Suggestion {
                replaces_locations: true,
                ..Default::default()
            },
            Parts::empty(),
        ),
        (
            Suggestion {
                instant: Some(now()),
                ..Default::default()
            },
            Parts::TIME,
        ),
        (
            Suggestion {
                zone_ids: ids(&["invalid"]),
                ..Default::default()
            },
            Parts::LOCATIONS,
        ),
        (
            Suggestion {
                anchor_zone_id: Some(String::new()),
                ..Default::default()
            },
            Parts::LOCATIONS,
        ),
        (mixed_suggestion(), Parts::TIME | Parts::LOCATIONS),
    ] {
        assert_eq!(suggestion.parts(), expected_parts);
        assert_eq!(suggestion.is_empty(), expected_parts.is_empty());
    }
}

#[test]
fn time_only_preview_leaves_location_part_available_to_dedicated_host() {
    let suggestion = mixed_suggestion();
    let preview = suggestion
        .plan(&ids(&["UTC"]), "UTC", now(), true, false, |_| {
            "1:00 PM".into()
        })
        .unwrap();
    let remaining = suggestion.parts().difference(preview.parts);
    assert_eq!(preview.parts, Parts::TIME);
    assert_eq!(remaining, Parts::LOCATIONS);
    let deferred = suggestion
        .plan(
            &ids(&["UTC"]),
            "UTC",
            preview.instant.unwrap(),
            false,
            true,
            |_| panic!("Already-handled time part must not be formatted"),
        )
        .unwrap();
    assert_eq!(deferred.parts, remaining);
    assert_eq!(deferred.instant, None);
    assert_eq!(deferred.zone_ids, Some(ids(&["UTC", "Asia/Tokyo"])));
    assert_eq!(deferred.summary, "Add Tokyo · Reference: Tokyo");
    assert!(
        suggestion
            .parts()
            .difference(preview.parts | deferred.parts)
            .is_empty()
    );
}

#[test]
fn parts_support_independent_completion_and_empty_set_membership() {
    let both = Parts::TIME.union(Parts::LOCATIONS);
    assert!(both.contains(Parts::TIME));
    assert!(both.contains(Parts::LOCATIONS));
    assert!(both.contains(Parts::empty()));
    assert!(!Parts::TIME.contains(Parts::LOCATIONS));
    assert_eq!(both.difference(Parts::LOCATIONS), Parts::TIME);
    assert_eq!(both.difference(Parts::TIME), Parts::LOCATIONS);
    assert!(both.difference(both).is_empty());
}
