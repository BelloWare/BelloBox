use super::*;
use crate::transport::http_request::tests::fixed;
use gpui::{EntityInputHandler, Keystroke, TestAppContext};
use std::{sync::atomic::Ordering, time::Duration};
fn tick(cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(11));
    cx.run_until_parked();
}
fn command() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }
}
#[gpui::test]
fn import_review_edit_and_complete_copy_require_explicit_send(cx: &mut TestAppContext) {
    let body = "{\"n\":9007199254740993,\"text\":\"雪😀\"}";
    let fixture = fixed(200, "X-Fixture: synthetic\r\n", body.as_bytes().to_vec());
    let input = format!("curl '{}' -X PATCH --json '{}'", fixture.url, body);
    let w = cx.add_window(|w, cx| HttpWindow::new(input, w, cx));
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
    w.update(cx, |s, w, cx| {
        assert!(s.inputs[0].read(cx).focus_handle(cx).is_focused(w));
        assert_eq!(s.inputs[1].read(cx).text(), "PATCH");
        assert_eq!(s.inputs[4].read(cx).text(), body);
        assert!(s.enabled(Control::Send, cx));
        assert!(!s.ready(cx));
        s.activate(Control::Send, w, cx);
        s.activate(Control::Send, w, cx);
    })
    .unwrap();
    tick(cx);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        s.copy(cx);
        let copied = cx.read_from_clipboard().unwrap().text().unwrap();
        assert_eq!(copied, s.session.read(cx).result.as_ref().unwrap().text());
        assert!(copied.contains("9007199254740993"));
        assert!(copied.contains("雪😀"));
        assert_eq!(s.output.read(cx).text(), copied);
    })
    .unwrap();
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
}
#[gpui::test]
fn dirty_import_rejected_paste_and_each_field_clear_stale_copy(cx: &mut TestAppContext) {
    for side in 0..5 {
        let fixture = fixed(200, "", b"synthetic response".to_vec());
        let w = cx.add_window(|w, cx| HttpWindow::new(fixture.url.clone(), w, cx));
        w.update(cx, |s, w, cx| s.activate(Control::Send, w, cx))
            .unwrap();
        tick(cx);
        w.update(cx, |s, _, cx| {
            assert!(s.ready(cx));
            cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
            s.inputs[side].update(cx, |e, cx| e.set_text("changed".into(), cx));
            assert!(!s.ready(cx));
            s.copy(cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                "sentinel"
            );
            s.sync_inputs(false, cx);
            assert!(s.output.read(cx).text().is_empty());
            if side == 0 {
                assert!(!s.enabled(Control::Send, cx));
            }
            s.replace(side, "x".repeat(field_limit(side) + 1), cx);
            assert!(!s.enabled(Control::Send, cx));
            assert!(!s.ready(cx));
        })
        .unwrap();
        assert_eq!(fixture.count.load(Ordering::Acquire), 1);
    }
}
#[gpui::test]
fn shortcuts_require_exact_modifiers_and_method_menu_preserves_custom(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", b"ok".to_vec());
    let w = cx
        .add_window(|w, cx| HttpWindow::new(format!("curl '{}' -X PROPFIND", fixture.url), w, cx));
    for key in [
        format!("{}-shift-enter", command()),
        format!("{}-alt-enter", command()),
        "enter".into(),
    ] {
        cx.simulate_keystrokes(w.into(), &key);
        tick(cx);
    }
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
    w.update(cx, |s, w, cx| {
        // Plain Return belongs to the import editor, so restore its literal seed.
        s.replace(0, fixture.url.clone(), cx);
        s.import(cx);
        s.replace(1, "PROPFIND".into(), cx);
        s.activate(Control::MethodMenu, w, cx);
        assert!(s.method_menu);
        assert!(s.enabled(Control::CustomMethod, cx));
        s.activate(Control::Method(4), w, cx);
        assert_eq!(s.inputs[1].read(cx).text(), "PATCH");
        assert!(!s.method_menu);
        s.inputs[2].read(cx).focus(w);
    })
    .unwrap();
    cx.simulate_keystrokes(w.into(), "enter");
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
    cx.simulate_keystrokes(w.into(), &format!("{}-enter", command()));
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
    cx.simulate_keystrokes(w.into(), "escape");
    assert!(w.update(cx, |_, _, _| ()).is_ok());
}
#[gpui::test]
fn same_byte_ime_on_each_input_invalidates_until_commit_without_sending(cx: &mut TestAppContext) {
    for side in 0..5 {
        let fixture = fixed(200, "", b"ok".to_vec());
        let w = cx.add_window(|w, cx| HttpWindow::new(fixture.url.clone(), w, cx));
        w.update(cx, |s, w, cx| {
            s.replace(3, "X-Fixture: A".into(), cx);
            s.replace(4, "A".into(), cx);
            s.activate(Control::Send, w, cx);
        })
        .unwrap();
        tick(cx);
        w.update(cx, |s, w, cx| {
            let original = s.inputs[side].read(cx).text().to_owned();
            let first = original.chars().next().unwrap();
            s.inputs[side].update(cx, |e, cx| {
                e.replace_and_mark_text_in_range(
                    Some(0..first.len_utf16()),
                    &first.to_string(),
                    None,
                    w,
                    cx,
                )
            });
            assert_eq!(s.inputs[side].read(cx).text(), original);
            assert!(s.composing(cx));
            assert!(!s.ready(cx));
            s.activate(Control::Send, w, cx);
            s.sync_inputs(false, cx);
            assert!(s.output.read(cx).text().is_empty());
            s.inputs[side].update(cx, |e, cx| e.unmark_text(w, cx));
            assert_eq!(s.inputs[side].read(cx).text(), original);
            s.sync_inputs(false, cx);
            assert!(!s.ready(cx));
        })
        .unwrap();
        tick(cx);
        assert_eq!(fixture.count.load(Ordering::Acquire), 1);
    }
}
#[gpui::test]
fn dispatched_stale_output_copy_and_cut_are_blocked_before_observers(cx: &mut TestAppContext) {
    for key in ["c", "x"] {
        let fixture = fixed(200, "", b"private synthetic".to_vec());
        let w = cx.add_window(|w, cx| HttpWindow::new(fixture.url.clone(), w, cx));
        w.update(cx, |s, w, cx| s.activate(Control::Send, w, cx))
            .unwrap();
        tick(cx);
        w.update(cx, |s, w, cx| {
            s.output.read(cx).focus(w);
            s.output.update(cx, |e, cx| e.select_all(cx).unwrap());
        })
        .unwrap();
        cx.update(|cx| {
            cx.update_window(w.into(), |root, window, cx| {
                root.downcast::<HttpWindow>().unwrap().update(cx, |s, cx| {
                    s.inputs[4].update(cx, |e, cx| e.set_text("new".into(), cx));
                });
                cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
                window.dispatch_keystroke(
                    Keystroke::parse(&format!("{}-{key}", command())).unwrap(),
                    cx,
                );
                assert_eq!(
                    cx.read_from_clipboard().unwrap().text().unwrap(),
                    "sentinel"
                );
            })
            .unwrap();
        });
    }
}
#[gpui::test]
fn search_and_other_window_do_not_mutate_current_response_close_retires_retained_owner(
    cx: &mut TestAppContext,
) {
    let fixture = fixed(200, "", b"ok".to_vec());
    let a = cx.add_window(|w, cx| HttpWindow::new(fixture.url.clone(), w, cx));
    let b = cx.add_window(|w, cx| HttpWindow::new(String::new(), w, cx));
    a.update(cx, |s, w, cx| s.activate(Control::Send, w, cx))
        .unwrap();
    tick(cx);
    a.update(cx, |s, w, cx| {
        assert!(s.ready(cx));
        s.activate(Control::Search, w, cx);
        assert!(s.ready(cx));
    })
    .unwrap();
    let retained = a.update(cx, |_, _, cx| cx.entity()).unwrap();
    a.update(cx, |_, w, _| {
        w.remove_window();
    })
    .unwrap();
    tick(cx);
    retained.update(cx, |s, cx| {
        assert!(s.closed);
        assert!(s.inputs.iter().all(|e| e.read(cx).text().is_empty()));
        assert!(!s.session.read(cx).can_copy());
        assert!(!s.session.read(cx).can_send());
        assert!(s.session.read(cx).draft.url.is_empty());
    });
    b.update(cx, |s, _, cx| {
        assert!(s.inputs[2].read(cx).text().is_empty());
        assert!(s.session.read(cx).result.is_none());
    })
    .unwrap();
    assert_eq!(fixture.count.load(Ordering::Acquire), 1);
}

#[gpui::test]
fn method_menu_owns_shortcuts_and_returns_focus_without_sending(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", b"ok".to_vec());
    let w = cx.add_window(|w, cx| HttpWindow::new(fixture.url.clone(), w, cx));
    w.update(cx, |s, w, cx| s.activate(Control::MethodMenu, w, cx))
        .unwrap();
    cx.simulate_keystrokes(w.into(), &format!("{}-enter", command()));
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
    cx.simulate_keystrokes(w.into(), "down enter");
    w.update(cx, |s, w, cx| {
        assert_eq!(s.inputs[1].read(cx).text(), "DELETE");
        assert!(!s.method_menu);
        assert!(
            s.controls
                .iter()
                .find(|(c, _)| *c == Control::MethodMenu)
                .unwrap()
                .1
                .is_focused(w)
        );
        s.activate(Control::MethodMenu, w, cx);
    })
    .unwrap();
    cx.simulate_keystrokes(w.into(), "down escape");
    w.update(cx, |s, w, _| {
        assert!(!s.method_menu);
        assert!(
            s.controls
                .iter()
                .find(|(c, _)| *c == Control::MethodMenu)
                .unwrap()
                .1
                .is_focused(w)
        );
    })
    .unwrap();
    tick(cx);
    assert_eq!(fixture.count.load(Ordering::Acquire), 0);
}

#[gpui::test]
fn full_bounded_response_is_copied_beyond_the_rendered_line_prefix(cx: &mut TestAppContext) {
    let fixture = fixed(200, "", vec![b'x'; http_request::MAX_RESPONSE_BYTES + 1]);
    let w = cx.add_window(|w, cx| HttpWindow::new(fixture.url.clone(), w, cx));
    w.update(cx, |s, w, cx| s.activate(Control::Send, w, cx))
        .unwrap();
    tick(cx);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        let response = s.session.read(cx).result.as_ref().unwrap();
        assert!(response.truncated());
        assert_eq!(response.retained_len(), http_request::MAX_RESPONSE_BYTES);
        let expected = response.text();
        assert!(expected.len() > http_request::MAX_RESPONSE_BYTES);
        assert_eq!(s.output.read(cx).text(), expected);
        s.copy(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), expected);
    })
    .unwrap();
}
