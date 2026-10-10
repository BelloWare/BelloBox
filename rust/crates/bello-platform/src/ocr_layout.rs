//! Swift Box's local OCR layout (e43b1c4): `OCRTileSegmenter` tiles and overlap
//! de-duplication, `OCRBoundingBoxConverter` pixel rectangles and reading
//! order, and `OCRResultFormatter.plainText`. Given the same Vision
//! observations, Rust returns the same text as Swift. Portable and pure; the
//! macOS backend supplies the observations.
//!
//! One knowing difference: Swift compares texts for de-duplication by canonical
//! equivalence, Rust by bytes. Both band readings come from the same
//! recognizer, so their texts are byte-identical where it matters.
use std::cmp::Ordering;

/// Image pixels, top-left origin (CoreGraphics `CGRect`, standardized).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        // CGRect.standardized: nonnegative size, same covered area.
        let (x, width) = if width < 0.0 {
            (x + width, -width)
        } else {
            (x, width)
        };
        let (y, height) = if height < 0.0 {
            (y + height, -height)
        } else {
            (y, height)
        };
        Self {
            x,
            y,
            width,
            height,
        }
    }
    fn max_x(&self) -> f64 {
        self.x + self.width
    }
    fn max_y(&self) -> f64 {
        self.y + self.height
    }
    fn mid_y(&self) -> f64 {
        self.y + self.height / 2.0
    }
    fn area(&self) -> f64 {
        if self.width == 0.0 || self.height == 0.0 {
            0.0
        } else {
            self.width * self.height
        }
    }
    /// CGRect.intersection, with a disjoint result as an empty rectangle.
    fn intersection_area(&self, other: &Rect) -> f64 {
        let width = self.max_x().min(other.max_x()) - self.x.max(other.x);
        let height = self.max_y().min(other.max_y()) - self.y.max(other.y);
        if width < 0.0 || height < 0.0 {
            0.0
        } else {
            width * height
        }
    }
}

/// One recognized line. `rect` is `None` only for regions without geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub text: String,
    pub rect: Option<Rect>,
}

/// `OCRTileSegmenter.tiles` defaults: an image taller than 3200 px is read in
/// 3200 px bands overlapping by 160 px, so no line is cut in every band.
pub const MAX_TILE_HEIGHT: u64 = 3200;
pub const TILE_OVERLAP: u64 = 160;

/// The `(y offset, height)` bands Swift reads an image of `height` pixels in.
pub fn tile_bands(height: u64) -> Vec<(u64, u64)> {
    if height <= MAX_TILE_HEIGHT {
        return vec![(0, height)];
    }
    let mut bands = Vec::new();
    let mut y = 0;
    while y < height {
        let band = MAX_TILE_HEIGHT.min(height - y);
        bands.push((y, band));
        if y + band >= height {
            break;
        }
        y += MAX_TILE_HEIGHT - TILE_OVERLAP;
    }
    bands
}

/// `OCRBoundingBoxConverter.imagePixelRect`: a Vision box (normalized,
/// bottom-left origin) in pixels of an image `width` × `height`, top-left origin.
pub fn pixel_rect(normalized: Rect, width: f64, height: f64) -> Rect {
    Rect::new(
        normalized.x * width,
        (1.0 - normalized.max_y()) * height,
        normalized.width * width,
        normalized.height * height,
    )
}

fn rect_or_zero(region: &Region) -> Rect {
    region.rect.unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    })
}

/// Swift's `primary == ? secondary < : primary <` comparator, with its float
/// equality (so -0 equals 0) and a stable sort, as Swift's own sort is.
fn swift_order(primary: f64, secondary: f64, other_primary: f64, other_secondary: f64) -> Ordering {
    if primary == other_primary {
        secondary.partial_cmp(&other_secondary)
    } else {
        primary.partial_cmp(&other_primary)
    }
    .unwrap_or(Ordering::Equal)
}

struct ReadingLine {
    regions: Vec<Region>,
    mid_y_total: f64,
    height_total: f64,
}

impl ReadingLine {
    fn min_y(&self) -> f64 {
        self.regions
            .iter()
            .map(|region| region.rect.map_or(0.0, |rect| rect.y))
            .fold(f64::INFINITY, f64::min)
    }
    fn contains(&self, rect: &Rect) -> bool {
        let count = self.regions.len() as f64;
        let tolerance = 6f64.max((self.height_total / count).min(rect.height) * 0.45);
        (self.mid_y_total / count - rect.mid_y()).abs() <= tolerance
    }
}

/// `Array<OCRTextRegion>.sortedByReadingOrder`: regions grouped into lines by
/// vertical midpoint, lines top to bottom, each line left to right.
pub fn reading_order(regions: &[Region]) -> Vec<Region> {
    let mut top_to_bottom = regions.to_vec();
    top_to_bottom.sort_by(|left, right| {
        let (left, right) = (rect_or_zero(left), rect_or_zero(right));
        swift_order(left.y, left.x, right.y, right.x)
    });
    let mut lines: Vec<ReadingLine> = Vec::new();
    for region in top_to_bottom {
        let rect = rect_or_zero(&region);
        if let Some(line) = lines.iter_mut().find(|line| line.contains(&rect)) {
            line.regions.push(region);
            line.mid_y_total += rect.mid_y();
            line.height_total += rect.height;
        } else {
            lines.push(ReadingLine {
                regions: vec![region],
                mid_y_total: rect.mid_y(),
                height_total: rect.height,
            });
        }
    }
    lines.sort_by(|left, right| {
        left.min_y()
            .partial_cmp(&right.min_y())
            .unwrap_or(Ordering::Equal)
    });
    lines
        .into_iter()
        .flat_map(|mut line| {
            line.regions.sort_by(|left, right| {
                let (left, right) = (rect_or_zero(left), rect_or_zero(right));
                swift_order(left.x, left.y, right.x, right.y)
            });
            line.regions
        })
        .collect()
}

/// `OCRTileSegmenter.deduplicateOverlapRegions`: a band overlap reads some
/// lines twice; a repeat with the same text covering at least 72% of its own
/// area by an earlier kept region is dropped.
pub fn deduplicate(regions: &[Region]) -> Vec<Region> {
    let mut kept: Vec<Region> = Vec::new();
    for region in reading_order(regions) {
        let Some(rect) = region.rect else {
            kept.push(region);
            continue;
        };
        let duplicate = kept.iter().any(|existing| {
            let Some(other) = existing.rect else {
                return false;
            };
            existing.text == region.text
                && rect.area() > 0.0
                && rect.intersection_area(&other) / rect.area() >= 0.72
        });
        if !duplicate {
            kept.push(region);
        }
    }
    kept
}

/// Foundation's `CharacterSet.whitespacesAndNewlines` (the Text Tools oracle
/// established `whitespaces` as tab, the space separators and U+200B).
fn is_whitespace_or_newline(c: char) -> bool {
    matches!(
        c,
        '\t'..='\r'
            | ' '
            | '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200b}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// `OCRResultFormatter.plainText(from:)`: lines in reading order, trimmed, with
/// a blank line where the gap above a line exceeds 1.35 median line heights.
pub fn plain_text(regions: &[Region]) -> String {
    let sorted = reading_order(regions);
    let mut heights: Vec<f64> = sorted
        .iter()
        .filter_map(|region| region.rect.map(|rect| rect.height).filter(|&h| h > 0.0))
        .collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let median = heights.get(heights.len() / 2).copied().unwrap_or(14.0);
    let mut lines: Vec<&str> = Vec::new();
    let mut previous: Option<Rect> = None;
    for region in &sorted {
        let text = region.text.trim_matches(is_whitespace_or_newline);
        if text.is_empty() {
            continue;
        }
        if let (Some(previous), Some(rect)) = (previous, region.rect) {
            if rect.y - previous.max_y() > median * 1.35 {
                lines.push("");
            }
        }
        lines.push(text);
        previous = region.rect.or(previous);
    }
    lines.join("\n")
}

/// Swift's whole local result: regions of every band, de-duplicated, as text.
/// `None` when nothing but whitespace was recognized ("No text was found").
pub fn recognized_text(regions: &[Region]) -> Option<String> {
    let text = plain_text(&deduplicate(regions));
    (!text.trim_matches(is_whitespace_or_newline).is_empty()).then_some(text)
}

#[cfg(test)]
mod tests;
