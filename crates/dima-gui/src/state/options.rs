//! The analysis options of the setup form and their mapping to the library's parameters.
//!
//! Defaults are the library's (D6, D36, D72); a value equal to its default is passed as
//! "not set", so the provenance records it as a default and the equivalent command (D103) shows
//! only what differs, exactly like the command line. The form never caps a value below what the
//! library accepts (D72): k is bounded by the alignment length only.

use dima_core::config::{
    Alphabet, AnalysisParams, DEFAULT_FILL, DEFAULT_SUPPORT_THRESHOLD, Estimator, ValidationMode,
};

use super::header_editor::HeaderEditor;
use super::settings::Settings;

/// The alphabet choice of the form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlphabetChoice {
    /// Detected from the data (Easel's rules).
    #[default]
    Auto,
    /// Protein.
    Protein,
    /// DNA.
    Dna,
    /// RNA.
    Rna,
}

impl AlphabetChoice {
    /// Every choice, in menu order.
    pub const ALL: [Self; 4] = [Self::Auto, Self::Protein, Self::Dna, Self::Rna];

    /// The library value (`None` = detect).
    #[must_use]
    pub const fn alphabet(self) -> Option<Alphabet> {
        match self {
            Self::Auto => None,
            Self::Protein => Some(Alphabet::Protein),
            Self::Dna => Some(Alphabet::Dna),
            Self::Rna => Some(Alphabet::Rna),
        }
    }

    /// Menu label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Protein => "Protein",
            Self::Dna => "DNA",
            Self::Rna => "RNA",
        }
    }
}

/// Human name of an estimator.
#[must_use]
pub const fn estimator_label(e: Estimator) -> &'static str {
    match e {
        Estimator::Dima => "DiMA (paper, Fig. 2)",
        Estimator::ChaoWangJost => "Chao\u{2013}Wang\u{2013}Jost",
        Estimator::MillerMadow => "Miller\u{2013}Madow",
        Estimator::Raw => "Raw (plug-in)",
    }
}

/// Human name of a validation mode.
#[must_use]
pub const fn validation_label(v: ValidationMode) -> &'static str {
    match v {
        ValidationMode::Strict => "Strict",
        ValidationMode::Permissive => "Permissive",
        ValidationMode::Report => "Report",
    }
}

/// The form's values.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisOptions {
    /// Alphabet (per file, G12).
    pub alphabet: AlphabetChoice,
    /// k; `None` = the alphabet's default (per file, G12).
    pub kmer_length: Option<usize>,
    /// Query name; empty = derived from the file name (per file, G13).
    pub query_name: String,
    /// Support threshold T.
    pub support_threshold: u64,
    /// Entropy estimator.
    pub estimator: Estimator,
    /// Validation mode.
    pub validation: ValidationMode,
    /// Flag positions with sample coverage below this value; `None` = off.
    pub min_coverage: Option<f64>,
    /// Fill value for empty header fields (validated, never written during drawing: G52).
    pub fill: String,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            alphabet: AlphabetChoice::Auto,
            kmer_length: None,
            query_name: String::new(),
            support_threshold: DEFAULT_SUPPORT_THRESHOLD,
            estimator: Estimator::default(),
            validation: ValidationMode::default(),
            min_coverage: None,
            fill: DEFAULT_FILL.to_owned(),
        }
    }
}

impl AnalysisOptions {
    /// The remembered options (D73) with per-file values at their defaults.
    #[must_use]
    pub fn from_settings(s: &Settings) -> Self {
        Self {
            support_threshold: s.support_threshold,
            estimator: s.estimator,
            validation: s.validation,
            min_coverage: s.min_coverage,
            fill: s.fill.clone(),
            ..Self::default()
        }
    }

    /// Copies the remembered options into `s`.
    pub fn store(&self, s: &mut Settings) {
        s.support_threshold = self.support_threshold;
        s.estimator = self.estimator;
        s.validation = self.validation;
        s.min_coverage = self.min_coverage;
        if !self.fill.trim().is_empty() {
            s.fill.clone_from(&self.fill);
        }
    }

    /// Resets the values that belong to one file (G12, G13).
    pub fn reset_for_new_file(&mut self) {
        self.alphabet = AlphabetChoice::Auto;
        self.kmer_length = None;
        self.query_name.clear();
    }

    /// The alphabet the run will use: the explicit choice, else the detected one.
    #[must_use]
    pub fn effective_alphabet(&self, detected: Option<Alphabet>) -> Option<Alphabet> {
        self.alphabet.alphabet().or(detected)
    }

    /// The default k for the effective alphabet.
    #[must_use]
    pub fn default_kmer(&self, detected: Option<Alphabet>) -> Option<usize> {
        self.effective_alphabet(detected)
            .map(Alphabet::default_kmer_length)
    }

    /// `true` when any option differs from its default (for "Reset to defaults").
    #[must_use]
    pub fn differs_from_defaults(&self) -> bool {
        *self != Self::default()
    }

    /// The library parameters. `header` contributes the header format and stratified fields
    /// when metadata is enabled; the fill value is passed only then (it has no effect
    /// otherwise). Values equal to their defaults are passed as unset.
    #[must_use]
    pub fn to_params(&self, header: &HeaderEditor) -> AnalysisParams {
        let metadata = header.enabled;
        let query = self.query_name.trim();
        AnalysisParams {
            alphabet: self.alphabet.alphabet(),
            kmer_length: self.kmer_length,
            support_threshold: (self.support_threshold != DEFAULT_SUPPORT_THRESHOLD)
                .then_some(self.support_threshold),
            estimator: (self.estimator != Estimator::default()).then_some(self.estimator),
            validation: (self.validation != ValidationMode::default()).then_some(self.validation),
            min_coverage: self.min_coverage,
            query_name: (!query.is_empty()).then(|| query.to_owned()),
            header_format: metadata.then(|| header.names()),
            metadata_fields: if metadata {
                header.metadata_fields()
            } else {
                None
            },
            header_fill: (metadata && self.fill != DEFAULT_FILL).then(|| self.fill.clone()),
        }
    }

    /// Problems that would make the run fail before reading, from the library's own checks
    /// (G2, G8, G52), plus k against the inspected alignment length (D36).
    #[must_use]
    pub fn problems(&self, header: &HeaderEditor, alignment_length: Option<usize>) -> Vec<String> {
        let params = self.to_params(header);
        let mut out = Vec::new();
        if let Err(e) = params.validate() {
            out.push(e.report());
        }
        if header.enabled
            && let Err(e) = header.validate(&self.fill)
        {
            out.push(e.report());
        }
        if self.fill.trim().is_empty() {
            out.push("the fill value must not be empty".to_owned());
        }
        if let (Some(k), Some(l)) = (self.kmer_length, alignment_length)
            && k > l
        {
            out.push(format!(
                "k = {k} exceeds the alignment length ({l} columns)"
            ));
        }
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_passed_as_unset() {
        let o = AnalysisOptions::default();
        assert_eq!(
            o.to_params(&HeaderEditor::default()),
            AnalysisParams::default()
        );
        assert!(!o.differs_from_defaults());
    }

    #[test]
    fn non_defaults_are_passed() {
        let mut header = HeaderEditor::default();
        header.set_names(&["Accession".into(), "Host".into()]);
        header.enabled = true;
        header.rows[0].stratify = false;
        let o = AnalysisOptions {
            alphabet: AlphabetChoice::Dna,
            kmer_length: Some(12),
            query_name: "  spike ".into(),
            support_threshold: 30,
            estimator: Estimator::Raw,
            validation: ValidationMode::Permissive,
            min_coverage: Some(0.9),
            fill: "NA".into(),
        };
        let p = o.to_params(&header);
        assert_eq!(p.alphabet, Some(Alphabet::Dna));
        assert_eq!(p.query_name.as_deref(), Some("spike"));
        assert_eq!(p.support_threshold, Some(30));
        assert_eq!(
            p.header_format,
            Some(vec!["Accession".into(), "Host".into()])
        );
        assert_eq!(p.metadata_fields, Some(vec!["Host".into()]));
        assert_eq!(p.header_fill.as_deref(), Some("NA"));
        assert!(o.differs_from_defaults());
        assert_eq!(
            p.to_args(),
            [
                "--alphabet",
                "dna",
                "--kmer-length",
                "12",
                "--support-threshold",
                "30",
                "--estimator",
                "raw",
                "--validation",
                "permissive",
                "--min-coverage",
                "0.9",
                "--query-name",
                "spike",
                "--header-format",
                "Accession|Host",
                "--metadata-fields",
                "Host",
                "--header-fillna",
                "NA"
            ]
        );
    }

    #[test]
    fn fill_without_metadata_is_not_passed() {
        let o = AnalysisOptions {
            fill: "NA".into(),
            ..AnalysisOptions::default()
        };
        assert_eq!(o.to_params(&HeaderEditor::default()).header_fill, None);
    }

    #[test]
    fn problems_come_from_the_library() {
        let mut o = AnalysisOptions::default();
        assert!(o.problems(&HeaderEditor::default(), Some(100)).is_empty());
        o.fill = " ".into();
        o.min_coverage = Some(2.0);
        o.kmer_length = Some(500);
        let p = o.problems(&HeaderEditor::default(), Some(100));
        assert!(p.iter().any(|m| m.contains("minimum coverage")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("fill value")), "{p:?}");
        assert!(
            p.iter().any(|m| m.contains("exceeds the alignment length")),
            "{p:?}"
        );
        // Duplicate header names are the library's error (G8).
        let mut header = HeaderEditor::default();
        header.set_names(&["Host".into(), "Host".into()]);
        header.enabled = true;
        let o = AnalysisOptions::default();
        let p = o.problems(&header, None);
        assert!(p.iter().any(|m| m.contains("more than once")), "{p:?}");
    }

    #[test]
    fn per_file_values_reset() {
        let mut o = AnalysisOptions {
            alphabet: AlphabetChoice::Protein,
            kmer_length: Some(5),
            query_name: "x".into(),
            support_threshold: 30,
            ..AnalysisOptions::default()
        };
        o.reset_for_new_file();
        assert_eq!(o.alphabet, AlphabetChoice::Auto);
        assert_eq!(o.kmer_length, None);
        assert!(o.query_name.is_empty());
        assert_eq!(o.support_threshold, 30, "remembered options stay");
        assert_eq!(o.default_kmer(Some(Alphabet::Dna)), Some(27));
        assert_eq!(o.default_kmer(None), None);
    }
}
