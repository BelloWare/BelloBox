use super::*;
use gpui::{KeyDownEvent, Keystroke, TestAppContext, WindowHandle};

fn editor(cx: &mut TestAppContext) -> WindowHandle<ScreenshotEditor> {
    let png = bellobox_core::qr::png("quit fixture").unwrap();
    let session = ScreenshotEditSession::new(ScreenshotDocument::from_png(&png).unwrap());
    let window = cx.add_window(|window, cx| {
        ScreenshotEditor::new(
            session,
            "Supplied image",
            CapturePresentation::default(),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    window
}
fn dirty(window: WindowHandle<ScreenshotEditor>, cx: &mut TestAppContext) {
    window
        .update(cx, |view, _, _| {
            view.session
                .add_annotation(
                    AnnotationKind::Rectangle(Rect::new(1., 1., 8., 8.)),
                    AnnotationStyle::default(),
                )
                .unwrap();
        })
        .unwrap();
}
#[gpui::test]
fn dirty_quit_keep_editing_and_local_discard_never_authorize_another_window(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let first = editor(cx);
    let second = editor(cx);
    dirty(first, cx);
    cx.dispatch_action(first.into(), crate::shutdown::Quit);
    assert!(!cx.read(crate::shutdown::requested));
    first
        .update(cx, |view, _, _| assert!(view.show_discard))
        .unwrap();
    // The same owner method used by Escape keeps edits and abandons only the
    // local confirmation. There is no app-Quit continuation to resume.
    first
        .update(cx, |view, window, cx| {
            view.key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                },
                window,
                cx,
            )
        })
        .unwrap();
    first
        .update(cx, |view, _, _| {
            assert!(!view.show_discard);
            assert!(view.session.has_edits());
        })
        .unwrap();
    dirty(second, cx);
    cx.dispatch_action(first.into(), crate::shutdown::Quit);
    // Existing Discard is window-local even though Quit first opened its prompt.
    first
        .update(cx, |view, window, cx| {
            view.cancel_window_refresh();
            crate::shutdown::close_window(window, cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert!(!cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    cx.dispatch_action(second.into(), crate::shutdown::Quit);
    second
        .update(cx, |view, _, _| {
            assert!(view.show_discard && view.session.has_edits())
        })
        .unwrap();
    assert!(!cx.read(crate::shutdown::requested));
}

#[gpui::test]
fn screenshot_export_text_and_gesture_refuse_quit_without_mutation_then_clean_retry_works(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let window = editor(cx);
    for kind in 0..3 {
        window
            .update(cx, |view, _, _| match kind {
                0 => view.export_busy = true,
                1 => view.text_origin = Some(Point::new(1., 1.)),
                _ => view.color_target = Some(ColorTarget::Stroke),
            })
            .unwrap();
        cx.dispatch_action(window.into(), crate::shutdown::Quit);
        assert!(!cx.read(crate::shutdown::requested));
        assert!(window.root(cx).is_ok());
        assert!(crate::shutdown::pending_refusal(cx).is_some());
        crate::shutdown::dismiss_refusal(cx);
        window
            .update(cx, |view, _, _| {
                view.export_busy = false;
                view.text_origin = None;
                view.color_target = None;
            })
            .unwrap();
        cx.run_until_parked();
    }
    cx.dispatch_action(window.into(), crate::shutdown::Quit);
    assert!(cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn active_dirty_owner_uses_only_its_existing_local_discard_prompt(cx: &mut TestAppContext) {
    cx.update(crate::shutdown::init);
    let window = editor(cx);
    window
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    dirty(window, cx);
    for _ in 0..3 {
        cx.dispatch_action(window.into(), crate::shutdown::Quit);
        assert!(!cx.read(crate::shutdown::requested));
        assert!(!crate::shutdown::pending_refusal(cx).is_some());
        window
            .update(cx, |view, _, _| {
                assert!(view.show_discard && view.session.has_edits())
            })
            .unwrap();
    }
}

#[gpui::test]
fn inactive_dirty_owner_explains_on_home_without_activation_or_stacked_dialogs(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let screenshot = editor(cx);
    dirty(screenshot, cx);
    let home = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        crate::home::Home::new(window, cx)
    });
    home.update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    let prompt =
        crate::shutdown::pending_refusal(cx).expect("active Home explains the hidden decision");
    assert_eq!(prompt.1, "Restore the screenshot to resolve edits.");
    assert!(prompt.1.chars().count() <= 40);
    for _ in 0..3 {
        cx.dispatch_action(home.into(), crate::shutdown::Quit);
        assert_eq!(crate::shutdown::pending_refusal(cx), Some(prompt.clone()));
        assert!(cx.read(|cx| cx.active_window()) == Some(home.into()));
        assert!(!cx.read(crate::shutdown::requested));
    }
    crate::shutdown::dismiss_refusal(cx);
    cx.run_until_parked();
    assert!(
        !crate::shutdown::pending_refusal(cx).is_some(),
        "one answer dismisses the only explanation"
    );
    assert!(!cx.read(crate::shutdown::requested));
    screenshot
        .update(cx, |view, _, _| {
            assert!(view.show_discard && view.session.has_edits())
        })
        .unwrap();
    assert_eq!(cx.read(|cx| cx.windows().len()), 2);
}

fn pointer_draw(handle: gpui::AnyWindowHandle, cx: &mut TestAppContext) {
    for _ in 0..3 {
        handle
            .update(cx, |_, window, cx| window.draw(cx).clear())
            .unwrap();
        cx.run_until_parked();
    }
}
fn pointer_event(
    handle: gpui::AnyWindowHandle,
    event: impl gpui::InputEvent,
    cx: &mut TestAppContext,
) {
    gpui::VisualTestContext::from_window(handle, cx).simulate_event(event);
    cx.run_until_parked();
}
fn pointer_key(handle: gpui::AnyWindowHandle, key: &str, cx: &mut TestAppContext) {
    let keystroke = Keystroke::parse(key).unwrap();
    pointer_event(
        handle,
        KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
        },
        cx,
    );
    pointer_event(handle, gpui::KeyUpEvent { keystroke }, cx);
}
fn pointer_quit(handle: gpui::AnyWindowHandle, cx: &mut TestAppContext) {
    pointer_key(
        handle,
        if cfg!(target_os = "macos") {
            "cmd-q"
        } else {
            "ctrl-q"
        },
        cx,
    );
}
fn pointer_down(
    handle: gpui::AnyWindowHandle,
    position: gpui::Point<gpui::Pixels>,
    cx: &mut TestAppContext,
) {
    pointer_event(
        handle,
        gpui::MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Default::default(),
        },
        cx,
    );
    pointer_event(
        handle,
        gpui::MouseDownEvent {
            position,
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
        cx,
    );
}
fn pointer_move(
    handle: gpui::AnyWindowHandle,
    position: gpui::Point<gpui::Pixels>,
    cx: &mut TestAppContext,
) {
    pointer_event(
        handle,
        gpui::MouseMoveEvent {
            position,
            pressed_button: Some(gpui::MouseButton::Left),
            modifiers: Default::default(),
        },
        cx,
    );
}
fn pointer_up(
    handle: gpui::AnyWindowHandle,
    position: gpui::Point<gpui::Pixels>,
    cx: &mut TestAppContext,
) {
    pointer_event(
        handle,
        gpui::MouseUpEvent {
            position,
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
        cx,
    );
}
fn pointer_editor(tool: AnnotationTool, cx: &mut TestAppContext) -> WindowHandle<ScreenshotEditor> {
    let handle = editor(cx);
    handle
        .update(cx, |view, window, cx| {
            view.tool = tool;
            window.activate_window();
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    pointer_draw(handle.into(), cx);
    handle
}
fn pointer_positions(
    handle: WindowHandle<ScreenshotEditor>,
    cx: &TestAppContext,
) -> (gpui::Point<gpui::Pixels>, gpui::Point<gpui::Pixels>) {
    handle
        .read_with(cx, |view, _| {
            let bounds = view.image_bounds.get();
            let start = bounds.origin + gpui::point(px(15.), px(15.));
            let end = start + gpui::point(px(35.), px(25.));
            assert!(bounds.contains(&start) && bounds.contains(&end));
            (start, end)
        })
        .unwrap()
}

#[gpui::test]
fn pointer_quit_rectangle_crop_pen_arrow_preserve_geometry_until_natural_release(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    for tool in [
        AnnotationTool::Rectangle,
        AnnotationTool::Crop,
        AnnotationTool::Pen,
        AnnotationTool::Arrow,
    ] {
        let handle = pointer_editor(tool, cx);
        let (start, end) = pointer_positions(handle, cx);
        let (bounds, first, last) = handle
            .read_with(cx, |view, _| {
                (
                    view.image_bounds.get(),
                    view.document_point(start),
                    view.document_point(end),
                )
            })
            .unwrap();
        pointer_down(handle.into(), start, cx);
        pointer_move(handle.into(), end, cx);
        let before = handle
            .read_with(cx, |view, _| {
                assert!(view.gesture.is_some());
                assert!(!view.session.has_edits());
                (
                    view.session.revision(),
                    view.gesture.as_ref().unwrap().points.clone(),
                )
            })
            .unwrap();
        for _ in 0..3 {
            pointer_quit(handle.into(), cx);
            assert!(!cx.read(crate::shutdown::requested));
            assert!(crate::shutdown::pending_refusal(cx).is_none());
            pointer_draw(handle.into(), cx);
            handle
                .read_with(cx, |view, _| {
                    assert_eq!(
                        view.pointer_quit_message(),
                        Some("Finish drag or Esc, then Quit.")
                    );
                    assert_eq!(
                        view.image_bounds.get(),
                        bounds,
                        "refusal cannot move the canvas under a held pointer"
                    );
                    assert_eq!(view.session.revision(), before.0);
                    assert_eq!(view.gesture.as_ref().unwrap().points, before.1);
                    assert!(view.session.document().annotations().is_empty());
                    assert!(view.session.document().crop_rect().is_none());
                })
                .unwrap();
        }
        pointer_up(handle.into(), end, cx);
        handle
            .read_with(cx, |view, _| {
                assert!(!view.pointer_edit_active());
                assert!(!view.quit_pointer_notice);
                assert!(view.session.has_edits());
                let expected = Rect::new(first.x, first.y, last.x - first.x, last.y - first.y);
                if tool == AnnotationTool::Crop {
                    // Crop's established contract rounds outward to pixels;
                    // annotation geometry below intentionally keeps raw floats.
                    let pixel_crop = Rect::new(
                        first.x.floor(),
                        first.y.floor(),
                        last.x.ceil() - first.x.floor(),
                        last.y.ceil() - first.y.floor(),
                    );
                    assert_eq!(view.session.document().crop_rect(), Some(pixel_crop));
                    assert!(view.session.document().annotations().is_empty());
                } else {
                    assert_eq!(view.session.document().annotations().len(), 1);
                    match &view.session.document().annotations()[0].kind {
                        AnnotationKind::Rectangle(rect) => assert_eq!(*rect, expected),
                        AnnotationKind::Arrow { start, end } => {
                            assert_eq!(*start, first);
                            assert_eq!(*end, last);
                        }
                        AnnotationKind::Freehand { points } => {
                            assert_eq!(points[0], first);
                            assert_eq!(*points.last().unwrap(), last);
                        }
                        other => panic!("unexpected completed gesture: {other:?}"),
                    }
                }
            })
            .unwrap();
        // A later explicit Quit still performs the ordinary dirty decision.
        pointer_quit(handle.into(), cx);
        assert!(!cx.read(crate::shutdown::requested));
        handle
            .read_with(cx, |view, _| assert!(view.show_discard))
            .unwrap();
        pointer_key(handle.into(), "escape", cx);
        handle
            .read_with(cx, |view, _| {
                assert!(!view.show_discard && view.session.has_edits())
            })
            .unwrap();
    }
}

#[gpui::test]
fn pointer_quit_escape_and_new_edit_do_not_waive_a_simultaneous_worker_blocker(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let handle = pointer_editor(AnnotationTool::Rectangle, cx);
    let worker =
        cx.update(|cx| crate::shutdown::block_quit(cx, "Finish or cancel the QR save first."));
    let (start, end) = pointer_positions(handle, cx);
    pointer_down(handle.into(), start, cx);
    pointer_move(handle.into(), end, cx);
    pointer_quit(handle.into(), cx);
    assert!(crate::shutdown::pending_refusal(cx).is_none());
    handle
        .read_with(cx, |view, _| assert!(view.quit_pointer_notice))
        .unwrap();
    pointer_key(handle.into(), "escape", cx);
    handle
        .read_with(cx, |view, _| {
            assert!(!view.quit_pointer_notice && !view.pointer_edit_active());
            assert!(!view.session.has_edits());
        })
        .unwrap();
    pointer_up(handle.into(), end, cx);
    // Normal editing is not stranded, and a new drag does not inherit a notice.
    pointer_down(handle.into(), start, cx);
    pointer_move(handle.into(), end, cx);
    handle
        .read_with(cx, |view, _| {
            assert!(!view.quit_pointer_notice && view.gesture.is_some())
        })
        .unwrap();
    pointer_key(handle.into(), "escape", cx);
    pointer_up(handle.into(), end, cx);
    pointer_quit(handle.into(), cx);
    assert_eq!(
        crate::shutdown::pending_refusal(cx).unwrap().1,
        "Finish or cancel the QR save first."
    );
    assert!(!cx.read(crate::shutdown::requested));
    crate::shutdown::dismiss_refusal(cx);
    drop(worker);
    assert!(
        !cx.read(crate::shutdown::requested),
        "refusal never schedules a later quit"
    );
    pointer_quit(handle.into(), cx);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[gpui::test]
fn pointer_quit_label_drag_preserves_document_and_release_or_escape_ownership(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    for cancel in [false, true] {
        let png = bellobox_core::qr::png("label quit fixture").unwrap();
        let mut session = prepare(png).unwrap();
        assert!(
            session.has_text_font(),
            "the declared local platform font is required; this regression must not silently skip"
        );
        let origin = Point::new(12., 16.);
        let width = 80.;
        let style = AnnotationStyle {
            font_size: 14.,
            ..Default::default()
        };
        session
            .add_annotation(
                AnnotationKind::Text {
                    text: "Drag label".into(),
                    origin,
                    max_width: width,
                },
                style,
            )
            .unwrap();
        let handle = cx.add_window(|window, cx| {
            ScreenshotEditor::new(
                session,
                "Label fixture",
                CapturePresentation::default(),
                window,
                cx,
            )
        });
        handle
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        pointer_draw(handle.into(), cx);
        let (start, end, bounds, before, revision) = handle
            .read_with(cx, |view, _| {
                let bounds = view.image_bounds.get();
                let frame = bellobox_core::screenshot::selection::visible_text_label_frame(
                    origin,
                    width,
                    style.font_size,
                    Point::default(),
                )
                .unwrap();
                let start = bounds.origin
                    + gpui::point(
                        px((frame.x + frame.width / 2.) * view.scale),
                        px((frame.y + frame.height / 2.) * view.scale),
                    );
                (
                    start,
                    start + gpui::point(px(24.), px(18.)),
                    bounds,
                    view.session.document().annotations().to_vec(),
                    view.session.revision(),
                )
            })
            .unwrap();
        pointer_down(handle.into(), start, cx);
        pointer_move(handle.into(), end, cx);
        let current = handle
            .read_with(cx, |view, _| {
                view.label_drag.expect("actual label drag").current
            })
            .unwrap();
        pointer_quit(handle.into(), cx);
        pointer_draw(handle.into(), cx);
        assert!(crate::shutdown::pending_refusal(cx).is_none());
        handle
            .read_with(cx, |view, _| {
                assert_eq!(view.session.document().annotations(), before);
                assert_eq!(view.session.revision(), revision);
                assert_eq!(view.image_bounds.get(), bounds);
                assert!(view.pointer_quit_message().is_some());
            })
            .unwrap();
        if cancel {
            pointer_key(handle.into(), "escape", cx);
        }
        pointer_up(handle.into(), end, cx);
        handle
            .read_with(cx, |view, _| {
                assert!(!view.quit_pointer_notice && view.label_drag.is_none());
                if cancel {
                    assert_eq!(view.session.document().annotations(), before);
                } else {
                    let AnnotationKind::Text { origin, .. } =
                        view.session.document().annotations()[0].kind
                    else {
                        panic!("text label retained")
                    };
                    assert_eq!(origin, current);
                    assert_ne!(view.session.document().annotations(), before);
                }
            })
            .unwrap();
    }
}

#[gpui::test]
fn pointer_quit_notice_retires_with_closed_owner_even_when_entity_is_retained(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let handle = pointer_editor(AnnotationTool::Rectangle, cx);
    let retained = handle.root(cx).unwrap();
    let home = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        crate::home::Home::new(window, cx)
    });
    handle
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    let (start, end) = pointer_positions(handle, cx);
    pointer_down(handle.into(), start, cx);
    pointer_move(handle.into(), end, cx);
    pointer_quit(handle.into(), cx);
    assert!(retained.read_with(cx, |view, _| view.quit_pointer_notice));
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert!(!retained.read_with(cx, |view, _| view.quit_pointer_notice));
    assert!(!cx.read(crate::shutdown::requested));
    home.update(cx, |_, window, _| window.activate_window())
        .unwrap();
    pointer_quit(home.into(), cx);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn pointer_quit_notice_never_reflows_empty_error_wrapped_or_rendering_status(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    for (width, height) in [(1280., 1000.), (700., 700.)] {
        for (message, rendering) in [
            (String::new(), false),
            (
                "An existing screenshot error has details.\n".repeat(8),
                false,
            ),
            (
                "A long existing screenshot error must retain its full layout width and wrapping. "
                    .repeat(8),
                false,
            ),
            (
                "An existing error is currently covered by the rendering caption.\n".repeat(4),
                true,
            ),
        ] {
            let handle = pointer_editor(AnnotationTool::Rectangle, cx);
            cx.simulate_window_resize(handle.into(), gpui::size(px(width), px(height)));
            handle
                .update(cx, |view, _, cx| {
                    view.status = message.clone();
                    view.error = true;
                    view.rendering = rendering;
                    cx.notify();
                })
                .unwrap();
            pointer_draw(handle.into(), cx);
            let (start, end) = pointer_positions(handle, cx);
            pointer_down(handle.into(), start, cx);
            pointer_move(handle.into(), end, cx);
            pointer_draw(handle.into(), cx);
            let before = handle
                .read_with(cx, |view, _| {
                    assert_eq!(view.status, message);
                    assert!(view.gesture.is_some());
                    assert!(!view.session.has_edits());
                    (
                        view.image_bounds.get(),
                        view.scale,
                        view.document_point(start),
                        view.document_point(end),
                    )
                })
                .unwrap();
            for _ in 0..2 {
                pointer_quit(handle.into(), cx);
                pointer_draw(handle.into(), cx);
                assert!(crate::shutdown::pending_refusal(cx).is_none());
                handle.read_with(cx, |view, _| {
                    assert_eq!(view.status, message, "ordinary status remains owned");
                    assert!(view.gesture.is_some());
                    assert!(!view.session.has_edits());
                    assert_eq!((view.image_bounds.get(), view.scale, view.document_point(start), view.document_point(end)), before,
                        "notice must not change layout, Fit scale or the physical release's source point");
                }).unwrap();
            }
            pointer_up(handle.into(), end, cx);
            handle
                .read_with(cx, |view, _| {
                    assert!(!view.pointer_edit_active() && !view.quit_pointer_notice);
                    assert_eq!(view.session.document().annotations().len(), 1);
                    assert_eq!(
                        view.session.document().annotations()[0].kind,
                        AnnotationKind::Rectangle(Rect::new(
                            before.2.x,
                            before.2.y,
                            before.3.x - before.2.x,
                            before.3.y - before.2.y
                        ))
                    );
                })
                .unwrap();
        }
    }
}
