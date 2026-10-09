use super::Launcher;
use crate::text_tool_state::Category;
use gpui::{Focusable, KeyDownEvent, Keystroke, TestAppContext};
use std::time::Duration;
fn tick(cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(301));
    cx.run_until_parked();
}
fn key(s: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(s).unwrap(),
        is_held: false,
    }
}
#[gpui::test]
fn text_palette_defaults_retention_snapshot_and_successful_open_latch(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("hello World".into(), w, cx));
    let session = view
        .update(cx, |v, w, cx| {
            v.query
                .update(cx, |e, cx| e.set_text("text tools".into(), cx));
            v.refresh_preview(w, cx);
            assert!(v.text_active);
            v.text.as_ref().unwrap().read(cx).session.clone()
        })
        .unwrap();
    tick(cx);
    session.update(cx, |s, cx| {
        assert_eq!(s.output, "HELLO WORLD");
        let mut c = s.choices;
        c.select_option("snake");
        c.select_category(Category::Encode);
        c.select_option("url");
        c.select_category(Category::Decode);
        c.select_option("decode-hex");
        c.select_category(Category::Lines);
        c.select_option("unique");
        c.select_category(Category::Case);
        s.set_choices(c, cx);
    });
    tick(cx);
    view.update(cx, |v, w, cx| {
        v.query.update(cx, |e, cx| e.set_text("qr".into(), cx));
        v.refresh_preview(w, cx);
        assert!(!v.text_active);
        v.query
            .update(cx, |e, cx| e.set_text("text tools".into(), cx));
        v.refresh_preview(w, cx);
        assert_eq!(
            v.text.as_ref().unwrap().read(cx).session.entity_id(),
            session.entity_id()
        );
        assert_eq!(session.read(cx).choices.argument(), "snake");
        v.launch(w, cx);
        assert!(v.closed);
        v.launch(w, cx);
    })
    .unwrap();
    tick(cx);
    assert_eq!(cx.windows().len(), 1);
    let full = cx.windows()[0];
    cx.update(|cx| {
        let (input, choices) = crate::desktop::text_window_state(full, cx).unwrap();
        assert_eq!(input, "hello World");
        assert_eq!(choices, session.read(cx).choices);
        assert_eq!(choices.case.argument(), "snake");
        assert_eq!(choices.encode.argument(), "url");
        assert_eq!(choices.decode.argument(), "decode-hex");
        assert_eq!(choices.lines.argument(), "unique");
    });
    session.update(cx, |s, _| {
        assert!(!s.can_snapshot());
        assert!(!s.can_copy());
    });
}
#[gpui::test]
fn text_query_copy_and_escape_keep_keyboard_owners(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("hello".into(), w, cx));
    view.update(cx, |v, w, cx| {
        v.query
            .update(cx, |e, cx| e.set_text("text tools".into(), cx));
        v.refresh_preview(w, cx);
        v.query.read(cx).focus(w);
        assert!(!v.route_text_key(
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
            Some("text tools")
        )
    });
    view.update(cx, |v, w, cx| {
        assert!(v.route_text_key(&key("tab"), w, cx));
        assert!(v.route_text_key(&key("escape"), w, cx));
        assert!(v.query.read(cx).focus_handle(cx).is_focused(w));
        assert!(v.route_text_key(&key("escape"), w, cx));
        assert!(!v.closed);
    })
    .unwrap();
}
#[gpui::test]
fn text_oversized_preview_defers_but_preserves_snapshot(cx: &mut TestAppContext) {
    let input = "a".repeat(bellobox_core::MAX_PREVIEW_BYTES + 1);
    let view = cx.add_window(|w, cx| Launcher::new(input.clone(), w, cx));
    let session = view
        .update(cx, |v, w, cx| {
            v.query
                .update(cx, |e, cx| e.set_text("text tools".into(), cx));
            v.refresh_preview(w, cx);
            v.text.as_ref().unwrap().read(cx).session.clone()
        })
        .unwrap();
    tick(cx);
    session.update(cx, |s, _| {
        assert!(s.oversized_preview());
        assert!(!s.busy);
        assert!(s.output.is_empty());
        assert_eq!(s.snapshot().unwrap().input, input);
    });
    view.update(cx, |v, w, cx| v.launch(w, cx)).unwrap();
    tick(cx);
    assert_eq!(cx.windows().len(), 1);
}
#[gpui::test]
fn text_selection_replacement_retires_old_preview_and_defaults_new(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("hello".into(), w, cx));
    view.update(cx, |v, w, cx| {
        v.query
            .update(cx, |e, cx| e.set_text("text tools".into(), cx));
        v.refresh_preview(w, cx);
        let old = v.text.as_ref().unwrap().read(cx).session.clone();
        old.update(cx, |s, cx| {
            let mut c = s.choices;
            c.select_category(Category::Lines);
            s.set_choices(c, cx);
        });
        v.discard_text(cx);
        v.input = "new".into();
        v.refresh_preview(w, cx);
        assert!(!old.read(cx).can_snapshot());
        let new = v.text.as_ref().unwrap().read(cx).session.read(cx);
        assert_eq!(new.choices.category, Category::Case);
        assert_eq!(new.input(), "new");
    })
    .unwrap();
}

#[gpui::test]
fn text_option_menu_keyboard_owns_return_and_preserves_palette(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("HELLO world".into(), w, cx));
    view.update(cx, |v, w, cx| {
        v.query
            .update(cx, |e, cx| e.set_text("text tools".into(), cx));
        v.refresh_preview(w, cx);
        v.query.read(cx).focus(w);
    })
    .unwrap();
    tick(cx);
    cx.simulate_keystrokes(
        view.into(),
        "tab tab tab tab tab tab tab tab space down enter",
    );
    tick(cx);
    view.update(cx, |v, _, cx| {
        assert!(!v.closed);
        let s = v.text.as_ref().unwrap().read(cx).session.read(cx);
        assert_eq!(s.choices.argument(), "lower");
        assert_eq!(s.output, "hello world");
    })
    .unwrap();
}

#[gpui::test]
fn text_output_native_copy_and_navigation_keep_source_owners(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| Launcher::new("hello".into(), w, cx));
    let output = view
        .update(cx, |v, w, cx| {
            v.query
                .update(cx, |e, cx| e.set_text("text tools".into(), cx));
            v.refresh_preview(w, cx);
            v.text.as_ref().unwrap().read(cx).output_editor()
        })
        .unwrap();
    tick(cx);
    view.update(cx, |v, w, cx| {
        output.read(cx).focus(w);
        assert!(!v.route_text_key(
            &key(if cfg!(target_os = "macos") {
                "cmd-c"
            } else {
                "ctrl-c"
            }),
            w,
            cx
        ));
        assert!(!v.route_text_key(&key("down"), w, cx));
        assert!(!v.route_text_key(&key("enter"), w, cx));
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
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("HELLO")
        )
    });
    view.update(cx, |v, w, cx| {
        assert!(!v.route_text_key(&key("x"), w, cx));
        assert!(v.query.read(cx).focus_handle(cx).is_focused(w));
    })
    .unwrap();
}
