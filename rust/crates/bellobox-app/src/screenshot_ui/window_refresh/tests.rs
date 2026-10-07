use super::{Host, Request};
use crate::{
    screenshot_ui::{CapturePresentation, ScreenshotEditor},
    session::SessionJobs,
};
use bellobox_core::screenshot::{
    AnnotationKind, AnnotationStyle, Point, Rect, ScreenshotEditSession,
    window::{FrozenWindowSession, synthetic_independent_window, synthetic_window_fixture},
    window_refresh::{PreparedWindowRefresh, WindowRefreshDecision, WindowRefreshPlan},
};
use gpui::{Entity, TestAppContext};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn selection(id: u32) -> (ScreenshotEditSession, Request) {
    let (display, catalog, image) = synthetic_window_fixture().unwrap();
    let mut selector = FrozenWindowSession::new(display, &catalog, 999).unwrap();
    selector
        .accept_frozen(selector.freeze_token().unwrap(), image, display)
        .unwrap();
    let point = if id == 1 {
        Point::new(400., 120.)
    } else {
        Point::new(150., 200.)
    };
    selector.begin_press(point, display).unwrap();
    let commit = selector.end_press(point, display).unwrap().unwrap();
    let request = Request::fixture(&commit, Arc::new(AtomicBool::new(false))).unwrap();
    let session = commit.materialize(&AtomicBool::new(false)).unwrap().editor;
    (session, request)
}
fn editor(cx: &mut TestAppContext, id: u32) -> (Entity<ScreenshotEditor>, Request) {
    let (session, request) = selection(id);
    let window = cx.add_window(|window, cx| {
        ScreenshotEditor::new(
            session,
            "Frozen Window · synthetic visible pixels",
            CapturePresentation::default(),
            window,
            cx,
        )
    });
    let root = window.root(cx).unwrap();
    cx.run_until_parked();
    (root, request)
}
fn pending(
    view: &mut ScreenshotEditor,
    request: Request,
) -> (crate::session::JobToken, PreparedWindowRefresh) {
    let mut host = Host {
        context: request.context,
        decision: request.decision,
        boundary: request.boundary,
        jobs: SessionJobs::default(),
    };
    let token = host.jobs.begin();
    let plan = WindowRefreshPlan::new(
        &view.session,
        host.context,
        host.decision,
        host.jobs.cancellation(),
    )
    .unwrap();
    let image =
        synthetic_independent_window(host.context.window_id, &AtomicBool::new(false)).unwrap();
    let prepared = plan.prepare(image).unwrap().unwrap();
    view.window_refresh = Some(host);
    (token, prepared)
}

#[gpui::test]
fn accepted_refresh_updates_real_editor_preview_and_invalidates_ocr(cx: &mut TestAppContext) {
    for id in [1, 2] {
        let (root, request) = editor(cx, id);
        assert_eq!(
            request.decision,
            if id == 1 {
                WindowRefreshDecision::MaskFrozenAlpha
            } else {
                WindowRefreshDecision::ReplaceWithIndependent
            }
        );
        root.update(cx, |view, cx| {
            let old_ocr = view.ocr_jobs.begin();
            view.ocr_busy = true;
            view.ocr_content.revision = Some(view.session.revision());
            view.ocr_content.text = "old synthetic OCR".into();
            view.ocr.update(cx, |editor, cx| {
                editor.set_text("old synthetic OCR".into(), cx)
            });
            let revision = view.preview_revision;
            let (token, prepared) = pending(view, request);
            assert!(view.accept_window_refresh(token, Some(prepared), cx));
            assert_eq!(view.session.revision(), 1);
            assert_eq!(view.preview_revision, revision + 1);
            assert!(view.preview_tiles.is_empty());
            assert!(!view.ocr_jobs.accepts(old_ocr));
            assert!(!view.ocr_busy);
            assert!(view.ocr_content.current(1).is_none());
            assert_eq!(view.ocr.read(cx).text(), "");
            assert!(!view.session.can_undo());
            assert!(!view.session.has_edits());
            let pixels = view.session.document().render_rgba().unwrap();
            assert_eq!(pixels.get_pixel(0, 0).0, [0, 0, 0, 0]);
            assert_eq!(
                pixels.get_pixel(100, 100).0,
                if id == 1 {
                    [255, 224, 186, 255]
                } else {
                    [185, 215, 241, 255]
                }
            );
            // The back window's old overlap was orange, now independent blue.
            if id == 2 {
                assert_eq!(pixels.get_pixel(500, 100).0, [185, 215, 241, 255]);
            }
            assert!(!view.accept_window_refresh(token, None, cx));
        });
        cx.run_until_parked();
        cx.read(|cx| assert!(!root.read(cx).preview_tiles.is_empty()));
    }
}

#[gpui::test]
fn async_fixture_preparation_publishes_through_editor_host(cx: &mut TestAppContext) {
    let (root, request) = editor(cx, 2);
    root.update(cx, |view, cx| view.start_window_refresh(request, cx));
    cx.run_until_parked();
    cx.read(|cx| {
        let view = root.read(cx);
        assert_eq!(view.session.revision(), 1);
        assert_eq!(view.source, "Window · synthetic independent pixels");
        assert!(!view.preview_tiles.is_empty());
    });
}

#[gpui::test]
fn edited_document_rejects_refresh_but_undo_to_clean_remains_eligible(cx: &mut TestAppContext) {
    for undo in [false, true] {
        let (root, request) = editor(cx, 1);
        root.update(cx, |view, cx| {
            let (token, prepared) = pending(view, request);
            view.session
                .add_annotation(
                    AnnotationKind::Rectangle(Rect::new(10., 10., 40., 40.)),
                    AnnotationStyle::default(),
                )
                .unwrap();
            if undo {
                assert!(view.session.undo());
            }
            let revision = view.session.revision();
            assert_eq!(view.accept_window_refresh(token, Some(prepared), cx), undo);
            assert_eq!(view.session.revision(), revision + u64::from(undo));
            if !undo {
                assert_eq!(view.session.document().annotations().len(), 1);
            }
        });
    }
}

#[gpui::test]
fn cancellation_new_generation_topology_and_active_interaction_reject_publication(
    cx: &mut TestAppContext,
) {
    for case in 0..10 {
        let (root, request) = editor(cx, 1);
        root.update(cx, |view, cx| {
            let (token, prepared) = pending(view, request);
            match case {
                0 => view
                    .window_refresh
                    .as_ref()
                    .unwrap()
                    .boundary
                    .store(true, Ordering::Release),
                1 => {
                    view.window_refresh.as_mut().unwrap().jobs.begin();
                }
                2 => {
                    view.window_refresh
                        .as_mut()
                        .unwrap()
                        .context
                        .display
                        .display_id = 9
                }
                3 => view.export_busy = true,
                4 => view.text_origin = Some(Point::new(5., 5.)),
                5 => view.show_discard = true,
                6 => {
                    view.session
                        .set_crop(Some(Rect::new(0., 0., 100., 100.)))
                        .unwrap();
                }
                7 => {
                    view.gesture = Some(crate::screenshot_ui::Gesture {
                        tool: bellobox_core::screenshot::AnnotationTool::Pen,
                        points: vec![Point::new(2., 2.)],
                        selected: None,
                    })
                }
                8 => {
                    view.label_drag = Some(crate::screenshot_ui::LabelDrag {
                        id: None,
                        start: Point::new(1., 1.),
                        origin: Point::new(1., 1.),
                        current: Point::new(1., 1.),
                        max_width: 40.,
                        font_size: 18.,
                        moved: false,
                    })
                }
                _ => view.cancel_window_refresh(),
            }
            let revision = view.session.revision();
            let before = view.session.document().render_rgba().unwrap();
            assert!(
                !view.accept_window_refresh(token, Some(prepared), cx),
                "case {case}"
            );
            assert_eq!(view.session.revision(), revision);
            assert_eq!(view.session.document().render_rgba().unwrap(), before);
        });
    }
}

#[gpui::test]
fn failed_preparation_retains_frozen_pixels_and_does_not_invalidate_ocr(cx: &mut TestAppContext) {
    let (root, request) = editor(cx, 2);
    root.update(cx, |view, cx| {
        let (token, _) = pending(view, request);
        let ocr = view.ocr_jobs.begin();
        assert!(!view.accept_window_refresh(token, None, cx));
        assert!(view.ocr_jobs.accepts(ocr));
        assert_eq!(view.session.revision(), 0);
        assert_eq!(view.source, "Frozen Window · synthetic visible pixels");
        assert_eq!(
            view.session
                .document()
                .render_rgba()
                .unwrap()
                .get_pixel(500, 100)
                .0,
            [255, 224, 186, 255]
        );
    });
}

#[gpui::test]
fn closing_editor_cancels_pending_work_even_with_an_entity_owner_retained(cx: &mut TestAppContext) {
    let (session, request) = selection(1);
    let window = cx.add_window(|window, cx| {
        ScreenshotEditor::new(
            session,
            "Frozen Window · synthetic visible pixels",
            CapturePresentation::default(),
            window,
            cx,
        )
    });
    let root = window.root(cx).unwrap();
    cx.run_until_parked();
    let (token, prepared, cancellation) = root.update(cx, |view, _| {
        let (token, prepared) = pending(view, request);
        (
            token,
            prepared,
            view.window_refresh.as_ref().unwrap().jobs.cancellation(),
        )
    });
    window
        .update(cx, |view, window, cx| view.close(window, cx))
        .unwrap();
    assert!(cancellation.load(Ordering::Acquire));
    root.update(cx, |view, cx| {
        assert!(!view.accept_window_refresh(token, Some(prepared), cx));
        assert_eq!(view.session.revision(), 0);
    });
}

#[gpui::test]
fn already_cancelled_new_request_still_retires_the_old_host(cx: &mut TestAppContext) {
    let (root, old_request) = editor(cx, 1);
    let (_, newer) = selection(2);
    newer.boundary.store(true, Ordering::Release);
    root.update(cx, |view, cx| {
        let (token, prepared) = pending(view, old_request);
        let old_cancellation = view.window_refresh.as_ref().unwrap().jobs.cancellation();
        view.start_window_refresh(newer, cx);
        assert!(old_cancellation.load(Ordering::Acquire));
        assert!(view.window_refresh.is_none());
        assert!(!view.accept_window_refresh(token, Some(prepared), cx));
        assert_eq!(view.session.revision(), 0);
    });
}
