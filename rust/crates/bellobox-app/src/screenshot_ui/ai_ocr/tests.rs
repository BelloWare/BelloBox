use super::*;
use gpui::{TestAppContext, WindowHandle};
use transport::fixture::{self, ResponseMode};

fn editor(cx: &mut TestAppContext) -> WindowHandle<ScreenshotEditor> {
    let fixture = fixture::start(ResponseMode::Success).unwrap();
    let window = cx.add_window(|window, cx| {
        let mut view = ScreenshotEditor::new(
            prepare_document(fixture.document),
            "Supplied image",
            CapturePresentation::default(),
            window,
            cx,
        );
        view.install_ai_fixture(fixture.authority, fixture.permit)
            .unwrap();
        view
    });
    cx.run_until_parked();
    window
}
fn key(value: &str) -> gpui::KeyDownEvent {
    gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse(value).unwrap(),
        is_held: false,
    }
}
fn start(window: WindowHandle<ScreenshotEditor>, cx: &mut TestAppContext) {
    window
        .update(cx, |view, window, cx| view.request_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |view, _, _| {
            assert!(view.ai_ocr.modal(), "{}", view.status)
        })
        .unwrap();
}
fn count(window: WindowHandle<ScreenshotEditor>, cx: &mut TestAppContext) -> usize {
    window
        .update(cx, |view, _, _| {
            fixture::request_count(&view.ai_ocr.fixture.as_ref().unwrap().permit)
        })
        .unwrap()
}
fn close(window: WindowHandle<ScreenshotEditor>, cx: &mut TestAppContext) {
    window
        .update(cx, |view, window, cx| {
            view.ai_ocr.retire(cx);
            window.remove_window();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(!PHYSICAL_WORK.load(Ordering::Acquire));
}

#[gpui::test]
fn actual_modal_approve_response_literal_copy_and_save(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    assert_eq!(count(window, cx), 0);
    window
        .update(cx, |view, window, cx| {
            let approved = &view.ai_ocr.confirmation.as_ref().unwrap().value;
            let recomputed =
                PreparedImage::prepare(view.session.document(), &approved.options).unwrap();
            assert_eq!(approved.image.png(), recomputed.png());
            assert_eq!(approved.image.digest(), recomputed.digest());
            assert!(
                approved
                    .provider
                    .destination()
                    .starts_with("http://127.0.0.1:")
            );
            assert!(view.ai_modal_key(&key("tab"), window, cx));
            assert_eq!(view.ai_ocr.modal_selection, 1);
            assert!(view.ai_modal_key(&key("enter"), window, cx));
            view.approve_ai_ocr(window, cx);
            assert!(!view.ai_ocr.modal());
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(count(window, cx), 1);
    window
        .update(cx, |view, _, cx| {
            assert!(view.ai_result_current(), "{}", view.status);
            let plain = view.reader_text(false).unwrap().to_owned();
            let markdown = view.reader_text(true).unwrap().to_owned();
            view.refresh_reader_mode(true, cx);
            assert_eq!(view.ocr.read(cx).text(), markdown);
            view.copy_reader(false, cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), plain);
            view.copy_reader(true, cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), markdown);
            let temp = tempfile::tempdir().unwrap();
            let destination = temp.path().join("ocr.md");
            save_result_file(&destination, &markdown, &AtomicBool::new(false), None).unwrap();
            assert_eq!(std::fs::read_to_string(&destination).unwrap(), markdown);
            assert!(
                save_result_file(&destination, "replacement", &AtomicBool::new(false), None)
                    .is_err()
            );
        })
        .unwrap();
    close(window, cx);
}

#[gpui::test]
fn modal_cancel_default_enter_escape_and_reopen_never_dispatch(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    for action in ["enter", "escape"] {
        start(window, cx);
        window
            .update(cx, |view, window, cx| {
                assert!(view.ai_modal_key(&key(action), window, cx));
                assert!(!view.ai_ocr.modal());
                assert!(
                    view.ai_ocr.busy(),
                    "cancel cannot release physical ownership on UI thread"
                );
                view.request_ai_ocr(window, cx);
                assert!(!view.ai_ocr.modal());
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(count(window, cx), 0);
    }
    close(window, cx);
}

#[gpui::test]
fn revoked_or_edited_confirmation_is_single_use_and_zero_dispatch(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for revoked in [false, true] {
        let window = editor(cx);
        start(window, cx);
        window
            .update(cx, |view, window, cx| {
                if revoked {
                    view.ai_ocr.fixture.as_ref().unwrap().authority.revoke();
                } else {
                    view.session
                        .set_crop(Some(Rect::new(0., 0., 40., 40.)))
                        .unwrap();
                }
                view.approve_ai_ocr(window, cx);
                view.approve_ai_ocr(window, cx);
                assert!(!view.ai_ocr.modal());
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(count(window, cx), 0);
        close(window, cx);
    }
}

#[gpui::test]
fn privacy_gesture_start_retires_approval_before_revision_and_undo_never_revives(
    cx: &mut TestAppContext,
) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    for tool in [
        AnnotationTool::Crop,
        AnnotationTool::Blur,
        AnnotationTool::Eraser,
    ] {
        start(window, cx);
        window
            .update(cx, |view, window, cx| {
                // Pointer events are blocked while modal is visible. Cancel, then
                // install a still-valid result before beginning the real gesture.
                view.cancel_ai_ocr(window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |view, window, cx| {
                view.tool = tool;
                let before = view.session.revision();
                let interaction = view.ai_ocr.interaction;
                view.ocr_content = RevisionText {
                    revision: Some(before),
                    text: "stale private reader".into(),
                };
                view.ocr
                    .update(cx, |e, cx| e.set_text("stale private reader".into(), cx));
                view.mouse_down(
                    &gpui::MouseDownEvent {
                        button: MouseButton::Left,
                        position: point(px(20.), px(20.)),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert_eq!(view.session.revision(), before);
                assert!(view.ai_ocr.interaction > interaction);
                assert!(view.reader_text(false).is_none());
                assert_eq!(view.ocr.read(cx).text(), "");
                view.request_ai_ocr(window, cx);
                assert!(!view.ai_ocr.modal());
                assert!(view.status.contains("Release the pointer"));
                view.gesture = None;
                view.apply_history(false, cx);
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(count(window, cx), 0);
    }
    close(window, cx);
}

#[gpui::test]
fn production_editor_gate_precedes_configuration_and_pixels(cx: &mut TestAppContext) {
    let window = cx.add_window(|window, cx| {
        ScreenshotEditor::new(
            ScreenshotEditSession::new(
                ScreenshotDocument::from_rgba(image::RgbaImage::new(1, 1)).unwrap(),
            ),
            "Ordinary image",
            CapturePresentation::default(),
            window,
            cx,
        )
    });
    window
        .update(cx, |view, window, cx| {
            view.request_ai_ocr(window, cx);
            assert!(!view.ai_ocr.modal());
            assert!(!view.ai_ocr.busy());
            assert!(view.error);
            assert!(view.status.to_ascii_lowercase().contains("unavailable"));
        })
        .unwrap();
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn modal_close_keeps_physical_ownership_until_disposal_and_new_editor_is_separate(
    cx: &mut TestAppContext,
) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    let permit = window
        .update(cx, |view, _, _| {
            view.ai_ocr.fixture.as_ref().unwrap().permit.clone()
        })
        .unwrap();
    close(window, cx);
    assert_eq!(fixture::request_count(&permit), 0);
    let reopened = editor(cx);
    start(reopened, cx);
    assert_eq!(count(reopened, cx), 0);
    close(reopened, cx);
}

#[test]
fn save_cancellation_never_publishes_or_leaves_staging() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ocr.txt");
    assert!(save_result_file(&path, "private", &AtomicBool::new(true), None).is_err());
    assert!(!path.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[gpui::test]
fn real_modal_event_capture_shields_reader_toolbar_and_default_enter(cx: &mut TestAppContext) {
    use gpui::Focusable;
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    cx.write_to_clipboard(ClipboardItem::new_string("clipboard unchanged".into()));
    window
        .update(cx, |view, window, cx| {
            view.ocr
                .update(cx, |e, cx| e.set_text("reader old value".into(), cx));
            view.ocr.read(cx).focus_handle(cx).focus(window);
            view.request_ai_ocr(window, cx);
        })
        .unwrap();
    cx.run_until_parked();
    let root = window.root(cx).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.draw(point(px(0.), px(0.)), size(px(640.), px(440.)), |_, _| {
        root.clone()
    });
    window
        .update(cx, |view, window, _| {
            assert!(view.ai_ocr.modal_focus.is_focused(window))
        })
        .unwrap();
    visual.simulate_keystrokes("ctrl-a ctrl-c ctrl-shift-c ctrl-z");
    window
        .update(cx, |view, _, cx| {
            assert!(view.ai_ocr.modal());
            assert!(!view.export_busy);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                "clipboard unchanged"
            );
        })
        .unwrap();
    visual.simulate_click(point(px(8.), px(8.)), Default::default());
    window
        .update(cx, |view, _, _| assert!(view.ai_ocr.modal()))
        .unwrap();
    visual.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(count(window, cx), 0);
    window
        .update(cx, |view, _, _| assert!(!view.ai_ocr.modal()))
        .unwrap();
    start(window, cx);
    visual.draw(point(px(0.), px(0.)), size(px(1040.), px(760.)), |_, _| {
        root
    });
    let approve = visual
        .debug_bounds("ai-ocr-approve")
        .expect("actual approve button bounds");
    visual.simulate_click(approve.center(), Default::default());
    cx.run_until_parked();
    assert_eq!(count(window, cx), 1);
    close(window, cx);
}

#[gpui::test]
fn direct_window_destruction_with_retained_entity_retires_modal_and_preparation(
    cx: &mut TestAppContext,
) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for prepared in [false, true] {
        let window = editor(cx);
        let root = window.root(cx).unwrap();
        window
            .update(cx, |view, window, cx| view.request_ai_ocr(window, cx))
            .unwrap();
        if prepared {
            cx.run_until_parked();
        }
        let permit = root.read_with(cx, |view, _| {
            view.ai_ocr.fixture.as_ref().unwrap().permit.clone()
        });
        // Deliberately retain the Entity and skip both explicit editor close and
        // native should-close callbacks, exercising actual window destruction.
        window
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        root.read_with(cx, |view, _| {
            assert!(view.ai_ocr.closed);
            assert!(!view.ai_ocr.modal());
            assert!(!view.ai_ocr.busy());
            assert!(view.ai_ocr.result.is_none());
        });
        assert!(!PHYSICAL_WORK.load(Ordering::Acquire));
        assert_eq!(fixture::request_count(&permit), 0);
        drop(root);
    }
}

#[gpui::test]
fn ordinary_settings_replacement_keeps_exact_approved_provider_snapshot(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    window
        .update(cx, |view, window, cx| {
            let old = view
                .ai_ocr
                .confirmation
                .as_ref()
                .unwrap()
                .value
                .provider
                .clone();
            // A new settings selection is not revocation. The old immutable lease
            // and matching fixture permit remain the sole dispatch authority.
            view.ai_ocr.fixture.as_mut().unwrap().authority = ProviderAuthority::new(
                bellobox_core::ai::Config {
                    provider: bellobox_core::ai::Provider::OpenAIResponses,
                    endpoint: "https://changed.invalid/v1".into(),
                    model: "new-unapproved-model".into(),
                    system_prompt: "unrelated".into(),
                    max_output_tokens: 2048,
                    generation_options: Default::default(),
                },
                "different-unread-key".into(),
            )
            .unwrap();
            assert!(old.is_valid());
            view.approve_ai_ocr(window, cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(count(window, cx), 1);
    let observed = window
        .update(cx, |view, _, _| {
            fixture::take_observed(&view.ai_ocr.fixture.as_ref().unwrap().permit).unwrap()
        })
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&observed.body).unwrap();
    assert_ne!(body["model"], "new-unapproved-model");
    assert!(observed.path.ends_with("/chat/completions"));
    close(window, cx);
}

#[gpui::test]
fn base_replacement_cannot_borrow_fixture_authority_even_after_undo(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    window
        .update(cx, |view, window, cx| {
            let original = view.ai_test_permit();
            // Preserve genuine old-base history, then return to clean before
            // refresh. Undo below really restores the original source Arc.
            let mark = view
                .session
                .add_annotation(
                    AnnotationKind::Rectangle(Rect::new(10., 10., 30., 30.)),
                    AnnotationStyle::default(),
                )
                .unwrap();
            assert!(view.session.remove_annotation(mark).unwrap());
            let replacement =
                ScreenshotDocument::from_rgba(image::RgbaImage::new(960, 540)).unwrap();
            assert!(
                view.session
                    .replace_base_capture(view.session.base_capture_token(), replacement)
            );
            view.changed(cx);
            assert!(!original.check_document(view.session.document()));
            assert!(view.session.can_undo());
            view.apply_history(false, cx);
            assert!(
                original.check_document(view.session.document()),
                "Undo must really restore the generated base owner"
            );
            view.request_ai_ocr(window, cx);
            assert!(!view.ai_ocr.busy());
            assert!(view.status.contains("replaced"));
        })
        .unwrap();
    assert_eq!(count(window, cx), 0);
    close(window, cx);
}

#[gpui::test]
fn result_revocation_clears_focused_reader_undo_and_native_copy(cx: &mut TestAppContext) {
    use gpui::Focusable;
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    window
        .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |view, window, cx| {
            assert!(view.ai_result_current());
            view.ocr.read(cx).focus_handle(cx).focus(window);
        })
        .unwrap();
    let root = window.root(cx).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.draw(point(px(0.), px(0.)), size(px(1040.), px(760.)), |_, _| {
        root
    });
    window
        .update(cx, |view, _, _| {
            view.ai_ocr.fixture.as_ref().unwrap().authority.revoke()
        })
        .unwrap();
    cx.write_to_clipboard(ClipboardItem::new_string("not overwritten".into()));
    visual.simulate_keystrokes("ctrl-z ctrl-a ctrl-c");
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.ocr.read(cx).text(), "");
            assert!(view.reader_text(false).is_none());
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                "not overwritten"
            );
        })
        .unwrap();
    close(window, cx);
}

#[gpui::test]
fn application_shutdown_retires_modal_owners_without_foreground_spawn(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut application = cx.new_app();
    let window = editor(&mut application);
    start(window, &mut application);
    let root = window.root(&mut application).unwrap();
    let permit = root.read_with(&application, |view, _| {
        view.ai_ocr.fixture.as_ref().unwrap().permit.clone()
    });
    application.quit();
    assert!(!PHYSICAL_WORK.load(Ordering::Acquire));
    assert_eq!(fixture::request_count(&permit), 0);
    root.read_with(&application, |view, _| {
        assert!(view.ai_ocr.closed);
        assert!(!view.ai_ocr.modal());
        assert!(!view.ai_ocr.busy());
        assert!(view.reader_text(false).is_none());
    });
}

#[gpui::test]
fn explicit_save_dialog_cancel_reopen_and_literal_file_publication(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    window
        .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    let root = window.root(cx).unwrap();
    root.update(cx, |view, cx| {
        view.refresh_reader_mode(true, cx);
        view.save_reader(cx);
        assert_eq!(view.export_caption(), "Saving the OCR result…");
        assert!(view.ai_ocr.active_save.is_some());
    });
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    root.read_with(cx, |view, _| {
        assert!(!view.export_busy);
        assert!(view.ai_ocr.active_save.is_none());
        assert!(view.ai_result_current());
    });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("literal.md");
    root.update(cx, |view, cx| view.save_reader(cx));
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    let expected = root.read_with(cx, |view, _| {
        assert!(view.ai_ocr.active_save.is_none());
        assert!(!view.export_busy);
        view.reader_text(true).unwrap().to_owned()
    });
    assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    // Retained OCR must not mislabel a subsequent ordinary PNG export.
    root.update(cx, |view, cx| {
        view.save(cx);
        assert!(view.export_busy);
        assert!(view.ai_ocr.active_save.is_none());
        assert_eq!(view.export_caption(), "Exporting the current screenshot…");
    });
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    window
        .update(cx, |view, window, cx| {
            view.copy(false, window, cx);
            assert!(view.export_busy);
            assert_eq!(view.export_caption(), "Exporting the current screenshot…");
        })
        .unwrap();
    cx.run_until_parked();
    close(window, cx);
}

#[gpui::test]
fn close_before_save_dialog_response_cannot_write_retained_result(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    window
        .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    let root = window.root(cx).unwrap();
    root.update(cx, |view, cx| view.save_reader(cx));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("must-not-exist.txt");
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    assert!(!path.exists());
    root.read_with(cx, |view, cx| {
        assert!(view.reader_text(false).is_none());
        assert!(view.ai_ocr.active_save.is_none());
        assert_eq!(view.ocr.read(cx).text(), "");
    });
    assert!(!PHYSICAL_WORK.load(Ordering::Acquire));
}

#[gpui::test]
fn bounded_transport_failures_keep_current_result_and_allow_explicit_recovery(
    cx: &mut TestAppContext,
) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    window
        .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    let prior = window
        .update(cx, |view, _, _| view.reader_text(false).unwrap().to_owned())
        .unwrap();
    for mode in [
        ResponseMode::HttpError,
        ResponseMode::Redirect,
        ResponseMode::Oversize,
    ] {
        window
            .update(cx, |view, _, _| {
                fixture::set_response_mode(&view.ai_ocr.fixture.as_ref().unwrap().permit, mode)
            })
            .unwrap();
        start(window, cx);
        window
            .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |view, _, _| {
                assert!(view.error);
                assert_eq!(view.reader_text(false).unwrap(), prior);
                assert!(!view.ai_ocr.busy());
                assert_eq!(
                    fixture::redirect_request_count(&view.ai_ocr.fixture.as_ref().unwrap().permit),
                    0
                );
            })
            .unwrap();
    }
    window
        .update(cx, |view, _, _| {
            fixture::set_response_mode(
                &view.ai_ocr.fixture.as_ref().unwrap().permit,
                ResponseMode::Success,
            )
        })
        .unwrap();
    start(window, cx);
    window
        .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |view, _, _| {
            assert!(!view.error);
            assert!(view.ai_result_current());
        })
        .unwrap();
    assert_eq!(count(window, cx), 5);
    close(window, cx);
}

#[test]
fn constrained_inline_status_never_overlaps_toolbar_or_reader() {
    use super::super::inline_area::{status_frame, toolbar_frame};
    for (bounds, selection) in [
        (Rect::new(0., 0., 960., 540.), Rect::new(0., 0., 960., 540.)),
        (
            Rect::new(0., 0., 960., 540.),
            Rect::new(38., 38., 521., 351.),
        ),
        (Rect::new(0., 0., 640., 440.), Rect::new(0., 0., 640., 440.)),
    ] {
        let toolbar = toolbar_frame(selection, bounds);
        for reader in [false, true] {
            let status = status_frame(toolbar, bounds, reader);
            assert!(status.y >= 12. && status.bottom() <= bounds.bottom() - 12.);
            assert!(status.bottom() <= toolbar.y - 8. || status.y >= toolbar.bottom() + 8.);
            assert!(status.right() <= bounds.right() - if reader { 305. } else { 12. });
            assert_eq!(status.height, 32.);
        }
    }
}

#[gpui::test]
fn constrained_reader_keeps_provider_warning_viewport_visible_and_bounded(cx: &mut TestAppContext) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let window = editor(cx);
    start(window, cx);
    window
        .update(cx, |view, window, cx| view.approve_ai_ocr(window, cx))
        .unwrap();
    cx.run_until_parked();
    let root = window.root(cx).unwrap();
    // Draw the real editor/reader in the constrained 960×540 fixture viewport.
    for large in [false, true] {
        if large {
            root.update(cx, |view, _| {
                view.ai_ocr.result.as_mut().unwrap().value.warnings =
                    vec!["literal provider warning\n".repeat(1000)];
            });
        }
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.draw(point(px(0.), px(0.)), size(px(960.), px(540.)), |_, _| {
            root.clone()
        });
        let warning = visual
            .debug_bounds("ocr-result-warnings")
            .expect("real warning viewport");
        assert!(warning.size.height > px(0.));
        assert!(warning.size.height <= px(60.));
        assert!(warning.origin.y >= px(0.) && warning.bottom() <= px(540.));
        if large {
            assert_eq!(warning.size.height, px(60.));
        }
    }
    close(window, cx);
}

#[gpui::test]
fn app_quit_refuses_actual_ai_confirmation_without_transmitting_or_latching(
    cx: &mut TestAppContext,
) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cx.update(crate::shutdown::init);
    let window = editor(cx);
    start(window, cx);
    cx.dispatch_action(window.into(), crate::shutdown::Quit);
    assert!(!cx.read(crate::shutdown::requested));
    assert_eq!(count(window, cx), 0);
    window
        .update(cx, |view, _, _| assert!(view.ai_ocr.modal()))
        .unwrap();
    crate::shutdown::dismiss_refusal(cx);
    window
        .update(cx, |view, _, cx| view.ai_ocr.retire(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!cx.read(crate::shutdown::requested));
    cx.dispatch_action(window.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn app_quit_refuses_retained_ocr_payload_until_worker_disposal_after_host_close(
    cx: &mut TestAppContext,
) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cx.update(crate::shutdown::init);
    let home = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        crate::home::Home::new(window, cx)
    });
    let window = editor(cx);
    let retained = window.root(cx).unwrap();
    let pause = window
        .update(cx, |view, _, _| view.ai_test_pause_disposal())
        .unwrap();
    start(window, cx);
    let permit = retained.read_with(cx, |view, _| view.ai_test_permit());
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert!(pause.is_waiting());
    assert!(PHYSICAL_WORK.load(Ordering::Acquire));
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    assert!(!cx.read(crate::shutdown::requested));
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(3600));
    cx.run_until_parked();
    assert_eq!(cx.read(crate::shutdown::quit_calls), 0);
    assert_eq!(fixture::request_count(&permit), 0);
    pause.release();
    cx.run_until_parked();
    assert!(!PHYSICAL_WORK.load(Ordering::Acquire));
    assert!(!cx.read(crate::shutdown::requested));
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
    retained.read_with(cx, |view, _| assert!(view.ai_ocr.closed));
}
