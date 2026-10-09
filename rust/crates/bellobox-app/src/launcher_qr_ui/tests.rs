use super::{Control, LauncherQrPreview};
use gpui::{EntityInputHandler, KeyDownEvent, Keystroke, TestAppContext};
fn key(name: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap(),
        is_held: false,
    }
}
#[gpui::test]
fn current_generation_alone_can_publish_copy_and_snapshot(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("original".into(), w, cx));
    view.update(cx, |v, _, cx| {
        let old = v.jobs.begin();
        let new = v.jobs.begin();
        v.publish(
            new,
            "original".into(),
            bellobox_core::qr::palette("original"),
            cx,
        );
        assert!(v.enabled(Control::Copy, cx));
        let snapshot = v.save_snapshot(cx).unwrap();
        v.editor.update(cx, |e, cx| e.set_text(String::new(), cx));
        v.changed(cx);
        assert_eq!(v.handoff(cx).unwrap(), "");
        assert!(!v.enabled(Control::Copy, cx));
        v.publish(
            old,
            "original".into(),
            bellobox_core::qr::palette("original"),
            cx,
        );
        v.publish(new, "original".into(), Err("late error".into()), cx);
        assert!(v.accepted.is_none());
        assert_ne!(v.status, "late error");
        assert_eq!(
            snapshot.as_ref(),
            &bellobox_core::qr::png("original").unwrap()
        );
    })
    .unwrap();
}
#[gpui::test]
fn retire_and_reactivate_cancel_pending_generations(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("retained".into(), w, cx));
    view.update(cx, |v, _, cx| {
        let old = v.jobs.begin();
        v.set_active(false, cx);
        v.publish(
            old,
            "retained".into(),
            bellobox_core::qr::palette("retained"),
            cx,
        );
        assert!(v.accepted.is_none());
        assert_eq!(v.handoff(cx).unwrap(), "retained");
        v.set_active(true, cx);
        let token = v.jobs.begin();
        v.retire(cx);
        v.publish(
            token,
            "retained".into(),
            bellobox_core::qr::palette("retained"),
            cx,
        );
        assert!(v.accepted.is_none());
        assert!(!v.enabled(Control::Copy, cx));
    })
    .unwrap();
}
#[gpui::test]
fn ime_arrows_return_and_tab_belong_to_retained_editor(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("draft".into(), w, cx));
    view.update(cx, |v, w, cx| {
        let search = cx.focus_handle();
        v.editor.read(cx).focus(w);
        for name in ["left", "right", "up", "down", "enter"] {
            assert!(v.handle_key(&key(name), &search, w, cx));
        }
        v.editor.update(cx, |e, cx| {
            e.replace_and_mark_text_in_range(None, "に", Some(1..1), w, cx)
        });
        assert!(v.handle_key(&key("tab"), &search, w, cx));
        assert!(v.editor_focused(w, cx));
        assert!(v.composing(w, cx));
        v.editor.update(cx, |e, cx| e.unmark_text(w, cx));
        assert!(v.handle_key(&key("tab"), &search, w, cx));
        assert!(v.controls[&Control::Paste].is_focused(w));
        assert!(v.handle_key(&key("shift-tab"), &search, w, cx));
        assert!(v.editor_focused(w, cx));
        assert!(v.handle_key(&key("shift-tab"), &search, w, cx));
        assert!(search.is_focused(w));
    })
    .unwrap();
}
#[gpui::test]
fn byte_limits_clear_copyability_without_truncating_handoff(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new(String::new(), w, cx));
    view.update(cx, |v, _, cx| {
        for text in [
            "x".repeat(2001),
            "界".repeat(667),
            "x".repeat(bellobox_core::MAX_PREVIEW_BYTES + 1),
        ] {
            v.editor.update(cx, |e, cx| e.set_text(text.clone(), cx));
            v.changed(cx);
            assert_eq!(v.handoff(cx).unwrap(), text);
            assert!(v.accepted.is_none());
            assert!(!v.enabled(Control::Copy, cx));
        }
    })
    .unwrap();
}

#[gpui::test]
fn save_dialog_cancel_then_edit_keeps_click_time_bytes_and_no_overwrite(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("qr.png");
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("click-time".into(), w, cx));
    cx.run_until_parked();
    view.update(cx, |v, _, cx| {
        let token = v.jobs.begin();
        v.publish(
            token,
            "click-time".into(),
            bellobox_core::qr::palette("click-time"),
            cx,
        );
        v.save(cx);
        assert!(v.save_pending);
    })
    .unwrap();
    cx.run_until_parked();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert!(!path.exists());
    view.update(cx, |v, _, cx| {
        assert!(!v.save_pending);
        v.save(cx);
        v.editor
            .update(cx, |e, cx| e.set_text("edited while choosing".into(), cx));
        v.changed(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bellobox_core::qr::png("click-time").unwrap()
    );
    view.update(cx, |v, _, cx| {
        assert!(!v.save_pending);
        assert!(!v.status.starts_with("Saved"));
        let token = v.jobs.begin();
        v.publish(
            token,
            "edited while choosing".into(),
            bellobox_core::qr::palette("edited while choosing"),
            cx,
        );
        v.save(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bellobox_core::qr::png("click-time").unwrap()
    );
    view.update(cx, |v, _, _| assert!(v.status.starts_with("Not saved:")))
        .unwrap();
}
#[gpui::test]
fn explicit_copy_and_empty_paste_use_current_image_only(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("current".into(), w, cx));
    view.update(cx, |v, w, cx| {
        let token = v.jobs.begin();
        v.publish(
            token,
            "current".into(),
            bellobox_core::qr::palette("current"),
            cx,
        );
        v.act(Control::Copy, w, cx);
        let item = cx.read_from_clipboard().unwrap();
        let gpui::ClipboardEntry::Image(image) = &item.entries()[0] else {
            panic!("expected PNG image");
        };
        assert_eq!(image.bytes(), bellobox_core::qr::png("current").unwrap());
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new()));
        v.act(Control::Paste, w, cx);
        assert_eq!(v.draft(cx), "current");
        assert!(v.status.contains("draft was kept"));
        v.act(Control::Clear, w, cx);
        v.changed(cx);
        assert!(v.save_snapshot(cx).is_none());
        v.act(Control::Copy, w, cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "");
    })
    .unwrap();
}
#[gpui::test]
fn held_clear_activation_cannot_type_into_newly_focused_editor(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("draft".into(), w, cx));
    view.update(cx, |v, w, cx| {
        let search = cx.focus_handle();
        for name in ["enter", "space"] {
            v.editor.update(cx, |e, cx| e.set_text("draft".into(), cx));
            v.controls[&Control::Clear].focus(w);
            assert!(v.handle_key(&key(name), &search, w, cx));
            assert_eq!(v.draft(cx), "");
            assert!(v.editor_focused(w, cx));
            let mut held = key(name);
            held.is_held = true;
            assert!(v.handle_key(&held, &search, w, cx));
            assert_eq!(v.draft(cx), "");
            assert_eq!(v.consume_release.as_deref(), Some(name));
            assert!(v.handle_key_up(
                &gpui::KeyUpEvent {
                    keystroke: held.keystroke
                },
                w,
                cx
            ));
            assert!(v.consume_release.is_none());
        }
    })
    .unwrap();
}
#[gpui::test]
fn global_input_limit_explains_disabled_open_and_preserves_draft(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new(String::new(), w, cx));
    view.update(cx, |v, _, cx| {
        let text = "x".repeat(bellobox_core::MAX_INPUT_BYTES + 1);
        v.editor.update(cx, |e, cx| e.set_text(text.clone(), cx));
        v.changed(cx);
        assert_eq!(v.draft(cx), text);
        assert!(v.handoff(cx).is_err());
        assert!(!v.enabled(Control::Open, cx));
        assert!(v.status.contains("500000"));
        assert!(v.status.contains("Clear or Paste"));
    })
    .unwrap();
}
#[gpui::test]
fn same_text_aba_rejects_old_success_and_error_then_recovers_at_byte_limit(
    cx: &mut TestAppContext,
) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("A".into(), w, cx));
    view.update(cx, |v, _, cx| {
        let first = v.jobs.begin();
        v.editor.update(cx, |e, cx| e.set_text("B".into(), cx));
        v.changed(cx);
        v.editor.update(cx, |e, cx| e.set_text("A".into(), cx));
        v.changed(cx);
        let current = v.jobs.begin();
        v.publish(current, "A".into(), bellobox_core::qr::palette("A"), cx);
        let current_image = v.accepted.as_ref().unwrap().export.clone();
        v.publish(first, "A".into(), Err("obsolete first A".into()), cx);
        assert!(std::sync::Arc::ptr_eq(
            &current_image,
            &v.accepted.as_ref().unwrap().export
        ));
        v.publish(first, "A".into(), bellobox_core::qr::palette("A"), cx);
        assert!(std::sync::Arc::ptr_eq(
            &current_image,
            &v.accepted.as_ref().unwrap().export
        ));
        for text in ["x".repeat(2000), format!("{}xx", "界".repeat(666))] {
            v.editor.update(cx, |e, cx| e.set_text(text.clone(), cx));
            v.changed(cx);
            let token = v.jobs.begin();
            v.publish(token, text.clone(), bellobox_core::qr::palette(&text), cx);
            assert!(v.enabled(Control::Copy, cx));
            assert_eq!(v.handoff(cx).unwrap(), text);
        }
    })
    .unwrap();
}
#[gpui::test]
fn density_toggle_retains_editor_identity_and_real_undo(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("draft".into(), w, cx));
    view.update(cx, |v, w, cx| v.editor.read(cx).focus(w))
        .unwrap();
    cx.run_until_parked();
    cx.simulate_input(view.into(), "X");
    cx.run_until_parked();
    view.update(cx, |v, w, cx| {
        assert!(v.current(cx).is_some());
        let editor = v.editor.entity_id();
        let compact = v.height();
        v.act(Control::Enlarge, w, cx);
        assert!(v.height() > compact);
        assert_eq!(v.editor.entity_id(), editor);
        v.act(Control::Enlarge, w, cx);
        assert_eq!(v.height(), compact);
        assert_eq!(v.editor.entity_id(), editor);
    })
    .unwrap();
    cx.simulate_keystrokes(
        view.into(),
        if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        },
    );
    view.update(cx, |v, _, cx| assert_eq!(v.draft(cx), "draft"))
        .unwrap();
}
#[gpui::test]
fn oversized_whitespace_heading_uses_byte_admission_before_scanning(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new(String::new(), w, cx));
    view.update(cx, |v, _, cx| {
        assert_eq!(v.title(cx), "Enter text to encode");
        for bytes in [
            2000,
            2001,
            bellobox_core::MAX_INPUT_BYTES + 1,
            8 * 1024 * 1024,
        ] {
            v.editor
                .update(cx, |e, cx| e.set_text(" ".repeat(bytes), cx));
            v.changed(cx);
            assert_eq!(v.draft(cx).len(), bytes);
            assert_eq!(
                v.title(cx),
                if bytes <= 2000 {
                    "Enter text to encode"
                } else {
                    "Not encodable"
                }
            );
            assert!(v.accepted.is_none());
        }
    })
    .unwrap();
}

#[test]
fn image_copy_capability_matches_pinned_actual_backend_contract() {
    for compositor in ["X11", "Wayland", "headless"] {
        assert!(
            super::image_copy_notice(compositor, false)
                .unwrap()
                .contains("Save…")
        );
    }
    // An empty name only enables the source-supported native macOS route.
    assert!(super::image_copy_notice("", true).is_none());
    assert!(super::image_copy_notice("", false).is_some());
    for native_macos in [false, true] {
        assert!(super::image_copy_notice("unknown compositor", native_macos).is_some());
    }
}

#[gpui::test]
fn unsupported_image_copy_never_changes_clipboard_or_claims_success(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("current".into(), w, cx));
    view.update(cx, |v, w, cx| {
        for compositor in ["X11", "Wayland", "headless"] {
            v.image_copy_notice = super::image_copy_notice(compositor, false);
            let token = v.jobs.begin();
            v.publish(
                token,
                "current".into(),
                bellobox_core::qr::palette("current"),
                cx,
            );
            assert!(v.current(cx).is_some());
            assert!(!v.enabled(Control::Copy, cx));
            assert!(v.enabled(Control::Save, cx));
            assert!(!v.tab_order(cx).contains(&v.controls[&Control::Copy]));
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("external sentinel".into()));
            v.act(Control::Copy, w, cx);
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("external sentinel")
            );
            assert!(!v.status.contains("copied"));
            assert!(!v.status.contains("Copy or save"));
            assert!(v.image_copy_notice.unwrap().contains("export PNG"));
        }
    })
    .unwrap();
}
#[gpui::test]
fn armed_save_is_cancelled_by_focus_or_row_change_before_release(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("save release".into(), w, cx));
    view.update(cx, |v, w, cx| {
        let token = v.jobs.begin();
        v.publish(
            token,
            "save release".into(),
            bellobox_core::qr::palette("save release"),
            cx,
        );
        let search = cx.focus_handle();
        for name in ["enter", "space"] {
            v.controls[&Control::Save].focus(w);
            v.handle_key(&key(name), &search, w, cx);
            assert!(v.keyboard_save_armed);
            assert!(!v.save_pending);
            search.focus(w);
            v.handle_key_up(
                &gpui::KeyUpEvent {
                    keystroke: gpui::Keystroke::parse(name).unwrap(),
                },
                w,
                cx,
            );
            assert!(!v.keyboard_save_armed);
            assert!(!v.save_pending);
        }
        v.controls[&Control::Save].focus(w);
        v.handle_key(&key("enter"), &search, w, cx);
        v.set_active(false, cx);
        v.handle_key_up(
            &gpui::KeyUpEvent {
                keystroke: gpui::Keystroke::parse("enter").unwrap(),
            },
            w,
            cx,
        );
        assert!(!v.keyboard_save_armed);
        assert!(!v.save_pending);
    })
    .unwrap();
}
#[gpui::test]
fn armed_save_rejects_edit_aba_and_retirement(cx: &mut TestAppContext) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("A".into(), w, cx));
    view.update(cx, |v, w, cx| {
        let search = cx.focus_handle();
        let token = v.jobs.begin();
        v.publish(token, "A".into(), bellobox_core::qr::palette("A"), cx);
        v.controls[&Control::Save].focus(w);
        v.handle_key(&key("enter"), &search, w, cx);
        assert!(v.keyboard_save_armed);
        v.editor.update(cx, |e, cx| e.set_text(String::new(), cx));
        v.changed(cx);
        v.editor.update(cx, |e, cx| e.set_text("A".into(), cx));
        v.changed(cx);
        let token = v.jobs.begin();
        v.publish(token, "A".into(), bellobox_core::qr::palette("A"), cx);
        v.handle_key_up(
            &gpui::KeyUpEvent {
                keystroke: gpui::Keystroke::parse("enter").unwrap(),
            },
            w,
            cx,
        );
        assert!(!v.save_pending);
        assert!(!v.keyboard_save_armed);
        v.controls[&Control::Save].focus(w);
        v.handle_key(&key("enter"), &search, w, cx);
        v.retire(cx);
        v.handle_key_up(
            &gpui::KeyUpEvent {
                keystroke: gpui::Keystroke::parse("enter").unwrap(),
            },
            w,
            cx,
        );
        assert!(!v.save_pending);
        assert!(!v.keyboard_save_armed);
    })
    .unwrap();
}
#[gpui::test]
fn mixed_mouse_or_programmatic_save_refuses_held_activation_without_replay(
    cx: &mut TestAppContext,
) {
    let view = cx.add_window(|w, cx| LauncherQrPreview::new("mixed activation".into(), w, cx));
    view.update(cx, |v, w, cx| {
        let token = v.jobs.begin();
        v.publish(
            token,
            "mixed activation".into(),
            bellobox_core::qr::palette("mixed activation"),
            cx,
        );
        let search = cx.focus_handle();
        for direct in [false, true] {
            v.controls[&Control::Save].focus(w);
            v.handle_key(&key("enter"), &search, w, cx);
            if direct {
                v.save(cx);
            } else {
                v.act(Control::Save, w, cx);
            }
            assert!(!v.save_pending);
            assert!(!v.keyboard_save_armed);
            assert!(v.status.contains("Release Enter/Space"));
            v.handle_key_up(
                &gpui::KeyUpEvent {
                    keystroke: gpui::Keystroke::parse("enter").unwrap(),
                },
                w,
                cx,
            );
            assert!(!v.save_pending);
        }
        v.act(Control::Save, w, cx);
        assert!(v.save_pending);
    })
    .unwrap();
    cx.run_until_parked();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    view.update(cx, |v, _, _| assert!(!v.save_pending)).unwrap();
}
