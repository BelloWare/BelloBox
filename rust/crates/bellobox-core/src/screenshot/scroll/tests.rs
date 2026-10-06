use super::*;
use image::{Rgba, RgbaImage};

fn page(width: u32, height: u32) -> RgbaImage {
    let mut state = 0x2545_f491u32;
    let mut raw = Vec::new();
    for _ in 0..height {
        let mut color = [0u32; 3];
        for c in &mut color {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *c = state % 256;
        }
        raw.push(color);
    }
    RgbaImage::from_fn(width, height, |_, y| {
        let start = y.saturating_sub(2);
        let end = (y + 2).min(height - 1);
        let mut c = [0; 4];
        for channel in 0..3 {
            c[channel] = (raw[start as usize..=end as usize]
                .iter()
                .map(|v| v[channel])
                .sum::<u32>()
                / (end - start + 1)) as u8;
        }
        c[3] = 255;
        Rgba(c)
    })
}
fn frame(page: &RgbaImage, offset: u32, height: u32) -> ScrollFrame {
    ScrollFrame::from_rgba(
        image::imageops::crop_imm(page, 0, offset, page.width(), height).to_image(),
    )
    .unwrap()
}
fn feed(session: &mut ScrollSession, frame: ScrollFrame) {
    let job = session.snapshot_for_analysis(frame).unwrap();
    assert!(session.accept_analysis(analyze_sample(job)).unwrap());
}
fn finish(session: &mut ScrollSession, final_frame: ScrollFrame) -> ScrollResult {
    session.begin_finish().unwrap();
    while session.final_samples_remaining() > 0 {
        feed(session, final_frame.clone());
    }
    let job = session.finish_snapshot().unwrap();
    session.accept_finish(finish_scroll(job)).unwrap().unwrap()
}
fn full_pixels(result: &ScrollResult) -> RgbaImage {
    result.document.base_image.as_ref().clone()
}
fn with_footer(frame: ScrollFrame, rows: u32) -> ScrollFrame {
    let mut image = frame.0.as_ref().clone();
    for y in frame.height() - rows..frame.height() {
        for x in 0..frame.width() {
            image.put_pixel(
                x,
                y,
                Rgba(if x > 10 && x < 65 && y % 12 < 4 {
                    [20, 180, 130, 255]
                } else {
                    [35, 35, 35, 255]
                }),
            );
        }
    }
    ScrollFrame::from_rgba(image).unwrap()
}

#[test]
fn exact_three_frame_overlap_and_pixels() {
    let source = page(120, 1000);
    let frames: Vec<_> = [0, 150, 300].map(|y| frame(&source, y, 300)).into();
    let result = stitch(&frames, &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.document.dimensions(), (120, 600));
    assert_eq!(
        full_pixels(&result),
        image::imageops::crop_imm(&source, 0, 0, 120, 600).to_image()
    );
    assert!(result.notes.is_empty());
}
#[test]
fn upward_frame_order_preserves_original_indices() {
    let source = page(120, 1000);
    let config = StitchConfig {
        direction: ScrollDirection::Up,
        ..Default::default()
    };
    let result = stitch(
        &[frame(&source, 150, 300), frame(&source, 0, 300)],
        &config,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(result.document.height(), 450);
    assert_eq!(
        result
            .placements
            .iter()
            .map(|p| p.frame_index)
            .collect::<Vec<_>>(),
        [1, 0]
    );
}
#[test]
fn unmatched_seam_is_explicit_and_stacks() {
    let a = ScrollFrame::from_rgba(RgbaImage::from_pixel(80, 160, Rgba([255, 0, 0, 255]))).unwrap();
    let b = ScrollFrame::from_rgba(RgbaImage::from_pixel(80, 160, Rgba([0, 0, 255, 255]))).unwrap();
    let result = stitch(&[a, b], &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.document.height(), 320);
    assert_eq!(result.notes, [ScrollCaptureNote::UnmatchedSeam(2)]);
}
#[test]
fn sticky_footers_recover_hidden_content_and_survive_once() {
    let source = page(120, 1000);
    for bar in [40, 44, 120] {
        let step = if bar == 120 { 80 } else { 150 };
        let frames = [0, step, step * 2].map(|y| with_footer(frame(&source, y, 300), bar));
        let result = stitch(&frames, &StitchConfig::default(), &Cancellation::default()).unwrap();
        assert_eq!(
            result.document.height(),
            300 + step * 2,
            "bar {bar}: {:?}",
            result.placements
        );
        let image = full_pixels(&result);
        for y in [
            300 - bar - 2,
            300 - bar + 2,
            300 - bar + step - 2,
            300 - bar + step + 2,
        ] {
            assert_eq!(
                image.get_pixel(60, y),
                source.get_pixel(60, y),
                "bar {bar}, row {y}"
            );
        }
        assert_eq!(
            image.get_pixel(100, image.height() - 4),
            frames[2].0.get_pixel(100, 296)
        );
    }
}
#[test]
fn disappearing_footer_is_removed_from_earlier_frames() {
    let source = page(120, 1000);
    let frames = [
        with_footer(frame(&source, 0, 300), 40),
        with_footer(frame(&source, 150, 300), 40),
        frame(&source, 300, 300),
    ];
    let result = stitch(&frames, &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.document.height(), 600);
    for y in [258, 262, 408, 412, 500] {
        assert_eq!(
            full_pixels(&result).get_pixel(60, y),
            source.get_pixel(60, y)
        );
    }
}
#[test]
fn sticky_header_is_kept_only_once() {
    let source = page(120, 1000);
    let mut frames = Vec::new();
    for offset in [0, 150, 300] {
        let mut image = frame(&source, offset, 300).0.as_ref().clone();
        for y in 0..32 {
            for x in 0..120 {
                image.put_pixel(x, y, Rgba([50, 50, 50, 255]));
            }
        }
        frames.push(ScrollFrame::from_rgba(image).unwrap());
    }
    let result = stitch(&frames, &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.document.height(), 600);
    assert!(result.placements[1].cropped_top >= 32);
    assert_eq!(
        full_pixels(&result).get_pixel(60, 300),
        source.get_pixel(60, 300)
    );
}
#[test]
fn blank_seam_prefers_largest_exact_overlap() {
    let mut source = page(120, 500);
    for y in 200..300 {
        for x in 0..120 {
            source.put_pixel(x, y, Rgba([255, 255, 255, 255]));
        }
    }
    let frames = [frame(&source, 0, 300), frame(&source, 200, 300)];
    let result = stitch(&frames, &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.placements[1].overlap, 100);
    assert_eq!(result.document.height(), 500);
}
#[test]
fn manual_session_settles_and_preview_matches_output() {
    let source = page(160, 1200);
    let mut session =
        ScrollSession::new(Some(frame(&source, 0, 300)), ScrollConfig::default()).unwrap();
    assert!(session.start());
    for offset in [90, 180] {
        feed(&mut session, frame(&source, offset, 300));
        feed(&mut session, frame(&source, offset, 300));
    }
    assert_eq!(session.frame_count(), 3);
    let result = finish(&mut session, frame(&source, 180, 300));
    assert_eq!(result.document.height(), 480);
    assert_eq!(session.preview_rows(), 480);
    assert_eq!(session.phase(), ScrollPhase::Finished);
}
#[test]
fn unchanged_samples_do_not_accumulate() {
    let source = page(160, 300);
    let frame = frame(&source, 0, 300);
    let mut session = ScrollSession::new(Some(frame.clone()), ScrollConfig::default()).unwrap();
    session.start();
    for _ in 0..5 {
        feed(&mut session, frame.clone());
    }
    assert_eq!(session.frame_count(), 1);
}
#[test]
fn fast_overlap_is_appended_without_settling() {
    let source = page(160, 900);
    let mut session =
        ScrollSession::new(Some(frame(&source, 0, 300)), ScrollConfig::default()).unwrap();
    session.start();
    feed(&mut session, frame(&source, 180, 300));
    assert_eq!(session.frame_count(), 2);
}
#[test]
fn reverse_movement_is_ignored_and_reported() {
    let source = page(160, 900);
    let mut session =
        ScrollSession::new(Some(frame(&source, 200, 300)), ScrollConfig::default()).unwrap();
    session.start();
    feed(&mut session, frame(&source, 100, 300));
    assert_eq!(session.frame_count(), 1);
    assert_eq!(session.message(), Some("Scroll down to capture more."));
}
#[test]
fn no_initial_frame_becomes_a_first_sample() {
    let mut session = ScrollSession::new(None, ScrollConfig::default()).unwrap();
    assert!(!session.can_finish());
    session.start();
    feed(&mut session, frame(&page(160, 300), 0, 300));
    assert_eq!(session.frame_count(), 1);
    assert!(session.can_finish());
}
#[test]
fn finish_takes_two_final_samples_and_recovers_last_scroll() {
    let source = page(160, 900);
    let mut session =
        ScrollSession::new(Some(frame(&source, 0, 300)), ScrollConfig::default()).unwrap();
    session.start();
    session.begin_finish().unwrap();
    assert_eq!(session.final_samples_remaining(), 2);
    feed(&mut session, frame(&source, 90, 300));
    assert_eq!(session.final_samples_remaining(), 1);
    assert!(!session.ready_to_stitch());
    feed(&mut session, frame(&source, 90, 300));
    let job = session.finish_snapshot().unwrap();
    let result = session.accept_finish(finish_scroll(job)).unwrap().unwrap();
    assert_eq!(result.document.height(), 390);
}
#[test]
fn failed_final_sample_keeps_frames_and_prioritizes_note() {
    let source = page(160, 1600);
    let mut session =
        ScrollSession::new(Some(frame(&source, 0, 300)), ScrollConfig::default()).unwrap();
    session.start();
    feed(&mut session, frame(&source, 800, 300));
    feed(&mut session, frame(&source, 800, 300));
    session.begin_finish().unwrap();
    session.sample_failed();
    let job = session.finish_snapshot().unwrap();
    let result = session.accept_finish(finish_scroll(job)).unwrap().unwrap();
    assert_eq!(result.frame_count, 2);
    assert_eq!(
        result.notes.first(),
        Some(&ScrollCaptureNote::FinalSampleFailed)
    );
}
#[test]
fn size_change_stops_watching_but_finish_preserves_frames() {
    let initial = frame(&page(160, 300), 0, 300);
    let wrong = frame(&page(80, 100), 0, 100);
    let mut session = ScrollSession::new(Some(initial), ScrollConfig::default()).unwrap();
    session.start();
    feed(&mut session, wrong.clone());
    assert_eq!(session.phase(), ScrollPhase::Idle);
    session.begin_finish().unwrap();
    feed(&mut session, wrong);
    let job = session.finish_snapshot().unwrap();
    let result = session.accept_finish(finish_scroll(job)).unwrap().unwrap();
    assert_eq!(result.document.dimensions(), (160, 300));
    assert_eq!(result.notes, [ScrollCaptureNote::FinalSampleFailed]);
}
#[test]
fn stale_analysis_cannot_mutate_restarted_or_other_session() {
    let initial = frame(&page(160, 900), 0, 300);
    let mut session = ScrollSession::new(Some(initial.clone()), ScrollConfig::default()).unwrap();
    session.start();
    let job = session.snapshot_for_analysis(initial.clone()).unwrap();
    session.stop();
    session.start();
    assert!(!session.accept_analysis(analyze_sample(job)).unwrap());
    let job = session.snapshot_for_analysis(initial.clone()).unwrap();
    let mut other = ScrollSession::new(Some(initial), ScrollConfig::default()).unwrap();
    other.start();
    assert!(!other.accept_analysis(analyze_sample(job)).unwrap());
}
#[test]
fn stale_finish_never_publishes_into_restarted_session() {
    let initial = frame(&page(160, 300), 0, 300);
    let mut session = ScrollSession::new(Some(initial), ScrollConfig::default()).unwrap();
    session.start();
    session.begin_finish().unwrap();
    session.sample_failed();
    let job = session.finish_snapshot().unwrap();
    let finished = finish_scroll(job);
    session.stop();
    session.start();
    assert!(session.accept_finish(finished).unwrap().is_none());
    assert_eq!(session.phase(), ScrollPhase::Watching);
}
#[test]
fn repeated_start_finish_and_inflight_samples_are_guarded() {
    let initial = frame(&page(160, 300), 0, 300);
    let mut session = ScrollSession::new(Some(initial.clone()), ScrollConfig::default()).unwrap();
    assert!(session.start());
    assert!(session.start());
    let _pending = session.snapshot_for_analysis(initial.clone()).unwrap();
    assert!(matches!(
        session.snapshot_for_analysis(initial),
        Err(ScrollError::InvalidState)
    ));
    session.begin_finish().unwrap();
    assert!(!session.start());
    assert_eq!(session.begin_finish(), Err(ScrollError::InvalidState));
}
#[test]
fn frame_limit_stops_sampling_but_allows_finish() {
    let source = page(160, 900);
    let mut session = ScrollSession::new(
        Some(frame(&source, 0, 300)),
        ScrollConfig {
            max_frames: 2,
            ..Default::default()
        },
    )
    .unwrap();
    session.start();
    feed(&mut session, frame(&source, 180, 300));
    assert!(!session.can_sample());
    assert!(session.can_finish());
    session.begin_finish().unwrap();
    assert!(session.ready_to_stitch());
}
#[test]
fn height_limit_drops_trailing_frames_with_warning() {
    let source = page(160, 900);
    let config = ScrollConfig {
        stitch: StitchConfig {
            max_output_height: 410,
            max_overlap_fraction: 0.985,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut session = ScrollSession::new(Some(frame(&source, 0, 300)), config).unwrap();
    session.start();
    for offset in [90, 180] {
        feed(&mut session, frame(&source, offset, 300));
        feed(&mut session, frame(&source, offset, 300));
    }
    let result = finish(&mut session, frame(&source, 180, 300));
    assert_eq!(result.document.height(), 390);
    assert_eq!(result.frame_count, 2);
    assert_eq!(
        result.notes.first(),
        Some(&ScrollCaptureNote::DroppedFrames(1))
    );
}
#[test]
fn stitch_failure_can_resume_watching() {
    let initial = frame(&page(160, 300), 0, 300);
    let config = ScrollConfig {
        stitch: StitchConfig {
            max_output_height: 100,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut session = ScrollSession::new(Some(initial), config).unwrap();
    session.start();
    session.begin_finish().unwrap();
    session.sample_failed();
    let job = session.finish_snapshot().unwrap();
    assert!(session.accept_finish(finish_scroll(job)).is_err());
    assert_eq!(session.phase(), ScrollPhase::Failed);
    assert!(session.resume_watching());
    assert_eq!(session.phase(), ScrollPhase::Watching);
}
#[test]
fn cancellation_and_drop_signal_worker_jobs() {
    let initial = frame(&page(160, 300), 0, 300);
    let mut session = ScrollSession::new(Some(initial.clone()), ScrollConfig::default()).unwrap();
    session.start();
    let job = session.snapshot_for_analysis(initial.clone()).unwrap();
    let cancel = job.cancellation();
    drop(session);
    assert!(cancel.is_cancelled());
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert!(matches!(
        stitch(&[initial], &StitchConfig::default(), &cancelled),
        Err(ScrollError::Cancelled)
    ));
}
#[test]
fn invalid_options_memory_and_dimension_limits_fail_before_work() {
    let initial = frame(&page(160, 300), 0, 300);
    let tiny = ScrollConfig {
        stitch: StitchConfig {
            max_working_bytes: 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        ScrollSession::new(Some(initial.clone()), tiny),
        Err(ScrollError::MemoryLimit)
    ));
    assert!(
        ScrollSession::new(
            None,
            ScrollConfig {
                max_frames: 61,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        ScrollSession::new(
            None,
            ScrollConfig {
                change_threshold: f64::NAN,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        check_budget([u64::MAX, 1], MAX_WORKING_BYTES),
        Err(ScrollError::MemoryLimit)
    );
    assert!(ScrollFrame::from_rgba(RgbaImage::new(0, 1)).is_err());
    assert!(ScrollFrame::from_png(b"not a PNG").is_err());
    let wrong = frame(&page(80, 300), 0, 300);
    assert!(matches!(
        stitch(
            &[initial, wrong],
            &StitchConfig::default(),
            &Cancellation::default()
        ),
        Err(ScrollError::AreaChanged)
    ));
}
#[test]
fn synthetic_fixture_png_is_roundtrippable_and_contains_no_external_input() {
    let frame = synthetic_page_frame(90, 160, 300, 1200).unwrap();
    let bytes = frame.png().unwrap();
    assert_eq!(
        ScrollFrame::from_png(&bytes).unwrap().dimensions(),
        (160, 300)
    );
    assert!(synthetic_page_frame(1000, 160, 300, 1200).is_err());
    let debug = format!("{frame:?}");
    assert!(debug.len() < 100);
    assert!(!debug.contains("pixels"));
}

#[test]
fn finished_capture_enters_existing_crop_mask_export_pipeline() {
    use crate::screenshot::{AnnotationKind, AnnotationStyle, Rect, ScreenshotEditSession};
    let source = page(160, 900);
    let mut capture =
        ScrollSession::new(Some(frame(&source, 0, 300)), ScrollConfig::default()).unwrap();
    capture.start();
    feed(&mut capture, frame(&source, 180, 300));
    let result = finish(&mut capture, frame(&source, 180, 300));
    let mut editor = ScreenshotEditSession::new(result.document);
    editor
        .add_annotation(
            AnnotationKind::Blur(Rect::new(20., 30., 40., 40.)),
            AnnotationStyle::redaction(),
        )
        .unwrap();
    editor
        .set_crop(Some(Rect::new(10., 20., 100., 300.)))
        .unwrap();
    let png = editor.render_png().unwrap();
    let exported = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8();
    assert_eq!(exported.dimensions(), (100, 300));
    assert_eq!(exported.get_pixel(90, 290), source.get_pixel(100, 310));
    assert_ne!(exported.get_pixel(20, 20), source.get_pixel(30, 40));
    assert_eq!(exported.get_pixel(20, 20)[3], 255);
    assert!(editor.document().render_preview_tiles().is_ok());
}

#[test]
fn stale_completion_after_finish_failure_cannot_take_new_session_state() {
    let source = page(160, 300);
    let initial = frame(&source, 0, 300);
    let mut session = ScrollSession::new(Some(initial.clone()), ScrollConfig::default()).unwrap();
    session.start();
    let obsolete = session.snapshot_for_analysis(initial.clone()).unwrap();
    session.begin_finish().unwrap();
    feed(&mut session, initial.clone());
    feed(&mut session, initial);
    let completed = finish_scroll(session.finish_snapshot().unwrap());
    assert!(!session.accept_analysis(analyze_sample(obsolete)).unwrap());
    assert!(session.accept_finish(completed).unwrap().is_some());
}

#[test]
fn preview_png_is_bounded_and_never_used_as_export_image() {
    let source = page(160, 900);
    let mut capture =
        ScrollSession::new(Some(frame(&source, 0, 300)), ScrollConfig::default()).unwrap();
    capture.start();
    feed(&mut capture, frame(&source, 180, 300));
    let preview = capture.preview_snapshot();
    let png = preview.png(&Cancellation::default()).unwrap();
    let small = ScrollFrame::from_png(&png).unwrap();
    assert_eq!(small.width(), 120);
    assert!(small.height() <= 2048);
    let result = finish(&mut capture, frame(&source, 180, 300));
    assert_eq!(result.document.dimensions(), (160, 480));
}

#[test]
fn repeated_middle_content_is_preserved_instead_of_mistaken_for_a_bar() {
    let a = ScrollFrame::from_rgba(RgbaImage::from_fn(100, 200, |_, y| {
        Rgba(if (80..120).contains(&y) {
            [120, 120, 120, 255]
        } else if y < 80 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        })
    }))
    .unwrap();
    let b = ScrollFrame::from_rgba(RgbaImage::from_fn(100, 200, |_, y| {
        Rgba(if (80..120).contains(&y) {
            [120, 120, 120, 255]
        } else if y < 80 {
            [0, 255, 0, 255]
        } else {
            [255, 0, 255, 255]
        })
    }))
    .unwrap();
    let result = stitch(&[a, b], &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.document.height(), 400);
    assert_eq!(result.placements[1].cropped_top, 0);
}

#[test]
fn constantly_changing_unmatched_content_is_never_appended() {
    let initial =
        ScrollFrame::from_rgba(RgbaImage::from_pixel(160, 300, Rgba([0, 0, 0, 255]))).unwrap();
    let mut session = ScrollSession::new(Some(initial), ScrollConfig::default()).unwrap();
    session.start();
    for value in [60, 120, 180, 240] {
        feed(
            &mut session,
            ScrollFrame::from_rgba(RgbaImage::from_pixel(
                160,
                300,
                Rgba([value, value, value, 255]),
            ))
            .unwrap(),
        );
    }
    assert_eq!(session.frame_count(), 1);
}

#[test]
fn full_resolution_refinement_recovers_downsampled_off_grid_shift() {
    let source = page(840, 1000);
    let frames = [frame(&source, 0, 420), frame(&source, 173, 420)];
    let result = stitch(&frames, &StitchConfig::default(), &Cancellation::default()).unwrap();
    assert_eq!(result.document.height(), 593);
    assert_eq!(
        full_pixels(&result).get_pixel(700, 419),
        source.get_pixel(700, 419)
    );
}

#[test]
fn cancelling_active_finish_restores_idle_and_allows_restart() {
    let initial = frame(&page(160, 300), 0, 300);
    let mut session = ScrollSession::new(Some(initial), ScrollConfig::default()).unwrap();
    session.start();
    session.begin_finish().unwrap();
    session.sample_failed();
    let job = session.finish_snapshot().unwrap();
    job.cancellation().cancel();
    assert!(matches!(
        session.accept_finish(finish_scroll(job)),
        Err(ScrollError::Cancelled)
    ));
    assert_eq!(session.phase(), ScrollPhase::Idle);
    assert!(session.start());
    assert!(session.can_sample());
}

#[test]
fn cancelling_after_computation_still_prevents_editor_publication() {
    let initial = frame(&page(160, 300), 0, 300);
    let mut session = ScrollSession::new(Some(initial), ScrollConfig::default()).unwrap();
    session.start();
    session.begin_finish().unwrap();
    session.sample_failed();
    let job = session.finish_snapshot().unwrap();
    let cancellation = job.cancellation();
    let completed = finish_scroll(job);
    cancellation.cancel();
    assert!(matches!(
        session.accept_finish(completed),
        Err(ScrollError::Cancelled)
    ));
    assert_eq!(session.phase(), ScrollPhase::Idle);
}

#[test]
fn actual_hud_fixture_sequential_down_steps_preserve_every_pixel() {
    let config = ScrollConfig::default();
    let mut session = ScrollSession::new(
        Some(synthetic_page_frame(0, 420, 320, 2200).unwrap()),
        config.clone(),
    )
    .unwrap();
    session.start();
    let mut previous = synthetic_page_frame(0, 420, 320, 2200).unwrap();
    for offset in (90..=900).step_by(90) {
        let current = synthetic_page_frame(offset, 420, 320, 2200).unwrap();
        let pair = stitch(
            &[previous, current.clone()],
            &config.stitch,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(pair.placements[1].overlap, 230, "fixture offset {offset}");
        assert_eq!(pair.document.height(), 410, "fixture offset {offset}");
        let expected = synthetic_page_frame(offset - 90, 420, 410, 2200).unwrap();
        assert_eq!(full_pixels(&pair), *expected.0, "fixture offset {offset}");
        assert!(
            pair.notes.is_empty(),
            "fixture offset {offset}: {:?}",
            pair.notes
        );
        feed(&mut session, current.clone());
        feed(&mut session, current.clone());
        assert_eq!(
            session.preview_rows(),
            u64::from(offset + 320),
            "fixture offset {offset}"
        );
        previous = current;
    }
    let result = finish(&mut session, previous);
    assert_eq!(result.document.dimensions(), (420, 1220));
    assert_eq!(
        full_pixels(&result),
        *synthetic_page_frame(0, 420, 1220, 2200).unwrap().0
    );
    assert!(result.notes.is_empty());
}

#[test]
fn actual_hud_fixture_wheel_offsets_preserve_every_pixel() {
    let config = ScrollConfig::default();
    let mut session = ScrollSession::new(
        Some(synthetic_page_frame(0, 420, 320, 2200).unwrap()),
        config.clone(),
    )
    .unwrap();
    session.start();
    let mut prior_offset = 0;
    let mut previous = synthetic_page_frame(0, 420, 320, 2200).unwrap();
    for offset in [17, 53, 119, 207, 293, 381, 461, 501, 674, 801] {
        let current = synthetic_page_frame(offset, 420, 320, 2200).unwrap();
        let pair = stitch(
            &[previous, current.clone()],
            &config.stitch,
            &Cancellation::default(),
        )
        .unwrap();
        let shift = offset - prior_offset;
        assert_eq!(
            pair.placements[1].overlap,
            320 - shift,
            "wheel {prior_offset} -> {offset}"
        );
        assert_eq!(
            pair.document.height(),
            320 + shift,
            "wheel {prior_offset} -> {offset}"
        );
        assert_eq!(
            full_pixels(&pair),
            *synthetic_page_frame(prior_offset, 420, 320 + shift, 2200)
                .unwrap()
                .0,
            "wheel {prior_offset} -> {offset}"
        );
        assert!(
            !pair
                .notes
                .iter()
                .any(|n| matches!(n, ScrollCaptureNote::UnmatchedSeam(_)))
        );
        feed(&mut session, current.clone());
        feed(&mut session, current.clone());
        assert_eq!(session.preview_rows(), u64::from(offset + 320));
        previous = current;
        prior_offset = offset;
    }
    let result = finish(&mut session, previous);
    assert_eq!(result.document.dimensions(), (420, 1121));
    assert_eq!(
        full_pixels(&result),
        *synthetic_page_frame(0, 420, 1121, 2200).unwrap().0
    );
    assert!(
        !result
            .notes
            .iter()
            .any(|n| matches!(n, ScrollCaptureNote::UnmatchedSeam(_)))
    );
}

#[test]
fn final_sample_deadline_is_relative_to_first_completion_not_timer_phase() {
    use std::time::Duration;
    let mut clock = FinalSampleSchedule::default();
    assert!(clock.ready(Duration::ZERO));
    // Sample one completes just before the free-running 140 ms timer fires.
    clock
        .first_sample_accepted(Duration::from_millis(139), 140)
        .unwrap();
    assert!(!clock.ready(Duration::from_millis(140)));
    assert!(!clock.ready(Duration::from_millis(278)));
    assert!(clock.ready(Duration::from_millis(279)));
    // Short configured intervals retain Swift's 50 ms minimum.
    clock.reset();
    clock
        .first_sample_accepted(Duration::from_millis(500), 1)
        .unwrap();
    assert!(!clock.ready(Duration::from_millis(549)));
    assert!(clock.ready(Duration::from_millis(550)));
    clock.reset();
    assert!(clock.ready(Duration::ZERO));
}

#[test]
fn controlled_finish_waits_before_second_frame_and_retains_last_scroll() {
    use std::time::Duration;
    let mut session = ScrollSession::new(
        Some(synthetic_page_frame(0, 420, 320, 2200).unwrap()),
        ScrollConfig::default(),
    )
    .unwrap();
    let mut clock = FinalSampleSchedule::default();
    session.start();
    session.begin_finish().unwrap();
    feed(
        &mut session,
        synthetic_page_frame(90, 420, 320, 2200).unwrap(),
    );
    clock
        .first_sample_accepted(
            Duration::from_millis(139),
            session.config().sample_interval_ms,
        )
        .unwrap();
    assert_eq!(session.final_samples_remaining(), 1);
    assert!(!clock.ready(Duration::from_millis(140)));
    assert!(!session.ready_to_stitch());
    assert!(clock.ready(Duration::from_millis(279)));
    feed(
        &mut session,
        synthetic_page_frame(90, 420, 320, 2200).unwrap(),
    );
    let job = session.finish_snapshot().unwrap();
    let result = session.accept_finish(finish_scroll(job)).unwrap().unwrap();
    assert_eq!(result.document.height(), 410);
    assert_eq!(
        full_pixels(&result),
        *synthetic_page_frame(0, 420, 410, 2200).unwrap().0
    );
}
