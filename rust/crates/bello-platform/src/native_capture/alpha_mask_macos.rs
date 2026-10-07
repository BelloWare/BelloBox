//! All native owners stay on the calling worker. This module only transforms
//! supplied pixels and never enters ScreenCaptureKit or a permission API.
use super::{encode_png, pool, require_worker_thread, InflightGuard, Job};
use crate::native_capture::{
    AlphaMaskError, AlphaMaskInput, CaptureCancellation, CaptureError, CaptureResult,
    MAX_ALPHA_MASK_TIMEOUT,
};
use objc2_core_foundation::{CFData, CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapInfo, CGBlendMode,
    CGColorRenderingIntent, CGColorSpace, CGContext, CGDataProvider, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, CGInterpolationQuality,
};
use std::{ptr, sync::atomic::AtomicBool};

// Independent of capture admission. The synchronous caller keeps this lease
// until native owners and the encoder are gone, even if its deadline expires.
static MASK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

// Native oracle tests and admission tests share this test-only lock; production
// keeps nonblocking Busy semantics rather than serializing/queuing callers.
#[cfg(test)]
pub(super) static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Check both sides of every opaque allocation/draw/snapshot operation. On an
/// interruption the outer Job::wait resolves Cancelled versus TimedOut, including
/// if a native failure races that interruption. No framework call is preempted.
fn native_step<T>(
    job: &Job<Vec<u8>>,
    operation: impl FnOnce() -> CaptureResult<T>,
) -> CaptureResult<T> {
    if !job.active() {
        return Err(CaptureError::Cancelled);
    }
    let result = operation();
    if !job.active() {
        return Err(CaptureError::Cancelled);
    }
    result
}

// Explicit field order releases the image before its retained backing owners.
// CFData copies the input; no borrowed Rust storage or release callback escapes.
struct NativeInput {
    image: CFRetained<CGImage>,
    _provider: CFRetained<CGDataProvider>,
    _data: CFRetained<CFData>,
    _color: CFRetained<CGColorSpace>,
}
impl NativeInput {
    fn new(input: AlphaMaskInput<'_>, job: &Job<Vec<u8>>) -> CaptureResult<Self> {
        let byte_len = isize::try_from(input.rgba.len()).map_err(|_| CaptureError::InvalidImage)?;
        // CFDataCreate copies the readable slice, with the default allocator.
        // Use its fallible primitive rather than from_bytes' allocation expect.
        let data = native_step(job, || unsafe {
            CFData::new(None, input.rgba.as_ptr(), byte_len).ok_or(CaptureError::NativeFailure)
        })?;
        let provider = native_step(job, || {
            CGDataProvider::with_cf_data(Some(&data)).ok_or(CaptureError::NativeFailure)
        })?;
        let color = native_step(job, || {
            CGColorSpace::new_device_rgb().ok_or(CaptureError::NativeFailure)
        })?;
        // Straight RGBA8 input, explicitly byte-ordered. The Swift-shaped output
        // context below instead uses only PremultipliedLast, with automatic stride.
        let info = CGBitmapInfo::from_bits_retain(
            CGImageByteOrderInfo::Order32Big.0 | CGImageAlphaInfo::Last.0,
        );
        let image = native_step(job, || unsafe {
            CGImage::new(
                input.width as usize,
                input.height as usize,
                8,
                32,
                input.width as usize * 4,
                Some(&color),
                info,
                Some(&provider),
                ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
            .ok_or(CaptureError::NativeFailure)
        })?;
        Ok(Self {
            image,
            _provider: provider,
            _data: data,
            _color: color,
        })
    }
}

pub(in crate::native_capture) fn mask_image_alpha(
    frozen: AlphaMaskInput<'_>,
    shape: AlphaMaskInput<'_>,
    cancellation: CaptureCancellation,
) -> Result<Vec<u8>, AlphaMaskError> {
    // Declare the lease outside the work closure so it outlives every native
    // object, all PNG callbacks, final deadline checks, and unwinding cleanup.
    let _lease = InflightGuard::acquire(&MASK_IN_FLIGHT)?;
    let job = Job::new(cancellation, MAX_ALPHA_MASK_TIMEOUT);
    let result = (|| {
        native_step(&job, require_worker_thread)?;
        let _pool = native_step(&job, || unsafe { pool() })?;
        let frozen_image = NativeInput::new(frozen, &job)?;
        let shape_image = NativeInput::new(shape, &job)?;
        let color = native_step(&job, || {
            CGColorSpace::new_device_rgb().ok_or(CaptureError::NativeFailure)
        })?;
        let context = native_step(&job, || unsafe {
            CGBitmapContextCreate(
                ptr::null_mut(),
                frozen.width as usize,
                frozen.height as usize,
                8,
                0,
                Some(&color),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
            .ok_or(CaptureError::NativeFailure)
        })?;
        let rect = CGRect {
            origin: CGPoint { x: 0., y: 0. },
            size: CGSize {
                width: f64::from(frozen.width),
                height: f64::from(frozen.height),
            },
        };
        let ctx = Some(&*context);
        native_step(&job, || {
            CGContext::set_interpolation_quality(ctx, CGInterpolationQuality::None);
            Ok(())
        })?;
        native_step(&job, || {
            CGContext::draw_image(ctx, rect, Some(&frozen_image.image));
            Ok(())
        })?;
        native_step(&job, || {
            CGContext::set_blend_mode(ctx, CGBlendMode::DestinationIn);
            Ok(())
        })?;
        native_step(&job, || {
            CGContext::draw_image(ctx, rect, Some(&shape_image.image));
            Ok(())
        })?;
        let masked = native_step(&job, || {
            CGBitmapContextCreateImage(ctx).ok_or(CaptureError::NativeFailure)
        })?;
        // The immutable snapshot and its input/context owners remain on this
        // thread for the complete synchronous, size-bounded ImageIO operation.
        native_step(&job, || unsafe {
            encode_png((&*masked as *const CGImage).cast_mut().cast(), job.clone())
        })
    })();
    job.complete(result);
    // This immediate completion/deadline resolution occurs after local native
    // cleanup. An expired job cannot expose even successfully encoded PNG bytes.
    job.wait().map_err(AlphaMaskError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};

    #[test]
    fn native_alpha_step_rejects_expired_work_without_calling_operation() {
        let job = Job::new(CaptureCancellation::default(), Duration::ZERO);
        let called = Cell::new(false);
        let result = native_step(&job, || {
            called.set(true);
            Ok(vec![1])
        });
        assert!(!called.get());
        assert_eq!(result, Err(CaptureError::Cancelled));
        // The step only gates work; final Job resolution preserves timeout's
        // distinct local-image error even though the generic PNG gate cancels.
        assert!(!job.complete(result));
        assert_eq!(
            job.wait().map_err(AlphaMaskError::from),
            Err(AlphaMaskError::TimedOut)
        );
    }

    #[test]
    fn native_alpha_step_discards_returned_owner_when_operation_cancels() {
        struct ReturnedOwner<'a>(&'a Cell<bool>);
        impl Drop for ReturnedOwner<'_> {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let cancellation = CaptureCancellation::default();
        let job = Job::new(cancellation.clone(), MAX_ALPHA_MASK_TIMEOUT);
        let dropped = Cell::new(false);
        let result = native_step(&job, || {
            cancellation.cancel();
            Ok(ReturnedOwner(&dropped))
        });
        assert!(matches!(&result, Err(CaptureError::Cancelled)));
        assert!(
            dropped.get(),
            "post-operation cancellation must drop the returned owner"
        );
        assert!(!job.complete(result.map(|_| vec![1])));
        assert_eq!(
            job.wait().map_err(AlphaMaskError::from),
            Err(AlphaMaskError::Cancelled)
        );
    }

    fn assert_public_mask_busy() {
        let input = AlphaMaskInput {
            width: 1,
            height: 1,
            rgba: &[1, 2, 3, 255],
        };
        // Valid supplied bytes reach actual mask admission, which rejects before
        // any CoreGraphics or encoding call. No test rendering is needed here.
        assert_eq!(
            crate::native_capture::mask_image_alpha(input, input, CaptureCancellation::default()),
            Err(AlphaMaskError::Busy)
        );
    }

    #[test]
    fn native_alpha_mask_admission_survives_cancellation_timeout_and_owner_cleanup() {
        struct PendingOwner<'a>(&'a Cell<bool>);
        impl Drop for PendingOwner<'_> {
            fn drop(&mut self) {
                assert_public_mask_busy();
                self.0.set(true);
            }
        }
        let _serial = TEST_SERIAL
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for timeout in [false, true] {
            let lease = InflightGuard::acquire(&MASK_IN_FLIGHT).unwrap();
            let cancellation = CaptureCancellation::default();
            let job = Job::new(
                cancellation.clone(),
                if timeout {
                    Duration::ZERO
                } else {
                    MAX_ALPHA_MASK_TIMEOUT
                },
            );
            // A Rust drop sentinel models an outstanding opaque owner. Even Job
            // interruption resolution must not reopen the actual admission slot
            // while that owner remains. Production drops owners before its final
            // Job resolution and keeps the independent lease through both.
            let dropped = Cell::new(false);
            let owner = PendingOwner(&dropped);
            if !timeout {
                cancellation.cancel();
            }
            assert_public_mask_busy();
            let result = native_step(&job, || panic!("interrupted work must not start"));
            assert!(!job.complete(result));
            assert_eq!(
                job.wait().map_err(AlphaMaskError::from),
                Err(if timeout {
                    AlphaMaskError::TimedOut
                } else {
                    AlphaMaskError::Cancelled
                })
            );
            assert_public_mask_busy();
            drop(owner);
            assert!(dropped.get());
            assert_public_mask_busy();
            drop(lease);
            let next = InflightGuard::acquire(&MASK_IN_FLIGHT)
                .expect("native mask slot must reopen only after owner cleanup and lease release");
            drop(next);
        }
    }
}
