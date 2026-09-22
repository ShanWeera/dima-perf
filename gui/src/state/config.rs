//! User-facing analysis configuration.
//!
//! These types are *state*, not UI: the setup form edits them and the analysis
//! worker consumes them. Keeping them here (rather than inside the UI module)
//! lets `workers::analysis` run without depending on any rendering code.

/// User's alphabet selection in the setup form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AlphabetChoice {
    Protein,
    Nucleotide,
    #[default]
    Auto,
}

impl AlphabetChoice {
    /// Label shown in the alphabet selector.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Auto => "Auto-detect",
            Self::Protein => "Protein",
            Self::Nucleotide => "Nucleotide",
        }
    }

    /// Value passed to `dima_lib::analyze`; `None` means "auto-detect".
    pub fn lib_name(&self) -> Option<String> {
        match self {
            Self::Protein => Some("protein".to_string()),
            Self::Nucleotide => Some("nucleotide".to_string()),
            Self::Auto => None,
        }
    }

    /// Largest k-mer length that cannot overflow the encoder for this alphabet.
    ///
    /// Exceeding it makes `encode_kmer_validated` return `None` and silently drop
    /// k-mers — wrong results rather than a crash — so the UI must clamp to it.
    /// `Auto` uses the protein bound, the conservative (smaller) of the two.
    pub fn max_kmer_length(&self) -> usize {
        match self {
            Self::Protein | Self::Auto => dima_lib::max_kmer_length(true),
            Self::Nucleotide => dima_lib::max_kmer_length(false),
        }
    }
}

/// Analysis parameters gathered by the setup form.
#[derive(Debug, Clone)]
pub struct AnalysisConfigUi {
    pub alphabet: AlphabetChoice,
    pub kmer_length: usize,
    /// IMPORTANT: a COUNT of sequences (1..=10000), NOT a percentage.
    pub support_threshold: usize,
    pub query_name: String,
    /// Pre-split header field names (e.g. `["id", "country", "host"]`).
    ///
    /// Stored already split rather than as a formatted string so the original
    /// delimiter (which may be a tab) never has to be recovered later.
    pub header_format: Option<Vec<String>>,
    pub header_fillna: Option<String>,
    pub metadata_fields: Vec<String>,
    pub validation_mode: dima_lib::ValidationMode,
    pub allow_lowercase: bool,
    /// Defaults to true so `ValidationStats` is available for the summary.
    pub report_invalid: bool,
}

impl Default for AnalysisConfigUi {
    fn default() -> Self {
        Self {
            alphabet: AlphabetChoice::Auto,
            kmer_length: 9,
            support_threshold: 30,
            query_name: String::new(),
            header_format: None,
            header_fillna: None,
            metadata_fields: Vec::new(),
            validation_mode: dima_lib::ValidationMode::default(),
            allow_lowercase: false,
            report_invalid: true,
        }
    }
}

impl AnalysisConfigUi {
    /// Clear every setting derived from the previously selected file.
    ///
    /// Header format, fill-NA and metadata field selection are all detected from
    /// a specific file's headers; carrying them into a different file would
    /// silently apply the wrong metadata schema.
    pub fn clear_file_derived(&mut self) {
        self.header_format = None;
        self.header_fillna = None;
        self.metadata_fields.clear();
    }

    /// Metadata fields to aggregate, or `None` for "all fields" (CLI semantics).
    pub fn metadata_fields_arg(&self) -> Option<Vec<String>> {
        if self.metadata_fields.is_empty() {
            None
        } else {
            Some(self.metadata_fields.clone())
        }
    }
}

/// Post-analysis settings (not inputs to `analyze`).
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub hcs_threshold: f64,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            hcs_threshold: 95.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_uses_conservative_protein_kmer_bound() {
        assert_eq!(
            AlphabetChoice::Auto.max_kmer_length(),
            AlphabetChoice::Protein.max_kmer_length()
        );
        assert!(
            AlphabetChoice::Protein.max_kmer_length()
                < AlphabetChoice::Nucleotide.max_kmer_length()
        );
    }

    #[test]
    fn lib_name_maps_to_library_vocabulary() {
        assert_eq!(
            AlphabetChoice::Protein.lib_name().as_deref(),
            Some("protein")
        );
        assert_eq!(
            AlphabetChoice::Nucleotide.lib_name().as_deref(),
            Some("nucleotide")
        );
        assert_eq!(AlphabetChoice::Auto.lib_name(), None);
    }

    #[test]
    fn empty_metadata_selection_means_all_fields() {
        let mut cfg = AnalysisConfigUi::default();
        assert_eq!(cfg.metadata_fields_arg(), None);
        cfg.metadata_fields.push("host".to_string());
        assert_eq!(cfg.metadata_fields_arg(), Some(vec!["host".to_string()]));
    }

    #[test]
    fn clear_file_derived_resets_all_header_state() {
        let mut cfg = AnalysisConfigUi {
            header_format: Some(vec!["a".into()]),
            header_fillna: Some("Unknown".into()),
            metadata_fields: vec!["a".into()],
            ..Default::default()
        };
        cfg.clear_file_derived();
        assert!(cfg.header_format.is_none());
        assert!(cfg.header_fillna.is_none());
        assert!(cfg.metadata_fields.is_empty());
    }
}
