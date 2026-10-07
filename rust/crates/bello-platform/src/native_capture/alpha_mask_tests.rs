//! Synthetic CoreGraphics comparison with ImageAlphaMask.swift. No capture,
//! display query, UI, permission, file or network operation occurs here.
//! The source-exact native adapter and independent Swift-shaped oracle receive
//! identical normalized SDR inputs (native PNG -> core decode -> straight RGBA).
//! All native-adapter RGBA assertions are exact. The portable path is separately
//! characterized: its sampling and straight RGB are not CoreGraphics-equivalent.
use super::{encode_png, pool, CaptureCancellation, Job, NativeCaptureSnapshot};
use bellobox_core::screenshot::{
    area::AreaDisplayGeometry,
    window::{FrozenWindowCandidate, FrozenWindowSession},
    window_refresh::{
        WindowRefreshContext, WindowRefreshDecision, WindowRefreshError, WindowRefreshPlan,
    },
    Point, Rect, ScreenshotDocument, ScreenshotEditSession,
};
use objc2_core_foundation::{CFData, CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapInfo, CGBlendMode,
    CGColorRenderingIntent, CGColorSpace, CGContext, CGDataProvider, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, CGInterpolationQuality,
};
use std::{
    ptr,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

// Field order drops the image before the provider/data/color owners. Retaining
// all of them explicitly also avoids any borrowed Rust storage or release hook.
struct Fixture {
    image: CFRetained<CGImage>,
    _provider: CFRetained<CGDataProvider>,
    _data: CFRetained<CFData>,
    _color: CFRetained<CGColorSpace>,
}
impl Fixture {
    fn new(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Self {
        Self::input(width, height, true, pixel)
    }
    fn normalized(document: &ScreenshotDocument) -> Self {
        let pixels = document.render_rgba().unwrap();
        Self::input(pixels.width(), pixels.height(), false, |x, y| {
            pixels.get_pixel(x, y).0
        })
    }
    fn input(
        width: u32,
        height: u32,
        premultiplied: bool,
        pixel: impl Fn(u32, u32) -> [u8; 4],
    ) -> Self {
        assert!(width > 0 && height > 0 && width <= 256 && height <= 256);
        let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let rgba = pixel(x, y);
                // The fixture accepts premultiplied pixels, independently of
                // the portable mask's straight-RGBA representation.
                if premultiplied {
                    assert!(rgba[..3].iter().all(|channel| *channel <= rgba[3]));
                }
                bytes.extend_from_slice(&rgba);
            }
        }
        let data = CFData::from_bytes(&bytes); // Copies; no borrowed Vec escapes.
        let provider = CGDataProvider::with_cf_data(Some(&data)).unwrap();
        let color = CGColorSpace::new_device_rgb().unwrap();
        // Input byte layout is explicit RGBA. The oracle output below instead
        // uses precisely Swift's PremultipliedLast flag without a byte-order flag.
        let info = CGBitmapInfo::from_bits_retain(
            CGImageByteOrderInfo::Order32Big.0
                | if premultiplied {
                    CGImageAlphaInfo::PremultipliedLast.0
                } else {
                    CGImageAlphaInfo::Last.0
                },
        );
        let image = unsafe {
            CGImage::new(
                width as usize,
                height as usize,
                8,
                32,
                width as usize * 4,
                Some(&color),
                info,
                Some(&provider),
                ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
        .unwrap();
        Self {
            image,
            _provider: provider,
            _data: data,
            _color: color,
        }
    }
    fn document(&self) -> ScreenshotDocument {
        decode_native(&self.image)
    }
}

fn worker(test: impl FnOnce() + Send + 'static) {
    // Only the Rust closure crosses threads. Every native owner is created and
    // dropped inside it; the actual PNG encoder requires a non-main thread.
    std::thread::spawn(move || {
        let _pool = unsafe { pool() }.unwrap();
        test();
    })
    .join()
    .unwrap();
}

fn decode_native(image: &CGImage) -> ScreenshotDocument {
    let job =
        Job::<NativeCaptureSnapshot>::new(CaptureCancellation::default(), Duration::from_secs(30));
    // Synchronous encode keeps the borrowed, retained image alive for its entire
    // use. No pointer is stored in a Rust integer or handed to another thread.
    let png = unsafe { encode_png((image as *const CGImage).cast_mut().cast(), job) }.unwrap();
    ScreenshotDocument::from_png(&png).unwrap()
}

fn swift_mask(frozen: &CGImage, shape: &CGImage) -> Option<CFRetained<CGImage>> {
    let width = CGImage::width(Some(frozen));
    let height = CGImage::height(Some(frozen));
    if width == 0
        || height == 0
        || width.abs_diff(CGImage::width(Some(shape))) > 1
        || height.abs_diff(CGImage::height(Some(shape))) > 1
    {
        return None;
    }
    let color = CGColorSpace::new_device_rgb()?;
    // Exact source configuration: CoreGraphics owns the output allocation and
    // chooses the row stride. We never assume its byte order or inspect its data.
    let context = unsafe {
        CGBitmapContextCreate(
            ptr::null_mut(),
            width,
            height,
            8,
            0,
            Some(&color),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }?;
    let rect = CGRect {
        origin: CGPoint { x: 0., y: 0. },
        size: CGSize {
            width: width as f64,
            height: height as f64,
        },
    };
    let ctx = Some(&*context);
    CGContext::set_interpolation_quality(ctx, CGInterpolationQuality::None);
    CGContext::draw_image(ctx, rect, Some(frozen));
    CGContext::set_blend_mode(ctx, CGBlendMode::DestinationIn);
    CGContext::draw_image(ctx, rect, Some(shape));
    // CreateImage returns an independent snapshot (possibly copy-on-write).
    // Its CFRetained owner survives the local context's destruction.
    CGBitmapContextCreateImage(ctx)
}

fn normalized_reference(
    frozen: &ScreenshotDocument,
    shape: &ScreenshotDocument,
) -> ScreenshotDocument {
    // Independent test-owned input creation and Swift-shaped context. Never call
    // the production adapter's image constructor or mask implementation here.
    let frozen = Fixture::normalized(frozen);
    let shape = Fixture::normalized(shape);
    decode_native(&swift_mask(&frozen.image, &shape.image).unwrap())
}

fn native_mask(
    frozen: ScreenshotDocument,
    shape: ScreenshotDocument,
    context: WindowRefreshContext,
) -> ScreenshotDocument {
    use crate::native_capture::{mask_image_alpha, AlphaMaskInput};
    let _serial = super::alpha_mask::TEST_SERIAL
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut session = ScreenshotEditSession::new(frozen);
    let cancellation = Arc::new(AtomicBool::new(false));
    let prepared = WindowRefreshPlan::new(
        &session,
        context,
        WindowRefreshDecision::MaskFrozenAlpha,
        cancellation.clone(),
    )
    .unwrap()
    .prepare_with_alpha_mask(shape, |frozen, shape, _| {
        let (width, height) = frozen.dimensions();
        let (sw, sh) = shape.dimensions();
        let png = mask_image_alpha(
            AlphaMaskInput {
                width,
                height,
                rgba: frozen.rgba(),
            },
            AlphaMaskInput {
                width: sw,
                height: sh,
                rgba: shape.rgba(),
            },
            CaptureCancellation::from_flag(cancellation.clone()),
        )
        .expect("native supplied-image mask");
        Ok(ScreenshotDocument::from_png(&png).unwrap())
    })
    .unwrap()
    .unwrap();
    assert!(prepared
        .apply(&mut session, context, &AtomicBool::new(false))
        .unwrap());
    assert_eq!(session.revision(), 1);
    assert!(session.document().annotations().is_empty());
    assert!(session.document().crop_rect().is_none());
    assert!(!session.can_undo());
    session.render_snapshot()
}
fn compare_rgba(case: &str, reference: &ScreenshotDocument, actual: &ScreenshotDocument) {
    assert_eq!(
        reference.dimensions(),
        actual.dimensions(),
        "{case}: dimensions"
    );
    let reference = reference.render_rgba().unwrap();
    let actual = actual.render_rgba().unwrap();
    for (x, y, expected) in reference.enumerate_pixels() {
        assert_eq!(
            actual.get_pixel(x, y).0,
            expected.0,
            "{case}: RGBA ({x},{y})"
        );
    }
}

fn context(width: u32, height: u32) -> WindowRefreshContext {
    let display = AreaDisplayGeometry {
        display_id: 1,
        cocoa_frame: Rect::new(0., 0., 16., 16.),
        pixel_size: (width, height),
        rotation_degrees: 0,
    };
    let candidate = FrozenWindowCandidate {
        window_id: 10,
        owner_process_id: 20,
        frame_local_points: Rect::new(0., 0., 16., 16.),
        layer: 0,
        alpha: 1.,
        on_screen: true,
    };
    let mut selector = FrozenWindowSession::new(display, &[candidate], 99).unwrap();
    let fixture = Fixture::new(width, height, |_, _| [0, 0, 0, 255]);
    selector
        .accept_frozen(
            selector.freeze_token().unwrap(),
            fixture.document(),
            display,
        )
        .unwrap();
    selector.begin_press(Point::new(8., 8.), display).unwrap();
    let commit = selector
        .end_press(Point::new(8., 8.), display)
        .unwrap()
        .unwrap();
    WindowRefreshContext {
        selection: commit.token(),
        window_id: 10,
        display,
    }
}

fn portable_mask(
    frozen: ScreenshotDocument,
    shape: ScreenshotDocument,
    context: WindowRefreshContext,
) -> ScreenshotDocument {
    let mut session = ScreenshotEditSession::new(frozen);
    let prepared = WindowRefreshPlan::new(
        &session,
        context,
        WindowRefreshDecision::MaskFrozenAlpha,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
    .prepare(shape)
    .unwrap()
    .unwrap();
    assert!(prepared
        .apply(&mut session, context, &AtomicBool::new(false))
        .unwrap());
    assert_eq!(session.revision(), 1);
    assert!(session.document().annotations().is_empty());
    assert!(session.document().crop_rect().is_none());
    session.render_snapshot()
}

fn compare_alpha(case: &str, expected: &ScreenshotDocument, actual: &ScreenshotDocument) {
    assert_eq!(
        actual.dimensions(),
        expected.dimensions(),
        "{case}: output size"
    );
    let expected = expected.render_rgba().unwrap();
    let actual = actual.render_rgba().unwrap();
    for (x, y, native) in expected.enumerate_pixels() {
        let portable = actual.get_pixel(x, y);
        assert_eq!(
            portable[3], native[3],
            "{case}: at ({x},{y}), native={:?}, portable={:?}",
            native.0, portable.0
        );
    }
}

#[test]
fn native_alpha_same_size_preserves_opaque_palette_and_asymmetric_shape() {
    worker(|| {
        let context = context(6, 4);
        let palette = [
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 0, 255, 255],
        ];
        let frozen = Fixture::new(6, 4, |x, y| palette[((x + 2 * y) % 6) as usize]);
        let shape = Fixture::new(6, 4, |x, y| {
            let alpha = [0, 1, 63, 128, 254, 255][((x + 3 * y) % 6) as usize];
            [0, 0, alpha, alpha]
        });
        let frozen_doc = frozen.document();
        let decoded_frozen = frozen_doc.render_rgba().unwrap();
        for (x, y, pixel) in decoded_frozen.enumerate_pixels() {
            assert_eq!(
                pixel.0,
                palette[((x + 2 * y) % 6) as usize],
                "input ({x},{y})"
            );
        }
        let shape_doc = shape.document();
        let reference = normalized_reference(&frozen_doc, &shape_doc);
        let native = native_mask(frozen_doc.clone(), shape_doc.clone(), context);
        compare_rgba("native same-size palette", &reference, &native);
        let actual = portable_mask(frozen_doc, shape_doc, context);
        compare_alpha("same-size palette", &reference, &actual);
        assert_eq!(
            actual.render_rgba().unwrap(),
            reference.render_rgba().unwrap()
        );
    });
}

#[test]
fn native_alpha_all_65536_same_size_alpha_pairs_match_exactly() {
    worker(|| {
        let context = context(256, 256);
        let frozen = Fixture::new(256, 256, |x, _| [x as u8; 4]);
        let shape = Fixture::new(256, 256, |_, y| [0, 0, 0, y as u8]);
        let frozen_doc = frozen.document();
        let shape_doc = shape.document();
        for (x, _, pixel) in frozen_doc.render_rgba().unwrap().enumerate_pixels() {
            assert_eq!(pixel[3], x as u8, "frozen PNG changed input alpha");
        }
        for (_, y, pixel) in shape_doc.render_rgba().unwrap().enumerate_pixels() {
            assert_eq!(pixel[3], y as u8, "shape PNG changed input alpha");
        }
        let reference = normalized_reference(&frozen_doc, &shape_doc);
        let native = native_mask(frozen_doc.clone(), shape_doc.clone(), context);
        compare_rgba("native all alpha pairs", &reference, &native);
        let actual = portable_mask(frozen_doc, shape_doc, context);
        compare_alpha("x=frozen alpha, y=shape alpha", &reference, &actual);
    });
}

#[test]
fn native_alpha_one_pixel_resize_matches_exact_sampling_on_both_axes() {
    worker(|| {
        let mut mismatches = Vec::new();
        for (width, height) in [
            (1_u32, 2_u32),
            (2, 1),
            (1, 3),
            (3, 1),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 4),
            (8, 7),
            (1, 255),
            (255, 1),
            (127, 128),
            (128, 127),
            (254, 255),
            (255, 254),
        ] {
            let context = context(width, height);
            for sw in width.saturating_sub(1).max(1)..=width + 1 {
                for sh in height.saturating_sub(1).max(1)..=height + 1 {
                    let frozen = Fixture::new(width, height, |x, y| {
                        if (x + 3 * y) % 2 == 0 {
                            [255, 0, 255, 255]
                        } else {
                            [0, 0, 0, 255]
                        }
                    });
                    let shape = Fixture::new(sw, sh, |x, y| {
                        let alpha = ((17 + 29 * x + 43 * y) % 256) as u8;
                        [0, alpha, 0, alpha]
                    });
                    let frozen = frozen.document();
                    let shape = shape.document();
                    let reference = normalized_reference(&frozen, &shape);
                    let actual = native_mask(frozen, shape, context);
                    assert_eq!(actual.dimensions(), reference.dimensions());
                    let expected = reference.render_rgba().unwrap();
                    let actual = actual.render_rgba().unwrap();
                    let differences: Vec<_> = expected
                        .enumerate_pixels()
                        .filter_map(|(x, y, native)| {
                            let backend = actual.get_pixel(x, y);
                            (native.0 != backend.0).then_some((x, y, native.0, backend.0))
                        })
                        .collect();
                    if !differences.is_empty() {
                        mismatches.push(format!("frozen={width}x{height}, shape={sw}x{sh}, count={}, first (x,y,oracle,backend)={:?}", differences.len(), &differences[..differences.len().min(16)]));
                    }
                }
            }
        }
        // Evaluate the whole bounded corpus before failing. This preserves exact
        // assertions and exposes axis/tie behavior instead of just the first pixel.
        assert!(
            mismatches.is_empty(),
            "exact native RGBA sampling mismatches (shape alpha=(17+29*x+43*y)%256):\n{}",
            mismatches.join("\n")
        );
    });
}

#[test]
fn native_alpha_incompatible_sizes_leave_the_original_document_unchanged() {
    worker(|| {
        let context = context(4, 4);
        let frozen = Fixture::new(4, 4, |_, _| [255, 0, 0, 255]);
        for (width, height) in [(2, 4), (6, 4), (4, 2), (4, 6)] {
            let shape = Fixture::new(width, height, |_, _| [0, 0, 0, 255]);
            assert!(swift_mask(&frozen.image, &shape.image).is_none());
            let session = ScreenshotEditSession::new(frozen.document());
            let before = session.document().render_rgba().unwrap();
            let plan = WindowRefreshPlan::new(
                &session,
                context,
                WindowRefreshDecision::MaskFrozenAlpha,
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            assert!(matches!(
                plan.prepare_with_alpha_mask(shape.document(), |_, _, _| panic!(
                    "size validation must precede native masking"
                )),
                Err(WindowRefreshError::IncompatibleImages)
            ));
            assert_eq!(session.revision(), 0);
            assert_eq!(session.document().render_rgba().unwrap(), before);
        }
    });
}

#[test]
fn native_alpha_rgb_corpus_characterizes_quantization_without_color_equivalence_claim() {
    worker(|| {
        let context = context(12, 12);
        let alphas = [1_u8, 2, 3, 7, 31, 63, 127, 128, 129, 191, 254, 255];
        let frozen = Fixture::new(12, 12, |x, y| {
            let alpha = alphas[x as usize];
            [
                alpha / 3,
                alpha / 2,
                alpha.saturating_sub(y as u8).min(alpha),
                alpha,
            ]
        });
        let shape = Fixture::new(12, 12, |_, y| [0, 0, 0, alphas[y as usize]]);
        let frozen_doc = frozen.document();
        let shape_doc = shape.document();
        let reference = normalized_reference(&frozen_doc, &shape_doc);
        let native = native_mask(frozen_doc.clone(), shape_doc.clone(), context);
        compare_rgba("native low-alpha RGB corpus", &reference, &native);
        let actual = portable_mask(frozen_doc.clone(), shape_doc, context);
        compare_alpha("RGB corpus", &reference, &actual);
        let input = frozen_doc.render_rgba().unwrap();
        let reference = reference.render_rgba().unwrap();
        let actual = actual.render_rgba().unwrap();
        let mut differing_rgb_pixels = 0;
        let mut differing_premultiplied_pixels = 0;
        let mut max_rgb_delta = 0;
        let mut max_premultiplied_delta = 0;
        for (x, y, native) in reference.enumerate_pixels() {
            let portable = actual.get_pixel(x, y);
            // A diagnostic projection only, not a native premultiplication oracle
            // or an error tolerance. Exact alpha remains asserted above.
            let project = |value: u8, alpha: u8| (u16::from(value) * u16::from(alpha) + 127) / 255;
            let rgb_delta = (0..3)
                .map(|c| native[c].abs_diff(portable[c]))
                .max()
                .unwrap();
            let premultiplied_delta = (0..3)
                .map(|c| project(native[c], native[3]).abs_diff(project(portable[c], portable[3])))
                .max()
                .unwrap();
            if rgb_delta != 0 {
                differing_rgb_pixels += 1;
                if differing_rgb_pixels <= 8 {
                    eprintln!("RGB diagnostic ({x},{y}): frozen={:?}, shape_alpha={}, native={:?}, portable={:?}, straight_delta={rgb_delta}, premultiplied_projection_delta={premultiplied_delta}",
                        input.get_pixel(x, y).0, alphas[y as usize], native.0, portable.0);
                }
            }
            differing_premultiplied_pixels += usize::from(premultiplied_delta != 0);
            max_rgb_delta = max_rgb_delta.max(rgb_delta);
            max_premultiplied_delta = max_premultiplied_delta.max(premultiplied_delta);
        }
        // Run with --show-output to retain this characterization on success.
        // No threshold is asserted or inferred, and no production tolerance or
        // color conversion is changed to accommodate the reported differences.
        eprintln!("RGB characterization (144 pixels): straight mismatches={differing_rgb_pixels}, max delta={max_rgb_delta}; integer-premultiplied projection mismatches={differing_premultiplied_pixels}, max delta={max_premultiplied_delta}. This does not establish ICC/HDR or general RGB equivalence.");
    });
}

#[test]
fn native_alpha_portable_resampling_remains_a_characterized_approximation() {
    worker(|| {
        let context = context(2, 3);
        let frozen = Fixture::new(2, 3, |_, _| [0, 0, 0, 255]).document();
        let shape = Fixture::new(1, 2, |_, y| [0, 0, 0, (17 + 43 * y) as u8]).document();
        let reference = normalized_reference(&frozen, &shape);
        let portable = portable_mask(frozen, shape, context);
        let native_alpha = reference.render_rgba().unwrap().get_pixel(0, 1)[3];
        let portable_alpha = portable.render_rgba().unwrap().get_pixel(0, 1)[3];
        // Original exact failure is retained, not silently relabeled equivalence.
        assert_eq!((native_alpha, portable_alpha), (17, 60));
        eprintln!("Known portable sampling difference: frozen2x3 shape1x2 pixel(0,1), CoreGraphics alpha={native_alpha}, portable={portable_alpha}. Native source-fidelity host uses the CoreGraphics adapter; Linux portable resampling remains approximate.");
    });
}
