use super::{
    LauncherClockPreview, PREVIEW_HEIGHT, Settings, apply, clock, placeholder_visible,
    suggestion_status,
};
use bellobox_core::clock::copilot::{Parts, Suggestion};
use gpui::{Modifiers, TestAppContext, px, size};
fn planner() -> clock::Planner {
    clock::Planner {
        instant: "2026-10-08T12:00:00Z".parse().unwrap(),
        zones: vec!["UTC".parse().unwrap()],
        reference: "UTC".parse().unwrap(),
        follows_now: false,
    }
}
#[test]
fn already_present_location_is_not_deferred() {
    let s = Suggestion {
        zone_ids: vec!["UTC".into()],
        ..Default::default()
    };
    let status = suggestion_status(&planner(), &s, Parts::empty());
    assert!(status.deferred.is_none());
    assert_eq!(status.label, Some("Already in effect."));
}
#[test]
fn remaining_locations_win_over_time_applied_history() {
    let p = planner();
    let s = Suggestion {
        instant: Some(p.instant),
        zone_ids: vec!["Asia/Tokyo".into()],
        ..Default::default()
    };
    let status = suggestion_status(&p, &s, Parts::TIME);
    assert!(status.deferred.unwrap().contains("Tokyo"));
    assert_eq!(status.label, Some("Time applied"));
}
#[test]
fn fully_effective_suggestion_has_applied_label_but_intervening_edit_reopens_it() {
    let mut p = planner();
    let s = Suggestion {
        zone_ids: vec!["Asia/Tokyo".into()],
        ..Default::default()
    };
    apply::prepare(&p, &s, Parts::empty(), false, true)
        .unwrap()
        .unwrap()
        .commit(&mut p)
        .unwrap();
    let status = suggestion_status(&p, &s, Parts::LOCATIONS);
    assert!(status.deferred.is_none());
    assert_eq!(status.label, Some("Applied"));
    p.zones = vec!["UTC".parse().unwrap()];
    let status = suggestion_status(&p, &s, Parts::LOCATIONS);
    assert!(status.deferred.is_some());
    assert_eq!(status.label, Some("Locations applied"));
}
#[test]
fn placeholder_never_covers_text_or_ime_composition() {
    assert!(placeholder_visible("", false));
    assert!(!placeholder_visible("", true));
    assert!(!placeholder_visible("\u{200b}", false));
    assert!(!placeholder_visible("你好", true));
}
#[gpui::test]
fn compact_input_bounds_and_placeholder_click_focus_at_small_width(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        LauncherClockPreview::new(
            "2026-10-08T12:00:00Z".into(),
            &Settings::default(),
            window,
            cx,
        )
    });
    for width in [320., 480., 680.] {
        cx.simulate_resize(size(px(width), px(365.)));
        cx.run_until_parked();
        let input = cx.debug_bounds("palette-copilot-input").unwrap();
        let field = cx.debug_bounds("palette-copilot-field").unwrap();
        assert_eq!(input.size.height, px(30.));
        assert_eq!(field.size.height, px(20.));
        assert!(field.size.width > px(20.));
        assert!(input.contains(&field.origin));
        assert!(field.right() <= input.right());
        assert!(input.bottom() <= px(PREVIEW_HEIGHT));
        // Outer bounds alone miss editor-internal padding. Measure the actual
        // configured editor with ascenders/descenders, not a duplicate formula.
        cx.update(|window, cx| {
            let draft = view.read(cx).copilot.draft.clone();
            draft.update(cx, |editor, cx| {
                editor.set_text("plan tokyo gjpqy ÁÉ".into(), cx);
                let content = editor.measured_content_height(f32::from(field.size.width), window);
                assert!(
                    content <= f32::from(field.size.height),
                    "typed editor content {content} exceeds field {:?}",
                    field.size.height
                );
                editor.set_text(String::new(), cx);
            });
        });
        cx.simulate_click(field.center(), Modifiers::none());
        cx.update(|window, cx| assert!(view.read(cx).draft_focused(window, cx)));
        view.update(cx, |v, cx| {
            v.copilot.notice = Some("Synthetic test notice".into());
            cx.notify();
        });
        cx.run_until_parked();
        let transcript = cx.debug_bounds("palette-copilot-transcript").unwrap();
        let input = cx.debug_bounds("palette-copilot-input").unwrap();
        assert_eq!(transcript.size.height, px(98.));
        assert!(input.origin.y >= transcript.bottom() + px(6.));
        assert!(input.bottom() <= px(365.));
        view.update(cx, |v, cx| {
            v.copilot.notice = None;
            cx.notify();
        });
    }
}

#[test]
fn readiness_cache_isolated_configuration_invalidation() {
    let folder = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "launcher_clock_ui::copilot::presentation_tests::readiness_cache_child",
            "--ignored",
            "--nocapture",
        ])
        .env("BELLOBOX_CONFIG_DIR", folder.path())
        .env("BELLOBOX_READY_CHILD", "1")
        .env_remove("BELLOBOX_AI_PROVIDER")
        .env_remove("BELLOBOX_AI_ENDPOINT")
        .env_remove("BELLOBOX_AI_MODEL")
        .env_remove("BELLOBOX_AI_KEY")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = child.spawn().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            panic!("isolated readiness test timeout");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("READY_CACHE_CHILD_EXECUTED"));
}
#[test]
#[ignore = "executed by isolated parent with temporary configuration"]
fn readiness_cache_child() {
    assert_eq!(std::env::var("BELLOBOX_READY_CHILD").as_deref(), Ok("1"));
    let path = bellobox_core::settings::config_dir().join("settings.json");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut settings = Settings {
        provider_kind: "openai".into(),
        provider_model: String::new(),
        provider_endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
        ..Default::default()
    };
    settings.save(&path).unwrap();
    crate::transport::settings_changed();
    assert!(!crate::transport::provider_is_configured());
    settings.provider_model = "synthetic-model".into();
    settings.save(&path).unwrap();
    assert!(
        !crate::transport::provider_is_configured(),
        "cache changes only after invalidation"
    );
    crate::transport::settings_changed();
    assert!(crate::transport::provider_is_configured());
    settings.provider_kind = "codex".into();
    settings.save(&path).unwrap();
    crate::transport::settings_changed();
    assert!(!crate::transport::provider_is_configured());
    settings.provider_kind = "openai".into();
    settings.provider_endpoint = "http://user:synthetic-secret@127.0.0.1:1/v1".into();
    settings.save(&path).unwrap();
    crate::transport::settings_changed();
    assert!(!crate::transport::provider_is_configured());
    assert!(
        listener.accept().is_err(),
        "readiness must never dispatch HTTP"
    );
    println!("READY_CACHE_CHILD_EXECUTED");
}
