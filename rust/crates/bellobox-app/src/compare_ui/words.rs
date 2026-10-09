//! Selectable, bounded inline word pages with semantic color and real strikes.
//! Pagination bounds shaping work, never the complete signed Copy payload.
use bellobox_core::developer::comparison::{Row, RowKind};
use gpui::{
    App, ClipboardItem, Context, FocusHandle, Focusable, HighlightStyle, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, StrikethroughStyle, StyledText, TextLayout, Window, div,
    prelude::*, px,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
const PAGE_BYTES: usize = 16_384;
const PAGE_SPANS: usize = 128;
#[derive(Default)]
struct Page {
    text: String,
    spans: Vec<(Range<usize>, RowKind)>,
}
pub(super) struct WordsView {
    pages: Vec<Page>,
    page: usize,
    anchor: usize,
    cursor: usize,
    dragging: bool,
    layout: Option<TextLayout>,
    focus: FocusHandle,
}
impl WordsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            pages: Vec::new(),
            page: 0,
            anchor: 0,
            cursor: 0,
            dragging: false,
            layout: None,
            focus: cx.focus_handle(),
        }
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.pages.clear();
        self.page = 0;
        self.reset_selection();
        cx.notify();
    }
    pub fn set_rows(&mut self, rows: Vec<Row>, cx: &mut Context<Self>) {
        self.pages = pages(rows);
        self.page = 0;
        self.reset_selection();
        cx.notify();
    }
    fn reset_selection(&mut self) {
        self.anchor = 0;
        self.cursor = 0;
        self.dragging = false;
        self.layout = None;
    }
    pub fn has_previous(&self) -> bool {
        self.page > 0
    }
    pub fn has_next(&self) -> bool {
        self.page + 1 < self.pages.len()
    }
    pub fn previous(&mut self, cx: &mut Context<Self>) {
        if self.has_previous() {
            self.page -= 1;
            self.reset_selection();
            cx.notify();
        }
    }
    pub fn next(&mut self, cx: &mut Context<Self>) {
        if self.has_next() {
            self.page += 1;
            self.reset_selection();
            cx.notify();
        }
    }
    pub fn page_label(&self) -> String {
        format!(
            "Inline page {} of {} · Copy Result includes every signed row",
            self.page + 1,
            self.pages.len().max(1)
        )
    }
    fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }
    fn text(&self) -> &str {
        self.pages
            .get(self.page)
            .map(|p| p.text.as_str())
            .unwrap_or("")
    }
    fn index(&self, position: gpui::Point<gpui::Pixels>) -> usize {
        let Some(layout) = &self.layout else {
            return 0;
        };
        let index = layout
            .index_for_position(position)
            .unwrap_or_else(|i| i)
            .min(self.text().len());
        floor_grapheme(self.text(), index)
    }
    fn key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let command = e.keystroke.modifiers.platform
            || (cfg!(target_os = "linux") && e.keystroke.modifiers.control);
        match (command, e.keystroke.key.as_str()) {
            (true, "a") => {
                self.anchor = 0;
                self.cursor = self.text().len();
            }
            (true, "c") => {
                let range = self.selection();
                if !range.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(self.text()[range].to_owned()));
                }
            }
            (false, "left" | "right" | "home" | "end") => {
                let next = match e.keystroke.key.as_str() {
                    "home" => 0,
                    "end" => self.text().len(),
                    "left" => self.text()[..self.cursor]
                        .grapheme_indices(true)
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0),
                    _ => self.text()[self.cursor..]
                        .graphemes(true)
                        .next()
                        .map(|c| self.cursor + c.len())
                        .unwrap_or(self.cursor),
                };
                self.cursor = next;
                if !e.keystroke.modifiers.shift {
                    self.anchor = next;
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}
fn floor_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}
fn floor_grapheme(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|i| *i <= index)
        .last()
        .unwrap_or(0)
}
fn page_boundary(text: &str, limit: usize) -> usize {
    if text.len() <= limit {
        return text.len();
    }
    let boundary = floor_grapheme(text, limit);
    if boundary == 0
        && text
            .graphemes(true)
            .next()
            .is_some_and(|g| g.len() > PAGE_BYTES)
    {
        // A pathological single grapheme can occupy the complete 500 KB input.
        // Bound shaping anyway; full Copy preserves the original bytes.
        floor_boundary(text, limit)
    } else {
        boundary
    }
}
fn pages(rows: Vec<Row>) -> Vec<Page> {
    let mut pages = Vec::new();
    let mut page = Page::default();
    for row in rows {
        let text = row.text + " ";
        let mut rest = text.as_str();
        while !rest.is_empty() {
            if page.text.len() == PAGE_BYTES || page.spans.len() == PAGE_SPANS {
                pages.push(page);
                page = Page::default();
            }
            let take = page_boundary(rest, (PAGE_BYTES - page.text.len()).min(rest.len()));
            if take == 0 {
                pages.push(page);
                page = Page::default();
                continue;
            }
            let start = page.text.len();
            page.text.push_str(&rest[..take]);
            page.spans.push((start..page.text.len(), row.kind));
            rest = &rest[take..];
        }
    }
    if !page.text.is_empty() {
        pages.push(page);
    }
    pages
}
impl Focusable for WordsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for WordsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::for_window(window);
        let selection = self.selection();
        let mut highlights = Vec::new();
        if let Some(page) = self.pages.get(self.page) {
            for (range, kind) in &page.spans {
                let mut points = vec![range.start, range.end];
                for point in [selection.start, selection.end] {
                    if range.contains(&point) {
                        points.push(point);
                    }
                }
                points.sort_unstable();
                points.dedup();
                for pair in points.windows(2) {
                    let range = pair[0]..pair[1];
                    let selected = selection.start <= range.start
                        && range.end <= selection.end
                        && !selection.is_empty();
                    let color = match kind {
                        RowKind::Added => p.success,
                        RowKind::Removed => p.danger,
                        RowKind::Same => p.primary,
                    };
                    let background = if selected {
                        Some(p.accent.opacity(0.30))
                    } else if *kind != RowKind::Same {
                        Some(color.opacity(0.12))
                    } else {
                        None
                    };
                    highlights.push((
                        range,
                        HighlightStyle {
                            color: Some(color),
                            background_color: background,
                            strikethrough: (*kind == RowKind::Removed).then_some(
                                StrikethroughStyle {
                                    thickness: px(1.),
                                    color: Some(p.danger),
                                },
                            ),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        let styled = StyledText::new(self.text().to_owned()).with_highlights(highlights);
        self.layout = Some(styled.layout().clone());
        div()
            .id(("inline-words", self.page))
            .track_focus(&self.focus)
            .h(px(210.))
            .w_full()
            .min_w_0()
            .border_1()
            .border_color(p.separator)
            .focus(|s| s.border_color(p.accent))
            .rounded(px(7.))
            .bg(p.well)
            .overflow_y_scroll()
            .p(px(12.))
            .font_family(if cfg!(target_os = "macos") {
                "Menlo"
            } else {
                "DejaVu Sans Mono"
            })
            .text_size(px(13.))
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus);
                    let index = s.index(e.position);
                    if !e.modifiers.shift {
                        s.anchor = index;
                    }
                    s.cursor = index;
                    s.dragging = true;
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if s.dragging {
                    s.cursor = s.index(e.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _, _, cx| {
                    s.dragging = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, _, _, cx| {
                    s.dragging = false;
                    cx.notify();
                }),
            )
            .child(styled)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn word_pages_are_complete_bounded_and_unicode_aligned() {
        let text = "😀é ".repeat(5000);
        let rows = vec![
            Row {
                kind: RowKind::Removed,
                text: text.clone(),
            },
            Row {
                kind: RowKind::Added,
                text: "new".into(),
            },
        ];
        let result = pages(rows);
        assert_eq!(
            result.iter().map(|p| p.text.as_str()).collect::<String>(),
            text + " new "
        );
        assert!(result.len() > 1);
        for page in result {
            assert!(page.text.len() <= PAGE_BYTES);
            assert!(page.spans.len() <= PAGE_SPANS);
        }
    }
    #[test]
    fn pages_and_navigation_preserve_graphemes_except_oversized_cluster() {
        let cluster = "👩‍👩‍👧‍👧é";
        let text = cluster.repeat(2000);
        let result = pages(vec![Row {
            kind: RowKind::Same,
            text: text.clone(),
        }]);
        let boundaries = (text.clone() + " ")
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let mut position = 0;
        for page in &result {
            assert!(boundaries.contains(&position));
            position += page.text.len();
        }
        assert_eq!(position, text.len() + 1);
        assert_eq!(floor_grapheme("é😀", 1), 0);
        assert_eq!(floor_grapheme("é😀", 4), "é".len());
        let oversized = "e".to_owned() + &"\u{301}".repeat(PAGE_BYTES);
        let result = pages(vec![Row {
            kind: RowKind::Removed,
            text: oversized.clone(),
        }]);
        assert!(result.iter().all(|p| p.text.len() <= PAGE_BYTES));
        assert_eq!(
            result.iter().map(|p| p.text.as_str()).collect::<String>(),
            oversized + " "
        );
    }
    #[test]
    fn many_words_keep_all_rows_and_semantics() {
        let result = pages(
            (0..8000)
                .map(|i| Row {
                    kind: if i % 2 == 0 {
                        RowKind::Added
                    } else {
                        RowKind::Removed
                    },
                    text: i.to_string(),
                })
                .collect(),
        );
        assert!(result.iter().all(|p| p.spans.len() <= PAGE_SPANS));
        assert_eq!(result.iter().map(|p| p.spans.len()).sum::<usize>(), 8000);
        assert!(result.last().unwrap().text.ends_with("7999 "));
    }
}
