use super::*;
use chrono::TimeZone;
use serde_json::json;

fn date(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}
fn request() -> CopilotRequest {
    let instant = date("2026-03-08T09:30:00Z");
    let planner = Planner {
        instant,
        zones: vec![chrono_tz::UTC, chrono_tz::America::Los_Angeles],
        reference: chrono_tz::America::Los_Angeles,
        follows_now: false,
    };
    CopilotRequest {
        question: "  Move to a better time? \n".into(),
        context: Context::from_planner(&planner, instant, "Europe/London").unwrap(),
        history: vec![],
    }
}
fn config(provider: ai::Provider) -> ai::Config {
    ai::Config {
        provider,
        endpoint: "https://example.test/v1".into(),
        model: "test-model".into(),
        system_prompt: "Original writing prompt".into(),
        max_output_tokens: 1024,
        generation_options: Default::default(),
    }
}

#[test]
fn provider_shapes_use_raw_prompt_and_preserve_config() {
    for (provider, route, system, user, tokens, auth) in [
        (
            ai::Provider::OpenAIChat,
            "chat/completions",
            "/messages/0/content",
            "/messages/1/content",
            "max_completion_tokens",
            "authorization",
        ),
        (
            ai::Provider::OpenAIResponses,
            "responses",
            "/instructions",
            "/input",
            "max_output_tokens",
            "authorization",
        ),
        (
            ai::Provider::Anthropic,
            "messages",
            "/system",
            "/messages/0/content",
            "max_tokens",
            "x-api-key",
        ),
    ] {
        let cfg = config(provider);
        let req = request();
        let result = provider_request(&cfg, "test-key", &req).unwrap();
        assert_eq!(
            result.url.as_str(),
            format!("https://example.test/v1/{route}")
        );
        assert_eq!(result.body.pointer(system).unwrap(), SYSTEM_PROMPT);
        assert_eq!(
            result.body.pointer(user).unwrap(),
            &user_prompt(&req).unwrap()
        );
        assert_eq!(result.body["model"], "test-model");
        if provider == ai::Provider::Anthropic {
            assert_eq!(result.body[tokens], 1024);
        } else {
            assert!(result.body.get(tokens).is_none());
        }
        assert_eq!(result.body["stream"], true);
        assert!(result.headers.iter().any(|(k, _)| k == auth));
        assert!(!result.body.to_string().contains("selected_text"));
        assert_eq!(cfg.system_prompt, "Original writing prompt");
    }
}

#[test]
fn provider_validation_is_not_bypassed() {
    let req = request();
    assert!(provider_request(&config(ai::Provider::Anthropic), "", &req).is_err());
    for provider in [
        ai::Provider::OpenAIChat,
        ai::Provider::OpenAIResponses,
        ai::Provider::Anthropic,
    ] {
        let cfg = config(provider);
        assert!(provider_request(&cfg, "bad\nkey", &req).is_err());
        let mut invalid = cfg.clone();
        invalid.endpoint = "https://example.test/?secret=bad".into();
        assert!(provider_request(&invalid, "key", &req).is_err());
        invalid = cfg.clone();
        invalid.model.clear();
        assert!(provider_request(&invalid, "key", &req).is_err());
        invalid = cfg;
        invalid.max_output_tokens = 0;
        assert!(provider_request(&invalid, "key", &req).is_err());
    }
}

#[test]
fn questions_enforce_graphemes_and_bytes_independently() {
    assert!(validate_question(" \n\t").is_err());
    assert_eq!(validate_question(" \nhello \t").unwrap(), "hello");
    assert!(validate_question(&"a".repeat(2000)).is_ok());
    assert!(validate_question(&"a".repeat(2001)).is_err());
    assert!(validate_question(&"e\u{301}".repeat(2000)).is_ok());
    assert!(validate_question(&"e\u{301}".repeat(2001)).is_err());
    assert!(validate_question(&"👩‍💻".repeat(700)).is_ok());
    assert!(validate_question(&"👩‍💻".repeat(800)).is_err());
    // Two graphemes, exactly 8192 bytes; one more combining scalar is rejected.
    let exact = format!("ab{}", "\u{301}".repeat(4095));
    assert_eq!(exact.len(), QUESTION_BYTE_LIMIT);
    assert!(validate_question(&exact).is_ok());
    assert!(validate_question(&(exact + "\u{301}")).is_err());
}

#[test]
fn prompt_uses_snapshot_with_dst_reference_and_live_intent() {
    let mut req = request();
    let prompt = user_prompt(&req).unwrap();
    assert!(
        prompt.contains("Current time: 2026-03-08T09:30:00Z (user's local zone Europe/London)")
    );
    assert!(prompt.contains("Selected instant: 2026-03-08T09:30:00Z (planning time)"));
    assert!(prompt.contains("Reference location: Los Angeles (America/Los_Angeles)"));
    assert!(prompt.contains("[reference]"));
    assert!(prompt.contains(&req.context.locations[1].local_description));
    assert!(prompt.ends_with("Question: Move to a better time?"));
    assert!(!prompt.contains("Conversation so far:"));
    req.context.is_following_now = true;
    assert!(user_prompt(&req).unwrap().contains("(live time)"));
}

#[test]
fn history_keeps_last_six_and_bounds_scalars_before_allocation() {
    let mut req = request();
    req.history = (0..8)
        .map(|i| Turn {
            role: if i % 2 == 0 {
                Role::User
            } else {
                Role::Assistant
            },
            text: format!("turn-{i}\ncontinued"),
        })
        .collect();
    req.history[7].text = "😀\n".repeat(301);
    let prompt = user_prompt(&req).unwrap();
    assert!(!prompt.contains("turn-0"));
    assert!(!prompt.contains("turn-1"));
    assert!(prompt.contains("User: turn-2 continued"));
    assert!(prompt.contains("Assistant: turn-3 continued"));
    assert!(prompt.contains(&format!("Assistant: {}…", "😀 ".repeat(300))));
    assert_eq!(bounded("e\u{301}x", 2), "e\u{301}…");
}

#[test]
fn fabricated_context_is_validated() {
    let base = request();
    let mut cases = Vec::new();
    let mut r = base.clone();
    r.context.locations.clear();
    cases.push(r);
    let mut r = base.clone();
    r.context.locations = vec![r.context.locations[0].clone(); 25];
    cases.push(r);
    let mut r = base.clone();
    r.context.reference_zone_id = "Asia/Tokyo".into();
    cases.push(r);
    let mut r = base.clone();
    r.context.local_zone_id = "invalid".into();
    cases.push(r);
    let mut r = base.clone();
    r.context.locations[0].zone_id = "invalid".into();
    cases.push(r);
    let mut r = base.clone();
    r.context.locations[0].name = "é".repeat(257);
    cases.push(r);
    let mut r = base.clone();
    r.context.locations[0].local_description = "x".repeat(513);
    cases.push(r);
    let mut r = base.clone();
    r.context.now = Utc.with_ymd_and_hms(0, 1, 1, 0, 0, 0).unwrap();
    cases.push(r);
    let mut r = base.clone();
    r.context.selected_instant = Utc.with_ymd_and_hms(10000, 1, 1, 0, 0, 0).unwrap();
    cases.push(r);
    for r in cases {
        assert!(user_prompt(&r).is_err());
    }
    let mut maximal = base;
    maximal.context.locations = vec![maximal.context.locations[1].clone(); 24];
    for l in &mut maximal.context.locations {
        l.name = "x".repeat(512);
        l.local_description = "x".repeat(512);
    }
    maximal.question = "😀".repeat(2000);
    maximal.history = vec![
        Turn {
            role: Role::User,
            text: "😀".repeat(100000)
        };
        6
    ];
    assert!(user_prompt(&maximal).unwrap().len() < MAX_PROMPT_BYTES);
}

#[test]
fn envelope_fields_keep_acronyms_and_all_proposal_parts() {
    let result = parse_response(r#"{"answer":"  Proposed  ","suggestion":{"referenceDate":" 2026-10-08T10:20:30.125+02:00 ","timeZoneIDs":[" Asia/Tokyo ","UTC","Asia/Tokyo",""],"replaceLocations":true,"anchorTimeZoneID":" Asia/Tokyo "}}"#).unwrap();
    assert_eq!(result.answer, "Proposed");
    let proposal = result.suggestion.unwrap();
    assert_eq!(proposal.instant, Some(date("2026-10-08T08:20:30.125Z")));
    assert_eq!(proposal.zone_ids, ["Asia/Tokyo", "UTC"]);
    assert_eq!(proposal.anchor_zone_id.as_deref(), Some("Asia/Tokyo"));
    assert!(proposal.replaces_locations);
    assert!(result.suggestion_issue.is_none());
}

#[test]
fn malformed_envelopes_remain_plain_text_without_actions() {
    for input in [
        "hello",
        "{broken}",
        r#"{"answer":2,"suggestion":{"timeZoneIDs":["UTC"]}}"#,
        r#"{"answer":"ok","suggestion":{"timeZoneIDs":"UTC"}}"#,
        r#"{"answer":"ok","suggestion":{"replaceLocations":"true"}}"#,
        r#"{"answer":"ok","suggestion":{"timeZoneIDs":[1]}}"#,
        r#"{"answer":"ok","suggestion":{"referenceDate":12}}"#,
        r#"{"answer":"ok","suggestion":{"anchorTimeZoneID":true}}"#,
        r#"{"answer":null,"suggestion":null}"#,
        "{}",
        "} before {",
        r#"{"answer":"one"} {"answer":"two"}"#,
    ] {
        let reply = parse_response(input).unwrap();
        assert_eq!(reply.answer, input);
        assert!(reply.suggestion.is_none());
        assert!(reply.suggestion_issue.is_none());
    }
    let fenced = parse_response("```json\n{\"answer\":\"Valid\",\"extra\":42}\n```").unwrap();
    assert_eq!(fenced.answer, "Valid");
}

#[test]
fn mixed_valid_and_invalid_parts_preserve_valid_changes() {
    let reply = parse_response(r#"{"answer":"ok","suggestion":{"referenceDate":"nonsense","timeZoneIDs":["Bad/Zone"," UTC ","GMT+8"],"anchorTimeZoneID":"bad"}}"#).unwrap();
    let proposal = reply.suggestion.unwrap();
    assert_eq!(proposal.zone_ids, ["UTC"]);
    assert!(proposal.instant.is_none());
    assert!(proposal.anchor_zone_id.is_none());
    let issue = reply.suggestion_issue.unwrap();
    assert!(issue.contains("could not be read"));
    assert!(issue.contains("Bad/Zone, GMT+8"));
    assert!(issue.contains("reference zone"));
    let reply = parse_response(r#"{"answer":"ok","suggestion":{"referenceDate":"2026-01-01T00:00:00Z","timeZoneIDs":["bad"]}}"#).unwrap();
    assert!(reply.suggestion.unwrap().instant.is_some());
    assert!(reply.suggestion_issue.is_some());
}

#[test]
fn zone_limits_are_stable_and_issue_lists_are_bounded() {
    let zones: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .take(20)
        .map(|z| z.name())
        .collect();
    let value = json!({"answer":"ok", "suggestion":{"timeZoneIDs":zones}});
    assert_eq!(
        parse_response(&value.to_string())
            .unwrap()
            .suggestion
            .unwrap()
            .zone_ids,
        zones[..12]
    );
    let value = json!({"answer":"ok", "suggestion":{"timeZoneIDs":["bad1","bad2","bad3","bad4","bad5", "UTC"]}});
    let reply = parse_response(&value.to_string()).unwrap();
    assert_eq!(reply.suggestion.unwrap().zone_ids, ["UTC"]);
    assert!(!reply.suggestion_issue.unwrap().contains("bad5"));
}

#[test]
fn parser_calendar_range_is_inclusive_and_checks_utc_conversion() {
    for stamp in [
        "0001-01-01T00:00:00Z",
        "9999-12-31T23:59:59Z",
        "1900-01-01T00:00:00Z",
        "9999-12-31T23:59:58.999Z",
    ] {
        let reply =
            parse_response(&json!({"suggestion":{"referenceDate":stamp}}).to_string()).unwrap();
        assert_eq!(reply.suggestion.unwrap().instant, Some(date(stamp)));
    }
    for stamp in [
        "0000-12-31T23:59:59Z",
        "+10000-01-01T00:00:00Z",
        "9999-12-31T23:59:59.001Z",
        "0001-01-01T00:00:00+01:00",
        "9999-12-31T23:59:59-01:00",
    ] {
        let reply = parse_response(
            &json!({"answer":"ok","suggestion":{"referenceDate":stamp,"timeZoneIDs":["UTC"]}})
                .to_string(),
        )
        .unwrap();
        assert!(reply.suggestion.unwrap().instant.is_none(), "{stamp}");
        assert!(reply.suggestion_issue.is_some(), "{stamp}");
    }
}

#[test]
fn empty_answers_and_empty_suggestions_match_source() {
    for input in [
        "",
        " \n ",
        r#"{"answer":" "}"#,
        r#"{"suggestion":{}}"#,
        r#"{"suggestion":{"replaceLocations":true}}"#,
        r#"{"suggestion":{"referenceDate":" ","timeZoneIDs":[""],"anchorTimeZoneID":" "}}"#,
    ] {
        assert!(parse_response(input).is_err(), "{input}");
    }
    let reply = parse_response(r#"{"suggestion":{"anchorTimeZoneID":"UTC"}}"#).unwrap();
    assert_eq!(reply.answer, "Here is a change you can apply.");
    assert_eq!(
        reply.suggestion.unwrap().anchor_zone_id.as_deref(),
        Some("UTC")
    );
    assert!(
        parse_response(r#"{"answer":"ok","suggestion":null}"#)
            .unwrap()
            .suggestion
            .is_none()
    );
}

#[test]
fn answer_and_response_bounds_are_utf8_safe_and_scalar_based() {
    let text = "😀".repeat(4001);
    let expected = format!("{}…", "😀".repeat(4000));
    assert_eq!(parse_response(&text).unwrap().answer, expected);
    assert_eq!(
        parse_response(&json!({"answer":text}).to_string())
            .unwrap()
            .answer,
        expected
    );
    assert_eq!(
        parse_response(&"a".repeat(4000)).unwrap().answer.len(),
        4000
    );
    assert!(parse_response(&"x".repeat(MAX_RESPONSE_BYTES)).is_ok());
    assert!(parse_response(&"x".repeat(MAX_RESPONSE_BYTES + 1)).is_err());
}

#[test]
fn planner_context_construction_is_read_only_and_rejects_invalid_state() {
    let instant = date("2026-11-01T09:30:00Z");
    let planner = Planner {
        instant,
        zones: vec![chrono_tz::UTC, chrono_tz::America::Los_Angeles],
        reference: chrono_tz::America::Los_Angeles,
        follows_now: true,
    };
    let before = planner.presentations();
    let context = Context::from_planner(&planner, instant, "UTC").unwrap();
    assert!(context.is_following_now);
    assert_eq!(context.selected_instant, instant);
    assert_eq!(context.locations.len(), 2);
    for (row, presentation) in context.locations.iter().zip(before) {
        assert_eq!(row.zone_id, presentation.id);
        assert_eq!(row.name, presentation.name);
        assert_eq!(row.quality, presentation.quality);
        assert_eq!(row.is_reference, presentation.is_reference);
    }
    assert_eq!(planner.instant, instant);
    assert!(planner.follows_now);
    assert_eq!(planner.reference, chrono_tz::America::Los_Angeles);
    assert_eq!(planner.zones.len(), 2);
    assert!(Context::from_planner(&planner, instant, "Not/AZone").is_err());
    let mut invalid = planner.clone();
    invalid.zones.clear();
    assert!(Context::from_planner(&invalid, instant, "UTC").is_err());
    invalid = planner.clone();
    invalid.zones = vec![chrono_tz::UTC; 25];
    assert!(Context::from_planner(&invalid, instant, "UTC").is_err());
    invalid = planner.clone();
    invalid.reference = chrono_tz::Asia::Tokyo;
    assert!(Context::from_planner(&invalid, instant, "UTC").is_err());
    let out_of_range = Utc.with_ymd_and_hms(0, 1, 1, 0, 0, 0).unwrap();
    assert!(Context::from_planner(&planner, out_of_range, "UTC").is_err());
    invalid = planner;
    invalid.instant = out_of_range;
    assert!(Context::from_planner(&invalid, instant, "UTC").is_err());
}
