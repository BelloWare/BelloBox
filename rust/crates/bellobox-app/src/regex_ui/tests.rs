use super::*;
use gpui::{EntityInputHandler, Keystroke, TestAppContext};
use std::time::Duration;
fn tick(cx: &mut TestAppContext, ms: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}
#[gpui::test]
fn pattern_first_focus_defaults_and_retained_replacement_mode_projection(cx: &mut TestAppContext) {
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
    let w = cx.add_window(|w, cx| RegexWindow::new("é😀 ab".into(), w, cx));
    w.update(cx, |s, w, cx| {
        assert!(s.inputs[1].read(cx).focus_handle(cx).is_focused(w));
        assert_eq!(s.inputs[1].read(cx).text(), "");
        assert_eq!(s.inputs[2].read(cx).text(), "");
        assert!(!s.ready(cx));
        s.replace(1, "(ab)".into(), cx);
        s.replace(2, "[$1]".into(), cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert!(s.ready(cx));
        assert!(!s.enabled(Control::Chain, cx));
        s.copy(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "1. [4..<6] ab\n   $1: ab"
        );
        s.activate(Control::Replace, w, cx);
        assert!(!s.session.read(cx).busy);
        assert_eq!(s.output.read(cx).text(), "é😀 [ab]");
        s.activate(Control::Matches, w, cx);
        s.activate(Control::Replace, w, cx);
        assert_eq!(s.inputs[2].read(cx).text(), "[$1]");
        s.activate(Control::Chain, w, cx);
        assert_eq!(s.inputs[0].read(cx).text(), "é😀 [ab]");
        assert_eq!(s.inputs[1].read(cx).text(), "(ab)");
        assert_eq!(s.inputs[2].read(cx).text(), "[$1]");
        assert!(!s.ready(cx));
    })
    .unwrap();
}
#[gpui::test]
fn stale_copy_is_fenced_for_every_field_even_before_changed_callback(cx: &mut TestAppContext) {
    for side in 0..3 {
        let w = cx.add_window(|w, cx| RegexWindow::new("old".into(), w, cx));
        w.update(cx, |s, _, cx| s.replace(1, "old".into(), cx))
            .unwrap();
        tick(cx, 221);
        w.update(cx, |s, _, cx| {
            assert!(s.ready(cx));
            s.copy(cx);
            let before = cx.read_from_clipboard().unwrap().text().unwrap();
            s.inputs[side].update(cx, |e, cx| e.set_text("new".into(), cx));
            assert!(!s.ready(cx));
            s.copy(cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), before);
            s.sync_inputs(false, cx);
            assert!(s.output.read(cx).text().is_empty());
            assert!(s.highlighted.read(cx).text().is_empty());
        })
        .unwrap();
    }
}
#[gpui::test]
fn dispatched_stale_output_and_highlight_copy_cut_are_fenced(cx: &mut TestAppContext) {
    for key in ["c", "x"] {
        for highlight in [false, true] {
            let w = cx.add_window(|w, cx| RegexWindow::new("old".into(), w, cx));
            w.update(cx, |s, _, cx| s.replace(1, "old".into(), cx))
                .unwrap();
            tick(cx, 221);
            w.update(cx, |s, w, cx| {
                let e = if highlight { &s.highlighted } else { &s.output };
                e.read(cx).focus(w);
                e.update(cx, |e, cx| e.select_all(cx).unwrap());
            })
            .unwrap();
            cx.update(|cx| {
                cx.update_window(w.into(), |root, window, cx| {
                    root.downcast::<RegexWindow>().unwrap().update(cx, |s, cx| {
                        s.inputs[1].update(cx, |e, cx| e.set_text("new".into(), cx));
                        assert!(!s.ready(cx));
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
}
#[gpui::test]
fn rejected_paste_stays_sticky_until_that_field_is_repaired(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| RegexWindow::new("a".into(), w, cx));
    w.update(cx, |s, _, cx| s.replace(1, "a".into(), cx))
        .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        s.replace(1, "x".repeat(regex_inspection::PATTERN_LIMIT + 1), cx);
        assert!(!s.ready(cx));
        assert_eq!(s.inputs[1].read(cx).text(), "a");
        s.activate(Control::Extract, w, cx);
        s.activate(Control::IgnoreCase, w, cx);
        s.activate(Control::Refresh, w, cx);
        assert!(s.session.read(cx).result.is_none());
        assert!(!s.session.read(cx).busy);
        s.replace(0, "aa".into(), cx);
        assert!(!s.session.read(cx).busy);
        s.replace(1, "a".into(), cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, _, cx| assert!(s.ready(cx))).unwrap();
}
#[gpui::test]
fn mounted_new_window_does_not_inherit_drafts_and_close_cancels_only_owner(
    cx: &mut TestAppContext,
) {
    let a = cx.add_window(|w, cx| RegexWindow::new("a".into(), w, cx));
    let b = cx.add_window(|w, cx| RegexWindow::new(String::new(), w, cx));
    a.update(cx, |s, _, cx| s.replace(1, "a".into(), cx))
        .unwrap();
    b.update(cx, |s, _, cx| {
        assert!(s.inputs.iter().all(|e| e.read(cx).text().is_empty()));
        s.replace(0, "b".into(), cx);
        s.replace(1, "b".into(), cx);
    })
    .unwrap();
    tick(cx, 221);
    a.update(cx, |s, w, cx| {
        s.session.update(cx, |s, cx| s.retire(cx));
        w.remove_window();
    })
    .unwrap();
    b.update(cx, |s, _, cx| {
        assert!(s.ready(cx));
        assert_eq!(s.inputs[0].read(cx).text(), "b");
    })
    .unwrap();
}
#[gpui::test]
fn keyboard_tab_enter_escape_and_composition_keep_correct_owner(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| RegexWindow::new("a".into(), w, cx));
    cx.simulate_keystrokes(w.into(), "a enter");
    w.update(cx, |s, _, cx| assert_eq!(s.inputs[1].read(cx).text(), "a"))
        .unwrap();
    tick(cx, 221);
    cx.simulate_keystrokes(w.into(), "tab space");
    w.update(cx, |s, w, cx| {
        assert!(s.session.read(cx).options.ignore_case);
        assert!(
            s.controls
                .iter()
                .find(|(c, _)| *c == Control::IgnoreCase)
                .unwrap()
                .1
                .is_focused(w)
        );
    })
    .unwrap();
    cx.simulate_keystrokes(w.into(), "tab space");
    w.update(cx, |s, w, cx| {
        assert!(!s.session.read(cx).options.multiline);
        assert!(s.session.read(cx).options.ignore_case);
        assert!(
            s.controls
                .iter()
                .find(|(c, _)| *c == Control::Multiline)
                .unwrap()
                .1
                .is_focused(w)
        );
    })
    .unwrap();
    cx.simulate_keystrokes(w.into(), "enter");
    w.update(cx, |s, w, cx| {
        assert!(s.session.read(cx).options.multiline);
        assert!(
            s.controls
                .iter()
                .find(|(c, _)| *c == Control::Multiline)
                .unwrap()
                .1
                .is_focused(w)
        );
        s.inputs[1].read(cx).focus(w);
        s.inputs[1].update(cx, |e, cx| {
            e.replace_and_mark_text_in_range(None, "あ", None, w, cx)
        });
        assert!(s.composing(cx));
        let options = s.session.read(cx).options;
        s.activate(Control::Multiline, w, cx);
        assert_eq!(s.session.read(cx).options, options);
    })
    .unwrap();
    cx.simulate_keystrokes(w.into(), "escape");
    assert!(w.update(cx, |_, _, _| ()).is_ok());
}

#[gpui::test]
fn ime_each_field_suspends_until_same_byte_unmark_commit_or_cancel(cx: &mut TestAppContext) {
    for side in 0..3 {
        for finish in ["unmark", "commit", "cancel"] {
            let w = cx.add_window(|w, cx| RegexWindow::new("a".into(), w, cx));
            w.update(cx, |s, _, cx| {
                s.replace(1, ".".into(), cx);
                s.replace(2, "x".into(), cx);
            })
            .unwrap();
            tick(cx, 221);
            let original = w
                .update(cx, |s, w, cx| {
                    assert!(s.ready(cx));
                    let original = s.inputs[side].read(cx).text().to_owned();
                    s.inputs[side].update(cx, |e, cx| {
                        e.replace_and_mark_text_in_range(Some(0..1), "b", None, w, cx)
                    });
                    assert!(s.composing(cx));
                    assert!(!s.ready(cx));
                    original
                })
                .unwrap();
            tick(cx, 500);
            w.update(cx, |s, w, cx| {
                assert!(!s.session.read(cx).busy);
                assert!(s.session.read(cx).result.is_none());
                assert!(s.output.read(cx).text().is_empty());
                assert!(s.highlighted.read(cx).text().is_empty());
                let marked_token = s.inputs[side].read(cx).edit_token();
                s.inputs[side].update(cx, |e, cx| match finish {
                    "unmark" => e.unmark_text(w, cx),
                    "commit" => e.replace_text_in_range(None, "b", w, cx),
                    _ => e.replace_text_in_range(None, &original, w, cx),
                });
                assert!(!s.composing(cx));
                if finish == "unmark" {
                    assert_eq!(s.inputs[side].read(cx).edit_token(), marked_token);
                }
            })
            .unwrap();
            tick(cx, 221);
            w.update(cx, |s, _, cx| {
                assert!(!s.session.read(cx).busy);
                assert!(
                    s.session.read(cx).result.is_some(),
                    "side={side} finish={finish}"
                );
                assert_eq!(
                    s.inputs[side].read(cx).text(),
                    if finish == "cancel" { &original } else { "b" }
                );
            })
            .unwrap();
        }
    }
}
#[gpui::test]
fn immediate_copy_cut_during_each_field_composition_is_blocked(cx: &mut TestAppContext) {
    for side in 0..3 {
        for key in ["c", "x"] {
            let w = cx.add_window(|w, cx| RegexWindow::new("a".into(), w, cx));
            w.update(cx, |s, _, cx| {
                s.replace(1, ".".into(), cx);
                s.replace(2, "x".into(), cx);
            })
            .unwrap();
            tick(cx, 221);
            w.update(cx, |s, w, cx| {
                s.output.read(cx).focus(w);
                s.output.update(cx, |e, cx| e.select_all(cx).unwrap());
            })
            .unwrap();
            cx.update(|cx| {
                cx.update_window(w.into(), |root, window, cx| {
                    root.downcast::<RegexWindow>().unwrap().update(cx, |s, cx| {
                        s.inputs[side].update(cx, |e, cx| {
                            e.replace_and_mark_text_in_range(Some(0..1), "b", None, window, cx)
                        });
                        assert!(s.composing(cx));
                        assert!(!s.ready(cx));
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
}

#[gpui::test]
fn actual_window_removal_retires_retained_entity_and_held_worker(cx: &mut TestAppContext) {
    use std::sync::atomic::Ordering::SeqCst;
    let w = cx.add_window(|w, cx| RegexWindow::new("a".into(), w, cx));
    let (retained, probe) = w
        .update(cx, |s, _, cx| {
            let probe = s.session.read(cx).worker_probe();
            probe.held.store(true, SeqCst);
            s.replace(1, "a".into(), cx);
            (cx.entity(), probe)
        })
        .unwrap();
    tick(cx, 221);
    assert_eq!(probe.starts.load(SeqCst), 1);
    w.update(cx, |_, w, _| w.remove_window()).unwrap();
    tick(cx, 1);
    retained.update(cx, |s, cx| {
        assert!(s.closed);
        assert!(!s.ready(cx));
        assert!(!s.session.read(cx).busy);
        assert!(s.session.read(cx).result.is_none());
        assert!(s.output.read(cx).text().is_empty());
        // A retained entity's editor notification cannot resurrect closed work.
        s.inputs[0].update(cx, |e, cx| e.set_text("new".into(), cx));
    });
    probe.held.store(false, SeqCst);
    tick(cx, 500);
    retained.update(cx, |s, cx| {
        assert!(!s.ready(cx));
        assert!(!s.session.read(cx).busy);
        assert!(s.session.read(cx).result.is_none());
    });
    assert_eq!(probe.starts.load(SeqCst), 1);
}

#[gpui::test]
fn valid_highlight_selection_remains_copyable_when_extract_is_empty(cx: &mut TestAppContext) {
    let w = cx.add_window(|w, cx| RegexWindow::new("source".into(), w, cx));
    w.update(cx, |s, w, cx| {
        s.replace(1, "z".into(), cx);
        s.activate(Control::Extract, w, cx);
    })
    .unwrap();
    tick(cx, 221);
    w.update(cx, |s, w, cx| {
        assert!(s.current(cx));
        assert!(!s.ready(cx));
        assert!(!s.enabled(Control::Chain, cx));
        s.highlighted.read(cx).focus(w);
        s.highlighted.update(cx, |e, cx| e.select_all(cx).unwrap());
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
    cx.update(|cx| assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "source"));
}

#[gpui::test]
fn oversized_initial_and_pasted_drafts_never_mutate_source_or_other_window(
    cx: &mut TestAppContext,
) {
    let source = "x".repeat(regex_inspection::INPUT_LIMIT + 1);
    let untouched = source.clone();
    let oversized = cx.add_window(|w, cx| RegexWindow::new(source.clone(), w, cx));
    let other = cx.add_window(|w, cx| RegexWindow::new("other selection".into(), w, cx));
    oversized
        .update(cx, |s, _, cx| {
            // Intentional bounded admission difference from Swift: do not mount
            // an initial oversized draft. The caller still owns its exact input.
            assert!(s.inputs[0].read(cx).text().is_empty());
            assert!(
                s.session
                    .read(cx)
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("100000")
            );
            assert!(!s.ready(cx));
        })
        .unwrap();
    other
        .update(cx, |s, _, cx| {
            s.replace(0, source.clone(), cx);
            assert_eq!(s.inputs[0].read(cx).text(), "other selection");
            assert!(s.session.read(cx).error.is_some());
            assert!(!s.ready(cx));
        })
        .unwrap();
    assert_eq!(source, untouched);
    oversized
        .update(cx, |s, _, cx| {
            assert!(s.inputs[0].read(cx).text().is_empty())
        })
        .unwrap();
}
