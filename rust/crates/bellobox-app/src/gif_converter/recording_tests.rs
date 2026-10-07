use super::{Converter, Model, ReplacePolicy, Source, model, recording};
use crate::recording_ui::tests::fixtures::{self, SERIAL};
use bello_platform::recording::fixture::Case;
use bellobox_core::recording::gif::GifError;
use gpui::TestAppContext;
use gpui::{px, size};

fn model(movie: &bello_platform::recording::FinalizedRecording) -> Model {
    let source = Source::Recording(recording::OwnedRecording(movie.clone()));
    let mut model = Model::default();
    let (generation, cancel) = model.load(source.clone()).unwrap();
    model.loaded(generation, source.inspect(cancel));
    model.options.frames_per_second = 10;
    model
}
#[test]
fn owned_recording_bridge_retains_exact_identity_and_never_reopens_path() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let model = model(&movie.0);
    let model::SelectedSource::Recording(selected) = model.selected.clone().unwrap() else {
        panic!("lost owned source");
    };
    assert_eq!(selected.0.selected_movie(), movie.0.selected_movie());
    let frame = selected.seek(0.642, Default::default()).unwrap();
    assert_eq!(frame.0, 0.6);
    assert_eq!((frame.1, frame.2), (96, 64));
    assert_eq!(frame.3.len(), 96 * 64 * 4);
    assert!(
        bello_platform::movie::MovieAsset::open(movie.0.path(), Default::default()).is_err(),
        "injected proof never admits ordinary movie decoding"
    );
    let bytes = std::fs::read(movie.0.path()).unwrap();
    let old = movie.0.path().with_extension("retained");
    std::fs::rename(movie.0.path(), old).unwrap();
    std::fs::write(movie.0.path(), bytes).unwrap();
    assert!(
        selected.seek(0., Default::default()).is_err(),
        "equal-looking replacement is not the selected owner"
    );
    assert!(
        Source::Recording(selected)
            .inspect(Default::default())
            .is_err()
    );
}
#[test]
fn recording_gif_export_cancel_failure_retry_and_close_keep_movie_and_prior_gif() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let original = std::fs::read(movie.0.path()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut model = model(&movie.0);
    model.options.trim_start = 0.2;
    model.options.trim_end = Some(0.9);
    model.options.loops = false;
    let work = model
        .begin(
            directory.path().join("first.gif"),
            ReplacePolicy::RefuseExisting,
        )
        .unwrap();
    let generation = work.generation;
    let result = work.run().unwrap();
    assert_eq!(result.size, (96, 64));
    assert_eq!(result.frame_count, 7);
    model.finished(generation, Ok(result.clone()));
    let gif = std::fs::read(&result.path).unwrap();
    let mut preview = model::open_preview(&result).unwrap();
    let mut count = 0;
    while let Some(frame) = preview.next_frame().unwrap() {
        count += 1;
        assert_eq!(frame.delay_centiseconds, 10);
        let pixels = image::load_from_memory(&frame.png).unwrap().to_rgba8();
        assert!(
            !pixels.pixels().any(|p| p.0 == [255, 0, 255, 255]),
            "trim excludes final sentinel"
        );
    }
    assert_eq!(count, 7);
    for cancel in [true, false] {
        let work = model
            .begin(result.path.clone(), ReplacePolicy::RefuseExisting)
            .unwrap();
        let generation = work.generation;
        if cancel {
            model.cancel();
        }
        assert!(model.busy());
        let failure = work.run();
        assert!(failure.is_err());
        model.finished(generation, failure);
        assert_eq!(model.result.as_ref().unwrap().path, result.path);
        assert_eq!(std::fs::read(&result.path).unwrap(), gif);
        assert_eq!(std::fs::read(movie.0.path()).unwrap(), original);
    }
    let work = model
        .begin(
            directory.path().join("retry.gif"),
            ReplacePolicy::RefuseExisting,
        )
        .unwrap();
    let generation = work.generation;
    model.finished(generation, work.run());
    assert!(model.result.as_ref().unwrap().path.ends_with("retry.gif"));
    let work = model
        .begin(
            directory.path().join("closed.gif"),
            ReplacePolicy::RefuseExisting,
        )
        .unwrap();
    let generation = work.generation;
    model.close();
    assert!(matches!(work.run(), Err(GifError::Cancelled)));
    model.finished(generation, Err(GifError::Cancelled));
    assert!(!directory.path().join("closed.gif").exists());
    assert_eq!(std::fs::read(movie.0.path()).unwrap(), original);
    assert_eq!(std::fs::read(&result.path).unwrap(), gif);
    assert!(
        std::fs::read_dir(directory.path()).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with('.'))
    );
}
#[gpui::test]
fn owned_review_blocks_choose_another_saves_exact_movie_and_closes_safely(cx: &mut TestAppContext) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::RecoverPublication);
    let original = std::fs::read(movie.0.path()).unwrap();
    let selection = movie.0.selected_movie();
    let window = cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    cx.simulate_window_resize(window.into(), size(px(640.), px(440.)));
    cx.run_until_parked();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("saved.mov");
    window
        .update(cx, |review, _, cx| {
            assert_eq!(
                review.recording.as_ref().unwrap().0.selected_movie(),
                selection
            );
            assert!(
                review
                    .recording
                    .as_ref()
                    .unwrap()
                    .0
                    .recovery_warning()
                    .is_some()
            );
            assert!(review.source_preview.image.is_some());
            let source = review.model.source.clone();
            review.choose(cx);
            assert!(!review.model.dialog);
            review.load(Source::Movie("not-opened.mov".into()), cx);
            assert_eq!(review.model.source, source);
            review.save_movie_to(destination.clone(), cx);
            review.save_movie_to(directory.path().join("double.mov"), cx);
            assert!(review.model.busy());
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(std::fs::read(&destination).unwrap(), original);
    assert!(!directory.path().join("double.mov").exists());
    window
        .update(cx, |review, _, cx| {
            assert!(review.movie_save.is_none());
            assert!(!review.model.error);
            review.save_movie_to(destination.clone(), cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(review.model.error);
            assert!(!review.model.busy());
            review.save_movie_to(directory.path().join("retry.mov"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(directory.path().join("retry.mov")).unwrap(),
        original
    );
    window
        .update(cx, |review, window, cx| {
            review.save_movie_to(directory.path().join("closed.mov"), cx);
            review.close(cx);
            window.remove_window();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(!directory.path().join("closed.mov").exists());
    assert_eq!(std::fs::read(movie.0.path()).unwrap(), original);
    assert_eq!(std::fs::read(&destination).unwrap(), original);
}
#[gpui::test]
fn owned_recording_review_runs_existing_gif_worker_and_preserves_result_on_cancel(
    cx: &mut TestAppContext,
) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let window = cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    cx.run_until_parked();
    let directory = tempfile::tempdir().unwrap();
    window
        .update(cx, |review, _, cx| {
            review.model.options.frames_per_second = 10;
            review.model.options.trim_end = Some(0.3);
            review.shows_options = true;
            review.convert(directory.path().join("first.gif"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(!review.shows_options);
            assert!(review.shows_result);
            assert!(review.preview.image.is_some());
            review.show_movie(cx);
            review.show_gif(cx);
            review.convert(directory.path().join("cancelled.gif"), cx);
            review.model.cancel();
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(
                review
                    .model
                    .result
                    .as_ref()
                    .unwrap()
                    .path
                    .ends_with("first.gif")
            );
            assert!(review.preview.image.is_some());
            assert!(!review.model.busy());
            review.close(cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert!(!directory.path().join("cancelled.gif").exists());
    assert!(directory.path().join("first.gif").exists());
    assert!(movie.0.verify().is_ok());
}

#[cfg(target_os = "macos")]
#[test]
fn native_generated_writer_owned_result_enters_actual_reader_and_gif_export() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let handle = bello_platform::recording::fixture::start_native(Case::Standard).unwrap();
    // Stop during startup still accepts the writer-owned first frame; no
    // wall-clock sleep is used to guess when a finite producer has completed.
    let movie = fixtures::finalized(handle);
    assert!(
        movie.0.known_frames().is_none(),
        "native writer output must use its actual reader"
    );
    let mut model = model(&movie.0);
    let selected = model.selected.as_ref().unwrap();
    assert!(selected.seek(0., Default::default()).is_ok());
    let directory = tempfile::tempdir().unwrap();
    let work = model
        .begin(
            directory.path().join("native.gif"),
            ReplacePolicy::RefuseExisting,
        )
        .unwrap();
    let result = work.run().unwrap();
    assert_eq!(result.size, (96, 64));
    assert!(result.frame_count >= 1);
    assert!(
        model::open_preview(&result)
            .unwrap()
            .next_frame()
            .unwrap()
            .is_some()
    );
    assert!(movie.0.verify().is_ok());
    assert!(bello_platform::recording::RecordingHandle::start().is_err());
}

#[gpui::test]
fn movie_save_after_metadata_was_blocked_by_dialog_restores_paused_preview(
    cx: &mut TestAppContext,
) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let window = cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    window
        .update(cx, |review, _, _| review.model.dialog = true)
        .unwrap();
    cx.run_until_parked();
    let directory = tempfile::tempdir().unwrap();
    window
        .update(cx, |review, _, cx| {
            assert!(review.model.selected.is_some());
            assert!(review.source_preview.image.is_none());
            assert!(!review.source_preview.loading);
            review.model.dialog = false;
            review.save_movie_to(directory.path().join("saved.mov"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, _| {
            assert!(review.source_preview.image.is_some());
            assert!(!review.model.busy());
            assert!(!review.model.error);
        })
        .unwrap();
}

#[gpui::test]
fn direct_removal_retained_owned_review_cancels_save_and_export(cx: &mut TestAppContext) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    for saving in [true, false] {
        let movie = fixtures::movie(Case::Standard);
        let window =
            cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
        let retained = window.root(cx).unwrap();
        cx.run_until_parked();
        let directory = tempfile::tempdir().unwrap();
        window
            .update(cx, |review, window, cx| {
                if saving {
                    review.save_movie_to(directory.path().join("late.mov"), cx);
                } else {
                    review.convert(directory.path().join("late.gif"), cx);
                }
                window.remove_window();
            })
            .unwrap();
        cx.run_until_parked();
        retained.read_with(cx, |review, _| {
            assert!(review.closed);
            assert!(review.model.source.is_none());
            assert!(review.preview.image.is_none());
            assert!(review.source_preview.image.is_none());
            assert!(review.movie_save.is_none());
            assert!(review.model.result.is_none());
        });
        assert!(
            std::fs::read_dir(directory.path())
                .unwrap()
                .next()
                .is_none()
        );
        assert!(movie.0.verify().is_ok());
    }
}

#[gpui::test]
fn shutdown_retained_owned_review_cancels_save_and_gif_without_late_adoption(
    cx: &mut TestAppContext,
) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let save_window =
        cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    let gif_window =
        cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    let saved = save_window.root(cx).unwrap();
    let converted = gif_window.root(cx).unwrap();
    cx.run_until_parked();
    let directory = tempfile::tempdir().unwrap();
    save_window
        .update(cx, |review, _, cx| {
            review.save_movie_to(directory.path().join("late.mov"), cx)
        })
        .unwrap();
    gif_window
        .update(cx, |review, _, cx| {
            review.convert(directory.path().join("late.gif"), cx)
        })
        .unwrap();
    cx.quit();
    for retained in [&saved, &converted] {
        retained.read_with(cx, |review, _| {
            assert!(review.closed);
            assert!(review.model.source.is_none());
            assert!(review.preview.image.is_none());
            assert!(review.source_preview.image.is_none());
            assert!(review.movie_save.is_none());
        });
    }
    cx.run_until_parked();
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
    assert!(movie.0.verify().is_ok());
    converted.read_with(cx, |review, _| assert!(review.model.result.is_none()));
}

#[gpui::test]
fn closed_retained_owned_review_refuses_new_movie_save(cx: &mut TestAppContext) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let window = cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    let retained = window.root(cx).unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let directory = tempfile::tempdir().unwrap();
    retained.update(cx, |review, cx| {
        assert!(review.closed);
        assert!(!review.model.busy());
        review.save_movie_to(directory.path().join("after-close.mov"), cx);
        assert!(review.movie_save.is_none());
        assert!(!review.model.busy());
    });
    cx.run_until_parked();
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
    assert!(movie.0.verify().is_ok());
}

#[gpui::test]
fn current_owned_review_worker_panics_clear_save_export_and_preview_busy_states(
    cx: &mut TestAppContext,
) {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let movie = fixtures::movie(Case::Standard);
    let window = cx.add_window(|window, cx| Converter::new_recording(movie.0.clone(), window, cx));
    cx.run_until_parked();
    let directory = tempfile::tempdir().unwrap();
    window
        .update(cx, |review, _, cx| {
            crate::shutdown::panic_next(cx);
            review.save_movie_to(directory.path().join("panic.mov"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(review.movie_save.is_none());
            assert!(!review.model.dialog);
            assert!(review.model.error);
            crate::shutdown::panic_next(cx);
            review.convert(directory.path().join("panic.gif"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(review.model.job.is_none());
            assert!(!review.model.busy());
            assert!(review.model.error);
            crate::shutdown::panic_next(cx);
            review.seek_source(0.5, cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(!review.source_preview.loading);
            assert!(review.model.error);
            review.convert(directory.path().join("retry.gif"), cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, cx| {
            assert!(review.model.result.is_some());
            crate::shutdown::panic_next(cx);
            review.preview_result(cx);
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |review, _, _| {
            assert!(!review.preview.loading);
            assert!(review.model.error);
            assert!(review.model.status.contains("Preview worker stopped"));
        })
        .unwrap();
    assert!(movie.0.verify().is_ok());
    assert_eq!(cx.update(crate::shutdown::registry).active(), 0);
    assert!(!directory.path().join("panic.mov").exists());
    assert!(!directory.path().join("panic.gif").exists());
}
