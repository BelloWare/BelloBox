use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Overlap {
    pub rows: u32,
    pub score: f64,
}
struct Gray {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
fn luminance(pixel: &image::Rgba<u8>) -> u8 {
    let alpha = u32::from(pixel[3]);
    ((u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114) * alpha
        / 255_000) as u8
}
impl Gray {
    fn new(
        frame: &ScrollFrame,
        target_width: u32,
        cancel: &Cancellation,
    ) -> ScrollResultValue<Self> {
        let width = target_width.min(frame.width()).max(1);
        let height =
            (u64::from(frame.height()) * u64::from(width) / u64::from(frame.width())).max(1) as u32;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes_for(width, height, 1)? as usize)
            .map_err(|_| ScrollError::MemoryLimit)?;
        for y in 0..height {
            cancel.check()?;
            let sy = (u64::from(y) * u64::from(frame.height()) / u64::from(height)) as u32;
            for x in 0..width {
                let sx = (u64::from(x) * u64::from(frame.width()) / u64::from(width)) as u32;
                pixels.push(luminance(frame.0.get_pixel(sx, sy)));
            }
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
    fn difference(
        &self,
        y: u32,
        other: &Self,
        other_y: u32,
        rows: u32,
        inset: u32,
        cancel: &Cancellation,
    ) -> ScrollResultValue<f64> {
        let width = self.width.min(other.width);
        let inset = inset.min(width / 4);
        let mut total = 0u64;
        let mut count = 0u64;
        for row in 0..rows {
            cancel.check()?;
            if y + row >= self.height || other_y + row >= other.height {
                break;
            }
            for x in inset..(width - inset).max(inset + 1) {
                total += u64::from(
                    self.pixels[((y + row) * self.width + x) as usize]
                        .abs_diff(other.pixels[((other_y + row) * other.width + x) as usize]),
                );
                count += 1;
            }
        }
        Ok(if count == 0 {
            1.
        } else {
            total as f64 / count as f64 / 255.
        })
    }
}
fn exact_difference(
    a: &ScrollFrame,
    ay: u32,
    b: &ScrollFrame,
    by: u32,
    rows: u32,
    inset: u32,
    cancel: &Cancellation,
) -> ScrollResultValue<f64> {
    let width = a.width().min(b.width());
    let inset = inset.min(width / 4);
    let mut total = 0u64;
    let mut count = 0u64;
    for row in 0..rows {
        cancel.check()?;
        if ay + row >= a.height() || by + row >= b.height() {
            break;
        }
        for x in inset..(width - inset).max(inset + 1) {
            total += u64::from(
                luminance(a.0.get_pixel(x, ay + row))
                    .abs_diff(luminance(b.0.get_pixel(x, by + row))),
            );
            count += 1;
        }
    }
    Ok(if count == 0 {
        1.
    } else {
        total as f64 / count as f64 / 255.
    })
}
pub(super) fn difference(
    a: &ScrollFrame,
    b: &ScrollFrame,
    cancel: &Cancellation,
) -> ScrollResultValue<f64> {
    if a.dimensions() != b.dimensions() {
        return Err(ScrollError::AreaChanged);
    }
    let a = Gray::new(a, 420, cancel)?;
    let b = Gray::new(b, 420, cancel)?;
    a.difference(0, &b, 0, a.height.min(b.height), 8, cancel)
}
pub(super) fn overlap(
    previous: &ScrollFrame,
    current: &ScrollFrame,
    config: &StitchConfig,
    header: u32,
    footer: u32,
    cancel: &Cancellation,
) -> ScrollResultValue<Option<Overlap>> {
    cancel.check()?;
    let a = Gray::new(previous, config.downsample_width, cancel)?;
    let b = Gray::new(current, config.downsample_width, cancel)?;
    let header = header.min(current.height() - 1);
    let footer = footer.min(previous.height() - 1);
    let usable = (previous.height() - footer).min(current.height() - header);
    let maximum = (f64::from(usable) * config.max_overlap_fraction) as u32;
    if maximum < config.min_overlap {
        return Ok(None);
    }
    let scale = f64::from(a.height) / f64::from(previous.height());
    let scaled_header = (f64::from(header) * scale).round() as u32;
    let end = a
        .height
        .saturating_sub((f64::from(footer) * scale).round() as u32);
    let mut best: Option<Overlap> = None;
    let mut scores = Vec::new();
    for rows in (config.min_overlap..=maximum).step_by(4) {
        cancel.check()?;
        let scaled = (f64::from(rows) * scale).max(1.) as u32;
        if scaled >= end || scaled_header + scaled > b.height {
            continue;
        }
        let score = a.difference(end - scaled, &b, scaled_header, scaled, 8, cancel)?;
        scores.push(score);
        if best.is_none_or(|old| {
            score < old.score - 0.0001 || ((score - old.score).abs() <= 0.0001 && rows > old.rows)
        }) {
            best = Some(Overlap { rows, score });
        }
    }
    let Some(mut best) = best else {
        return Ok(None);
    };
    let min = best.rows.saturating_sub(4).max(1);
    let max = (best.rows + 4).min(usable);
    best.score = exact_difference(
        previous,
        previous.height() - footer - best.rows,
        current,
        header,
        best.rows,
        8,
        cancel,
    )?;
    for rows in min..=max {
        let score = exact_difference(
            previous,
            previous.height() - footer - rows,
            current,
            header,
            rows,
            8,
            cancel,
        )?;
        if score < best.score - 0.0005 {
            best = Overlap { rows, score };
        }
    }
    if best.score > config.score_threshold {
        return Ok(None);
    }
    if best.score > 0.01 && scores.len() >= 5 {
        scores.sort_by(f64::total_cmp);
        if best.score > scores[scores.len() / 2] * 0.5 {
            return Ok(None);
        }
    }
    Ok(Some(best))
}
fn repeated_band(
    a: &ScrollFrame,
    b: &ScrollFrame,
    top: bool,
    cancel: &Cancellation,
) -> ScrollResultValue<u32> {
    if a.width() != b.width() {
        return Ok(0);
    }
    let cap = (a.height().min(b.height()) / 3).min(240);
    if cap < 24 {
        return Ok(0);
    }
    let ga = Gray::new(a, 420, cancel)?;
    let gb = Gray::new(b, 420, cancel)?;
    let scale = f64::from(ga.height) / f64::from(a.height());
    let mut coarse = 0;
    for rows in (24..=cap).step_by(8) {
        let n = (f64::from(rows) * scale).round().max(1.) as u32;
        if n > ga.height || n > gb.height {
            continue;
        }
        let score = ga.difference(
            if top { 0 } else { ga.height - n },
            &gb,
            if top { 0 } else { gb.height - n },
            n,
            12,
            cancel,
        )?;
        if score < 0.012 {
            coarse = rows;
        }
    }
    if coarse == 0 {
        return Ok(0);
    }
    let limit = (coarse + 8).min(cap);
    let matches = |depth| {
        exact_difference(
            a,
            if top { depth } else { a.height() - 1 - depth },
            b,
            if top { depth } else { b.height() - 1 - depth },
            1,
            12,
            cancel,
        )
        .map(|n| n < 0.02)
    };
    let mut rows = coarse.min(limit);
    while rows > 0 && !matches(rows - 1)? {
        rows -= 1;
    }
    while rows < limit && matches(rows)? {
        rows += 1;
    }
    Ok(if rows >= 8 { rows } else { 0 })
}
pub(super) fn sticky_band(
    first: &ScrollFrame,
    previous: &ScrollFrame,
    current: &ScrollFrame,
    top: bool,
    cancel: &Cancellation,
) -> ScrollResultValue<u32> {
    let a = repeated_band(first, current, top, cancel)?;
    if a == 0 {
        return Ok(0);
    }
    Ok(a.min(repeated_band(previous, current, top, cancel)?))
}

/// Stitch same-width frames. The live session requires identical dimensions.
/// Arbitrary imported-image resizing is intentionally outside this capture foundation.
pub fn stitch(
    frames: &[ScrollFrame],
    config: &StitchConfig,
    cancel: &Cancellation,
) -> ScrollResultValue<ScrollResult> {
    config.validate()?;
    cancel.check()?;
    if frames.is_empty() {
        return Err(ScrollError::NoFrames);
    }
    if frames.len() > 60 {
        return Err(ScrollError::InvalidInput);
    }
    let ordered: Vec<_> = match config.direction {
        ScrollDirection::Down => frames.iter().enumerate().collect(),
        ScrollDirection::Up => frames.iter().enumerate().rev().collect(),
    };
    let first = ordered[0].1;
    if frames.iter().any(|f| f.width() != first.width()) {
        return Err(ScrollError::AreaChanged);
    }
    let retained = check_budget(
        frames.iter().map(ScrollFrame::byte_len),
        config.max_working_bytes,
    )?;
    let largest = frames.iter().map(ScrollFrame::byte_len).max().unwrap_or(0);
    // Two grayscale images are always smaller than their RGBA sources; reserve conservatively.
    check_budget([retained, largest], config.max_working_bytes)?;
    let mut headers = vec![0; frames.len()];
    let mut footers = vec![0; frames.len()];
    for i in 1..frames.len() {
        cancel.check()?;
        if config.remove_repeated_bars {
            headers[i] = sticky_band(first, ordered[i - 1].1, ordered[i].1, true, cancel)?;
            footers[i] = sticky_band(first, ordered[i - 1].1, ordered[i].1, false, cancel)?;
        }
    }
    if frames.len() > 1 {
        footers[0] = footers[1];
    }
    let mut placements = vec![FramePlacement {
        frame_index: ordered[0].0,
        y: 0,
        overlap: 0,
        confidence: 1.,
        cropped_top: 0,
        cropped_bottom: 0,
    }];
    let mut height = i64::from(first.height());
    let mut notes = Vec::new();
    for i in 1..frames.len() {
        cancel.check()?;
        let (frame_index, current) = ordered[i];
        let previous = ordered[i - 1].1;
        let found = overlap(
            previous,
            current,
            config,
            headers[i],
            footers[i - 1],
            cancel,
        )?;
        let rows = found.map_or(0, |m| m.rows);
        if found.is_none() {
            notes.push(ScrollCaptureNote::UnmatchedSeam(frame_index + 1));
        } else if (previous.dimensions() == current.dimensions()
            && difference(previous, current, cancel)? <= 0.015)
            || f64::from(rows)
                > f64::from(current.height().saturating_sub(headers[i] + footers[i])) * 0.88
        {
            notes.push(ScrollCaptureNote::NearlyUnchanged(frame_index + 1));
        }
        let slack = if found.is_some() {
            SEAM_SLACK.min(rows)
        } else {
            0
        };
        if found.is_some() {
            placements.last_mut().unwrap().cropped_bottom = footers[i - 1] + slack;
            height -= i64::from(footers[i - 1] + slack);
        }
        let top = headers[i] + rows - slack;
        placements.push(FramePlacement {
            frame_index,
            y: height - i64::from(top),
            overlap: rows,
            confidence: found.map_or(0., |m| 1. - m.score),
            cropped_top: top,
            cropped_bottom: 0,
        });
        height += i64::from(current.height()) - i64::from(top);
    }
    if height <= 0 {
        return Err(ScrollError::CannotRender);
    }
    let height = height as u64;
    if height > u64::from(config.max_output_height)
        || u64::from(first.width())
            .checked_mul(height)
            .is_none_or(|n| n > MAX_IMAGE_PIXELS)
    {
        return Err(ScrollError::OutputTooTall(height));
    }
    let output_bytes = bytes_for(first.width(), height as u32, 4)?;
    check_budget([retained, largest, output_bytes], config.max_working_bytes)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(output_bytes as usize)
        .map_err(|_| ScrollError::MemoryLimit)?;
    bytes.resize(output_bytes as usize, 0);
    let mut output = RgbaImage::from_raw(first.width(), height as u32, bytes)
        .ok_or(ScrollError::CannotRender)?;
    for (placement, (_, frame)) in placements.iter().zip(ordered.iter()) {
        let end = frame.height().saturating_sub(placement.cropped_bottom);
        for row in placement.cropped_top..end {
            cancel.check()?;
            let dest = placement.y + i64::from(row);
            if dest < 0 || dest >= height as i64 {
                return Err(ScrollError::CannotRender);
            }
            let stride = first.width() as usize * 4;
            let src = row as usize * stride;
            let dst = dest as usize * stride;
            output.as_mut()[dst..dst + stride]
                .copy_from_slice(&frame.0.as_raw()[src..src + stride]);
        }
    }
    cancel.check()?;
    Ok(ScrollResult {
        document: ScreenshotDocument::from_rgba(output).map_err(|_| ScrollError::CannotRender)?,
        placements,
        notes,
        frame_count: frames.len(),
    })
}
