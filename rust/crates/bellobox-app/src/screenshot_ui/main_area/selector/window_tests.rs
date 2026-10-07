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
                assert_eq!(view.source, "Window · synthetic alpha on frozen pixels");
            } else {
                assert_eq!(pixels.get_pixel(500, 100).0, [185, 215, 241, 255]);
                assert_eq!(view.source, "Window · synthetic independent pixels");
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
    let (display, candidates, image) =
        bellobox_core::screenshot::window::synthetic_window_fixture().unwrap();
    let mut selector = FrozenWindowSession::new(display, &candidates, 999).unwrap();
    selector
        .accept_frozen(selector.freeze_token().unwrap(), image, display)
        .unwrap();
    selector
        .begin_press(Point::new(400., 120.), display)
        .unwrap();
    let commit = selector
        .end_press(Point::new(400., 120.), display)
        .unwrap()
        .unwrap();
    window_refresh::Request::fixture(&commit, boundary).unwrap()
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
            assert_eq!(view.source, "Window · frozen supplied pixels");
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
