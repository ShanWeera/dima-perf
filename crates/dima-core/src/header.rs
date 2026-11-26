//! FASTA header metadata: `|`-separated fields named by `--header-format` (Tharanga et al.
//! 2025, "Metadata"), cleaned once and interned so the engine only handles small integers.
//!
//! * **Format.** Field names are trimmed and NFC-normalised, and must be non-empty and unique;
//!   `--metadata-fields` (default: all fields) must name a subset of them.
//! * **Split first.** The whole header line after `>` is split on `|` *before* any cleaning,
//!   so cleaning can never create or remove a delimiter.
//! * **Encoding (D48).** Invalid UTF-8 is an error in strict mode (with the record and byte);
//!   permissive and report modes replace it with U+FFFD and count the replacements.
//! * **Value cleaning (D59).** Remove format characters (General_Category Cf, e.g. zero-width
//!   spaces and bidirectional marks) and control characters except TAB; trim Unicode
//!   White_Space; normalise to NFC (UAX #15; W3C charmod-norm); an empty value becomes the fill
//!   value. Interior text is never changed. Every change is counted.
//! * **Mismatches (D18).** A header whose field count differs from the format is an error in
//!   strict mode (all headers are read first and the first 20 offenders are listed); in
//!   permissive and report modes its values become the fill value and a diagnostic is raised.
//! * **Diagnostics.** Near-duplicate values (equal after case folding, whitespace collapsing and
//!   NFKC), missing-like tokens (kept literally; INSDC missing-value vocabulary and common NA
//!   spellings) and identifier-like fields (every value unique).

use std::collections::{BTreeMap, HashMap};

use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

use crate::config::{ID_DIAG_MIN_RECORDS, MISMATCH_LOG_MAX, ValidationMode};
use crate::diagnostics::{Code, Diagnostic, Tally, agree, plural};
use crate::error::{DimaError, Location, Result};
use crate::model::{CodePointCount, HeaderCleaning, HeaderMismatch};

/// Splits a `--header-format` string such as `Accession|Host|Year` into names.
#[must_use]
pub fn split_format(text: &str) -> Vec<String> {
    text.split('|').map(str::to_owned).collect()
}

/// A field name or fill value as used: trimmed and NFC-normalised.
pub(crate) fn clean_name(name: &str) -> String {
    name.trim().nfc().collect()
}

/// Validated header format: field names, stratified fields and fill value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderSpec {
    fields: Vec<String>,
    stratified: Vec<usize>,
    fill: String,
}

impl HeaderSpec {
    /// Validates the format, the stratified subset and the fill value.
    ///
    /// # Errors
    /// A usage error naming the offending entry (and the valid names) when a name is empty or
    /// repeated, a metadata field is not in the format, or the fill value is empty.
    pub fn new(format: &[String], metadata_fields: Option<&[String]>, fill: &str) -> Result<Self> {
        if format.is_empty() {
            return Err(DimaError::usage("the header format names no fields"));
        }
        let fields: Vec<String> = format.iter().map(|f| clean_name(f)).collect();
        for (i, f) in fields.iter().enumerate() {
            if f.is_empty() {
                return Err(
                    DimaError::usage(format!("header format field {} has no name", i + 1))
                        .with_hint(
                            "name every '|'-separated field, e.g. 'Accession|Host|Country|Year'",
                        ),
                );
            }
            if fields[..i].contains(f) {
                return Err(DimaError::usage(format!(
                    "header format field {:?} appears more than once",
                    f
                ))
                .with_hint("give every field a distinct name"));
            }
        }
        let stratified = match metadata_fields {
            None => (0..fields.len()).collect(),
            Some(names) => {
                if names.is_empty() {
                    return Err(DimaError::usage("no metadata fields were given"));
                }
                let mut idx = Vec::with_capacity(names.len());
                for n in names {
                    let n = clean_name(n);
                    let Some(i) = fields.iter().position(|f| *f == n) else {
                        return Err(DimaError::usage(format!(
                            "metadata field {n:?} is not in the header format"
                        ))
                        .with_hint(format!("the header format names: {}", fields.join(", "))));
                    };
                    if idx.contains(&i) {
                        return Err(DimaError::usage(format!(
                            "metadata field {n:?} is listed more than once"
                        )));
                    }
                    idx.push(i);
                }
                // Keep the header-format order regardless of the order given.
                idx.sort_unstable();
                idx
            }
        };
        let fill = clean_name(fill);
        if fill.is_empty() {
            return Err(DimaError::usage("the header fill value must not be empty")
                .with_hint("empty values are what the fill value replaces; use e.g. Unknown"));
        }
        Ok(Self {
            fields,
            stratified,
            fill,
        })
    }

    /// Field names, in header order.
    #[must_use]
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Indices (into [`HeaderSpec::fields`]) of the stratified fields, in header order.
    #[must_use]
    pub fn stratified(&self) -> &[usize] {
        &self.stratified
    }

    /// Names of the stratified fields, in header order.
    #[must_use]
    pub fn stratified_names(&self) -> Vec<String> {
        self.stratified
            .iter()
            .map(|&i| self.fields[i].clone())
            .collect()
    }

    /// The fill value.
    #[must_use]
    pub fn fill(&self) -> &str {
        &self.fill
    }
}

/// Cleaning counters (D59), mergeable in input order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanStats {
    /// Values with leading or trailing white space.
    pub trimmed: u64,
    /// Values changed by NFC.
    pub nfc_changed: u64,
    /// Stratified values replaced by the fill value: empty values, and every stratified field of
    /// a header with the wrong number of fields.
    pub fill_uses: u64,
    /// Invisible code points removed, by code point.
    pub invisible: BTreeMap<char, u64>,
}

impl CleanStats {
    /// Adds counts of later input.
    pub fn merge(&mut self, other: Self) {
        self.trimmed += other.trimmed;
        self.nfc_changed += other.nfc_changed;
        self.fill_uses += other.fill_uses;
        for (c, n) in other.invisible {
            *self.invisible.entry(c).or_default() += n;
        }
    }

    /// `true` when nothing was changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.trimmed == 0
            && self.nfc_changed == 0
            && self.fill_uses == 0
            && self.invisible.is_empty()
    }
}

/// `true` for code points removed from header values: format characters (Cf) and controls (Cc)
/// other than TAB.
fn is_invisible(c: char) -> bool {
    (c.is_control() && c != '\t') || c.general_category() == GeneralCategory::Format
}

/// Cleans one header value (D59): remove Cf and Cc-except-TAB, trim White_Space, NFC; an empty
/// result becomes `fill`.
pub fn clean_value(raw: &str, fill: &str, stats: &mut CleanStats) -> String {
    // Fast path: printable ASCII needs at most trimming.
    if raw.bytes().all(|b| (0x20..0x7F).contains(&b)) {
        let t = raw.trim();
        if t.len() != raw.len() {
            stats.trimmed += 1;
        }
        if t.is_empty() {
            stats.fill_uses += 1;
            return fill.to_owned();
        }
        return t.to_owned();
    }
    let mut visible = String::with_capacity(raw.len());
    for c in raw.chars() {
        if is_invisible(c) {
            *stats.invisible.entry(c).or_default() += 1;
        } else {
            visible.push(c);
        }
    }
    let t = visible.trim();
    if t.len() != visible.len() {
        stats.trimmed += 1;
    }
    let value = if is_nfc_quick(t.chars()) == IsNormalized::Yes {
        t.to_owned()
    } else {
        let n: String = t.nfc().collect();
        if n != t {
            stats.nfc_changed += 1;
        }
        n
    };
    if value.is_empty() {
        stats.fill_uses += 1;
        return fill.to_owned();
    }
    value
}

/// Header-processing counters for one batch (or the whole run), mergeable in input order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderStats {
    /// Headers processed.
    pub records: u64,
    /// Invalid UTF-8 sequences replaced by U+FFFD.
    pub utf8_replacements: u64,
    /// Value cleaning.
    pub cleaning: CleanStats,
    /// Headers whose field count differs from the format.
    pub mismatch_count: u64,
    /// The first mismatches, in input order.
    pub mismatches: Vec<HeaderMismatch>,
}

impl HeaderStats {
    /// Adds counters of later input.
    pub fn merge(&mut self, other: Self) {
        self.records += other.records;
        self.utf8_replacements += other.utf8_replacements;
        self.cleaning.merge(other.cleaning);
        self.mismatch_count += other.mismatch_count;
        let room = MISMATCH_LOG_MAX.saturating_sub(self.mismatches.len());
        self.mismatches
            .extend(other.mismatches.into_iter().take(room));
    }
}

/// Where a header is in the input, for messages.
#[derive(Clone, Copy, Debug)]
pub struct HeaderPosition<'a> {
    /// Input name.
    pub source: &'a str,
    /// 1-based record number.
    pub record: u64,
    /// 1-based line number.
    pub line: u64,
    /// Byte offset of the `>`.
    pub byte: u64,
}

/// Parses one header (the bytes after `>`) into the cleaned values of the stratified fields.
///
/// Returns the fill value for every stratified field when the field count does not match
/// (recording the mismatch); the caller turns recorded mismatches into an error in strict mode
/// once all headers have been read.
///
/// # Errors
/// [`DimaError::InputData`] for invalid UTF-8 in strict mode.
pub fn parse_header(
    header: &[u8],
    spec: &HeaderSpec,
    mode: ValidationMode,
    at: HeaderPosition<'_>,
    stats: &mut HeaderStats,
) -> Result<Vec<String>> {
    stats.records += 1;
    let text = match std::str::from_utf8(header) {
        Ok(t) => std::borrow::Cow::Borrowed(t),
        Err(e) => {
            if mode == ValidationMode::Strict {
                let offset = at.byte + 1 + e.valid_up_to() as u64;
                return Err(DimaError::input_data(
                    Some(
                        Location::new(at.source)
                            .record(at.record)
                            .line(at.line)
                            .byte(offset),
                    ),
                    "header is not valid UTF-8",
                )
                .with_hint(
                    "convert the file to UTF-8, e.g. `iconv -f LATIN1 -t UTF-8 in.fasta > \
                     out.fasta`, or use --validation permissive to replace invalid bytes",
                ));
            }
            stats.utf8_replacements += header
                .utf8_chunks()
                .filter(|c| !c.invalid().is_empty())
                .count() as u64;
            String::from_utf8_lossy(header)
        }
    };
    let parts: Vec<&str> = text.split('|').collect();
    if parts.len() != spec.fields.len() {
        stats.mismatch_count += 1;
        if stats.mismatches.len() < MISMATCH_LOG_MAX {
            stats.mismatches.push(HeaderMismatch {
                record: at.record,
                line: at.line,
                header: text.clone().into_owned(),
                expected: spec.fields.len() as u64,
                found: parts.len() as u64,
            });
        }
        stats.cleaning.fill_uses += spec.stratified.len() as u64;
        return Ok(vec![spec.fill.clone(); spec.stratified.len()]);
    }
    Ok(spec
        .stratified
        .iter()
        .map(|&i| clean_value(parts[i], &spec.fill, &mut stats.cleaning))
        .collect())
}

/// The error raised in strict mode when headers do not match the format.
#[must_use]
pub fn mismatch_error(source: &str, spec: &HeaderSpec, stats: &HeaderStats) -> DimaError {
    let mut msg = format!(
        "{} not have the {} of the header format ({})",
        plural(stats.mismatch_count, "header does", "headers do"),
        plural(spec.fields.len() as u64, "field", "fields"),
        spec.fields.join("|")
    );
    for m in &stats.mismatches {
        msg.push_str(&format!(
            "\n  record {} (line {}): {}: {}",
            m.record,
            m.line,
            plural(m.found, "field", "fields"),
            truncate_for_display(&m.header, 120)
        ));
    }
    if stats.mismatch_count > stats.mismatches.len() as u64 {
        msg.push_str(&format!(
            "\n  and {} more",
            stats.mismatch_count - stats.mismatches.len() as u64
        ));
    }
    DimaError::input_data(Some(Location::new(source)), msg).with_hint(
        "fix the headers, change --header-format, or use --validation permissive to give \
         mismatched headers the fill value",
    )
}

fn truncate_for_display(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((i, _)) => format!("{}...", &s[..i]),
        None => s.to_owned(),
    }
}

/// Interned header values: each distinct (field, value) pair gets a small integer id.
#[derive(Clone, Debug, Default)]
pub struct ValueDictionary {
    ids: Vec<HashMap<String, u32>>,
    entries: Vec<(usize, String, u64)>,
}

impl ValueDictionary {
    /// An empty dictionary for `fields` stratified fields.
    #[must_use]
    pub fn new(fields: usize) -> Self {
        Self {
            ids: vec![HashMap::new(); fields],
            entries: Vec::new(),
        }
    }

    /// The id of `value` in stratified field `field` (0-based among stratified fields),
    /// counting one more record.
    ///
    /// # Errors
    /// [`DimaError::Overflow`] beyond 2³² distinct values.
    pub fn intern(&mut self, field: usize, value: String) -> Result<u32> {
        if let Some(&id) = self.ids[field].get(&value) {
            self.entries[id as usize].2 += 1;
            return Ok(id);
        }
        let id = u32::try_from(self.entries.len()).map_err(|_| DimaError::Overflow {
            what: "the number of distinct header values",
        })?;
        self.ids[field].insert(value.clone(), id);
        self.entries.push((field, value, 1));
        Ok(id)
    }

    /// `(stratified field index, value, records)` of an id.
    #[must_use]
    pub fn entry(&self, id: u32) -> (usize, &str, u64) {
        let (f, v, n) = &self.entries[id as usize];
        (*f, v, *n)
    }

    /// Number of distinct values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when no value was interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Distinct values of one stratified field with their record counts, sorted by value bytes.
    #[must_use]
    pub fn values_of(&self, field: usize) -> Vec<(&str, u64)> {
        let mut v: Vec<(&str, u64)> = self
            .entries
            .iter()
            .filter(|e| e.0 == field)
            .map(|e| (e.1.as_str(), e.2))
            .collect();
        v.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        v
    }
}

/// Tokens that usually mean "missing" (matched case-insensitively): the INSDC missing-value
/// vocabulary and common spellings of NA.
const MISSING_LIKE: &[&str] = &[
    "missing",
    "not applicable",
    "not collected",
    "not provided",
    "restricted access",
    "na",
    "n/a",
    "nan",
    "null",
    "none",
];

fn is_missing_like(v: &str, fill: &str) -> bool {
    if v == fill {
        return false;
    }
    let l = v.to_lowercase();
    MISSING_LIKE.contains(&l.as_str()) || l.starts_with("missing:")
}

/// Key under which values count as near-duplicates: lower case, whitespace runs collapsed to a
/// single space, NFKC.
fn near_duplicate_key(v: &str) -> String {
    let lower = v.to_lowercase();
    let collapsed = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.nfkc().collect()
}

/// Diagnostics about the values of the stratified fields: missing-like tokens and
/// near-duplicates, per field.
#[must_use]
pub fn value_diagnostics(spec: &HeaderSpec, dict: &ValueDictionary) -> Vec<Diagnostic> {
    let names = spec.stratified_names();
    let mut out = Vec::new();
    for (f, name) in names.iter().enumerate() {
        let values = dict.values_of(f);
        let mut missing = Tally::default();
        for &(v, n) in &values {
            if is_missing_like(v, spec.fill()) {
                missing.add(n, || format!("{v:?} ({})", plural(n, "record", "records")));
            }
        }
        if !missing.is_empty() {
            out.push(
                Diagnostic::new(
                    Code::MissingLikeValues,
                    format!(
                        "field {name:?}: {} that usually {} \"missing\"; {}",
                        plural(missing.count, "record has a value", "records have values"),
                        agree(missing.count, "means", "mean"),
                        agree(
                            missing.count,
                            "it is kept as a literal value",
                            "they are kept as literal values"
                        )
                    ),
                    missing,
                )
                .with_hint(
                    "if they mean missing, empty them in the headers so they receive the fill \
                     value, or edit them consistently",
                ),
            );
        }
        let mut groups: BTreeMap<String, Vec<(&str, u64)>> = BTreeMap::new();
        for &(v, n) in &values {
            groups
                .entry(near_duplicate_key(v))
                .or_default()
                .push((v, n));
        }
        let mut dups = Tally::default();
        for g in groups.values().filter(|g| g.len() > 1) {
            dups.add(1, || {
                g.iter()
                    .map(|(v, n)| format!("{v:?} ({n})"))
                    .collect::<Vec<_>>()
                    .join(" / ")
            });
        }
        if !dups.is_empty() {
            out.push(
                Diagnostic::new(
                    Code::NearDuplicateValues,
                    format!(
                        "field {name:?}: {} of values {} only in case, spacing or character \
                         forms and {} counted separately",
                        plural(dups.count, "group", "groups"),
                        agree(dups.count, "differs", "differ"),
                        agree(dups.count, "is", "are")
                    ),
                    dups,
                )
                .with_hint("make the spellings consistent in the headers if they mean the same"),
            );
        }
    }
    out
}

/// Diagnostic for stratified fields whose values were all distinct in the probe (likely
/// identifiers such as accessions), which multiply the metadata held per variant.
#[must_use]
pub fn identifier_diagnostic(
    spec: &HeaderSpec,
    probe_values: &[Vec<String>],
    probe_records: u64,
) -> Option<Diagnostic> {
    if probe_records < ID_DIAG_MIN_RECORDS as u64 {
        return None;
    }
    let names = spec.stratified_names();
    let mut tally = Tally::default();
    let mut flagged = Vec::new();
    for (f, values) in probe_values.iter().enumerate() {
        let mut distinct: Vec<&String> = values.iter().collect();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() as u64 == probe_records {
            tally.add(1, || names[f].clone());
            flagged.push(names[f].clone());
        }
    }
    if tally.is_empty() {
        return None;
    }
    let keep: Vec<&String> = names.iter().filter(|n| !flagged.contains(n)).collect();
    let hint = if keep.is_empty() {
        "every stratified field is identifier-like; omit --header-format if no stratification \
         is needed"
            .to_owned()
    } else {
        format!(
            "to stratify without them: --metadata-fields '{}'",
            keep.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("|")
        )
    };
    Some(
        Diagnostic::new(
            Code::IdentifierLikeField,
            format!(
                "{} {} had a different value in each of the first {probe_records} records; \
                 identifier-like fields add one metadata entry per sequence to every variant, \
                 which makes the results large",
                agree(flagged.len() as u64, "field", "fields"),
                flagged
                    .iter()
                    .map(|f| format!("{f:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            tally,
        )
        .with_hint(hint),
    )
}

/// Provenance of the cleaning (D59, D97).
#[must_use]
pub fn cleaning_record(stats: &CleanStats) -> HeaderCleaning {
    let (a, b, c) = unicode_normalization::UNICODE_VERSION;
    HeaderCleaning {
        unicode_version: format!("{a}.{b}.{c}"),
        trimmed: stats.trimmed,
        invisible_removed: stats
            .invisible
            .iter()
            .map(|(c, n)| CodePointCount {
                code_point: format!("U+{:04X}", u32::from(*c)),
                count: *n,
            })
            .collect(),
        nfc_changed: stats.nfc_changed,
        fill_uses: stats.fill_uses,
    }
}

/// Diagnostic summarising value cleaning, if anything was changed.
#[must_use]
pub fn cleaning_diagnostic(stats: &CleanStats) -> Option<Diagnostic> {
    if stats.is_empty() {
        return None;
    }
    let invisible: u64 = stats.invisible.values().sum();
    let mut tally = Tally::default();
    for (c, n) in &stats.invisible {
        tally.add(*n, || {
            format!(
                "U+{:04X} ({})",
                u32::from(*c),
                crate::human::plural(*n, "time", "times")
            )
        });
    }
    tally.count = stats.trimmed + stats.nfc_changed + stats.fill_uses + invisible;
    Some(Diagnostic::new(
        Code::HeaderValuesCleaned,
        format!(
            "header values cleaned: {} trimmed, {} removed, {} normalised to NFC, {} filled",
            stats.trimmed,
            plural(invisible, "invisible character", "invisible characters"),
            stats.nfc_changed,
            plural(
                stats.fill_uses,
                "empty or mismatched value",
                "empty or mismatched values"
            )
        ),
        tally,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(format: &str) -> HeaderSpec {
        HeaderSpec::new(&split_format(format), None, "Unknown").unwrap()
    }

    #[test]
    fn spec_validation() {
        let s = HeaderSpec::new(
            &split_format(" Accession | Host|Year"),
            Some(&["Year".into(), "Accession".into()]),
            "NA",
        )
        .unwrap();
        assert_eq!(s.fields(), ["Accession", "Host", "Year"]);
        assert_eq!(
            s.stratified(),
            [0, 2],
            "header order regardless of the order given"
        );
        assert_eq!(s.stratified_names(), ["Accession", "Year"]);
        for (format, fields, fill, needle) in [
            ("A||B", None, "x", "field 2 has no name"),
            ("A|B|A", None, "x", "appears more than once"),
            (
                "A|B",
                Some(vec!["C".to_owned()]),
                "x",
                "not in the header format",
            ),
            ("A|B", Some(vec![]), "x", "no metadata fields"),
            (
                "A|B",
                Some(vec!["A".to_owned(), " A".to_owned()]),
                "x",
                "more than once",
            ),
            ("A|B", None, " \u{3000}", "fill value"),
        ] {
            let e = HeaderSpec::new(&split_format(format), fields.as_deref(), fill).unwrap_err();
            assert!(e.to_string().contains(needle), "{format}: {e}");
            assert_eq!(e.class(), crate::error::ExitClass::Usage);
        }
        let e = HeaderSpec::new(&split_format("A|B"), Some(&["Hots".into()]), "x").unwrap_err();
        assert!(e.hint().unwrap().contains("A, B"));
        // NFC: a decomposed name equals its composed form.
        let e =
            HeaderSpec::new(&["Cafe\u{301}".into(), "Caf\u{e9}".into()], None, "x").unwrap_err();
        assert!(e.to_string().contains("more than once"));
    }

    #[test]
    fn value_cleaning() {
        let mut st = CleanStats::default();
        assert_eq!(clean_value("  Human ", "U", &mut st), "Human");
        assert_eq!(clean_value("", "U", &mut st), "U");
        assert_eq!(clean_value("\u{200B}\u{FEFF}", "U", &mut st), "U");
        assert_eq!(
            clean_value("Sa\u{300}o\tPaulo", "U", &mut st),
            "S\u{e0}o\tPaulo"
        );
        assert_eq!(
            clean_value("\u{202F}Riyadh\u{2003}", "U", &mut st),
            "Riyadh"
        );
        assert_eq!(clean_value("Ri\u{200D}yadh\u{7}", "U", &mut st), "Riyadh");
        assert_eq!(
            clean_value("x\u{85}", "U", &mut st),
            "x",
            "NEL is a C1 control (and White_Space)"
        );
        // Interior text, including visible spaces, is untouched; NFKC is never applied.
        assert_eq!(
            clean_value("Saudi  Arabia ﬁ", "U", &mut st),
            "Saudi  Arabia ﬁ"
        );
        assert_eq!(st.trimmed, 2);
        assert_eq!(st.nfc_changed, 1);
        assert_eq!(st.fill_uses, 2);
        assert_eq!(st.invisible.get(&'\u{200B}'), Some(&1));
        assert_eq!(st.invisible.get(&'\u{7}'), Some(&1));
        assert_eq!(
            st.invisible.get(&'\u{202F}'),
            None,
            "U+202F is a visible (white) space"
        );
        let rec = cleaning_record(&st);
        assert!(
            rec.invisible_removed
                .iter()
                .any(|c| c.code_point == "U+200B")
        );
        assert!(cleaning_diagnostic(&st).is_some());
        assert!(cleaning_diagnostic(&CleanStats::default()).is_none());
    }

    #[test]
    fn header_parsing_and_mismatches() {
        let s = spec("Accession|Host|Year");
        let at = HeaderPosition {
            source: "x.fa",
            record: 3,
            line: 7,
            byte: 100,
        };
        let mut st = HeaderStats::default();
        let v = parse_header(b"KX1| Camel |2015", &s, ValidationMode::Strict, at, &mut st).unwrap();
        assert_eq!(v, ["KX1", "Camel", "2015"]);
        let v = parse_header(b"KX2|Human", &s, ValidationMode::Strict, at, &mut st).unwrap();
        assert_eq!(v, ["Unknown"; 3]);
        assert_eq!(st.mismatch_count, 1);
        assert_eq!(st.mismatches[0].found, 2);
        assert_eq!(st.cleaning.fill_uses, 3);
        let e = mismatch_error("x.fa", &s, &st);
        assert!(
            e.to_string()
                .contains("record 3 (line 7): 2 fields: KX2|Human"),
            "{e}"
        );
        // Invalid UTF-8.
        let bad = b"KX3|Cam\xE9l|2015";
        let e = parse_header(bad, &s, ValidationMode::Strict, at, &mut st).unwrap_err();
        assert!(e.to_string().contains("record 3 (line 7, byte 108)"), "{e}");
        assert!(e.hint().unwrap().contains("iconv"));
        let v = parse_header(bad, &s, ValidationMode::Permissive, at, &mut st).unwrap();
        assert_eq!(v[1], "Cam\u{FFFD}l");
        assert_eq!(st.utf8_replacements, 1);
        // Pipes inside values are not possible: splitting happens first.
        let v = parse_header(
            "A|B|\u{FF5C}".as_bytes(),
            &s,
            ValidationMode::Strict,
            at,
            &mut st,
        )
        .unwrap();
        assert_eq!(
            v[2], "\u{FF5C}",
            "a full-width bar stays a value; NFC does not map it to '|'"
        );
    }

    #[test]
    fn mismatch_log_is_capped_in_order() {
        let s = spec("A|B");
        let mut total = HeaderStats::default();
        for batch in 0..3 {
            let mut st = HeaderStats::default();
            for r in 0..10 {
                let at = HeaderPosition {
                    source: "x",
                    record: batch * 10 + r + 1,
                    line: 1,
                    byte: 0,
                };
                parse_header(b"only-one", &s, ValidationMode::Permissive, at, &mut st).unwrap();
            }
            total.merge(st);
        }
        assert_eq!(total.mismatch_count, 30);
        assert_eq!(total.mismatches.len(), MISMATCH_LOG_MAX);
        assert_eq!(total.mismatches[19].record, 20);
        let e = mismatch_error("x", &s, &total);
        assert!(e.to_string().ends_with("\n  and 10 more"));
    }

    #[test]
    fn dictionary_and_value_diagnostics() {
        let s = spec("Host|Country");
        let mut d = ValueDictionary::new(2);
        for (h, c) in [
            ("Human", "Saudi Arabia"),
            ("human", "Saudi  Arabia"),
            ("Camel", "NA"),
            ("Human", "missing: control sample"),
            ("Camel", "Unknown"),
        ] {
            d.intern(0, h.into()).unwrap();
            d.intern(1, c.into()).unwrap();
        }
        assert_eq!(d.len(), 8);
        assert_eq!(d.values_of(0), [("Camel", 2), ("Human", 2), ("human", 1)]);
        let diags = value_diagnostics(&s, &d);
        let codes: Vec<_> = diags.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(
            codes,
            [
                "near_duplicate_values",
                "missing_like_values",
                "near_duplicate_values"
            ]
        );
        assert!(
            diags[1].message.contains("2 records"),
            "the fill value itself is not flagged: {}",
            diags[1].message
        );
        assert!(diags[0].examples[0].contains("\"Human\" (2) / \"human\" (1)"));
    }

    #[test]
    fn identifier_like_fields() {
        let s = spec("Accession|Host");
        let ids: Vec<String> = (0..150).map(|i| format!("ACC{i}")).collect();
        let hosts: Vec<String> = (0..150)
            .map(|i| ["Camel", "Human"][i % 2].to_owned())
            .collect();
        let d = identifier_diagnostic(&s, &[ids.clone(), hosts.clone()], 150).unwrap();
        assert!(d.message.contains("\"Accession\""));
        assert_eq!(
            d.hint.as_deref(),
            Some("to stratify without them: --metadata-fields 'Host'")
        );
        assert!(
            identifier_diagnostic(&s, &[ids[..50].to_vec(), hosts[..50].to_vec()], 50).is_none(),
            "too few records"
        );
        assert!(identifier_diagnostic(&s, &[hosts.clone(), hosts], 150).is_none());
    }
}
