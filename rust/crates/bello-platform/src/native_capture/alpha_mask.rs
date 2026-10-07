//! Local supplied-image masking, independent of capture availability and TCC.
use super::{CaptureCancellation, MAX_CAPTURE_DIMENSION};
use std::{fmt, time::Duration};

/// Combined tight RGBA byte length of both supplied inputs, before native copies.
/// This bounds known input buffers, not opaque CoreGraphics allocations or total
/// process memory. PNG output has its own existing bounded-encoder limit.
pub const MAX_ALPHA_MASK_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Late native results are rejected. Synchronous CoreGraphics/ImageIO execution
/// cannot be forcibly interrupted and may outlast this publication deadline.
pub const MAX_ALPHA_MASK_TIMEOUT: Duration = Duration::from_secs(10);

/// Borrowed, normalized SDR, straight-alpha RGBA8 base pixels, in tight row order.
/// No annotation/crop rendering, ICC/HDR conversion, or capture occurs here.
#[derive(Clone, Copy)]
pub struct AlphaMaskInput<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}
impl fmt::Debug for AlphaMaskInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlphaMaskInput")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgba_byte_count", &self.rgba.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMaskError {
    Cancelled,
    TimedOut,
    Unsupported,
    Busy,
    WorkerThreadRequired,
    InvalidImage,
    IncompatibleImages,
    InputTooLarge,
    NativeFailure,
    OutputTooLarge,
}
impl fmt::Display for AlphaMaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Image alpha masking was cancelled.",
            Self::TimedOut => "Image alpha masking timed out. No image was published.",
            Self::Unsupported => "Native image alpha masking requires macOS.",
            Self::Busy => "A previous native image mask is still finishing.",
            Self::WorkerThreadRequired => {
                "Native image masking and PNG encoding must run off the main thread."
            }
            Self::InvalidImage => "Image masking requires nonempty, tightly packed RGBA8 pixels.",
            Self::IncompatibleImages => {
                "The mask dimensions differ from the image by more than one pixel."
            }
            Self::InputTooLarge => {
                "The supplied image pair exceeds the bounded dimension or 64 MiB RGBA limit."
            }
            Self::NativeFailure => "CoreGraphics could not mask the supplied image.",
            Self::OutputTooLarge => "The masked PNG exceeds the bounded encoding limit.",
        })
    }
}
impl std::error::Error for AlphaMaskError {}

#[cfg(target_os = "macos")]
impl From<super::CaptureError> for AlphaMaskError {
    fn from(error: super::CaptureError) -> Self {
        use super::CaptureError;
        match error {
            CaptureError::Cancelled => Self::Cancelled,
            CaptureError::TimedOut => Self::TimedOut,
            CaptureError::Unsupported => Self::Unsupported,
            CaptureError::Busy => Self::Busy,
            CaptureError::WorkerThreadRequired => Self::WorkerThreadRequired,
            CaptureError::InvalidImage | CaptureError::InvalidRequest => Self::InvalidImage,
            CaptureError::OutputTooLarge => Self::OutputTooLarge,
            // Capture-specific errors cannot arise on this local-image path.
            // Never expose a misleading permission or display-reselection prompt.
            CaptureError::PermissionNotGranted
            | CaptureError::DisplayNotFound
            | CaptureError::DisplayChanged
            | CaptureError::NativeFailure => Self::NativeFailure,
        }
    }
}

fn validate_layout(width: u32, height: u32, byte_len: usize) -> Result<usize, AlphaMaskError> {
    if width == 0 || height == 0 {
        return Err(AlphaMaskError::InvalidImage);
    }
    if width > MAX_CAPTURE_DIMENSION || height > MAX_CAPTURE_DIMENSION {
        return Err(AlphaMaskError::InputTooLarge);
    }
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .and_then(|stride| usize::try_from(height).ok()?.checked_mul(stride))
        .ok_or(AlphaMaskError::InputTooLarge)?;
    if expected > MAX_ALPHA_MASK_INPUT_BYTES {
        return Err(AlphaMaskError::InputTooLarge);
    }
    if byte_len != expected {
        return Err(AlphaMaskError::InvalidImage);
    }
    Ok(expected)
}

fn validate_byte_budget(frozen: usize, shape: usize) -> Result<(), AlphaMaskError> {
    match frozen.checked_add(shape) {
        Some(total) if total <= MAX_ALPHA_MASK_INPUT_BYTES => Ok(()),
        _ => Err(AlphaMaskError::InputTooLarge),
    }
}

fn validate_inputs(
    frozen: AlphaMaskInput<'_>,
    shape: AlphaMaskInput<'_>,
) -> Result<(), AlphaMaskError> {
    let frozen_bytes = validate_layout(frozen.width, frozen.height, frozen.rgba.len())?;
    let shape_bytes = validate_layout(shape.width, shape.height, shape.rgba.len())?;
    validate_byte_budget(frozen_bytes, shape_bytes)?;
    if frozen.width.abs_diff(shape.width) > 1 || frozen.height.abs_diff(shape.height) > 1 {
        return Err(AlphaMaskError::IncompatibleImages);
    }
    Ok(())
}

/// Apply ImageAlphaMask.swift's CoreGraphics draw sequence to supplied base pixels
/// and return bounded PNG bytes with the frozen input's dimensions.
///
/// This synchronous action requires a worker thread on macOS. Input owners remain
/// borrowed for the call; copied native data, images, and the context are created
/// and released on that same thread. A separate single-mask admission lease stays
/// held until all native work and encoding end, including on cancellation/timeout.
/// The fixed deadline rejects late publication, not an opaque native call's runtime.
/// The caller must still fence its own session/cancellation before publication.
/// No screen/window enumeration, permission check, capture, file, or network I/O.
pub fn mask_image_alpha(
    frozen: AlphaMaskInput<'_>,
    shape: AlphaMaskInput<'_>,
    cancellation: CaptureCancellation,
) -> Result<Vec<u8>, AlphaMaskError> {
    validate_inputs(frozen, shape)?;
    if cancellation.is_cancelled() {
        return Err(AlphaMaskError::Cancelled);
    }
    #[cfg(target_os = "macos")]
    {
        super::macos::alpha_mask::mask_image_alpha(frozen, shape, cancellation)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(AlphaMaskError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_out_of_bounds_dimensions() {
        for (width, height) in [(0, 0), (0, 1), (1, 0)] {
            assert_eq!(
                validate_layout(width, height, 0),
                Err(AlphaMaskError::InvalidImage)
            );
        }
        for (width, height) in [(u32::MAX, 1), (1, u32::MAX), (32_769, 1)] {
            assert_eq!(
                validate_layout(width, height, 4),
                Err(AlphaMaskError::InputTooLarge)
            );
        }
        assert_eq!(
            validate_layout(32_768, 32_768, 0),
            Err(AlphaMaskError::InputTooLarge)
        );
    }

    #[test]
    fn requires_exact_tight_rgba_length() {
        assert_eq!(validate_layout(2, 3, 24), Ok(24));
        for len in [0, 23, 25, 32] {
            assert_eq!(
                validate_layout(2, 3, len),
                Err(AlphaMaskError::InvalidImage)
            );
        }
    }

    #[test]
    fn combined_byte_budget_is_inclusive_and_checked_without_allocation() {
        let half = MAX_ALPHA_MASK_INPUT_BYTES / 2;
        assert_eq!(validate_byte_budget(half, half), Ok(()));
        assert_eq!(
            validate_byte_budget(half, half + 4),
            Err(AlphaMaskError::InputTooLarge)
        );
        assert_eq!(
            validate_byte_budget(usize::MAX, 1),
            Err(AlphaMaskError::InputTooLarge)
        );
        assert_eq!(
            validate_layout(4096, 4096, MAX_ALPHA_MASK_INPUT_BYTES),
            Ok(MAX_ALPHA_MASK_INPUT_BYTES)
        );
    }

    #[test]
    fn documented_equal_size_limits_match_checked_byte_budget() {
        for (width, height, fits) in [
            (3840_u32, 2160_u32, true),
            (4096, 2048, true),
            (4096, 2160, false),
            (5120, 2880, false),
        ] {
            let bytes = width as usize * height as usize * 4;
            assert_eq!(validate_layout(width, height, bytes), Ok(bytes));
            assert_eq!(validate_byte_budget(bytes, bytes).is_ok(), fits);
        }
        let frozen = 4096 * 2048 * 4;
        let larger_shape = 4097 * 2048 * 4;
        assert_eq!(
            validate_byte_budget(frozen, larger_shape),
            Err(AlphaMaskError::InputTooLarge)
        );
    }

    #[test]
    fn accepts_only_same_size_or_one_pixel_per_axis() {
        let frozen_bytes = [0; 3 * 4 * 4];
        let frozen = AlphaMaskInput {
            width: 3,
            height: 4,
            rgba: &frozen_bytes,
        };
        for width in 2..=4 {
            for height in 3..=5 {
                let bytes = vec![0; width as usize * height as usize * 4];
                let shape = AlphaMaskInput {
                    width,
                    height,
                    rgba: &bytes,
                };
                assert_eq!(validate_inputs(frozen, shape), Ok(()));
            }
        }
        for (width, height) in [(1, 4), (5, 4), (3, 2), (3, 6)] {
            let bytes = vec![0; width as usize * height as usize * 4];
            let shape = AlphaMaskInput {
                width,
                height,
                rgba: &bytes,
            };
            assert_eq!(
                validate_inputs(frozen, shape),
                Err(AlphaMaskError::IncompatibleImages)
            );
        }
    }

    #[test]
    fn cancelled_request_never_reaches_native_work() {
        let input = AlphaMaskInput {
            width: 1,
            height: 1,
            rgba: &[1, 2, 3, 255],
        };
        let cancelled = CaptureCancellation::default();
        cancelled.cancel();
        assert_eq!(
            mask_image_alpha(input, input, cancelled),
            Err(AlphaMaskError::Cancelled)
        );
    }

    #[test]
    fn public_entry_rejects_invalid_pixels_and_size_before_native_work() {
        let input = AlphaMaskInput {
            width: 1,
            height: 1,
            rgba: &[1, 2, 3, 255],
        };
        for invalid in [
            AlphaMaskInput { width: 0, ..input },
            AlphaMaskInput {
                rgba: &[0; 3],
                ..input
            },
            AlphaMaskInput {
                rgba: &[0; 5],
                ..input
            },
        ] {
            assert_eq!(
                mask_image_alpha(input, invalid, CaptureCancellation::default()),
                Err(AlphaMaskError::InvalidImage)
            );
        }
        let incompatible = AlphaMaskInput {
            width: 3,
            height: 1,
            rgba: &[0; 12],
        };
        assert_eq!(
            mask_image_alpha(input, incompatible, CaptureCancellation::default()),
            Err(AlphaMaskError::IncompatibleImages)
        );
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn non_macos_is_explicitly_unsupported() {
        let input = AlphaMaskInput {
            width: 1,
            height: 1,
            rgba: &[1, 2, 3, 255],
        };
        assert_eq!(
            mask_image_alpha(input, input, CaptureCancellation::default()),
            Err(AlphaMaskError::Unsupported)
        );
    }
}
