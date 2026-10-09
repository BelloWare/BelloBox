//! Bounded Text Tools state. Palette opens copy choices into a fresh window owner;
//! they never transfer the worker, mutable draft, or cancellation identity.
use crate::session::{JobToken, SessionJobs};
use gpui::Context;
use std::{sync::Arc, time::Duration};

const MAX_OUTPUT_BYTES: usize = 4_000_000;
const DEBOUNCE_MS: u64 = 60;
const HASH_NOTE: &str = "MD5 and SHA-1 are for compatibility, not security.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Category {
    #[default]
    Case,
    Encode,
    Decode,
    Pretty,
    Hash,
    Lines,
    Count,
}
impl Category {
    pub const ALL: [Self; 7] = [
        Self::Case,
        Self::Encode,
        Self::Decode,
        Self::Pretty,
        Self::Hash,
        Self::Lines,
        Self::Count,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Case => "Case",
            Self::Encode => "Encode",
            Self::Decode => "Decode",
            Self::Pretty => "Pretty",
            Self::Hash => "Hash",
            Self::Lines => "Lines",
            Self::Count => "Count",
        }
    }
    #[cfg(test)]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|category| *category == self)
            .unwrap()
    }
    fn is_primary(self) -> bool {
        !matches!(self, Self::Hash | Self::Count)
    }
}

macro_rules! option_enum {
    ($name:ident, $default:ident, $( $variant:ident => ($argument:literal, $label:literal) ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum $name { $( $variant ),+ }
        impl Default for $name {
            fn default() -> Self { Self::$default }
        }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            const OPTIONS: &'static [(&'static str, &'static str)] = &[
                $((Self::$variant.argument(), Self::$variant.label())),+
            ];
            pub const fn argument(self) -> &'static str {
                match self { $( Self::$variant => $argument ),+ }
            }
            pub const fn label(self) -> &'static str {
                match self { $( Self::$variant => $label ),+ }
            }
            fn parse(argument: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|value| value.argument() == argument)
            }
        }
    };
}
option_enum!(CaseOption, Upper,
    Upper => ("upper", "UPPERCASE"),
    Lower => ("lower", "lowercase"),
    Title => ("title", "Title Case"),
    Sentence => ("sentence", "Sentence case"),
    Camel => ("camel", "camelCase"),
    Pascal => ("pascal", "PascalCase"),
    Snake => ("snake", "snake_case"),
    Kebab => ("kebab", "kebab-case"),
    Constant => ("constant", "CONSTANT_CASE"),
);
option_enum!(EncodeOption, Base64,
    Base64 => ("base64", "Base64"),
    Url => ("url", "URL"),
    Html => ("html", "HTML entities"),
    Hex => ("hex", "Hex"),
);
option_enum!(DecodeOption, Auto,
    Auto => ("decode", "Auto-detect"),
    Base64 => ("decode-base64", "Base64"),
    Url => ("decode-url", "URL"),
    Html => ("decode-html", "HTML entities"),
    Hex => ("decode-hex", "Hex"),
);
option_enum!(LineOption, Sort,
    Sort => ("sort", "Sort A → Z"),
    SortReverse => ("sort-reverse", "Sort Z → A"),
    Reverse => ("reverse", "Reverse"),
    Unique => ("unique", "Remove duplicates"),
    Nonempty => ("nonempty", "Remove empty lines"),
    Trim => ("trim", "Trim each line"),
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Choices {
    pub category: Category,
    pub case: CaseOption,
    pub encode: EncodeOption,
    pub decode: DecodeOption,
    pub lines: LineOption,
}
impl Choices {
    pub fn argument(self) -> &'static str {
        match self.category {
            Category::Case => self.case.argument(),
            Category::Encode => self.encode.argument(),
            Category::Decode => self.decode.argument(),
            Category::Pretty => "pretty",
            Category::Hash => "hash",
            Category::Lines => self.lines.argument(),
            Category::Count => "count",
        }
    }
    pub fn options(self) -> &'static [(&'static str, &'static str)] {
        match self.category {
            Category::Case => CaseOption::OPTIONS,
            Category::Encode => EncodeOption::OPTIONS,
            Category::Decode => DecodeOption::OPTIONS,
            Category::Lines => LineOption::OPTIONS,
            Category::Pretty | Category::Hash | Category::Count => &[],
        }
    }
    /// Selecting another category never resets any of its independent options.
    pub fn select_category(&mut self, category: Category) {
        self.category = category;
    }
    pub fn select_option(&mut self, argument: &str) -> bool {
        match self.category {
            Category::Case => CaseOption::parse(argument).map(|value| self.case = value),
            Category::Encode => EncodeOption::parse(argument).map(|value| self.encode = value),
            Category::Decode => DecodeOption::parse(argument).map(|value| self.decode = value),
            Category::Lines => LineOption::parse(argument).map(|value| self.lines = value),
            Category::Pretty | Category::Hash | Category::Count => None,
        }
        .is_some()
    }
    pub fn scope_note(self) -> &'static str {
        match self.category {
            Category::Pretty => "Pretty supports JSON only in this Rust build.",
            Category::Hash => HASH_NOTE,
            Category::Count => {
                "Characters count graphemes; without whitespace counts scalars. Tokens use a generic heuristic."
            }
            Category::Lines if matches!(self.lines, LineOption::Sort | LineOption::SortReverse) => {
                "Sorting uses ordinal ordering, not localized case-insensitive ordering."
            }
            _ => "Processed locally.",
        }
    }
}

/// A value snapshot only. No entity, token, mutable draft, or worker is shared.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Handoff {
    pub input: String,
    pub choices: Choices,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Palette,
    Window,
    Retired,
}

#[derive(Default)]
struct Calculation {
    output: String,
    digest_rows: Vec<(&'static str, String)>,
    count_rows: Vec<(&'static str, String)>,
}
impl Calculation {
    fn text(output: String) -> Self {
        Self {
            output,
            ..Self::default()
        }
    }
    fn validate(&self) -> Result<(), String> {
        // Include structured copies in the ownership budget, not only the text.
        let bytes = self
            .digest_rows
            .iter()
            .chain(&self.count_rows)
            .fold(self.output.len(), |sum, (_, value)| {
                sum.saturating_add(value.len())
            });
        if bytes > MAX_OUTPUT_BYTES {
            Err(format!(
                "Output exceeds {MAX_OUTPUT_BYTES} UTF-8 bytes; it has not been truncated."
            ))
        } else {
            Ok(())
        }
    }
}

fn calculate(input: &str, choices: Choices) -> Result<Calculation, String> {
    bellobox_core::validate_input(input)?;
    let result = match choices.category {
        Category::Count => {
            let count = bellobox_core::text::counts(input, "");
            let count_rows = vec![
                ("Characters", count.characters.to_string()),
                ("Without whitespace", count.without_whitespace.to_string()),
                ("Words", count.words.to_string()),
                ("Lines", count.lines.to_string()),
                ("Tokens ≈", count.estimated_tokens.to_string()),
            ];
            Calculation {
                output: format!(
                    "{} characters (graphemes)\n{} without whitespace (scalars)\n{} words\n{} lines\n~{} tokens ({})",
                    count.characters,
                    count.without_whitespace,
                    count.words,
                    count.lines,
                    count.estimated_tokens,
                    count.tokenizer_family
                ),
                count_rows,
                ..Calculation::default()
            }
        }
        Category::Hash => hash_result(bellobox_core::text::hashes(input))?,
        _ => Calculation::text(crate::execute("textTools", input, choices.argument())?),
    };
    result.validate()?;
    Ok(result)
}

/// The core exposes one dedicated digest formatter. Adapt exactly its four
/// fixed rows and compatibility note; never parse CLI help or count prose.
fn hash_result(output: String) -> Result<Calculation, String> {
    let error = || "The local hash result could not be displayed safely.".to_string();
    let mut rows = output.lines();
    let mut digest_rows = Vec::with_capacity(4);
    for (name, width) in [
        ("MD5", 32),
        ("SHA-1", 40),
        ("SHA-256", 64),
        ("SHA-512", 128),
    ] {
        let digest = rows
            .next()
            .and_then(|row| row.strip_prefix(name))
            .and_then(|rest| rest.strip_prefix(' '))
            .map(str::trim)
            .filter(|digest| digest.len() == width && digest.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(error)?;
        digest_rows.push((name, digest.to_owned()));
    }
    if rows.next() != Some("") || rows.next() != Some(HASH_NOTE) || rows.next().is_some() {
        return Err(error());
    }
    // The UI's complete Copy payload is a compact labelled digest list. The
    // compatibility note remains visible through scope_note(), not clipboard
    // text. This does not change the existing core/CLI hash serialization.
    let output = digest_rows
        .iter()
        .map(|(label, digest)| format!("{label}: {digest}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Calculation {
        output,
        digest_rows,
        ..Calculation::default()
    })
}

pub(crate) struct TextSession {
    original: Arc<str>,
    original_valid: bool,
    input: Arc<str>,
    pub choices: Choices,
    pub output: String,
    pub digest_rows: Vec<(&'static str, String)>,
    pub count_rows: Vec<(&'static str, String)>,
    pub error: Option<String>,
    pub status: String,
    pub busy: bool,
    owner: Owner,
    jobs: SessionJobs,
    desired: Option<JobToken>,
    completed: Option<(JobToken, Choices)>,
    running: bool,
    invalid_input: bool,
    #[cfg(test)]
    probe: WorkerProbe,
}
impl TextSession {
    pub fn new(input: String, choices: Choices, preview: bool, cx: &mut Context<Self>) -> Self {
        // Validate before preserving a second reference to an admitted input.
        let valid = bellobox_core::validate_input(&input);
        let original: Arc<str> = if valid.is_ok() {
            Arc::from(input)
        } else {
            Arc::from("")
        };
        let mut session = Self {
            input: original.clone(),
            original,
            original_valid: valid.is_ok(),
            choices,
            output: String::new(),
            digest_rows: Vec::new(),
            count_rows: Vec::new(),
            error: None,
            status: String::new(),
            busy: false,
            owner: if preview {
                Owner::Palette
            } else {
                Owner::Window
            },
            jobs: SessionJobs::default(),
            desired: None,
            completed: None,
            running: false,
            invalid_input: false,
            #[cfg(test)]
            probe: WorkerProbe::default(),
        };
        match valid {
            Ok(()) => session.changed(cx),
            Err(error) => session.reject_input(error, cx),
        }
        session
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    #[cfg(test)]
    pub fn original_input(&self) -> &str {
        &self.original
    }
    pub fn oversized_preview(&self) -> bool {
        self.owner == Owner::Palette && self.input.len() > bellobox_core::MAX_PREVIEW_BYTES
    }
    pub fn can_snapshot(&self) -> bool {
        self.owner == Owner::Palette
            && self.original_valid
            && !self.invalid_input
            && bellobox_core::validate_input(&self.original).is_ok()
            && bellobox_core::validate_input(&self.input).is_ok()
    }
    pub fn snapshot(&self) -> Result<Handoff, String> {
        // Revalidate before cloning. A rejected editor draft never opens the
        // older admitted input, even if options subsequently change.
        if !self.can_snapshot() {
            return Err("This Text Tools selection is no longer available to open.".into());
        }
        Ok(Handoff {
            input: self.original.to_string(),
            choices: self.choices,
        })
    }
    pub fn can_copy(&self) -> bool {
        self.owner != Owner::Retired
            && !self.invalid_input
            && !self.busy
            && self.error.is_none()
            && !self.output.is_empty()
            && self
                .completed
                .is_some_and(|(token, choices)| self.jobs.accepts(token) && choices == self.choices)
    }
    pub fn copy_text(&self) -> Option<&str> {
        self.can_copy().then_some(self.output.as_str())
    }
    pub fn hash_copy(&self, index: usize) -> Option<&str> {
        if !self.can_copy() || self.choices.category != Category::Hash {
            return None;
        }
        self.digest_rows
            .get(index)
            .map(|(_, digest)| digest.as_str())
    }
    pub fn can_chain(&self) -> bool {
        self.owner == Owner::Window
            && self.can_copy()
            && self.choices.category.is_primary()
            && self.output != self.input.as_ref()
            && self.output.len() <= bellobox_core::MAX_INPUT_BYTES
    }
    #[cfg(test)]
    pub fn chain(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.can_chain() {
            return false;
        }
        self.set_input(self.output.clone(), cx);
        true
    }
    #[cfg(test)]
    pub fn reset(&mut self, cx: &mut Context<Self>) -> bool {
        if self.owner == Owner::Retired || !self.original_valid {
            return false;
        }
        // This original is admitted once and is never changed by chaining.
        self.set_input(self.original.to_string(), cx);
        true
    }
    pub fn set_draft(&mut self, input: String, choices: Choices, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        if !self.invalid_input && self.input() == input && self.choices == choices {
            return;
        }
        self.choices = choices;
        self.set_input(input, cx);
    }
    pub fn set_input(&mut self, input: String, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        if let Err(error) = bellobox_core::validate_input(&input) {
            self.reject_input(error, cx);
            return;
        }
        self.invalid_input = false;
        self.input = Arc::from(input);
        self.changed(cx);
    }
    pub fn set_choices(&mut self, choices: Choices, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired || self.choices == choices {
            return;
        }
        self.choices = choices;
        self.changed(cx);
    }
    /// Hosts call this before cloning an oversized local editor draft. The
    /// editor retains that draft; this owner cannot process its previous input.
    pub fn reject_input(&mut self, error: String, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        self.invalidate();
        self.invalid_input = true;
        self.error = Some(error);
        cx.notify();
    }
    #[cfg(test)]
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.owner != Owner::Retired {
            self.changed(cx);
        }
    }
    #[cfg(test)]
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        self.invalidate();
        self.status = "Cancelled.".into();
        cx.notify();
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        if self.owner == Owner::Retired {
            return;
        }
        self.invalidate();
        self.owner = Owner::Retired;
        cx.notify();
    }
    fn invalidate(&mut self) {
        self.jobs.cancel();
        self.desired = None;
        self.completed = None;
        self.busy = false;
        self.output.clear();
        self.digest_rows.clear();
        self.count_rows.clear();
        self.error = None;
        self.status.clear();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.invalidate();
        if self.invalid_input {
            self.error = Some(format!(
                "Input exceeds {} UTF-8 bytes; it has not been truncated.",
                bellobox_core::MAX_INPUT_BYTES
            ));
        } else if self.oversized_preview() {
            self.status = "Open Text Tools to process the complete selection.".into();
        } else if !self.input.is_empty() || !self.choices.category.is_primary() {
            self.desired = Some(self.jobs.begin());
            self.busy = true;
            self.status = "Working locally…".into();
            self.start_worker(cx);
        }
        cx.notify();
    }
    fn start_worker(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(token)) = this.update(cx, |s, _| s.desired) else {
                    let _ = this.update(cx, |s, _| s.running = false);
                    break;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(DEBOUNCE_MS))
                    .await;
                let request = this.update(cx, |s, _| {
                    // Check admission immediately before taking a worker input
                    // reference; edits only replace the latest desired state.
                    (s.desired == Some(token)
                        && s.jobs.accepts(token)
                        && s.owner != Owner::Retired
                        && !s.invalid_input
                        && !s.oversized_preview()
                        && bellobox_core::validate_input(&s.input).is_ok())
                    .then(|| (s.input.clone(), s.choices))
                });
                let (input, choices) = match request {
                    Ok(Some(request)) => request,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                #[cfg(test)]
                let probe = match this.update(cx, |s, _| s.probe.clone()) {
                    Ok(probe) => probe,
                    Err(_) => break,
                };
                #[cfg(test)]
                let executor = cx.background_executor().clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        #[cfg(test)]
                        {
                            use std::sync::atomic::Ordering::SeqCst;
                            probe.starts.fetch_add(1, SeqCst);
                            while probe.held.load(SeqCst) {
                                executor.timer(Duration::from_millis(10)).await;
                            }
                        }
                        calculate(&input, choices)
                    })
                    .await;
                if this
                    .update(cx, |s, cx| s.publish(token, choices, result, cx))
                    .is_err()
                {
                    break;
                }
                // A canceled calculation drains before another can start.
                // There is at most one physical worker and one desired token.
            }
        })
        .detach();
    }
    fn publish(
        &mut self,
        token: JobToken,
        choices: Choices,
        result: Result<Calculation, String>,
        cx: &mut Context<Self>,
    ) {
        if self.desired != Some(token)
            || !self.jobs.accepts(token)
            || self.owner == Owner::Retired
            || self.invalid_input
            || self.choices != choices
        {
            return;
        }
        self.desired = None;
        self.busy = false;
        match result.and_then(|result| {
            result.validate()?;
            Ok(result)
        }) {
            Ok(result) => {
                self.output = result.output;
                self.digest_rows = result.digest_rows;
                self.count_rows = result.count_rows;
                self.completed = Some((token, choices));
                self.status = self.choices.scope_note().into();
            }
            Err(error) => {
                self.completed = None;
                self.error = Some(error);
                self.status = "This operation needs attention.".into();
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct WorkerProbe {
    pub held: Arc<std::sync::atomic::AtomicBool>,
    pub starts: Arc<std::sync::atomic::AtomicUsize>,
}
#[cfg(test)]
impl TextSession {
    pub(crate) fn worker_probe(&self) -> WorkerProbe {
        self.probe.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};
    use std::sync::atomic::Ordering::SeqCst;

    fn tick(cx: &mut TestAppContext, milliseconds: u64) {
        cx.run_until_parked();
        cx.background_executor
            .advance_clock(Duration::from_millis(milliseconds));
        cx.run_until_parked();
    }
    fn options(category: Category, argument: &str) -> Choices {
        let mut choices = Choices::default();
        choices.select_category(category);
        assert!(choices.select_option(argument));
        choices
    }
    fn publish_text(session: &mut TextSession, output: &str, cx: &mut Context<TextSession>) {
        session.publish(
            session.desired.unwrap(),
            session.choices,
            Ok(Calculation::text(output.into())),
            cx,
        );
    }

    #[test]
    fn every_typed_option_has_a_unique_argument_and_retains_its_independent_choice() {
        let mut choices = Choices::default();
        assert_eq!(choices.category, Category::Case);
        assert_eq!(choices.argument(), "upper");
        assert_eq!(
            (
                CaseOption::ALL.len(),
                EncodeOption::ALL.len(),
                DecodeOption::ALL.len(),
                LineOption::ALL.len()
            ),
            (9, 4, 5, 6)
        );
        for (index, category) in Category::ALL.into_iter().enumerate() {
            assert_eq!(category.index(), index);
            assert!(!category.label().is_empty());
            choices.select_category(category);
            let mut arguments = std::collections::HashSet::new();
            for &(argument, label) in choices.options() {
                assert!(!label.is_empty());
                assert!(arguments.insert(argument));
                assert!(choices.select_option(argument));
                assert_eq!(choices.argument(), argument);
            }
            let previous = choices;
            assert!(!choices.select_option("not-an-option"));
            assert_eq!(choices, previous);
        }
        for (category, argument) in [
            (Category::Case, "snake"),
            (Category::Encode, "url"),
            (Category::Decode, "decode-hex"),
            (Category::Lines, "unique"),
        ] {
            choices.select_category(category);
            assert!(choices.select_option(argument));
        }
        for (category, argument) in [
            (Category::Case, "snake"),
            (Category::Encode, "url"),
            (Category::Decode, "decode-hex"),
            (Category::Lines, "unique"),
        ] {
            choices.select_category(category);
            assert_eq!(choices.argument(), argument);
        }
    }

    #[test]
    fn structured_hashes_and_counts_do_not_contain_cli_help() {
        let hash = calculate(
            "hello",
            Choices {
                category: Category::Hash,
                ..Choices::default()
            },
        )
        .unwrap();
        assert_eq!(hash.digest_rows.len(), 4);
        assert_eq!(
            hash.digest_rows[0],
            ("MD5", "5d41402abc4b2a76b9719d911017c592".into())
        );
        assert_eq!(
            hash.digest_rows[2].1,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(
            hash.output,
            "MD5: 5d41402abc4b2a76b9719d911017c592\nSHA-1: aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d\nSHA-256: 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824\nSHA-512: 9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043"
        );
        assert!(!hash.output.contains("compatibility"));
        let valid_hashes = bellobox_core::text::hashes("hello");
        assert_eq!(
            hash_result(valid_hashes.clone()).unwrap().output,
            hash.output
        );
        for malformed in [
            format!("{valid_hashes}\nSHA-256 extra"),
            valid_hashes.replacen("MD5", "MD6", 1),
            valid_hashes.replacen("MD5     ", "MD5", 1),
            valid_hashes.replacen("5d41402abc4b2a76b9719d911017c592", "5d4", 1),
            valid_hashes.replace(HASH_NOTE, "unrecognized footer"),
            valid_hashes.lines().take(4).collect::<Vec<_>>().join("\n"),
        ] {
            assert!(hash_result(malformed).is_err());
        }
        let count = calculate(
            "e\u{301} 界",
            Choices {
                category: Category::Count,
                ..Choices::default()
            },
        )
        .unwrap();
        assert_eq!(count.count_rows.len(), 5);
        assert_eq!(count.count_rows[0].1, "3");
        assert_eq!(count.count_rows[1].1, "3");
        assert!(count.output.contains("characters (graphemes)"));
        assert!(count.output.contains("generic heuristic"));
        assert!(!count.output.contains("Operation in second input"));
        assert_eq!(
            calculate(
                "",
                Choices {
                    category: Category::Count,
                    ..Choices::default()
                }
            )
            .unwrap()
            .count_rows[4]
                .1,
            "0"
        );
        assert!(
            Choices {
                category: Category::Pretty,
                ..Choices::default()
            }
            .scope_note()
            .contains("JSON only")
        );
        assert!(
            Choices {
                category: Category::Lines,
                ..Choices::default()
            }
            .scope_note()
            .contains("ordinal")
        );
    }

    #[test]
    fn typed_dispatch_preserves_existing_engines_and_rejects_bad_decode() {
        for category in Category::ALL {
            let mut choices = Choices {
                category,
                ..Choices::default()
            };
            for &(argument, _) in choices.options() {
                choices.select_option(argument);
                let input = match argument {
                    "decode" | "decode-base64" => "aGVsbG8=",
                    "decode-url" => "hello%20world",
                    "decode-html" => "&lt;hello&gt;",
                    "decode-hex" => "68656c6c6f",
                    _ => "b\na\nb é <&>",
                };
                assert_eq!(
                    calculate(input, choices).unwrap().output,
                    crate::execute("textTools", input, argument).unwrap()
                );
            }
        }
        assert!(calculate("%zz", options(Category::Decode, "decode-url")).is_err());
        assert_eq!(
            calculate("%C3%A9", options(Category::Decode, "decode-url"))
                .unwrap()
                .output,
            "é"
        );
        #[cfg(feature = "developer-tools")]
        {
            let pretty = Choices {
                category: Category::Pretty,
                ..Choices::default()
            };
            assert!(calculate("<x/>", pretty).is_err());
            assert!(
                calculate("{\"x\":1}", pretty)
                    .unwrap()
                    .output
                    .contains("  \"x\"")
            );
        }
        #[cfg(not(feature = "developer-tools"))]
        assert!(
            calculate(
                "{}",
                Choices {
                    category: Category::Pretty,
                    ..Choices::default()
                }
            )
            .err()
            .unwrap()
            .contains("minimal build")
        );
    }

    #[gpui::test]
    fn snapshot_copies_original_and_choices_into_independent_window_jobs(cx: &mut TestAppContext) {
        let original = "a\r\nb é\n";
        let palette = cx.new(|cx| {
            TextSession::new(original.into(), options(Category::Case, "snake"), true, cx)
        });
        let handoff = palette.read_with(cx, |s, _| s.snapshot().unwrap());
        let a_handoff = handoff.clone();
        let a = cx.new(|cx| TextSession::new(a_handoff.input, a_handoff.choices, false, cx));
        let b = cx.new(|cx| TextSession::new(handoff.input, handoff.choices, false, cx));
        let palette_token = palette.read_with(cx, |s, _| s.desired.unwrap());
        a.update(cx, |s, cx| {
            assert_eq!(s.input(), original);
            assert_eq!(s.original_input(), original);
            assert_eq!(s.choices.argument(), "snake");
            assert!(!s.jobs.accepts(palette_token));
            assert!(s.snapshot().is_err());
            s.set_draft("edited".into(), options(Category::Encode, "url"), cx);
        });
        palette.update(cx, |s, cx| s.retire(cx));
        b.update(cx, |s, _| {
            assert_eq!(s.input(), original);
            assert_eq!(s.choices.argument(), "snake");
            assert!(s.busy);
        });
        tick(cx, DEBOUNCE_MS + 1);
        a.update(cx, |s, _| assert_eq!(s.output, "edited"));
        b.update(cx, |s, _| assert_eq!(s.output, "a_b_é"));
    }

    #[gpui::test]
    fn current_result_copy_and_primary_chain_guards_preserve_reset(cx: &mut TestAppContext) {
        let session =
            cx.new(|cx| TextSession::new("original".into(), Choices::default(), false, cx));
        session.update(cx, |s, cx| {
            assert!(!s.can_copy());
            publish_text(s, "ORIGINAL", cx);
            assert_eq!(s.copy_text(), Some("ORIGINAL"));
            assert!(s.chain(cx));
            assert_eq!(s.input(), "ORIGINAL");
            assert!(!s.can_copy());
            publish_text(s, "third", cx);
            assert!(s.chain(cx));
            assert_eq!(s.input(), "third");
            assert!(s.reset(cx));
            assert_eq!(s.input(), "original");
            publish_text(s, "original", cx);
            assert!(!s.can_chain());
            s.refresh(cx);
            publish_text(s, "", cx);
            assert!(!s.can_copy());
            assert!(!s.can_chain());
            s.refresh(cx);
            publish_text(s, &"x".repeat(bellobox_core::MAX_INPUT_BYTES + 1), cx);
            assert!(s.can_copy());
            assert!(!s.can_chain());
            s.refresh(cx);
            publish_text(s, &"x".repeat(bellobox_core::MAX_INPUT_BYTES), cx);
            assert!(s.can_chain());
            s.cancel(cx);
            assert!(!s.can_copy());
            for category in [Category::Hash, Category::Count] {
                s.set_choices(
                    Choices {
                        category,
                        ..Choices::default()
                    },
                    cx,
                );
                let result = calculate(s.input(), s.choices).unwrap();
                s.publish(s.desired.unwrap(), s.choices, Ok(result), cx);
                assert!(s.can_copy());
                assert!(!s.can_chain());
                assert!(!s.chain(cx));
                if category == Category::Hash {
                    assert_eq!(s.hash_copy(0).unwrap().len(), 32);
                    assert_eq!(s.copy_text().unwrap().lines().count(), 4);
                    assert!(
                        s.copy_text()
                            .unwrap()
                            .starts_with(&format!("MD5: {}\n", s.hash_copy(0).unwrap()))
                    );
                    assert!(!s.copy_text().unwrap().contains("compatibility"));
                    assert!(s.hash_copy(4).is_none());
                } else {
                    assert!(s.hash_copy(0).is_none());
                }
            }
            s.set_choices(Choices::default(), cx);
            assert!(s.digest_rows.is_empty());
            assert!(s.count_rows.is_empty());
            publish_text(s, "current", cx);
            // A host must schedule changes; even accidental direct option
            // mutation cannot make the old completed output eligible to copy.
            s.choices.case = CaseOption::Lower;
            assert!(!s.can_copy());
            s.retire(cx);
            assert!(!s.reset(cx));
            assert!(!s.can_copy());
        });
        let preview = cx.new(|cx| TextSession::new("x".into(), Choices::default(), true, cx));
        preview.update(cx, |s, cx| {
            publish_text(s, "X", cx);
            assert!(s.can_copy());
            assert!(!s.can_chain());
        });
    }

    #[gpui::test]
    fn byte_admission_and_invalid_snapshot_never_reuse_older_input(cx: &mut TestAppContext) {
        let preview =
            cx.new(|cx| TextSession::new("é".repeat(32_000), Choices::default(), true, cx));
        preview.update(cx, |s, cx| {
            assert_eq!(s.input().len(), bellobox_core::MAX_PREVIEW_BYTES);
            assert!(s.busy);
            s.set_input(format!("{}a", s.input()), cx);
            assert_eq!(s.input().len(), bellobox_core::MAX_PREVIEW_BYTES + 1);
            assert!(s.oversized_preview());
            assert!(!s.busy);
            assert!(s.desired.is_none());
            assert!(s.snapshot().is_ok());
            let old = s.input().to_owned();
            s.reject_input("Rejected local editor draft.".into(), cx);
            assert_eq!(s.input(), old);
            assert!(!s.can_snapshot());
            s.set_choices(options(Category::Encode, "hex"), cx);
            s.refresh(cx);
            assert!(s.snapshot().is_err());
            assert!(s.desired.is_none());
            assert!(!s.can_copy());
            s.set_input("recovered".into(), cx);
            assert!(s.snapshot().is_ok());
            // A value snapshot always carries the exact original selection.
            assert_eq!(s.snapshot().unwrap().input, "é".repeat(32_000));
            s.retire(cx);
            assert!(s.snapshot().is_err());
        });
        let full = cx.new(|cx| TextSession::new("original".into(), Choices::default(), false, cx));
        full.update(cx, |s, cx| {
            s.set_input("é".repeat(250_000), cx);
            assert!(s.busy);
            s.set_input(format!("{}a", s.input()), cx);
            assert!(s.error.is_some());
            assert!(s.desired.is_none());
            s.set_choices(options(Category::Encode, "html"), cx);
            s.refresh(cx);
            assert!(s.error.is_some());
            assert!(s.desired.is_none());
            assert!(s.reset(cx));
            assert_eq!(s.input(), "original");
            assert!(s.busy);
        });
        let rejected = cx.new(|cx| {
            TextSession::new(
                "x".repeat(bellobox_core::MAX_INPUT_BYTES + 1),
                Choices::default(),
                true,
                cx,
            )
        });
        rejected.update(cx, |s, cx| {
            assert!(s.input().is_empty());
            assert!(s.snapshot().is_err());
            assert!(!s.reset(cx));
            assert!(s.desired.is_none());
        });
    }

    #[test]
    fn encoded_outputs_and_output_cap_are_complete_and_never_truncated() {
        let input = "\"".repeat(bellobox_core::MAX_INPUT_BYTES);
        let encoded = calculate(&input, options(Category::Encode, "html")).unwrap();
        assert_eq!(encoded.output.len(), 3_000_000);
        assert_eq!(
            encoded.output,
            "&quot;".repeat(bellobox_core::MAX_INPUT_BYTES)
        );
        assert!(
            Calculation::text("x".repeat(MAX_OUTPUT_BYTES))
                .validate()
                .is_ok()
        );
        assert!(
            Calculation::text("x".repeat(MAX_OUTPUT_BYTES + 1))
                .validate()
                .is_err()
        );
        assert!(calculate(&format!("{input}x"), options(Category::Encode, "html")).is_err());
    }

    #[gpui::test]
    fn physical_worker_coalesces_rapid_input_and_option_changes(cx: &mut TestAppContext) {
        let session = cx.new(|cx| TextSession::new("first".into(), Choices::default(), false, cx));
        let probe = session.read_with(cx, |s, _| s.worker_probe());
        probe.held.store(true, SeqCst);
        tick(cx, DEBOUNCE_MS + 1);
        assert_eq!(probe.starts.load(SeqCst), 1);
        session.update(cx, |s, cx| {
            for i in 2..100 {
                let argument = if i % 2 == 0 { "upper" } else { "snake" };
                s.set_draft(format!("value {i}"), options(Category::Case, argument), cx);
            }
            assert!(s.running);
            assert!(s.busy);
            assert!(!s.can_copy());
        });
        tick(cx, 1000);
        assert_eq!(probe.starts.load(SeqCst), 1);
        probe.held.store(false, SeqCst);
        tick(cx, 11);
        tick(cx, DEBOUNCE_MS + 1);
        assert_eq!(probe.starts.load(SeqCst), 2);
        session.update(cx, |s, _| {
            assert_eq!(s.output, "value_99");
            assert!(s.can_copy());
            assert!(!s.busy);
            assert!(!s.running);
        });
    }

    #[gpui::test]
    fn held_success_and_error_are_fenced_by_cancel_clear_reset_and_retirement(
        cx: &mut TestAppContext,
    ) {
        for invalid_decode in [false, true] {
            for action in 0..4 {
                let choices = if invalid_decode {
                    options(Category::Decode, "decode-hex")
                } else {
                    Choices::default()
                };
                let session = cx.new(|cx| TextSession::new("first".into(), choices, false, cx));
                let probe = session.read_with(cx, |s, _| s.worker_probe());
                probe.held.store(true, SeqCst);
                tick(cx, DEBOUNCE_MS + 1);
                assert_eq!(probe.starts.load(SeqCst), 1);
                session.update(cx, |s, cx| {
                    match action {
                        0 => s.cancel(cx),
                        1 => s.set_input(String::new(), cx),
                        2 => {
                            s.set_draft("edited".into(), Choices::default(), cx);
                            s.reset(cx);
                        }
                        _ => s.retire(cx),
                    }
                    assert!(s.output.is_empty());
                    assert!(!s.can_copy());
                });
                probe.held.store(false, SeqCst);
                tick(cx, 11);
                session.update(cx, |s, _| {
                    assert!(s.output.is_empty());
                    assert!(s.error.is_none());
                    assert!(!s.can_copy());
                });
                tick(cx, DEBOUNCE_MS + 1);
                session.update(cx, |s, _| {
                    if action == 2 {
                        assert_eq!(s.output, "FIRST");
                        assert!(s.can_copy());
                    } else {
                        assert!(s.output.is_empty());
                        assert!(!s.can_copy());
                        assert_eq!(probe.starts.load(SeqCst), 1);
                    }
                    assert!(!s.busy);
                    assert!(!s.running);
                });
            }
        }
    }

    #[gpui::test]
    fn cancelled_worker_drains_before_recovery_and_empty_structured_results_work(
        cx: &mut TestAppContext,
    ) {
        let session = cx.new(|cx| {
            TextSession::new(
                "%zz".into(),
                options(Category::Decode, "decode-url"),
                false,
                cx,
            )
        });
        let probe = session.read_with(cx, |s, _| s.worker_probe());
        probe.held.store(true, SeqCst);
        tick(cx, DEBOUNCE_MS + 1);
        session.update(cx, |s, cx| {
            s.cancel(cx);
            s.set_input("%C3%A9".into(), cx);
        });
        tick(cx, 1000);
        assert_eq!(probe.starts.load(SeqCst), 1);
        probe.held.store(false, SeqCst);
        tick(cx, 11);
        tick(cx, DEBOUNCE_MS + 1);
        session.update(cx, |s, cx| {
            assert_eq!(s.output, "é");
            assert!(s.error.is_none());
            assert!(s.can_copy());
            s.set_draft(
                String::new(),
                Choices {
                    category: Category::Hash,
                    ..Choices::default()
                },
                cx,
            );
        });
        tick(cx, DEBOUNCE_MS + 1);
        session.update(cx, |s, cx| {
            assert_eq!(s.hash_copy(0), Some("d41d8cd98f00b204e9800998ecf8427e"));
            assert!(!s.can_chain());
            s.set_choices(
                Choices {
                    category: Category::Count,
                    ..Choices::default()
                },
                cx,
            );
            assert!(s.digest_rows.is_empty());
            assert!(s.count_rows.is_empty());
            assert!(!s.can_copy());
        });
        tick(cx, DEBOUNCE_MS + 1);
        session.update(cx, |s, _| {
            assert_eq!(s.count_rows.len(), 5);
            assert!(s.count_rows.iter().all(|(_, value)| value == "0"));
            assert!(s.can_copy());
            assert!(!s.can_chain());
        });
    }

    #[gpui::test]
    fn late_manual_results_output_overflow_and_retired_mutations_are_rejected(
        cx: &mut TestAppContext,
    ) {
        let session = cx.new(|cx| TextSession::new("first".into(), Choices::default(), true, cx));
        session.update(cx, |s, cx| {
            let old_token = s.desired.unwrap();
            let old_choices = s.choices;
            s.set_choices(options(Category::Encode, "url"), cx);
            s.publish(old_token, old_choices, Err("late failure".into()), cx);
            assert!(s.error.is_none());
            s.publish(
                old_token,
                old_choices,
                Ok(Calculation::text("late success".into())),
                cx,
            );
            assert!(s.output.is_empty());
            let token = s.desired.unwrap();
            s.publish(
                token,
                s.choices,
                Ok(Calculation::text("x".repeat(MAX_OUTPUT_BYTES + 1))),
                cx,
            );
            assert!(s.output.is_empty());
            assert!(s.error.as_ref().unwrap().contains("not been truncated"));
            assert!(!s.can_copy());
            s.refresh(cx);
            let token = s.desired.unwrap();
            s.reject_input("oversized draft".into(), cx);
            s.publish(token, s.choices, Ok(Calculation::text("late".into())), cx);
            assert!(!s.can_copy());
            assert!(s.snapshot().is_err());
            s.retire(cx);
            let choices = s.choices;
            s.set_input("never".into(), cx);
            s.set_draft("never".into(), Choices::default(), cx);
            s.set_choices(Choices::default(), cx);
            s.reject_input("never".into(), cx);
            s.refresh(cx);
            assert_eq!(s.choices, choices);
            assert_eq!(s.input(), "first");
            assert!(s.desired.is_none());
            assert!(!s.can_copy());
            assert!(s.snapshot().is_err());
        });
    }
}
