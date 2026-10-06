use super::*;
use std::{
    collections::VecDeque,
    fs,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Barrier, mpsc},
    thread,
};
use tempfile::tempdir;

fn info(duration: f64, width: f64, height: f64) -> GifSourceInfo {
    GifSourceInfo {
        duration,
        display_width: width,
        display_height: height,
        nominal_frame_rate: 60.,
    }
}
fn plan(duration: f64, loops: bool) -> GifExportPlan {
    GifExportPlan::make(
        info(duration, 2., 2.),
        &GifExportOptions {
            frames_per_second: 10,
            loops,
            ..Default::default()
        },
    )
    .unwrap()
}
fn frame(time: f64, color: [u8; 4]) -> DisplayRgbaFrame {
    DisplayRgbaFrame::new(time, 2, 2, color.repeat(4)).unwrap()
}
struct Frames(VecDeque<DisplayRgbaFrame>);
impl SequentialFrameSource for Frames {
    fn next_frame(&mut self, _: &ExportControl) -> Result<Option<DisplayRgbaFrame>, GifError> {
        Ok(self.0.pop_front())
    }
}
fn frames() -> Frames {
    Frames([frame(0., [255, 0, 0, 255])].into())
}
fn export(
    source: &mut impl SequentialFrameSource,
    plan: &GifExportPlan,
    path: &Path,
    control: &ExportControl,
) -> Result<GifExportResult, GifError> {
    export_gif(
        source,
        plan,
        None,
        path,
        ReplacePolicy::RefuseExisting,
        control,
        |_, _| {},
    )
}
fn decode(path: &Path) -> Vec<(u16, Vec<u8>)> {
    let mut options = ::gif::DecodeOptions::new();
    options.set_color_output(::gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(fs::File::open(path).unwrap()).unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        frames.push((frame.delay, frame.buffer.to_vec()));
    }
    frames
}
fn no_stage(directory: &Path) {
    assert!(fs::read_dir(directory).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".BelloBox-export-")
    }));
}

#[test]
fn options_defaults_normalize_and_deserialize_missing_fields() {
    let swift_json =
        r#"{"framesPerSecond":20,"maxWidth":1080,"loops":false,"trimStart":1.5,"trimEnd":2.5}"#;
    let swift: GifExportOptions = serde_json::from_str(swift_json).unwrap();
    assert_eq!(
        (
            swift.frames_per_second,
            swift.max_width,
            swift.loops,
            swift.trim_start,
            swift.trim_end
        ),
        (20, 1080, false, 1.5, Some(2.5))
    );
    assert_eq!(
        serde_json::to_value(&swift).unwrap(),
        serde_json::from_str::<serde_json::Value>(swift_json).unwrap()
    );
    assert_eq!(
        serde_json::from_str::<GifExportOptions>("{}").unwrap(),
        GifExportOptions::default()
    );
    let normalized = GifExportOptions {
        frames_per_second: -100,
        max_width: 9000,
        trim_start: f64::NAN,
        trim_end: Some(f64::INFINITY),
        ..Default::default()
    }
    .normalized();
    assert_eq!(
        (
            normalized.frames_per_second,
            normalized.max_width,
            normalized.trim_start,
            normalized.trim_end
        ),
        (5, 1080, 0., None)
    );
    let normalized = GifExportOptions {
        frames_per_second: 50,
        max_width: 1,
        trim_start: 5.,
        trim_end: Some(4.),
        ..Default::default()
    }
    .normalized();
    assert_eq!(
        (
            normalized.frames_per_second,
            normalized.max_width,
            normalized.trim_end
        ),
        (20, 64, None)
    );
}
#[test]
fn planning_preserves_source_caps_portrait_and_no_upscale() {
    let a = GifExportPlan::make(info(400., 1080., 1920.), &GifExportOptions::default()).unwrap();
    assert_eq!(
        (a.duration(), a.frame_count(), a.output_size()),
        (120., 1800, (360, 640))
    );
    assert_eq!(
        GifExportPlan::make(info(120., 640., 640.), &GifExportOptions::default())
            .unwrap()
            .frame_count(),
        1800
    );
    let b = GifExportPlan::make(
        info(120., 1920., 1920.),
        &GifExportOptions {
            frames_per_second: 20,
            max_width: 1080,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((b.frame_count(), b.output_size()), (2400, (1080, 1080)));
    let c = GifExportPlan::make(info(1., 20., 10.), &GifExportOptions::default()).unwrap();
    assert_eq!(c.output_size(), (20, 10));
}
#[test]
fn planning_clamps_trim_and_rejects_bad_metadata() {
    let p = GifExportPlan::make(
        info(10., 20., 20.),
        &GifExportOptions {
            trim_start: 2.,
            trim_end: Some(3.01),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((p.start(), p.end(), p.frame_count()), (2., 3.01, 16));
    assert!(matches!(
        GifExportPlan::make(info(0.049, 20., 20.), &Default::default()),
        Err(GifError::ClipTooShort)
    ));
    for duration in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(GifExportPlan::make(info(duration, 20., 20.), &Default::default()).is_err());
    }
    for width in [0., -1., f64::NAN, f64::INFINITY, 1e99] {
        assert!(GifExportPlan::make(info(1., width, 20.), &Default::default()).is_err());
    }
}
#[test]
fn delays_use_cumulative_budget_and_borrow_for_partial_last_frame() {
    let p = GifExportPlan::make(info(1., 2., 2.), &Default::default()).unwrap();
    assert_eq!(&p.delays_centiseconds()[..6], &[7, 6, 7, 7, 6, 7]);
    assert_eq!(p.encoded_duration(), 1.);
    let p = GifExportPlan::make(info(1.001, 2., 2.), &Default::default()).unwrap();
    assert_eq!(p.frame_count(), 16);
    assert_eq!(&p.delays_centiseconds()[14..], &[5, 2]);
    assert_eq!(p.encoded_duration(), 1.);
}
#[test]
fn timing_invariants_cover_all_rates_and_partial_endings() {
    for fps in 5..=20 {
        for ms in 50..=2050 {
            let p = GifExportPlan::make(
                info(f64::from(ms) / 1000., 1., 1.),
                &GifExportOptions {
                    frames_per_second: fps,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(p.delays.iter().all(|d| *d >= 2));
            assert!((p.duration() - p.encoded_duration()).abs() <= 0.00500001);
            assert_eq!(
                p.delays.iter().map(|d| usize::from(*d)).sum::<usize>(),
                (p.duration() * 100.).round() as usize
            );
        }
    }
}
#[test]
fn frame_validation_and_debug_never_expose_pixels() {
    assert!(DisplayRgbaFrame::new(0., 0, 1, vec![]).is_err());
    assert!(DisplayRgbaFrame::new(0., 100000, 1, vec![]).is_err());
    assert!(DisplayRgbaFrame::new(0., 4096, 4096, vec![]).is_err());
    assert!(matches!(
        DisplayRgbaFrame::new(f64::NAN, 1, 1, vec![0; 4]),
        Err(GifError::InvalidTimestamp)
    ));
    assert!(matches!(
        DisplayRgbaFrame::new(0., 1, 1, vec![0; 3]),
        Err(GifError::InvalidFrame)
    ));
    let debug = format!("{:?}", frame(0., [17, 23, 45, 67]));
    assert!(!debug.contains("17, 23"));
}
#[cfg(unix)]
#[test]
fn sequential_sampling_tolerance_trim_end_and_hold_match_source() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("timing.gif");
    let p = GifExportPlan::make(
        info(0.4, 2., 2.),
        &GifExportOptions {
            frames_per_second: 10,
            trim_start: 0.1,
            trim_end: Some(0.31),
            ..Default::default()
        },
    )
    .unwrap();
    let mut source = Frames(
        [
            frame(0., [255, 0, 0, 255]),
            frame(0.09, [0, 255, 0, 255]),
            frame(0.16, [0, 0, 255, 255]),
            frame(0.32, [255, 255, 255, 255]),
        ]
        .into(),
    );
    export(&mut source, &p, &path, &Default::default()).unwrap();
    let decoded = decode(&path);
    assert_eq!(decoded.len(), 3);
    assert_eq!(&decoded[0].1[..4], &[0, 255, 0, 255]);
    assert_eq!(&decoded[1].1[..4], &[0, 0, 255, 255]);
    assert_eq!(&decoded[2].1[..4], &[0, 0, 255, 255]);
    assert_eq!(decoded.iter().map(|f| f.0).collect::<Vec<_>>(), p.delays);
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn fallback_first_frame_and_eof_hold_keep_planned_count() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hold.gif");
    let mut source = Frames([frame(0.08, [17, 23, 45, 255])].into());
    let p = plan(0.3, false);
    let result = export(&mut source, &p, &path, &Default::default()).unwrap();
    assert_eq!(
        (result.frame_count, result.duration, result.size),
        (3, 0.3, (2, 2))
    );
    assert_eq!(result.file_size, fs::metadata(&path).unwrap().len());
    assert!(
        decode(&path)
            .iter()
            .all(|(_, rgba)| rgba[..4] == [17, 23, 45, 255])
    );
}
#[cfg(unix)]
#[test]
fn once_omits_repeat_block_forever_emits_zero_and_validates() {
    let dir = tempdir().unwrap();
    for loops in [false, true] {
        let path = dir.path().join(format!("loop-{loops}.gif"));
        export(&mut frames(), &plan(0.3, loops), &path, &Default::default()).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            bytes.windows(11).filter(|w| *w == b"NETSCAPE2.0").count(),
            usize::from(loops)
        );
        assert!(
            gif_structure::validate(
                &mut fs::File::open(&path).unwrap(),
                &plan(0.3, !loops),
                &Default::default()
            )
            .is_err()
        );
    }
}
#[cfg(unix)]
#[test]
fn orientation_is_display_input_and_alpha_is_black_composited() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("orientation.gif");
    let pixels = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 128],
        [255, 255, 255, 0],
    ]
    .concat();
    let mut source = Frames([DisplayRgbaFrame::new(0., 2, 2, pixels).unwrap()].into());
    export(&mut source, &plan(0.1, false), &path, &Default::default()).unwrap();
    assert_eq!(
        decode(&path)[0].1,
        [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 128, 255],
            [0, 0, 0, 255]
        ]
        .concat()
    );
}
#[cfg(unix)]
#[test]
fn rejects_empty_after_cut_wrong_size_and_backward_frames() {
    let dir = tempdir().unwrap();
    for (name, mut source) in [
        ("empty", Frames(VecDeque::new())),
        ("after", Frames([frame(0.2, [0; 4])].into())),
        (
            "backward",
            Frames([frame(0.02, [0; 4]), frame(0.01, [0; 4])].into()),
        ),
        (
            "size",
            Frames([DisplayRgbaFrame::new(0., 1, 1, vec![0; 4]).unwrap()].into()),
        ),
    ] {
        let path = dir.path().join(format!("{name}.gif"));
        assert!(export(&mut source, &plan(0.1, false), &path, &Default::default()).is_err());
        assert!(!path.exists());
        no_stage(dir.path());
    }
}
#[cfg(unix)]
#[test]
fn source_failure_and_panics_preserve_prior_destination_and_cleanup() {
    struct Broken;
    impl SequentialFrameSource for Broken {
        fn next_frame(&mut self, _: &ExportControl) -> Result<Option<DisplayRgbaFrame>, GifError> {
            Err(GifError::Source("fixture failure".into()))
        }
    }
    let dir = tempdir().unwrap();
    let path = dir.path().join("existing.gif");
    fs::write(&path, b"prior").unwrap();
    assert!(
        export_gif(
            &mut Broken,
            &plan(0.3, false),
            None,
            &path,
            ReplacePolicy::ReplaceExistingFile,
            &Default::default(),
            |_, _| {}
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"prior");
    let control = ExportControl::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        export_gif(
            &mut frames(),
            &plan(0.3, false),
            None,
            &path,
            ReplacePolicy::ReplaceExistingFile,
            &control,
            |_, _| panic!("fixture progress panic"),
        )
    }));
    assert!(result.is_err());
    assert_eq!(control.status().unwrap(), ExportStatus::Failed);
    assert_eq!(fs::read(&path).unwrap(), b"prior");
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn no_overwrite_default_and_explicit_replace_success() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("existing.gif");
    fs::write(&path, b"prior").unwrap();
    assert!(matches!(
        export(&mut frames(), &plan(0.1, false), &path, &Default::default()),
        Err(GifError::DestinationExists)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"prior");
    export_gif(
        &mut frames(),
        &plan(0.1, false),
        None,
        &path,
        ReplacePolicy::ReplaceExistingFile,
        &Default::default(),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(decode(&path).len(), 1);
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn cancellation_before_during_and_after_encoding_prevents_publication() {
    let dir = tempdir().unwrap();
    for at in [0, 1, 3] {
        let path = dir.path().join(format!("cancel-{at}.gif"));
        fs::write(&path, b"prior").unwrap();
        let control = ExportControl::default();
        if at == 0 {
            control.cancel().unwrap();
        }
        let result = export_gif(
            &mut frames(),
            &plan(0.3, false),
            None,
            &path,
            ReplacePolicy::ReplaceExistingFile,
            &control,
            |written, _| {
                if written == at {
                    control.cancel().unwrap();
                }
            },
        );
        assert!(matches!(result, Err(GifError::Cancelled)));
        assert_eq!(fs::read(&path).unwrap(), b"prior");
        no_stage(dir.path());
    }
}
#[cfg(unix)]
#[test]
fn successful_publication_is_terminal_and_late_cancel_preserves_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("success.gif");
    let control = ExportControl::default();
    export(&mut frames(), &plan(0.1, false), &path, &control).unwrap();
    assert_eq!(control.status().unwrap(), ExportStatus::Published);
    assert_eq!(control.cancel().unwrap(), CancelOutcome::AlreadyPublished);
    assert!(path.exists());
    assert!(matches!(
        export(
            &mut frames(),
            &plan(0.1, false),
            &dir.path().join("again.gif"),
            &control
        ),
        Err(GifError::ControlAlreadyUsed)
    ));
}
#[test]
fn cancellation_and_publication_share_a_linearized_fence() {
    let control = ExportControl::default();
    let _job = control.begin().unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let publisher = {
        let control = control.clone();
        let entered = entered.clone();
        let release = release.clone();
        thread::spawn(move || {
            control.publish(|| {
                entered.wait();
                release.wait();
                Ok(())
            })
        })
    };
    entered.wait();
    // The publisher holds the fence before cancellation is requested. Cancelling
    // must wait and report AlreadyPublished, never falsely report successful cancel.
    let (requested, observed) = mpsc::channel();
    let canceller = {
        let control = control.clone();
        thread::spawn(move || {
            requested.send(()).unwrap();
            control.cancel()
        })
    };
    observed.recv().unwrap();
    release.wait();
    publisher.join().unwrap().unwrap();
    assert_eq!(
        canceller.join().unwrap().unwrap(),
        CancelOutcome::AlreadyPublished
    );
    let cancelled = ExportControl::default();
    let _job = cancelled.begin().unwrap();
    cancelled.cancel().unwrap();
    assert!(matches!(
        cancelled.publish::<()>(|| panic!("must not execute")),
        Err(GifError::Cancelled)
    ));
}
#[cfg(unix)]
#[test]
fn competing_exports_do_not_clobber_and_new_destination_race_is_safe() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("race.gif");
    let result = export_gif(
        &mut frames(),
        &plan(0.1, false),
        None,
        &path,
        ReplacePolicy::RefuseExisting,
        &Default::default(),
        |_, _| fs::write(&path, b"raced prior").unwrap(),
    );
    assert!(matches!(result, Err(GifError::DestinationExists)));
    assert_eq!(fs::read(&path).unwrap(), b"raced prior");
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn source_same_path_is_never_replaced_even_with_replace_permission() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("movie.mov");
    fs::write(&path, b"source").unwrap();
    assert!(matches!(
        export_gif(
            &mut frames(),
            &plan(0.1, false),
            Some(&path),
            &path,
            ReplacePolicy::ReplaceExistingFile,
            &Default::default(),
            |_, _| {}
        ),
        Err(GifError::SameFile)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"source");
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn source_symlinks_hardlinks_and_late_aliases_are_protected() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    let movie = dir.path().join("movie.mov");
    fs::write(&movie, b"source").unwrap();
    for (name, symbolic) in [("symlink.gif", true), ("hardlink.gif", false)] {
        let dest = dir.path().join(name);
        if symbolic {
            symlink(&movie, &dest).unwrap();
        } else {
            fs::hard_link(&movie, &dest).unwrap();
        }
        assert!(matches!(
            export_gif(
                &mut frames(),
                &plan(0.1, false),
                Some(&movie),
                &dest,
                ReplacePolicy::ReplaceExistingFile,
                &Default::default(),
                |_, _| {}
            ),
            Err(GifError::SameFile)
        ));
        assert_eq!(fs::read(&movie).unwrap(), b"source");
    }
    let dest = dir.path().join("late.gif");
    assert!(matches!(
        export_gif(
            &mut frames(),
            &plan(0.1, false),
            Some(&movie),
            &dest,
            ReplacePolicy::ReplaceExistingFile,
            &Default::default(),
            |_, _| fs::hard_link(&movie, &dest).unwrap()
        ),
        Err(GifError::SameFile)
    ));
    assert_eq!(fs::read(&dest).unwrap(), b"source");
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn staged_and_published_files_are_owner_only_and_other_symlinks_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempdir().unwrap();
    let path = dir.path().join("private.gif");
    export_gif(
        &mut frames(),
        &plan(0.1, false),
        None,
        &path,
        ReplacePolicy::RefuseExisting,
        &Default::default(),
        |_, _| {
            let entry = fs::read_dir(dir.path()).unwrap().next().unwrap().unwrap();
            assert_eq!(
                entry.metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
        },
    )
    .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let link = dir.path().join("other.gif");
    symlink(&path, &link).unwrap();
    assert!(matches!(
        export_gif(
            &mut frames(),
            &plan(0.1, false),
            None,
            &link,
            ReplacePolicy::ReplaceExistingFile,
            &Default::default(),
            |_, _| {}
        ),
        Err(GifError::UnsafeDestination)
    ));
}
#[cfg(unix)]
#[test]
fn malformed_gif_readbacks_are_rejected_without_loading_full_output() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("valid.gif");
    let p = plan(0.2, true);
    export(&mut frames(), &p, &path, &Default::default()).unwrap();
    let bytes = fs::read(&path).unwrap();
    let mut cases = vec![bytes[..bytes.len() - 1].to_vec(), bytes[..20].to_vec()];
    let mut trailing = bytes.clone();
    trailing.push(0);
    cases.push(trailing);
    let mut delay = bytes.clone();
    let gce = delay.windows(3).position(|b| b == [0x21, 0xf9, 4]).unwrap();
    delay[gce + 4] = 1;
    cases.push(delay);
    let mut repeat = bytes.clone();
    let extension = repeat
        .windows(11)
        .position(|b| b == b"NETSCAPE2.0")
        .unwrap();
    repeat[extension + 13] = 1;
    cases.push(repeat);
    let mut corrupt = bytes.clone();
    corrupt[0] = 0;
    cases.push(corrupt);
    for (index, data) in cases.into_iter().enumerate() {
        let path = dir.path().join(format!("bad-{index}.gif"));
        fs::write(&path, data).unwrap();
        assert!(
            gif_structure::validate(&mut fs::File::open(path).unwrap(), &p, &Default::default())
                .is_err(),
            "case {index}"
        );
    }
}
#[cfg(unix)]
#[test]
fn loop_markers_in_comment_payload_are_not_repeat_blocks() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("comment.gif");
    let p = plan(0.1, false);
    export(&mut frames(), &p, &path, &Default::default()).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes.pop();
    bytes.extend_from_slice(b"\x21\xfe\x0bNETSCAPE2.0\x00\x3b");
    fs::write(&path, bytes).unwrap();
    gif_structure::validate(&mut fs::File::open(path).unwrap(), &p, &Default::default()).unwrap();
}
#[test]
fn counting_writer_rejects_output_overflow_and_reports_underlying_failure() {
    let control = ExportControl::default();
    let mut data = vec![];
    let mut writer = gif_encode::BoundedWriter {
        inner: &mut data,
        written: 0,
        maximum: 4,
        control: &control,
    };
    writer.write_all(b"1234").unwrap();
    assert!(writer.write_all(b"5").is_err());
    assert_eq!(data, b"1234");
    struct Failing;
    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("fixture disk full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = gif_encode::BoundedWriter {
        inner: Failing,
        written: 0,
        maximum: 4,
        control: &control,
    };
    assert!(writer.write_all(b"12").is_err());
    assert_eq!(writer.written, 0);
}
#[cfg(unix)]
#[test]
fn source_sample_cap_stops_pathological_equal_timestamp_stream() {
    struct Endless(usize);
    impl SequentialFrameSource for Endless {
        fn next_frame(&mut self, _: &ExportControl) -> Result<Option<DisplayRgbaFrame>, GifError> {
            self.0 += 1;
            Ok(Some(frame(0., [0, 0, 0, 255])))
        }
    }
    let dir = tempdir().unwrap();
    let path = dir.path().join("endless.gif");
    let mut source = Endless(0);
    assert!(matches!(
        export(&mut source, &plan(0.1, false), &path, &Default::default()),
        Err(GifError::LimitExceeded("sequential source samples"))
    ));
    assert_eq!(source.0, MAX_SOURCE_SAMPLES);
    assert!(!path.exists());
    no_stage(dir.path());
}
#[cfg(unix)]
#[test]
fn truncated_lzw_payload_fails_full_readback() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("broken.gif");
    let p = plan(0.1, false);
    export(&mut frames(), &p, &path, &Default::default()).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).unwrap();
    // Retain block structure, corrupt every compressed byte of the single tiny frame.
    let image = bytes.iter().position(|b| *b == 0x2c).unwrap();
    let palette = if bytes[image + 9] & 0x80 != 0 {
        3usize << (usize::from(bytes[image + 9] & 7) + 1)
    } else {
        0
    };
    let sub = image + 10 + palette + 1;
    let length = usize::from(bytes[sub]);
    bytes[sub + 1..sub + 1 + length].fill(0xff);
    fs::write(&path, bytes).unwrap();
    assert!(
        gif_structure::validate(&mut fs::File::open(path).unwrap(), &p, &Default::default())
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn failed_staged_readback_never_replaces_the_previous_file() {
    use std::io::{Seek, SeekFrom};
    let dir = tempdir().unwrap();
    let path = dir.path().join("previous.gif");
    fs::write(&path, b"previous").unwrap();
    let control = ExportControl::default();
    let result = export_gif(
        &mut frames(),
        &plan(0.1, false),
        None,
        &path,
        ReplacePolicy::ReplaceExistingFile,
        &control,
        |_, _| {
            let stage = fs::read_dir(dir.path())
                .unwrap()
                .find_map(|entry| {
                    let entry = entry.unwrap();
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".BelloBox-export-")
                        .then(|| entry.path())
                })
                .unwrap();
            let mut file = fs::OpenOptions::new().write(true).open(stage).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(b"BAD").unwrap();
        },
    );
    assert!(matches!(result, Err(GifError::InvalidOutput)));
    assert_eq!(control.status().unwrap(), ExportStatus::Failed);
    assert_eq!(fs::read(&path).unwrap(), b"previous");
    no_stage(dir.path());
}

#[test]
fn encoder_trailer_failure_is_observable() {
    struct FailTrailer;
    impl Write for FailTrailer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes == [0x3b] {
                Err(std::io::Error::other("fixture trailer failure"))
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let encoder = ::gif::Encoder::new(FailTrailer, 1, 1, &[]).unwrap();
    assert!(encoder.into_inner().is_err());
}

#[test]
fn source_wire_null_defaults_and_signed_64_bit_bounds() {
    assert!(
        serde_json::to_value(GifExportOptions::default())
            .unwrap()
            .get("trimEnd")
            .is_none()
    );
    let nulls =
        r#"{"framesPerSecond":null,"maxWidth":null,"loops":null,"trimStart":null,"trimEnd":null}"#;
    assert_eq!(
        serde_json::from_str::<GifExportOptions>(nulls).unwrap(),
        GifExportOptions::default()
    );
    let minimum = r#"{"framesPerSecond":-9223372036854775808,"maxWidth":-9223372036854775808}"#;
    let raw = serde_json::from_str::<GifExportOptions>(minimum).unwrap();
    assert_eq!((raw.frames_per_second, raw.max_width), (i64::MIN, i64::MIN));
    assert_eq!(
        (
            raw.normalized().frames_per_second,
            raw.normalized().max_width
        ),
        (5, 64)
    );
    let maximum = r#"{"framesPerSecond":9223372036854775807,"maxWidth":9223372036854775807}"#;
    let raw = serde_json::from_str::<GifExportOptions>(maximum).unwrap();
    assert_eq!((raw.frames_per_second, raw.max_width), (i64::MAX, i64::MAX));
    assert_eq!(
        (
            raw.normalized().frames_per_second,
            raw.normalized().max_width
        ),
        (20, 1080)
    );
    for invalid in [
        r#"{"framesPerSecond":9223372036854775808}"#,
        r#"{"maxWidth":-9223372036854775809}"#,
        r#"{"framesPerSecond":20.0}"#,
        r#"{"loops":"true"}"#,
    ] {
        assert!(serde_json::from_str::<GifExportOptions>(invalid).is_err());
    }
}

#[test]
fn filesystem_support_gate_matches_the_platform_contract() {
    assert_eq!(gif_file::ensure_filesystem_supported().is_ok(), cfg!(unix));
}

#[cfg(not(unix))]
#[test]
fn unsupported_filesystem_export_fails_before_any_effect() {
    struct MustNotRead;
    impl SequentialFrameSource for MustNotRead {
        fn next_frame(&mut self, _: &ExportControl) -> Result<Option<DisplayRgbaFrame>, GifError> {
            panic!("unsupported export must not read a source");
        }
    }
    let dir = tempdir().unwrap();
    let existing = dir.path().join("existing.gif");
    fs::write(&existing, b"prior").unwrap();
    let missing_source = dir.path().join("missing.mov");
    for destination in [
        &existing,
        &dir.path().join("fresh.gif"),
        &dir.path().join("missing-directory/output.gif"),
    ] {
        for source in [None, Some(missing_source.as_path())] {
            let control = ExportControl::default();
            assert!(matches!(
                export_gif(
                    &mut MustNotRead,
                    &plan(0.1, false),
                    source,
                    destination,
                    ReplacePolicy::ReplaceExistingFile,
                    &control,
                    |_, _| panic!("no progress")
                ),
                Err(GifError::UnsupportedFilesystem)
            ));
            assert_eq!(control.status().unwrap(), ExportStatus::Ready);
        }
    }
    assert_eq!(fs::read(existing).unwrap(), b"prior");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn replaced_stage_is_neither_published_nor_removed() {
    let dir = tempdir().unwrap();
    let destination = dir.path().join("prior.gif");
    fs::write(&destination, b"prior destination").unwrap();
    let moved_original = dir.path().join("moved-owned-stage.gif");
    let mut replaced_path = None;
    let result = export_gif(
        &mut frames(),
        &plan(0.1, false),
        None,
        &destination,
        ReplacePolicy::ReplaceExistingFile,
        &ExportControl::default(),
        |_, _| {
            let stage = fs::read_dir(dir.path())
                .unwrap()
                .find_map(|entry| {
                    let entry = entry.unwrap();
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".BelloBox-export-")
                        .then(|| entry.path())
                })
                .unwrap();
            fs::rename(&stage, &moved_original).unwrap();
            fs::write(&stage, b"replacement belongs to another writer").unwrap();
            replaced_path = Some(stage);
        },
    );
    assert!(matches!(result, Err(GifError::StageIdentityChanged)));
    assert_eq!(fs::read(&destination).unwrap(), b"prior destination");
    assert_eq!(
        fs::read(replaced_path.unwrap()).unwrap(),
        b"replacement belongs to another writer"
    );
    // External movement prevents path-owned cleanup of the original; the exporter
    // deliberately does not discover or remove arbitrary other directory entries.
    assert_eq!(decode(&moved_original).len(), 1);
}
