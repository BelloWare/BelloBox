use super::*;
use crate::screenshot::{
    AnnotationKind, AnnotationStyle, Point,
    window::{FrozenWindowCandidate, FrozenWindowSession},
};
use image::{Rgba, RgbaImage};
fn layers() -> OcclusionLayers {
    OcclusionLayers {
        normal: 0,
        floating: 3,
        modal_panel: 8,
        main_menu: 24,
        status: 25,
        popup_menu: 101,
        screen_saver: 1000,
    }
}
fn row(id: u32, pid: i32, layer: i64, frame: Rect) -> OcclusionRow {
    OcclusionRow {
        window_id: Some(id),
        owner_process_id: Some(pid),
        layer: Some(layer),
        alpha: Some(1.),
        frame: Some(frame),
    }
}
fn target() -> Rect {
    Rect::new(10., 10., 80., 60.)
}
fn source() -> WindowRefreshSource {
    WindowRefreshSource {
        independent_window: true,
        scrolling_active: false,
        cut_from_frozen: true,
        window_id: 10,
        frame: Some(target()),
    }
}
fn image(width: u32, height: u32, color: [u8; 4]) -> ScreenshotDocument {
    ScreenshotDocument::from_rgba(RgbaImage::from_pixel(width, height, Rgba(color))).unwrap()
}
fn context() -> WindowRefreshContext {
    let display = AreaDisplayGeometry {
        display_id: 1,
        cocoa_frame: Rect::new(0., 0., 100., 80.),
        pixel_size: (100, 80),
        rotation_degrees: 0,
    };
    let candidate = FrozenWindowCandidate {
        window_id: 10,
        owner_process_id: 20,
        frame_local_points: target(),
        layer: 0,
        alpha: 1.,
        on_screen: true,
    };
    let mut selector = FrozenWindowSession::new(display, &[candidate], 99).unwrap();
    selector
        .accept_frozen(
            selector.freeze_token().unwrap(),
            image(100, 80, [1, 2, 3, 255]),
            display,
        )
        .unwrap();
    selector.begin_press(Point::new(20., 20.), display).unwrap();
    let commit = selector
        .end_press(Point::new(20., 20.), display)
        .unwrap()
        .unwrap();
    WindowRefreshContext {
        selection: commit.token(),
        window_id: 10,
        display,
    }
}
fn plan(
    session: &ScreenshotEditSession,
    context: WindowRefreshContext,
    decision: WindowRefreshDecision,
) -> WindowRefreshPlan {
    WindowRefreshPlan::new(session, context, decision, Arc::new(AtomicBool::new(false))).unwrap()
}
fn add_mark(session: &mut ScreenshotEditSession) -> u64 {
    session
        .add_annotation(
            AnnotationKind::Rectangle(Rect::new(1., 1., 3., 3.)),
            AnnotationStyle::default(),
        )
        .unwrap()
}
fn add_then_remove(session: &mut ScreenshotEditSession) {
    let id = add_mark(session);
    assert!(session.remove_annotation(id).unwrap());
}
#[test]
fn occlusion_scans_front_to_back_and_stops_at_target_before_other_fields() {
    let overlap = row(20, 200, 0, Rect::new(20., 20., 20., 20.));
    let bare_target = OcclusionRow {
        window_id: Some(10),
        owner_process_id: None,
        layer: None,
        alpha: None,
        frame: None,
    };
    assert!(is_occluded(10, target(), Some(&[overlap, bare_target]), 99, layers()).unwrap());
    assert!(!is_occluded(10, target(), Some(&[bare_target, overlap]), 99, layers()).unwrap());
    assert!(is_occluded(10, target(), Some(&[overlap]), 99, layers()).unwrap()); // target absent, source loop still sees overlap
}
#[test]
fn own_regular_windows_and_all_source_selectable_layers_can_occlude() {
    for layer in [0, 3, 8, 24, 25, 101] {
        assert!(
            is_occluded(
                10,
                target(),
                Some(&[row(20, 99, layer, target())]),
                99,
                layers()
            )
            .unwrap()
        );
    }
    assert!(
        !is_occluded(
            10,
            target(),
            Some(&[row(20, 99, 1000, target())]),
            99,
            layers()
        )
        .unwrap()
    );
    assert!(
        !is_occluded(
            10,
            target(),
            Some(&[row(20, 20, 3000, target())]),
            99,
            layers()
        )
        .unwrap()
    );
}
#[test]
fn occluder_catalog_is_not_restricted_to_wholly_contained_selectable_windows() {
    assert!(
        is_occluded(
            10,
            target(),
            Some(&[row(20, 99, 3, Rect::new(-100., -100., 200., 200.))]),
            99,
            layers()
        )
        .unwrap()
    );
}
#[test]
fn occlusion_requires_two_points_on_both_axes_and_source_size_alpha_thresholds() {
    for (frame, expected) in [
        (Rect::new(88., 20., 20., 20.), true),
        (Rect::new(88.01, 20., 20., 20.), false),
        (Rect::new(20., 68., 20., 20.), true),
        (Rect::new(20., 68.01, 20., 20.), false),
        (Rect::new(20., 20., 8., 12.), true),
        (Rect::new(20., 20., 8., 11.99), false),
    ] {
        assert_eq!(
            is_occluded(10, target(), Some(&[row(20, 20, 0, frame)]), 99, layers()).unwrap(),
            expected
        );
    }
    for (alpha, expected) in [(0., false), (0.01, false), (0.0101, true)] {
        let mut item = row(20, 20, 0, target());
        item.alpha = Some(alpha);
        assert_eq!(
            is_occluded(10, target(), Some(&[item]), 99, layers()).unwrap(),
            expected
        );
    }
}
#[test]
fn malformed_rows_skip_and_oversized_catalog_fails_without_fallback() {
    let mut malformed = row(20, 20, 0, target());
    malformed.frame = None;
    assert!(!is_occluded(10, target(), Some(&[malformed]), 99, layers()).unwrap());
    malformed = row(20, 20, 0, target());
    malformed.window_id = None;
    assert!(!is_occluded(10, target(), Some(&[malformed]), 99, layers()).unwrap());
    assert_eq!(
        is_occluded(
            10,
            target(),
            Some(&vec![malformed; MAX_OCCLUSION_ROWS + 1]),
            99,
            layers()
        ),
        Err(WindowRefreshError::InvalidCatalog)
    );
}
#[test]
fn decisions_preserve_visible_scrolling_missing_frame_and_query_failure_defaults() {
    assert_eq!(
        decide_window_refresh(source(), None, 99, layers()).unwrap(),
        WindowRefreshDecision::MaskFrozenAlpha
    );
    let mut input = source();
    input.frame = None;
    assert_eq!(
        decide_window_refresh(input, None, 99, layers()).unwrap(),
        WindowRefreshDecision::ReplaceWithIndependent
    );
    input = source();
    input.cut_from_frozen = false;
    assert_eq!(
        decide_window_refresh(input, None, 99, layers()).unwrap(),
        WindowRefreshDecision::ReplaceWithIndependent
    );
    input.independent_window = false;
    assert_eq!(
        decide_window_refresh(input, None, 99, layers()).unwrap(),
        WindowRefreshDecision::KeepFrozen
    );
    input.independent_window = true;
    input.scrolling_active = true;
    assert_eq!(
        decide_window_refresh(input, None, 99, layers()).unwrap(),
        WindowRefreshDecision::KeepFrozen
    );
}
#[test]
fn mask_keeps_frozen_rgb_multiplies_alpha_and_clears_fully_transparent_rgb() {
    let frozen = image(3, 1, [30, 60, 90, 128]);
    let shape = ScreenshotDocument::from_rgba(RgbaImage::from_fn(3, 1, |x, _| {
        Rgba([250, 1, 2, [0, 128, 255][x as usize]])
    }))
    .unwrap();
    let masked = mask_frozen_alpha(
        WindowAlphaPixels::from_document(&frozen),
        WindowAlphaPixels::from_document(&shape),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(masked.base_image.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(masked.base_image.get_pixel(1, 0).0, [30, 60, 90, 64]);
    assert_eq!(masked.base_image.get_pixel(2, 0).0, [30, 60, 90, 128]);
    assert_eq!(frozen.base_image.get_pixel(0, 0).0, [30, 60, 90, 128]);
}
#[test]
fn mask_preserves_frozen_size_and_uses_explicit_nearest_centres_for_one_pixel_delta() {
    let frozen = image(3, 2, [1, 2, 3, 255]);
    let shape = ScreenshotDocument::from_rgba(RgbaImage::from_fn(2, 3, |x, _| {
        Rgba([9, 9, 9, if x == 0 { 0 } else { 255 }])
    }))
    .unwrap();
    let masked = mask_frozen_alpha(
        WindowAlphaPixels::from_document(&frozen),
        WindowAlphaPixels::from_document(&shape),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(masked.dimensions(), (3, 2));
    for y in 0..2 {
        assert_eq!(masked.base_image.get_pixel(0, y)[3], 0);
        assert_eq!(masked.base_image.get_pixel(1, y).0, [1, 2, 3, 255]);
        assert_eq!(masked.base_image.get_pixel(2, y).0, [1, 2, 3, 255]);
    }
    assert!(matches!(
        mask_frozen_alpha(
            WindowAlphaPixels::from_document(&frozen),
            WindowAlphaPixels::from_document(&image(5, 2, [0; 4])),
            &AtomicBool::new(false)
        ),
        Err(WindowRefreshError::IncompatibleImages)
    ));
}
#[test]
fn successful_refresh_preserves_font_advances_revision_and_adds_no_undo_step() {
    let mut session = ScreenshotEditSession::new(image(8, 8, [1, 2, 3, 255]));
    // Ownership sentinel only; no font parsing/rendering or filesystem involved.
    let font = Arc::new(vec![7u8]);
    session.document.text_font = Some(font.clone());
    let old = Arc::downgrade(&session.document.base_image);
    let ctx = context();
    let revision = session.revision();
    let prepared = plan(&session, ctx, WindowRefreshDecision::ReplaceWithIndependent)
        .prepare(image(12, 6, [4, 5, 6, 255]))
        .unwrap()
        .unwrap();
    assert!(
        prepared
            .apply(&mut session, ctx, &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(session.document().dimensions(), (12, 6));
    assert!(session.revision() > revision);
    assert!(!session.has_edits());
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert!(Arc::ptr_eq(
        session.document.text_font.as_ref().unwrap(),
        &font
    ));
    assert!(
        old.upgrade().is_none(),
        "the dirty baseline must not pin the obsolete base"
    );
}
#[test]
fn current_clean_after_undo_accepts_the_same_base_token_despite_nonzero_revision() {
    let mut session = ScreenshotEditSession::new(image(8, 8, [1; 4]));
    let token = session.base_capture_token();
    let ctx = context();
    let plan = plan(&session, ctx, WindowRefreshDecision::ReplaceWithIndependent);
    add_mark(&mut session);
    assert!(session.undo());
    assert!(session.revision() > 0);
    assert_eq!(session.base_capture_token(), token);
    let prepared = plan.prepare(image(10, 6, [2; 4])).unwrap().unwrap();
    assert!(
        prepared
            .apply(&mut session, ctx, &AtomicBool::new(false))
            .unwrap()
    );
}
#[test]
fn initial_dirty_document_is_rejected_even_when_has_edits_is_false() {
    for crop in [false, true] {
        let mut seed = ScreenshotEditSession::new(image(8, 8, [1; 4]));
        if crop {
            seed.set_crop(Some(Rect::new(0., 0., 4., 4.))).unwrap();
        } else {
            add_mark(&mut seed);
        }
        let mut session = ScreenshotEditSession::new(seed.render_snapshot());
        assert!(!session.has_edits());
        let token = session.base_capture_token();
        let revision = session.revision();
        assert!(!session.replace_base_capture(token, image(10, 6, [2; 4])));
        assert_eq!(session.document().dimensions(), (8, 8));
        assert_eq!(session.revision(), revision);
    }
}
#[test]
fn current_annotation_or_crop_prevents_refresh_without_mutating_state() {
    for crop in [false, true] {
        let mut session = ScreenshotEditSession::new(image(8, 8, [1; 4]));
        let ctx = context();
        let prepared = plan(&session, ctx, WindowRefreshDecision::ReplaceWithIndependent)
            .prepare(image(10, 6, [2; 4]))
            .unwrap()
            .unwrap();
        if crop {
            session.set_crop(Some(Rect::new(0., 0., 4., 4.))).unwrap();
        } else {
            add_mark(&mut session);
        }
        let before = session.revision();
        assert!(
            !prepared
                .apply(&mut session, ctx, &AtomicBool::new(false))
                .unwrap()
        );
        assert_eq!(session.revision(), before);
        assert_eq!(session.document().dimensions(), (8, 8));
    }
}
#[test]
fn another_edit_session_or_successful_base_swap_rejects_an_old_token() {
    let document = image(8, 8, [1; 4]);
    let mut a = ScreenshotEditSession::new(document.clone());
    let mut b = ScreenshotEditSession::new(document);
    let token = a.base_capture_token();
    assert!(!b.replace_base_capture(token, image(9, 9, [2; 4])));
    assert!(a.replace_base_capture(token, image(9, 9, [2; 4])));
    assert!(!a.replace_base_capture(token, image(7, 7, [3; 4])));
}
#[test]
fn stale_selection_window_or_display_context_preserves_the_frozen_document() {
    for field in 0..4 {
        let mut session = ScreenshotEditSession::new(image(8, 8, [1; 4]));
        let ctx = context();
        let prepared = plan(&session, ctx, WindowRefreshDecision::ReplaceWithIndependent)
            .prepare(image(10, 6, [2; 4]))
            .unwrap()
            .unwrap();
        let mut changed = ctx;
        match field {
            0 => changed.selection = context().selection,
            1 => changed.window_id += 1,
            2 => changed.display.pixel_size.0 += 1,
            _ => changed.display.display_id += 1,
        }
        assert_eq!(
            prepared.apply(&mut session, changed, &AtomicBool::new(false)),
            Err(WindowRefreshError::StaleContext)
        );
        assert_eq!(session.revision(), 0);
        assert_eq!(session.document().dimensions(), (8, 8));
    }
}
#[test]
fn keep_mask_failure_and_either_cancellation_flag_leave_the_original_untouched() {
    let mut session = ScreenshotEditSession::new(image(8, 8, [1; 4]));
    let ctx = context();
    assert!(
        plan(&session, ctx, WindowRefreshDecision::KeepFrozen)
            .prepare(image(8, 8, [2; 4]))
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        plan(&session, ctx, WindowRefreshDecision::MaskFrozenAlpha).prepare(image(20, 20, [2; 4])),
        Err(WindowRefreshError::IncompatibleImages)
    ));
    let cancellation = Arc::new(AtomicBool::new(false));
    let pending = WindowRefreshPlan::new(
        &session,
        ctx,
        WindowRefreshDecision::ReplaceWithIndependent,
        cancellation.clone(),
    )
    .unwrap();
    let prepared = pending.prepare(image(8, 8, [2; 4])).unwrap().unwrap();
    cancellation.store(true, Ordering::Release);
    assert_eq!(
        prepared.apply(&mut session, ctx, &AtomicBool::new(false)),
        Err(WindowRefreshError::Cancelled)
    );
    let prepared = plan(&session, ctx, WindowRefreshDecision::ReplaceWithIndependent)
        .prepare(image(8, 8, [2; 4]))
        .unwrap()
        .unwrap();
    assert_eq!(
        prepared.apply(&mut session, ctx, &AtomicBool::new(true)),
        Err(WindowRefreshError::Cancelled)
    );
    assert_eq!(session.revision(), 0);
}
#[test]
fn resize_refresh_undo_and_redo_restore_the_matching_image_and_annotation_geometry() {
    let mut session = ScreenshotEditSession::new(image(20, 12, [1; 4]));
    add_mark(&mut session);
    assert!(session.undo());
    let old_token = session.base_capture_token();
    assert!(session.replace_base_capture(old_token, image(8, 6, [2; 4])));
    let after_refresh = session.revision();
    assert!(session.redo());
    assert_eq!(session.document().dimensions(), (20, 12));
    assert_eq!(session.document().annotations().len(), 1);
    assert!(session.revision() > after_refresh);
    assert!(session.undo());
    assert_eq!(session.document().dimensions(), (8, 6));
    assert!(session.document().annotations().is_empty());
    assert!(!session.replace_base_capture(old_token, image(4, 4, [3; 4])));
}
#[test]
fn revisiting_the_original_arc_never_revives_an_old_base_epoch() {
    let mut session = ScreenshotEditSession::new(image(20, 12, [1; 4]));
    let original = session.render_snapshot();
    let original_token = session.base_capture_token();
    add_then_remove(&mut session);
    assert!(session.replace_base_capture(original_token, image(8, 6, [2; 4])));
    let newer_token = session.base_capture_token();
    assert!(session.undo());
    assert!(session.undo());
    assert!(Arc::ptr_eq(
        &original.base_image,
        &session.document.base_image
    ));
    assert!(session.document().annotations().is_empty());
    assert_ne!(session.base_capture_token(), original_token);
    assert_ne!(session.base_capture_token(), newer_token);
    assert!(!session.replace_base_capture(original_token, image(4, 4, [3; 4])));
}
#[test]
fn render_snapshots_remain_immutable_and_revision_gates_stale_ocr_work() {
    let mut session = ScreenshotEditSession::new(image(8, 8, [1, 2, 3, 255]));
    let old_render = session.render_snapshot();
    let old_ocr_revision = session.revision();
    assert!(
        session.replace_base_capture(session.base_capture_token(), image(12, 6, [4, 5, 6, 255]))
    );
    assert_ne!(session.revision(), old_ocr_revision);
    assert_eq!(old_render.render_rgba().unwrap().dimensions(), (8, 8));
    assert_eq!(
        old_render.render_rgba().unwrap().get_pixel(0, 0).0,
        [1, 2, 3, 255]
    );
    assert_eq!(
        session
            .render_snapshot()
            .render_rgba()
            .unwrap()
            .dimensions(),
        (12, 6)
    );
}
#[test]
fn history_budget_excludes_active_pixels_and_deduplicates_equal_allocation_references() {
    let mut session = ScreenshotEditSession::new(image(20, 12, [1; 4]));
    add_then_remove(&mut session);
    let metadata = session.undo.iter().map(|s| s.byte_cost()).sum::<usize>();
    assert_eq!(session.retained_history_bytes(), metadata);
    session.trim_history(metadata);
    assert_eq!(session.undo.len(), 2);
    let old_capacity = session.document.base_image.as_raw().capacity();
    assert!(session.replace_base_capture(session.base_capture_token(), image(20, 12, [1; 4])));
    // Pixel equality does not make two separate RGBA allocations identical.
    assert_eq!(session.retained_history_bytes(), metadata + old_capacity);
    add_then_remove(&mut session);
    assert!(session.replace_base_capture(session.base_capture_token(), image(20, 12, [1; 4])));
    let metadata = session.undo.iter().map(|s| s.byte_cost()).sum::<usize>();
    assert_eq!(
        session.retained_history_bytes(),
        metadata + 2 * old_capacity
    );
    assert!(session.undo());
    let metadata = session
        .undo
        .iter()
        .chain(&session.redo)
        .map(|s| s.byte_cost())
        .sum::<usize>();
    assert_eq!(
        session.retained_history_bytes(),
        metadata + 2 * old_capacity
    );
}
#[test]
fn tiny_budget_evicts_oldest_cross_base_history_without_wrong_geometry_replay() {
    assert_eq!(crate::screenshot::MAX_HISTORY_BYTES, 64 * 1024 * 1024);
    let mut session = ScreenshotEditSession::new(image(80, 60, [1; 4]));
    add_then_remove(&mut session);
    assert!(session.replace_base_capture(session.base_capture_token(), image(12, 8, [2; 4])));
    add_then_remove(&mut session);
    let metadata = session.undo.iter().map(|s| s.byte_cost()).sum::<usize>();
    session.trim_history(metadata + 100); // production pruning, inexpensive injected cap
    assert_eq!(session.undo.len(), 2);
    assert!(session.undo());
    assert_eq!(session.document().dimensions(), (12, 8));
    assert!(session.undo());
    assert_eq!(session.document().dimensions(), (12, 8));
    assert!(!session.undo());
    assert!(session.redo());
    assert_eq!(session.document().dimensions(), (12, 8));
}

#[test]
fn redo_restores_old_base_with_its_crop_after_a_different_size_refresh() {
    let mut session = ScreenshotEditSession::new(image(40, 30, [1; 4]));
    session
        .set_crop(Some(Rect::new(25., 15., 10., 10.)))
        .unwrap();
    assert!(session.undo());
    let old = session.base_capture_token();
    assert!(session.replace_base_capture(old, image(8, 6, [2; 4])));
    let after_refresh = session.revision();
    assert!(session.redo());
    assert_eq!(session.document().dimensions(), (40, 30));
    assert_eq!(
        session.document().crop_rect(),
        Some(Rect::new(25., 15., 10., 10.))
    );
    assert_eq!(
        session
            .render_snapshot()
            .render_rgba()
            .unwrap()
            .dimensions(),
        (10, 10)
    );
    let after_redo = session.revision();
    assert!(after_redo > after_refresh);
    assert!(session.undo());
    assert_eq!(session.document().dimensions(), (8, 6));
    assert_eq!(session.document().crop_rect(), None);
    assert!(session.revision() > after_redo);
}

#[test]
fn mask_backend_receives_only_base_pixels_despite_annotation_crop_and_undo() {
    let mut session = ScreenshotEditSession::new(image(8, 8, [40, 80, 120, 255]));
    add_mark(&mut session);
    session.set_crop(Some(Rect::new(1., 1., 4., 4.))).unwrap();
    let ctx = context();
    let request = plan(&session, ctx, WindowRefreshDecision::MaskFrozenAlpha);
    let mut shape = ScreenshotEditSession::new(image(8, 8, [10, 20, 30, 128]));
    add_mark(&mut shape);
    shape.set_crop(Some(Rect::new(0., 0., 2., 2.))).unwrap();
    let prepared = request
        .prepare_with_alpha_mask(shape.render_snapshot(), |frozen, shape, _| {
            assert_eq!(frozen.dimensions(), (8, 8));
            assert_eq!(shape.dimensions(), (8, 8));
            assert_eq!(frozen.rgba().len(), 8 * 8 * 4);
            assert!(
                frozen
                    .rgba()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [40, 80, 120, 255])
            );
            assert!(
                shape
                    .rgba()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [10, 20, 30, 128])
            );
            Ok(image(8, 8, [40, 80, 120, 128]))
        })
        .unwrap()
        .unwrap();
    assert!(session.undo());
    assert!(session.undo());
    assert!(
        prepared
            .apply(&mut session, ctx, &AtomicBool::new(false))
            .unwrap()
    );
    assert!(session.document().annotations().is_empty());
    assert!(session.document().crop_rect().is_none());
    assert_eq!(
        session.document().render_rgba().unwrap().get_pixel(2, 2).0,
        [40, 80, 120, 128]
    );
}

#[test]
fn mask_backend_is_skipped_for_keep_replace_incompatible_and_cancelled_requests() {
    let session = ScreenshotEditSession::new(image(8, 8, [1; 4]));
    let ctx = context();
    for decision in [
        WindowRefreshDecision::KeepFrozen,
        WindowRefreshDecision::ReplaceWithIndependent,
    ] {
        plan(&session, ctx, decision)
            .prepare_with_alpha_mask(image(8, 8, [2; 4]), |_, _, _| {
                panic!("mask callback must not run")
            })
            .unwrap();
    }
    assert!(matches!(
        plan(&session, ctx, WindowRefreshDecision::MaskFrozenAlpha)
            .prepare_with_alpha_mask(image(10, 8, [2; 4]), |_, _, _| panic!(
                "invalid size callback"
            )),
        Err(WindowRefreshError::IncompatibleImages)
    ));
    let cancellation = Arc::new(AtomicBool::new(false));
    let request = WindowRefreshPlan::new(
        &session,
        ctx,
        WindowRefreshDecision::MaskFrozenAlpha,
        cancellation.clone(),
    )
    .unwrap();
    cancellation.store(true, Ordering::Release);
    assert!(matches!(
        request
            .prepare_with_alpha_mask(image(8, 8, [2; 4]), |_, _, _| panic!("cancelled callback")),
        Err(WindowRefreshError::Cancelled)
    ));
}

#[test]
fn mask_backend_rejects_wrong_size_annotations_crop_errors_and_late_cancellation() {
    for case in 0..5 {
        let session = ScreenshotEditSession::new(image(8, 8, [1; 4]));
        let ctx = context();
        let cancellation = Arc::new(AtomicBool::new(false));
        let request = WindowRefreshPlan::new(
            &session,
            ctx,
            WindowRefreshDecision::MaskFrozenAlpha,
            cancellation.clone(),
        )
        .unwrap();
        let result = request.prepare_with_alpha_mask(image(8, 8, [2; 4]), |_, _, flag| {
            let mut output =
                ScreenshotEditSession::new(image(if case == 0 { 9 } else { 8 }, 8, [2; 4]));
            match case {
                1 => {
                    add_mark(&mut output);
                }
                2 => {
                    output.set_crop(Some(Rect::new(0., 0., 4., 4.))).unwrap();
                }
                3 => return Err(WindowRefreshError::MaskFailed),
                4 => flag.store(true, Ordering::Release),
                _ => {}
            }
            Ok(output.render_snapshot())
        });
        let expected = match case {
            0..=2 => WindowRefreshError::InvalidMaskOutput,
            3 => WindowRefreshError::MaskFailed,
            _ => WindowRefreshError::Cancelled,
        };
        assert!(matches!(result, Err(error) if error == expected));
        assert_eq!(session.revision(), 0);
        assert_eq!(session.document().dimensions(), (8, 8));
        assert!(session.document().annotations().is_empty());
    }
}

#[test]
fn mask_backend_never_sees_trailing_image_storage() {
    let mut bytes = vec![1_u8; 16];
    bytes.extend_from_slice(&[222, 223, 224, 225]);
    let raster = RgbaImage::from_raw(2, 2, bytes).unwrap();
    assert_eq!(raster.as_raw().len(), 20);
    let session = ScreenshotEditSession::new(ScreenshotDocument::from_rgba(raster).unwrap());
    let result = plan(&session, context(), WindowRefreshDecision::MaskFrozenAlpha)
        .prepare_with_alpha_mask(image(2, 2, [2; 4]), |frozen, _, _| {
            assert_eq!(frozen.rgba(), &[1; 16]);
            Ok(image(2, 2, [3; 4]))
        });
    assert!(result.unwrap().is_some());
}

#[test]
fn retained_refresh_clone_cannot_republish_after_success_or_clean_undo() {
    let mut session = ScreenshotEditSession::new(image(8, 8, [1, 2, 3, 255]));
    let context = context();
    let epoch = session.base_capture_token();
    let prepared = plan(
        &session,
        context,
        WindowRefreshDecision::ReplaceWithIndependent,
    )
    .prepare(image(10, 6, [4, 5, 6, 255]))
    .unwrap()
    .unwrap();
    let retained = prepared.clone();
    let replacement = Arc::downgrade(&retained.replacement.base_image);
    assert!(
        prepared
            .apply(&mut session, context, &AtomicBool::new(false))
            .unwrap()
    );
    assert_ne!(session.base_capture_token(), epoch);
    add_mark(&mut session);
    assert!(session.undo());
    assert!(session.document().annotations().is_empty());
    let revision = session.revision();
    assert!(
        !retained
            .clone()
            .apply(&mut session, context, &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(session.revision(), revision);
    drop(session);
    assert!(
        replacement.upgrade().is_some(),
        "completion owns pixels until disposal"
    );
    std::thread::spawn(move || drop(retained)).join().unwrap();
    assert!(replacement.upgrade().is_none());
}

#[test]
fn deferred_decision_preserves_original_base_context_and_cancellation_guards() {
    for case in 0..4 {
        let mut session = ScreenshotEditSession::new(image(8, 8, [1, 2, 3, 255]));
        let context = context();
        let cancellation = Arc::new(AtomicBool::new(false));
        let pending = WindowRefreshPlan::new(
            &session,
            context,
            WindowRefreshDecision::KeepFrozen,
            cancellation.clone(),
        )
        .unwrap();
        let base = pending.base;
        let frozen = pending.frozen.base_image.clone();
        let resolved = pending.with_decision(WindowRefreshDecision::ReplaceWithIndependent);
        assert_eq!(resolved.base, base);
        assert_eq!(resolved.context, context);
        assert!(Arc::ptr_eq(&resolved.cancellation, &cancellation));
        assert!(Arc::ptr_eq(&resolved.frozen.base_image, &frozen));
        let prepared = resolved
            .prepare(image(8, 8, [4, 5, 6, 255]))
            .unwrap()
            .unwrap();
        let mut current = context;
        match case {
            0 => {
                add_mark(&mut session);
            }
            1 => cancellation.store(true, Ordering::Release),
            2 => current.window_id += 1,
            _ => {
                assert!(
                    session.replace_base_capture(session.base_capture_token(), image(8, 8, [7; 4]))
                );
            }
        }
        assert!(
            !prepared
                .apply(&mut session, current, &AtomicBool::new(false))
                .unwrap_or(false)
        );
        assert_ne!(
            session.document().base_image.get_pixel(0, 0).0,
            [4, 5, 6, 255]
        );
    }
}
