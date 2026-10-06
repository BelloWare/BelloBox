//! Synthetic only: Rust/CoreVideo frames, test-only AVAssetWriter MOV files,
//! native reader, and the existing Rust staged GIF encoder. Never screen/audio.
use super::*;
use bellobox_core::recording::gif::{
    self, DisplayRgbaFrame, ExportControl, GifError, GifExportOptions, GifExportPlan,
    GifSourceInfo, ReplacePolicy, SequentialFrameSource,
};
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterStatus, AVFileTypeQuickTimeMovie,
    AVVideoCodecKey, AVVideoCodecTypeH264, AVVideoHeightKey, AVVideoWidthKey,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    kCMTimeInvalid, CMSampleBuffer, CMSampleTimingInfo, CMVideoFormatDescription,
    CMVideoFormatDescriptionCreateForImageBuffer,
};
use std::{fs, ptr::NonNull, sync::Mutex};

// Source admission is deliberately single-job. Keep independent native fixtures
// serialized even when Cargo's test harness runs other pure tests concurrently.
static NATIVE_TESTS: Mutex<()> = Mutex::new(());
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

fn pixel(
    width: u32,
    height: u32,
    pattern: impl Fn(u32, u32) -> [u8; 4],
) -> CFRetained<CVPixelBuffer> {
    let mut output = ptr::null_mut();
    let status = unsafe {
        CVPixelBufferCreate(
            None,
            width as usize,
            height as usize,
            kCVPixelFormatType_32BGRA,
            None,
            NonNull::from(&mut output),
        )
    };
    assert_eq!(status, kCVReturnSuccess);
    let pixel = unsafe { CFRetained::from_raw(NonNull::new(output).unwrap()) };
    assert_eq!(
        unsafe { CVPixelBufferLockBaseAddress(&pixel, CVPixelBufferLockFlags::empty()) },
        kCVReturnSuccess
    );
    let stride = CVPixelBufferGetBytesPerRow(&pixel);
    let size = CVPixelBufferGetDataSize(&pixel);
    validate_pixel_storage(width, height, stride, size).unwrap();
    let base = CVPixelBufferGetBaseAddress(&pixel);
    assert!(!base.is_null());
    for y in 0..height {
        let row = unsafe {
            std::slice::from_raw_parts_mut(
                base.cast::<u8>().add(y as usize * stride),
                width as usize * 4,
            )
        };
        for x in 0..width {
            row[x as usize * 4..x as usize * 4 + 4].copy_from_slice(&pattern(x, y));
        }
    }
    assert_eq!(
        unsafe { CVPixelBufferUnlockBaseAddress(&pixel, CVPixelBufferLockFlags::empty()) },
        kCVReturnSuccess
    );
    pixel
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

// AVAssetWriter is enabled only through the macOS dev-dependency features.
fn write_movie(path: &Path, transform: CGAffineTransform) {
    autoreleasepool(|_| {
        let url = NSURL::from_file_path(path).unwrap();
        let writer = unsafe {
            AVAssetWriter::initWithURL_fileType_error(
                AVAssetWriter::alloc(),
                &url,
                AVFileTypeQuickTimeMovie.unwrap(),
            )
        }
        .unwrap();
        struct Guard(Retained<AVAssetWriter>);
        impl Drop for Guard {
            fn drop(&mut self) {
                if unsafe { self.0.status() } == AVAssetWriterStatus::Writing {
                    unsafe {
                        self.0.cancelWriting();
                    }
                }
            }
        }
        let writer = Guard(writer);
        let width = NSNumber::new_u32(64);
        let height = NSNumber::new_u32(48);
        let settings: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
            &[
                unsafe { AVVideoCodecKey.unwrap() },
                unsafe { AVVideoWidthKey.unwrap() },
                unsafe { AVVideoHeightKey.unwrap() },
            ],
            &[unsafe { AVVideoCodecTypeH264.unwrap() }, &*width, &*height],
        );
        let input = unsafe {
            AVAssetWriterInput::initWithMediaType_outputSettings(
                AVAssetWriterInput::alloc(),
                AVMediaTypeVideo.unwrap(),
                Some(&settings),
            )
        };
        unsafe {
            input.setExpectsMediaDataInRealTime(false);
            input.setTransform(transform);
            assert!(writer.0.canAddInput(&input));
            writer.0.addInput(&input);
            assert!(writer.0.startWriting());
            writer
                .0
                .startSessionAtSourceTime(CMTime::with_seconds(0., 600));
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        for index in 0..4 {
            while !unsafe { input.isReadyForMoreMediaData() } {
                assert!(Instant::now() < deadline, "synthetic writer stalled");
                std::thread::sleep(Duration::from_millis(5));
            }
            let pixel = pixel(64, 48, |x, _| {
                if index == 3 {
                    [255, 0, 255, 255]
                } else if x < 32 {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                }
            });
            let mut format: *const CMVideoFormatDescription = ptr::null();
            assert_eq!(
                unsafe {
                    CMVideoFormatDescriptionCreateForImageBuffer(
                        None,
                        &pixel,
                        NonNull::from(&mut format),
                    )
                },
                0
            );
            let format = unsafe { CFRetained::from_raw(NonNull::new(format.cast_mut()).unwrap()) };
            let mut timing = CMSampleTimingInfo {
                duration: unsafe { CMTime::with_seconds(0.1, 600) },
                presentationTimeStamp: unsafe { CMTime::with_seconds(f64::from(index) / 10., 600) },
                decodeTimeStamp: unsafe { kCMTimeInvalid },
            };
            let mut sample: *mut CMSampleBuffer = ptr::null_mut();
            assert_eq!(
                unsafe {
                    CMSampleBuffer::create_ready_with_image_buffer(
                        None,
                        &pixel,
                        &format,
                        NonNull::from(&mut timing),
                        NonNull::from(&mut sample),
                    )
                },
                0
            );
            let sample = unsafe { CFRetained::from_raw(NonNull::new(sample).unwrap()) };
            assert!(unsafe { input.appendSampleBuffer(&sample) });
        }
        unsafe {
            input.markAsFinished();
        }
        let done = Arc::new(AtomicBool::new(false));
        let ready = done.clone();
        let callback = move || ready.store(true, Ordering::Release);
        sendable(&callback);
        let block = RcBlock::new(callback);
        unsafe {
            writer.0.finishWritingWithCompletionHandler(&block);
        }
        while !done.load(Ordering::Acquire) {
            assert!(
                Instant::now() < deadline,
                "synthetic writer finalization stalled"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(unsafe { writer.0.status() }, AVAssetWriterStatus::Completed);
    });
}
struct GifSource(super::super::MovieReader);
impl SequentialFrameSource for GifSource {
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
    let asset = super::super::MovieAsset::open(&movie, MovieCancellation::default()).unwrap();
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
        let asset = super::super::MovieAsset::open(&movie, MovieCancellation::default()).unwrap();
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
    let asset = super::super::MovieAsset::open(&movie, MovieCancellation::default()).unwrap();
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
    let asset = Asset::open(&movie, MovieCancellation::default()).unwrap();
    let mut reader = asset
        .reader(MovieReadRange::new(0., 0.3, 0.1).unwrap())
        .unwrap();
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
    assert!(super::super::MovieAsset::open(&bad, MovieCancellation::default()).is_err());
    let cancelled = MovieCancellation::default();
    cancelled.cancel();
    assert!(matches!(
        super::super::MovieAsset::open(&bad, cancelled),
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
