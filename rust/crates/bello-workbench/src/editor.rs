//! A deliberately documented Vim subset, not a Vim emulator. Byte offsets are
//! always UTF-8 boundaries; motions count Unicode scalar values (not graphemes).
use std::ops::Range;

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
            .unwrap_or_else(|| self.buffer.prev(self.cursor)..self.cursor);
        self.cursor = r.start;
        self.replace(r, "");
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
                    let p = self.cursor;
                    self.replace(p..self.buffer.next(p), "");
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
