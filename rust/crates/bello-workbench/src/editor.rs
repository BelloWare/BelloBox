//! A deliberately documented Vim subset, not a Vim emulator. Byte offsets are
//! always UTF-8 boundaries; Vim motions count Unicode scalar values. Ordinary
//! horizontal navigation follows extended grapheme clusters.
use std::ops::Range;
use unicode_segmentation::GraphemeCursor;

pub const MAX_EDIT_BYTES: usize = 8 * 1024 * 1024;
const UNDO_BUDGET: usize = 16 * 1024 * 1024;
const MAX_COUNT: usize = 100_000;

#[derive(Clone, Debug)]
pub struct TextBuffer {
    text: String,
    lines: Vec<usize>,
    utf16_lines: Vec<usize>,
}
impl TextBuffer {
    pub fn new(text: String) -> Self {
        let mut s = Self {
            text,
            lines: Vec::new(),
            utf16_lines: Vec::new(),
        };
        s.reindex();
        s
    }
    fn reindex(&mut self) {
        self.lines = vec![0];
        self.utf16_lines = vec![0];
        let mut units = 0;
        for (i, c) in self.text.char_indices() {
            units += c.len_utf16();
            if c == '\n' {
                self.lines.push(i + 1);
                self.utf16_lines.push(units);
            }
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn len(&self) -> usize {
        self.text.len()
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
    pub fn line_start(&self, row: usize) -> usize {
        self.lines[row.min(self.lines.len() - 1)]
    }
    pub fn line_range(&self, row: usize) -> Range<usize> {
        let a = self.line_start(row);
        let mut b = self.lines.get(row + 1).copied().unwrap_or(self.len());
        while b > a && matches!(self.text.as_bytes()[b - 1], b'\n' | b'\r') {
            b -= 1;
        }
        a..b
    }
    pub fn line(&self, row: usize) -> &str {
        &self.text[self.line_range(row)]
    }
    pub fn line_full_range(&self, row: usize) -> Range<usize> {
        self.line_start(row)..self.lines.get(row + 1).copied().unwrap_or(self.len())
    }
    pub fn row_at(&self, offset: usize) -> usize {
        self.lines
            .partition_point(|&x| x <= offset.min(self.len()))
            .saturating_sub(1)
    }
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let p = self.floor_boundary(offset);
        let r = self.row_at(p);
        (r, self.text[self.line_start(r)..p].chars().count())
    }
    pub fn offset(&self, row: usize, column: usize) -> usize {
        let r = self.line_range(row.min(self.line_count() - 1));
        self.text[r.clone()]
            .char_indices()
            .nth(column)
            .map(|(p, _)| r.start + p)
            .unwrap_or(r.end)
    }
    pub fn floor_boundary(&self, offset: usize) -> usize {
        let mut p = offset.min(self.len());
        while !self.text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }
    pub fn next(&self, p: usize) -> usize {
        let p = self.floor_boundary(p);
        self.text[p..]
            .chars()
            .next()
            .map(|c| p + c.len_utf8())
            .unwrap_or(p)
    }
    pub fn prev(&self, p: usize) -> usize {
        let p = self.floor_boundary(p);
        self.text[..p]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
    /// Find a neighboring composed-character boundary without scanning the
    /// document or enumerating every grapheme before the caret. A full indexed
    /// line supplies the context needed by regional indicators and ZWJ chains.
    /// Include its terminator so CRLF remains one navigation step.
    fn grapheme_step(&self, offset: usize, forward: bool) -> usize {
        let p = self.floor_boundary(offset);
        let mut row = self.row_at(p);
        if !forward && row > 0 && p == self.line_start(row) {
            row -= 1;
        }
        let range = self.line_full_range(row);
        let line = &self.text[range.clone()];
        let local = p - range.start;
        let mut cursor = GraphemeCursor::new(local, line.len(), true);
        let boundary = if forward {
            cursor.next_boundary(line, 0)
        } else {
            cursor.prev_boundary(line, 0)
        };
        // The complete line is provided, so no additional chunk can be needed.
        range.start
            + boundary
                .expect("complete grapheme context")
                .unwrap_or(local)
    }
    pub fn utf16_offset(&self, byte: usize) -> usize {
        let p = self.floor_boundary(byte);
        let row = self.row_at(p);
        self.utf16_lines[row] + self.text[self.lines[row]..p].encode_utf16().count()
    }
    pub fn byte_offset(&self, utf16: usize) -> usize {
        let row = self
            .utf16_lines
            .partition_point(|&x| x <= utf16)
            .saturating_sub(1);
        let base = self.lines[row];
        let mut units = self.utf16_lines[row];
        for (i, c) in self.text[base..].char_indices() {
            if units + c.len_utf16() > utf16 {
                return base + i;
            }
            units += c.len_utf16();
        }
        self.len()
    }
    pub fn newline(&self) -> &'static str {
        if self.text.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }
    fn replace(&mut self, r: Range<usize>, s: &str) {
        let base_units = self.utf16_offset(r.start);
        let removed_units = self.text[r.clone()].encode_utf16().count();
        let first = self.lines.partition_point(|&x| x <= r.start);
        let last = self.lines.partition_point(|&x| x <= r.end);
        let mut added = Vec::new();
        let mut added_units = Vec::new();
        let mut units = 0;
        for (i, c) in s.char_indices() {
            units += c.len_utf16();
            if c == '\n' {
                added.push(r.start + i + 1);
                added_units.push(base_units + units);
            }
        }
        for p in &mut self.lines[last..] {
            *p = (*p - r.len()) + s.len();
        }
        for p in &mut self.utf16_lines[last..] {
            *p = (*p - removed_units) + units;
        }
        self.lines.splice(first..last, added);
        self.utf16_lines.splice(first..last, added_units);
        self.text.replace_range(r, s);
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Search,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Char(char),
    Escape,
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Ctrl(char),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operator {
    Delete,
    Change,
    Yank,
}
#[derive(Clone, Debug)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
}
#[derive(Clone, Debug)]
struct Transaction {
    edits: Vec<Edit>,
    before: usize,
    after: usize,
    bytes: usize,
}
impl Transaction {
    fn cost(&self) -> usize {
        self.bytes
    }
}
#[derive(Clone, Debug, Default)]
struct Register {
    text: String,
    linewise: bool,
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub buffer: TextBuffer,
    pub cursor: usize,
    pub mode: Mode,
    pub vim: bool,
    pub read_only: bool,
    pub message: String,
    pub search: String,
    anchor: usize,
    count: usize,
    operator: Option<(Operator, usize)>,
    pending_g: bool,
    search_backwards: bool,
    search_origin: usize,
    desired_column: Option<usize>,
    register: Register,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    transaction: Option<Transaction>,
    revision: u64,
}
impl Editor {
    pub fn new(text: String) -> Self {
        let read_only = text.len() > MAX_EDIT_BYTES;
        Self {
            buffer: TextBuffer::new(text),
            cursor: 0,
            mode: Mode::Insert,
            vim: false,
            read_only,
            message: if read_only {
                "Large buffer is read-only (8 MiB editing limit)".into()
            } else {
                String::new()
            },
            search: String::new(),
            anchor: 0,
            count: 0,
            operator: None,
            pending_g: false,
            search_backwards: false,
            search_origin: 0,
            desired_column: None,
            register: Register::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            transaction: None,
            revision: 0,
        }
    }
    pub fn text(&self) -> &str {
        self.buffer.text()
    }
    /// Conservative cache-budget accounting, not measured allocator/RSS bytes.
    /// Includes inline storage, every owned String/Vec capacity (including undo,
    /// redo, the open transaction and register), plus 64 bytes per allocation.
    /// The allowance is a budgeting convention, not an allocator overhead bound.
    pub fn accounted_bytes(&self) -> usize {
        fn allocation(capacity: usize, element_size: usize) -> usize {
            capacity
                .saturating_mul(element_size)
                .saturating_add(if capacity == 0 { 0 } else { 64 })
        }
        fn string(value: &String) -> usize {
            allocation(value.capacity(), 1)
        }
        fn transaction(value: &Transaction) -> usize {
            value.edits.iter().fold(
                allocation(value.edits.capacity(), std::mem::size_of::<Edit>()),
                |bytes, edit| {
                    bytes
                        .saturating_add(string(&edit.removed))
                        .saturating_add(string(&edit.inserted))
                },
            )
        }
        let mut bytes = std::mem::size_of::<Self>();
        for amount in [
            string(&self.buffer.text),
            allocation(self.buffer.lines.capacity(), std::mem::size_of::<usize>()),
            allocation(
                self.buffer.utf16_lines.capacity(),
                std::mem::size_of::<usize>(),
            ),
            string(&self.message),
            string(&self.search),
            string(&self.register.text),
            allocation(self.undo.capacity(), std::mem::size_of::<Transaction>()),
            allocation(self.redo.capacity(), std::mem::size_of::<Transaction>()),
        ] {
            bytes = bytes.saturating_add(amount);
        }
        for value in self
            .undo
            .iter()
            .chain(&self.redo)
            .chain(self.transaction.iter())
        {
            bytes = bytes.saturating_add(transaction(value));
        }
        bytes
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn set_vim(&mut self, enabled: bool) {
        self.finish_transaction();
        self.vim = enabled;
        self.mode = if enabled { Mode::Normal } else { Mode::Insert };
        self.reset_pending();
        self.clamp_cursor();
    }
    pub fn set_cursor(&mut self, offset: usize) {
        self.finish_transaction();
        self.cursor = self.buffer.floor_boundary(offset);
        self.desired_column = None;
        self.clamp_cursor();
    }
    /// Ordinary (non-Vim) Left/Right, with the UI's half-open selection.
    /// Without Shift, a selection collapses toward the requested side without
    /// an extra step. With Shift, its opposite endpoint stays the anchor even
    /// when the caret crosses it. Explicit selection endpoints remain literal.
    pub fn move_horizontal(
        &mut self,
        forward: bool,
        extend: bool,
        selection: Option<Range<usize>>,
    ) -> Option<Range<usize>> {
        self.message.clear();
        self.finish_transaction();
        self.desired_column = None;
        let old = self.cursor;
        let selection = selection
            .map(|r| self.buffer.floor_boundary(r.start)..self.buffer.floor_boundary(r.end));
        if !extend && let Some(range) = selection.as_ref().filter(|r| !r.is_empty()) {
            self.cursor = if forward { range.end } else { range.start };
            return None;
        }
        let anchor = selection
            .as_ref()
            .map(|r| if old == r.start { r.end } else { r.start })
            .unwrap_or(old);
        self.cursor = self.buffer.grapheme_step(old, forward);
        extend.then(|| anchor.min(self.cursor)..anchor.max(self.cursor))
    }
    pub fn selection(&self) -> Option<Range<usize>> {
        match self.mode {
            Mode::Visual => {
                Some(self.anchor.min(self.cursor)..self.buffer.next(self.anchor.max(self.cursor)))
            }
            Mode::VisualLine => {
                let a = self.buffer.row_at(self.anchor.min(self.cursor));
                let b = self.buffer.row_at(self.anchor.max(self.cursor));
                Some(self.buffer.line_start(a)..self.buffer.line_full_range(b).end)
            }
            _ => None,
        }
    }
    pub fn selected_text(&self) -> Option<&str> {
        self.selection().map(|r| &self.text()[r])
    }
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) -> bool {
        let changed = self.replace_range_grouped(range, text);
        self.finish_transaction();
        changed
    }
    pub fn replace_range_grouped(&mut self, range: Range<usize>, text: &str) -> bool {
        let a = self.buffer.floor_boundary(range.start);
        let b = self.buffer.floor_boundary(range.end).max(a);
        self.begin_transaction();
        let changed = self.replace(a..b, text);
        if changed {
            self.cursor = a + text.len();
        }
        self.clamp_cursor();
        changed
    }
    fn begin_transaction(&mut self) {
        if self.transaction.is_none() {
            self.transaction = Some(Transaction {
                edits: Vec::new(),
                before: self.cursor,
                after: self.cursor,
                bytes: 0,
            });
        }
    }
    pub fn finish_transaction(&mut self) {
        if let Some(mut t) = self.transaction.take() {
            if t.edits.is_empty() {
                return;
            }
            t.after = self.cursor;
            self.undo.push(t);
            self.redo.clear();
            let mut bytes: usize = self.undo.iter().map(Transaction::cost).sum();
            while (bytes > UNDO_BUDGET || self.undo.len() > 2_000) && self.undo.len() > 1 {
                bytes -= self.undo.remove(0).cost();
            }
        }
    }
    fn replace(&mut self, r: Range<usize>, s: &str) -> bool {
        if self.read_only {
            self.message = "Read-only buffer".into();
            return false;
        }
        if self.buffer.len() - r.len() + s.len() > MAX_EDIT_BYTES {
            self.message = "Edit exceeds 8 MiB limit".into();
            return false;
        }
        let edit_cost = r.len() + s.len() + 64;
        if self.transaction.as_ref().is_some_and(|t| {
            !t.edits.is_empty()
                && (t.bytes.saturating_add(edit_cost) > UNDO_BUDGET || t.edits.len() >= 10_000)
        }) {
            self.finish_transaction();
        }
        self.begin_transaction();
        let removed = self.text()[r.clone()].to_owned();
        if let Some(t) = self.transaction.as_mut() {
            if let Some(last) = t.edits.last_mut().filter(|e| {
                e.removed.is_empty() && removed.is_empty() && e.start + e.inserted.len() == r.start
            }) {
                last.inserted.push_str(s);
                t.bytes += s.len();
            } else {
                t.bytes += edit_cost;
                t.edits.push(Edit {
                    start: r.start,
                    removed,
                    inserted: s.into(),
                });
            }
        }
        self.buffer.replace(r, s);
        self.revision = self.revision.wrapping_add(1);
        true
    }
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.begin_transaction();
        let r = self.selection().unwrap_or(self.cursor..self.cursor);
        let p = r.start;
        if self.replace(r, text) {
            self.cursor = p + text.len();
        }
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.mode = Mode::Insert;
        }
        self.desired_column = None;
    }
    pub fn undo(&mut self) {
        self.finish_transaction();
        if self.read_only {
            return;
        }
        if let Some(t) = self.undo.pop() {
            for e in t.edits.iter().rev() {
                self.buffer
                    .replace(e.start..e.start + e.inserted.len(), &e.removed);
            }
            self.cursor = t.before;
            self.redo.push(t);
            self.revision = self.revision.wrapping_add(1);
            self.clamp_cursor();
        }
    }
    pub fn redo(&mut self) {
        self.finish_transaction();
        if self.read_only {
            return;
        }
        if let Some(t) = self.redo.pop() {
            for e in &t.edits {
                self.buffer
                    .replace(e.start..e.start + e.removed.len(), &e.inserted);
            }
            self.cursor = t.after;
            self.undo.push(t);
            self.revision = self.revision.wrapping_add(1);
            self.clamp_cursor();
        }
    }
    fn reset_pending(&mut self) {
        self.count = 0;
        self.operator = None;
        self.pending_g = false;
    }
    fn clamp_cursor(&mut self) {
        self.cursor = self.buffer.floor_boundary(self.cursor);
        if self.mode != Mode::Insert {
            let r = self.buffer.line_range(self.buffer.row_at(self.cursor));
            if self.cursor >= r.end && r.end > r.start {
                self.cursor = self.buffer.prev(r.end);
            } else {
                self.cursor = self.cursor.min(r.end);
            }
        }
    }
    fn backspace(&mut self) {
        self.begin_transaction();
        let r = self
            .selection()
            .unwrap_or_else(|| self.deletion_range(true));
        let start = r.start;
        if self.replace(r, "") {
            self.cursor = start;
        }
    }
    /// A CRLF is one line ending for unselected deletion, including when a
    /// platform range placed the caret between its two bytes. Explicit ranges
    /// remain literal: deleting a selection must not consume adjacent text.
    fn deletion_range(&self, backward: bool) -> Range<usize> {
        let mut range = if backward {
            self.buffer.prev(self.cursor)..self.cursor
        } else {
            self.cursor..self.buffer.next(self.cursor)
        };
        let bytes = self.text().as_bytes();
        if range.start > 0
            && bytes.get(range.start) == Some(&b'\n')
            && bytes[range.start - 1] == b'\r'
        {
            range.start -= 1;
        }
        if range.end > 0 && bytes.get(range.end) == Some(&b'\n') && bytes[range.end - 1] == b'\r' {
            range.end += 1;
        }
        range
    }
    fn delete_forward(&mut self, count: usize) {
        self.begin_transaction();
        let mut end = self.cursor;
        let limit = self.buffer.line_range(self.buffer.row_at(end)).end;
        for _ in 0..count {
            end = self.buffer.next(end).min(limit);
        }
        if end > self.cursor {
            self.register = Register {
                text: self.text()[self.cursor..end].into(),
                linewise: false,
            };
            self.replace(self.cursor..end, "");
        }
        self.finish_transaction();
        self.clamp_cursor();
    }
    fn word_class(c: char) -> u8 {
        if c.is_whitespace() {
            0
        } else if c.is_alphanumeric() || c == '_' {
            1
        } else {
            2
        }
    }
    fn word_forward(&self, mut p: usize, count: usize) -> usize {
        for _ in 0..count {
            let class = self.text()[p..]
                .chars()
                .next()
                .map(Self::word_class)
                .unwrap_or(0);
            while p < self.buffer.len()
                && self.text()[p..].chars().next().map(Self::word_class) == Some(class)
            {
                p = self.buffer.next(p);
            }
            while p < self.buffer.len()
                && self.text()[p..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                p = self.buffer.next(p);
            }
        }
        p
    }
    fn word_back(&self, mut p: usize, count: usize) -> usize {
        for _ in 0..count {
            p = self.buffer.prev(p);
            while p > 0
                && self.text()[p..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                p = self.buffer.prev(p);
            }
            let class = self.text()[p..]
                .chars()
                .next()
                .map(Self::word_class)
                .unwrap_or(0);
            while p > 0 {
                let q = self.buffer.prev(p);
                if self.text()[q..].chars().next().map(Self::word_class) != Some(class) {
                    break;
                }
                p = q;
            }
        }
        p
    }
    fn word_end(&self, mut p: usize, count: usize) -> usize {
        for _ in 0..count {
            if p < self.buffer.len() {
                p = self.buffer.next(p);
            }
            while p < self.buffer.len()
                && self.text()[p..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                p = self.buffer.next(p);
            }
            let class = self.text()[p..]
                .chars()
                .next()
                .map(Self::word_class)
                .unwrap_or(0);
            while p < self.buffer.len() {
                let q = self.buffer.next(p);
                if self.text()[q..].chars().next().map(Self::word_class) != Some(class) {
                    break;
                }
                p = q;
            }
        }
        p
    }
    fn motion(&mut self, c: char, n: usize) -> Option<(usize, bool, bool)> {
        let (row, col) = self.buffer.position(self.cursor);
        let line = self.buffer.line_range(row);
        let out = match c {
            'h' => {
                let mut p = self.cursor;
                for _ in 0..n {
                    p = self.buffer.prev(p).max(line.start);
                }
                (p, false, false)
            }
            'l' => {
                let mut p = self.cursor;
                for _ in 0..n {
                    p = self.buffer.next(p).min(line.end);
                }
                (p, false, false)
            }
            'j' | 'k' => {
                let desired = *self.desired_column.get_or_insert(col);
                let r = if c == 'j' {
                    row.saturating_add(n).min(self.buffer.line_count() - 1)
                } else {
                    row.saturating_sub(n)
                };
                (self.buffer.offset(r, desired), false, true)
            }
            '0' => (line.start, false, false),
            '^' => (
                line.start
                    + self.text()[line.clone()]
                        .find(|c: char| !c.is_whitespace())
                        .unwrap_or(0),
                false,
                false,
            ),
            '$' => {
                let r = self
                    .buffer
                    .line_range(row.saturating_add(n - 1).min(self.buffer.line_count() - 1));
                (
                    if r.end > r.start {
                        self.buffer.prev(r.end)
                    } else {
                        r.end
                    },
                    true,
                    false,
                )
            }
            'w' => (self.word_forward(self.cursor, n), false, false),
            'b' => (self.word_back(self.cursor, n), false, false),
            'e' => (self.word_end(self.cursor, n), true, false),
            'G' => {
                let r = if self.count > 0 {
                    n.saturating_sub(1).min(self.buffer.line_count() - 1)
                } else {
                    self.buffer.line_count() - 1
                };
                (self.buffer.line_start(r), false, true)
            }
            _ => return None,
        };
        if c != 'j' && c != 'k' {
            self.desired_column = None;
        }
        Some(out)
    }
    fn operate(&mut self, op: Operator, target: usize, inclusive: bool, linewise: bool) {
        let mut r = self.cursor.min(target)..self.cursor.max(target);
        if linewise {
            let a = self.buffer.row_at(r.start);
            let b = self.buffer.row_at(r.end);
            r = self.buffer.line_start(a)..self.buffer.line_full_range(b).end;
        } else if inclusive && r.end < self.buffer.line_range(self.buffer.row_at(r.end)).end {
            r.end = self.buffer.next(r.end);
        }
        self.apply_operator(op, r, linewise);
    }
    fn apply_operator(&mut self, op: Operator, mut r: Range<usize>, linewise: bool) {
        r.end = r.end.min(self.buffer.len());
        self.register = Register {
            text: self.text()[r.clone()].into(),
            linewise,
        };
        if op == Operator::Yank {
            self.cursor = r.start;
            self.clamp_cursor();
            return;
        }
        self.begin_transaction();
        let newline = self.buffer.newline();
        // Last-line deletion owns the preceding line separator, but the register
        // contains only the deleted line. Change preserves the line being edited.
        if linewise
            && op == Operator::Delete
            && r.end == self.buffer.len()
            && r.start > 0
            && !self.text()[r.clone()].ends_with('\n')
        {
            r.start = self.buffer.prev(r.start);
            if self.text().as_bytes()[r.start] == b'\n'
                && r.start > 0
                && self.text().as_bytes()[r.start - 1] == b'\r'
            {
                r.start -= 1;
            }
        }
        let replacement = if linewise && op == Operator::Change && r.end < self.buffer.len() {
            newline
        } else {
            ""
        };
        let p = r.start;
        if self.replace(r, replacement) {
            self.cursor = p;
        }
        if op == Operator::Change {
            self.mode = Mode::Insert;
        } else {
            self.mode = Mode::Normal;
            self.finish_transaction();
            self.clamp_cursor();
        }
    }
    fn paste(&mut self, before: bool, count: usize) {
        if self.register.text.is_empty() {
            return;
        }
        self.begin_transaction();
        let mut text = self
            .register
            .text
            .repeat(count.min(MAX_EDIT_BYTES / self.register.text.len().max(1)));
        let linewise = self.register.linewise;
        let mut p = if before {
            self.cursor
        } else {
            self.buffer
                .next(self.cursor)
                .min(self.buffer.line_range(self.buffer.row_at(self.cursor)).end)
        };
        if linewise {
            let row = self.buffer.row_at(self.cursor);
            p = if before {
                self.buffer.line_start(row)
            } else {
                self.buffer.line_full_range(row).end
            };
            if !text.ends_with('\n') {
                text.push_str(self.buffer.newline());
            }
            if p == self.buffer.len() && p > 0 && !self.text().ends_with('\n') {
                text.insert_str(0, self.buffer.newline());
            }
        }
        if self.replace(p..p, &text) {
            self.cursor = if linewise && text.starts_with('\n') {
                p + 1
            } else {
                p
            };
        }
        self.finish_transaction();
        self.clamp_cursor();
    }
    pub fn search_next(&mut self, reverse: bool) -> bool {
        if self.search.is_empty() {
            return false;
        }
        let backwards = self.search_backwards ^ reverse;
        let hay = self.text();
        let found = if backwards {
            hay[..self.cursor].rfind(&self.search).or_else(|| {
                hay[self.cursor..]
                    .rfind(&self.search)
                    .map(|p| p + self.cursor)
            })
        } else {
            let from = self.buffer.next(self.cursor);
            hay[from..]
                .find(&self.search)
                .map(|p| p + from)
                .or_else(|| hay[..from].find(&self.search))
        };
        if let Some(p) = found {
            self.cursor = p;
            self.message.clear();
            true
        } else {
            self.message = "Pattern not found".into();
            false
        }
    }
    pub fn key(&mut self, key: Key) {
        self.message.clear();
        if key == Key::Ctrl('r') {
            self.redo();
            return;
        }
        if key == Key::Ctrl('z') {
            self.undo();
            return;
        }
        if key == Key::Escape {
            if self.mode == Mode::Search {
                self.cursor = self.search_origin;
            }
            if self.mode == Mode::Insert && self.vim {
                self.cursor = self
                    .buffer
                    .prev(self.cursor)
                    .max(self.buffer.line_start(self.buffer.row_at(self.cursor)));
            }
            self.mode = if self.vim { Mode::Normal } else { Mode::Insert };
            self.finish_transaction();
            self.reset_pending();
            self.clamp_cursor();
            return;
        }
        if self.mode == Mode::Search {
            match key {
                Key::Char(c) => self.search.push(c),
                Key::Backspace => {
                    self.search.pop();
                }
                Key::Enter => {
                    self.mode = Mode::Normal;
                    self.search_next(false);
                }
                _ => {}
            }
            return;
        }
        if self.mode == Mode::Insert {
            match key {
                Key::Char(c) => self.insert_text(&c.to_string()),
                Key::Enter => self.insert_text(self.buffer.newline()),
                Key::Backspace => self.backspace(),
                Key::Delete => {
                    let range = self.deletion_range(false);
                    let start = range.start;
                    if self.replace(range, "") {
                        self.cursor = start;
                    }
                }
                Key::Left | Key::Right if !self.vim => {
                    self.move_horizontal(key == Key::Right, false, None);
                }
                Key::Left => {
                    self.finish_transaction();
                    self.cursor = self.buffer.prev(self.cursor);
                }
                Key::Right => {
                    self.finish_transaction();
                    self.cursor = self.buffer.next(self.cursor);
                }
                Key::Up | Key::Down => {
                    self.finish_transaction();
                    if let Some((p, _, _)) = self.motion(if key == Key::Up { 'k' } else { 'j' }, 1)
                    {
                        self.cursor = p;
                    }
                }
                _ => {}
            }
            return;
        }
        let c = match key {
            Key::Char(c) => c,
            Key::Left => 'h',
            Key::Right => 'l',
            Key::Up => 'k',
            Key::Down => 'j',
            Key::Delete => 'x',
            _ => return,
        };
        if c.is_ascii_digit() && (c != '0' || self.count > 0) {
            self.count = self
                .count
                .saturating_mul(10)
                .saturating_add(c.to_digit(10).unwrap() as usize)
                .min(MAX_COUNT);
            return;
        }
        let n = self.count.max(1);
        let total = self
            .operator
            .map(|(_, m)| m.saturating_mul(n).min(MAX_COUNT))
            .unwrap_or(n);
        if self.pending_g {
            self.pending_g = false;
            if c == 'g' {
                let p = self.buffer.line_start(n - 1);
                if let Some((op, _)) = self.operator.take() {
                    self.operate(op, p, false, true);
                } else {
                    self.cursor = p;
                }
                self.count = 0;
                return;
            }
            self.reset_pending();
            return;
        }
        if c == 'g' {
            self.pending_g = true;
            return;
        }
        if let Some((op, _)) = self.operator {
            let same = matches!(
                (op, c),
                (Operator::Delete, 'd') | (Operator::Yank, 'y') | (Operator::Change, 'c')
            );
            if same {
                let row = self.buffer.row_at(self.cursor);
                let end = self
                    .buffer
                    .line_full_range((row + total - 1).min(self.buffer.line_count() - 1))
                    .end;
                self.apply_operator(op, self.buffer.line_start(row)..end, true);
                self.reset_pending();
                return;
            }
        }
        if let Some((mut p, mut inclusive, linewise)) = self.motion(c, total) {
            if let Some((op, _)) = self.operator.take() {
                if op == Operator::Change
                    && c == 'w'
                    && self.text()[self.cursor..]
                        .chars()
                        .next()
                        .is_some_and(|c| !c.is_whitespace())
                {
                    p = self.cursor;
                    let class = self.text()[p..].chars().next().map(Self::word_class);
                    while p < self.buffer.len() {
                        let next = self.buffer.next(p);
                        if self.text()[next..].chars().next().map(Self::word_class) != class {
                            break;
                        }
                        p = next;
                    }
                    if total > 1 {
                        p = self.word_end(p, total - 1);
                    }
                    inclusive = true;
                }
                self.operate(op, p, inclusive, linewise);
            } else {
                self.cursor = p;
                self.clamp_cursor();
            }
            self.count = 0;
            return;
        }
        match c {
            'd' | 'c' | 'y' => {
                let op = match c {
                    'd' => Operator::Delete,
                    'c' => Operator::Change,
                    _ => Operator::Yank,
                };
                if let Some(r) = self.selection() {
                    let linewise = self.mode == Mode::VisualLine;
                    self.apply_operator(op, r, linewise);
                    if op == Operator::Yank {
                        self.mode = Mode::Normal;
                    }
                    self.reset_pending();
                } else {
                    self.operator = Some((op, n));
                    self.count = 0;
                }
                return;
            }
            'i' | 'a' | 'I' | 'A' => {
                self.begin_transaction();
                self.mode = Mode::Insert;
                let r = self.buffer.line_range(self.buffer.row_at(self.cursor));
                self.cursor = match c {
                    'a' => self.buffer.next(self.cursor).min(r.end),
                    'I' => r.start,
                    'A' => r.end,
                    _ => self.cursor,
                };
            }
            'o' | 'O' => {
                self.begin_transaction();
                let row = self.buffer.row_at(self.cursor);
                let p = if c == 'O' {
                    self.buffer.line_start(row)
                } else {
                    self.buffer.line_range(row).end
                };
                let newline = self.buffer.newline();
                if self.replace(p..p, newline) {
                    self.cursor = if c == 'O' { p } else { p + newline.len() };
                }
                self.mode = Mode::Insert;
            }
            'v' | 'V' => {
                let mode = if c == 'v' {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
                if self.mode == mode {
                    self.mode = Mode::Normal;
                } else {
                    self.anchor = self.cursor;
                    self.mode = mode;
                }
            }
            'x' => {
                if let Some(r) = self.selection() {
                    let linewise = self.mode == Mode::VisualLine;
                    self.apply_operator(Operator::Delete, r, linewise);
                } else {
                    self.delete_forward(n);
                }
            }
            'X' => {
                let start = self.buffer.line_start(self.buffer.row_at(self.cursor));
                let mut p = self.cursor;
                for _ in 0..n {
                    p = self.buffer.prev(p).max(start);
                }
                self.apply_operator(Operator::Delete, p..self.cursor, false);
            }
            'D' | 'C' => {
                let r = self.cursor
                    ..self
                        .buffer
                        .line_range(
                            (self.buffer.row_at(self.cursor) + n - 1)
                                .min(self.buffer.line_count() - 1),
                        )
                        .end;
                self.apply_operator(
                    if c == 'D' {
                        Operator::Delete
                    } else {
                        Operator::Change
                    },
                    r,
                    false,
                );
            }
            'p' | 'P' => self.paste(c == 'P', n),
            'u' => {
                for _ in 0..n.min(self.undo.len()) {
                    self.undo();
                }
            }
            '/' | '?' => {
                self.search.clear();
                self.search_origin = self.cursor;
                self.search_backwards = c == '?';
                self.mode = Mode::Search;
            }
            'n' | 'N' => {
                for _ in 0..n {
                    self.search_next(c == 'N');
                }
            }
            _ => {
                self.message = format!("Unsupported Vim key: {c}");
            }
        }
        self.reset_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vim(text: &str) -> Editor {
        let mut e = Editor::new(text.into());
        e.set_vim(true);
        e
    }
    fn keys(e: &mut Editor, s: &str) {
        for c in s.chars() {
            e.key(Key::Char(c));
        }
    }
    #[test]
    fn retained_accounting_includes_spare_capacity_and_all_history() {
        let mut editor = Editor::new("a\r\n😀".into());
        let baseline = editor.accounted_bytes();
        editor.buffer.text.reserve(4096);
        editor.buffer.lines.reserve(1024);
        editor.buffer.utf16_lines.reserve(1024);
        editor.message.reserve(1024);
        editor.search.reserve(1024);
        editor.register.text.reserve(1024);
        assert!(editor.accounted_bytes() >= baseline + 8192);
        let empty_history = editor.accounted_bytes();
        editor.set_cursor(editor.text().len());
        editor.insert_text("pending");
        let pending = editor.accounted_bytes();
        assert!(pending > empty_history);
        editor.finish_transaction();
        assert!(editor.accounted_bytes() >= pending);
        let committed = editor.accounted_bytes();
        editor.undo();
        assert!(editor.accounted_bytes() >= committed);
        assert!(!editor.redo.is_empty());
        assert!(editor.transaction.is_none());
        // Capacity, not length: deleting the register must not hide its allocation.
        let retained = editor.accounted_bytes();
        editor.register.text.clear();
        assert_eq!(editor.accounted_bytes(), retained);
    }
    #[test]
    fn ordinary_arrows_follow_composed_characters_from_every_scalar_boundary() {
        use unicode_segmentation::UnicodeSegmentation;
        for text in [
            "",
            "A",
            "Ae\u{301}B",
            "A👩‍💻B",
            "A🇺🇸🇨🇦🇯B",
            "A👍🏽B",
            "a\r\nb\nc\r\n",
            "\r\n\r\n",
            "a\r\r\nb",
            "क्‍षx",
            "e\u{301}\u{302}x",
        ] {
            let boundaries: Vec<_> = text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
                .collect();
            for p in text
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
            {
                for forward in [false, true] {
                    let expected = if forward {
                        boundaries.iter().copied().find(|&b| b > p).unwrap_or(p)
                    } else {
                        boundaries
                            .iter()
                            .copied()
                            .rev()
                            .find(|&b| b < p)
                            .unwrap_or(p)
                    };
                    let mut e = Editor::new(text.into());
                    e.set_cursor(p);
                    e.key(if forward { Key::Right } else { Key::Left });
                    assert_eq!(e.cursor, expected, "{text:?} at {p}, forward={forward}");
                    assert_eq!(e.text(), text);
                    assert_eq!(e.revision(), 0);
                }
            }
        }
    }
    #[test]
    fn ordinary_selection_collapses_to_requested_side_without_a_step() {
        let text = "Ae\u{301}👩‍💻B\r\nC";
        // Endpoints can also originate from a literal platform/IME range
        // inside a composed character or between CR and LF.
        for range in [1..15, 2..4, 17..18, 0..text.len()] {
            for caret in [range.start, range.end] {
                for forward in [false, true] {
                    let mut e = Editor::new(text.into());
                    e.set_cursor(caret);
                    assert_eq!(e.move_horizontal(forward, false, Some(range.clone())), None);
                    assert_eq!(e.cursor, if forward { range.end } else { range.start });
                }
            }
        }
    }
    #[test]
    fn ordinary_shift_arrows_keep_anchor_when_shrinking_and_crossing() {
        let mut e = Editor::new("Ae\u{301}👩‍💻B".into());
        e.set_cursor(4);
        let mut selection = None;
        for (forward, cursor, expected) in [
            (false, 1, 1..4),
            (false, 0, 0..4),
            (false, 0, 0..4),
            (true, 1, 1..4),
            (true, 4, 4..4),
            (true, 15, 4..15),
            (true, 16, 4..16),
            (true, 16, 4..16),
            (false, 15, 4..15),
            (false, 4, 4..4),
            (false, 1, 1..4),
        ] {
            selection = e.move_horizontal(forward, true, selection);
            assert_eq!(e.cursor, cursor);
            assert_eq!(selection, Some(expected));
        }
    }
    #[test]
    fn ordinary_shift_extends_existing_selection_in_either_direction() {
        for (caret, forward, cursor, expected) in [
            (1, false, 0, 0..4),
            (1, true, 4, 4..4),
            (4, false, 1, 1..1),
            (4, true, 15, 1..15),
        ] {
            let mut e = Editor::new("Ae\u{301}👩‍💻B".into());
            e.set_cursor(caret);
            let selection = e.move_horizontal(forward, true, Some(1..4));
            assert_eq!(e.cursor, cursor);
            assert_eq!(selection, Some(expected));
        }
    }
    #[test]
    fn ordinary_navigation_preserves_literal_utf16_replacement_and_undo() {
        let mut e = Editor::new("Ae\u{301}B\r\nC".into());
        let range = e.buffer.byte_offset(2)..e.buffer.byte_offset(3);
        assert_eq!(range, 2..4);
        assert!(e.replace_range(range, "x"));
        assert_eq!(e.text(), "AexB\r\nC");
        e.move_horizontal(false, true, None);
        e.undo();
        assert_eq!(e.text(), "Ae\u{301}B\r\nC");
        assert_eq!(e.buffer.byte_offset(5), 6); // literal position inside CRLF
        e.set_cursor(6);
        e.key(Key::Right);
        assert_eq!(e.cursor, 7);
    }
    #[test]
    fn ordinary_navigation_commits_typing_and_works_read_only() {
        let mut e = Editor::new("A".into());
        e.set_cursor(1);
        e.insert_text("e\u{301}");
        e.key(Key::Left);
        assert_eq!(e.cursor, 1);
        e.insert_text("x");
        e.undo();
        assert_eq!(e.text(), "Ae\u{301}");
        e.undo();
        assert_eq!(e.text(), "A");
        e.read_only = true;
        assert_eq!(e.move_horizontal(false, true, None), Some(0..1));
        assert_eq!(e.cursor, 0);
    }
    #[test]
    fn ordinary_grapheme_lookup_handles_long_lines_and_regional_context() {
        let prefix = "a".repeat(1_000_000);
        let text = format!("older\r\n{prefix}e\u{301}{}z\r\nlast", "🇺".repeat(257));
        let mut e = Editor::new(text);
        let base = 7 + prefix.len();
        e.set_cursor(base);
        e.key(Key::Right);
        assert_eq!(e.cursor, base + 3);
        e.set_cursor(base + 3 + 256 * 4);
        e.key(Key::Left);
        assert_eq!(e.cursor, base + 3 + 254 * 4);
        e.key(Key::Right);
        e.key(Key::Right);
        assert_eq!(e.cursor, base + 3 + 257 * 4);
        e.key(Key::Right);
        e.key(Key::Right);
        assert_eq!(e.cursor, e.buffer.line_start(2));
        e.key(Key::Left);
        assert_eq!(e.cursor, e.buffer.line_start(2) - 2);
    }
    #[test]
    fn vim_horizontal_motions_remain_scalar_based() {
        let mut e = vim("Ae\u{301}👩‍💻B");
        for expected in [1, 2, 4, 8, 11, 15] {
            e.key(Key::Right);
            assert_eq!(e.cursor, expected);
        }
        e.key(Key::Char('i'));
        e.set_cursor(4);
        e.key(Key::Left);
        assert_eq!(e.cursor, 2);
        e.key(Key::Right);
        assert_eq!(e.cursor, 4);
        e.key(Key::Right);
        assert_eq!(e.cursor, 8);
    }
    #[test]
    fn incremental_index_matches_rebuild() {
        let mut b = TextBuffer::new("a😀\r\n中\nlast".into());
        for (r, s) in [(1..5, "x\n😀"), (0..1, "\n"), (3..3, "abc"), (0..8, "")] {
            let r = b.floor_boundary(r.start)..b.floor_boundary(r.end);
            b.replace(r, s);
            let rebuilt = TextBuffer::new(b.text().to_owned());
            assert_eq!(b.lines, rebuilt.lines);
            assert_eq!(b.utf16_lines, rebuilt.utf16_lines);
        }
    }
    #[test]
    fn delete_to_end_on_empty_line_preserves_newline() {
        let mut e = vim("\nnext");
        keys(&mut e, "d$");
        assert_eq!(e.text(), "\nnext");
    }
    #[test]
    fn change_single_letter_word_does_not_eat_next_word() {
        let mut e = vim("a next");
        keys(&mut e, "cwX");
        e.key(Key::Escape);
        assert_eq!(e.text(), "X next");
    }
    #[test]
    fn rejected_edit_reports_failure_without_cursor_change() {
        let mut e = Editor::new("x".into());
        let huge = "a".repeat(MAX_EDIT_BYTES + 1);
        assert!(!e.replace_range(0..0, &huge));
        assert_eq!(e.cursor, 0);
        assert_eq!(e.text(), "x");
    }
    #[test]
    fn insert_transaction_is_memory_bounded() {
        let mut e = Editor::new(String::new());
        for _ in 0..12_000 {
            e.insert_text("x");
            e.key(Key::Backspace);
        }
        assert!(
            e.transaction
                .as_ref()
                .is_none_or(|t| t.edits.len() <= 10_000)
        );
        assert!(e.undo.iter().map(Transaction::cost).sum::<usize>() <= UNDO_BUDGET);
    }
    #[test]
    fn unicode_boundaries_and_utf16() {
        let b = TextBuffer::new("a😀中\r\nnext".into());
        assert_eq!(b.offset(0, 2), 5);
        assert_eq!(b.byte_offset(3), 5);
        assert_eq!(b.utf16_offset(5), 3);
        assert_eq!(b.line(0), "a😀中");
        assert_eq!(b.position(10), (1, 0));
    }
    #[test]
    fn counted_delete_and_undo() {
        let mut e = vim("one two three four\n");
        keys(&mut e, "2dw");
        assert_eq!(e.text(), "three four\n");
        keys(&mut e, "u");
        assert_eq!(e.text(), "one two three four\n");
        e.key(Key::Ctrl('r'));
        assert_eq!(e.text(), "three four\n");
    }
    #[test]
    fn multiplied_operator_count() {
        let mut e = vim("a b c d e f g");
        keys(&mut e, "2d3w");
        assert_eq!(e.text(), "g");
    }
    #[test]
    fn insert_undo_is_transaction() {
        let mut e = vim("cat");
        keys(&mut e, "iHello ");
        e.key(Key::Escape);
        assert_eq!(e.text(), "Hello cat");
        keys(&mut e, "u");
        assert_eq!(e.text(), "cat");
    }
    #[test]
    fn change_undo_keeps_delete_and_insert_together() {
        let mut e = vim("one two");
        keys(&mut e, "cwnew");
        e.key(Key::Escape);
        assert_eq!(e.text(), "new two");
        keys(&mut e, "u");
        assert_eq!(e.text(), "one two");
    }
    #[test]
    fn lines_yank_delete_paste() {
        let mut e = vim("a\nb\nc");
        keys(&mut e, "yyjp");
        assert_eq!(e.text(), "a\nb\na\nc");
        keys(&mut e, "Gdd");
        assert_eq!(e.text(), "a\nb\na");
        keys(&mut e, "u");
        assert_eq!(e.text(), "a\nb\na\nc");
    }
    #[test]
    fn visual_inclusive_unicode() {
        let mut e = vim("a😀中z");
        keys(&mut e, "lvld");
        assert_eq!(e.text(), "az");
        keys(&mut e, "u");
        assert_eq!(e.text(), "a😀中z");
    }
    #[test]
    fn search_wrap_and_reverse() {
        let mut e = vim("red blue red blue");
        keys(&mut e, "/red");
        e.key(Key::Enter);
        assert_eq!(e.cursor, 9);
        keys(&mut e, "n");
        assert_eq!(e.cursor, 0);
        keys(&mut e, "N");
        assert_eq!(e.cursor, 9);
    }
    #[test]
    fn vertical_column_restores_after_short_line() {
        let mut e = vim("abcdef\nx\nabcdef");
        keys(&mut e, "4ljj");
        assert_eq!(e.buffer.position(e.cursor), (2, 4));
    }
    #[test]
    fn readonly_does_not_mutate() {
        let mut e = vim("safe");
        e.read_only = true;
        keys(&mut e, "ddiunsafe");
        assert_eq!(e.text(), "safe");
    }
    #[test]
    fn unselected_deletion_joins_crlf_atomically_and_undo_restores_bytes() {
        for vim_insert in [false, true] {
            for (key, cursor) in [
                (Key::Backspace, 6),
                (Key::Delete, 4),
                (Key::Backspace, 5),
                (Key::Delete, 5),
            ] {
                let mut e = Editor::new("😀\r\n中".into());
                if vim_insert {
                    e.set_vim(true);
                    e.key(Key::Char('i'));
                }
                e.set_cursor(cursor);
                e.key(key);
                assert_eq!(e.text(), "😀中");
                assert_eq!(e.cursor, 4);
                assert_eq!(e.buffer.line_count(), 1);
                e.undo();
                assert_eq!(e.text(), "😀\r\n中");
                assert_eq!(e.cursor, cursor);
                e.redo();
                assert_eq!(e.text(), "😀中");
                assert_eq!(e.cursor, 4);
            }
        }
    }
    #[test]
    fn newline_deletion_preserves_single_cr_lf_and_document_boundaries() {
        for newline in ["\n", "\r", "\r\n"] {
            for (key, cursor) in [(Key::Backspace, 1 + newline.len()), (Key::Delete, 1)] {
                let mut e = Editor::new(format!("a{newline}b"));
                e.set_cursor(cursor);
                e.key(key);
                assert_eq!(e.text(), "ab");
                e.undo();
                assert_eq!(e.text(), format!("a{newline}b"));
            }
        }
        for (key, cursor) in [(Key::Backspace, 0), (Key::Delete, 2)] {
            let mut e = Editor::new("\r\n".into());
            e.set_cursor(cursor);
            e.key(key);
            assert_eq!(e.text(), "\r\n");
        }
    }
    #[test]
    fn explicit_selection_deletion_is_literal_and_read_only_keeps_caret() {
        // The UI routes an explicit selection through replace_range.
        for (selection, expected) in [(1..3, "ab"), (1..2, "a\nb"), (2..3, "a\rb")] {
            let mut e = Editor::new("a\r\nb".into());
            e.replace_range(selection, "");
            assert_eq!(e.text(), expected);
            e.undo();
            assert_eq!(e.text(), "a\r\nb");
        }
        for (key, cursor) in [(Key::Backspace, 3), (Key::Delete, 1)] {
            let mut e = Editor::new("a\r\nb".into());
            e.set_cursor(cursor);
            e.read_only = true;
            e.key(key);
            assert_eq!(e.text(), "a\r\nb");
            assert_eq!(e.cursor, cursor);
        }
    }
    #[test]
    fn crlf_insert_and_delete() {
        let mut e = vim("a\r\nb\r\nc");
        keys(&mut e, "johello");
        e.key(Key::Escape);
        assert_eq!(e.text(), "a\r\nb\r\nhello\r\nc");
        keys(&mut e, "Gdd");
        assert_eq!(e.text(), "a\r\nb\r\nhello");
    }
    #[test]
    fn no_panic_on_empty_or_unicode_sequences() {
        for seed in ["", "😀\n中\n", "\r\n", "x"] {
            let mut e = vim(seed);
            for _ in 0..10 {
                keys(&mut e, "99j99k$0weddbGggvlypux");
                e.key(Key::Escape);
            }
            assert!(e.text().is_char_boundary(e.cursor));
        }
    }
}
