use super::*;
use super::{
    control::{Admission, Callbacks},
    output::OutputTransaction,
    validation::{Frame, Spec, Timeline},
};
use std::{
    fs,
    io::Write,
    sync::{atomic::Ordering, mpsc, Mutex},
    time::{Duration, Instant},
};
static SERIAL: Mutex<()> = Mutex::new(());
fn movie(fixture: bool, recovery: bool) -> FinalizedRecording {
    let output = OutputTransaction::new(fixture).unwrap();
    let mut file = output::new_file(&output.capture).unwrap();
    file.write_all(include_bytes!("assets/known-rgb.mov"))
        .unwrap();
    drop(file);
    if recovery {
        fs::write(&output.destination, b"keep previous").unwrap();
    }
    output
        .finish(
            crate::movie::MovieInfo {
                duration: 1.2,
                display_width: 96.,
                display_height: 64.,
                nominal_frame_rate: 10.,
            },
            false,
            true,
            &RecordingControl::default(),
        )
        .unwrap()
}
fn wait_event(handle: &RecordingHandle) -> RecordingEvent {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(event) = handle.try_event() {
            return event;
        }
        assert!(Instant::now() < deadline, "recording event timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn drained(control: &RecordingControl) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !control.is_drained() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn ordinary_admission_remains_closed_in_fixture_builds() {
    assert_eq!(
        RecordingHandle::start().unwrap_err(),
        RecordingError::Unavailable
    );
    const { assert!(!NATIVE_RECORDING_IMPLEMENTED) };
}
#[test]
fn validator_rejects_storage_alpha_dimensions_and_time_without_native_work() {
    let spec = Spec {
        width: 96,
        height: 64,
        fps: 10,
    };
    let mut frame = Frame {
        rgba: vec![255; 96 * 64 * 4],
        stride: 96 * 4,
        pts: 0,
        duration: 60,
    };
    let mut timeline = Timeline::default();
    assert!(timeline.validate(spec, &frame).is_ok());
    frame.stride = usize::MAX;
    assert_eq!(
        timeline.validate(spec, &frame),
        Err(RecordingError::LimitExceeded)
    );
    frame.stride = 96 * 4;
    frame.rgba[3] = 0;
    assert_eq!(
        timeline.validate(spec, &frame),
        Err(RecordingError::InvalidFrame)
    );
    frame.rgba[3] = 255;
    for pts in [-1, 1, i64::MAX] {
        frame.pts = pts;
        assert_eq!(
            timeline.validate(spec, &frame),
            Err(RecordingError::InvalidTimestamp)
        );
    }
    frame.pts = 0;
    timeline.appended(&frame);
    assert_eq!(
        timeline.validate(spec, &frame),
        Err(RecordingError::InvalidTimestamp)
    );
    frame.pts = 60;
    assert!(timeline.validate(spec, &frame).is_ok());
    frame.duration = 0;
    assert_eq!(
        timeline.validate(spec, &frame),
        Err(RecordingError::InvalidTimestamp)
    );
    assert_eq!(
        Spec { width: 95, ..spec }.validate(),
        Err(RecordingError::InvalidFrame)
    );
    assert_eq!(
        Spec {
            width: 3840,
            height: 3840,
            ..spec
        }
        .validate(),
        Err(RecordingError::LimitExceeded)
    );
    assert!(super::validation::validate_native_storage(spec, 383, 64 * 384).is_err());
    assert!(super::validation::validate_native_storage(spec, 384, 64 * 384 - 1).is_err());
}
#[test]
fn frame_count_duration_and_backing_capacity_are_bounded() {
    let spec = Spec {
        width: 2,
        height: 2,
        fps: 30,
    };
    let mut timeline = Timeline::default();
    for index in 0..MAX_RECORDING_FRAMES {
        let frame = Frame {
            rgba: vec![255; 16],
            stride: 8,
            pts: index as i64 * 20,
            duration: 20,
        };
        timeline.validate(spec, &frame).unwrap();
        timeline.appended(&frame);
    }
    let frame = Frame {
        rgba: vec![255; 16],
        stride: 8,
        pts: 72000,
        duration: 20,
    };
    assert_eq!(
        timeline.validate(spec, &frame),
        Err(RecordingError::LimitExceeded)
    );
    let mut large_backing = Vec::with_capacity(super::validation::MAX_RGBA_BYTES + 1);
    large_backing.extend_from_slice(&[255; 16]);
    assert_eq!(
        Timeline::default().validate(
            spec,
            &Frame {
                rgba: large_backing,
                stride: 8,
                pts: 0,
                duration: 20
            }
        ),
        Err(RecordingError::LimitExceeded)
    );
}
#[test]
fn cancellation_cannot_release_admission_before_held_callback_destruction() {
    let _serial = SERIAL.lock().unwrap();
    let control = RecordingControl::default();
    let admission = Admission::acquire(control.clone()).unwrap();
    let callbacks = Callbacks::default();
    let held = callbacks.ticket();
    let (send, received) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        send.send(()).unwrap();
        callbacks.wait();
        drop(admission);
    });
    received.recv().unwrap();
    control.cancel();
    assert!(!control.is_drained());
    assert!(matches!(
        Admission::acquire(RecordingControl::default()),
        Err(RecordingError::Busy)
    ));
    drop(held);
    worker.join().unwrap();
    assert!(control.is_drained());
    drop(Admission::acquire(RecordingControl::default()).unwrap());
}
#[test]
fn cancelled_output_cleans_only_its_owned_stage() {
    let output = OutputTransaction::new(true).unwrap();
    let parent = output.capture.parent().unwrap().to_path_buf();
    fs::write(&output.capture, b"not yet published").unwrap();
    let control = RecordingControl::default();
    control.cancel();
    let info = crate::movie::MovieInfo {
        duration: 1.,
        display_width: 96.,
        display_height: 64.,
        nominal_frame_rate: 10.,
    };
    assert!(matches!(
        output.finish(info, false, false, &control),
        Err(RecordingError::Cancelled)
    ));
    assert!(!parent.exists());
}
#[test]
fn successful_publication_snapshots_identity_after_its_own_link_changes() {
    let movie = movie(true, false);
    assert_eq!(movie.publication(), RecordingPublication::Published);
    movie.verify().unwrap();
    assert!(!movie.path().parent().unwrap().join("capture.mov").exists());
    assert_eq!(movie.selected_movie(), movie.selected_movie());
    let source = movie.selected_movie();
    let path = movie.path().to_path_buf();
    drop(movie);
    assert!(path.exists(), "selection retains generated lifetime");
    source.verify().unwrap();
    drop(source);
    assert!(
        path.exists(),
        "completed fixture survives last adopted owner drop"
    );
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn failed_publication_preserves_capture_and_prior_destination() {
    let movie = movie(true, true);
    assert_eq!(movie.publication(), RecordingPublication::RecoveredCapture);
    assert!(movie.recovery_warning().is_some());
    movie.verify().unwrap();
    assert_eq!(
        fs::read(movie.path().parent().unwrap().join("recording.mov")).unwrap(),
        b"keep previous"
    );
}
#[test]
fn completed_ordinary_media_survives_last_review_owner_drop() {
    for recovery in [false, true] {
        let movie = movie(false, recovery);
        let path = movie.path().to_path_buf();
        let directory = path.parent().unwrap().to_path_buf();
        drop(movie);
        assert!(path.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
#[test]
fn save_copies_retained_source_and_never_overwrites_or_deletes_it() {
    let movie = movie(true, false);
    let parent = movie.path().parent().unwrap();
    let destination = parent.join("saved.mov");
    assert_eq!(
        movie
            .save_copy(&destination, &SaveControl::default())
            .unwrap(),
        destination
    );
    assert_eq!(
        fs::read(&destination).unwrap(),
        fs::read(movie.path()).unwrap()
    );
    assert_eq!(
        movie.save_copy(&destination, &SaveControl::default()),
        Err(RecordingError::AlreadyExists)
    );
    assert_eq!(
        movie.save_copy(movie.path(), &SaveControl::default()),
        Err(RecordingError::AlreadyExists)
    );
    let cancelled = SaveControl::default();
    cancelled.cancel();
    assert_eq!(
        movie.save_copy(&parent.join("cancelled.mov"), &cancelled),
        Err(RecordingError::Cancelled)
    );
    assert!(!parent.join("cancelled.mov").exists());
    assert_eq!(
        movie.save_copy(
            &parent.join("missing-dir/saved.mov"),
            &SaveControl::default()
        ),
        Err(RecordingError::Io)
    );
    movie.verify().unwrap();
    assert!(fs::read_dir(parent).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".BelloBox")
    }));
}
#[test]
fn replacement_rejects_save_and_identity_bound_known_frames() {
    let movie = movie(true, false);
    let known = movie.known_frames().unwrap();
    assert!(known.is_bound_to(&movie));
    assert!(!known.is_bound_to(&self::movie(true, false)));
    assert_eq!(known.frame(11).unwrap().rgba[..4], [255, 0, 255, 255]);
    let replacement = movie.path().with_extension("new");
    fs::copy(movie.path(), &replacement).unwrap();
    fs::rename(replacement, movie.path()).unwrap();
    assert_eq!(movie.verify(), Err(RecordingError::SourceChanged));
    assert!(matches!(known.frame(0), Err(RecordingError::SourceChanged)));
    assert_eq!(
        movie.save_copy(
            &movie.path().with_extension("copy.mov"),
            &SaveControl::default()
        ),
        Err(RecordingError::SourceChanged)
    );
}
#[test]
fn injected_start_stop_finalizes_without_granting_native_reader_admission() {
    let _serial = SERIAL.lock().unwrap();
    let handle = fixture::start_injected(fixture::Case::Standard).unwrap();
    assert!(matches!(wait_event(&handle), RecordingEvent::Started));
    assert!(matches!(
        fixture::start_injected(fixture::Case::Standard),
        Err(RecordingError::Busy)
    ));
    handle.stop();
    handle.stop();
    let RecordingEvent::Finalized(movie) = wait_event(&handle) else {
        panic!("expected completed movie")
    };
    movie.verify().unwrap();
    assert!(movie.known_frames().is_some());
    assert!(matches!(
        crate::movie::MovieAsset::open_selected(movie.selected_movie(), Default::default()),
        Err(crate::movie::MovieError::Unavailable)
    ));
    drained(&handle.control());
}
#[test]
fn dropped_receiver_cancels_worker_and_drain_is_observable_from_control() {
    let _serial = SERIAL.lock().unwrap();
    let handle = fixture::start_injected(fixture::Case::Standard).unwrap();
    let control = handle.control();
    assert!(matches!(wait_event(&handle), RecordingEvent::Started));
    drop(handle);
    drained(&control);
    drop(Admission::acquire(RecordingControl::default()).unwrap());
}
#[test]
fn stop_failure_and_recovery_have_distinct_terminal_outcomes() {
    let _serial = SERIAL.lock().unwrap();
    for case in [
        fixture::Case::FailFinalize,
        fixture::Case::RecoverPublication,
    ] {
        let handle = fixture::start_injected(case).unwrap();
        assert!(matches!(wait_event(&handle), RecordingEvent::Started));
        handle.stop();
        match (case, wait_event(&handle)) {
            (
                fixture::Case::FailFinalize,
                RecordingEvent::Failed(RecordingError::NativeFailure),
            ) => {}
            (fixture::Case::RecoverPublication, RecordingEvent::Finalized(movie)) => {
                assert!(movie.recovery_warning().is_some())
            }
            pair => panic!("wrong terminal outcome: {pair:?}"),
        }
        drained(&handle.control());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn native_generated_writer_finalizes_and_same_owned_token_decodes() {
    use crate::movie::{MovieAsset, MovieCancellation, MovieReadRange};
    let _serial = SERIAL.lock().unwrap();
    let handle = fixture::start_native(fixture::Case::Standard).unwrap();
    assert!(matches!(wait_event(&handle), RecordingEvent::Started));
    let ready_deadline = Instant::now() + Duration::from_secs(15);
    while !handle.control().fixture_frames_ready() {
        if let Some(event) = handle.try_event() {
            panic!("native producer ended before its complete fixture: {event:?}");
        }
        assert!(
            Instant::now() < ready_deadline,
            "finite native producer did not finish"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    handle.stop();
    let RecordingEvent::Finalized(movie) = wait_event(&handle) else {
        panic!("native writer did not finalize")
    };
    assert!(movie.known_frames().is_none());
    assert_eq!(movie.publication(), RecordingPublication::Published);
    movie.verify().unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(movie.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let asset =
        MovieAsset::open_selected(movie.selected_movie(), MovieCancellation::default()).unwrap();
    assert_eq!(asset.info().display_width, 96.);
    assert_eq!(asset.info().display_height, 64.);
    assert!((asset.info().duration - 1.2).abs() < 0.02);
    let mut reader = asset
        .reader(MovieReadRange::new(0., 1.2, 0.1).unwrap())
        .unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = reader.next_frame(|| false).unwrap() {
        frames.push(frame);
    }
    reader.finish(|| false).unwrap();
    assert_eq!(frames.len(), 12);
    for (index, frame) in frames.iter().enumerate() {
        assert!((frame.presentation_seconds() - index as f64 / 10.).abs() < 0.002);
    }
    let (_, _, _, last) = frames.pop().unwrap().into_parts();
    assert!(last
        .chunks_exact(4)
        .all(|pixel| pixel[0] > 220 && pixel[1] < 35 && pixel[2] > 220 && pixel[3] == 255));
    drained(&handle.control());
}

#[cfg(target_os = "macos")]
#[test]
fn checked_in_raw_movie_native_decode_matches_every_known_pixel_and_timestamp() {
    use crate::movie::{MovieAsset, MovieReadRange};
    let _serial = SERIAL.lock().unwrap();
    // Sealed test source: this exact compiled-in finite asset, never a path input.
    let output = OutputTransaction::new(true).unwrap();
    let mut file = output::new_file(&output.capture).unwrap();
    file.write_all(include_bytes!("assets/known-rgb.mov"))
        .unwrap();
    drop(file);
    let movie = output
        .finish(
            crate::movie::MovieInfo {
                duration: 1.2,
                display_width: 96.,
                display_height: 64.,
                nominal_frame_rate: 10.,
            },
            true,
            true,
            &RecordingControl::default(),
        )
        .unwrap();
    let known = movie.known_frames().unwrap();
    let asset = MovieAsset::open_selected(movie.selected_movie(), Default::default()).unwrap();
    assert!((asset.info().duration - 1.2).abs() < 0.001);
    let mut reader = asset
        .reader(MovieReadRange::new(0., 1.2, 0.1).unwrap())
        .unwrap();
    for index in 0..12 {
        let (pts, width, height, rgba) = reader.next_frame(|| false).unwrap().unwrap().into_parts();
        let expected = known.frame(index).unwrap();
        assert!((pts - expected.presentation_seconds).abs() < 0.001);
        assert_eq!((width, height), (expected.width, expected.height));
        assert_eq!(rgba, expected.rgba, "frame {index} raw RGB identity");
    }
    assert!(reader.next_frame(|| false).unwrap().is_none());
    reader.finish(|| false).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn native_cancel_does_not_publish_or_reopen_admission_before_drain() {
    let _serial = SERIAL.lock().unwrap();
    let handle = fixture::start_native(fixture::Case::Standard).unwrap();
    assert!(matches!(wait_event(&handle), RecordingEvent::Started));
    handle.cancel();
    assert!(matches!(wait_event(&handle), RecordingEvent::Cancelled));
    drained(&handle.control());
    drop(Admission::acquire(RecordingControl::default()).unwrap());
}

#[test]
fn competing_cancel_and_final_publication_have_one_linearized_outcome() {
    for _ in 0..24 {
        let output = OutputTransaction::new(true).unwrap();
        let directory = output.capture.parent().unwrap().to_path_buf();
        fs::write(&output.capture, include_bytes!("assets/known-rgb.mov")).unwrap();
        let control = RecordingControl::default();
        let cancel = control.clone();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let start = barrier.clone();
        let thread = std::thread::spawn(move || {
            start.wait();
            cancel.cancel();
        });
        barrier.wait();
        let result = output.finish(
            crate::movie::MovieInfo {
                duration: 1.2,
                display_width: 96.,
                display_height: 64.,
                nominal_frame_rate: 10.,
            },
            false,
            true,
            &control,
        );
        thread.join().unwrap();
        match result {
            Ok(movie) => {
                movie.verify().unwrap();
                control.cancel();
                movie.verify().unwrap();
                assert!(
                    movie.path().exists(),
                    "post-publication cancel cannot discard completed ownership"
                );
            }
            Err(RecordingError::Cancelled) => assert!(!directory.exists()),
            other => panic!("unexpected race result: {other:?}"),
        }
    }
}

#[test]
fn oversized_complete_file_is_rejected_without_reading_it_into_memory() {
    let output = OutputTransaction::new(true).unwrap();
    let directory = output.capture.parent().unwrap().to_path_buf();
    let file = output::new_file(&output.capture).unwrap();
    file.set_len(MAX_RECORDING_FILE_BYTES + 1).unwrap();
    drop(file);
    let result = output.finish(
        crate::movie::MovieInfo {
            duration: 1.2,
            display_width: 96.,
            display_height: 64.,
            nominal_frame_rate: 10.,
        },
        false,
        false,
        &RecordingControl::default(),
    );
    assert!(matches!(result, Err(RecordingError::LimitExceeded)));
    assert!(!directory.exists());
}

#[cfg(unix)]
#[test]
fn save_refuses_existing_symlink_without_touching_source_or_target() {
    use std::os::unix::fs::symlink;
    let movie = movie(true, false);
    let parent = movie.path().parent().unwrap();
    let alias = parent.join("alias.mov");
    symlink(movie.path(), &alias).unwrap();
    assert_eq!(
        movie.save_copy(&alias, &SaveControl::default()),
        Err(RecordingError::AlreadyExists)
    );
    movie.verify().unwrap();
    assert!(fs::symlink_metadata(&alias)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(target_os = "macos")]
#[test]
fn native_finish_callback_hold_keeps_lease_until_real_block_and_pool_retire() {
    let _serial = SERIAL.lock().unwrap();
    let control = RecordingControl::default();
    let admission = Admission::acquire(control.clone()).unwrap();
    let (gate, called) = super::macos::FinishGate::new();
    let held = gate.clone();
    let worker_control = control.clone();
    let (done, received) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let owner = admission;
        let output = OutputTransaction::new(true).unwrap();
        let result = super::macos::write(
            &owner,
            &output.capture,
            Spec {
                width: 96,
                height: 64,
                fps: 10,
            },
            &worker_control,
            |writer| {
                writer.hold_completion_for_test(held);
                writer.append(
                    Frame {
                        rgba: vec![255; 96 * 64 * 4],
                        stride: 96 * 4,
                        pts: 0,
                        duration: 60,
                    },
                    &worker_control,
                )
            },
        );
        let result = result.and_then(|info| output.finish(info, true, false, &worker_control));
        done.send(result).unwrap();
    });
    called.recv_timeout(Duration::from_secs(10)).unwrap();
    control.cancel();
    assert!(!control.is_drained());
    assert!(received.try_recv().is_err());
    assert!(matches!(
        Admission::acquire(RecordingControl::default()),
        Err(RecordingError::Busy)
    ));
    gate.release();
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)).unwrap(),
        Err(RecordingError::Cancelled)
    ));
    worker.join().unwrap();
    assert!(control.is_drained());
    drop(Admission::acquire(RecordingControl::default()).unwrap());
}

#[test]
fn cancellation_and_publication_claim_have_one_nonblocking_winner() {
    for cancel_first in [true, false] {
        let mut output = OutputTransaction::new(true).unwrap();
        fs::write(&output.capture, include_bytes!("assets/known-rgb.mov")).unwrap();
        let directory = output.capture.parent().unwrap().to_path_buf();
        let control = RecordingControl::default();
        let other = control.clone();
        let hook: Box<dyn FnOnce() + Send> = Box::new(move || {
            let expected = if cancel_first {
                CancelOutcome::Cancelled
            } else {
                CancelOutcome::PublicationClaimed
            };
            assert_eq!(other.cancel(), expected);
            assert_eq!(other.cancel(), expected);
            other.stop();
            other.stop();
        });
        if cancel_first {
            output.before_claim = Some(hook);
        } else {
            output.after_claim = Some(hook);
        }
        let result = output.finish(
            MovieInfo {
                duration: 1.2,
                display_width: 96.,
                display_height: 64.,
                nominal_frame_rate: 10.,
            },
            false,
            true,
            &control,
        );
        if cancel_first {
            assert!(matches!(result, Err(RecordingError::Cancelled)));
            assert!(!directory.exists());
        } else {
            let movie = result.unwrap();
            movie.verify().unwrap();
            let path = movie.path().to_path_buf();
            drop(movie);
            assert!(path.exists());
            fs::remove_dir_all(directory).unwrap();
        }
    }
}

#[test]
fn publication_failure_after_claim_preserves_recovery_and_never_means_success() {
    let mut output = OutputTransaction::new(true).unwrap();
    fs::write(&output.capture, include_bytes!("assets/known-rgb.mov")).unwrap();
    let destination = output.destination.clone();
    let control = RecordingControl::default();
    let other = control.clone();
    output.after_claim = Some(Box::new(move || {
        assert_eq!(other.cancel(), CancelOutcome::PublicationClaimed);
        fs::write(destination, b"prior destination").unwrap();
    }));
    let movie = output
        .finish(
            MovieInfo {
                duration: 1.2,
                display_width: 96.,
                display_height: 64.,
                nominal_frame_rate: 10.,
            },
            false,
            true,
            &control,
        )
        .unwrap();
    assert_eq!(movie.publication(), RecordingPublication::RecoveredCapture);
    assert!(movie.recovery_warning().is_some());
    movie.verify().unwrap();
    let path = movie.path().to_path_buf();
    drop(movie);
    assert!(path.exists());
    assert_eq!(
        fs::read(path.parent().unwrap().join("recording.mov")).unwrap(),
        b"prior destination"
    );
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn owner_drop_and_cancel_do_not_wait_for_event_mutex_and_retire_payload_on_worker() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let control = RecordingControl::default();
    let handle = RecordingHandle {
        control: control.clone(),
    };
    let admission = Admission::acquire(control.clone()).unwrap();
    let output = OutputTransaction::new(true).unwrap();
    fs::write(&output.capture, include_bytes!("assets/known-rgb.mov")).unwrap();
    let (dropped, drop_received) = mpsc::channel();
    output.observe_drop(dropped);
    let movie = output
        .finish(
            MovieInfo {
                duration: 1.2,
                display_width: 96.,
                display_height: 64.,
                nominal_frame_rate: 10.,
            },
            false,
            true,
            &control,
        )
        .unwrap();
    let path = movie.path().to_path_buf();
    control.started();
    assert!(control.terminal(RecordingEvent::Finalized(movie)));
    assert!(
        !control.terminal(RecordingEvent::Cancelled),
        "terminal exactly once"
    );
    let held = control.hold_events_for_test();
    assert!(
        handle.try_event().is_none(),
        "poll never blocks on slot lock"
    );
    let worker_control = control.clone();
    let worker = std::thread::spawn(move || {
        let owner = admission;
        worker_control.retire_terminal();
        drop(owner);
        std::thread::current().id()
    });
    let (closed, close_received) = mpsc::channel();
    let close_thread = std::thread::spawn(move || {
        assert_eq!(handle.cancel(), CancelOutcome::PublicationClaimed);
        handle.stop();
        drop(handle);
        closed.send(()).unwrap();
    });
    close_received
        .recv_timeout(Duration::from_secs(1))
        .expect("close must not wait for queue mutex");
    assert!(!control.is_drained());
    assert!(matches!(
        Admission::acquire(RecordingControl::default()),
        Err(RecordingError::Busy)
    ));
    drop(held);
    close_thread.join().unwrap();
    let worker_id = worker.join().unwrap();
    assert_eq!(
        drop_received.recv_timeout(Duration::from_secs(1)).unwrap(),
        worker_id
    );
    assert!(control.is_drained());
    assert!(path.exists());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn terminal_transfer_keeps_completed_movie_after_ui_owner_drop() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let control = RecordingControl::default();
    let handle = RecordingHandle {
        control: control.clone(),
    };
    let admission = Admission::acquire(control.clone()).unwrap();
    let movie = movie(true, false);
    let path = movie.path().to_path_buf();
    assert!(control.terminal(RecordingEvent::Finalized(movie)));
    let worker_control = control.clone();
    let worker = std::thread::spawn(move || {
        let owner = admission;
        worker_control.retire_terminal();
        drop(owner);
    });
    let event = handle.try_event().unwrap();
    assert!(matches!(event, RecordingEvent::Finalized(_)));
    assert!(handle.try_event().is_none());
    drop(event);
    drop(handle);
    worker.join().unwrap();
    assert!(
        path.exists(),
        "adopted payload drop does not remove completed fixture"
    );
    assert!(control.is_drained());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn save_cancel_claim_is_single_use_and_claim_does_not_imply_success() {
    let movie = movie(true, false);
    let directory = movie.path().parent().unwrap().to_path_buf();
    let control = SaveControl::default();
    assert_eq!(control.cancel(), CancelOutcome::Cancelled);
    assert_eq!(
        movie.save_copy(&directory.join("cancelled.mov"), &control),
        Err(RecordingError::Cancelled)
    );
    let control = SaveControl::default();
    assert_eq!(
        movie.save_copy(movie.path(), &control),
        Err(RecordingError::AlreadyExists)
    );
    assert_eq!(control.cancel(), CancelOutcome::PublicationClaimed);
    assert_eq!(
        movie.save_copy(&directory.join("second.mov"), &control),
        Err(RecordingError::Busy)
    );
    movie.verify().unwrap();
    drop(movie);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn app_retirement_owner_waits_for_callback_destruction_and_worker_admission_drop() {
    struct Probe(mpsc::Sender<std::thread::ThreadId>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let _serial = SERIAL.lock().unwrap();
    let control = RecordingControl::default();
    let admission = Admission::acquire(control.clone()).unwrap();
    let (sent, received) = mpsc::channel();
    control.retain_until_drained(Probe(sent));
    let callbacks = Callbacks::default();
    let held = callbacks.ticket();
    let worker = std::thread::spawn(move || {
        callbacks.wait();
        drop(admission);
    });
    control.close();
    assert!(!control.is_drained());
    assert!(received.try_recv().is_err());
    drop(held);
    let worker_id = worker.thread().id();
    worker.join().unwrap();
    assert!(control.is_drained());
    assert_eq!(received.recv().unwrap(), worker_id);
    assert_ne!(worker_id, std::thread::current().id());
}

#[test]
fn app_retirement_registration_race_with_physical_drain_never_leaks_an_owner() {
    struct Probe(Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }
    let _serial = SERIAL.lock().unwrap();
    for _ in 0..64 {
        let control = RecordingControl::default();
        let admission = Admission::acquire(control.clone()).unwrap();
        let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let worker = std::thread::spawn(move || drop(admission));
        control.retain_until_drained(Probe(dropped.clone()));
        worker.join().unwrap();
        assert!(control.is_drained());
        assert_eq!(dropped.load(Ordering::Acquire), 1);
    }
}
