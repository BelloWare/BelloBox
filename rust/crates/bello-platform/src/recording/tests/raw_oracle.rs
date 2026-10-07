use super::*;

// The raw RGB24 payload is lossless; the Swift-shaped AVAssetReader BGRA output
// plus DeviceRGB CoreGraphics render is not a byte-copy contract. On macOS CI,
// frame 0 differs only by one RGB code value (alpha remains exact). Keep that
// fixture-specific ceiling, not the much wider lossy H.264 writer tolerance.
pub(super) fn check_decoded_frame(
    expected: &fixture::KnownFrame,
    pts: f64,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    if !pts.is_finite() || (pts - expected.presentation_seconds).abs() >= 0.001 {
        return Err(format!(
            "PTS {pts}, expected {}",
            expected.presentation_seconds
        ));
    }
    if (width, height) != (expected.width, expected.height) {
        return Err(format!("dimensions {width}x{height}"));
    }
    if rgba.len() != expected.rgba.len() {
        return Err(format!(
            "RGBA length {}, expected {}",
            rgba.len(),
            expected.rgba.len()
        ));
    }
    for (offset, (&actual, &wanted)) in rgba.iter().zip(&expected.rgba).enumerate() {
        let allowance = u8::from(offset % 4 != 3);
        if actual.abs_diff(wanted) > allowance {
            let pixel = offset / 4;
            return Err(format!(
                "pixel ({}, {}), channel {}: {actual}, expected {wanted}, allowance {allowance}",
                pixel % width as usize,
                pixel / width as usize,
                offset % 4,
            ));
        }
    }
    Ok(())
}

// This is an invariant of this sealed fixture, not an arbitrary MOV parser.
// Its sole mdat begins at byte 28; 12 packed RGB24 samples follow its 8-byte header.
fn check_raw_payload(bytes: &[u8], known: &fixture::KnownFrames) -> Result<(), String> {
    let length = fixture::WIDTH as usize * fixture::HEIGHT as usize * 3 * fixture::FRAME_COUNT;
    if bytes.get(28..32) != Some(((length + 8) as u32).to_be_bytes().as_slice())
        || bytes.get(32..36) != Some(b"mdat")
    {
        return Err("raw fixture mdat header changed".into());
    }
    let payload = bytes.get(36..36 + length).ok_or("truncated raw fixture")?;
    let mut offset = 0;
    for index in 0..fixture::FRAME_COUNT {
        let frame = known.frame(index).unwrap();
        for pixel in frame.rgba.chunks_exact(4) {
            if pixel[3] != 255 || payload[offset..offset + 3] != pixel[..3] {
                return Err(format!(
                    "raw payload differs at frame {index}, byte {offset}"
                ));
            }
            offset += 3;
        }
    }
    Ok(())
}

#[test]
fn checked_in_raw_payload_is_exact_and_one_byte_mutation_is_detected() {
    let known = movie(true, false).known_frames().unwrap();
    let bytes = include_bytes!("../assets/known-rgb.mov");
    check_raw_payload(bytes, &known).unwrap();
    let payload_end =
        36 + fixture::WIDTH as usize * fixture::HEIGHT as usize * 3 * fixture::FRAME_COUNT;
    for offset in [36, payload_end - 1] {
        let mut corrupt = bytes.to_vec();
        corrupt[offset] -= 1;
        assert!(check_raw_payload(&corrupt, &known).is_err());
    }
    assert!(check_raw_payload(&bytes[..payload_end - 1], &known).is_err());
}

#[test]
fn decoded_raw_oracle_accepts_only_one_rgb_code_value_and_exact_alpha() {
    let known = movie(true, false).known_frames().unwrap();
    for index in 0..fixture::FRAME_COUNT {
        let expected = known.frame(index).unwrap();
        let check = |rgba: &[u8]| {
            check_decoded_frame(
                &expected,
                expected.presentation_seconds,
                expected.width,
                expected.height,
                rgba,
            )
        };
        check(&expected.rgba).unwrap();
        for delta in [-1i16, 1] {
            let mut rgba = expected.rgba.clone();
            for pixel in rgba.chunks_exact_mut(4) {
                for value in &mut pixel[..3] {
                    *value = (i16::from(*value) + delta).clamp(0, 255) as u8;
                }
            }
            check(&rgba).unwrap();
        }
        for channel in 0..4 {
            let mut rgba = expected.rgba.clone();
            let offset = rgba.len() - 4 + channel;
            rgba[offset] = if channel == 3 {
                254
            } else if rgba[offset] <= 253 {
                rgba[offset] + 2
            } else {
                rgba[offset] - 2
            };
            assert!(check(&rgba).is_err(), "frame {index}, channel {channel}");
        }
    }
}

#[test]
fn decoded_raw_oracle_rejects_channel_frame_row_length_dimension_and_time_corruption() {
    let known = movie(true, false).known_frames().unwrap();
    let expected = known.frame(0).unwrap();
    let check =
        |rgba: &[u8]| check_decoded_frame(&expected, 0., expected.width, expected.height, rgba);
    let mut swapped = expected.rgba.clone();
    for pixel in swapped.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    assert!(check(&swapped).is_err());
    for index in [1, fixture::FRAME_COUNT - 1] {
        assert!(check(&known.frame(index).unwrap().rgba).is_err());
    }
    let row = expected.width as usize * 4;
    let flipped: Vec<_> = expected
        .rgba
        .chunks_exact(row)
        .rev()
        .flatten()
        .copied()
        .collect();
    assert!(check(&flipped).is_err());
    assert!(check(&expected.rgba[..expected.rgba.len() - 1]).is_err());
    let mut extra = expected.rgba.clone();
    extra.push(255);
    assert!(check(&extra).is_err());
    for (width, height) in [(95, 64), (96, 65), (64, 96)] {
        assert!(check_decoded_frame(&expected, 0., width, height, &expected.rgba).is_err());
    }
    for pts in [0.1, f64::NAN, f64::INFINITY] {
        assert!(check_decoded_frame(&expected, pts, 96, 64, &expected.rgba).is_err());
    }
}

#[test]
fn serialization_lock_recovery_does_not_hide_the_original_panic() {
    let mutex = Mutex::new(());
    let failure = std::panic::catch_unwind(|| {
        let _serial = lock_serial(&mutex);
        panic!("intentional independent-test failure");
    });
    assert!(failure.is_err());
    assert!(mutex.is_poisoned());
    drop(lock_serial(&mutex));
}
