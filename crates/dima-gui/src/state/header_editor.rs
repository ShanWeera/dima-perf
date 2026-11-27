//! The header-format editor (D71): one row per `|`-separated field of the first header, named
//! with one of the paper's tags or a custom name, with a preview of sample values and a
//! "stratify" choice. Rows start as `field_1…field_n`, never as values from the file (G36); the
//! editor is always available, also when the first header has a single field, and a format can
//! be pasted (G37). Names are validated by the library (`HeaderSpec`), which also rejects empty
//! and duplicate names (G2, G8). Stratification is per row, so renaming a field keeps it (G9).

use dima_core::config::DEFAULT_FILL;
use dima_core::header::{HeaderSpec, split_format};
use dima_core::validation::HeaderField;

/// Why stratified fields cannot be used when no field is stratified: the app's words for the
/// library's "no metadata fields were given".
pub const NO_STRATIFIED_FIELD: &str = "tick at least one Stratify box";

/// The predefined metadata tags of the paper (Tharanga et al. 2025, "Metadata").
pub const PAPER_TAGS: [&str; 6] = [
    "Accession",
    "Strain",
    "Species",
    "Year",
    "Geolocation",
    "Host",
];

/// One header field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderRow {
    /// Field name.
    pub name: String,
    /// Stratify variants by this field.
    pub stratify: bool,
    /// Up to three sample values from the pre-check.
    pub samples: Vec<String>,
    /// Every probed value was different (an identifier, D57).
    pub identifier_like: bool,
}

impl HeaderRow {
    fn new(i: usize) -> Self {
        Self {
            name: format!("field_{}", i + 1),
            stratify: true,
            samples: Vec::new(),
            identifier_like: false,
        }
    }

    /// The paper tag this row is named with, if any.
    #[must_use]
    pub fn tag(&self) -> Option<&'static str> {
        PAPER_TAGS.iter().copied().find(|t| *t == self.name)
    }
}

/// The editor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderEditor {
    /// Pass a header format to the run (off by default, as on the command line).
    pub enabled: bool,
    /// The fields.
    pub rows: Vec<HeaderRow>,
    /// Fields of the first header in the pre-check.
    pub detected_fields: usize,
    /// Probed headers whose field count differs from the first.
    pub mismatched_headers: u64,
}

impl HeaderEditor {
    /// Rows for the fields found by the pre-check.
    #[must_use]
    pub fn from_inspection(fields: &[HeaderField], mismatched_headers: u64) -> Self {
        Self {
            enabled: false,
            rows: fields
                .iter()
                .enumerate()
                .map(|(i, f)| HeaderRow {
                    samples: f.samples.clone(),
                    identifier_like: f.identifier_like,
                    ..HeaderRow::new(i)
                })
                .collect(),
            detected_fields: fields.len(),
            mismatched_headers,
        }
    }

    /// The field names, in header order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.rows.iter().map(|r| r.name.clone()).collect()
    }

    /// The stratified fields, or `None` when every field is stratified (the library's default).
    #[must_use]
    pub fn metadata_fields(&self) -> Option<Vec<String>> {
        if self.rows.iter().all(|r| r.stratify) {
            None
        } else {
            Some(
                self.rows
                    .iter()
                    .filter(|r| r.stratify)
                    .map(|r| r.name.clone())
                    .collect(),
            )
        }
    }

    /// What keeps the fields from being used, in the app's words, `None` when nothing does: a
    /// name the library refuses (`HeaderSpec`: empty or repeated, its message without the
    /// command line's hint), or no Stratify box ticked. The fill value is an option of its own,
    /// checked with the options.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        let names = self.names();
        if let Err(e) = HeaderSpec::new(&names, None, DEFAULT_FILL) {
            return Some(e.to_string());
        }
        if !self.rows.iter().any(|r| r.stratify) {
            return Some(NO_STRATIFIED_FIELD.to_owned());
        }
        HeaderSpec::new(&names, self.metadata_fields().as_deref(), DEFAULT_FILL)
            .err()
            .map(|e| e.to_string())
    }

    /// Renames the rows from a list; extra names add rows, missing ones remove rows. Samples
    /// stay with their column.
    pub fn set_names(&mut self, names: &[String]) {
        self.rows.truncate(names.len());
        while self.rows.len() < names.len() {
            self.rows.push(HeaderRow::new(self.rows.len()));
        }
        for (row, name) in self.rows.iter_mut().zip(names) {
            row.name = name.trim().to_owned();
        }
    }

    /// Applies a pasted `--header-format` value (`Accession|Host|Year`, optionally quoted).
    /// Returns a note when the field count differs from the file's first header.
    pub fn paste(&mut self, text: &str) -> Option<String> {
        let t = text.trim();
        let t = t
            .strip_prefix("--header-format")
            .map_or(t, str::trim_start)
            .trim_start_matches('=')
            .trim()
            .trim_matches(|c| c == '\'' || c == '"');
        let names = split_format(t);
        self.set_names(&names);
        self.enabled = true;
        (self.detected_fields > 0 && names.len() != self.detected_fields).then(|| {
            format!(
                "{} fields pasted, but the first header has {}: headers that do not match are \
                 an error in strict mode",
                names.len(),
                self.detected_fields
            )
        })
    }

    /// The equivalent command-line options (`--header-format '…'` and, for a subset,
    /// `--metadata-fields '…'`), quoted for a POSIX shell.
    #[must_use]
    pub fn cli_args(&self) -> String {
        let mut s = format!(
            "--header-format {}",
            crate::util::shell_quote(&self.names().join("|"))
        );
        if let Some(fields) = self.metadata_fields() {
            s.push_str(&format!(
                " --metadata-fields {}",
                crate::util::shell_quote(&fields.join("|"))
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(n: usize) -> Vec<HeaderField> {
        (0..n)
            .map(|i| HeaderField {
                samples: vec![format!("v{i}a"), format!("v{i}b")],
                identifier_like: i == 0,
            })
            .collect()
    }

    #[test]
    fn rows_default_to_field_names_never_values() {
        let e = HeaderEditor::from_inspection(&fields(3), 0);
        assert!(!e.enabled, "off by default, like the command line");
        assert_eq!(e.names(), ["field_1", "field_2", "field_3"]);
        assert_eq!(e.rows[0].samples, ["v0a", "v0b"]);
        assert!(e.rows[0].identifier_like);
        assert_eq!(
            e.metadata_fields(),
            None,
            "all stratified = the library default"
        );
        assert_eq!(e.problem(), None);
    }

    #[test]
    fn renaming_keeps_the_stratify_choice() {
        let mut e = HeaderEditor::from_inspection(&fields(3), 0);
        e.rows[1].stratify = false;
        e.rows[1].name = "Host".into();
        assert_eq!(
            e.metadata_fields(),
            Some(vec!["field_1".into(), "field_3".into()])
        );
        assert_eq!(e.rows[1].tag(), Some("Host"));
        assert_eq!(e.rows[0].tag(), None);
    }

    /// The problems shown in the app: the library's messages without the command line's hints,
    /// and the Stratify boxes for "no metadata fields were given".
    #[test]
    fn problems_in_the_apps_words() {
        let mut e = HeaderEditor::from_inspection(&fields(3), 0);
        assert_eq!(e.problem(), None);
        e.rows[2].name = "field_1".into();
        assert_eq!(
            e.problem().as_deref(),
            Some("header format field \"field_1\" appears more than once")
        );
        e.rows[2].name = "  ".into();
        assert_eq!(
            e.problem().as_deref(),
            Some("header format field 3 has no name")
        );
        e.rows[2].name = "Host".into();
        for r in &mut e.rows {
            r.stratify = false;
        }
        assert_eq!(e.problem().as_deref(), Some(NO_STRATIFIED_FIELD));
        assert_eq!(NO_STRATIFIED_FIELD, "tick at least one Stratify box");
        // A name problem is named first: it is what the user sees in the editor's rows.
        e.rows[0].name = String::new();
        assert_eq!(
            e.problem().as_deref(),
            Some("header format field 1 has no name")
        );
        // Every row's name and choice as the library takes them: what the editor accepts the
        // library accepts (with the default fill value).
        let ok = HeaderEditor::from_inspection(&fields(2), 0);
        assert_eq!(ok.problem(), None);
        assert!(HeaderSpec::new(&ok.names(), ok.metadata_fields().as_deref(), "Unknown").is_ok());
    }

    #[test]
    fn paste_and_copy() {
        let mut e = HeaderEditor::from_inspection(&fields(4), 2);
        assert_eq!(e.mismatched_headers, 2);
        let note = e.paste("--header-format 'Accession|Host|Country|Year'");
        assert_eq!(note, None);
        assert!(e.enabled);
        assert_eq!(e.names(), ["Accession", "Host", "Country", "Year"]);
        assert_eq!(
            e.rows[2].samples,
            ["v2a", "v2b"],
            "samples stay with their column"
        );
        assert_eq!(
            e.cli_args(),
            "--header-format 'Accession|Host|Country|Year'"
        );
        e.rows[0].stratify = false;
        assert_eq!(
            e.cli_args(),
            "--header-format 'Accession|Host|Country|Year' --metadata-fields 'Host|Country|Year'"
        );
        let note = e.paste("A|B");
        assert!(note.unwrap().contains("2 fields pasted"));
        assert_eq!(e.rows.len(), 2);
        e.paste("\"X|Y|Z|W|V\"");
        assert_eq!(e.rows.len(), 5);
        assert_eq!(e.rows[4].name, "V");
        assert!(e.rows[4].samples.is_empty());
    }
}
