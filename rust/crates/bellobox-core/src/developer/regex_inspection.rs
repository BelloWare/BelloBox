//! Bounded full-window regex inspection. Pattern syntax is Rust regex, not ICU.
//! UI details use Swift's UTF-16 offsets; editor decoration uses UTF-8 offsets.
//! Replacement templates use numbered $0/$1 groups and backslash escaping,
//! independently of the legacy CLI's Rust replacement syntax.
use regex::RegexBuilder;
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const INPUT_LIMIT: usize = 100_000;
pub const PATTERN_LIMIT: usize = 4_000;
pub const REPLACEMENT_LIMIT: usize = 100_000;
pub const MATCH_LIMIT: usize = 1_000;
pub const GROUP_LIMIT: usize = 128;
pub const OUTPUT_LIMIT: usize = 4_000_000;
const BUDGET: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub ignore_case: bool,
    pub multiline: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            ignore_case: false,
            multiline: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Matches,
    Extract,
    Replace,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub bytes: Range<usize>,
    pub utf16: Range<usize>,
    /// Includes group zero. Unmatched optional groups remain None.
    pub groups: Vec<Option<Range<usize>>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub matches: Vec<Match>,
    pub details: String,
    pub extracted: String,
    pub replaced: String,
}
impl Inspection {
    pub fn output(&self, mode: Mode) -> &str {
        match mode {
            Mode::Matches => &self.details,
            Mode::Extract => &self.extracted,
            Mode::Replace => &self.replaced,
        }
    }
}
pub fn validate(text: &str, pattern: &str, replacement: &str) -> Result<(), String> {
    if text.len() > INPUT_LIMIT
        || pattern.len() > PATTERN_LIMIT
        || replacement.len() > REPLACEMENT_LIMIT
    {
        Err("Regex limits: 100000 UTF-8 input bytes, 4000 pattern bytes, 100000 replacement bytes. Nothing was truncated.".into())
    } else {
        Ok(())
    }
}
struct Guard<'a> {
    cancelled: &'a AtomicBool,
    started: Instant,
    budget: Duration,
}
impl Guard<'_> {
    fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err("Regex inspection cancelled.".into());
        }
        if self.started.elapsed() >= self.budget {
            return Err("Stopped after the local regex time budget. Narrow the text or simplify the pattern.".into());
        }
        Ok(())
    }
}
/// Checks cancellation/deadline between bounded phases and matches. An individual
/// regex-library compile/search call cannot be forcibly interrupted.
pub fn inspect(
    text: &str,
    pattern: &str,
    replacement: &str,
    options: Options,
    cancelled: &AtomicBool,
) -> Result<Inspection, String> {
    inspect_with_budget(text, pattern, replacement, options, cancelled, BUDGET)
}
fn inspect_with_budget(
    text: &str,
    pattern: &str,
    replacement: &str,
    options: Options,
    cancelled: &AtomicBool,
    budget: Duration,
) -> Result<Inspection, String> {
    validate(text, pattern, replacement)?;
    let guard = Guard {
        cancelled,
        started: Instant::now(),
        budget,
    };
    guard.check()?;
    if pattern.is_empty() {
        return Err("Enter a regular expression.".into());
    }
    let regex = RegexBuilder::new(pattern).case_insensitive(options.ignore_case).multi_line(options.multiline)
        .size_limit(2_000_000).dfa_size_limit(2_000_000).build()
        .map_err(|e| format!("Regex error: {e}. Portable Rust patterns do not support lookaround or backreferences."))?;
    guard.check()?;
    if regex.captures_len() > GROUP_LIMIT + 1 {
        return Err("Regex supports at most 128 capture groups.".into());
    }
    // Input admission precedes every input-sized allocation. Only valid scalar
    // boundaries are queried; byte interiors deliberately have no mapping.
    let mut utf16 = vec![0usize; text.len() + 1];
    let mut units = 0;
    for (byte, scalar) in text.char_indices() {
        utf16[byte] = units;
        units += scalar.len_utf16();
    }
    utf16[text.len()] = units;
    let mut matches = Vec::new();
    let mut search_start = 0;
    while let Some(captures) = regex.captures_at(text, search_start) {
        guard.check()?;
        if matches.len() == MATCH_LIMIT {
            return Err("More than 1000 matches; narrow the expression.".into());
        }
        let whole = captures.get(0).expect("regex always has group zero");
        matches.push(Match {
            bytes: whole.range(),
            utf16: utf16[whole.start()]..utf16[whole.end()],
            groups: captures.iter().map(|c| c.map(|m| m.range())).collect(),
        });
        // Foundation reports a zero-width match adjacent to a nonempty match.
        // Rust captures_iter deliberately suppresses that case. Advance only
        // after an empty match, at a complete Unicode scalar boundary.
        if whole.is_empty() {
            if whole.end() == text.len() {
                break;
            }
            search_start = whole.end() + text[whole.end()..].chars().next().unwrap().len_utf8();
        } else {
            search_start = whole.end();
        }
    }
    guard.check()?;
    let template = template_parts(replacement, regex.captures_len());
    let mut result = Inspection {
        matches,
        details: String::new(),
        extracted: String::new(),
        replaced: String::new(),
    };
    let mut remaining = OUTPUT_LIMIT;
    let mut end = 0;
    for (index, m) in result.matches.iter().enumerate() {
        guard.check()?;
        if index != 0 {
            append(&mut result.details, "\n", &mut remaining)?;
            append(&mut result.extracted, "\n", &mut remaining)?;
        }
        append(
            &mut result.details,
            &format!("{}. [{}..<{}] ", index + 1, m.utf16.start, m.utf16.end),
            &mut remaining,
        )?;
        append(&mut result.details, &text[m.bytes.clone()], &mut remaining)?;
        for (group, range) in m.groups.iter().enumerate().skip(1) {
            append(
                &mut result.details,
                &format!("\n   ${group}: "),
                &mut remaining,
            )?;
            append(
                &mut result.details,
                range
                    .as_ref()
                    .map(|r| &text[r.clone()])
                    .unwrap_or("(not matched)"),
                &mut remaining,
            )?;
        }
        append(
            &mut result.extracted,
            &text[m.bytes.clone()],
            &mut remaining,
        )?;
        append(
            &mut result.replaced,
            &text[end..m.bytes.start],
            &mut remaining,
        )?;
        for part in &template {
            guard.check()?;
            let value = match part {
                TemplatePart::Literal(range) => &replacement[range.clone()],
                TemplatePart::Group(group) => m
                    .groups
                    .get(*group)
                    .and_then(|r| r.as_ref())
                    .map(|r| &text[r.clone()])
                    .unwrap_or(""),
            };
            append(&mut result.replaced, value, &mut remaining)?;
        }
        end = m.bytes.end;
    }
    append(&mut result.replaced, &text[end..], &mut remaining)?;
    if result.matches.is_empty() {
        append(&mut result.details, "No matches.", &mut remaining)?;
    }
    guard.check()?;
    Ok(result)
}
fn append(output: &mut String, value: &str, remaining: &mut usize) -> Result<(), String> {
    if value.len() > *remaining {
        return Err(
            "Combined regex outputs exceed 4000000 UTF-8 bytes. Nothing was truncated.".into(),
        );
    }
    *remaining -= value.len();
    output.push_str(value);
    Ok(())
}
enum TemplatePart {
    Literal(Range<usize>),
    Group(usize),
}
// Parse once, before expanding into any result. Parts borrow bounded template
// slices, so repeated large groups cannot allocate an unchecked intermediate.
fn template_parts(template: &str, ranges: usize) -> Vec<TemplatePart> {
    let digits = (ranges.saturating_sub(1).max(1).ilog10() + 1) as usize;
    let b = template.as_bytes();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 1;
            if i < b.len() {
                let len = template[i..].chars().next().unwrap().len_utf8();
                parts.push(TemplatePart::Literal(i..i + len));
                i += len;
            }
        } else if b[i] == b'$' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            i += 1;
            let start = i;
            let mut group = 0;
            while i < b.len() && i - start < digits && b[i].is_ascii_digit() {
                group = group * 10 + usize::from(b[i] - b'0');
                i += 1;
            }
            parts.push(TemplatePart::Group(group));
        } else {
            let start = i;
            i += template[i..].chars().next().unwrap().len_utf8();
            while i < b.len() && !matches!(b[i], b'$' | b'\\') {
                i += template[i..].chars().next().unwrap().len_utf8();
            }
            parts.push(TemplatePart::Literal(start..i));
        }
    }
    parts
}
#[cfg(test)]
#[path = "regex_inspection_tests.rs"]
mod tests;
