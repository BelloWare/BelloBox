use super::{gif_decode::Decoder, *};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    cell::Cell,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
};

// Unchanged generated mirrored Once diagnostic from the native assessment.
// The source MOV/pixels never came from user media. This is the exact 794 bytes,
// not regenerated or padded to make the strict decoder succeed.
const MIRRORED: &str = concat!(
    "R0lGODlhQAAwAIAAAAAAAAAAACH5BAQKAAAALAAAAABAADAAgwIA8QMA8wMA9QQA9QQA9gUA9ugA",
    "AukAAukAA+sAA+43Be84BgAAAAAAAAAAAAAAAAT/cMlJq704V8W791oojtRndmSqbqe5vmnrwrQm",
    "f3V+3aDuSzzUzxfkDI7IpHJQaDqf0MJhSq1WEwmEdsvtIpZgZHTstJqpWK96Gw6Tyedzeq1ug9/j",
    "uHlO79qXeFF6Vnx9bH9JgVCDV1mGXIiJimWMU4WPkZlKlZaOj1oCoaKjpAIBp6ipqgGcjVivsLCl",
    "s6KrtqitaLG7r7S0AMDBwsMAucYHvrPEy8HHucmlzMzOrdCk0svUnNaj2MTaldyi3sPgjOKh5MLm",
    "g+gC6s3sce7wwPLz6PXF92b09fz98v0DWMUfPIIFBR5EOMWdO2MGIkqcSNGAQ3QQK2qMeFFcxo0V",
    "Qjty+whyokhrJEtyPJkspUqWLXOppAiTFgGXJWvOuilzpkSds3CCBFpK6EaipIxqRDpKaUimoZzS",
    "hCpAqkmqViVGAAAh+QQEBQAAACwAAAAAQAAwAIMCAPEDAPMDAPUEAPboAALpAALpAAPqAAXrAAPy",
    "iAXziQb0iggAAAAAAAAAAAAAAAAE/3DJSau9OEvFk/8gqI1kuXBKqH5m627dqr40icpzrV83Lu7A",
    "Sc/nCRqHRGMQ6RM4n9CoYECtWq8DgnbL5Rq+h7B4TJaan9h0tcvefg3k+Ph8Vqvb7bd8Tzfb03hs",
    "entxfVJ/WIFdg4RzhlCIV4peYI2Oj06RVpNulZZhmKFRnFpvpqeoogIBrK2urwEFsrO0tbIIuLm6",
    "u6qwvq22wbS7xLqqAMjJyssAws7PtcfM08jQ1s7S1MzX3NGi2tPd4gXZ4Mnj3eXmzejX6ubt7t/r",
    "5/HQ7+D29/P07PrC+LT9w8aP3kCABdcdDBaQ2kJbqlSJI7UloqiJFAlYDIWR4kZMHTpJfXwUktNI",
    "QyUnneyTUtHKOi0DvfQTE89MMzXb3JSSk83OKD27/IQSlMvQJ0UrHhWQVMtSpt0yEogAADs=",
);
fn fixture() -> Vec<u8> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(MIRRORED)
        .unwrap();
    assert_eq!(bytes.len(), 794);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "6629b6303725b54e46052c802b3d1c430e1fd665b6194920946a5971f2063d35"
    );
    bytes
}
fn decode(bytes: &[u8]) -> Result<Vec<(u16, Vec<u8>)>, GifError> {
    let mut reader = Decoder::new(Cursor::new(bytes))?;
    let mut result = Vec::new();
    while let Some(frame) = reader.next_frame(|| Ok(()))? {
        result.push((frame.delay, frame.buffer.into_owned()));
    }
    Ok(result)
}
fn fixture_plan() -> GifExportPlan {
    GifExportPlan::make(
        GifSourceInfo {
            duration: 0.4,
            display_width: 64.,
            display_height: 48.,
            nominal_frame_rate: 10.,
        },
        &GifExportOptions {
            frames_per_second: 10,
            max_width: 320,
            loops: false,
            trim_start: 0.1,
            trim_end: Some(0.251),
        },
    )
    .unwrap()
}
fn validate(bytes: &[u8], plan: &GifExportPlan) -> Result<(), GifError> {
    let mut file = tempfile::tempfile().unwrap();
    use std::io::Write;
    file.write_all(bytes).unwrap();
    super::gif_structure::validate(&mut file, plan, &ExportControl::default())
}
#[test]
fn unchanged_mirrored_payload_reproduces_legacy_eoi_failure_and_strict_drain_preserves_pixels() {
    let bytes = fixture();
    let mut options = ::gif::DecodeOptions::new();
    options.set_color_output(::gif::ColorOutput::RGBA);
    options.check_frame_consistency(true);
    options.check_lzw_end_code(true);
    let mut legacy = options.read_info(Cursor::new(&bytes)).unwrap();
    let expected: Vec<_> = (0..2)
        .map(|_| {
            let frame = legacy.read_next_frame().unwrap().unwrap();
            (frame.delay, frame.buffer.to_vec())
        })
        .collect();
    assert!(matches!(
        legacy.read_next_frame(),
        Err(::gif::DecodingError::EndCodeNotFound)
    ));
    let actual = decode(&bytes).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.iter().map(|f| f.0).collect::<Vec<_>>(), [10, 5]);
    assert!(actual.iter().all(|f| f.1.len() == 64 * 48 * 4));
    // Independently dictionary-decoded by dot's Python fixture oracle, not
    // expected values derived from this adapter or the gif crate's decoder.
    assert_eq!(
        format!("{:x}", Sha256::digest(&actual[0].1)),
        "ff8565eb548b7addfec7a86e9620afb730975d4e38859011d1f1efd72a9e5655"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&actual[1].1)),
        "8b6e6610ec5d5eefdcde5f4e539fdaba94eed64319ffb4bbb4278c5f8763f64d"
    );
}
#[test]
fn exact_fixture_passes_full_readback_and_owned_preview_rewind() {
    let bytes = fixture();
    validate(&bytes, &fixture_plan()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mirrored.gif");
    fs::write(&path, &bytes).unwrap();
    let result = GifExportResult {
        path: path.clone(),
        frame_count: 2,
        size: (64, 48),
        duration: 0.15,
        file_size: bytes.len() as u64,
    };
    let expected = decode(&bytes).unwrap();
    let mut preview = GifPreview::open(fs::File::open(path).unwrap(), &result).unwrap();
    for _ in 0..2 {
        for (delay, pixels) in &expected {
            let frame = preview.next_frame().unwrap().unwrap();
            assert_eq!(frame.delay_centiseconds, *delay);
            assert_eq!(
                image::load_from_memory(&frame.png)
                    .unwrap()
                    .to_rgba8()
                    .as_raw(),
                pixels
            );
        }
        assert!(preview.next_frame().unwrap().is_none());
        preview = preview.rewind().unwrap();
    }
}
// A tiny, full-canvas, opaque GIF whose LZW codes are supplied explicitly.
// min code size2: literals0..3, clear4, EOI5; these short streams stay at3 bits.
fn tiny(codes: &[u8]) -> Vec<u8> {
    let mut bytes = b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff".to_vec();
    bytes.extend_from_slice(&[0x21, 0xf9, 4, 4, 10, 0, 0, 0]);
    bytes.extend_from_slice(&[0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2]);
    let mut payload = vec![0; (codes.len() * 3).div_ceil(8)];
    for (i, code) in codes.iter().copied().enumerate() {
        assert!(code < 8);
        for bit in 0..3 {
            payload[(i * 3 + bit) / 8] |= ((code >> bit) & 1) << ((i * 3 + bit) % 8);
        }
    }
    bytes.push(payload.len() as u8);
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&[0, 0x3b]);
    bytes
}
#[test]
fn strict_eoi_is_required_even_after_all_expected_pixels_have_arrived() {
    assert_eq!(decode(&tiny(&[4, 0, 5])).unwrap()[0].1, [0, 0, 0, 255]);
    // A clear after the final pixel is legal; it is not another pixel or EOI.
    assert!(decode(&tiny(&[4, 0, 4, 5])).is_ok());
    for codes in [&[4, 0][..], &[4, 0, 4], &[4, 5], &[4, 7, 5]] {
        assert!(decode(&tiny(codes)).is_err(), "accepted codes {codes:?}");
    }
}
#[test]
fn additional_pixels_and_invalid_palette_references_are_rejected() {
    // Each stream has a real EOI, but emits two pixels for a one-pixel frame.
    for codes in [&[4, 0, 1, 5][..], &[4, 0, 6, 5]] {
        assert!(
            decode(&tiny(codes)).is_err(),
            "accepted extra output {codes:?}"
        );
    }
    // Literal2 is legal LZW but outside the two-entry global palette.
    assert!(decode(&tiny(&[4, 2, 5])).is_err());
}
#[test]
fn truncated_frames_and_missing_trailer_never_become_clean_eof() {
    let good = tiny(&[4, 0, 5]);
    for end in 0..good.len() {
        assert!(
            decode(&good[..end]).is_err(),
            "accepted prefix length {end}"
        );
    }
    // Keep container terminator/trailer but remove the byte holding EOI.
    let mut no_eoi = good.clone();
    let size = 38;
    assert_eq!(no_eoi[size], 2);
    no_eoi[size] = 1;
    no_eoi.remove(size + 2);
    assert!(decode(&no_eoi).is_err());
}
#[test]
fn full_readback_still_rejects_trailing_bytes_frame_count_timing_and_loop_changes() {
    let good = fixture();
    let plan = fixture_plan();
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(validate(&trailing, &plan).is_err());
    assert!(validate(&good[..good.len() - 1], &plan).is_err());
    let mut timing = good.clone();
    let gce = timing
        .windows(3)
        .position(|b| b == [0x21, 0xf9, 4])
        .unwrap();
    timing[gce + 4] = 9;
    assert!(validate(&timing, &plan).is_err());
    let mut loops = GifExportOptions {
        frames_per_second: 10,
        max_width: 320,
        loops: true,
        trim_start: 0.1,
        trim_end: Some(0.251),
    };
    let info = GifSourceInfo {
        duration: 0.4,
        display_width: 64.,
        display_height: 48.,
        nominal_frame_rate: 10.,
    };
    assert!(validate(&good, &GifExportPlan::make(info, &loops).unwrap()).is_err());
    loops.loops = false;
    loops.trim_end = Some(0.4);
    assert!(validate(&good, &GifExportPlan::make(info, &loops).unwrap()).is_err());
}
#[test]
fn bounds_and_failure_are_checked_before_later_frames_can_be_admitted() {
    let mut oversized = tiny(&[4, 0, 5]);
    oversized[6..8].copy_from_slice(&1081_u16.to_le_bytes());
    assert!(Decoder::new(Cursor::new(&oversized)).is_err());
    let mut bad = Decoder::new(Cursor::new(tiny(&[4, 0, 1, 5]))).unwrap();
    assert!(bad.next_frame(|| Ok(())).is_err());
    assert!(bad.next_frame(|| Ok(())).is_err(), "failure must be sticky");
}
#[test]
fn cancellation_during_decoding_is_preserved_and_cannot_resume() {
    let calls = Cell::new(0);
    let mut decoder = Decoder::new(Cursor::new(fixture())).unwrap();
    let result = decoder.next_frame(|| {
        calls.set(calls.get() + 1);
        if calls.get() >= 3 {
            Err(GifError::Cancelled)
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(GifError::Cancelled)));
    assert!(decoder.next_frame(|| Ok(())).is_err());
}
#[test]
fn dictionary_growth_decodes_without_losing_pixels() {
    let width = 257_u16;
    let height = 63_u16;
    let mut state = 17_u32;
    let indices: Vec<_> = (0..usize::from(width) * usize::from(height))
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    let palette: Vec<_> = (0..=255_u8)
        .flat_map(|v| [v, v.wrapping_mul(7), 255 - v])
        .collect();
    let mut bytes = Vec::new();
    let mut writer = ::gif::Encoder::new(&mut bytes, width, height, &palette).unwrap();
    writer
        .write_frame(&::gif::Frame {
            width,
            height,
            delay: 10,
            buffer: Cow::Borrowed(&indices),
            ..Default::default()
        })
        .unwrap();
    writer.into_inner().unwrap();
    let frames = decode(&bytes).unwrap();
    let expected: Vec<_> = indices
        .iter()
        .flat_map(|&v| [v, v.wrapping_mul(7), 255 - v, 255])
        .collect();
    assert_eq!(frames[0].1, expected);
}

fn preview_all(bytes: &[u8], plan: &GifExportPlan) -> Result<Vec<(u16, Vec<u8>)>, GifError> {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    file.rewind().unwrap();
    let mut preview = GifPreview::open(
        file,
        &GifExportResult {
            path: Default::default(),
            frame_count: plan.frame_count(),
            size: plan.output_size(),
            duration: plan.encoded_duration(),
            file_size: bytes.len() as u64,
        },
    )?;
    let mut decoded = Vec::new();
    while let Some(frame) = preview.next_frame()? {
        decoded.push((
            frame.delay_centiseconds,
            image::load_from_memory(&frame.png)
                .unwrap()
                .to_rgba8()
                .into_raw(),
        ));
    }
    Ok(decoded)
}
fn two_tiny(frame: &[u8], index: usize) -> Vec<u8> {
    let good = tiny(&[4, 0, 5]);
    let mut result = good[..19].to_vec();
    for i in 0..2 {
        let input = if index == i { frame } else { &good };
        result.extend_from_slice(&input[19..input.len() - 1]);
    }
    result.push(0x3b);
    result
}
fn tiny_plan() -> GifExportPlan {
    GifExportPlan::make(
        GifSourceInfo {
            duration: 0.2,
            display_width: 1.,
            display_height: 1.,
            nominal_frame_rate: 10.,
        },
        &GifExportOptions {
            frames_per_second: 10,
            loops: false,
            ..Default::default()
        },
    )
    .unwrap()
}
#[test]
fn export_and_preview_reject_first_and_last_frame_completion_failures() {
    let plan = tiny_plan();
    let good = tiny(&[4, 0, 5]);
    let mut cases = vec![
        ("missing EOI after exact pixels", tiny(&[4, 0])),
        ("early EOI", tiny(&[4, 5])),
        ("extra pixel before EOI", tiny(&[4, 0, 1, 5])),
        ("invalid dictionary code", tiny(&[4, 7, 5])),
        ("invalid palette index", tiny(&[4, 2, 5])),
    ];
    let mut no_terminator = good.clone();
    no_terminator.remove(no_terminator.len() - 2);
    cases.push(("missing subblock terminator", no_terminator));
    let mut bad_delay = good.clone();
    bad_delay[23] = 0;
    cases.push(("zero delay", bad_delay));
    let mut bad_bounds = good.clone();
    bad_bounds[28] = 1;
    cases.push(("nonzero left offset", bad_bounds));
    for index in 0..2 {
        let valid = two_tiny(&good, index);
        validate(&valid, &plan).unwrap();
        assert_eq!(preview_all(&valid, &plan).unwrap(), decode(&valid).unwrap());
        for (name, frame) in &cases {
            let bytes = two_tiny(frame, index);
            assert!(
                validate(&bytes, &plan).is_err(),
                "export accepted {name}, frame{index}"
            );
            assert!(
                preview_all(&bytes, &plan).is_err(),
                "preview accepted {name}, frame{index}"
            );
        }
    }
}
#[test]
fn export_and_preview_require_trailer_and_exact_eof() {
    let plan = fixture_plan();
    let good = fixture();
    for suffix in [&[0][..], &[0x3b], &good[..]] {
        let mut bytes = good.clone();
        bytes.extend_from_slice(suffix);
        assert!(validate(&bytes, &plan).is_err());
        assert!(preview_all(&bytes, &plan).is_err());
    }
    assert!(preview_all(&good[..good.len() - 1], &plan).is_err());
}

struct Fragmented<R> {
    inner: R,
    maximum: usize,
}
impl<R: Read> Read for Fragmented<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = buffer.len().min(self.maximum);
        self.inner.read(&mut buffer[..length])
    }
}
// Test-only structural walk: preserve all payload bytes, but put every byte of
// image data in its own subblock. EOI then straddles real subblock boundaries.
fn reblock_images(bytes: &[u8]) -> Vec<u8> {
    let mut position = 13;
    if bytes[10] & 128 != 0 {
        position += 3 << (1 + (bytes[10] & 7));
    }
    let mut result = bytes[..position].to_vec();
    loop {
        let start = position;
        match bytes[position] {
            0x3b => {
                result.extend_from_slice(&bytes[position..]);
                break;
            }
            0x21 => {
                position += 2;
                while bytes[position] != 0 {
                    position += 1 + usize::from(bytes[position]);
                }
                position += 1;
                result.extend_from_slice(&bytes[start..position]);
            }
            0x2c => {
                position += 10;
                if bytes[start + 9] & 128 != 0 {
                    position += 3 << (1 + (bytes[start + 9] & 7));
                }
                position += 1; // LZW minimum code size
                result.extend_from_slice(&bytes[start..position]);
                while bytes[position] != 0 {
                    let count = usize::from(bytes[position]);
                    position += 1;
                    for &byte in &bytes[position..position + count] {
                        result.extend_from_slice(&[1, byte]);
                    }
                    position += count;
                }
                result.push(0);
                position += 1;
            }
            tag => panic!("unexpected fixture block {tag}"),
        }
    }
    result
}
#[test]
fn legal_reblocking_and_fragmented_reads_preserve_eoi_pixels_and_timing() {
    let good = fixture();
    let reblocked = reblock_images(&good);
    let expected = decode(&good).unwrap();
    for bytes in [&good, &reblocked] {
        validate(bytes, &fixture_plan()).unwrap();
        assert_eq!(preview_all(bytes, &fixture_plan()).unwrap(), expected);
        for maximum in [1, 2, 3, 7, 31, 255] {
            let mut decoder = Decoder::new(Fragmented {
                inner: Cursor::new(bytes),
                maximum,
            })
            .unwrap();
            let mut actual = Vec::new();
            while let Some(frame) = decoder.next_frame(|| Ok(())).unwrap() {
                actual.push((frame.delay, frame.buffer.into_owned()));
            }
            assert_eq!(actual, expected);
            assert!(decoder.next_frame(|| Ok(())).unwrap().is_none());
        }
    }
}

// Pass1 is container inspection; pass2 is strict pixel readback. Limit reads to
// one byte so a failure at the subblock terminator cannot be hidden by read-ahead.
struct CompletionFailure<'a, R> {
    inner: R,
    position: u64,
    pass: usize,
    fail_at: u64,
    control: &'a ExportControl,
    cancel: bool,
    fired: &'a Cell<bool>,
}
impl<R: Read> Read for CompletionFailure<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.pass == 2 && self.position == self.fail_at {
            self.fired.set(true);
            if self.cancel {
                self.control.cancel().unwrap();
            }
            return Err(io::Error::other("injected completion read failure"));
        }
        let read = self.inner.read(&mut buffer[..1])?;
        self.position += read as u64;
        Ok(read)
    }
}
impl<R: Seek> Seek for CompletionFailure<'_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = self.inner.seek(from)?;
        if matches!(from, SeekFrom::Start(0)) {
            self.pass += 1;
        }
        Ok(self.position)
    }
}
#[cfg(unix)]
#[test]
fn completion_io_failure_and_cancellation_preserve_prior_output_and_remove_stage() {
    let bytes = two_tiny(&tiny(&[4, 0, 5]), 0);
    // With bytewise input, gif may read the EOI byte while filling the pixel.
    // The terminator is fetched during explicit completion, as proved below.
    for fail_at in [41, 64] {
        for cancel in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("previous.gif");
            fs::write(&output, b"prior-output").unwrap();
            let control = ExportControl::default();
            let fired = Cell::new(false);
            let result = (|| -> Result<(), GifError> {
                let _job = control.begin()?;
                let target = super::gif_file::Target::new(
                    None,
                    &output,
                    ReplacePolicy::ReplaceExistingFile,
                )?;
                let mut stage = target.stage()?;
                stage.file.write_all(&bytes)?;
                let mut reader = CompletionFailure {
                    inner: &mut stage.file,
                    position: bytes.len() as u64,
                    pass: 0,
                    fail_at,
                    control: &control,
                    cancel,
                    fired: &fired,
                };
                super::gif_structure::validate_reader(&mut reader, &tiny_plan(), &control)?;
                target.publish(&stage, &control)?;
                Ok(())
            })();
            assert!(fired.get(), "failure location was not reached: {fail_at}");
            if cancel {
                assert!(matches!(result, Err(GifError::Cancelled)), "{result:?}");
                assert_eq!(control.status().unwrap(), ExportStatus::Cancelled);
            } else {
                assert!(matches!(result, Err(GifError::InvalidOutput)), "{result:?}");
                assert_eq!(control.status().unwrap(), ExportStatus::Failed);
            }
            assert_eq!(fs::read(&output).unwrap(), b"prior-output");
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}

#[test]
fn injected_completion_failures_occur_after_exact_pixel_output() {
    let bytes = two_tiny(&tiny(&[4, 0, 5]), 0);
    for (target_frame, fail_at) in [41, 64].into_iter().enumerate() {
        let control = ExportControl::default();
        let fired = Cell::new(false);
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::RGBA);
        options.check_lzw_end_code(true);
        let mut decoder = options
            .read_info(CompletionFailure {
                inner: Cursor::new(&bytes),
                position: 0,
                pass: 2,
                fail_at,
                control: &control,
                cancel: false,
                fired: &fired,
            })
            .unwrap();
        for index in 0..=target_frame {
            decoder.next_frame_info().unwrap().unwrap();
            let mut pixel = [0; 4];
            decoder.read_into_buffer(&mut pixel).unwrap();
            assert_eq!(pixel, [0, 0, 0, 255]);
            assert!(!fired.get(), "failure must not occur during pixel fill");
            let completion = decoder.fill_buffer(&mut [0; 4]);
            if index == target_frame {
                assert!(matches!(completion, Err(::gif::DecodingError::Io(_))));
                assert!(fired.get());
            } else {
                assert!(!completion.unwrap());
            }
        }
    }
}
