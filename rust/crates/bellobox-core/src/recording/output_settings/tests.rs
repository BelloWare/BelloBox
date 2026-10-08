use super::*;

pub(super) const PRESETS: [RecordingQualityPreset; 3] = [
    RecordingQualityPreset::Compact,
    RecordingQualityPreset::Balanced,
    RecordingQualityPreset::High,
];

#[test]
fn source_presets_preserve_dimensions_fps_bitrate_and_audio_metadata() {
    let expected = [
        (1600, 900, 24, 3_456_000),
        (2560, 1440, 30, 13_271_040),
        (3840, 2160, 30, 39_813_120),
    ];
    for (quality, (width, height, frames_per_second, video_bitrate)) in
        PRESETS.into_iter().zip(expected)
    {
        assert_eq!(
            RecordingOutputSettings::make(3840.0, 2160.0, quality).unwrap(),
            RecordingOutputSettings {
                width,
                height,
                frames_per_second,
                video_bitrate,
                audio_sample_rate: 48_000,
                audio_channel_count: 2,
            }
        );
        let portrait = RecordingOutputSettings::make(2160.0, 3840.0, quality).unwrap();
        assert_eq!((portrait.width, portrait.height), (height, width));
        assert_eq!(portrait.video_bitrate, video_bitrate);
    }
}

#[test]
fn smaller_sources_are_not_scaled_up_but_odd_dimensions_pad_upward() {
    for quality in PRESETS {
        let even = RecordingOutputSettings::make(640.0, 480.0, quality).unwrap();
        assert_eq!((even.width, even.height), (640, 480));
        let odd = RecordingOutputSettings::make(641.0, 479.0, quality).unwrap();
        assert_eq!((odd.width, odd.height), (642, 480));
    }
}

#[test]
fn fractional_ties_round_away_from_zero_before_even_padding() {
    for quality in PRESETS {
        for (input, expected) in [(2.49, 2), (2.5, 4), (3.49, 4), (4.5, 6)] {
            let result = RecordingOutputSettings::make(input, input, quality).unwrap();
            assert_eq!((result.width, result.height), (expected, expected));
        }
    }
}

#[test]
fn bitrate_is_truncated_after_floating_density_and_has_a_floor() {
    let fractional =
        RecordingOutputSettings::make(1713.0, 997.0, RecordingQualityPreset::High).unwrap();
    assert_eq!((fractional.width, fractional.height), (1714, 998));
    assert_eq!(fractional.video_bitrate, 8_210_745);
    for quality in PRESETS {
        assert_eq!(
            RecordingOutputSettings::make(2.0, 2.0, quality)
                .unwrap()
                .video_bitrate,
            1_000_000
        );
    }
}

#[test]
fn square_policy_does_not_silently_apply_the_writer_memory_limit() {
    let expected = [(1600, 6_144_000), (2560, 23_592_960), (3840, 70_778_880)];
    for (quality, (edge, bitrate)) in PRESETS.into_iter().zip(expected) {
        let result = RecordingOutputSettings::make(8192.0, 8192.0, quality).unwrap();
        assert_eq!((result.width, result.height), (edge, edge));
        assert_eq!(result.video_bitrate, bitrate);
    }
    // A valid pure High square plan exceeds today's separate 32 MiB RGBA gate.
    // Returning it is policy parity, not writer admission or capture permission.
    let high = RecordingOutputSettings::make(8192.0, 8192.0, RecordingQualityPreset::High).unwrap();
    assert!(u64::from(high.width) * u64::from(high.height) * 4 > 32 * 1024 * 1024);
}

#[test]
fn finite_zero_negative_and_tiny_values_keep_the_swift_minimum() {
    for value in [
        -9_223_372_036_854_775_808.0,
        -2.5,
        -0.0,
        0.0,
        f64::MIN_POSITIVE,
        1.49,
    ] {
        for quality in PRESETS {
            let result = RecordingOutputSettings::make(value, value, quality).unwrap();
            assert_eq!((result.width, result.height), (2, 2));
        }
    }
}

#[test]
fn invalid_dimensions_return_errors_instead_of_saturating_or_panicking() {
    let upper = 9_223_372_036_854_775_808.0_f64;
    for (invalid, error) in [
        (f64::NAN, OutputSettingsError::NonFiniteDimension),
        (f64::INFINITY, OutputSettingsError::NonFiniteDimension),
        (f64::NEG_INFINITY, OutputSettingsError::NonFiniteDimension),
        (upper, OutputSettingsError::DimensionOutOfRange),
        (
            -f64::from_bits(upper.to_bits() + 1),
            OutputSettingsError::DimensionOutOfRange,
        ),
        (f64::MAX, OutputSettingsError::DimensionOutOfRange),
        (f64::MIN, OutputSettingsError::DimensionOutOfRange),
    ] {
        for quality in PRESETS {
            assert_eq!(
                RecordingOutputSettings::make(invalid, 10.0, quality),
                Err(error)
            );
            assert_eq!(
                RecordingOutputSettings::make(10.0, invalid, quality),
                Err(error)
            );
        }
    }
    let largest = f64::from_bits(upper.to_bits() - 1);
    let result = RecordingOutputSettings::make(largest, 2.0, RecordingQualityPreset::High).unwrap();
    assert_eq!((result.width, result.height), (3840, 2));
}

pub(super) fn source_cases() -> Vec<(f64, f64)> {
    let edges = [
        -2.5, -0.0, 0.5, 1.49, 1.5, 2.49, 2.5, 3.49, 3.5, 4.5, 479.5, 640.5, 997.0, 1080.5,
        1599.49, 1599.5, 1600.5, 1713.0, 2559.5, 2560.5, 3839.5, 3840.5, 7680.5,
    ];
    let mut cases: Vec<_> = edges
        .into_iter()
        .flat_map(|x| edges.map(|y| (x, y)))
        .collect();
    let upper = 9_223_372_036_854_775_808.0_f64;
    let largest = f64::from_bits(upper.to_bits() - 1);
    cases.extend([
        (3840.0, 2160.0),
        (2160.0, 3840.0),
        (8192.0, 8192.0),
        (largest, 2.0),
        (2.0, largest),
        (largest, largest),
        (-upper, -upper),
        (f64::MIN_POSITIVE, f64::MIN_POSITIVE),
    ]);
    cases
}

#[test]
fn policy_outputs_stay_bounded_across_the_source_comparison_matrix() {
    for quality in PRESETS {
        for (width, height) in source_cases() {
            let result = RecordingOutputSettings::make(width, height, quality).unwrap();
            assert!((2..=3840).contains(&result.width));
            assert!((2..=3840).contains(&result.height));
            assert_eq!((result.width % 2, result.height % 2), (0, 0));
            assert!((1_000_000..=70_778_880).contains(&result.video_bitrate));
        }
    }
}
