use super::*;
use image::{Rgba, RgbaImage};

fn geometry(id: u32, frame: Rect, pixels: (u32, u32)) -> AreaDisplayGeometry {
    AreaDisplayGeometry {
        display_id: id,
        cocoa_frame: frame,
        pixel_size: pixels,
        rotation_degrees: 0,
    }
}
fn primary() -> AreaDisplayGeometry {
    geometry(1, Rect::new(0., 0., 200., 100.), (400, 200))
}
fn content(size: (u32, u32)) -> RgbaImage {
    RgbaImage::from_fn(size.0, size.1, |x, y| {
        Rgba([(x % 251) as u8, (y % 251) as u8, ((x + y) % 251) as u8, 255])
    })
}
fn document(g: AreaDisplayGeometry) -> ScreenshotDocument {
    ScreenshotDocument::from_rgba(content(g.pixel_size)).unwrap()
}
fn frozen(g: AreaDisplayGeometry) -> FrozenAreaSession {
    let mut state = FrozenAreaSession::new(g).unwrap();
    assert!(
        state
            .accept_frozen(state.freeze_token().unwrap(), document(g), g)
            .unwrap()
    );
    state
}
fn select(state: &mut FrozenAreaSession, a: Point, b: Point) -> AreaSelection {
    let display = state.display();
    state.begin_drag(a, display).unwrap();
    state.end_drag(b, display).unwrap().unwrap()
}
fn decode(png: &[u8]) -> RgbaImage {
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8()
}

#[test]
fn overlay_selection_cannot_start_before_frozen_pixels_arrive() {
    let display = primary();
    let mut state = FrozenAreaSession::new(display).unwrap();
    assert_eq!(state.phase(), AreaPhase::Freezing);
    assert!(!state.can_select());
    assert!(state.frozen_document().is_none());
    assert_eq!(
        state.begin_drag(Point::new(10., 10.), display),
        Err(AreaError::InvalidState)
    );
    assert!(matches!(
        state.end_drag(Point::new(50., 50.), display),
        Err(AreaError::InvalidState)
    ));
    let token = state.freeze_token().unwrap();
    assert!(
        state
            .accept_frozen(token, document(display), display)
            .unwrap()
    );
    assert_eq!(state.phase(), AreaPhase::Selecting);
    assert!(state.can_select());
    assert!(state.freeze_token().is_none());
}
#[test]
fn actual_mouse_up_point_is_committed_at_two_x() {
    let mut state = frozen(primary());
    let display = state.display();
    state.begin_drag(Point::new(10., 12.), display).unwrap();
    state.update_drag(Point::new(30., 24.), display).unwrap();
    let selected = state
        .end_drag(Point::new(50., 42.), display)
        .unwrap()
        .unwrap();
    assert_eq!(
        selected.selection_local_points,
        Rect::new(10., 12., 40., 30.)
    );
    assert_eq!(
        selected.selection_cocoa_points,
        Rect::new(10., 58., 40., 30.)
    );
    assert_eq!(selected.pixel_crop, Some(Rect::new(20., 24., 80., 60.)));
    assert_eq!(selected.editor.document().visible_dimensions(), (80, 60));
}
#[test]
fn reversed_drag_has_identical_crop_and_pixels() {
    let mut a = frozen(primary());
    let mut b = frozen(primary());
    let forward = select(&mut a, Point::new(10., 12.), Point::new(50., 42.));
    let reverse = select(&mut b, Point::new(50., 42.), Point::new(10., 12.));
    assert_eq!(forward.pixel_crop, reverse.pixel_crop);
    assert_eq!(
        forward.editor.render_png().unwrap(),
        reverse.editor.render_png().unwrap()
    );
}
#[test]
fn pixels_stay_frozen_when_the_live_page_changes_after_capture() {
    let display = primary();
    let original = content(display.pixel_size);
    let mut live_page = original.clone();
    let mut state = FrozenAreaSession::new(display).unwrap();
    let token = state.freeze_token().unwrap();
    // This is the one and only supplied capture. Drag/commit have no acquisition API.
    state
        .accept_frozen(
            token,
            ScreenshotDocument::from_rgba(live_page.clone()).unwrap(),
            display,
        )
        .unwrap();
    for pixel in live_page.pixels_mut() {
        *pixel = Rgba([255, 0, 0, 255]);
    }
    let selected = select(&mut state, Point::new(10., 10.), Point::new(70., 50.));
    let exported = decode(&selected.editor.render_png().unwrap());
    assert_eq!(
        exported,
        image::imageops::crop_imm(&original, 20, 20, 120, 80).to_image()
    );
    assert_ne!(
        exported,
        image::imageops::crop_imm(&live_page, 20, 20, 120, 80).to_image()
    );
    assert!(Arc::ptr_eq(
        &selected.editor.document().base_image,
        &state.frozen_document().unwrap().base_image
    ));
}
#[test]
fn one_and_two_x_and_nonuniform_ratios_use_actual_pixel_dimensions() {
    for (pixels, expected) in [
        ((200, 100), Rect::new(10., 12., 40., 30.)),
        ((400, 200), Rect::new(20., 24., 80., 60.)),
        ((300, 250), Rect::new(15., 30., 60., 75.)),
    ] {
        let display = geometry(5, Rect::new(-1920., 900., 200., 100.), pixels);
        let mut state = frozen(display);
        let selected = select(&mut state, Point::new(10., 12.), Point::new(50., 42.));
        assert_eq!(selected.pixel_crop, Some(expected));
        assert_eq!(
            selected.selection_cocoa_points,
            Rect::new(-1910., 958., 40., 30.)
        );
        assert_eq!(selected.editor.document().dimensions(), pixels);
    }
}
#[test]
fn pixel_edges_round_outward_and_do_not_depend_on_global_origin() {
    let local = Rect::new(10.25, 12.25, 40.5, 30.5);
    for origin in [(0., 0.), (-1920., 1080.), (-999_000., -900_000.)] {
        let display = geometry(1, Rect::new(origin.0, origin.1, 200., 100.), (300, 250));
        assert_eq!(
            display.pixel_crop(local).unwrap(),
            Some(Rect::new(15., 30., 62., 77.))
        );
    }
}
#[test]
fn cocoa_and_local_coordinates_round_trip_negative_above_and_below_displays() {
    for frame in [
        Rect::new(-200., 0., 200., 100.),
        Rect::new(0., 100., 200., 100.),
        Rect::new(200., -100., 200., 100.),
    ] {
        let display = geometry(1, frame, (400, 200));
        let local = Rect::new(11.5, 22.25, 50.5, 32.25);
        assert_eq!(
            display
                .cocoa_rect_to_local(display.local_rect_to_cocoa(local).unwrap())
                .unwrap(),
            local
        );
    }
    assert_eq!(
        core_graphics_frame_to_cocoa(Rect::new(-200., 0., 200., 100.), 100.).unwrap(),
        Rect::new(-200., 0., 200., 100.)
    );
    assert_eq!(
        core_graphics_frame_to_cocoa(Rect::new(0., -100., 200., 100.), 100.).unwrap(),
        Rect::new(0., 100., 200., 100.)
    );
    assert_eq!(
        core_graphics_frame_to_cocoa(Rect::new(0., 100., 200., 100.), 100.).unwrap(),
        Rect::new(0., -100., 200., 100.)
    );
}
#[test]
fn selection_clamps_to_its_display_in_both_drag_directions() {
    for (a, b) in [
        (Point::new(-10., -20.), Point::new(250., 150.)),
        (Point::new(250., 150.), Point::new(-10., -20.)),
    ] {
        let mut state = frozen(primary());
        let selected = select(&mut state, a, b);
        assert_eq!(
            selected.selection_local_points,
            Rect::new(0., 0., 200., 100.)
        );
        assert_eq!(selected.pixel_crop, None);
        assert_eq!(
            selected.editor.document().visible_dimensions(),
            primary().pixel_size
        );
    }
}
#[test]
fn click_tiny_and_skinny_drags_reset_without_committing() {
    for end in [
        Point::new(20., 20.),
        Point::new(27.9, 27.9),
        Point::new(21., 90.),
        Point::new(100., 21.),
    ] {
        let display = primary();
        let mut state = frozen(display);
        state.begin_drag(Point::new(20., 20.), display).unwrap();
        assert!(state.end_drag(end, display).unwrap().is_none());
        assert_eq!(state.phase(), AreaPhase::Selecting);
        assert!(state.can_select());
        assert!(state.drag_rect().is_none());
    }
    let mut state = frozen(primary());
    assert_eq!(
        select(&mut state, Point::new(20., 20.), Point::new(28., 28.)).pixel_crop,
        Some(Rect::new(40., 40., 16., 16.))
    );
}
#[test]
fn source_six_point_preview_does_not_relax_eight_point_commit_minimum() {
    let display = primary();
    let mut state = frozen(display);
    state.begin_drag(Point::new(20., 20.), display).unwrap();
    state.update_drag(Point::new(25., 25.), display).unwrap();
    assert!(state.preview_rect().is_none());
    state.update_drag(Point::new(26., 25.), display).unwrap();
    assert_eq!(state.preview_rect(), Some(Rect::new(20., 20., 6., 5.)));
    assert!(
        state
            .end_drag(Point::new(26., 25.), display)
            .unwrap()
            .is_none()
    );
}
#[test]
fn initial_capture_crop_is_a_clean_baseline_and_adjustment_is_one_undo_step() {
    let mut state = frozen(primary());
    let mut selected = select(&mut state, Point::new(10., 10.), Point::new(60., 40.));
    assert!(!selected.editor.has_edits());
    assert!(!selected.editor.can_undo());
    assert!(!selected.editor.can_redo());
    let initial = selected.pixel_crop;
    let mut adjustment = selection::SelectionAdjustmentDraft::begin(&selected.editor, true, 2.)
        .unwrap()
        .unwrap();
    adjustment.update(Rect::new(30., 30., 120., 70.)).unwrap();
    assert!(adjustment.commit(&mut selected.editor).unwrap());
    assert!(selected.editor.has_edits());
    assert!(selected.editor.undo());
    assert_eq!(selected.editor.document().crop_rect(), initial);
    assert!(!selected.editor.has_edits());
    assert!(!selected.editor.can_undo());
}
#[test]
fn commit_publishes_once_and_locks_dragging() {
    let display = primary();
    let mut state = frozen(display);
    let _ = select(&mut state, Point::new(10., 10.), Point::new(60., 40.));
    assert_eq!(state.phase(), AreaPhase::Committed);
    assert!(!state.can_select());
    assert!(matches!(
        state.end_drag(Point::new(80., 80.), display),
        Err(AreaError::InvalidState)
    ));
    assert_eq!(
        state.begin_drag(Point::new(0., 0.), display),
        Err(AreaError::InvalidState)
    );
}
#[test]
fn cancelled_freeze_late_reply_cannot_create_selection() {
    let display = primary();
    let mut state = FrozenAreaSession::new(display).unwrap();
    let token = state.freeze_token().unwrap();
    let flag = state.cancellation();
    state.cancel();
    assert!(flag.load(Ordering::Acquire));
    assert!(
        !state
            .accept_frozen(token, document(display), display)
            .unwrap()
    );
    assert_eq!(state.phase(), AreaPhase::Cancelled);
    assert!(state.frozen_document().is_none());
}
#[test]
fn newer_freeze_rejects_old_generation_and_uses_a_fresh_cancellation_flag() {
    let display = primary();
    let mut state = FrozenAreaSession::new(display).unwrap();
    let old = state.freeze_token().unwrap();
    let old_flag = state.cancellation();
    let next = state.restart(display).unwrap();
    assert_ne!(old, next);
    assert!(old_flag.load(Ordering::Acquire));
    assert!(!state.cancellation().load(Ordering::Acquire));
    assert!(
        !state
            .accept_frozen(old, document(display), display)
            .unwrap()
    );
    assert!(
        state
            .accept_frozen(next, document(display), display)
            .unwrap()
    );
    assert!(
        !state
            .accept_frozen(next, document(display), display)
            .unwrap()
    );
}
#[test]
fn invalid_newer_request_cancels_previous_selection() {
    let mut state = frozen(primary());
    let invalid = AreaDisplayGeometry {
        display_id: 0,
        ..primary()
    };
    assert_eq!(state.restart(invalid), Err(AreaError::InvalidGeometry));
    assert_eq!(state.phase(), AreaPhase::Cancelled);
    assert!(state.frozen_document().is_none());
}
#[test]
fn tokens_cannot_transfer_between_identical_display_sessions() {
    let display = primary();
    let a = FrozenAreaSession::new(display).unwrap();
    let mut b = FrozenAreaSession::new(display).unwrap();
    assert!(
        !b.accept_frozen(a.freeze_token().unwrap(), document(display), display)
            .unwrap()
    );
    assert_eq!(b.phase(), AreaPhase::Freezing);
}
#[test]
fn drop_signals_outstanding_freeze_without_holding_a_window_alive() {
    let flag;
    {
        let state = FrozenAreaSession::new(primary()).unwrap();
        flag = state.cancellation();
    }
    assert!(flag.load(Ordering::Acquire));
}
#[test]
fn focus_loss_cancels_only_unlocked_selection_after_freeze() {
    let display = primary();
    let mut state = FrozenAreaSession::new(display).unwrap();
    assert!(!state.focus_lost());
    assert_eq!(state.phase(), AreaPhase::Freezing);
    state
        .accept_frozen(state.freeze_token().unwrap(), document(display), display)
        .unwrap();
    assert!(state.focus_lost());
    assert_eq!(state.phase(), AreaPhase::Cancelled);
    let mut state = frozen(display);
    state.begin_drag(Point::new(10., 10.), display).unwrap();
    assert!(state.focus_lost());
    let mut state = frozen(display);
    let _ = select(&mut state, Point::new(10., 10.), Point::new(60., 40.));
    assert!(!state.focus_lost());
    assert_eq!(state.phase(), AreaPhase::Committed);
}
#[test]
fn all_topology_components_are_checked_before_drag_commit() {
    let display = primary();
    for changed in [
        AreaDisplayGeometry {
            display_id: 2,
            ..display
        },
        AreaDisplayGeometry {
            cocoa_frame: Rect::new(-200., 0., 200., 100.),
            ..display
        },
        AreaDisplayGeometry {
            pixel_size: (200, 100),
            ..display
        },
        AreaDisplayGeometry {
            rotation_degrees: 90,
            ..display
        },
    ] {
        let mut state = frozen(display);
        state.begin_drag(Point::new(10., 10.), display).unwrap();
        assert!(matches!(
            state.end_drag(Point::new(60., 40.), changed),
            Err(AreaError::TopologyChanged)
        ));
        assert_eq!(state.phase(), AreaPhase::Cancelled);
        assert!(state.frozen_document().is_none());
    }
}
#[test]
fn topology_change_while_freezing_never_makes_a_selector_ready() {
    let display = primary();
    let mut state = FrozenAreaSession::new(display).unwrap();
    let token = state.freeze_token().unwrap();
    let changed = AreaDisplayGeometry {
        display_id: 2,
        ..display
    };
    assert_eq!(
        state.accept_frozen(token, document(display), changed),
        Err(AreaError::TopologyChanged)
    );
    assert!(!state.can_select());
}
#[test]
fn invalid_snapshot_dimensions_or_prior_crop_fail_closed() {
    let display = primary();
    let mut state = FrozenAreaSession::new(display).unwrap();
    let token = state.freeze_token().unwrap();
    let wrong = ScreenshotDocument::from_rgba(content((100, 100))).unwrap();
    assert_eq!(
        state.accept_frozen(token, wrong, display),
        Err(AreaError::InvalidSnapshot)
    );
    let mut state = FrozenAreaSession::new(display).unwrap();
    let token = state.freeze_token().unwrap();
    let mut cropped = document(display);
    cropped.crop_rect = Some(Rect::new(0., 0., 40., 40.));
    assert_eq!(
        state.accept_frozen(token, cropped, display),
        Err(AreaError::InvalidSnapshot)
    );
}
#[test]
fn invalid_geometry_and_nonfinite_points_never_mutate_capture() {
    for invalid in [
        AreaDisplayGeometry {
            display_id: 0,
            ..primary()
        },
        AreaDisplayGeometry {
            pixel_size: (u32::MAX, u32::MAX),
            ..primary()
        },
        AreaDisplayGeometry {
            cocoa_frame: Rect::new(f32::NAN, 0., 200., 100.),
            ..primary()
        },
        AreaDisplayGeometry {
            rotation_degrees: 360,
            ..primary()
        },
    ] {
        assert!(matches!(
            FrozenAreaSession::new(invalid),
            Err(AreaError::InvalidGeometry)
        ));
    }
    let display = primary();
    let mut state = frozen(display);
    assert_eq!(
        state.begin_drag(Point::new(f32::INFINITY, 1.), display),
        Err(AreaError::InvalidPoint)
    );
    assert_eq!(state.phase(), AreaPhase::Selecting);
    assert!(state.drag_rect().is_none());
}
#[test]
fn dim_bands_cover_only_unselected_frozen_pixels() {
    let display = primary();
    let mut state = frozen(display);
    state.begin_drag(Point::new(20., 10.), display).unwrap();
    state.update_drag(Point::new(80., 50.), display).unwrap();
    let bands = state.dim_bands().unwrap();
    let area: f32 = bands.iter().map(|r| r.width * r.height).sum();
    assert_eq!(area + 60. * 40., 200. * 100.);
    for y in 0..100 {
        for x in 0..200 {
            let p = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let coverage = bands.iter().filter(|r| r.contains(p)).count();
            assert_eq!(
                coverage,
                usize::from(!Rect::new(20., 10., 60., 40.).contains(p))
            );
        }
    }
}
#[test]
fn cancel_flag_is_checked_before_a_late_pointer_commit() {
    let display = primary();
    let mut state = frozen(display);
    state.begin_drag(Point::new(10., 10.), display).unwrap();
    state.cancellation().store(true, Ordering::Release);
    assert!(matches!(
        state.end_drag(Point::new(60., 40.), display),
        Err(AreaError::Cancelled)
    ));
    assert!(state.frozen_document().is_none());
}

#[test]
fn traced_fractional_mouse_events_keep_source_outward_coverage() {
    let display = geometry(1, Rect::new(0., 0., 800., 500.), (1600, 1000));
    let mut integer = frozen(display);
    let exact = select(&mut integer, Point::new(100., 80.), Point::new(400., 280.));
    assert_eq!(exact.pixel_crop, Some(Rect::new(200., 160., 600., 400.)));

    // Native cloud trace: fixed integer canvas origin (20,84), size (800,500),
    // scale 1. The injected pointer events themselves were fractional.
    let start = Point::new(100.00184, 80.0025);
    let end = Point::new(400.0064, 280.00555);
    let mut forward = frozen(display);
    let crop = select(&mut forward, start, end);
    assert_eq!(crop.pixel_crop, Some(Rect::new(200., 160., 601., 401.)));
    assert!(!crop.editor.has_edits());
    let mut reverse = frozen(display);
    assert_eq!(select(&mut reverse, end, start).pixel_crop, crop.pixel_crop);
}
