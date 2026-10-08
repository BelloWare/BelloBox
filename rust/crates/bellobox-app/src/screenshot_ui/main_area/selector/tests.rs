//! GPUI tests enter supplied pixels through the coordinator and exercise the
//! same selector and ScreenshotEditor entities used by the gated native caller.
use super::*;
use crate::screenshot_ui::{ScreenshotEditor, inline_area::toolbar_frame};
use bellobox_core::screenshot::selection::SelectionHandle;
use bellobox_core::screenshot::{AnnotationKind, AnnotationStyle, AnnotationTool};
use gpui::{Entity, TestAppContext, VisualTestContext, WindowHandle};

fn supplied_layout() -> host::MainDisplayOverlayLayout {
    host::MainDisplayOverlayLayout {
        display: capture::CaptureDisplay {
            id: 7,
            bounds: capture::CaptureRect::new(0., 0., 600., 400.),
            pixels: capture::CapturePixelSize {
                width: 1200,
                height: 1000,
            },
        },
        cocoa_frame: capture::CaptureRect::new(0., 0., 600., 400.),
        rotation_degrees: 0,
        backing_scale: 2.,
    }
}
fn supplied_snapshot(
    request: capture::CaptureRequest,
    _: capture::CaptureCancellation,
) -> capture::CaptureResult<capture::NativeCaptureSnapshot> {
    let png = bellobox_core::screenshot::scroll::synthetic_page_frame(
        0,
        request.output_size.width,
        request.output_size.height,
        2000,
    )
    .unwrap()
    .png()
    .unwrap();
    Ok(capture::NativeCaptureSnapshot {
        png,
        diagnostics: capture::CaptureDiagnostics {
            requested_display_id: request.display.id,
            resolved_display_id: request.display.id,
            resolution_path: capture::DisplayResolutionPath::InitialId,
            output_size: request.output_size,
            region: None,
            includes_cursor: false,
            backend: "supplied test pixels",
        },
    })
}
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, WindowHandle<MainAreaSelector>) {
    let requester = cx.add_window(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus);
        CaptureChooser {
            busy: false,
            status: String::new(),
            jobs: Default::default(),
            focus,
        }
    });
    cx.update(|cx| {
        begin_supplied(requester.into(), supplied_layout(), cx, supplied_snapshot).unwrap()
    });
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
        .update(cx, |view, _, _| {
            view.viewport
                .set(Bounds::new(point(px(0.), px(0.)), size(px(600.), px(400.))));
            assert!(view.presented);
        })
        .unwrap();
    selector
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    selector
        .update(cx, |_, window, _| assert!(window.is_window_active()))
        .unwrap();
    (requester.into(), selector)
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
fn lock(selector: WindowHandle<MainAreaSelector>, rect: Rect, cx: &mut TestAppContext) {
    selector
        .update(cx, |view, window, cx| {
            view.mouse_down(&down(rect.x, rect.y), window, cx);
            view.mouse_up(&up(rect.right(), rect.bottom()), window, cx);
            assert!(view.locked.is_some());
            assert!(view.editor.is_none());
        })
        .unwrap();
}
fn editor(
    selector: WindowHandle<MainAreaSelector>,
    cx: &mut TestAppContext,
) -> Entity<ScreenshotEditor> {
    cx.run_until_parked();
    cx.read(|cx| {
        selector
            .read(cx)
            .unwrap()
            .editor
            .clone()
            .expect("inline editor in same overlay")
    })
}
fn close(selector: WindowHandle<MainAreaSelector>, cx: &mut TestAppContext) {
    selector
        .update(cx, |view, window, cx| view.retire(window, cx))
        .unwrap();
    cx.run_until_parked();
    cx.read(|cx| {
        assert!(cx.global::<Coordinator>().active.is_none());
        assert!(!cx.global::<NativeCaptureVisibility>().busy);
    });
}
fn selected() -> Rect {
    Rect::new(100., 100., 300., 200.)
}

// These helpers deliberately enter the drawn GPUI window. In particular, an
// outside release must traverse GPUI's capture phase rather than call the
// editor's mouse_up method directly.
pub(super) fn quit_draw(handle: AnyWindowHandle, cx: &mut TestAppContext) {
    // Measured canvas bounds can schedule a second preview/layout pass. Settle
    // that work before comparing geometry or starting a held pointer gesture.
    for _ in 0..3 {
        handle
            .update(cx, |_, window, cx| window.draw(cx).clear())
            .unwrap();
        cx.run_until_parked();
    }
}
pub(super) fn quit_input(
    handle: AnyWindowHandle,
    event: impl gpui::InputEvent,
    cx: &mut TestAppContext,
) {
    VisualTestContext::from_window(handle, cx).simulate_event(event);
    cx.run_until_parked();
}
pub(super) fn quit_pointer_down(handle: AnyWindowHandle, x: f32, y: f32, cx: &mut TestAppContext) {
    quit_input(
        handle,
        MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: None,
            ..Default::default()
        },
        cx,
    );
    quit_input(handle, down(x, y), cx);
}
pub(super) fn quit_key(handle: AnyWindowHandle, key: &str, cx: &mut TestAppContext) {
    quit_input(
        handle,
        gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
            is_held: false,
        },
        cx,
    );
    quit_input(
        handle,
        gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        },
        cx,
    );
}
pub(super) fn quit_shortcut(handle: AnyWindowHandle, cx: &mut TestAppContext) {
    #[cfg(target_os = "macos")]
    quit_key(handle, "cmd-q", cx);
    #[cfg(not(target_os = "macos"))]
    quit_key(handle, "ctrl-q", cx);
    quit_draw(handle, cx);
}

#[derive(PartialEq)]
pub(super) struct QuitInlineSnapshot {
    selection: Rect,
    crop: Option<Rect>,
    dimensions: (u32, u32),
    revision: u64,
    annotations: Vec<bellobox_core::screenshot::ScreenshotAnnotation>,
    pixels: Vec<u8>,
    image_bounds: Bounds<gpui::Pixels>,
    viewport: Bounds<gpui::Pixels>,
    can_undo: bool,
    can_redo: bool,
    has_edits: bool,
    status: String,
}
pub(super) fn quit_inline_snapshot(
    selector: WindowHandle<MainAreaSelector>,
    editor: &Entity<ScreenshotEditor>,
    cx: &TestAppContext,
) -> QuitInlineSnapshot {
    cx.read(|cx| {
        let view = editor.read(cx);
        QuitInlineSnapshot {
            selection: view.inline_selection(),
            crop: view.session.document().crop_rect(),
            dimensions: view.session.document().dimensions(),
            revision: view.session.revision(),
            annotations: view.session.document().annotations().to_vec(),
            pixels: view.session.document().render_rgba().unwrap().into_raw(),
            image_bounds: view.image_bounds.get(),
            viewport: selector.read(cx).unwrap().viewport.get(),
            can_undo: view.session.can_undo(),
            can_redo: view.session.can_redo(),
            has_edits: view.session.has_edits(),
            status: view.status.clone(),
        }
    })
}

#[gpui::test]
fn quit_during_inline_adjustment_runs_all_foreground_guards_and_keeps_natural_release(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    let checks = Rc::new(Cell::new(0));
    let calls = checks.clone();
    let _explaining_owner = selector
        .update(cx, |_, window, cx| {
            cx.new(|cx| {
                crate::shutdown::guard_quit(window, cx, move |_: &mut (), _, _| {
                    calls.set(calls.get() + 1);
                    crate::shutdown::QuitAdmission::Explain("Another foreground task is busy.")
                });
            })
        })
        .unwrap();
    let _worker =
        cx.update(|cx| crate::shutdown::block_quit(cx, "A supplied worker is still publishing."));
    quit_draw(selector.into(), cx);
    quit_pointer_down(selector.into(), 200., 200., cx);
    quit_input(
        selector.into(),
        MouseMoveEvent {
            position: point(px(240.), px(220.)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        },
        cx,
    );
    quit_draw(selector.into(), cx);
    cx.read(|cx| {
        let view = editor.read(cx);
        assert!(view.inline.as_ref().unwrap().adjustment.is_some());
        assert_eq!(view.inline_selection(), Rect::new(140., 120., 300., 200.));
        assert_eq!(
            view.session.document().crop_rect(),
            Some(Rect::new(200., 250., 600., 500.))
        );
        assert!(!view.session.has_edits());
    });
    let before = quit_inline_snapshot(selector, &editor, cx);
    let windows = cx.read(|cx| cx.windows());
    for attempt in 1..=3 {
        quit_shortcut(selector.into(), cx);
        assert_eq!(
            checks.get(),
            attempt,
            "all same-window guards run exactly once"
        );
        assert!(quit_inline_snapshot(selector, &editor, cx) == before);
        assert!(crate::shutdown::pending_refusal(cx).is_none());
        assert!(!cx.has_pending_prompt());
        cx.read(|cx| {
            assert!(cx.windows() == windows);
            assert!(cx.active_window() == Some(selector.into()));
            assert!(!crate::shutdown::requested(cx));
            assert_eq!(crate::shutdown::quit_calls(cx), 0);
            assert!(selector.read(cx).unwrap().quit_notice);
            let view = editor.read(cx);
            assert!(view.quit_pointer_notice, "nested editor guard also ran");
            assert!(view.inline.as_ref().unwrap().adjustment.is_some());
            assert!(!view.show_discard);
        });
    }
    // Release over the newly painted top-right selector notice. Its chrome
    // must not steal the owned move, nor turn its ordinary release into an OK.
    quit_input(selector.into(), up(500., 24.), cx);
    quit_draw(selector.into(), cx);
    cx.read(|cx| {
        let view = editor.read(cx);
        assert!(view.inline.as_ref().unwrap().adjustment.is_none());
        assert!(!view.quit_pointer_notice);
        assert!(!selector.read(cx).unwrap().quit_notice);
        assert_eq!(view.inline_selection(), Rect::new(300., 0., 300., 200.));
        assert_eq!(
            view.session.document().crop_rect(),
            Some(Rect::new(600., 0., 600., 500.))
        );
        assert!(view.session.can_undo());
        assert!(view.session.document().annotations().is_empty());
        assert!(!crate::shutdown::requested(cx));
    });
    quit_key(selector.into(), "ctrl-z", cx);
    cx.read(|cx| {
        let view = editor.read(cx);
        assert_eq!(view.inline_selection(), selected());
        assert!(
            !view.session.can_undo(),
            "natural release adds exactly one undo"
        );
        assert!(!view.session.has_edits());
    });
    close(selector, cx);
}

#[gpui::test]
fn quit_during_inline_handle_adjustment_keeps_escape_and_owner_retirement_semantics(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let mut last_requester = None;
    for navigation in [false, true] {
        let (requester, selector) = open(cx);
        requester
            .update(cx, |_, window, cx| {
                crate::shutdown::admit_quit_window(window, cx)
            })
            .unwrap();
        last_requester = Some(requester);
        lock(selector, selected(), cx);
        let editor = editor(selector, cx);
        let owner = selector.root(cx).unwrap();
        let worker = cx
            .update(|cx| crate::shutdown::block_quit(cx, "A supplied worker is still publishing."));
        quit_draw(selector.into(), cx);
        quit_pointer_down(selector.into(), 400., 300., cx);
        quit_input(
            selector.into(),
            MouseMoveEvent {
                position: point(px(340.), px(260.)),
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            },
            cx,
        );
        quit_draw(selector.into(), cx);
        let before = quit_inline_snapshot(selector, &editor, cx);
        for _ in 0..2 {
            quit_shortcut(selector.into(), cx);
            assert!(quit_inline_snapshot(selector, &editor, cx) == before);
            assert!(crate::shutdown::pending_refusal(cx).is_none());
            cx.read(|cx| {
                let view = editor.read(cx);
                assert!(view.inline.as_ref().unwrap().adjustment.is_some());
                assert!(view.quit_pointer_notice);
                assert!(owner.read(cx).quit_notice);
                assert!(!crate::shutdown::requested(cx));
            });
        }
        if navigation {
            cx.update(navigation_changed);
            cx.run_until_parked();
        } else {
            // Inline Escape normally retires the capture owner. Quit must not
            // interpose a modal that consumes it or commits this draft first.
            quit_input(
                selector.into(),
                KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("escape").unwrap(),
                    is_held: false,
                },
                cx,
            );
        }
        cx.read(|cx| {
            assert!(cx.global::<Coordinator>().active.is_none());
            assert!(!cx.global::<NativeCaptureVisibility>().busy);
            assert!(!cx.windows().contains(&selector.into()));
            assert!(owner.read(cx).retired);
            assert!(!owner.read(cx).quit_notice);
            let view = editor.read(cx);
            assert!(!view.quit_pointer_notice);
            assert!(view.inline.as_ref().unwrap().adjustment.is_none());
            assert!(view.gesture.is_none());
            assert!(view.label_drag.is_none());
            assert_eq!(view.session.document().dimensions(), (1, 1));
            assert!(!view.session.can_undo());
            assert!(!crate::shutdown::requested(cx));
        });
        assert!(crate::shutdown::pending_refusal(cx).is_none());
        drop(worker);
        cx.run_until_parked();
        assert!(
            !cx.read(crate::shutdown::requested),
            "refusal stores no retry"
        );
    }
    let requester = last_requester.unwrap();
    requester
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.dispatch_action(requester, crate::shutdown::Quit);
    assert!(cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

#[gpui::test]
fn supplied_route_mounts_real_editor_in_same_owned_overlay(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    let before = cx.read(|cx| cx.windows());
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    cx.read(|cx| {
        assert!(cx.windows() == before);
        let view = editor.read(cx);
        assert!(view.inline.is_some());
        assert_eq!(view.session.document().dimensions(), (1200, 1000));
        assert_eq!(view.session.document().visible_dimensions(), (600, 500));
        assert_eq!(view.inline_selection(), selected());
        assert!(!view.session.can_undo());
        assert!(!view.session.has_edits());
        assert!(!view.preview_tiles.is_empty());
        let run = cx.global::<Coordinator>().active.as_ref().unwrap();
        assert!(run.selection_locked);
        assert!(run.deactivation.is_none());
        assert!(!run.transaction.can_release());
    });
    close(selector, cx);
}

#[gpui::test]
fn every_inline_handle_uses_full_display_pixels_and_one_undo(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    for handle in SelectionHandle::ALL {
        editor.update(cx, |view, cx| {
            let before = view.session.document().crop_rect();
            let start = handle.position(view.inline_selection()).unwrap();
            view.begin_selection_adjustment(Some(handle), point(px(start.x), px(start.y)), cx);
            view.update_selection_adjustment(point(px(start.x + 10.), px(start.y + 5.)), false, cx);
            view.update_selection_adjustment(
                point(px(start.x + 20.), px(start.y + 10.)),
                false,
                cx,
            );
            let expected = selection::resized_rect(
                selected(),
                handle,
                Point::new(20., 10.),
                Rect::new(0., 0., 600., 400.),
                8.,
            )
            .unwrap();
            assert_eq!(view.inline_selection(), expected);
            assert_eq!(
                view.session.document().crop_rect(),
                before,
                "draft does not mutate history"
            );
            let bands =
                selection::dim_bands(Rect::new(0., 0., 600., 400.), Some(view.inline_selection()))
                    .unwrap();
            assert_eq!(
                bands.iter().map(|r| r.width * r.height).sum::<f32>(),
                240000. - expected.width * expected.height
            );
            assert!(view.update_selection_adjustment(
                point(px(start.x + 20.), px(start.y + 10.)),
                true,
                cx
            ));
            assert_eq!(view.session.document().dimensions(), (1200, 1000));
            view.apply_history(false, cx);
            assert_eq!(view.session.document().crop_rect(), before);
            assert_eq!(view.inline_selection(), selected());
            assert!(!view.session.can_undo());
            view.apply_history(true, cx);
            assert_eq!(view.inline_selection(), expected);
            view.apply_history(false, cx);
        });
    }
    close(selector, cx);
}

#[gpui::test]
fn select_move_and_crop_use_shared_editor_coordinate_and_history_paths(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| {
                view.image_bounds.set(Bounds::new(
                    point(px(100.), px(100.)),
                    size(px(300.), px(200.)),
                ));
                view.mouse_down(&down(200., 200.), window, cx);
                view.mouse_move(
                    &MouseMoveEvent {
                        position: point(px(250.), px(240.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                view.mouse_up(&up(260., 250.), window, cx);
                assert_eq!(view.inline_selection(), Rect::new(160., 150., 300., 200.));
                view.apply_history(false, cx);
                assert_eq!(view.inline_selection(), selected());
                assert!(!view.session.can_undo());
                view.tool = AnnotationTool::Crop;
                view.mouse_down(&down(120., 130.), window, cx);
                view.mouse_up(&up(300., 250.), window, cx);
                assert_eq!(
                    view.session.document().crop_rect(),
                    Some(Rect::new(240., 325., 360., 300.))
                );
                assert_eq!(view.inline_selection(), Rect::new(120., 130., 180., 120.));
                view.apply_history(false, cx);
                assert_eq!(view.inline_selection(), selected());
            })
        })
        .unwrap();
    close(selector, cx);
}

#[gpui::test]
fn adjustment_preserves_annotation_pixels_and_cancels_old_ocr(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    editor.update(cx, |view, cx| {
        view.session
            .add_annotation(
                AnnotationKind::Rectangle(Rect::new(250., 280., 80., 70.)),
                AnnotationStyle::default(),
            )
            .unwrap();
        let annotation = view.session.document().annotations()[0].clone();
        let old_ocr = view.ocr_jobs.begin();
        view.ocr_busy = true;
        view.ocr_content.text = "stale text".into();
        view.begin_selection_adjustment(None, point(px(200.), px(200.)), cx);
        assert!(!view.ocr_jobs.accepts(old_ocr));
        assert!(!view.ocr_busy);
        assert!(view.ocr_content.text.is_empty());
        view.update_selection_adjustment(point(px(240.), px(220.)), true, cx);
        assert_eq!(view.session.document().annotations()[0], annotation);
        view.apply_history(false, cx);
        assert_eq!(view.session.document().annotations()[0], annotation);
        assert_eq!(view.inline_selection(), selected());
    });
    close(selector, cx);
}

#[gpui::test]
fn deactivation_before_selection_lock_retires_the_selector(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    let mut visual = VisualTestContext::from_window(selector.into(), cx);
    visual.deactivate_window();
    cx.run_until_parked();
    assert!(cx.read(|cx| cx.global::<Coordinator>().active.is_none()));
    assert!(!cx.read(|cx| cx.global::<NativeCaptureVisibility>().busy));
}

#[gpui::test]
fn deactivation_during_font_preparation_and_inline_editing_preserves_owner(
    cx: &mut TestAppContext,
) {
    for key in ["escape", "enter"] {
        let (_, selector) = open(cx);
        lock(selector, selected(), cx);
        cx.update(|cx| {
            cx.default_global::<Coordinator>()
                .active
                .as_mut()
                .unwrap()
                .source = HostSource::SuppliedPixels {
                application_active: false,
                topology_valid: true,
            };
        });
        let mut visual = VisualTestContext::from_window(selector.into(), cx);
        visual.deactivate_window();
        let editor = editor(selector, cx);
        assert!(cx.read(|cx| editor.read(cx).inline.is_some()));
        selector
            .update(cx, |_, window, cx| {
                assert!(
                    !window.is_window_active(),
                    "completion must not reactivate the overlay"
                );
                assert!(current(
                    cx.global::<Coordinator>()
                        .active
                        .as_ref()
                        .unwrap()
                        .transaction
                        .id,
                    window,
                    cx
                ));
                window.activate_window();
            })
            .unwrap();
        cx.run_until_parked();
        let root = selector.root(cx).unwrap();
        visual.draw(point(px(0.), px(0.)), size(px(600.), px(400.)), |_, _| root);
        visual.simulate_keystrokes(key);
        cx.run_until_parked();
        assert!(
            cx.read(|cx| cx.global::<Coordinator>().active.is_none()),
            "keyboard-only return must route {key} without a click"
        );
        assert!(!cx.read(|cx| cx.global::<NativeCaptureVisibility>().busy));
    }
}

#[gpui::test]
fn navigation_and_requester_close_cancel_locked_pending_work(cx: &mut TestAppContext) {
    for navigation in [true, false] {
        let (requester, selector) = open(cx);
        lock(selector, selected(), cx);
        cx.update(|cx| {
            if navigation {
                navigation_changed(cx);
            } else {
                requester_closed(requester, cx);
            }
        });
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(cx.global::<Coordinator>().active.is_none());
            assert!(!cx.global::<NativeCaptureVisibility>().busy);
            assert!(!cx.windows().contains(&selector.into()));
        });
    }
}

#[gpui::test]
fn topology_change_rejects_pending_inline_mount(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    cx.update(|cx| {
        cx.default_global::<Coordinator>()
            .active
            .as_mut()
            .unwrap()
            .source = HostSource::SuppliedPixels {
            application_active: true,
            topology_valid: false,
        }
    });
    cx.run_until_parked();
    assert!(cx.read(|cx| cx.global::<Coordinator>().active.is_none()));
    assert!(!cx.read(|cx| cx.global::<NativeCaptureVisibility>().busy));
}

#[gpui::test]
fn close_keeps_busy_until_inline_worker_resources_have_drained(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    editor.update(cx, |view, _| {
        view.session
            .add_annotation(
                AnnotationKind::Rectangle(Rect::new(250., 280., 20., 20.)),
                AnnotationStyle::default(),
            )
            .unwrap();
        assert!(view.session.can_undo());
        assert!(!view.preview_tiles.is_empty());
    });
    let cancellation = cx.read(|cx| editor.read(cx).jobs.cancellation());
    selector
        .update(cx, |view, window, cx| {
            view.retire(window, cx);
            let retired = editor.read(cx);
            assert_eq!(retired.session.document().dimensions(), (1, 1));
            assert!(retired.preview_tiles.is_empty());
            assert!(!retired.session.can_undo());
            assert!(cx.global::<NativeCaptureVisibility>().busy);
            assert!(
                !cx.global::<Coordinator>()
                    .active
                    .as_ref()
                    .unwrap()
                    .transaction
                    .can_release()
            );
        })
        .unwrap();
    assert!(cancellation.load(Ordering::Acquire));
    cx.run_until_parked();
    assert!(!cx.read(|cx| cx.global::<NativeCaptureVisibility>().busy));
    assert!(cx.read(|cx| cx.global::<Coordinator>().active.is_none()));
}

#[gpui::test]
fn toolbar_height_clamps_and_bottom_handle_wins_actual_hit_testing(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    let tall = Rect::new(20., 0., 300., 365.);
    lock(selector, tall, cx);
    let editor = editor(selector, cx);
    let toolbar = toolbar_frame(tall, Rect::new(0., 0., 600., 400.));
    assert_eq!(toolbar.height, 44.);
    assert_eq!(toolbar.y, 344.);
    assert_eq!(toolbar.width, 576.);
    let actual = cx.read(|cx| editor.read(cx).inline_selection());
    assert_eq!(
        actual.height, 365.2,
        "initial half pixel is rounded outward"
    );
    let handle = SelectionHandle::Bottom.position(actual).unwrap();
    assert!(toolbar.contains(handle));
    let mut visual = VisualTestContext::from_window(selector.into(), cx);
    let root = selector.root(cx).unwrap();
    visual.draw(point(px(0.), px(0.)), size(px(600.), px(400.)), |_, _| root);
    visual.simulate_mouse_down(
        point(px(handle.x), px(handle.y)),
        MouseButton::Left,
        Default::default(),
    );
    assert!(
        cx.read(|cx| editor
            .read(cx)
            .inline
            .as_ref()
            .unwrap()
            .adjustment
            .is_some()),
        "handle above clamped toolbar must begin resize"
    );
    visual.simulate_mouse_move(
        point(px(handle.x), px(330.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_up(
        point(px(handle.x), px(330.)),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(
        cx.read(|cx| editor.read(cx).inline_selection().height),
        330.
    );
    close(selector, cx);
}

#[gpui::test]
fn visible_adjustment_is_committed_before_copy_and_undo_cancels_drafts(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| {
                view.begin_selection_adjustment(
                    Some(SelectionHandle::BottomRight),
                    point(px(400.), px(300.)),
                    cx,
                );
                view.update_selection_adjustment(point(px(280.), px(220.)), false, cx);
                assert_eq!(view.inline_selection(), Rect::new(100., 100., 180., 120.));
                window.focus(&view.focus);
            })
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(selector.into(), cx);
    let root = selector.root(cx).unwrap();
    visual.draw(point(px(0.), px(0.)), size(px(600.), px(400.)), |_, _| root);
    visual.simulate_keystrokes("ctrl-shift-c");
    cx.run_until_parked();
    cx.read(|cx| {
        let view = editor.read(cx);
        assert!(view.inline.as_ref().unwrap().adjustment.is_none());
        assert_eq!(view.session.document().visible_dimensions(), (360, 300));
        assert!(view.session.can_undo());
        let entry = cx.read_from_clipboard().expect("copied cropped image");
        let image = entry
            .entries()
            .iter()
            .find_map(|entry| match entry {
                gpui::ClipboardEntry::Image(image) => Some(image),
                _ => None,
            })
            .unwrap();
        let document = ScreenshotDocument::from_png(image.bytes()).unwrap();
        assert_eq!(document.dimensions(), (360, 300));
        assert_eq!(
            document.render_rgba().unwrap(),
            view.session.document().render_rgba().unwrap()
        );
    });
    editor.update(cx, |view, cx| {
        view.apply_history(false, cx);
        assert_eq!(view.inline_selection(), selected());
        assert!(!view.session.can_undo());
        view.session
            .add_annotation(
                AnnotationKind::Rectangle(Rect::new(250., 280., 20., 20.)),
                AnnotationStyle::default(),
            )
            .unwrap();
        view.begin_selection_adjustment(None, point(px(200.), px(200.)), cx);
        view.update_selection_adjustment(point(px(220.), px(220.)), false, cx);
        view.apply_history(false, cx);
        assert_eq!(view.inline_selection(), selected());
        assert_eq!(
            view.session.document().annotations().len(),
            1,
            "Undo cancels the draft before older edits"
        );
    });
    close(selector, cx);
}

#[gpui::test]
fn copy_and_finish_and_external_close_both_release_app_owned_worker_count(cx: &mut TestAppContext) {
    for external_close in [false, true] {
        let (_, selector) = open(cx);
        lock(selector, selected(), cx);
        let editor = editor(selector, cx);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("before export".into()));
        selector
            .update(cx, |view, window, cx| {
                editor.update(cx, |editor, cx| editor.copy(!external_close, window, cx));
                if external_close {
                    view.retire(window, cx);
                }
            })
            .unwrap();
        drop(editor);
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(cx.global::<Coordinator>().active.is_none());
            assert!(!cx.global::<NativeCaptureVisibility>().busy);
            assert!(!cx.windows().contains(&selector.into()));
            if external_close {
                assert_eq!(
                    cx.read_from_clipboard().unwrap().text().as_deref(),
                    Some("before export")
                );
            }
        });
    }
}

#[gpui::test]
fn production_area_entry_remains_closed_before_any_native_or_fixture_work(cx: &mut TestAppContext) {
    let requester = cx.add_window(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus);
        CaptureChooser {
            busy: false,
            status: String::new(),
            jobs: Default::default(),
            focus,
        }
    });
    requester
        .update(cx, |_, window, cx| {
            assert_eq!(
                begin(window, cx).unwrap_err(),
                "Area · main display only is awaiting native review."
            );
            assert!(cx.try_global::<Coordinator>().is_none());
            assert!(!cx.default_global::<NativeCaptureVisibility>().busy);
        })
        .unwrap();
}

#[gpui::test]
fn ocr_snapshot_uses_the_same_committed_visible_crop_without_running_ocr(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, selected(), cx);
    let editor = editor(selector, cx);
    editor.update(cx, |view, cx| {
        view.begin_selection_adjustment(
            Some(SelectionHandle::TopLeft),
            point(px(100.), px(100.)),
            cx,
        );
        view.update_selection_adjustment(point(px(140.), px(120.)), false, cx);
        let snapshot = view.prepare_output_snapshot(cx).unwrap();
        let png = snapshot.render_for_external_ocr_png().unwrap();
        let output = ScreenshotDocument::from_png(&png).unwrap();
        assert_eq!(output.dimensions(), (520, 450));
        assert_eq!(
            output.render_rgba().unwrap(),
            view.session.document().render_rgba().unwrap()
        );
        view.apply_history(false, cx);
        assert_eq!(view.inline_selection(), selected());
        assert!(!view.session.can_undo());
    });
    close(selector, cx);
}

#[gpui::test]
fn inline_menu_and_color_scrims_own_handles_and_panels_stay_in_bounds(cx: &mut TestAppContext) {
    let (_, selector) = open(cx);
    lock(selector, Rect::new(20., 0., 300., 365.), cx);
    let editor = editor(selector, cx);
    let selection = cx.read(|cx| editor.read(cx).inline_selection());
    let handle = SelectionHandle::Bottom.position(selection).unwrap();
    for color in [false, true] {
        editor.update(cx, |view, cx| {
            view.record_inline_menu_anchor(point(px(590.), px(360.)));
            if color {
                view.color_target = Some(crate::screenshot_ui::ColorTarget::Stroke);
            } else {
                view.open_menu = Some(crate::screenshot_ui::Menu::Image);
            }
            view.begin_selection_adjustment(
                Some(SelectionHandle::Bottom),
                point(px(handle.x), px(handle.y)),
                cx,
            );
            assert!(
                view.inline.as_ref().unwrap().adjustment.is_none(),
                "modal admission refuses a direct adjustment too"
            );
            let (width, height) = if color { (300., 260.) } else { (180., 40.) };
            let origin = view.inline_panel_origin(width, height, false).unwrap();
            assert!(origin.x >= 12. && origin.x + width <= 588.);
            assert!(origin.y >= 12. && origin.y + height <= 388.);
        });
        let mut visual = VisualTestContext::from_window(selector.into(), cx);
        let root = selector.root(cx).unwrap();
        visual.draw(point(px(0.), px(0.)), size(px(600.), px(400.)), |_, _| root);
        visual.simulate_mouse_down(
            point(px(handle.x), px(handle.y)),
            MouseButton::Left,
            Default::default(),
        );
        cx.read(|cx| {
            let view = editor.read(cx);
            assert!(view.inline.as_ref().unwrap().adjustment.is_none());
            assert_eq!(view.inline_selection(), selection);
            assert!(view.open_menu.is_none());
            assert!(view.color_target.is_none());
        });
    }
    close(selector, cx);
}

#[gpui::test]
fn topology_change_during_inline_preview_or_copy_retires_without_publication(
    cx: &mut TestAppContext,
) {
    for copying in [false, true] {
        let (_, selector) = open(cx);
        lock(selector, selected(), cx);
        let editor = editor(selector, cx);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
            "unchanged clipboard".into(),
        ));
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| {
                    if copying {
                        view.copy(false, window, cx);
                    } else {
                        view.changed(cx);
                    }
                });
                cx.default_global::<Coordinator>()
                    .active
                    .as_mut()
                    .unwrap()
                    .source = HostSource::SuppliedPixels {
                    application_active: true,
                    topology_valid: false,
                };
            })
            .unwrap();
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(cx.global::<Coordinator>().active.is_none());
            assert!(!cx.global::<NativeCaptureVisibility>().busy);
            assert!(!cx.windows().contains(&selector.into()));
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some("unchanged clipboard")
            );
            assert_eq!(editor.read(cx).session.document().dimensions(), (1, 1));
        });
    }
}

#[gpui::test]
fn active_crop_and_mask_block_keyboard_export_and_ocr_until_pointer_release(
    cx: &mut TestAppContext,
) {
    for tool in [AnnotationTool::Crop, AnnotationTool::Blur] {
        let (_, selector) = open(cx);
        lock(selector, selected(), cx);
        let editor = editor(selector, cx);
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| {
                    view.image_bounds.set(Bounds::new(
                        point(px(100.), px(100.)),
                        size(px(300.), px(200.)),
                    ));
                    view.tool = tool;
                    view.mouse_down(&down(120., 130.), window, cx);
                    view.mouse_move(
                        &MouseMoveEvent {
                            position: point(px(200.), px(180.)),
                            pressed_button: Some(MouseButton::Left),
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                    assert!(view.gesture.is_some());
                })
            })
            .unwrap();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("private baseline".into()));
        let mut visual = VisualTestContext::from_window(selector.into(), cx);
        let root = selector.root(cx).unwrap();
        visual.draw(point(px(0.), px(0.)), size(px(600.), px(400.)), |_, _| root);
        visual.simulate_keystrokes("ctrl-shift-c");
        editor.update(cx, |view, cx| {
            assert!(!view.export_busy);
            assert!(
                view.prepare_output_snapshot(cx).is_none(),
                "same OCR snapshot boundary must refuse"
            );
            assert!(view.gesture.is_some(), "refusal retains the active gesture");
            assert!(!view.session.can_undo());
            assert!(view.session.document().annotations().is_empty());
        });
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("private baseline")
        );
        selector
            .update(cx, |_, window, cx| {
                editor.update(cx, |view, cx| {
                    view.mouse_up(&up(300., 250.), window, cx);
                    assert!(view.gesture.is_none());
                    let snapshot = view.prepare_output_snapshot(cx).unwrap();
                    let output = ScreenshotDocument::from_png(
                        &snapshot.render_for_external_ocr_png().unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        output.render_rgba().unwrap(),
                        view.session.document().render_rgba().unwrap()
                    );
                    if tool == AnnotationTool::Crop {
                        assert_eq!(output.dimensions(), (360, 300));
                    } else {
                        assert_eq!(view.session.document().annotations().len(), 1);
                    }
                    view.apply_history(false, cx);
                    assert_eq!(view.inline_selection(), selected());
                    assert!(view.session.document().annotations().is_empty());
                    assert!(!view.session.can_undo());
                })
            })
            .unwrap();
        close(selector, cx);
    }
}

#[cfg(debug_assertions)]
#[gpui::test]
fn debug_fixture_composes_zero_based_gpui_displays_without_native_capture(cx: &mut TestAppContext) {
    for (status, expected) in [
        ("Selecting from supplied synthetic pixels…", ""),
        ("Explicit fixture failure", "Explicit fixture failure"),
    ] {
        let requester = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus);
            CaptureChooser {
                busy: false,
                status: String::new(),
                jobs: Default::default(),
                focus,
            }
        });
        requester
            .update(cx, |_, window, cx| begin_fixture(window, cx).unwrap())
            .unwrap();
        cx.run_until_parked();
        let selector = cx.read(|cx| {
            let run = cx.global::<Coordinator>().active.as_ref().unwrap();
            assert_ne!(run.layout.display.id, 0);
            assert!(is_supplied(run.transaction.id, cx));
            run.transaction
                .selector
                .unwrap()
                .downcast::<MainAreaSelector>()
                .unwrap()
        });
        requester
            .update(cx, |view, _, _| view.status = status.into())
            .unwrap();
        close(selector, cx);
        cx.read(|cx| assert_eq!(requester.read(cx).unwrap().status, expected));
    }
}

#[gpui::test]
fn anisotropic_area_crop_pointer_keeps_direct_ratio_without_reciprocal_rounding(
    cx: &mut TestAppContext,
) {
    let (_, selector) = open(cx);
    lock(selector, Rect::new(0., 0., 600., 400.), cx);
    let editor = editor(selector, cx);
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| {
                view.image_bounds
                    .set(Bounds::new(point(px(0.), px(0.)), size(px(600.), px(400.))));
                assert_eq!(
                    view.document_point(point(px(330.), px(330.))),
                    Point::new(660., 825.)
                );
                view.tool = AnnotationTool::Crop;
                view.mouse_down(&down(0., 0.), window, cx);
                view.mouse_up(&up(330., 330.), window, cx);
                assert_eq!(view.session.document().visible_dimensions(), (660, 825));
            })
        })
        .unwrap();
    close(selector, cx);
}
