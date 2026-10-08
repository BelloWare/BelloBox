//! Supplied records enter the actual owned coordinator, selector, editor and
//! refresh worker. These tests do not enumerate or capture operating-system windows.
use super::*;
use crate::screenshot_ui::{ScreenshotEditor, window_refresh, window_workflow};
use bellobox_core::screenshot::{AnnotationKind, AnnotationStyle, AnnotationTool};
use gpui::{Entity, TestAppContext, WindowHandle};

fn open_fixture(
    fixture: window_workflow::Fixture,
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, WindowHandle<MainAreaSelector>) {
    let layout = fixture.layout;
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
    cx.update(|cx| begin_window_supplied(requester.into(), fixture, cx).unwrap());
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
            view.viewport.set(Bounds::new(
                point(px(0.), px(0.)),
                size(
                    px(layout.cocoa_frame.size.width as f32),
                    px(layout.cocoa_frame.size.height as f32),
                ),
            ));
            assert!(view.presented);
            window.activate_window();
        })
        .unwrap();
    cx.run_until_parked();
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
fn choose(
    selector: WindowHandle<MainAreaSelector>,
    id: u32,
    cx: &mut TestAppContext,
) -> Entity<ScreenshotEditor> {
    let (x, y) = if id == 1 { (400., 120.) } else { (150., 200.) };
    selector
        .update(cx, |view, window, cx| {
            view.mouse_down(&down(x, y), window, cx);
            view.mouse_up(&up(x, y), window, cx);
            assert!(view.locked.is_some());
        })
        .unwrap();
    cx.run_until_parked();
    cx.read(|cx| selector.read(cx).unwrap().editor.clone().unwrap())
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

#[gpui::test]
fn quit_during_nested_window_rectangle_preserves_document_and_outside_release(
    cx: &mut TestAppContext,
) {
    use super::tests::{
        quit_draw, quit_inline_snapshot, quit_input, quit_key, quit_pointer_down, quit_shortcut,
    };

    cx.update(crate::shutdown::init);
    let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
    let editor = choose(selector, 1, cx);
    editor.update(cx, |view, cx| {
        view.tool = AnnotationTool::Rectangle;
        cx.notify();
    });
    let _worker =
        cx.update(|cx| crate::shutdown::block_quit(cx, "A supplied worker is still publishing."));
    quit_draw(selector.into(), cx);
    quit_pointer_down(selector.into(), 400., 160., cx);
    quit_input(
        selector.into(),
        MouseMoveEvent {
            position: point(px(500.), px(260.)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        },
        cx,
    );
    quit_draw(selector.into(), cx);
    let release = point(px(700.), px(24.));
    let (points, expected) = cx.read(|cx| {
        let view = editor.read(cx);
        let gesture = view.gesture.as_ref().expect("actual canvas owns Rectangle");
        assert_eq!(gesture.points.len(), 2);
        assert!(view.session.document().annotations().is_empty());
        assert!(!view.session.can_undo());
        (
            gesture.points.clone(),
            Rect::from_points(gesture.points[0], view.document_point(release)),
        )
    });
    let before = quit_inline_snapshot(selector, &editor, cx);
    let windows = cx.read(|cx| cx.windows());
    for _ in 0..3 {
        quit_shortcut(selector.into(), cx);
        assert!(quit_inline_snapshot(selector, &editor, cx) == before);
        assert!(crate::shutdown::pending_refusal(cx).is_none());
        assert!(!cx.has_pending_prompt());
        cx.read(|cx| {
            assert!(cx.windows() == windows);
            assert!(!crate::shutdown::requested(cx));
            assert_eq!(crate::shutdown::quit_calls(cx), 0);
            assert!(selector.read(cx).unwrap().quit_notice);
            let view = editor.read(cx);
            assert!(
                view.quit_pointer_notice,
                "both nested guards were consulted"
            );
            assert_eq!(view.gesture.as_ref().unwrap().points, points);
            assert!(!view.show_discard);
        });
    }
    // The notice is outside the fixed selected window. This is the real
    // capture-phase mouse_up_out path that a modal used to misinterpret.
    quit_input(selector.into(), up(700., 24.), cx);
    quit_draw(selector.into(), cx);
    cx.read(|cx| {
        let view = editor.read(cx);
        assert!(view.gesture.is_none());
        assert!(!view.quit_pointer_notice);
        assert!(!selector.read(cx).unwrap().quit_notice);
        assert_eq!(view.inline_selection(), Rect::new(300., 80., 340., 270.));
        assert!(view.session.document().crop_rect().is_none());
        assert_eq!(view.session.document().annotations().len(), 1);
        assert_eq!(
            view.session.document().annotations()[0].kind,
            AnnotationKind::Rectangle(expected)
        );
        assert!(view.session.can_undo());
        assert!(!crate::shutdown::requested(cx));
    });
    let committed = quit_inline_snapshot(selector, &editor, cx);
    for _ in 0..2 {
        quit_shortcut(selector.into(), cx);
        assert!(quit_inline_snapshot(selector, &editor, cx) == committed);
        assert!(crate::shutdown::pending_refusal(cx).is_none());
        assert!(!cx.has_pending_prompt());
        cx.read(|cx| {
            assert!(cx.windows() == windows);
            assert!(selector.read(cx).unwrap().quit_notice);
            assert!(!crate::shutdown::requested(cx));
            assert_eq!(crate::shutdown::quit_calls(cx), 0);
            let view = editor.read(cx);
            assert!(view.session.has_edits());
            assert!(view.gesture.is_none());
            assert!(!view.quit_pointer_notice);
            assert!(
                !view.show_discard,
                "inline capture must not acquire a hidden standalone Discard prompt"
            );
        });
    }
    quit_key(selector.into(), "ctrl-z", cx);
    cx.read(|cx| {
        let view = editor.read(cx);
        assert!(view.session.document().annotations().is_empty());
        assert!(
            !view.session.can_undo(),
            "outside release committed exactly once"
        );
        assert!(!view.session.has_edits());
    });
    close(selector, cx);
}

#[gpui::test]
fn window_mode_selects_and_refreshes_inside_the_same_owned_overlay(cx: &mut TestAppContext) {
    for id in [1, 2] {
        let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
        let before = cx.read(|cx| cx.windows());
        let editor = choose(selector, id, cx);
        cx.read(|cx| {
            assert!(cx.windows() == before);
            let view = editor.read(cx);
            assert!(!view.inline.as_ref().unwrap().adjustable());
            assert!(view.session.document().crop_rect().is_none());
            assert!(!view.session.has_edits());
            assert!(!view.session.can_undo());
            assert_eq!(
                view.session.document().dimensions(),
                if id == 1 { (680, 540) } else { (840, 460) }
            );
            let pixels = view.session.document().render_rgba().unwrap();
            if id == 1 {
                assert_eq!(pixels.get_pixel(0, 0).0, [0, 0, 0, 0]);
                assert_eq!(pixels.get_pixel(200, 200).0, [255, 224, 186, 255]);
                assert_eq!(view.source, "Window · alpha on frozen pixels");
            } else {
                assert_eq!(pixels.get_pixel(500, 100).0, [185, 215, 241, 255]);
                assert_eq!(view.source, "Window · independent pixels");
            }
            assert!(!view.preview_tiles.is_empty());
        });
        close(selector, cx);
    }
}

#[gpui::test]
fn window_pointer_policy_latches_front_order_and_never_falls_back_to_area(cx: &mut TestAppContext) {
    let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
    selector
        .update(cx, |view, window, cx| {
            view.mouse_down(&down(20., 20.), window, cx);
            view.mouse_up(&up(20., 20.), window, cx);
            assert!(view.locked.is_none());
            view.mouse_down(&down(400., 200.), window, cx);
            view.mouse_up(&up(408., 200.), window, cx);
            assert!(view.locked.is_none(), "exactly eight points is not a click");
            view.mouse_down(&down(400., 200.), window, cx);
            view.mouse_move(
                &MouseMoveEvent {
                    position: point(px(150.), px(200.)),
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                },
                window,
                cx,
            );
            // End near the original press: the front window remains latched.
            view.mouse_up(&up(407., 207.), window, cx);
            assert_eq!(view.locked, Some(Rect::new(300., 80., 340., 270.)));
        })
        .unwrap();
    cx.run_until_parked();
    close(selector, cx);
}

#[gpui::test]
fn crop_fits_inside_fixed_window_frame_and_selection_adjustment_is_disabled(
    cx: &mut TestAppContext,
) {
    let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
    let editor = choose(selector, 1, cx);
    let fixed = Rect::new(300., 80., 340., 270.);
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| {
                assert_eq!(view.inline_selection(), fixed);
                view.begin_selection_adjustment(None, point(px(400.), px(200.)), cx);
                assert!(view.inline.as_ref().unwrap().adjustment.is_none());
                view.session
                    .set_crop(Some(Rect::new(40., 60., 200., 100.)))
                    .unwrap();
                view.changed(cx);
                assert_eq!(view.inline_selection(), fixed);
                let visible = view.inline_visible_rect();
                let inline = view.inline.as_ref().unwrap();
                assert_eq!(inline.canvas_scales(visible), (1.7, 1.7));
                let canvas = inline.canvas_frame(visible);
                assert_eq!(canvas, Rect::new(300., 130., 340., 170.));
                view.image_bounds.set(Bounds::new(
                    point(px(canvas.x), px(canvas.y)),
                    size(px(canvas.width), px(canvas.height)),
                ));
                assert_eq!(
                    view.document_point(point(px(470.), px(215.))),
                    Point::new(140., 110.)
                );
                view.tool = AnnotationTool::Select;
                view.mouse_down(&down(470., 215.), window, cx);
                view.mouse_move(
                    &MouseMoveEvent {
                        position: point(px(500.), px(220.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                view.mouse_up(&up(500., 220.), window, cx);
                assert_eq!(view.inline_selection(), fixed);
                assert_eq!(view.session.document().visible_dimensions(), (200, 100));
                view.apply_history(false, cx);
                assert!(view.session.document().crop_rect().is_none());
                assert_eq!(view.inline_selection(), fixed);
                assert_eq!(view.session.document().dimensions(), (680, 540));
            })
        })
        .unwrap();
    close(selector, cx);
}

#[gpui::test]
fn window_copy_finish_exports_only_window_pixels_and_releases_coordinator(cx: &mut TestAppContext) {
    let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
    let editor = choose(selector, 2, cx);
    selector
        .update(cx, |_, window, cx| {
            editor.update(cx, |view, cx| view.copy(true, window, cx))
        })
        .unwrap();
    drop(editor);
    cx.run_until_parked();
    cx.read(|cx| {
        assert!(cx.global::<Coordinator>().active.is_none());
        assert!(!cx.global::<NativeCaptureVisibility>().busy);
        let item = cx.read_from_clipboard().unwrap();
        let png = item
            .entries()
            .iter()
            .find_map(|entry| {
                if let gpui::ClipboardEntry::Image(image) = entry {
                    Some(image.bytes())
                } else {
                    None
                }
            })
            .unwrap();
        let image = ScreenshotDocument::from_png(png).unwrap();
        assert_eq!(image.dimensions(), (840, 460));
        assert_eq!(
            image.render_rgba().unwrap().get_pixel(500, 100).0,
            [185, 215, 241, 255]
        );
    });
}

fn refresh_request(boundary: Arc<AtomicBool>) -> window_refresh::Request {
    let fixture = window_workflow::fixture().unwrap();
    let display = geometry(fixture.layout);
    let mut selector = FrozenWindowSession::new(
        display,
        &fixture.source.candidates().unwrap(),
        fixture.source.own_pid(),
    )
    .unwrap();
    selector
        .accept_frozen(selector.freeze_token().unwrap(), fixture.document, display)
        .unwrap();
    selector
        .begin_press(Point::new(400., 120.), display)
        .unwrap();
    let commit = selector
        .end_press(Point::new(400., 120.), display)
        .unwrap()
        .unwrap();
    let (context, decision, acquisition) =
        fixture.source.request(&commit, boundary.clone()).unwrap();
    window_refresh::Request::supplied(context, decision, boundary, acquisition)
}

#[gpui::test]
fn cancellation_keeps_busy_until_rejected_refresh_pixel_disposal_completes(
    cx: &mut TestAppContext,
) {
    let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
    let editor = choose(selector, 1, cx);
    let id = cx.read(|cx| editor.read(cx).inline.as_ref().unwrap().id);
    let boundary = cx.update(|cx| editor_cancellation(id, cx).unwrap());
    let gate = window_refresh::DisposalGate::default();
    let mut request = refresh_request(boundary);
    request.disposal_gate = Some(gate.clone());
    editor.update(cx, |view, cx| {
        view.session
            .add_annotation(
                AnnotationKind::Rectangle(Rect::new(20., 20., 30., 40.)),
                AnnotationStyle::default(),
            )
            .unwrap();
        view.start_window_refresh(request, cx);
    });
    cx.run_until_parked();
    assert!(
        gate.arrived(),
        "actual completion reached background disposal"
    );
    cx.read(|cx| assert_eq!(editor.read(cx).session.document().annotations().len(), 1));
    selector
        .update(cx, |view, window, cx| view.retire(window, cx))
        .unwrap();
    drop(editor);
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(cx.global::<NativeCaptureVisibility>().busy);
        assert!(cx.global::<Coordinator>().active.is_some());
        assert!(
            begin_editor_work(id, cx).is_none(),
            "cancelled owner rejects new work admission"
        );
    });
    gate.release();
    cx.run_until_parked();
    cx.read(|cx| {
        assert!(cx.global::<Coordinator>().active.is_none());
        assert!(!cx.global::<NativeCaptureVisibility>().busy);
    });
}

#[gpui::test]
fn failed_or_changed_independent_source_keeps_frozen_window_usable(cx: &mut TestAppContext) {
    for changed in [false, true] {
        let mut fixture = window_workflow::fixture().unwrap();
        let owner = Arc::new(std::sync::Mutex::new(
            None::<std::sync::Weak<window_workflow::Source>>,
        ));
        let worker_owner = owner.clone();
        fixture.source = fixture
            .source
            .with_generator(move |submission, _, cancellation| {
                if !changed {
                    return Err("supplied failure".into());
                }
                let source = worker_owner
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .unwrap();
                source.update_current(|evidence| {
                    evidence
                        .observations
                        .iter_mut()
                        .find(|row| row.window_id == submission.window_id)
                        .unwrap()
                        .owner_process_id += 1
                });
                bellobox_core::screenshot::window::synthetic_independent_window(
                    submission.window_id,
                    cancellation,
                )
                .map_err(|e| e.to_string())
            });
        *owner.lock().unwrap() = Some(Arc::downgrade(&fixture.source));
        let (_, selector) = open_fixture(fixture, cx);
        let editor = choose(selector, 2, cx);
        editor.update(cx, |view, cx| {
            assert_eq!(view.source, "Window · frozen pixels");
            assert!(!view.session.has_edits());
            assert!(!view.session.can_undo());
            assert_eq!(
                view.session
                    .document()
                    .render_rgba()
                    .unwrap()
                    .get_pixel(500, 100)
                    .0,
                [255, 224, 186, 255]
            );
            view.session
                .add_annotation(
                    AnnotationKind::Rectangle(Rect::new(10., 10., 40., 30.)),
                    AnnotationStyle::default(),
                )
                .unwrap();
            view.changed(cx);
            assert!(view.session.can_undo());
        });
        close(selector, cx);
    }
}

#[gpui::test]
fn coordinator_topology_or_navigation_change_retires_pending_refresh(cx: &mut TestAppContext) {
    for topology in [false, true] {
        let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
        let editor = choose(selector, 1, cx);
        let id = cx.read(|cx| editor.read(cx).inline.as_ref().unwrap().id);
        let boundary = cx.update(|cx| editor_cancellation(id, cx).unwrap());
        editor.update(cx, |view, cx| {
            view.start_window_refresh(refresh_request(boundary), cx)
        });
        cx.update(|cx| {
            if topology {
                if let HostSource::SuppliedPixels { topology_valid, .. } = &mut cx
                    .global_mut::<Coordinator>()
                    .active
                    .as_mut()
                    .unwrap()
                    .source
                {
                    *topology_valid = false;
                }
            } else {
                navigation_changed(cx);
            }
        });
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(cx.global::<Coordinator>().active.is_none());
            assert!(!cx.global::<NativeCaptureVisibility>().busy);
            assert_eq!(editor.read(cx).session.document().dimensions(), (1, 1));
        });
    }
}

#[gpui::test]
fn fixed_window_letterbox_gesture_clamps_to_the_fitted_crop_once(cx: &mut TestAppContext) {
    let (_, selector) = open_fixture(window_workflow::fixture().unwrap(), cx);
    let editor = choose(selector, 1, cx);
    editor.update(cx, |view, cx| {
        // Excludes the contrasting dark header. Painting remains clipped to the
        // 340x170 fitted canvas; the fixed frame has 50-point well bands.
        view.session
            .set_crop(Some(Rect::new(40., 80., 200., 100.)))
            .unwrap();
        view.tool = AnnotationTool::Pen;
        view.changed(cx);
    });
    cx.run_until_parked();
    let mut visual = gpui::VisualTestContext::from_window(selector.into(), cx);
    let root = selector.root(cx).unwrap();
    visual.draw(point(px(0.), px(0.)), size(px(800.), px(500.)), |_, _| root);
    visual.simulate_mouse_down(
        point(px(470.), px(100.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.read(|cx| {
        let view = editor.read(cx);
        let gesture = view
            .gesture
            .as_ref()
            .expect("fixed frame owns letterbox input");
        assert_eq!(gesture.points.len(), 1);
        assert_eq!(gesture.points[0], Point::new(140., 80.));
    });
    visual.simulate_mouse_move(
        point(px(555.), px(215.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_up(
        point(px(555.), px(215.)),
        MouseButton::Left,
        Default::default(),
    );
    cx.read(|cx| {
        let view = editor.read(cx);
        assert_eq!(
            view.session.document().annotations().len(),
            1,
            "one gesture, no duplicate inner handler"
        );
        assert!(view.gesture.is_none());
        assert_eq!(view.inline_selection(), Rect::new(300., 80., 340., 270.));
    });
    close(selector, cx);
}

#[gpui::test]
fn injected_callback_mismatch_missing_and_duplicate_sources_never_acquire_pixels(
    cx: &mut TestAppContext,
) {
    use std::sync::atomic::AtomicUsize;
    for case in 0..4 {
        let mut fixture = window_workflow::fixture().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        fixture.source = fixture
            .source
            .with_submissions(|submissions| match case {
                0 => submissions[1].owner_process_id += 1,
                1 => submissions[1].frame.origin.x += 0.000_001,
                2 => {
                    submissions.pop();
                }
                _ => submissions.push(submissions[1].clone()),
            })
            .with_generator(move |_, _, _| {
                counter.fetch_add(1, Ordering::Relaxed);
                Err("invalid callback reached pixels".into())
            });
        let (_, selector) = open_fixture(fixture, cx);
        let editor = choose(selector, 2, cx);
        cx.read(|cx| {
            assert_eq!(calls.load(Ordering::Relaxed), 0, "case {case}");
            let view = editor.read(cx);
            assert_eq!(view.source, "Window · frozen pixels");
            assert_eq!(
                view.session
                    .document()
                    .render_rgba()
                    .unwrap()
                    .get_pixel(500, 100)
                    .0,
                [255, 224, 186, 255]
            );
            assert!(!view.session.has_edits());
        });
        close(selector, cx);
    }
}

#[gpui::test]
fn injected_completion_revalidates_full_evidence_and_dimensions_in_coordinator(
    cx: &mut TestAppContext,
) {
    for case in 0..6 {
        let mut fixture = window_workflow::fixture().unwrap();
        let owner = Arc::new(std::sync::Mutex::new(
            None::<std::sync::Weak<window_workflow::Source>>,
        ));
        let worker_owner = owner.clone();
        fixture.source = fixture
            .source
            .with_generator(move |submission, _, cancellation| {
                let source = worker_owner
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .unwrap();
                match case {
                    0 => source.update_current(|e| {
                        e.observations.pop();
                    }),
                    1 => source.update_current(|e| e.observations.push(e.observations[1].clone())),
                    2 => source.update_current(|e| e.observations[1].frame.origin.x += 0.000_001),
                    3 => {
                        source.update_current(|e| e.topology.displays[0].display.pixels.width += 1)
                    }
                    4 => source.advance_generation(),
                    _ => {}
                }
                bellobox_core::screenshot::window::synthetic_independent_window(
                    if case == 5 { 1 } else { submission.window_id },
                    cancellation,
                )
                .map_err(|e| e.to_string())
            });
        *owner.lock().unwrap() = Some(Arc::downgrade(&fixture.source));
        let (_, selector) = open_fixture(fixture, cx);
        let editor = choose(selector, 2, cx);
        editor.update(cx, |view, cx| {
            assert_eq!(view.source, "Window · frozen pixels", "case {case}");
            assert_eq!(view.session.document().dimensions(), (840, 460));
            assert_eq!(
                view.session
                    .document()
                    .render_rgba()
                    .unwrap()
                    .get_pixel(500, 100)
                    .0,
                [255, 224, 186, 255]
            );
            view.session
                .add_annotation(
                    AnnotationKind::Rectangle(Rect::new(10., 10., 20., 20.)),
                    AnnotationStyle::default(),
                )
                .unwrap();
            view.changed(cx);
            assert!(view.session.can_undo());
        });
        close(selector, cx);
    }
}

#[gpui::test]
fn returned_plan_for_other_equal_sized_window_or_options_keeps_frozen_pixels(
    cx: &mut TestAppContext,
) {
    use window_workflow::ReturnedPlanFault;
    for fault in [
        ReturnedPlanFault::OtherWindow,
        ReturnedPlanFault::EquivalentSelection,
        ReturnedPlanFault::Cursor,
        ReturnedPlanFault::Deadline,
    ] {
        let mut fixture = window_workflow::fixture().unwrap();
        fixture.source = fixture.source.with_returned_plan_fault(fault);
        let (_, selector) = open_fixture(fixture, cx);
        let editor = choose(selector, 2, cx);
        cx.read(|cx| {
            let view = editor.read(cx);
            assert_eq!(view.source, "Window · frozen pixels");
            assert_eq!(view.session.document().dimensions(), (840, 460));
            assert_eq!(
                view.session
                    .document()
                    .render_rgba()
                    .unwrap()
                    .get_pixel(500, 100)
                    .0,
                [255, 224, 186, 255]
            );
            assert!(!view.session.has_edits());
            assert!(!view.session.can_undo());
        });
        close(selector, cx);
    }
}

#[gpui::test]
fn delayed_observation_mounts_usable_editor_and_keeps_busy_through_queued_drain(
    cx: &mut TestAppContext,
) {
    for retire in [false, true] {
        let gate = window_refresh::DisposalGate::default();
        let mut fixture = window_workflow::fixture().unwrap();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let worker_calls = calls.clone();
        let executor = cx.background_executor.clone();
        fixture.source = fixture
            .source
            .with_observer(move |_| {
                assert!(
                    !executor.is_main_thread(),
                    "observe must never run in foreground apply"
                );
                worker_calls.fetch_add(1, Ordering::Relaxed);
                Ok(())
            })
            .with_observation_gate(gate.clone());
        let (_, selector) = open_fixture(fixture, cx);
        let editor = choose(selector, 1, cx);
        assert!(
            gate.arrived(),
            "same counted refresh worker is waiting for fake main dispatch"
        );
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "editor mounted before decision observation"
        );
        editor.update(cx, |view, cx| {
            assert_eq!(view.source, "Window · frozen pixels");
            view.session
                .add_annotation(
                    AnnotationKind::Rectangle(Rect::new(10., 10., 30., 30.)),
                    AnnotationStyle::default(),
                )
                .unwrap();
            view.changed(cx);
        });
        if retire {
            selector
                .update(cx, |view, window, cx| view.retire(window, cx))
                .unwrap();
            drop(editor);
            cx.run_until_parked();
            cx.update(|cx| {
                assert!(cx.global::<NativeCaptureVisibility>().busy);
                assert!(
                    begin_window_supplied(selector.into(), window_workflow::fixture().unwrap(), cx)
                        .is_err()
                );
            });
        }
        gate.release();
        cx.run_until_parked();
        if retire {
            assert_eq!(
                calls.load(Ordering::Relaxed),
                0,
                "cancelled queued observation performs no source action"
            );
            cx.read(|cx| {
                assert!(cx.global::<Coordinator>().active.is_none());
                assert!(!cx.global::<NativeCaptureVisibility>().busy);
            });
        } else {
            let editor = cx.read(|cx| selector.read(cx).unwrap().editor.clone().unwrap());
            editor.update(cx, |view, cx| {
                assert_eq!(view.source, "Window · frozen pixels");
                assert_eq!(view.session.document().annotations().len(), 1);
                view.apply_history(false, cx);
                assert!(view.session.document().annotations().is_empty());
                assert_eq!(
                    view.source, "Window · frozen pixels",
                    "one-shot rejection cannot revive after Undo"
                );
            });
            close(selector, cx);
        }
        // A successor is admitted only after the original queued work drained.
        let (_, next) = open_fixture(window_workflow::fixture().unwrap(), cx);
        close(next, cx);
    }
}

#[gpui::test]
fn final_observation_success_after_deadline_preserves_frozen_pixels(cx: &mut TestAppContext) {
    let mut fixture = window_workflow::fixture().unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let worker_calls = calls.clone();
    fixture.source = fixture
        .source
        .with_observer(move |request| {
            if worker_calls.fetch_add(1, Ordering::Relaxed) == 3 {
                // Decision, callback binding, acquisition validation, then final
                // post-mask observation. Opaque work returns success too late.
                std::thread::sleep(
                    request
                        .deadline()
                        .saturating_duration_since(std::time::Instant::now())
                        + std::time::Duration::from_millis(2),
                );
            }
            Ok(())
        })
        .with_refresh_timeout(std::time::Duration::from_secs(2));
    let (_, selector) = open_fixture(fixture, cx);
    let editor = choose(selector, 1, cx);
    assert_eq!(calls.load(Ordering::Relaxed), 4);
    cx.read(|cx| {
        let view = editor.read(cx);
        assert_eq!(view.source, "Window · frozen pixels");
        assert_eq!(
            view.session.revision(),
            1,
            "only initial prepared font revision, no refresh"
        );
        assert!(!view.session.can_undo());
        assert_eq!(
            view.session
                .document()
                .render_rgba()
                .unwrap()
                .get_pixel(0, 0)
                .0[3],
            255
        );
    });
    close(selector, cx);
}
