//! Transient, exact-text-bound decorations. These values never enter edit history.
use gpui::{Bounds, Hsla, Pixels, WindowId};
use std::{ops::Range, sync::Arc};

pub const MAX_TEXT_DECORATIONS: usize = 16_384;
pub const MAX_PRESENTATION_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct TextDecoration {
    /// Nonempty original UTF-8 byte range, never normalized-string offsets.
    pub range: Range<usize>,
    pub color: Hsla,
}
#[derive(Clone)]
pub struct TextPresentation {
    /// Opaque host generation. Hosts must replace this on document/query changes.
    pub token: u64,
    pub text: Arc<str>,
    /// Sorted, nonoverlapping spans. An emphasized span may overlap these.
    pub decorations: Arc<[TextDecoration]>,
    pub emphasized: Option<TextDecoration>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationError {
    TextChanged,
    InvalidRange,
    LimitExceeded,
    Busy,
    StaleToken,
}
/// A one-shot receipt from a completed paint, not a persistent geometry cache.
/// Hosts MUST check their current navigation/lifetime generation before using it.
/// A later outer scroll or layout invalidates these window coordinates.
#[derive(Clone, Debug)]
pub struct PresentationGeometry {
    pub token: u64,
    pub host_generation: u64,
    pub layout_generation: u64,
    pub window_id: WindowId,
    pub first_visible_fragment: Bounds<Pixels>,
    pub fully_visible: bool,
}
impl TextPresentation {
    pub(crate) fn validate(&self, current: &str) -> Result<(), PresentationError> {
        if self.text.len() > MAX_PRESENTATION_BYTES || self.decorations.len() > MAX_TEXT_DECORATIONS
        {
            return Err(PresentationError::LimitExceeded);
        }
        if self.text.as_ref() != current {
            return Err(PresentationError::TextChanged);
        }
        let mut end = 0;
        for decoration in self.decorations.iter() {
            validate_range(current, &decoration.range)?;
            if decoration.range.start < end {
                return Err(PresentationError::InvalidRange);
            }
            end = decoration.range.end;
        }
        if let Some(decoration) = &self.emphasized {
            validate_range(current, &decoration.range)?;
        }
        Ok(())
    }
}
pub(crate) fn validate_range(text: &str, range: &Range<usize>) -> Result<(), PresentationError> {
    if range.start >= range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err(PresentationError::InvalidRange);
    }
    Ok(())
}
pub(crate) fn intersection(a: &Range<usize>, b: &Range<usize>) -> Option<Range<usize>> {
    let range = a.start.max(b.start)..a.end.min(b.end);
    (!range.is_empty()).then_some(range)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn presentation(text: &str, ranges: &[Range<usize>]) -> TextPresentation {
        TextPresentation {
            token: 1,
            text: Arc::from(text),
            decorations: ranges
                .iter()
                .cloned()
                .map(|range| TextDecoration {
                    range,
                    color: gpui::rgb(0xff9900).into(),
                })
                .collect(),
            emphasized: None,
        }
    }
    #[test]
    fn exact_text_unicode_and_order_are_required() {
        let valid = presentation("a😀é\r\nz", &[1..5, 5..8, 10..11]);
        assert_eq!(valid.validate("a😀é\r\nz"), Ok(()));
        assert_eq!(
            valid.validate("b😀é\r\nz"),
            Err(PresentationError::TextChanged)
        );
        for ranges in [
            std::iter::once(2..5).collect(),
            std::iter::once(1..1).collect(),
            std::iter::once(0..99).collect(),
            vec![5..8, 1..5],
            vec![1..8, 5..8],
        ] {
            assert_eq!(
                presentation("a😀é\r\nz", &ranges).validate("a😀é\r\nz"),
                Err(PresentationError::InvalidRange)
            );
        }
    }
    #[test]
    fn explicit_limits_never_silently_truncate() {
        let text = "x".repeat(MAX_PRESENTATION_BYTES + 1);
        assert_eq!(
            presentation(&text, &[]).validate(&text),
            Err(PresentationError::LimitExceeded)
        );
        let ranges = vec![0..1; MAX_TEXT_DECORATIONS + 1];
        assert_eq!(
            presentation("x", &ranges).validate("x"),
            Err(PresentationError::LimitExceeded)
        );
    }
    #[test]
    fn intersections_use_original_byte_coordinates() {
        assert_eq!(intersection(&(2..9), &(5..12)), Some(5..9));
        assert_eq!(intersection(&(2..5), &(5..12)), None);
        assert_eq!(intersection(&(9..12), &(2..5)), None);
    }
}
