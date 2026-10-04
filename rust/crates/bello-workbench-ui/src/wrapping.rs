//! Font-measured visual rows retain source byte ranges. Unchanged paragraphs
//! reuse their wrapping after local edits, including insertions/removals of lines.
use bello_workbench::editor::TextBuffer;
use std::ops::Range;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DisplayRow {
    pub source_row: usize,
    pub range: Range<usize>,
    pub first: bool,
}
#[derive(Clone, Debug)]
struct Paragraph {
    text: String,
    ends: Vec<usize>,
}
#[derive(Default)]
pub(crate) struct WrappedRows {
    paragraphs: Vec<Paragraph>,
    pub rows: Vec<DisplayRow>,
    revision: Option<u64>,
    width: u32,
    style: u64,
}
impl WrappedRows {
    pub fn clear(&mut self) {
        self.paragraphs.clear();
        self.rows.clear();
        self.revision = None;
    }
    pub fn refresh(
        &mut self,
        buffer: &TextBuffer,
        revision: u64,
        width: f32,
        style: u64,
        mut boundaries: impl FnMut(&str) -> Vec<usize>,
    ) {
        let width = width.max(1.).round() as u32;
        if self.revision == Some(revision) && self.width == width && self.style == style {
            return;
        }
        if self.width != width || self.style != style {
            self.paragraphs.clear();
        }
        let old_len = self.paragraphs.len();
        let new_len = buffer.line_count();
        let mut prefix = 0;
        while prefix < old_len.min(new_len) && self.paragraphs[prefix].text == buffer.line(prefix) {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < old_len - prefix
            && suffix < new_len - prefix
            && self.paragraphs[old_len - suffix - 1].text == buffer.line(new_len - suffix - 1)
        {
            suffix += 1;
        }
        let replacement = (prefix..new_len - suffix)
            .map(|row| {
                let text = buffer.line(row).to_owned();
                let mut ends = Vec::new();
                for end in boundaries(&text) {
                    if end > ends.last().copied().unwrap_or(0)
                        && end < text.len()
                        && text.is_char_boundary(end)
                    {
                        ends.push(end);
                    }
                }
                ends.push(text.len());
                Paragraph { text, ends }
            })
            .collect::<Vec<_>>();
        self.paragraphs
            .splice(prefix..old_len - suffix, replacement);
        self.rows.clear();
        for (row, paragraph) in self.paragraphs.iter().enumerate() {
            let mut start = 0;
            let base = buffer.line_start(row);
            for &end in &paragraph.ends {
                self.rows.push(DisplayRow {
                    source_row: row,
                    range: base + start..base + end,
                    first: start == 0,
                });
                start = end;
            }
        }
        self.revision = Some(revision);
        self.width = width;
        self.style = style;
    }
    pub fn row_at(&self, offset: usize) -> usize {
        self.rows
            .partition_point(|row| row.range.start <= offset)
            .saturating_sub(1)
            .min(self.rows.len().saturating_sub(1))
    }
}
#[cfg(test)]
mod tests {
    use super::{DisplayRow, WrappedRows};
    use bello_workbench::editor::TextBuffer;
    fn chars(text: &str) -> Vec<usize> {
        text.char_indices()
            .filter_map(|(i, _)| {
                (text[..i].chars().count().is_multiple_of(4) && i > 0).then_some(i)
            })
            .collect()
    }
    #[test]
    fn wrapped_ranges_preserve_unicode_and_physical_line_numbers() {
        let b = TextBuffer::new("ab😀中xy\r\nnext\n".into());
        let mut map = WrappedRows::default();
        map.refresh(&b, 0, 40., 0, chars);
        assert_eq!(
            map.rows[0],
            DisplayRow {
                source_row: 0,
                range: 0..9,
                first: true
            }
        );
        assert_eq!(&b.text()[map.rows[1].range.clone()], "xy");
        assert_eq!(map.rows[2].source_row, 1);
        assert_eq!(map.rows[3].source_row, 2);
        assert_eq!(map.row_at(9), 1);
        assert_eq!(map.row_at(b.len()), 3);
    }
    #[test]
    fn unchanged_paragraphs_reuse_font_measurements() {
        let mut map = WrappedRows::default();
        let b = TextBuffer::new("one\ntwo\nthree".into());
        let mut calls = 0;
        map.refresh(&b, 0, 80., 0, |s| {
            calls += 1;
            chars(s)
        });
        assert_eq!(calls, 3);
        map.refresh(&b, 0, 80., 0, |_| panic!("unchanged repaint rewrapped"));
        let b = TextBuffer::new("one\ninserted\ntwo\nthree".into());
        calls = 0;
        map.refresh(&b, 1, 80., 0, |s| {
            calls += 1;
            chars(s)
        });
        assert_eq!(calls, 1);
        assert_eq!(map.rows.last().unwrap().source_row, 3);
    }
    #[test]
    fn resize_reflows_and_invalid_boundaries_are_ignored() {
        let b = TextBuffer::new("😀wide".into());
        let mut map = WrappedRows::default();
        map.refresh(&b, 0, 100., 0, |_| vec![1, 4, 4, 3, 99]);
        assert_eq!(map.rows.len(), 2);
        assert_eq!(map.rows[0].range, 0..4);
        map.refresh(&b, 0, 200., 0, |_| Vec::new());
        assert_eq!(map.rows.len(), 1);
    }
}
