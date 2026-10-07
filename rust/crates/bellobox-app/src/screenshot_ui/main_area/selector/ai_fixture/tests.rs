//! Real inline coordinator/selector/editor tests, with no capture and only the
//! sealed numeric listener. Holds retain actual prepared or parsed resources.
use super::*;
use crate::screenshot_ui::{ScreenshotEditor, ai_ocr::TEST_LOCK};
use bellobox_core::screenshot::{AnnotationTool, selection::SelectionHandle};
use gpui::{Entity, TestAppContext, VisualTestContext, WindowHandle};

fn requester(cx: &mut TestAppContext) -> AnyWindowHandle {
    cx.add_window(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus);
        CaptureChooser {
            busy: false,
            status: String::new(),
            jobs: Default::default(),
            focus,
        }
    })
    .into()
}
fn open(
    fixed: bool,
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    WindowHandle<MainAreaSelector>,
    Entity<ScreenshotEditor>,
) {
    let requester = requester(cx);
    cx.update(|cx| begin(requester, cx, fixed).unwrap());
    cx.run_until_parked();
    let selector = cx.read(|cx| {
        cx.global::<Coordinator>()
            .active
            .as_ref()
            .unwrap()
            .transaction
            .selector
            .unwrap()
            .downcast::<MainAreaSelector>()
            .unwrap()
    });
    selector
        .update(cx, |view, window, _| {
            assert!(view.presented);
            view.viewport
                .set(Bounds::new(point(px(0.), px(0.)), size(px(960.), px(540.))));
            window.activate_window();
        })
        .unwrap();
    cx.run_until_parked();
    if !fixed {
        selector
            .update(cx, |view, window, cx| {
                view.mouse_down(&down(80., 60.), window, cx);
                view.mouse_up(&up(880., 480.), window, cx);
                assert!(view.locked.is_some());
            })
            .unwrap();
        cx.run_until_parked();
    }
    let editor = cx.read(|cx| selector.read(cx).unwrap().editor.clone().unwrap());
    (requester, selector, editor)
}
fn down(x: f32, y: f32) -> MouseDownEvent {
    MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(x), px(y)),
        click_count: 1,
        ..Default::default()
    }
}
fn up(x: f32, y: f32) -> MouseUpEvent {
    MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(x), px(y)),
        click_count: 1,
        ..Default::default()
    }
}
fn key(value: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: gpui::Keystroke::parse(value).unwrap(),
        is_held: false,
    }
}
fn start(
    selector: WindowHandle<MainAreaSelector>,
    editor: &Entity<ScreenshotEditor>,
    cx: &mut TestAppContext,
) {
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| view.request_ai_ocr(window, cx))
        })
        .unwrap();
    cx.run_until_parked();
    editor.read_with(cx, |view, _| {
        assert!(view.ai_ocr.modal(), "{}", view.status)
    });
}
fn approve(
    selector: WindowHandle<MainAreaSelector>,
    editor: &Entity<ScreenshotEditor>,
    cx: &mut TestAppContext,
) {
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| view.approve_ai_ocr(window, cx))
        })
        .unwrap();
    cx.run_until_parked();
}
fn close(selector: WindowHandle<MainAreaSelector>, cx: &mut TestAppContext) {
    selector
        .update(cx, |view, window, cx| view.retire(window, cx))
        .unwrap();
    cx.run_until_parked();
}
fn drained(cx: &TestAppContext) {
    cx.read(|cx| {
        assert!(cx.global::<Coordinator>().active.is_none());
        assert!(!cx.global::<NativeCaptureVisibility>().busy);
    });
}
fn busy_rejects(requester: AnyWindowHandle, fixed: bool, cx: &mut TestAppContext) {
    cx.update(|cx| {
        let run = cx.global::<Coordinator>().active.as_ref().unwrap();
        assert!(run.transaction.cancelled());
        assert!(run.transaction.selector.is_none());
        assert_eq!(run.transaction.owned_worker_count(), 1);
        assert!(!run.transaction.can_release());
        assert!(cx.global::<NativeCaptureVisibility>().busy);
        assert_eq!(
            begin(requester, cx, fixed).unwrap_err(),
            "Another capture is still finishing."
        );
    });
}

#[gpui::test]
fn inline_generated_area_and_fixed_window_use_same_consent_transport_and_reader(
    cx: &mut TestAppContext,
) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (_, selector, editor) = open(fixed, cx);
        let windows = cx.read(|cx| cx.windows());
        let permit = editor.read_with(cx, |view, _| {
            assert_eq!(view.inline.as_ref().unwrap().adjustable(), !fixed);
            assert_eq!(view.session.document().dimensions(), (960, 540));
            assert_eq!(
                view.session.document().visible_dimensions(),
                if fixed { (960, 540) } else { (800, 420) }
            );
            let permit = view.ai_test_permit();
            assert!(permit.check_document(view.session.document()));
            permit
        });
        start(selector, &editor, cx);
        assert_eq!(fixture::request_count(&permit), 0);
        assert!(
            cx.read(|cx| cx.windows()) == windows,
            "consent remains in the existing overlay"
        );
        approve(selector, &editor, cx);
        assert_eq!(fixture::request_count(&permit), 1);
        editor.update(cx, |view, cx| {
            let text = view
                .reader_text(false)
                .expect("actual parsed result")
                .to_owned();
            assert_eq!(text, "Synthetic OCR result\nGenerated image only");
            view.copy_reader(false, cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), text);
            assert_eq!(
                view.reader_text(true),
                Some("# Synthetic OCR result\nGenerated image only")
            );
        });
        close(selector, cx);
        drained(cx);
    }
}

#[gpui::test]
fn parsed_completion_then_disposal_keep_real_coordinator_busy_after_editor_destruction(
    cx: &mut TestAppContext,
) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (requester, selector, editor) = open(fixed, cx);
        let (completion, disposal, permit) = editor.update(cx, |view, _| {
            (
                view.ai_test_pause_completion(),
                view.ai_test_pause_disposal(),
                view.ai_test_permit(),
            )
        });
        start(selector, &editor, cx);
        approve(selector, &editor, cx);
        assert!(
            completion.is_waiting(),
            "real HTTP/parser reached held completion"
        );
        assert_eq!(fixture::request_count(&permit), 1);
        assert!(editor.read_with(cx, |view, _| view.ai_ocr_busy()));
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
            "must stay unchanged".into(),
        ));
        let weak = editor.downgrade();
        if fixed {
            selector
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
            cx.run_until_parked();
        } else {
            close(selector, cx);
        }
        drop(editor);
        cx.run_until_parked();
        assert!(weak.upgrade().is_none(), "actual editor entity retired");
        busy_rejects(requester, fixed, cx);
        completion.release();
        cx.run_until_parked();
        assert!(
            disposal.is_waiting(),
            "owned bytes are still pending actual drop"
        );
        busy_rejects(requester, fixed, cx);
        disposal.release();
        cx.run_until_parked();
        drained(cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("must stay unchanged")
        );
        let (_, successor, _) = open(fixed, cx);
        close(successor, cx);
        drained(cx);
    }
}

#[gpui::test]
fn canceling_inline_modal_holds_capture_until_real_disposal_and_reopen(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (requester, selector, editor) = open(fixed, cx);
        let disposal = editor.update(cx, |view, _| view.ai_test_pause_disposal());
        let permit = editor.read_with(cx, |view, _| view.ai_test_permit());
        start(selector, &editor, cx);
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| {
                    assert!(view.ai_modal_key(&key("escape"), window, cx));
                    view.request_ai_ocr(window, cx);
                    assert!(!view.ai_ocr.modal());
                })
            })
            .unwrap();
        cx.run_until_parked();
        assert!(disposal.is_waiting());
        assert_eq!(fixture::request_count(&permit), 0);
        close(selector, cx);
        drop(editor);
        cx.run_until_parked();
        busy_rejects(requester, fixed, cx);
        disposal.release();
        cx.run_until_parked();
        drained(cx);
    }
}

#[gpui::test]
fn inline_file_worker_retains_guard_after_save_ui_entity_drops(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (requester, selector, editor) = open(fixed, cx);
        start(selector, &editor, cx);
        approve(selector, &editor, cx);
        let pause = editor.update(cx, |view, cx| {
            let pause = view.ai_test_pause_save();
            view.save_reader(cx);
            pause
        });
        assert!(cx.did_prompt_for_new_path());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("must-not-publish.txt");
        cx.simulate_new_path_selection(|_| Some(path.clone()));
        cx.run_until_parked();
        assert!(
            pause.is_waiting(),
            "actual file worker owns text and inline guard"
        );
        let weak = editor.downgrade();
        close(selector, cx);
        drop(editor);
        cx.run_until_parked();
        assert!(weak.upgrade().is_none());
        busy_rejects(requester, fixed, cx);
        assert!(!path.exists());
        pause.release();
        cx.run_until_parked();
        drained(cx);
        assert!(
            !path.exists(),
            "close cancellation suppresses file publication"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[gpui::test]
fn inline_modal_shields_pointer_tab_enter_escape_and_area_handles(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (_, selector, editor) = open(fixed, cx);
        let permit = editor.read_with(cx, |view, _| view.ai_test_permit());
        start(selector, &editor, cx);
        let selection = editor.read_with(cx, |view, _| view.inline_selection());
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| {
                    view.begin_selection_adjustment(
                        Some(SelectionHandle::TopLeft),
                        point(px(selection.x), px(selection.y)),
                        cx,
                    );
                    assert!(view.inline.as_ref().unwrap().adjustment.is_none());
                    view.mouse_down(&down(selection.x + 10., selection.y + 10.), window, cx);
                    assert!(view.gesture.is_none());
                    assert!(view.ai_modal_key(&key("tab"), window, cx));
                    assert!(view.ai_modal_key(&key("shift-tab"), window, cx));
                    assert!(
                        view.ai_modal_key(&key("enter"), window, cx),
                        "default selection is cancel"
                    );
                })
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(fixture::request_count(&permit), 0);
        start(selector, &editor, cx);
        let mut visual = VisualTestContext::from_window(selector.into(), cx);
        let root = selector.root(cx).unwrap();
        visual.draw(point(px(0.), px(0.)), size(px(960.), px(540.)), |_, _| root);
        visual.simulate_mouse_down(
            point(px(selection.x + 1.), px(selection.y + 1.)),
            MouseButton::Left,
            Default::default(),
        );
        editor.read_with(cx, |view, _| {
            assert_eq!(view.inline_selection(), selection);
            assert!(view.inline.as_ref().unwrap().adjustment.is_none());
            assert!(view.gesture.is_none());
            assert!(view.ai_ocr.modal());
        });
        visual.simulate_keystrokes("escape");
        cx.run_until_parked();
        editor.read_with(cx, |view, _| assert!(!view.ai_ocr.modal()));
        assert_eq!(fixture::request_count(&permit), 0);
        close(selector, cx);
        drained(cx);
    }
}

#[gpui::test]
fn inline_privacy_gestures_clear_actual_results_before_revision_and_undo_cannot_revive(
    cx: &mut TestAppContext,
) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (_, selector, editor) = open(fixed, cx);
        for tool in [
            AnnotationTool::Crop,
            AnnotationTool::Blur,
            AnnotationTool::Eraser,
        ] {
            start(selector, &editor, cx);
            approve(selector, &editor, cx);
            selector
                .update(cx, |_, window, cx| {
                    editor.update(cx, |view, cx| {
                        assert!(view.reader_text(false).is_some());
                        let selection = view.inline_selection();
                        view.image_bounds.set(Bounds::new(
                            point(px(selection.x), px(selection.y)),
                            size(px(selection.width), px(selection.height)),
                        ));
                        view.tool = tool;
                        let revision = view.session.revision();
                        view.mouse_down(&down(selection.x + 30., selection.y + 30.), window, cx);
                        assert_eq!(view.session.revision(), revision);
                        assert!(view.reader_text(false).is_none());
                        assert!(view.ocr.read(cx).text().is_empty());
                        view.request_ai_ocr(window, cx);
                        assert!(!view.ai_ocr.modal());
                        view.mouse_up(&up(selection.x + 80., selection.y + 80.), window, cx);
                        view.apply_history(false, cx);
                        assert!(view.reader_text(false).is_none());
                    })
                })
                .unwrap();
            cx.run_until_parked();
        }
        if !fixed {
            start(selector, &editor, cx);
            approve(selector, &editor, cx);
            editor.update(cx, |view, cx| {
                let selection = view.inline_selection();
                let revision = view.session.revision();
                view.begin_selection_adjustment(
                    Some(SelectionHandle::TopLeft),
                    point(px(selection.x), px(selection.y)),
                    cx,
                );
                assert!(view.inline.as_ref().unwrap().adjustment.is_some());
                assert_eq!(view.session.revision(), revision);
                assert!(view.reader_text(false).is_none());
                view.apply_history(false, cx);
                assert!(view.reader_text(false).is_none());
            });
        }
        close(selector, cx);
        drained(cx);
    }
}

#[gpui::test]
fn generated_preparation_owns_capture_after_close_until_real_preview_drop(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (requester, selector, editor) = open(fixed, cx);
        let (preparation, disposal, permit) = editor.update(cx, |view, _| {
            (
                view.ai_test_pause_preparation(),
                view.ai_test_pause_disposal(),
                view.ai_test_permit(),
            )
        });
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| view.request_ai_ocr(window, cx))
            })
            .unwrap();
        cx.run_until_parked();
        assert!(preparation.is_waiting());
        assert_eq!(fixture::request_count(&permit), 0);
        editor.read_with(cx, |view, _| {
            assert!(view.ai_ocr_busy());
            assert!(!view.ai_ocr.modal());
        });
        close(selector, cx);
        drop(editor);
        cx.run_until_parked();
        busy_rejects(requester, fixed, cx);
        preparation.release();
        cx.run_until_parked();
        assert!(disposal.is_waiting());
        busy_rejects(requester, fixed, cx);
        disposal.release();
        cx.run_until_parked();
        drained(cx);
        assert_eq!(fixture::request_count(&permit), 0);
    }
}

#[gpui::test]
fn dropped_file_awaiter_does_not_retire_real_synced_writer_guard(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (requester, selector, editor) = open(fixed, cx);
        start(selector, &editor, cx);
        approve(selector, &editor, cx);
        let id = cx.read(|cx| {
            cx.global::<Coordinator>()
                .active
                .as_ref()
                .unwrap()
                .transaction
                .id
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("must-not-publish.txt");
        let held = editor.update(cx, |view, cx| view.ai_test_held_save(path.clone(), cx));
        held.entered
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        drop(held.result);
        cx.read(|cx| {
            assert_eq!(
                cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .owned_worker_count(),
                1
            )
        });
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "real staged file was written and synced"
        );
        close(selector, cx);
        drop(editor);
        cx.run_until_parked();
        busy_rejects(requester, fixed, cx);
        cx.read(|cx| {
            assert_eq!(
                cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .owned_worker_count(),
                1
            )
        });
        held.release.send(()).unwrap();
        held.finished
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        // Observe physical retirement BEFORE any foreground completion callback.
        cx.read(|cx| {
            assert_eq!(
                cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .owned_worker_count(),
                0
            )
        });
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        cx.update(|cx| owned_editor_work_drained(Some(id), cx));
        cx.run_until_parked();
        drained(cx);
    }
}

#[gpui::test]
fn application_shutdown_retires_inline_modal_owners_without_ui_completion(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let mut application = cx.new_app();
        let (_, selector, editor) = open(fixed, &mut application);
        start(selector, &editor, &mut application);
        let permit = editor.read_with(&application, |view, _| view.ai_test_permit());
        application.read(|cx| {
            assert_eq!(
                cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .owned_worker_count(),
                1
            )
        });
        application.quit();
        application.read(|cx| {
            assert_eq!(
                cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .owned_worker_count(),
                0
            )
        });
        editor.read_with(&application, |view, _| {
            assert!(!view.ai_ocr_busy());
            assert!(!view.ai_ocr.modal());
            assert!(view.reader_text(false).is_none());
        });
        assert_eq!(fixture::request_count(&permit), 0);
    }
}

#[gpui::test]
fn cancellation_between_snapshot_and_owned_admission_keeps_actual_prepared_pixels_counted(
    cx: &mut TestAppContext,
) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (requester, selector, editor) = open(fixed, cx);
        let cancellation = cx.read(|cx| {
            cx.global::<Coordinator>()
                .active
                .as_ref()
                .unwrap()
                .transaction
                .cancellation
                .clone()
        });
        assert!(!cancellation.load(Ordering::Acquire));
        let (preparation, disposal, permit) = editor.update(cx, |view, _| {
            view.ai_test_cancel_before_count(cancellation.clone());
            (
                view.ai_test_pause_preparation(),
                view.ai_test_pause_disposal(),
                view.ai_test_permit(),
            )
        });
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| view.request_ai_ocr(window, cx));
            })
            .unwrap();
        cx.run_until_parked();
        assert!(
            cancellation.load(Ordering::Acquire),
            "actual transaction flag flipped in the admission gap"
        );
        assert!(
            preparation.is_waiting(),
            "real PNG and preview exist before the held worker returns"
        );
        cx.read(|cx| {
            let run = cx.global::<Coordinator>().active.as_ref().unwrap();
            assert_eq!(run.transaction.owned_worker_count(), 1);
            assert!(cx.global::<NativeCaptureVisibility>().busy);
        });
        editor.read_with(cx, |view, _| {
            assert!(view.ai_ocr_busy());
            assert!(!view.ai_ocr.modal());
            assert!(view.reader_text(false).is_none());
        });
        assert_eq!(fixture::request_count(&permit), 0);
        close(selector, cx);
        drop(editor);
        cx.run_until_parked();
        busy_rejects(requester, fixed, cx);
        preparation.release();
        cx.run_until_parked();
        assert!(disposal.is_waiting());
        busy_rejects(requester, fixed, cx);
        assert_eq!(fixture::request_count(&permit), 0);
        disposal.release();
        cx.run_until_parked();
        drained(cx);
        assert_eq!(fixture::request_count(&permit), 0);
    }
}

#[gpui::test]
fn stale_inline_owner_cannot_borrow_missing_or_successor_transaction(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (_, selector, editor) = open(fixed, cx);
        editor.update(cx, |view, cx| {
            assert!(view.ai_test_owned_admission_available(cx))
        });
        let original = editor.read_with(cx, |view, _| view.inline.as_ref().unwrap().id);
        close(selector, cx);
        drained(cx);
        editor.update(cx, |view, cx| {
            assert!(
                !view.ai_test_owned_admission_available(cx),
                "missing transaction must not become an uncounted inline owner"
            )
        });
        let (_, successor, successor_editor) = open(fixed, cx);
        cx.read(|cx| {
            let run = cx.global::<Coordinator>().active.as_ref().unwrap();
            assert_ne!(run.transaction.id, original);
            assert_eq!(run.transaction.owned_worker_count(), 0);
        });
        editor.update(cx, |view, cx| {
            assert!(
                !view.ai_test_owned_admission_available(cx),
                "stale inline ID must not attach to its successor"
            )
        });
        successor_editor.update(cx, |view, cx| {
            assert!(view.ai_test_owned_admission_available(cx))
        });
        cx.read(|cx| {
            assert_eq!(
                cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .owned_worker_count(),
                0
            )
        });
        close(successor, cx);
        drained(cx);
    }
}

#[gpui::test]
fn canceled_inline_session_refuses_save_chooser_even_with_current_reader_text(
    cx: &mut TestAppContext,
) {
    let _lock = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixed in [false, true] {
        let (_, selector, editor) = open(fixed, cx);
        start(selector, &editor, cx);
        approve(selector, &editor, cx);
        assert!(!cx.did_prompt_for_new_path());
        cx.read(|cx| {
            cx.global::<Coordinator>()
                .active
                .as_ref()
                .unwrap()
                .transaction
                .cancellation
                .store(true, Ordering::Release);
        });
        editor.update(cx, |view, cx| {
            assert!(
                view.reader_text(false).is_some(),
                "valid text alone is insufficient for inline Save"
            );
            view.save_reader(cx);
            assert!(!view.export_busy);
        });
        assert!(
            !cx.did_prompt_for_new_path(),
            "stale inline ownership must be checked before opening the chooser"
        );
        cx.run_until_parked();
        drained(cx);
        editor.read_with(cx, |view, _| assert!(view.reader_text(false).is_none()));
    }
}
