//! Bounded display parsing. The raw Git patch remains authoritative; unusual
//! headers are preserved as metadata rather than guessed as changed lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffLineKind {
    Header,
    Hunk,
    Context,
    Added,
    Removed,
    Note,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct ParsedDiff {
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub added: usize,
    pub removed: usize,
}
impl ParsedDiff {
    pub fn parse(text: &str, max_lines: usize) -> Self {
        let mut result = Self {
            lines: Vec::new(),
            truncated: false,
            added: 0,
            removed: 0,
        };
        let (mut old, mut new) = (0, 0);
        let mut hunk = false;
        for line in text.lines() {
            if result.lines.len() >= max_lines {
                result.truncated = true;
                break;
            }
            let (kind, o, n, body) = if line.starts_with("diff --git ") {
                hunk = false;
                (DiffLineKind::Header, None, None, line)
            } else if line.starts_with("@@ ") {
                let mut p = line.split_whitespace();
                p.next();
                let a = p.next().and_then(start);
                let b = p.next().and_then(start);
                if let (Some(a), Some(b)) = (a, b) {
                    old = a;
                    new = b;
                    hunk = true;
                } else {
                    hunk = false;
                }
                (DiffLineKind::Hunk, None, None, line)
            } else if hunk && line.starts_with('+') {
                let n = new;
                new = new.saturating_add(1);
                result.added += 1;
                (DiffLineKind::Added, None, Some(n), &line[1..])
            } else if hunk && line.starts_with('-') {
                let o = old;
                old = old.saturating_add(1);
                result.removed += 1;
                (DiffLineKind::Removed, Some(o), None, &line[1..])
            } else if hunk && line.starts_with(' ') {
                let (o, n) = (old, new);
                old = old.saturating_add(1);
                new = new.saturating_add(1);
                (DiffLineKind::Context, Some(o), Some(n), &line[1..])
            } else {
                (
                    if line.starts_with('\\') {
                        DiffLineKind::Note
                    } else {
                        DiffLineKind::Header
                    },
                    None,
                    None,
                    line,
                )
            };
            result.lines.push(DiffLine {
                kind,
                old_line: o,
                new_line: n,
                text: body.into(),
            });
        }
        result
    }
}
fn start(token: &str) -> Option<usize> {
    token.get(1..)?.split(',').next()?.parse().ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbers_and_headers() {
        let d = ParsedDiff::parse(
            "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1,2 +1,2 @@\n same\n-old\n+new\n\\ No newline at end of file\n",
            100,
        );
        assert_eq!((d.added, d.removed), (1, 1));
        assert_eq!(d.lines[6].new_line, Some(2));
        assert_eq!(d.lines[7].kind, DiffLineKind::Note);
        assert!(!d.truncated);
    }
    #[test]
    fn cap_is_visible() {
        let d = ParsedDiff::parse("a\nb\nc", 2);
        assert_eq!(d.lines.len(), 2);
        assert!(d.truncated);
    }
    #[test]
    fn zero_start_new_file() {
        let d = ParsedDiff::parse("@@ -0,0 +1 @@\n+new", 3);
        assert_eq!(d.lines[1].new_line, Some(1));
    }
}
