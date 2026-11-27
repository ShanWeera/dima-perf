//! The analysis options of the setup form and their mapping to the library's parameters.
//!
//! Defaults are the library's (D6, D36, D72); a value equal to its default is passed as
//! "not set", so the provenance records it as a default and the equivalent command (D103) shows
//! only what differs, exactly like the command line. The form never caps a value below what the
//! library accepts (D72): k is bounded by the alignment length only.

use dima_core::config::{
    Alphabet, AnalysisParams, DEFAULT_FILL, DEFAULT_SUPPORT_THRESHOLD, Estimator, ValidationMode,
};
use dima_core::validation::Inspection;

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
    /// Also compute similarity-weighted entropy and the normalised entropies (Wu 2025).
    pub similarity_weighted_entropy: bool,
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
            similarity_weighted_entropy: false,
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
            similarity_weighted_entropy: s.similarity_weighted_entropy,
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
        s.similarity_weighted_entropy = self.similarity_weighted_entropy;
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
            similarity_weighted_entropy: self.similarity_weighted_entropy,
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

    /// Problems that would make the run fail before reading, in the app's words, first the one
    /// shown under Analyze: the library's own checks (G2, G8, G52) without the command line's
    /// hints, the header fields (named in the editor, at least one Stratify box ticked), the
    /// fill value when variants are stratified (it is not used otherwise), and k, given or the
    /// alphabet's default, against the inspected alignment length (D36).
    #[must_use]
    pub fn problems(&self, header: &HeaderEditor, inspection: Option<&Inspection>) -> Vec<String> {
        let mut params = self.to_params(header);
        // The fill value is checked once, below, in the form's words.
        params.header_fill = None;
        let mut out = Vec::new();
        if let Err(e) = params.validate() {
            out.push(e.to_string());
        }
        if header.enabled {
            out.extend(header.problem());
            if self.fill.trim().is_empty() {
                out.push(FILL_EMPTY.to_owned());
            }
        }
        out.extend(inspection.and_then(|i| self.kmer_problem(i)));
        let mut seen = std::collections::HashSet::new();
        out.retain(|p| seen.insert(p.clone()));
        out
    }

    /// k against the inspected alignment, whether given or the alphabet's default (which the
    /// run would use): `None` when it fits, or while the alphabet, and so the default, is
    /// unknown.
    fn kmer_problem(&self, i: &Inspection) -> Option<String> {
        let l = i.alignment_length;
        let (k, which) = match self.kmer_length {
            Some(k) => (k, format!("k = {k}")),
            None => {
                let a = self.effective_alphabet(i.alphabet)?;
                let k = a.default_kmer_length();
                let of = match a {
                    Alphabet::Protein => "proteins",
                    Alphabet::Dna => "DNA",
                    Alphabet::Rna => "RNA",
                };
                (k, format!("the default k for {of} ({k})"))
            }
        };
        if k <= l {
            return None;
        }
        let columns = dima_core::human::plural(l as u64, "column", "columns");
        let fix = if l == 0 {
            String::new()
        } else {
            format!(": choose a k-mer length of at most {l}")
        };
        Some(format!(
            "{which} exceeds the alignment length ({columns}){fix}"
        ))
    }
}

/// The problem of an empty fill value, in the form's words.
pub const FILL_EMPTY: &str = "the Fill value under Advanced must not be empty";

/// Why the pre-check could not detect the alphabet, in the library's words (too few letters,
/// both T and U, or a composition that does not decide it; Easel's rules), without the command
/// line's hint; `None` when it detected one.
#[must_use]
pub fn undetected_alphabet(i: &Inspection) -> Option<String> {
    dima_core::alphabet::resolve(None, &i.probe.letters, i.probe.records)
        .err()
        .map(|e| e.to_string())
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
            similarity_weighted_entropy: true,
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
                "NA",
                "--similarity-weighted-entropy"
            ]
        );
    }

    /// The option is remembered (D73) and "Reset to defaults" turns it off.
    #[test]
    fn similarity_weighted_entropy_is_remembered() {
        let o = AnalysisOptions {
            similarity_weighted_entropy: true,
            ..AnalysisOptions::default()
        };
        assert!(o.differs_from_defaults());
        assert!(
            o.to_params(&HeaderEditor::default())
                .similarity_weighted_entropy
        );
        let mut s = Settings::default();
        o.store(&mut s);
        assert!(s.similarity_weighted_entropy);
        assert_eq!(AnalysisOptions::from_settings(&s), o);
        assert!(!AnalysisOptions::default().similarity_weighted_entropy);
    }

    #[test]
    fn fill_without_metadata_is_not_passed() {
        let o = AnalysisOptions {
            fill: "NA".into(),
            ..AnalysisOptions::default()
        };
        assert_eq!(o.to_params(&HeaderEditor::default()).header_fill, None);
    }

    /// The pre-check of an alignment written to a temporary file.
    fn inspected(text: &str) -> (tempfile::TempDir, Inspection) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.fasta");
        std::fs::write(&path, text).unwrap();
        let i = dima_core::validation::inspect(&dima_core::input::Source::Path(path)).unwrap();
        (dir, i)
    }

    #[test]
    fn problems_come_from_the_library() {
        let mers = dima_core::validation::inspect(&dima_core::input::Source::Path(
            crate::tests::fixtures::mers_path(),
        ))
        .unwrap();
        let mut o = AnalysisOptions::default();
        assert!(o.problems(&HeaderEditor::default(), Some(&mers)).is_empty());
        o.fill = " ".into();
        o.min_coverage = Some(2.0);
        o.kmer_length = Some(5000);
        let p = o.problems(&HeaderEditor::default(), Some(&mers));
        assert_eq!(
            p,
            [
                "minimum coverage 2 is outside [0, 1]",
                "k = 5000 exceeds the alignment length (1353 columns): choose a k-mer length of \
                 at most 1353"
            ],
            "no command-line hints; an unused fill value is no problem"
        );
        // Duplicate header names are the library's error (G8); an empty fill value is one
        // problem, in the form's words.
        let mut header = HeaderEditor::default();
        header.set_names(&["Host".into(), "Host".into()]);
        header.enabled = true;
        let p = o.problems(&header, None);
        assert!(p.iter().any(|m| m.contains("more than once")), "{p:?}");
        assert_eq!(
            p.iter()
                .filter(|m| m.to_lowercase().contains("fill value"))
                .count(),
            1,
            "{p:?}"
        );
        assert!(p.contains(&FILL_EMPTY.to_owned()), "{p:?}");
        // No Stratify box ticked.
        header.set_names(&["Accession".into(), "Host".into()]);
        for r in &mut header.rows {
            r.stratify = false;
        }
        let p = AnalysisOptions::default().problems(&header, None);
        assert_eq!(p, ["tick at least one Stratify box"]);
    }

    /// The pre-check covers the default k too, which the run would use: a 6-column protein
    /// alignment is too short for 9-mers. The message counts one column in the singular.
    #[test]
    fn a_default_k_longer_than_the_alignment_is_a_problem() {
        let (_dir, six) = inspected(">a\nMKVLAE\n>b\nMKVLAE\n>c\nMKVLAQ\n");
        assert_eq!(six.alphabet, Some(Alphabet::Protein));
        assert_eq!(six.alignment_length, 6);
        let mut o = AnalysisOptions::default();
        assert_eq!(
            o.problems(&HeaderEditor::default(), Some(&six)),
            [
                "the default k for proteins (9) exceeds the alignment length (6 columns): choose a \
              k-mer length of at most 6"
            ]
        );
        o.kmer_length = Some(6);
        assert!(o.problems(&HeaderEditor::default(), Some(&six)).is_empty());
        o.kmer_length = None;
        o.alphabet = AlphabetChoice::Rna;
        assert!(
            o.problems(&HeaderEditor::default(), Some(&six))[0]
                .starts_with("the default k for RNA (27) exceeds")
        );
        let (_dir, one) = inspected(">a\nM\n");
        assert_eq!(one.alphabet, None, "too few letters to detect");
        let mut o = AnalysisOptions::default();
        assert!(
            o.problems(&HeaderEditor::default(), Some(&one)).is_empty(),
            "no default k while the alphabet is unknown"
        );
        o.alphabet = AlphabetChoice::Protein;
        assert_eq!(
            o.problems(&HeaderEditor::default(), Some(&one)),
            [
                "the default k for proteins (9) exceeds the alignment length (1 column): choose a \
              k-mer length of at most 1"
            ]
        );
    }

    /// The real reason the alphabet was not detected, in the library's words and without the
    /// command line's hint: too few letters, both T and U, or a composition that decides
    /// nothing (the PSTVd alignments of the repository).
    #[test]
    fn the_reason_the_alphabet_was_not_detected() {
        let (_dir, few) = inspected(">a\nACD\n");
        assert_eq!(
            undetected_alphabet(&few).as_deref(),
            Some(
                "only 3 residue letters in the first record; that is too few to tell protein \
                 from nucleotide sequences"
            )
        );
        let (_dir, tu) = inspected(">a\nACGTACGUAC\n>b\nACGTACGUAA\n");
        assert_eq!(
            undetected_alphabet(&tu).as_deref(),
            Some("the sequences contain both T (2) and U (2), so DNA and RNA cannot be told apart")
        );
        let pstvd = dima_core::validation::inspect(&dima_core::input::Source::Path(
            crate::tests::fixtures::repo().join("tests/data/pstvd_loop27_ir1.fasta.bz2"),
        ))
        .unwrap();
        assert_eq!(
            undetected_alphabet(&pstvd).as_deref(),
            Some(
                "the residue composition of the first 10000 records (60000 letters) does not \
                 identify the alphabet"
            )
        );
        for reason in [few, tu, pstvd].iter().filter_map(undetected_alphabet) {
            assert!(!reason.contains("--alphabet"), "{reason}");
        }
        let mers = dima_core::validation::inspect(&dima_core::input::Source::Path(
            crate::tests::fixtures::mers_path(),
        ))
        .unwrap();
        assert_eq!(undetected_alphabet(&mers), None);
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
