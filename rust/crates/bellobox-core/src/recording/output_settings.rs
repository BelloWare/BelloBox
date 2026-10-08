//! Pure output policy from `RecordingModels.swift::RecordingOutputSettings.make`.
//! These values do not authorize recording or bypass the writer's separate limits.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingQualityPreset {
    Compact,
    Balanced,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordingOutputSettings {
    pub width: u32,
    pub height: u32,
    pub frames_per_second: u32,
    pub video_bitrate: u32,
    pub audio_sample_rate: u32,
    pub audio_channel_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputSettingsError {
    NonFiniteDimension,
    DimensionOutOfRange,
}

impl fmt::Display for OutputSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFiniteDimension => "Recording dimensions must be finite.",
            Self::DimensionOutOfRange => {
                "Rounded recording dimensions must fit a signed 64-bit integer."
            }
        })
    }
}
impl std::error::Error for OutputSettingsError {}

impl RecordingOutputSettings {
    /// Match the 64-bit Swift policy, including ties rounded away from zero,
    /// minimum two-pixel source dimensions and upward padding of odd outputs.
    /// There is no proportional upscaling. Audio fields are metadata only.
    ///
    /// Swift traps when converting non-finite/out-of-range rounded inputs to
    /// Int. This API returns an error instead. Finite negative/zero inputs in
    /// range retain Swift's two-pixel clamp rather than a new rejection rule.
    pub fn make(
        target_width: f64,
        target_height: f64,
        quality: RecordingQualityPreset,
    ) -> Result<Self, OutputSettingsError> {
        let source_width = source_dimension(target_width)?;
        let source_height = source_dimension(target_height)?;
        let source_long_edge = source_width.max(source_height);
        let (max_long_edge, frames_per_second, bits_per_pixel_per_frame) = match quality {
            RecordingQualityPreset::Compact => (1600, 24, 0.10),
            RecordingQualityPreset::Balanced => (2560, 30, 0.12),
            RecordingQualityPreset::High => (3840, 30, 0.16),
        };
        let scale = if source_long_edge > max_long_edge {
            max_long_edge as f64 / source_long_edge as f64
        } else {
            1.0
        };
        // Scaling bounds both axes by the largest preset edge (3,840). Casts,
        // even padding and the integer product below cannot overflow u32.
        let width = even(((source_width as f64 * scale).round() as u32).max(2));
        let height = even(((source_height as f64 * scale).round() as u32).max(2));
        // Preserve Swift's floating multiplication followed by truncation;
        // replacing the density with rational integer arithmetic can differ.
        let video_bitrate =
            (f64::from(width * height * frames_per_second) * bits_per_pixel_per_frame) as u32;
        Ok(Self {
            width,
            height,
            frames_per_second,
            video_bitrate: video_bitrate.max(1_000_000),
            audio_sample_rate: 48_000,
            audio_channel_count: 2,
        })
    }
}

fn source_dimension(value: f64) -> Result<i64, OutputSettingsError> {
    if !value.is_finite() {
        return Err(OutputSettingsError::NonFiniteDimension);
    }
    let rounded = value.round();
    // i64::MAX rounds UP to 2^63 as f64, so the upper bound must be exclusive.
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&rounded) {
        return Err(OutputSettingsError::DimensionOutOfRange);
    }
    Ok((rounded as i64).max(2))
}

fn even(value: u32) -> u32 {
    value + value % 2
}

#[cfg(all(test, target_os = "macos"))]
#[path = "output_settings/swift_oracle.rs"]
mod swift_oracle;
#[cfg(test)]
#[path = "output_settings/tests.rs"]
mod tests;
