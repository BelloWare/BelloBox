use super::*;
use gpui::{EntityInputHandler, TestAppContext};
#[gpui::test]
fn full_url_requires_build_and_blocks_same_callback_stale_copy(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new(EXAMPLE.into(), w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        assert!(!s.ready(cx));
        assert!(s.output.read(cx).text().is_empty());
        s.build(w, cx);
        assert!(s.ready(cx));
        s.fields[1].update(cx, |e, cx| e.set_text("changed.example".into(), cx));
        assert!(!s.ready(cx));
        s.copy(cx);
        s.build(w, cx);
        assert!(s.ready(cx));
        assert!(s.output.read(cx).text().contains("changed.example"));
    })
    .unwrap();
}
#[gpui::test]
fn independent_close_reopen_clear_and_chain(cx: &mut TestAppContext) {
    let a = cx.add_window(|w, cx| UrlWindow::new(EXAMPLE.into(), w, cx));
    let b = cx.add_window(|w, cx| UrlWindow::new("https://other.example/".into(), w, cx));
    cx.run_until_parked();
    a.update(cx, |s, w, cx| {
        s.build(w, cx);
        s.rows[0]
            .value
            .update(cx, |e, cx| e.set_text("full + value".into(), cx));
        s.build(w, cx);
        let output = s.output.read(cx).text().to_owned();
        s.replace_source(output, w, cx);
        assert!(!s.ready(cx));
        assert_eq!(
            s.draft.as_ref().unwrap().parameters[0].value,
            "full + value"
        );
        s.replace_source(String::new(), w, cx);
        assert!(s.draft.is_none());
        assert!(s.output.read(cx).text().is_empty());
        w.remove_window();
    })
    .unwrap();
    b.update(cx, |s, w, cx| {
        s.build(w, cx);
        assert_eq!(s.output.read(cx).text(), "https://other.example/");
    })
    .unwrap();
    let c = cx.add_window(|w, cx| UrlWindow::new(String::new(), w, cx));
    c.update(cx, |s, _, cx| {
        assert!(s.draft.is_none());
        assert!(s.input.read(cx).text().is_empty());
        assert!(!s.ready(cx));
    })
    .unwrap();
}
#[gpui::test]
fn all_3001_parameters_survive_paging_last_edit_and_stable_remove(cx: &mut TestAppContext) {
    let source = format!(
        "https://example.com/?{}",
        (0..3001)
            .map(|i| format!("x={i}"))
            .collect::<Vec<_>>()
            .join("&")
    );
    let w = cx.add_window(|w, cx| UrlWindow::new(source, w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        assert_eq!(s.rows.len(), PAGE);
        s.navigate(usize::MAX, w, cx);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].id, 3000);
        s.rows[0]
            .value
            .update(cx, |e, cx| e.set_text("last +".into(), cx));
        s.build(w, cx);
        assert!(s.output.read(cx).text().ends_with("x=last%20+"));
        assert_eq!(s.draft.as_ref().unwrap().parameters.len(), 3001);
        s.navigate(0, w, cx);
        assert_eq!(s.rows.len(), PAGE);
        s.build(w, cx);
        assert!(s.output.read(cx).text().ends_with("x=last%20+"));
        let id = s.draft.as_mut().unwrap().add().unwrap();
        s.draft.as_mut().unwrap().remove(2);
        assert_eq!(s.draft.as_ref().unwrap().parameters[2].id, 3);
        assert!(id > 3000);
    })
    .unwrap();
}
#[gpui::test]
fn source_typing_is_explicit_inspection_and_rejected_paste_latches(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new(EXAMPLE.into(), w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        s.build(w, cx);
        s.input
            .update(cx, |e, cx| e.set_text("https://new.example/".into(), cx));
        assert!(!s.ready(cx));
        s.build(w, cx);
        assert!(!s.ready(cx));
        s.inspect(w, cx);
        s.build(w, cx);
        assert!(s.ready(cx));
        s.replace_source("x".repeat(500001), w, cx);
        assert!(!s.ready(cx));
        s.fields[1].update(cx, |e, cx| e.set_text("edited.example".into(), cx));
        s.build(w, cx);
        assert!(!s.ready(cx));
        s.replace_source(EXAMPLE.into(), w, cx);
        s.build(w, cx);
        assert!(s.ready(cx));
        s.rows[0]
            .value
            .update(cx, |e, cx| e.set_text("x".repeat(500001), cx));
        assert!(!s.ready(cx));
        s.build(w, cx);
        assert!(!s.ready(cx));
        s.replace_source(EXAMPLE.into(), w, cx);
        s.build(w, cx);
        assert!(s.ready(cx));
    })
    .unwrap();
}
#[gpui::test]
fn aggregate_edits_are_bounded_without_truncation(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new(EXAMPLE.into(), w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        s.fields[3].update(cx, |e, cx| {
            e.set_text(format!("/{}", "a".repeat(260000)), cx)
        });
        s.rows[0]
            .value
            .update(cx, |e, cx| e.set_text("b".repeat(260000), cx));
        s.build(w, cx);
        assert!(!s.ready(cx));
        assert_eq!(s.rows[0].value.read(cx).text().len(), 260000);
        assert!(s.status.contains("500000"));
    })
    .unwrap();
}
#[gpui::test]
fn composing_row_blocks_navigation_build_and_source_replacement(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new(EXAMPLE.into(), w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        s.build(w, cx);
        let id = s.rows[0].id;
        s.rows[0].value.update(cx, |e, cx| {
            e.replace_and_mark_text_in_range(Some(0..3), "日", None, w, cx)
        });
        assert!(!s.ready(cx));
        s.navigate(1, w, cx);
        assert_eq!(s.rows[0].id, id);
        s.build(w, cx);
        assert!(!s.ready(cx));
        s.replace_source(String::new(), w, cx);
        assert!(s.draft.is_some());
        assert!(s.rows[0].value.read(cx).has_marked_text());
        s.rows[0].value.update(cx, |e, cx| e.unmark_text(w, cx));
        s.build(w, cx);
        assert!(s.ready(cx));
        assert!(s.output.read(cx).text().contains("%E6%97%A5"));
    })
    .unwrap();
}
#[gpui::test]
fn mount_does_not_read_clipboard_and_copy_is_complete(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(
            "synthetic clipboard sentinel".into(),
        ))
    });
    let source = format!("https://example.com/?value={}", "a".repeat(70000));
    let w = cx.add_window(|w, cx| UrlWindow::new(source.clone(), w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        assert_eq!(s.input.read(cx).text(), source);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "synthetic clipboard sentinel"
        );
        s.copy(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "synthetic clipboard sentinel"
        );
        s.build(w, cx);
        s.copy(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), source);
    })
    .unwrap();
}
#[gpui::test]
fn oversized_mount_is_rejected_with_visible_error_and_no_old_draft(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new("x".repeat(500001), w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        assert!(s.draft.is_none());
        assert!(s.rejected);
        assert!(s.status.contains("500000"));
        s.build(w, cx);
        assert!(!s.ready(cx));
        s.replace_source(EXAMPLE.into(), w, cx);
        s.build(w, cx);
        assert!(s.ready(cx));
    })
    .unwrap();
}
#[gpui::test]
fn page_remount_cannot_aba_a_built_token_before_queued_notifications(cx: &mut TestAppContext) {
    let source = format!(
        "https://example.com/?{}",
        (0..16)
            .map(|i| format!("x={i}"))
            .collect::<Vec<_>>()
            .join("&")
    );
    let w = cx.add_window(|w, cx| UrlWindow::new(source, w, cx));
    cx.run_until_parked();
    w.update(cx, |s, w, cx| {
        s.build(w, cx);
        assert!(s.ready(cx));
        s.rows[0]
            .value
            .update(cx, |e, cx| e.set_text("edited".into(), cx));
        s.navigate(1, w, cx);
        assert_eq!(s.rows.len(), 8);
        assert!(!s.ready(cx));
        assert!(s.built.is_none());
        assert!(s.output.read(cx).text().is_empty());
        s.copy(cx);
        s.build(w, cx);
        assert!(s.ready(cx));
        assert!(s.output.read(cx).text().contains("x=edited"));
        s.navigate(0, w, cx);
        assert!(s.ready(cx));
    })
    .unwrap();
}
#[gpui::test]
fn focused_editor_build_copy_shortcuts_and_plain_copy_ownership(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new(EXAMPLE.into(), w, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-enter"
        } else {
            "ctrl-enter"
        },
    );
    w.update(cx, |s, w, cx| {
        assert!(s.ready(cx));
        s.fields[1].read(cx).focus(w);
        s.fields[1].update(cx, |e, cx| e.select_all(cx).unwrap());
    })
    .unwrap();
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-shift-c"
        } else {
            "ctrl-shift-c"
        },
    );
    w.update(cx, |s, w, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            s.output.read(cx).text()
        );
        s.fields[1].read(cx).focus(w);
        s.fields[1].update(cx, |e, cx| e.select_all(cx).unwrap());
    })
    .unwrap();
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        },
    );
    w.update(cx, |_, _, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "example.com"
        )
    })
    .unwrap();
}
#[gpui::test]
fn empty_and_failed_inspection_tab_never_focus_hidden_fields(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| UrlWindow::new(String::new(), w, cx));
    cx.run_until_parked();
    for key in ["tab", "shift-tab"] {
        cx.simulate_keystrokes(w.into(), key);
        w.update(cx, |s, w, cx| {
            assert!(s.input.read(cx).focus_handle(cx).is_focused(w));
            assert!(
                s.fields
                    .iter()
                    .all(|e| !e.read(cx).focus_handle(cx).is_focused(w))
            );
        })
        .unwrap();
    }
    w.update(cx, |s, w, cx| s.replace_source("invalid".into(), w, cx))
        .unwrap();
    cx.run_until_parked();
    cx.simulate_keystrokes(w.into(), "tab");
    w.update(cx, |s, w, cx| {
        assert!(s.input.read(cx).focus_handle(cx).is_focused(w))
    })
    .unwrap();
}
