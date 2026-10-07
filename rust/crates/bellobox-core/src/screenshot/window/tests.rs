use super::*;
use image::{Rgba, RgbaImage};
fn geometry() -> AreaDisplayGeometry {
    AreaDisplayGeometry {
        display_id: 1,
        cocoa_frame: Rect::new(0., 0., 100., 80.),
        pixel_size: (200, 240),
        rotation_degrees: 0,
    }
}
fn candidate(id: u32, frame: Rect) -> FrozenWindowCandidate {
    FrozenWindowCandidate {
        window_id: id,
        owner_process_id: 10,
        frame_local_points: frame,
        layer: 0,
        alpha: 1.,
        on_screen: true,
    }
}
fn document(g: AreaDisplayGeometry) -> ScreenshotDocument {
    ScreenshotDocument::from_rgba(RgbaImage::from_fn(
        g.pixel_size.0,
        g.pixel_size.1,
        |x, y| Rgba([(x % 251) as u8, (y % 251) as u8, 47, 255]),
    ))
    .unwrap()
}
fn session(catalog: &[FrozenWindowCandidate]) -> FrozenWindowSession {
    let g = geometry();
    let mut s = FrozenWindowSession::new(g, catalog, 99).unwrap();
    assert!(
        s.accept_frozen(s.freeze_token().unwrap(), document(g), g)
            .unwrap()
    );
    s
}
fn ordinary() -> FrozenWindowSession {
    session(&[candidate(1, Rect::new(10., 10., 60., 50.))])
}
fn click(s: &mut FrozenWindowSession, point: Point) -> FrozenWindowCommit {
    s.begin_press(point, geometry()).unwrap();
    s.end_press(point, geometry()).unwrap().unwrap()
}
#[test]
fn freeze_must_finish_before_any_pointer_selection() {
    let mut s = FrozenWindowSession::new(geometry(), &[], 99).unwrap();
    assert_eq!(s.phase(), FrozenWindowPhase::Freezing);
    assert_eq!(
        s.hover(Point::new(20., 20.), geometry()),
        Err(FrozenWindowError::InvalidState)
    );
    assert_eq!(
        s.begin_press(Point::new(20., 20.), geometry()),
        Err(FrozenWindowError::InvalidState)
    );
}
#[test]
fn hover_keeps_catalog_front_to_back_order_without_size_sorting() {
    let large = candidate(1, Rect::new(5., 5., 90., 70.));
    let small = candidate(2, Rect::new(20., 20., 30., 30.));
    for (catalog, expected) in [([large, small], 1), ([small, large], 2)] {
        let mut s = session(&catalog);
        s.hover(Point::new(30., 30.), geometry()).unwrap();
        assert_eq!(s.hovered().unwrap().window_id, expected);
    }
}
#[test]
fn cocoa_half_open_edges_are_preserved_after_local_y_inversion() {
    let window = candidate(1, Rect::new(10., 10., 30., 30.));
    let mut s = session(&[window]);
    for (point, hit) in [
        (Point::new(10., 20.), true),
        (Point::new(40., 20.), false),
        (Point::new(20., 10.), false),
        (Point::new(20., 40.), true),
        (Point::new(10., 40.), true),
        (Point::new(40., 40.), false),
    ] {
        s.hover(point, geometry()).unwrap();
        assert_eq!(s.hovered().is_some(), hit, "{point:?}");
    }
}
#[test]
fn mouse_down_refreshes_hover_and_latches_it_through_dragging() {
    let a = candidate(1, Rect::new(5., 5., 30., 40.));
    let b = candidate(2, Rect::new(40., 5., 40., 40.));
    let mut s = session(&[a, b]);
    s.hover(Point::new(20., 20.), geometry()).unwrap();
    s.begin_press(Point::new(50., 20.), geometry()).unwrap();
    assert_eq!(s.hovered().unwrap(), b);
    s.hover(Point::new(20., 20.), geometry()).unwrap();
    assert_eq!(s.preview_rect(), Some(b.frame_local_points));
    assert!(
        s.end_press(Point::new(20., 20.), geometry())
            .unwrap()
            .is_none()
    );
    assert_eq!(s.hovered().unwrap(), a);
    assert_eq!(s.phase(), FrozenWindowPhase::Selecting);
}
#[test]
fn both_clamped_axes_must_be_below_eight_and_drag_never_falls_back() {
    for (dx, dy, commits) in [
        (0., 0., true),
        (7., 7., true),
        (7.999, 0., true),
        (8., 0., false),
        (0., 8., false),
        (30., 30., false),
        (-8., 0., false),
    ] {
        let mut s = ordinary();
        s.begin_press(Point::new(30., 30.), geometry()).unwrap();
        assert_eq!(
            s.end_press(Point::new(30. + dx, 30. + dy), geometry())
                .unwrap()
                .is_some(),
            commits
        );
    }
    // Source measures the final clamped rectangle, not total travel distance.
    let mut s = ordinary();
    s.begin_press(Point::new(30., 30.), geometry()).unwrap();
    s.hover(Point::new(90., 70.), geometry()).unwrap();
    assert!(
        s.end_press(Point::new(31., 31.), geometry())
            .unwrap()
            .is_some()
    );
}
#[test]
fn empty_space_and_invalid_mouse_up_do_not_create_display_or_area_targets() {
    let mut s = ordinary();
    assert!(
        s.end_press(Point::new(20., 20.), geometry())
            .unwrap()
            .is_none()
    );
    s.begin_press(Point::new(90., 70.), geometry()).unwrap();
    assert!(
        s.end_press(Point::new(90., 70.), geometry())
            .unwrap()
            .is_none()
    );
    assert_eq!(s.phase(), FrozenWindowPhase::Selecting);
    assert!(s.preview_rect().is_none());
    s.hover(Point::new(-1., 20.), geometry()).unwrap();
    assert!(s.hovered().is_none());
}
#[test]
fn source_eligibility_filters_own_layers_alpha_visibility_and_small_windows() {
    let eligible = candidate(2, Rect::new(0., 0., 90., 70.));
    for case in 0..6 {
        let mut front = candidate(1, Rect::new(10., 10., 50., 50.));
        match case {
            0 => front.owner_process_id = 99,
            1 => front.layer = 3,
            2 => front.on_screen = false,
            3 => front.alpha = 0.01,
            4 => front.frame_local_points = Rect::new(10., 10., 8., 11.99),
            _ => front.frame_local_points = Rect::new(10., 10., 7.99, 50.),
        }
        let mut s = session(&[front, eligible]);
        s.hover(Point::new(12., 12.), geometry()).unwrap();
        assert_eq!(s.hovered().unwrap().window_id, 2);
    }
    let mut s = session(&[candidate(1, Rect::new(10., 10., 8., 12.))]);
    s.hover(Point::new(12., 12.), geometry()).unwrap();
    assert!(s.hovered().is_some());
}
#[test]
fn whole_window_must_fit_the_main_display_without_clipping_fallback() {
    for frame in [
        Rect::new(-1., 10., 20., 20.),
        Rect::new(90., 10., 20., 20.),
        Rect::new(10., 70., 20., 20.),
    ] {
        let mut s = session(&[candidate(1, frame)]);
        s.hover(Point::new(95., 75.), geometry()).unwrap();
        assert!(s.hovered().is_none());
    }
    let mut g = geometry();
    g.rotation_degrees = 90;
    assert!(matches!(
        FrozenWindowSession::new(g, &[], 99),
        Err(FrozenWindowError::InvalidGeometry)
    ));
    g.rotation_degrees = 0;
    g.cocoa_frame.x = 100.;
    assert!(matches!(
        FrozenWindowSession::new(g, &[], 99),
        Err(FrozenWindowError::InvalidGeometry)
    ));
}
#[test]
fn malformed_or_ambiguous_catalog_is_rejected_before_owned_copy() {
    let good = candidate(1, Rect::new(10., 10., 20., 20.));
    assert!(FrozenWindowSession::new(geometry(), &[good, good], 99).is_err());
    assert!(
        FrozenWindowSession::new(
            geometry(),
            &vec![good; MAX_FROZEN_WINDOW_CANDIDATES + 1],
            99
        )
        .is_err()
    );
    for field in 0..6 {
        let mut bad = good;
        match field {
            0 => bad.window_id = 0,
            1 => bad.owner_process_id = 0,
            2 => bad.alpha = f64::NAN,
            3 => bad.alpha = 1.1,
            4 => bad.frame_local_points.width = f32::INFINITY,
            _ => bad.frame_local_points.height = 0.,
        }
        assert!(matches!(
            FrozenWindowSession::new(geometry(), &[bad], 99),
            Err(FrozenWindowError::InvalidCatalog)
        ));
    }
}
#[test]
fn physically_cropped_document_has_correct_ratios_clean_undo_and_no_hidden_pixels() {
    let mut s = ordinary();
    let commit = click(&mut s, Point::new(20., 20.));
    assert_eq!(commit.pixel_crop(), Rect::new(20., 30., 120., 150.));
    let mut selection = commit.materialize(&AtomicBool::new(false)).unwrap();
    assert_eq!(selection.editor.document().dimensions(), (120, 150));
    assert_eq!(selection.editor.document().crop_rect(), None);
    assert_eq!(
        selection.selection_cocoa_points,
        Rect::new(10., 20., 60., 50.)
    );
    assert!(!selection.editor.has_edits());
    assert!(!selection.editor.can_undo());
    assert!(!selection.editor.undo());
    selection
        .editor
        .set_crop(Some(Rect::new(5., 5., 20., 20.)))
        .unwrap();
    assert!(selection.editor.undo());
    selection.editor.set_crop(None).unwrap();
    assert_eq!(selection.editor.document().visible_dimensions(), (120, 150));
    assert_eq!(selection.editor.document().dimensions(), (120, 150));
    assert_eq!(
        selection.editor.document().base_image.get_pixel(0, 0),
        &Rgba([20, 30, 47, 255])
    );
    assert_eq!(
        selection.editor.document().base_image.get_pixel(119, 149),
        &Rgba([139, 179, 47, 255])
    );
}
#[test]
fn frozen_back_window_crop_preserves_front_window_occlusion() {
    let (g, catalog, document) = synthetic_window_fixture().unwrap();
    let mut s = FrozenWindowSession::new(g, &catalog, 999).unwrap();
    s.accept_frozen(s.freeze_token().unwrap(), document, g)
        .unwrap();
    let point = Point::new(150., 200.); // exposed part of the back window
    s.begin_press(point, g).unwrap();
    let crop = s
        .end_press(point, g)
        .unwrap()
        .unwrap()
        .materialize(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(crop.candidate.window_id, 2);
    assert_eq!(crop.editor.document().dimensions(), (840, 460));
    // Global (350, 200) is still covered by the front orange window.
    assert_eq!(
        crop.editor.document().base_image.get_pixel(500, 60),
        &Rgba([255, 224, 186, 255])
    );
}
#[test]
fn duplicate_commit_and_stale_publication_are_rejected() {
    let mut s = ordinary();
    let commit = click(&mut s, Point::new(20., 20.));
    let token = commit.token();
    assert_eq!(s.phase(), FrozenWindowPhase::Committed);
    assert!(s.end_press(Point::new(20., 20.), geometry()).is_err());
    assert!(s.accepts_commit(token, geometry()).unwrap());
    s.cancel();
    assert!(!s.accepts_commit(token, geometry()).unwrap());
    assert!(matches!(
        commit.materialize(&AtomicBool::new(false)),
        Err(FrozenWindowError::Cancelled)
    ));
}
#[test]
fn restarting_or_reopening_cannot_accept_an_older_freeze_or_commit() {
    let mut s = ordinary();
    let commit = click(&mut s, Point::new(20., 20.));
    let old_commit = commit.token();
    let catalog = [candidate(1, Rect::new(10., 10., 60., 50.))];
    let next = s.restart(geometry(), &catalog, 99).unwrap();
    assert!(!s.accepts_commit(old_commit, geometry()).unwrap());
    assert!(
        s.accept_frozen(next, document(geometry()), geometry())
            .unwrap()
    );
    let mut other = ordinary();
    let other_commit = click(&mut other, Point::new(20., 20.));
    assert!(!s.accepts_commit(other_commit.token(), geometry()).unwrap());
    let mut pending = FrozenWindowSession::new(geometry(), &catalog, 99).unwrap();
    let old = pending.freeze_token().unwrap();
    let new = pending.restart(geometry(), &catalog, 99).unwrap();
    assert!(
        !pending
            .accept_frozen(old, document(geometry()), geometry())
            .unwrap()
    );
    assert!(
        pending
            .accept_frozen(new, document(geometry()), geometry())
            .unwrap()
    );
}
#[test]
fn topology_change_before_pointer_or_publication_invalidates_selection() {
    for field in 0..4 {
        let mut s = ordinary();
        let commit = click(&mut s, Point::new(20., 20.));
        let mut changed = geometry();
        match field {
            0 => changed.display_id += 1,
            1 => changed.pixel_size.0 += 1,
            2 => changed.cocoa_frame.width += 1.,
            _ => changed.rotation_degrees = 90,
        }
        assert_eq!(
            s.accepts_commit(commit.token(), changed),
            Err(FrozenWindowError::TopologyChanged)
        );
        assert!(matches!(
            commit.materialize(&AtomicBool::new(false)),
            Err(FrozenWindowError::Cancelled)
        ));
    }
    let mut s = ordinary();
    let mut changed = geometry();
    changed.display_id += 1;
    assert_eq!(
        s.hover(Point::new(20., 20.), changed),
        Err(FrozenWindowError::TopologyChanged)
    );
    assert_eq!(s.phase(), FrozenWindowPhase::Cancelled);
}
#[test]
fn either_original_or_handoff_cancellation_blocks_pixel_materialization() {
    let mut s = ordinary();
    let commit = click(&mut s, Point::new(20., 20.));
    assert!(matches!(
        commit.materialize(&AtomicBool::new(true)),
        Err(FrozenWindowError::Cancelled)
    ));
    let mut s = ordinary();
    let original = s.cancellation();
    let commit = click(&mut s, Point::new(20., 20.));
    original.store(true, Ordering::Release);
    assert!(matches!(
        commit.materialize(&AtomicBool::new(false)),
        Err(FrozenWindowError::Cancelled)
    ));
}
#[test]
fn close_drop_and_unlocked_focus_loss_cancel_outstanding_work() {
    let mut s = ordinary();
    s.begin_press(Point::new(20., 20.), geometry()).unwrap();
    s.focus_lost();
    assert_eq!(s.phase(), FrozenWindowPhase::Cancelled);
    let mut s = ordinary();
    let commit = click(&mut s, Point::new(20., 20.));
    drop(s);
    assert!(matches!(
        commit.materialize(&AtomicBool::new(false)),
        Err(FrozenWindowError::Cancelled)
    ));
}
#[test]
fn invalid_pointer_never_commits_or_chooses_a_fallback() {
    let mut s = ordinary();
    for point in [Point::new(f32::NAN, 1.), Point::new(1., f32::INFINITY)] {
        assert_eq!(
            s.hover(point, geometry()),
            Err(FrozenWindowError::InvalidPoint)
        );
        assert_eq!(
            s.begin_press(point, geometry()),
            Err(FrozenWindowError::InvalidPoint)
        );
    }
}

#[test]
fn cancellation_after_materialization_still_blocks_final_handoff() {
    let mut s = ordinary();
    let commit = click(&mut s, Point::new(20., 20.));
    let selection = commit.materialize(&AtomicBool::new(false)).unwrap();
    assert!(s.accepts_commit(selection.token, geometry()).unwrap());
    s.cancel();
    assert!(!s.accepts_commit(selection.token, geometry()).unwrap());
}
#[test]
fn frozen_snapshot_dimensions_and_clean_baseline_are_required() {
    let mut s = FrozenWindowSession::new(geometry(), &[], 99).unwrap();
    let token = s.freeze_token().unwrap();
    let mut cropped = document(geometry());
    cropped.crop_rect = Some(Rect::new(0., 0., 10., 10.));
    assert_eq!(
        s.accept_frozen(token, cropped, geometry()),
        Err(FrozenWindowError::InvalidSnapshot)
    );
    assert_eq!(s.phase(), FrozenWindowPhase::Cancelled);
    let mut s = FrozenWindowSession::new(geometry(), &[], 99).unwrap();
    let wrong = ScreenshotDocument::from_rgba(RgbaImage::new(2, 2)).unwrap();
    assert_eq!(
        s.accept_frozen(s.freeze_token().unwrap(), wrong, geometry()),
        Err(FrozenWindowError::InvalidSnapshot)
    );
}

#[test]
fn display_bottom_edge_is_outside_the_local_viewport() {
    let mut s = session(&[candidate(1, Rect::new(10., 60., 30., 20.))]);
    s.hover(Point::new(20., 79.99), geometry()).unwrap();
    assert!(s.hovered().is_some());
    s.hover(Point::new(20., 80.), geometry()).unwrap();
    assert!(s.hovered().is_none());
    s.hover(Point::new(100., 70.), geometry()).unwrap();
    assert!(s.hovered().is_none());
}
