use super::Launcher;
use crate::json_session::Mode;
use gpui::{Focusable, KeyDownEvent, Keystroke, TestAppContext};
use std::time::Duration;
fn tick(cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(221));
    cx.run_until_parked();
}
fn key(k: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(k).unwrap(),
        is_held: false,
    }
}
#[gpui::test]
fn json_palette_retains_mode_chained_text_and_transfers_actual_owner(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new(r#"{ "z": 2, "a": 1 }"#.into(), w, cx));
    let session = view
        .update(cx, |v, w, cx| {
            v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
            v.refresh_preview(w, cx);
            assert!(v.json_active);
            v.json.as_ref().unwrap().read(cx).session.clone()
        })
        .unwrap();
    session.update(cx, |s, cx| s.set_mode(Mode::Minify, cx));
    tick(cx);
    session.update(cx, |s, cx| {
        assert_eq!(s.output, "{\"a\":1,\"z\":2}");
        assert!(s.chain(cx));
    });
    tick(cx);
    let preview = view
        .update(cx, |v, w, cx| {
            let original = v.json.as_ref().unwrap().clone();
            v.query.update(cx, |q, cx| q.set_text("qr".into(), cx));
            v.refresh_preview(w, cx);
            assert!(!v.json_active);
            v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
            v.refresh_preview(w, cx);
            assert_eq!(v.json.as_ref().unwrap().entity_id(), original.entity_id());
            assert_eq!(
                v.json.as_ref().unwrap().read(cx).session.entity_id(),
                session.entity_id()
            );
            v.launch(w, cx);
            assert!(v.closed);
            v.launch(w, cx); // Same-turn repeat must not open a second original draft.
            original
        })
        .unwrap();
    cx.run_until_parked();
    assert!(view.root(cx).is_err());
    assert_eq!(cx.windows().len(), 1);
    // A retained stale palette view cannot revoke a successfully adopted job.
    preview.update(cx, |v, cx| v.retire(cx));
    session.update(cx, |s, _| {
        assert!(!s.can_transfer());
        assert!(s.can_copy());
        assert_eq!(s.mode, Mode::Minify);
        assert_eq!(s.input(), "{\"a\":1,\"z\":2}");
    });
}
#[gpui::test]
fn json_search_copy_keeps_query_clipboard_ownership(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("{\"payload\":42}".into(), w, cx));
    view.update(cx, |v, w, cx| {
        v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
        v.refresh_preview(w, cx);
        v.query.read(cx).focus(w);
        assert!(!v.route_json_key(
            &key(if cfg!(target_os = "macos") {
                "cmd-c"
            } else {
                "ctrl-c"
            }),
            w,
            cx
        ));
    })
    .unwrap();
    tick(cx);
    cx.simulate_keystrokes(
        view.into(),
        if cfg!(target_os = "macos") {
            "cmd-a cmd-c"
        } else {
            "ctrl-a ctrl-c"
        },
    );
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("json")
        )
    });
    view.update(cx, |v, w, cx| {
        // Tab reaches mode controls, Escape returns search, repeat cannot close.
        assert!(v.route_json_key(&key("tab"), w, cx));
        assert!(v.route_json_key(&key("escape"), w, cx));
        assert!(v.query.read(cx).focus_handle(cx).is_focused(w));
        assert!(v.route_json_key(&key("escape"), w, cx));
        assert!(!v.closed);
    })
    .unwrap();
}
#[gpui::test]
fn oversized_json_preview_retains_draft_and_starts_only_after_open(cx: &mut TestAppContext) {
    let draft = format!("\"{}\"", "a".repeat(bellobox_core::MAX_PREVIEW_BYTES));
    let view = cx.add_window(|w, cx| Launcher::new(draft.clone(), w, cx));
    let session = view
        .update(cx, |v, w, cx| {
            v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
            v.refresh_preview(w, cx);
            v.json.as_ref().unwrap().read(cx).session.clone()
        })
        .unwrap();
    tick(cx);
    session.update(cx, |s, _| {
        assert!(s.oversized_preview());
        assert!(!s.busy);
        assert_eq!(s.input(), draft);
    });
    view.update(cx, |v, w, cx| v.launch(w, cx)).unwrap();
    tick(cx);
    session.update(cx, |s, _| {
        assert!(!s.oversized_preview());
        assert!(s.can_copy());
        assert_eq!(s.input(), draft);
    });
}

#[gpui::test]
fn json_output_copy_and_navigation_keep_their_source_owners(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("[42]".into(), w, cx));
    let output = view
        .update(cx, |v, w, cx| {
            v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
            v.refresh_preview(w, cx);
            v.json.as_ref().unwrap().read(cx).output_editor()
        })
        .unwrap();
    tick(cx);
    view.update(cx, |v, w, cx| {
        output.read(cx).focus(w);
        assert!(!v.route_json_key(
            &key(if cfg!(target_os = "macos") {
                "cmd-c"
            } else {
                "ctrl-c"
            }),
            w,
            cx
        ));
        assert!(!v.route_json_key(&key("down"), w, cx));
        assert!(!v.route_json_key(&key("enter"), w, cx));
    })
    .unwrap();
    cx.simulate_keystrokes(
        view.into(),
        if cfg!(target_os = "macos") {
            "cmd-a cmd-c"
        } else {
            "ctrl-a ctrl-c"
        },
    );
    let expected = crate::execute("json", "[42]", "pretty").unwrap();
    cx.update(|cx| assert_eq!(cx.read_from_clipboard().unwrap().text(), Some(expected)));
    view.update(cx, |v, w, cx| {
        assert!(!v.route_json_key(&key("x"), w, cx));
        assert!(v.query.read(cx).focus_handle(cx).is_focused(w));
    })
    .unwrap();
}

#[gpui::test]
fn json_clear_closes_old_work_and_fresh_palette_has_default_mode(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("[]".into(), w, cx));
    let s = view
        .update(cx, |v, w, cx| {
            v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
            v.refresh_preview(w, cx);
            let s = v.json.as_ref().unwrap().read(cx).session.clone();
            s.update(cx, |s, cx| s.set_mode(Mode::Validate, cx));
            v.discard_json(cx);
            s
        })
        .unwrap();
    tick(cx);
    s.update(cx, |s, _| {
        assert!(!s.can_transfer());
        assert!(!s.can_copy());
        assert!(!s.busy);
    });
    view.update(cx, |v, w, cx| {
        v.input = "[2]".into();
        v.refresh_preview(w, cx);
        let fresh = &v.json.as_ref().unwrap().read(cx).session;
        assert_ne!(fresh.entity_id(), s.entity_id());
        assert_eq!(fresh.read(cx).mode, Mode::Pretty);
        v.close(w, cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(view.root(cx).is_err());
}

#[gpui::test]
fn json_preview_controls_fit_the_source_reserved_row(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("[42]".into(), w, cx));
    view.update(cx, |v, w, cx| {
        v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
        v.refresh_preview(w, cx);
    })
    .unwrap();
    tick(cx);
    let mut visual = gpui::VisualTestContext::from_window(view.into(), cx);
    let row = visual.debug_bounds("json-preview").unwrap();
    let output = visual.debug_bounds("json-preview-output").unwrap();
    assert_eq!(f32::from(row.size.height), 224.);
    assert!(output.size.height > gpui::px(0.));
    for name in [
        "json-preview-control-0",
        "json-preview-control-1",
        "json-preview-control-2",
        "json-preview-control-5",
    ] {
        let b = visual.debug_bounds(name).unwrap();
        assert!(b.left() >= row.left() && b.right() <= row.right());
        assert!(b.top() >= row.top() && b.bottom() <= row.bottom());
    }
}
#[gpui::test]
fn oversized_json_open_is_keyboard_reachable(cx: &mut TestAppContext) {
    let input = format!("\"{}\"", "a".repeat(bellobox_core::MAX_PREVIEW_BYTES));
    let view = cx.add_window(|w, cx| Launcher::new(input, w, cx));
    view.update(cx, |v, w, cx| {
        v.query.update(cx, |q, cx| q.set_text("json".into(), cx));
        v.refresh_preview(w, cx);
        v.query.read(cx).focus(w);
        assert!(v.route_json_key(&key("tab"), w, cx));
        assert!(v.route_json_key(&key("enter"), w, cx));
    })
    .unwrap();
    cx.run_until_parked();
    assert!(view.root(cx).is_err());
    assert_eq!(cx.windows().len(), 1);
}
