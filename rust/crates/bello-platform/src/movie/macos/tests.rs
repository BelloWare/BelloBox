//! Synthetic only: Rust/CoreVideo frames, test-only AVAssetWriter MOV files,
//! native reader, and the existing Rust staged GIF encoder. Never screen/audio.
use super::fixtures::{pixel, write_movie};
use super::*;
use bellobox_core::recording::gif::{
    self, DisplayRgbaFrame, ExportControl, GifError, GifExportOptions, GifExportPlan,
    GifSourceInfo, ReplacePolicy, SequentialFrameSource,
};
use std::{fs, path::PathBuf, sync::Mutex};

// Source admission is deliberately single-job. Keep independent native fixtures
// serialized even when Cargo's test harness runs other pure tests concurrently.
static NATIVE_TESTS: Mutex<()> = Mutex::new(());

#[test]
fn native_positive_composition_before_first_seek_matches_full_range_control() {
    use sha2::{Digest, Sha256};
    let _serial = NATIVE_TESTS.lock().unwrap();
    let dir = TempDir::new();
    let mut traces = Vec::new();
    for (positive, expected_hash, expected_pts) in [
        (
            false,
            "ef59f7b291b1fbbe16969b65b97d5918e03224527fcbd68f7c984ce4ba7fed16",
            [0., 0.1, 0.2],
        ),
        (
            true,
            "abd5e23b328507ea0c1c4f894e2d7d34cb5493ae0011b86413cb2821bd5959a8",
            [0.1, 0.2, 0.3],
        ),
    ] {
        let path = dir
            .0
            .join(if positive { "positive.mov" } else { "zero.mov" });
        super::fixtures::write_composition_movie(&path, positive).unwrap();
        let original = fs::read(&path).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&original)), expected_hash);
        let asset = open_fixture(&path, Default::default()).unwrap();
        let duration = asset.info.duration;
        let mut reader = asset.native.reader(0., duration).unwrap();
        let mut frames = Vec::new();
        while let Some(frame) = reader.next_frame(&|| false).unwrap() {
            assert!(frames.len() < 3, "unexpected extra decoded image");
            frames.push(frame.into_parts());
        }
        reader.finish(&|| false).unwrap();
        drop(reader);
        assert!(
            !JOB_ACTIVE.load(Ordering::Acquire),
            "full-range reader retired"
        );
        assert_eq!(frames.iter().map(|f| f.0).collect::<Vec<_>>(), expected_pts);
        for (pts, width, height, rgba) in &frames {
            assert_eq!((*width, *height), (64, 48));
            eprintln!("composition full range positive={positive} PTS={pts} size={width}x{height} RGBA_SHA256={:x}", Sha256::digest(rgba));
        }
        for (request, index) in [
            (0.05, 0),
            (0.25, if positive { 1 } else { 2 }),
            (duration, 2),
        ] {
            let actual = open_fixture(&path, Default::default())
                .unwrap()
                .seek(MovieSeek::new(request).unwrap())
                .unwrap()
                .into_parts();
            assert_eq!(actual, frames[index], "request {request}");
            assert!(!JOB_ACTIVE.load(Ordering::Acquire), "seek reader retired");
            eprintln!("composition seek positive={positive} request={request} actual={} exact_full_range_RGBA=true", actual.0);
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        traces.push(frames);
    }
    for (control, positive) in traces[0].iter().zip(&traces[1]) {
        assert_eq!(
            (control.1, control.2, &control.3),
            (positive.1, positive.2, &positive.3)
        );
    }
    eprintln!("composition pair: all three same-host decoded RGBA frames match exactly; positive first image is 0.1, before-first request 0.05 selects it");
}

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "bellobox-native-movie-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity() -> CGAffineTransform {
    CGAffineTransform {
        a: 1.,
        b: 0.,
        c: 0.,
        d: 1.,
        tx: 0.,
        ty: 0.,
    }
}
fn geometry(natural: CGSize, t: CGAffineTransform) -> DisplayGeometry {
    display_geometry(
        natural.width,
        natural.height,
        [t.a, t.b, t.c, t.d, t.tx, t.ty],
    )
    .unwrap()
}
#[test]
fn native_render_matches_display_orientation_and_coded_size_scaling() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    autoreleasepool(|_| {
        let source = pixel(4, 2, |x, y| match (x < 2, y == 0) {
            (true, true) => [0, 0, 255, 255],     // red, top-left
            (false, true) => [0, 255, 0, 255],    // green, top-right
            (true, false) => [255, 0, 0, 255],    // blue, bottom-left
            (false, false) => [0, 255, 255, 255], // yellow, bottom-right
        });
        let natural = CGSize {
            width: 4.,
            height: 2.,
        };
        let plain = render(
            &source,
            natural,
            identity(),
            geometry(natural, identity()),
            &|| false,
        )
        .unwrap();
        assert_eq!(&plain[..4], &[255, 0, 0, 255]);
        assert_eq!(&plain[12..16], &[0, 255, 0, 255]);
        assert_eq!(&plain[16..20], &[0, 0, 255, 255]);
        assert_eq!(&plain[28..32], &[255, 255, 0, 255]);
        let rotate = CGAffineTransform {
            a: 0.,
            b: 1.,
            c: -1.,
            d: 0.,
            tx: 2.,
            ty: 0.,
        };
        let rgba = render(&source, natural, rotate, geometry(natural, rotate), &|| {
            false
        })
        .unwrap();
        assert_eq!(&rgba[..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255]);
        assert_eq!(&rgba[24..28], &[255, 255, 0, 255]);
        assert_eq!(&rgba[28..32], &[0, 255, 0, 255]);
        let mirror = CGAffineTransform {
            a: -1.,
            b: 0.,
            c: 0.,
            d: 1.,
            tx: 4.,
            ty: 0.,
        };
        let rgba = render(&source, natural, mirror, geometry(natural, mirror), &|| {
            false
        })
        .unwrap();
        assert_eq!(&rgba[..4], &[0, 255, 0, 255]);
        assert_eq!(&rgba[12..16], &[255, 0, 0, 255]);
        assert_eq!(&rgba[16..20], &[255, 255, 0, 255]);
        assert_eq!(&rgba[28..32], &[0, 0, 255, 255]);
        assert!(rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
        // Coded2x2 pixels presented in a natural4x2 rectangle must be accepted.
        let small = pixel(2, 2, |x, _| {
            if x == 0 {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 255]
            }
        });
        let scaled = render(
            &small,
            natural,
            identity(),
            geometry(natural, identity()),
            &|| false,
        )
        .unwrap();
        assert_eq!(scaled.len(), 32);
        assert!(scaled[0] > 200);
        assert!(scaled[14] > 200);
        let alpha = pixel(2, 1, |x, _| {
            if x == 0 {
                [0, 0, 128, 128]
            } else {
                [0, 0, 0, 0]
            }
        });
        let alpha_size = CGSize {
            width: 2.,
            height: 1.,
        };
        let opaque = render(
            &alpha,
            alpha_size,
            identity(),
            geometry(alpha_size, identity()),
            &|| false,
        )
        .unwrap();
        assert_eq!(&opaque, &[128, 0, 0, 255, 0, 0, 0, 255]);
        assert!(matches!(
            render(
                &source,
                natural,
                identity(),
                geometry(natural, identity()),
                &|| true
            ),
            Err(MovieError::Cancelled)
        ));
    });
}
#[test]
fn native_time_rejects_invalid_nonfinite_negative_and_epoch_values() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    let time = CMTime {
        value: 3,
        timescale: 10,
        flags: CMTimeFlags::Valid,
        epoch: 0,
    };
    assert!((numeric_seconds(time).unwrap() - 0.3).abs() < 1e-9);
    for time in [
        CMTime {
            timescale: 0,
            ..time
        },
        CMTime { value: -1, ..time },
        CMTime { epoch: 1, ..time },
        CMTime {
            flags: CMTimeFlags::Valid | CMTimeFlags::Indefinite,
            ..time
        },
        CMTime {
            flags: CMTimeFlags::empty(),
            ..time
        },
    ] {
        assert!(numeric_seconds(time).is_err());
    }
}

fn open_fixture(
    path: &Path,
    cancellation: MovieCancellation,
) -> MovieResult<super::super::MovieAsset> {
    let mut selected = SelectedMovie::select(path)?;
    selected.generated = true;
    super::super::MovieAsset::open_selected(selected, cancellation)
}
struct GifSource(super::super::MovieReader);
impl SequentialFrameSource for GifSource {
    fn finish(&mut self, control: &ExportControl) -> Result<(), GifError> {
        self.0
            .finish(|| control.check_active().is_err())
            .map_err(|e| GifError::Source(e.to_string()))
    }
    fn next_frame(
        &mut self,
        control: &ExportControl,
    ) -> Result<Option<DisplayRgbaFrame>, GifError> {
        control.check_active()?;
        let frame = self
            .0
            .next_frame(|| control.check_active().is_err())
            .map_err(|error| {
                if error == MovieError::Cancelled {
                    GifError::Cancelled
                } else {
                    GifError::Source(error.to_string())
                }
            })?;
        frame
            .map(|frame| {
                let (pts, w, h, rgba) = frame.into_parts();
                DisplayRgbaFrame::new(pts, w, h, rgba)
            })
            .transpose()
    }
}
fn source_plan(asset: &super::super::MovieAsset, loops: bool) -> GifExportPlan {
    let info = asset.info();
    GifExportPlan::make(
        GifSourceInfo {
            duration: info.duration,
            display_width: info.display_width,
            display_height: info.display_height,
            nominal_frame_rate: info.nominal_frame_rate,
        },
        &GifExportOptions {
            frames_per_second: 10,
            max_width: 64,
            loops,
            trim_start: 0.,
            trim_end: Some(0.3),
        },
    )
    .unwrap()
}
fn into_gif_source(asset: super::super::MovieAsset, plan: &GifExportPlan) -> GifSource {
    GifSource(
        asset
            .reader(MovieReadRange::new(plan.start(), plan.end(), plan.frame_delay()).unwrap())
            .unwrap(),
    )
}
#[test]
fn native_synthetic_movie_pts_orientation_and_staged_gif_integration() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    let dir = TempDir::new();
    let movie = dir.0.join("fixture.mov");
    write_movie(
        &movie,
        CGAffineTransform {
            a: 0.,
            b: 1.,
            c: -1.,
            d: 0.,
            tx: 48.,
            ty: 0.,
        },
    );
    let before = fs::read(&movie).unwrap();
    let asset = open_fixture(&movie, MovieCancellation::default()).unwrap();
    assert_eq!(
        (asset.info().display_width, asset.info().display_height),
        (48., 64.)
    );
    let mut reader = asset
        .reader(MovieReadRange::new(0., 0.3, 0.1).unwrap())
        .unwrap();
    for index in 0..4 {
        let frame = reader.next_frame(|| false).unwrap().unwrap();
        assert!((frame.presentation_seconds() - f64::from(index) / 10.).abs() < 0.001);
        assert_eq!(frame.size(), (48, 64));
        let (_, width, _, rgba) = frame.into_parts();
        let top = (8 * width as usize + 24) * 4;
        let bottom = (56 * width as usize + 24) * 4;
        if index < 3 {
            assert!(rgba[top] > 180);
            assert!(rgba[bottom + 2] > 180);
        } else {
            assert!(rgba[top] > 180 && rgba[top + 2] > 180);
        }
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
    }
    assert!(reader.next_frame(|| false).unwrap().is_none());
    drop(reader);
    for loops in [false, true] {
        let asset = open_fixture(&movie, MovieCancellation::default()).unwrap();
        let plan = source_plan(&asset, loops);
        let mut source = into_gif_source(asset, &plan);
        let destination = dir.0.join(format!("result-{loops}.gif"));
        let result = gif::export_gif(
            &mut source,
            &plan,
            Some(&movie),
            &destination,
            ReplacePolicy::RefuseExisting,
            &ExportControl::default(),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(result.frame_count, 3);
        assert_eq!(result.size, (48, 64));
        assert!((result.duration - 0.3).abs() < 0.001);
        assert!(fs::read(destination).unwrap().starts_with(b"GIF"));
    }
    assert_eq!(fs::read(&movie).unwrap(), before);
    assert!(fs::read_dir(&dir.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".BelloBox-export-")));
}
#[test]
fn native_cancel_and_sample_limit_retire_reader_and_preserve_prior_output() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    let dir = TempDir::new();
    let movie = dir.0.join("fixture.mov");
    write_movie(&movie, identity());
    let before = fs::read(&movie).unwrap();
    let asset = open_fixture(&movie, MovieCancellation::default()).unwrap();
    let plan = source_plan(&asset, false);
    let mut source = into_gif_source(asset, &plan);
    let control = ExportControl::default();
    let destination = dir.0.join("prior.gif");
    fs::write(&destination, b"prior").unwrap();
    assert!(matches!(
        gif::export_gif(
            &mut source,
            &plan,
            Some(&movie),
            &destination,
            ReplacePolicy::ReplaceExistingFile,
            &control,
            |_, _| {
                control.cancel().unwrap();
            }
        ),
        Err(GifError::Cancelled)
    ));
    drop(source);
    assert_eq!(fs::read(&destination).unwrap(), b"prior");
    assert_eq!(fs::read(&movie).unwrap(), before);
    let asset = Asset::open(
        Arc::new(SourceFile::open(&movie).unwrap()),
        MovieCancellation::default(),
    )
    .unwrap();
    let mut reader = asset.reader(0., 0.4).unwrap();
    reader.samples = MAX_MOVIE_SAMPLES;
    assert!(matches!(
        reader.next_frame(&|| false),
        Err(MovieError::LimitExceeded)
    ));
    assert!(reader.finished);
    assert!(reader.next_frame(&|| false).unwrap().is_none());
    assert!(fs::read_dir(&dir.0).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".BelloBox-export-")));
}
#[test]
fn native_invalid_movie_reports_metadata_error_and_honors_precancel() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    let dir = TempDir::new();
    let bad = dir.0.join("invalid.mov");
    fs::write(&bad, b"synthetic invalid movie").unwrap();
    assert!(open_fixture(&bad, MovieCancellation::default()).is_err());
    let cancelled = MovieCancellation::default();
    cancelled.cancel();
    assert!(matches!(
        open_fixture(&bad, cancelled),
        Err(MovieError::Cancelled)
    ));
}

#[test]
fn nil_sample_status_mapping_never_masks_failure_as_clean_eof() {
    assert_eq!(classify_end(AVAssetReaderStatus::Completed), Ok(()));
    assert_eq!(
        classify_end(AVAssetReaderStatus::Cancelled),
        Err(MovieError::Cancelled)
    );
    for status in [
        AVAssetReaderStatus::Unknown,
        AVAssetReaderStatus::Reading,
        AVAssetReaderStatus::Failed,
    ] {
        assert_eq!(classify_end(status), Err(MovieError::DecodeFailed));
    }
}

#[test]
fn native_admission_waits_for_old_converter_lease_and_cancels_own_waiter() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    let cancel = MovieCancellation::default();
    let old = Lease::acquire(&cancel, Instant::now() + Duration::from_secs(5)).unwrap();
    let waiting = MovieCancellation::default();
    let worker_cancel = waiting.clone();
    let (started, start) = std::sync::mpsc::channel();
    let (done, result) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        done.send(
            Lease::acquire(&worker_cancel, Instant::now() + Duration::from_secs(5)).map(|_| ()),
        )
        .unwrap();
    });
    start.recv().unwrap();
    assert!(result.try_recv().is_err());
    waiting.cancel();
    assert_eq!(
        result.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(MovieError::Cancelled)
    );
    worker.join().unwrap();
    assert!(
        JOB_ACTIVE.load(Ordering::Acquire),
        "cancelled waiter did not release old owner's lease"
    );
    drop(old);
    let fresh = Lease::acquire(&cancel, Instant::now() + Duration::from_secs(5)).unwrap();
    drop(fresh);
}

#[test]
fn native_final_validation_checks_late_cancel_source_mutation_and_own_retirement() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    let dir = TempDir::new();
    let movie = dir.0.join("late.mov");
    write_movie(&movie, identity());
    let cancel = MovieCancellation::default();
    let asset = open_fixture(&movie, cancel.clone()).unwrap();
    let mut reader = asset
        .reader(MovieReadRange::new(0., 0.1, 0.1).unwrap())
        .unwrap();
    assert!(reader.next_frame(|| false).unwrap().is_some());
    reader.finish(|| false).unwrap();
    reader.finish(|| false).unwrap();
    cancel.cancel();
    assert_eq!(reader.finish(|| false), Err(MovieError::Cancelled));
    drop(reader);
    let asset = open_fixture(&movie, Default::default()).unwrap();
    let mut reader = asset
        .reader(MovieReadRange::new(0., 0.1, 0.1).unwrap())
        .unwrap();
    assert!(reader.next_frame(|| false).unwrap().is_some());
    let bytes = fs::read(&movie).unwrap();
    let replacement = dir.0.join("replacement.mov");
    fs::write(&replacement, &bytes).unwrap();
    fs::rename(replacement, &movie).unwrap();
    assert_eq!(reader.finish(|| false), Err(MovieError::SourceChanged));
}

#[test]
fn native_metadata_delivery_rechecks_late_cancel_and_source_after_callback_drain() {
    let _serial = NATIVE_TESTS.lock().unwrap();
    for mutate in [false, true] {
        let dir = TempDir::new();
        let movie = dir.0.join("held.mov");
        write_movie(&movie, identity());
        let cancel = MovieCancellation::default();
        let worker_cancel = cancel.clone();
        let worker_path = movie.clone();
        let (held, receipt) = std::sync::mpsc::channel();
        let (done, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let source = Arc::new(SourceFile::open(&worker_path).unwrap());
            let asset = Asset::open(source, worker_cancel.clone()).unwrap();
            let lease = asset._lease.clone();
            let ticket = lease.callbacks.ticket();
            held.send(ticket).unwrap();
            let outcome = complete_metadata(
                Ok(asset),
                &lease,
                &worker_cancel,
                Instant::now() + Duration::from_secs(5),
            )
            .map(|_| ());
            done.send(outcome).unwrap();
        });
        let ticket = receipt.recv_timeout(Duration::from_secs(10)).unwrap();
        if mutate {
            let replacement = dir.0.join("same.mov");
            fs::copy(&movie, &replacement).unwrap();
            fs::rename(replacement, &movie).unwrap();
        } else {
            cancel.cancel();
        }
        assert!(
            result.try_recv().is_err(),
            "physical callback ticket still held"
        );
        drop(ticket);
        assert_eq!(
            result.recv_timeout(Duration::from_secs(5)).unwrap(),
            Err(if mutate {
                MovieError::SourceChanged
            } else {
                MovieError::Cancelled
            })
        );
        worker.join().unwrap();
        assert!(!JOB_ACTIVE.load(Ordering::Acquire));
    }
}
