use super::{ExportControl, GifError, GifExportPlan, MAX_OUTPUT_BYTES};
use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
};

/// Walks actual GIF blocks, never searches compressed bytes for an identifier.
/// The strict writer-produced subset rejects missing trailers, partial blocks,
/// extra frames, duplicate/malformed loop blocks, bad delays and trailing bytes.
fn inspect(reader: &mut impl Read, plan: &GifExportPlan) -> Result<(), GifError> {
    let mut header = [0; 13];
    reader
        .read_exact(&mut header)
        .map_err(|_| GifError::InvalidOutput)?;
    if (&header[..6] != b"GIF89a" && &header[..6] != b"GIF87a")
        || u16::from_le_bytes([header[6], header[7]]) != plan.output_width
        || u16::from_le_bytes([header[8], header[9]]) != plan.output_height
    {
        return Err(GifError::InvalidOutput);
    }
    if header[10] & 0x80 != 0 {
        skip(reader, 3usize << (usize::from(header[10] & 7) + 1))?;
    }
    let mut frames = 0;
    let mut loops = 0;
    let mut delay = None;
    loop {
        match byte(reader)? {
            0x3b => {
                let mut trailing = [0];
                if frames != plan.frame_count()
                    || delay.is_some()
                    || loops != usize::from(plan.loops)
                    || reader
                        .read(&mut trailing)
                        .map_err(|_| GifError::InvalidOutput)?
                        != 0
                {
                    return Err(GifError::InvalidOutput);
                }
                return Ok(());
            }
            0x2c => {
                if frames >= plan.frame_count() || delay.take() != Some(plan.delays[frames]) {
                    return Err(GifError::InvalidOutput);
                }
                let mut descriptor = [0; 9];
                reader
                    .read_exact(&mut descriptor)
                    .map_err(|_| GifError::InvalidOutput)?;
                if descriptor[..4] != [0; 4]
                    || u16::from_le_bytes([descriptor[4], descriptor[5]]) != plan.output_width
                    || u16::from_le_bytes([descriptor[6], descriptor[7]]) != plan.output_height
                {
                    return Err(GifError::InvalidOutput);
                }
                if descriptor[8] & 0x80 != 0 {
                    skip(reader, 3usize << (usize::from(descriptor[8] & 7) + 1))?;
                }
                if !(2..=8).contains(&byte(reader)?) {
                    return Err(GifError::InvalidOutput);
                }
                subblocks(reader)?;
                frames += 1;
            }
            0x21 => match byte(reader)? {
                0xf9 => {
                    if byte(reader)? != 4 || delay.is_some() {
                        return Err(GifError::InvalidOutput);
                    }
                    let mut data = [0; 4];
                    reader
                        .read_exact(&mut data)
                        .map_err(|_| GifError::InvalidOutput)?;
                    if byte(reader)? != 0 || data[0] & 1 != 0 {
                        return Err(GifError::InvalidOutput);
                    }
                    delay = Some(u16::from_le_bytes([data[1], data[2]]));
                }
                0xff => {
                    if byte(reader)? != 11 {
                        return Err(GifError::InvalidOutput);
                    }
                    let mut identifier = [0; 11];
                    reader
                        .read_exact(&mut identifier)
                        .map_err(|_| GifError::InvalidOutput)?;
                    if &identifier == b"NETSCAPE2.0" || &identifier == b"ANIMEXTS1.0" {
                        if byte(reader)? != 3
                            || byte(reader)? != 1
                            || byte(reader)? != 0
                            || byte(reader)? != 0
                            || byte(reader)? != 0
                        {
                            return Err(GifError::InvalidOutput);
                        }
                        loops += 1;
                        if loops > 1 || !plan.loops {
                            return Err(GifError::InvalidOutput);
                        }
                    } else {
                        subblocks(reader)?;
                    }
                }
                0xfe => subblocks(reader)?,
                _ => return Err(GifError::InvalidOutput),
            },
            _ => return Err(GifError::InvalidOutput),
        }
    }
}
fn byte(reader: &mut impl Read) -> Result<u8, GifError> {
    let mut byte = [0];
    reader
        .read_exact(&mut byte)
        .map_err(|_| GifError::InvalidOutput)?;
    Ok(byte[0])
}
fn skip(reader: &mut impl Read, mut count: usize) -> Result<(), GifError> {
    let mut scratch = [0; 255];
    while count > 0 {
        let size = count.min(scratch.len());
        reader
            .read_exact(&mut scratch[..size])
            .map_err(|_| GifError::InvalidOutput)?;
        count -= size;
    }
    Ok(())
}
fn subblocks(reader: &mut impl Read) -> Result<(), GifError> {
    loop {
        let size = byte(reader)?;
        if size == 0 {
            return Ok(());
        }
        skip(reader, usize::from(size))?;
    }
}

pub(super) fn validate(
    file: &mut File,
    plan: &GifExportPlan,
    control: &ExportControl,
) -> Result<(), GifError> {
    control.check_active()?;
    if file.metadata()?.len() > MAX_OUTPUT_BYTES {
        return Err(GifError::LimitExceeded("encoded GIF bytes"));
    }
    file.seek(SeekFrom::Start(0))?;
    inspect(
        &mut BufReader::new(CancelReader {
            file: &mut *file,
            control,
        }),
        plan,
    )?;
    file.seek(SeekFrom::Start(0))?;
    let mut options = ::gif::DecodeOptions::new();
    options.set_color_output(::gif::ColorOutput::RGBA);
    options.check_frame_consistency(true);
    options.check_lzw_end_code(true);
    options.set_memory_limit(::gif::MemoryLimit::Bytes(
        (1080 * 1080 * 4).try_into().expect("positive constant"),
    ));
    let mut decoder = options
        .read_info(CancelReader { file, control })
        .map_err(|_| GifError::InvalidOutput)?;
    let mut count = 0;
    while let Some(frame) = decoder
        .read_next_frame()
        .map_err(|_| GifError::InvalidOutput)?
    {
        control.check_active()?;
        if count >= plan.frame_count()
            || frame.delay != plan.delays[count]
            || frame.width != plan.output_width
            || frame.height != plan.output_height
            || frame.buffer.len()
                != usize::from(plan.output_width) * usize::from(plan.output_height) * 4
        {
            return Err(GifError::InvalidOutput);
        }
        count += 1;
    }
    if count != plan.frame_count() {
        return Err(GifError::InvalidOutput);
    }
    control.check_active()
}
struct CancelReader<'a> {
    file: &'a mut File,
    control: &'a ExportControl,
}
impl Read for CancelReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.control.check_active().map_err(io::Error::other)?;
        self.file.read(buffer)
    }
}
