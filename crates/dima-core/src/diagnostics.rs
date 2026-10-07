//! Typed, aggregated diagnostics: conditions that do not stop an analysis but that the user
//! should know about (FastQC/Nextclade-style warnings recorded with the results).
//!
//! Each condition has a stable [`Code`]. Producers count occurrences in a [`Tally`] (cheap,
//! mergeable, keeps the first [`MAX_EXAMPLES`] examples in input order) and turn the totals into
//! one [`Diagnostic`] per condition at the end of the run, so the user sees every condition
//! once, with a count, examples and a hint, rather than a flood of per-record messages.

use serde::{Deserialize, Serialize};

/// Maximum number of examples stored per diagnostic.
pub const MAX_EXAMPLES: usize = 5;

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Something DiMA did on the user's behalf (for example folding lower case).
    Info,
    /// Something that may affect the interpretation of the results.
    Warning,
}

impl Severity {
    /// Lower-case name, as serialised.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
        }
    }
}

macro_rules! codes {
    ($($(#[$doc:meta])* $variant:ident = $name:literal, $severity:ident;)+) => {
        /// Stable diagnostic codes. The serialised names never change within a schema major
        /// version; new codes may be added in minor versions.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Code {
            $($(#[$doc])* $variant,)+
        }

        impl Code {
            /// Every code, in report order.
            pub const ALL: &'static [Code] = &[$(Code::$variant,)+];

            /// The stable serialised name.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Code::$variant => $name,)+
                }
            }

            /// The severity of the condition.
            #[must_use]
            pub const fn severity(self) -> Severity {
                match self {
                    $(Code::$variant => Severity::$severity,)+
                }
            }
        }
    };
}

codes! {
    /// Lower-case residues were folded to upper case.
    LowercaseFolded = "lowercase_folded", Info;
    /// T and U were both present and folded to the resolved alphabet's letter.
    TUFolded = "t_u_folded", Info;
    /// An explicit `--alphabet` contradicts the residues seen in the data.
    AlphabetContradiction = "alphabet_contradiction", Warning;
    /// Whitespace inside sequence lines was ignored.
    InteriorWhitespace = "interior_whitespace", Warning;
    /// Characters outside the alphabet were excluded (permissive and report modes).
    UnknownCharactersExcluded = "unknown_characters_excluded", Warning;
    /// Headers whose field count differs from `--header-format` received the fill value.
    HeaderFieldCountMismatch = "header_field_count_mismatch", Warning;
    /// Invalid UTF-8 in headers was replaced by U+FFFD.
    HeaderUtf8Replaced = "header_utf8_replaced", Warning;
    /// Header values were cleaned (invisible characters removed, trimmed, NFC-normalised,
    /// empty values filled).
    HeaderValuesCleaned = "header_values_cleaned", Info;
    /// Distinct values of a field differ only by case, spacing or compatibility forms.
    NearDuplicateValues = "near_duplicate_values", Warning;
    /// A field contains tokens that usually mean "missing" (they are kept literally).
    MissingLikeValues = "missing_like_values", Warning;
    /// A stratified field looks like an identifier (every value unique).
    IdentifierLikeField = "identifier_like_field", Warning;
    /// The support threshold is not below the number of sequences, so no position can be
    /// eligible.
    ThresholdExceedsSequenceCount = "threshold_exceeds_sequence_count", Warning;
    /// No position has support above the threshold.
    NoEligiblePositions = "no_eligible_positions", Warning;
    /// Positions have sample coverage below `--min-coverage`.
    BelowMinCoverage = "below_min_coverage", Warning;
    /// Conserved regions include positions with support at or below the threshold.
    HcsLowSupportRegions = "hcs_low_support_regions", Warning;
    /// An option was given that has no effect with the other options.
    FlagHasNoEffect = "flag_has_no_effect", Warning;
}

impl Serialize for Code {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// One reported condition.
///
/// `code` is stored as a string so that results written by a newer minor version (which may add
/// codes) can still be read and converted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable code (see [`Code`]).
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// Human-readable description, including the total count.
    pub message: String,
    /// Number of occurrences (records, values, positions, … as the message says).
    pub count: u64,
    /// Up to [`MAX_EXAMPLES`] examples, in input order.
    pub examples: Vec<String>,
    /// How to address the condition, if anything should be done.
    pub hint: Option<String>,
}

impl Diagnostic {
    /// A diagnostic for `code` built from a tally.
    #[must_use]
    pub fn new(code: Code, message: impl Into<String>, tally: Tally) -> Self {
        Self {
            code: code.as_str().to_owned(),
            severity: code.severity(),
            message: message.into(),
            count: tally.count,
            examples: tally.examples,
            hint: None,
        }
    }

    /// Adds a hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The typed code, if this version knows it.
    #[must_use]
    pub fn known_code(&self) -> Option<Code> {
        Code::ALL.iter().copied().find(|c| c.as_str() == self.code)
    }
}

pub(crate) use crate::human::plural;

/// The verb form that agrees with a count: `agree(n, "is", "are")`.
pub(crate) const fn agree(n: u64, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

/// Orders diagnostics for reporting: by [`Code`] order (unknown codes last), keeping the
/// emission order within a code. Deterministic regardless of thread scheduling.
pub fn sort(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by_key(|d| d.known_code().map_or(usize::MAX, |c| c as usize));
}

/// A mergeable count with the first examples in input order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Number of occurrences.
    pub count: u64,
    /// The first [`MAX_EXAMPLES`] examples.
    pub examples: Vec<String>,
}

impl Tally {
    /// A tally of `count` occurrences without examples.
    #[must_use]
    pub const fn count_only(count: u64) -> Self {
        Self {
            count,
            examples: Vec::new(),
        }
    }

    /// Records `n` occurrences; `example` is evaluated only while fewer than
    /// [`MAX_EXAMPLES`] examples are stored.
    pub fn add(&mut self, n: u64, example: impl FnOnce() -> String) {
        if n == 0 {
            return;
        }
        self.count = self.count.saturating_add(n);
        if self.examples.len() < MAX_EXAMPLES {
            self.examples.push(example());
        }
    }

    /// Appends a tally of later input (`self` precedes `later` in input order).
    pub fn merge(&mut self, later: Self) {
        self.count = self.count.saturating_add(later.count);
        let room = MAX_EXAMPLES.saturating_sub(self.examples.len());
        self.examples.extend(later.examples.into_iter().take(room));
    }

    /// `true` when nothing was recorded.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_snake_case() {
        let mut names: Vec<_> = Code::ALL.iter().map(|c| c.as_str()).collect();
        assert!(names.iter().all(|n| {
            n.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        }));
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n);
        for (i, c) in Code::ALL.iter().enumerate() {
            assert_eq!(*c as usize, i, "ALL must list codes in declaration order");
        }
    }

    #[test]
    fn tally_keeps_first_examples_in_order() {
        let mut a = Tally::default();
        for i in 0..3 {
            a.add(1, || format!("a{i}"));
        }
        let mut b = Tally::default();
        for i in 0..4 {
            b.add(2, || format!("b{i}"));
        }
        b.add(0, || unreachable!("zero-count adds are ignored"));
        a.merge(b);
        assert_eq!(a.count, 11);
        assert_eq!(a.examples, ["a0", "a1", "a2", "b0", "b1"]);
        let mut calls = 0;
        a.add(1, || {
            calls += 1;
            String::new()
        });
        assert_eq!(calls, 0, "examples are not built once the list is full");
    }

    #[test]
    fn diagnostics_serialise_and_sort() {
        let mut t = Tally::default();
        t.add(2, || "record 1".into());
        let d = Diagnostic::new(Code::InteriorWhitespace, "2 records", t).with_hint("h");
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(
            json,
            r#"{"code":"interior_whitespace","severity":"warning","message":"2 records","count":2,"examples":["record 1"],"hint":"h"}"#
        );
        let back: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        let mut v = vec![
            Diagnostic::new(Code::FlagHasNoEffect, "z", Tally::default()),
            Diagnostic {
                code: "from_the_future".into(),
                ..d.clone()
            },
            Diagnostic::new(Code::LowercaseFolded, "a", Tally::default()),
            d,
        ];
        sort(&mut v);
        let order: Vec<_> = v.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(
            order,
            [
                "lowercase_folded",
                "interior_whitespace",
                "flag_has_no_effect",
                "from_the_future"
            ]
        );
        assert_eq!(
            serde_json::to_string(&Code::TUFolded).unwrap(),
            r#""t_u_folded""#
        );
    }
}
