use super::*;
use gpui::{EntityInputHandler, Keystroke, TestAppContext};
use std::{sync::atomic::Ordering::SeqCst, time::Duration};
fn tick(cx: &mut TestAppContext, ms: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}
#[gpui::test]
fn mount_never_reads_clipboard_and_either_side_compares(cx: &mut TestAppContext) {
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
    let w = cx.add_window(|w, cx| CompareWindow::new(String::new(), w, cx));
    w.update(cx, |s, _, cx| {
        assert!(s.inputs.iter().all(|e| e.read(cx).text().is_empty()));
        assert!(!s.ready(cx));
        s.replace(1, "right\n".into(), cx);
        assert!(!s.ready(cx));
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        s.copy(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "+ right\n+ "
        );
        s.replace(0, "left".into(), cx);
        s.replace(1, String::new(), cx);
        assert!(!s.ready(cx));
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        s.copy(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "− left");
    })
    .unwrap();
}
#[gpui::test]
fn same_callback_edit_fences_copy_before_changed_subscription(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("old".into(), w, cx));
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        s.copy(cx);
        let before = cx.read_from_clipboard().unwrap().text().unwrap();
        s.inputs[0].update(cx, |e, cx| e.set_text("new".into(), cx));
        assert!(!s.ready(cx));
        s.copy(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), before);
        s.sync_inputs(false, cx);
        assert!(s.output.read(cx).text().is_empty());
        assert!(s.session.read(cx).result.is_none());
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        s.copy(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "− new");
    })
    .unwrap();
}

#[gpui::test]
fn dispatched_stale_result_copy_and_cut_are_fenced_before_changed(cx: &mut TestAppContext) {
    for key in ["c", "x"] {
        let w = cx.add_window(|w, cx| CompareWindow::new("old".into(), w, cx));
        tick(cx, 221);
        w.update(cx, |s, w, cx| {
            s.output.read(cx).focus(w);
            s.output.update(cx, |e, cx| e.select_all(cx).unwrap());
        })
        .unwrap();
        cx.simulate_keystrokes(
            w.into(),
            if cfg!(target_os = "macos") {
                "cmd-a cmd-c"
            } else {
                "ctrl-a ctrl-c"
            },
        );
        cx.update(|cx| assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "− old"));

        cx.update(|cx| {
            cx.update_window(w.into(), |root, window, cx| {
                let root = root.downcast::<CompareWindow>().unwrap();
                root.update(cx, |s, cx| {
                    s.inputs[0].update(cx, |e, cx| e.set_text("new".into(), cx));
                    assert!(!s.ready(cx));
                    // The queued Changed event has not cleared the old selection.
                    assert_eq!(s.output.read(cx).text(), "− old");
                });
                cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
                window.dispatch_keystroke(
                    Keystroke::parse(&format!(
                        "{}-{key}",
                        if cfg!(target_os = "macos") {
                            "cmd"
                        } else {
                            "ctrl"
                        }
                    ))
                    .unwrap(),
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
fn independent_drafts_and_memory_pin_survive_all_window_closes(cx: &mut TestAppContext) {
    let a = cx.add_window(|w, cx| CompareWindow::new("snapshot".into(), w, cx));
    let b = cx.add_window(|w, cx| CompareWindow::new(String::new(), w, cx));
    a.update(cx, |s, w, cx| {
        s.activate(Control::Pin, w, cx);
        s.replace(0, "edited afterward".into(), cx);
    })
    .unwrap();
    b.update(cx, |s, w, cx| {
        assert!(s.inputs[0].read(cx).text().is_empty());
        assert_eq!(s.session.read(cx).options, Default::default());
        s.activate(Control::UsePin, w, cx);
        assert_eq!(s.inputs[1].read(cx).text(), "snapshot");
        assert!(s.inputs[0].read(cx).text().is_empty());
    })
    .unwrap();
    let session = a
        .update(cx, |s, w, _| {
            w.remove_window();
            s.session.clone()
        })
        .unwrap();
    b.update(cx, |_, w, _| w.remove_window()).unwrap();
    cx.run_until_parked();
    session.update(cx, |s, _| assert!(!s.can_copy()));
    let c = cx.add_window(|w, cx| CompareWindow::new(String::new(), w, cx));
    c.update(cx, |s, w, cx| {
        s.activate(Control::UsePin, w, cx);
        assert_eq!(s.inputs[1].read(cx).text(), "snapshot");
    })
    .unwrap();
}
#[gpui::test]
fn no_pin_notice_preserves_second_draft_and_clear_is_side_specific(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("first".into(), w, cx));
    w.update(cx, |s, w, cx| {
        s.replace(1, "second".into(), cx);
        s.activate(Control::UsePin, w, cx);
        assert!(s.notice.contains("Pin a selection first"));
        assert_eq!(s.inputs[1].read(cx).text(), "second");
        s.activate(Control::Clear(0), w, cx);
        assert_eq!(s.inputs[1].read(cx).text(), "second");
        assert!(s.inputs[0].read(cx).text().is_empty());
        cx.write_to_clipboard(ClipboardItem::new_string("pasted".into()));
        s.activate(Control::Paste(0), w, cx);
        assert_eq!(s.inputs[0].read(cx).text(), "pasted");
        assert_eq!(s.inputs[1].read(cx).text(), "second");
        s.activate(Control::Paste(1), w, cx);
        assert_eq!(s.inputs[1].read(cx).text(), "pasted");
    })
    .unwrap();
}
#[gpui::test]
fn mode_and_whitespace_changes_preserve_editors_and_state(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("a   b".into(), w, cx));
    w.update(cx, |s, _, cx| s.replace(1, "a b".into(), cx))
        .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert_eq!(s.session.read(cx).result.as_ref().unwrap().removed, 1);
        let tokens = s.tokens(cx);
        s.activate(Control::Whitespace, w, cx);
        assert!(s.output.read(cx).text().is_empty());
        assert_eq!(s.tokens(cx), tokens);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert_eq!(s.session.read(cx).result.as_ref().unwrap().removed, 0);
        let tokens = s.tokens(cx);
        s.activate(Control::Words, w, cx);
        assert!(!s.enabled(Control::Whitespace, cx));
        s.activate(Control::Whitespace, w, cx);
        assert!(s.session.read(cx).options.ignore_whitespace);
        s.activate(Control::Json, w, cx);
        s.activate(Control::Lines, w, cx);
        assert_eq!(s.tokens(cx), tokens);
        assert!(s.session.read(cx).options.ignore_whitespace);
    })
    .unwrap();
}
#[gpui::test]
fn rejected_paste_latches_until_that_side_changes(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("first".into(), w, cx));
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert!(s.ready(cx));
        s.replace(0, "x".repeat(500001), cx);
        assert!(!s.ready(cx));
        assert!(s.output.read(cx).text().is_empty());
        assert!(s.session.read(cx).error.is_some());
        s.replace(1, "valid other side".into(), cx);
        s.activate(Control::Words, w, cx);
        s.activate(Control::Refresh, w, cx);
        assert!(!s.ready(cx));
        assert!(s.session.read(cx).error.is_some());
        s.replace(0, "recovered".into(), cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| assert!(s.ready(cx))).unwrap();
}
#[gpui::test]
fn rejected_editor_attempt_and_aggregate_limit_fence_old_success(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("first".into(), w, cx));
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        s.inputs[1].update(cx, |e, cx| e.set_text("x".repeat(500001), cx));
        assert!(!s.ready(cx));
        s.sync_inputs(false, cx);
        assert!(s.session.read(cx).error.is_some());
        assert_eq!(s.inputs[1].read(cx).text(), "");
        s.replace(1, "x".repeat(250000), cx);
        s.replace(0, "y".repeat(250001), cx);
        assert!(s.session.read(cx).error.is_some());
        assert!(!s.ready(cx));
        assert_eq!(s.inputs[0].read(cx).text().len(), 250001);
        s.replace(0, "fixed".into(), cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| assert!(s.ready(cx))).unwrap();
}
#[gpui::test]
fn held_completion_cannot_restore_output_after_edit_mode_cancel_or_close(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("old".into(), w, cx));
    let probe = w
        .update(cx, |s, _, cx| s.session.read(cx).worker_probe())
        .unwrap();
    probe.held.store(true, SeqCst);
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        s.replace(0, "new".into(), cx);
        s.replace(1, "other".into(), cx);
        s.activate(Control::Words, w, cx);
        assert!(!s.ready(cx));
        s.activate(Control::Cancel, w, cx);
        assert!(s.output.read(cx).text().is_empty());
        assert!(!s.session.read(cx).busy);
    })
    .unwrap();
    probe.held.store(false, SeqCst);
    tick(cx, 11);
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert!(!s.ready(cx));
        assert!(s.session.read(cx).result.is_none());
        s.activate(Control::Refresh, w, cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| assert!(s.ready(cx))).unwrap();
    assert_eq!(probe.starts.load(SeqCst), 2);
}
#[gpui::test]
fn json_error_recovers_without_stale_rows_or_copy(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("{\"x\":1}".into(), w, cx));
    w.update(cx, |s, w, cx| {
        s.replace(1, "{\"x\":2}".into(), cx);
        s.activate(Control::Json, w, cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        s.replace(1, "{\"x\":}".into(), cx);
        assert!(!s.ready(cx));
        assert!(s.output.read(cx).text().is_empty());
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.session.read(cx).error.is_some());
        assert!(!s.ready(cx));
        s.replace(1, "{\"x\":1}".into(), cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        assert_eq!(
            s.session.read(cx).result.as_ref().unwrap().copy_text,
            "  $[\"x\"] = 1"
        );
    })
    .unwrap();
}
#[gpui::test]
fn copy_has_every_row_even_when_result_exceeds_preview(cx: &mut TestAppContext) {
    let text = (0..3000)
        .map(|i| format!("line {i}: {}", "x".repeat(30)))
        .collect::<Vec<_>>()
        .join("\n");
    let expected = text
        .split('\n')
        .map(|l| format!("− {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    let w = cx.add_window(|w, cx| CompareWindow::new(text, w, cx));
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        s.copy(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), expected);
        assert!(
            s.output
                .read(cx)
                .text()
                .ends_with("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")
        );
    })
    .unwrap();
}
#[gpui::test]
fn composition_keeps_shortcuts_and_draft_actions_out_of_the_editor(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("first".into(), w, cx));
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        s.inputs[0].update(cx, |e, cx| {
            e.replace_and_mark_text_in_range(Some(0..1), "日", None, w, cx)
        });
        let before = s.inputs[0].read(cx).text().to_owned();
        s.activate(Control::Clear(0), w, cx);
        s.activate(Control::Words, w, cx);
        assert_eq!(s.inputs[0].read(cx).text(), before);
        assert!(s.inputs[0].read(cx).has_marked_text());
        assert_eq!(s.session.read(cx).options.mode, Mode::Lines);
        assert!(!s.ready(cx));
        s.key(
            &KeyDownEvent {
                keystroke: Keystroke::parse("ctrl-n").unwrap(),
                is_held: false,
            },
            w,
            cx,
        );
        s.inputs[0].update(cx, |e, cx| e.unmark_text(w, cx));
        s.sync_inputs(false, cx);
    })
    .unwrap();
}
#[gpui::test]
fn keyboard_focus_traverses_controls_and_activation_is_one_shot(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("first".into(), w, cx));
    w.update(cx, |s, w, cx| {
        let control = s
            .controls
            .iter()
            .find(|(c, _)| *c == Control::Whitespace)
            .unwrap()
            .1
            .clone();
        w.focus(&control);
        let key = |held| KeyDownEvent {
            keystroke: Keystroke::parse("space").unwrap(),
            is_held: held,
        };
        s.key(&key(false), w, cx);
        assert!(s.session.read(cx).options.ignore_whitespace);
        s.key(&key(true), w, cx);
        assert!(s.session.read(cx).options.ignore_whitespace);
        s.key(
            &KeyDownEvent {
                keystroke: Keystroke::parse("tab").unwrap(),
                is_held: false,
            },
            w,
            cx,
        );
        assert!(!control.is_focused(w));
        assert!(s.tab_handles(cx).iter().any(|h| h.is_focused(w)));
    })
    .unwrap();
}
#[gpui::test]
fn dispatched_shortcuts_copy_complete_result_new_window_and_close_independently(
    cx: &mut TestAppContext,
) {
    let w = cx.add_window(|w, cx| CompareWindow::new("first".into(), w, cx));
    w.update(cx, |s, _, cx| s.replace(1, "second".into(), cx))
        .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        s.inputs[0].read(cx).focus(w);
        s.inputs[0].update(cx, |e, cx| e.select_all(cx).unwrap());
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
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "− first\n+ second"
        )
    });
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        },
    );
    cx.update(|cx| assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "first"));
    let before = cx.update(|cx| cx.windows().len());
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-n"
        } else {
            "ctrl-n"
        },
    );
    let windows = cx.update(|cx| cx.windows());
    assert_eq!(windows.len(), before + 1);
    let fresh = windows
        .into_iter()
        .find(|candidate| candidate.window_id() != w.window_id())
        .unwrap()
        .downcast::<CompareWindow>()
        .unwrap();
    fresh
        .update(cx, |s, _, cx| {
            assert!(s.inputs.iter().all(|e| e.read(cx).text().is_empty()));
            assert_eq!(s.session.read(cx).options, Default::default());
        })
        .unwrap();
    cx.simulate_keystrokes(
        fresh.into(),
        if cfg!(target_os = "macos") {
            "cmd-w"
        } else {
            "ctrl-w"
        },
    );
    assert_eq!(cx.update(|cx| cx.windows().len()), before);
    w.update(cx, |s, _, cx| assert!(s.ready(cx))).unwrap();
}
#[gpui::test]
fn option_changes_preserve_input_undo_and_inline_selection_copy(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new("one two".into(), w, cx));
    w.update(cx, |s, w, cx| {
        s.inputs[0].read(cx).focus(w);
        s.inputs[0].update(cx, |e, cx| {
            e.replace_text_in_range(Some(0..3), "ONE", w, cx)
        });
        s.replace(1, "one three".into(), cx);
        s.activate(Control::Words, w, cx);
    })
    .unwrap();
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        },
    );
    w.update(cx, |s, _, cx| {
        assert_eq!(s.inputs[0].read(cx).text(), "one two")
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| w.focus(&s.words.read(cx).focus_handle(cx)))
        .unwrap();
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-a cmd-c"
        } else {
            "ctrl-a ctrl-c"
        },
    );
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "one two three "
        )
    });
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-shift-c"
        } else {
            "ctrl-shift-c"
        },
    );
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "  one\n− two\n+ three"
        )
    });
}
#[gpui::test]
fn dispatched_enter_inserts_literal_newline_in_both_independent_drafts(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new(String::new(), w, cx));
    cx.simulate_keystrokes(w.into(), "a enter b shift-enter c");
    w.update(cx, |s, w, cx| {
        assert_eq!(s.inputs[0].read(cx).text(), "a\nb\nc");
        s.inputs[1].read(cx).focus(w);
    })
    .unwrap();
    cx.simulate_keystrokes(w.into(), "a enter c enter");
    tick(cx, 221);
    // Drain the superseded first-key debounce, then the latest desired draft.
    tick(cx, 221);
    w.update(cx, |s, _, cx| {
        assert_eq!(s.inputs[0].read(cx).text(), "a\nb\nc");
        assert_eq!(s.inputs[1].read(cx).text(), "a\nc\n");
        assert_eq!(
            s.session.read(cx).result.as_ref().unwrap().copy_text,
            "  a\n− b\n  c\n+ "
        );
        assert!(s.ready(cx));
    })
    .unwrap();
}
#[gpui::test]
fn clipboard_tabs_crlf_and_result_selection_survive_noncompact_inputs(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| CompareWindow::new(String::new(), w, cx));
    let literal = "\tx\r\ny\tz\r";
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(literal.into())));
    cx.simulate_keystrokes(
        w.into(),
        if cfg!(target_os = "macos") {
            "cmd-v"
        } else {
            "ctrl-v"
        },
    );
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert_eq!(s.inputs[0].read(cx).text(), literal);
        assert_eq!(s.output.read(cx).text(), "− \tx\n− y\tz\n− ");
        s.output.read(cx).focus(w);
        s.output.update(cx, |e, cx| e.select_all(cx).unwrap());
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
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "− \tx\n− y\tz\n− "
        )
    });
    cx.simulate_keystrokes(w.into(), "enter q");
    w.update(cx, |s, _, cx| {
        assert_eq!(s.output.read(cx).text(), "− \tx\n− y\tz\n− ")
    })
    .unwrap();
}
