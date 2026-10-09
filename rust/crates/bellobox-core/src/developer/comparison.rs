//! Bounded full-window comparison, independent of the legacy `execute("compare")` route.
//!
//! Tokenization follows the pinned Swift TextComparison source. Alignment follows
//! the linear-space Myers traversal in Swift 6.3.3's CollectionDifference:
//! https://github.com/swiftlang/swift/blob/064859e41d68596f486c5d724401cb370f260409/stdlib/public/core/Diffing.swift
//! The native differential corpus, not this implementation, establishes runtime
//! agreement. Newer Swift runtimes and Unicode versions may choose differently.
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

pub const INPUT_LIMIT: usize = 500_000;
pub const OUTPUT_LIMIT: usize = 4_000_000;
pub const TOKEN_LIMIT: usize = 8_000;
const TOKEN_ERROR: &str =
    "Compare up to 8,000 combined lines, words, or JSON fields. Narrow the selection first.";
const OUTPUT_ERROR: &str = "Result exceeds 4,000,000 UTF-8 bytes; nothing was truncated.";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Lines,
    Words,
    Json,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub ignore_whitespace: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowKind {
    Same,
    Added,
    Removed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub kind: RowKind,
    pub text: String,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComparisonResult {
    pub rows: Vec<Row>,
    pub added: usize,
    pub removed: usize,
    pub copy_text: String,
}

pub(super) fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        Err("Comparison cancelled.".into())
    } else {
        Ok(())
    }
}

/// No partial output is returned on cancellation or any resource/parse error.
pub fn compare(
    left: &str,
    right: &str,
    options: Options,
    cancelled: &AtomicBool,
) -> Result<ComparisonResult, String> {
    check_cancelled(cancelled)?;
    if left.len().saturating_add(right.len()) > INPUT_LIMIT {
        return Err("Inputs exceed 500,000 UTF-8 bytes; nothing was truncated.".into());
    }
    let a = tokens(left, options, TOKEN_LIMIT, cancelled)?;
    let b = tokens(right, options, TOKEN_LIMIT - a.len(), cancelled)?;
    // Comparing IDs avoids O(token bytes) work in every diff cell. Original text
    // stays separate, including the left-hand spelling of canonical matches.
    let (a_ids, b_ids) = identities(&a, &b, cancelled)?;
    let (removed, added) = changes(&a_ids, &b_ids, cancelled)?;
    let mut result = ComparisonResult::default();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        check_cancelled(cancelled)?;
        let (text, kind) = if i < a.len() && removed[i] {
            let text = &a[i];
            i += 1;
            (text, RowKind::Removed)
        } else if j < b.len() && added[j] {
            let text = &b[j];
            j += 1;
            (text, RowKind::Added)
        } else if i < a.len() && j < b.len() {
            let text = &a[i];
            i += 1;
            j += 1;
            (text, RowKind::Same)
        } else {
            return Err("Comparison alignment could not be reconstructed.".into());
        };
        result.append(text, kind)?;
    }
    check_cancelled(cancelled)?;
    Ok(result)
}
impl ComparisonResult {
    fn append(&mut self, text: &str, kind: RowKind) -> Result<(), String> {
        let prefix = match kind {
            RowKind::Same => "  ",
            RowKind::Added => "+ ",
            RowKind::Removed => "− ",
        };
        let extra = usize::from(!self.rows.is_empty()) + prefix.len() + text.len();
        if extra > OUTPUT_LIMIT.saturating_sub(self.copy_text.len()) {
            return Err(OUTPUT_ERROR.into());
        }
        if !self.rows.is_empty() {
            self.copy_text.push('\n');
        }
        self.copy_text.push_str(prefix);
        self.copy_text.push_str(text);
        self.rows.push(Row {
            kind,
            text: text.to_owned(),
        });
        self.added += usize::from(kind == RowKind::Added);
        self.removed += usize::from(kind == RowKind::Removed);
        Ok(())
    }
}

fn tokens(
    text: &str,
    options: Options,
    limit: usize,
    cancelled: &AtomicBool,
) -> Result<Vec<String>, String> {
    if options.mode == Mode::Json {
        return super::json_formatter::comparison_fields(text, limit, OUTPUT_LIMIT, cancelled);
    }
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut normalized = String::with_capacity(text.len());
    let mut previous_cr = false;
    for (index, scalar) in text.chars().enumerate() {
        if index.is_multiple_of(1024) {
            check_cancelled(cancelled)?;
        }
        if scalar != '\n' || !previous_cr {
            normalized.push(if scalar == '\r' { '\n' } else { scalar });
        }
        previous_cr = scalar == '\r';
    }
    let mut output = Vec::new();
    if options.mode == Mode::Words {
        for_each_word(&normalized, cancelled, |word| {
            push_token(&mut output, word.to_owned(), limit)
        })?;
    } else {
        // Unlike str::lines(), split preserves the final empty row.
        for line in normalized.split('\n') {
            check_cancelled(cancelled)?;
            if output.len() == limit {
                return Err(TOKEN_ERROR.into());
            }
            let line = if options.ignore_whitespace {
                let mut collapsed = String::new();
                for_each_word(line, cancelled, |word| {
                    if !collapsed.is_empty() {
                        collapsed.push(' ');
                    }
                    collapsed.push_str(word);
                    Ok(())
                })?;
                collapsed
            } else {
                line.to_owned()
            };
            output.push(line);
        }
    }
    Ok(output)
}
fn push_token(output: &mut Vec<String>, text: String, limit: usize) -> Result<(), String> {
    if output.len() == limit {
        return Err(TOKEN_ERROR.into());
    }
    output.push(text);
    Ok(())
}
fn for_each_word(
    text: &str,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let mut start = None;
    for (index, (offset, grapheme)) in text.grapheme_indices(true).enumerate() {
        if index.is_multiple_of(256) {
            check_cancelled(cancelled)?;
        }
        // Swift Character.isWhitespace tests its first Unicode scalar, not all
        // scalars, so a space + combining mark is one separator and is removed.
        if grapheme.chars().next().is_some_and(char::is_whitespace) {
            if let Some(start) = start.take() {
                emit(&text[start..offset])?;
            }
        } else if start.is_none() {
            start = Some(offset);
        }
    }
    if let Some(start) = start {
        emit(&text[start..])?;
    }
    check_cancelled(cancelled)
}
fn identities(
    left: &[String],
    right: &[String],
    cancelled: &AtomicBool,
) -> Result<(Vec<u16>, Vec<u16>), String> {
    let mut dictionary = BTreeMap::<String, u16>::new();
    let mut intern = |tokens: &[String]| -> Result<Vec<u16>, String> {
        let mut result = Vec::with_capacity(tokens.len());
        for token in tokens {
            check_cancelled(cancelled)?;
            let mut normalized = String::new();
            for (index, scalar) in token.nfc().enumerate() {
                if index.is_multiple_of(1024) {
                    check_cancelled(cancelled)?;
                }
                normalized.push(scalar);
            }
            let next = dictionary.len() as u16; // At most TOKEN_LIMIT identities.
            result.push(*dictionary.entry(normalized).or_insert(next));
        }
        Ok(result)
    };
    Ok((intern(left)?, intern(right)?))
}

// The Region/Frontiers alignment below is a modified Rust adaptation of the
// Swift.org open source project's Diffing.swift at the immutable revision above.
// Copyright (c) 2015 - 2019 Apple Inc. and the Swift project authors
// Licensed under Apache License v2.0 with Runtime Library Exception
// See https://swift.org/LICENSE.txt for license information
// See https://swift.org/CONTRIBUTORS.txt for the list of Swift project authors
//
// Modifications (2026-10-09): safe Rust vectors, interned token IDs, bounded
// inputs/output, cancellation checks, direct empty-axis handling, and edit masks.
// Exact upstream license: rust/docs/licenses/swift-runtime-LICENSE.txt.
// Provenance and distribution notice: rust/docs/swift-comparison-attribution.md.
#[derive(Clone, Copy, Debug)]
struct Region {
    left: isize,
    top: isize,
    right: isize,
    bottom: isize,
}
impl Region {
    fn size(self) -> isize {
        self.right - self.left + self.bottom - self.top
    }
    fn delta(self) -> isize {
        self.right - self.left - (self.bottom - self.top)
    }
    fn shrink(mut self, a: &[u16], b: &[u16], cancelled: &AtomicBool) -> Result<Self, String> {
        while self.left < self.right
            && self.top < self.bottom
            && a[self.left as usize] == b[self.top as usize]
        {
            check_cancelled(cancelled)?;
            self.left += 1;
            self.top += 1;
        }
        while self.left < self.right
            && self.top < self.bottom
            && a[self.right as usize - 1] == b[self.bottom as usize - 1]
        {
            check_cancelled(cancelled)?;
            self.right -= 1;
            self.bottom -= 1;
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug)]
struct Edit {
    insertion: bool,
    offset: usize,
}
struct Frontiers {
    forward: Vec<isize>,
    backward: Vec<isize>,
    offset: isize,
}
impl Frontiers {
    fn new(size: usize) -> Self {
        Self {
            forward: vec![0; size * 2 + 3],
            backward: vec![0; size * 2 + 3],
            offset: size as isize + 1,
        }
    }
    fn index(&self, diagonal: isize) -> usize {
        (self.offset + diagonal) as usize
    }
    fn middle(
        &mut self,
        region: Region,
        a: &[u16],
        b: &[u16],
        cancelled: &AtomicBool,
    ) -> Result<(Edit, Region), String> {
        let first = self.index(1);
        self.forward[first] = region.left;
        self.backward[first] = region.bottom;
        let odd = region.delta() % 2 != 0;
        for depth in 0..=(region.size() + 1) / 2 {
            check_cancelled(cancelled)?;
            if let Some(middle) = self.search_forward(region, depth, a, b, cancelled)? {
                return Ok(middle);
            }
            if let Some(middle) = self.search_backward(region, depth, a, b, cancelled)? {
                return Ok(middle);
            }
        }
        // Nonempty finite edit graphs always intersect. Return a failure rather
        // than publish partial rows if a future algorithm edit violates this.
        Err(format!("Comparison alignment failed (odd region: {odd})."))
    }
    fn search_forward(
        &mut self,
        region: Region,
        depth: isize,
        a: &[u16],
        b: &[u16],
        cancelled: &AtomicBool,
    ) -> Result<Option<(Edit, Region)>, String> {
        for step in 0..=depth {
            if step % 256 == 0 {
                check_cancelled(cancelled)?;
            }
            let k = depth - 2 * step;
            let insertion = k == -depth
                || (k != depth
                    && self.forward[self.index(k - 1)] < self.forward[self.index(k + 1)]);
            let left = self.forward[self.index(if insertion { k + 1 } else { k - 1 })];
            let mut right = left + isize::from(!insertion);
            let mut bottom = region.top + right - region.left - k;
            let top = bottom - isize::from(depth != 0 && insertion);
            let edit = Edit {
                insertion,
                offset: if insertion { bottom - 1 } else { left } as usize,
            };
            while right < region.right
                && bottom < region.bottom
                && a[right as usize] == b[bottom as usize]
            {
                check_cancelled(cancelled)?;
                right += 1;
                bottom += 1;
            }
            let index = self.index(k);
            self.forward[index] = right;
            let opposite = k - region.delta();
            if region.delta() % 2 != 0
                && opposite > -depth
                && opposite < depth
                && bottom >= self.backward[self.index(opposite)]
            {
                return Ok(Some((
                    edit,
                    Region {
                        left,
                        top,
                        right,
                        bottom,
                    },
                )));
            }
        }
        Ok(None)
    }
    fn search_backward(
        &mut self,
        region: Region,
        depth: isize,
        a: &[u16],
        b: &[u16],
        cancelled: &AtomicBool,
    ) -> Result<Option<(Edit, Region)>, String> {
        for step in 0..=depth {
            if step % 256 == 0 {
                check_cancelled(cancelled)?;
            }
            let diagonal = depth - 2 * step;
            let k = diagonal + region.delta();
            let removal = diagonal == -depth
                || (diagonal != depth
                    && self.backward[self.index(diagonal - 1)]
                        > self.backward[self.index(diagonal + 1)]);
            let bottom =
                self.backward[self.index(if removal { diagonal + 1 } else { diagonal - 1 })];
            let mut top = bottom - isize::from(!removal);
            let mut left = region.left + top - region.top + k;
            let right = left + isize::from(depth != 0 && removal);
            let edit = Edit {
                insertion: !removal,
                offset: if removal { left } else { top } as usize,
            };
            while left > region.left
                && top > region.top
                && a[left as usize - 1] == b[top as usize - 1]
            {
                check_cancelled(cancelled)?;
                left -= 1;
                top -= 1;
            }
            let index = self.index(diagonal);
            self.backward[index] = top;
            if region.delta() % 2 == 0
                && k >= -depth
                && k <= depth
                && left <= self.forward[self.index(k)]
            {
                return Ok(Some((
                    edit,
                    Region {
                        left,
                        top,
                        right,
                        bottom,
                    },
                )));
            }
        }
        Ok(None)
    }
}
fn changes(a: &[u16], b: &[u16], cancelled: &AtomicBool) -> Result<(Vec<bool>, Vec<bool>), String> {
    check_cancelled(cancelled)?;
    let initial = Region {
        left: 0,
        top: 0,
        right: a.len() as isize,
        bottom: b.len() as isize,
    }
    .shrink(a, b, cancelled)?;
    // Swift 6.3.3's middle-snake traversal and prefix/suffix shrinking decide
    // repeated-token alignment. Two frontiers plus an iterative region stack
    // use linear memory (< 0.6 MB at 8,000 tokens on 64-bit hosts).
    let mut frontiers = Frontiers::new(initial.size() as usize);
    let mut regions = vec![initial];
    let mut removed = vec![false; a.len()];
    let mut added = vec![false; b.len()];
    while let Some(region) = regions.pop() {
        check_cancelled(cancelled)?;
        // Empty axes have no matching choices; mark their unique edits directly.
        if region.left == region.right {
            for index in region.top..region.bottom {
                check_cancelled(cancelled)?;
                added[index as usize] = true;
            }
            continue;
        }
        if region.top == region.bottom {
            for index in region.left..region.right {
                check_cancelled(cancelled)?;
                removed[index as usize] = true;
            }
            continue;
        }
        let (edit, middle) = frontiers.middle(region, a, b, cancelled)?;
        if edit.insertion {
            added[edit.offset] = true;
        } else {
            removed[edit.offset] = true;
        }
        let head = Region {
            right: middle.left,
            bottom: middle.top,
            ..region
        }
        .shrink(a, b, cancelled)?;
        let tail = Region {
            left: middle.right,
            top: middle.bottom,
            ..region
        }
        .shrink(a, b, cancelled)?;
        regions.push(tail);
        regions.push(head);
    }
    Ok((removed, added))
}

#[cfg(test)]
#[path = "comparison_tests.rs"]
mod tests;
