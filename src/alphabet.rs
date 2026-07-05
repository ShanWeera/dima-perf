//! Alphabet Module - Character Validation for K-mer Generation
//!
//! This module provides robust character validation using a whitelist approach
//! to ensure only valid biological sequences are processed. It distinguishes
//! between valid characters, known ambiguous characters, and completely invalid
//! characters for proper reporting and handling.
//!
//! Performance characteristics:
//! - O(1) per character validation via 256-byte lookup table
//! - Compatible with SIMD optimizations
//! - Thread-safe and zero-allocation validation
//! - Supports both protein and nucleotide alphabets

use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Valid standard amino acids (20 canonical amino acids)
pub const VALID_PROTEIN_CHARS: &[u8] = b"ACDEFGHIKLMNPQRSTVWY";

/// Valid nucleotides (DNA + RNA)
pub const VALID_NUCLEOTIDE_CHARS: &[u8] = b"ACGTU";

/// Known ambiguous protein characters (IUPAC codes for amino acid ambiguity)
/// - X: Any amino acid
/// - B: Aspartic acid or Asparagine (D or N)
/// - J: Leucine or Isoleucine (L or I)
/// - Z: Glutamic acid or Glutamine (E or Q)
/// - O: Pyrrolysine (rare, non-standard)
/// - U: Selenocysteine (rare, non-standard)
pub const AMBIGUOUS_PROTEIN_CHARS: &[u8] = b"XBJZOU";

/// Known ambiguous nucleotide characters (IUPAC codes)
/// - R: Purine (A or G)
/// - Y: Pyrimidine (C or T)
/// - K: Keto (G or T)
/// - M: Amino (A or C)
/// - S: Strong (G or C)
/// - W: Weak (A or T)
/// - B: Not A (C, G, or T)
/// - D: Not C (A, G, or T)
/// - H: Not G (A, C, or T)
/// - V: Not T (A, C, or G)
/// - N: Any nucleotide
pub const AMBIGUOUS_NUCLEOTIDE_CHARS: &[u8] = b"RYKMSWBDHVN";

/// Gap characters commonly found in alignments.
/// Standard MSA tools (MAFFT, Clustal, MUSCLE) use both '-' and '.' for gaps.
pub const GAP_CHAR: u8 = b'-';
pub const GAP_CHAR_DOT: u8 = b'.';

/// Special marker values in the lookup table
pub const MARKER_AMBIGUOUS: u8 = 254;
pub const MARKER_INVALID: u8 = 255;

/// Classification of a character in the biological alphabet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterClass {
    /// Valid character with its encoding index (0-19 for protein, 0-4 for nucleotide)
    Valid(u8),
    /// Known ambiguous character (X, B, N, etc.)
    Ambiguous,
    /// Gap character (-)
    Gap,
    /// Completely invalid/unknown character (#, *, @, numbers, etc.)
    Invalid,
}

/// Type of biological alphabet being used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphabetType {
    Protein,
    Nucleotide,
}

impl fmt::Display for AlphabetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlphabetType::Protein => write!(f, "protein"),
            AlphabetType::Nucleotide => write!(f, "nucleotide"),
        }
    }
}

impl AlphabetType {
    /// Resolve an alphabet type from a string, accepting common synonyms.
    /// Returns Protein for None (default). Logs a warning for unrecognized strings.
    pub fn from_optional_str(s: Option<&str>) -> Self {
        match s.map(|v| v.to_lowercase()) {
            Some(ref v) if matches!(v.as_str(), "nucleotide" | "dna" | "rna") => {
                AlphabetType::Nucleotide
            }
            Some(ref v) if matches!(v.as_str(), "protein" | "amino_acid" | "aa") => {
                AlphabetType::Protein
            }
            None => AlphabetType::Protein,
            Some(ref unknown) => {
                tracing::warn!(alphabet = %unknown, "unrecognized alphabet, defaulting to protein");
                AlphabetType::Protein
            }
        }
    }
}

/// Validation mode for character checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidationMode {
    /// Only accept valid alphabet characters (whitelist approach)
    /// This is the recommended mode for scientific accuracy
    #[default]
    Strict,
    /// Accept valid + ambiguous characters, reject only completely invalid
    /// Ambiguous characters still result in NA k-mers but won't trigger warnings
    Permissive,
    /// Accept all characters, but report invalid ones
    /// Useful for debugging or data quality assessment
    ReportOnly,
}

impl std::str::FromStr for ValidationMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "strict" => Ok(ValidationMode::Strict),
            "permissive" => Ok(ValidationMode::Permissive),
            "report" | "report-only" | "reportonly" => Ok(ValidationMode::ReportOnly),
            _ => Err(format!(
                "Invalid validation mode: '{}'. Valid options: strict, permissive, report",
                s
            )),
        }
    }
}

/// Statistics about invalid characters encountered during validation
#[derive(Debug, Default)]
pub struct ValidationStats {
    /// Total characters processed
    pub total_chars: AtomicUsize,
    /// Number of valid characters
    pub valid_chars: AtomicUsize,
    /// Number of ambiguous characters
    pub ambiguous_chars: AtomicUsize,
    /// Number of gap characters
    pub gap_chars: AtomicUsize,
    /// Number of invalid characters
    pub invalid_chars: AtomicUsize,
    /// Number of k-mers invalidated
    pub invalidated_kmers: AtomicUsize,
}

impl ValidationStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a character classification
    pub fn record(&self, class: CharacterClass) {
        self.total_chars.fetch_add(1, Ordering::Relaxed);
        match class {
            CharacterClass::Valid(_) => {
                self.valid_chars.fetch_add(1, Ordering::Relaxed);
            }
            CharacterClass::Ambiguous => {
                self.ambiguous_chars.fetch_add(1, Ordering::Relaxed);
            }
            CharacterClass::Gap => {
                self.gap_chars.fetch_add(1, Ordering::Relaxed);
            }
            CharacterClass::Invalid => {
                self.invalid_chars.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record an invalidated k-mer
    pub fn record_invalidated_kmer(&self) {
        self.invalidated_kmers.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a summary of the validation statistics
    pub fn summary(&self) -> ValidationStatsSummary {
        ValidationStatsSummary {
            total_chars: self.total_chars.load(Ordering::Relaxed),
            valid_chars: self.valid_chars.load(Ordering::Relaxed),
            ambiguous_chars: self.ambiguous_chars.load(Ordering::Relaxed),
            gap_chars: self.gap_chars.load(Ordering::Relaxed),
            invalid_chars: self.invalid_chars.load(Ordering::Relaxed),
            invalidated_kmers: self.invalidated_kmers.load(Ordering::Relaxed),
        }
    }
}

/// Non-atomic summary of validation statistics
#[derive(Debug, Clone)]
pub struct ValidationStatsSummary {
    pub total_chars: usize,
    pub valid_chars: usize,
    pub ambiguous_chars: usize,
    pub gap_chars: usize,
    pub invalid_chars: usize,
    pub invalidated_kmers: usize,
}

impl fmt::Display for ValidationStatsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Character Validation Summary:")?;
        writeln!(f, "  Total characters:     {}", self.total_chars)?;
        writeln!(
            f,
            "  Valid characters:     {} ({:.1}%)",
            self.valid_chars,
            if self.total_chars > 0 {
                self.valid_chars as f64 / self.total_chars as f64 * 100.0
            } else {
                0.0
            }
        )?;
        writeln!(
            f,
            "  Ambiguous characters: {} ({:.1}%)",
            self.ambiguous_chars,
            if self.total_chars > 0 {
                self.ambiguous_chars as f64 / self.total_chars as f64 * 100.0
            } else {
                0.0
            }
        )?;
        writeln!(
            f,
            "  Gap characters:       {} ({:.1}%)",
            self.gap_chars,
            if self.total_chars > 0 {
                self.gap_chars as f64 / self.total_chars as f64 * 100.0
            } else {
                0.0
            }
        )?;
        writeln!(
            f,
            "  Invalid characters:   {} ({:.1}%)",
            self.invalid_chars,
            if self.total_chars > 0 {
                self.invalid_chars as f64 / self.total_chars as f64 * 100.0
            } else {
                0.0
            }
        )?;
        write!(f, "  Invalidated k-mers:   {}", self.invalidated_kmers)
    }
}

/// Character validator using a 256-byte lookup table for O(1) validation
///
/// The lookup table maps each possible byte value to:
/// - 0-19: Valid protein amino acid encoding index
/// - 0-4: Valid nucleotide encoding index
/// - 254 (MARKER_AMBIGUOUS): Known ambiguous character
/// - 255 (MARKER_INVALID): Invalid/unknown character
#[derive(Clone)]
pub struct CharacterValidator {
    /// Lookup table for character classification
    lookup: [u8; 256],
    /// Type of alphabet being validated
    alphabet_type: AlphabetType,
    /// Validation mode
    mode: ValidationMode,
    /// Whether to allow lowercase characters (auto-converted to uppercase)
    allow_lowercase: bool,
}

impl CharacterValidator {
    /// Create a new validator for the specified alphabet type
    pub fn new(alphabet_type: AlphabetType) -> Self {
        Self::with_options(alphabet_type, ValidationMode::Strict, false)
    }

    /// Create a new validator with custom options
    pub fn with_options(
        alphabet_type: AlphabetType,
        mode: ValidationMode,
        allow_lowercase: bool,
    ) -> Self {
        let mut lookup = [MARKER_INVALID; 256];

        match alphabet_type {
            AlphabetType::Protein => {
                // Map valid amino acids to their encoding indices
                for (idx, &ch) in VALID_PROTEIN_CHARS.iter().enumerate() {
                    lookup[ch as usize] = idx as u8;
                    if allow_lowercase {
                        lookup[ch.to_ascii_lowercase() as usize] = idx as u8;
                    }
                }
                // Map ambiguous characters
                for &ch in AMBIGUOUS_PROTEIN_CHARS {
                    lookup[ch as usize] = MARKER_AMBIGUOUS;
                    if allow_lowercase {
                        lookup[ch.to_ascii_lowercase() as usize] = MARKER_AMBIGUOUS;
                    }
                }
            }
            AlphabetType::Nucleotide => {
                // Map valid nucleotides to their encoding indices
                for (idx, &ch) in VALID_NUCLEOTIDE_CHARS.iter().enumerate() {
                    lookup[ch as usize] = idx as u8;
                    if allow_lowercase {
                        lookup[ch.to_ascii_lowercase() as usize] = idx as u8;
                    }
                }
                // Map ambiguous characters
                for &ch in AMBIGUOUS_NUCLEOTIDE_CHARS {
                    lookup[ch as usize] = MARKER_AMBIGUOUS;
                    if allow_lowercase {
                        lookup[ch.to_ascii_lowercase() as usize] = MARKER_AMBIGUOUS;
                    }
                }
            }
        }

        // Gap characters are always marked specially (treated as ambiguous for k-mer purposes)
        // Both '-' and '.' are standard gap characters in MSA formats
        lookup[GAP_CHAR as usize] = MARKER_AMBIGUOUS;
        lookup[GAP_CHAR_DOT as usize] = MARKER_AMBIGUOUS;

        Self {
            lookup,
            alphabet_type,
            mode,
            allow_lowercase,
        }
    }

    /// Create a protein validator with default settings
    pub fn protein() -> Self {
        Self::new(AlphabetType::Protein)
    }

    /// Create a nucleotide validator with default settings
    pub fn nucleotide() -> Self {
        Self::new(AlphabetType::Nucleotide)
    }

    /// Create a validator from an alphabet string.
    ///
    /// Accepts common synonyms (case-insensitive):
    ///   - Protein: "protein", "amino_acid", "aa"
    ///   - Nucleotide: "nucleotide", "dna", "rna"
    ///
    /// Logs a warning for unrecognized values rather than silently defaulting.
    pub fn from_alphabet_string(alphabet: Option<&String>) -> Self {
        match alphabet.map(|s| s.to_lowercase()) {
            Some(ref s) if matches!(s.as_str(), "nucleotide" | "dna" | "rna") => Self::nucleotide(),
            Some(ref s) if matches!(s.as_str(), "protein" | "amino_acid" | "aa") => Self::protein(),
            None => Self::protein(),
            Some(ref unknown) => {
                tracing::warn!(
                    alphabet = %unknown,
                    "unrecognized alphabet, defaulting to protein (valid: protein, amino_acid, aa, nucleotide, dna, rna)"
                );
                Self::protein()
            }
        }
    }

    /// Create a validator with full configuration.
    /// Case-insensitive alphabet matching.
    pub fn from_config(
        alphabet: Option<&String>,
        mode: ValidationMode,
        allow_lowercase: bool,
    ) -> Self {
        let alphabet_type = match alphabet.map(|s| s.to_lowercase()) {
            Some(ref s) if s == "nucleotide" => AlphabetType::Nucleotide,
            _ => AlphabetType::Protein,
        };
        Self::with_options(alphabet_type, mode, allow_lowercase)
    }

    /// Get the alphabet type
    pub fn alphabet_type(&self) -> AlphabetType {
        self.alphabet_type
    }

    /// Get the validation mode
    pub fn mode(&self) -> ValidationMode {
        self.mode
    }

    /// Check if lowercase is allowed
    pub fn allows_lowercase(&self) -> bool {
        self.allow_lowercase
    }

    /// Classify a single character
    #[inline(always)]
    pub fn classify(&self, ch: u8) -> CharacterClass {
        let code = self.lookup[ch as usize];
        match code {
            MARKER_INVALID => CharacterClass::Invalid,
            MARKER_AMBIGUOUS => {
                if ch == GAP_CHAR || ch == GAP_CHAR_DOT {
                    CharacterClass::Gap
                } else {
                    CharacterClass::Ambiguous
                }
            }
            valid_code => CharacterClass::Valid(valid_code),
        }
    }

    /// Check if a character is valid (can be encoded)
    #[inline(always)]
    pub fn is_valid(&self, ch: u8) -> bool {
        let code = self.lookup[ch as usize];
        code != MARKER_INVALID && code != MARKER_AMBIGUOUS
    }

    /// Check if a character should cause the k-mer to be marked as NA.
    ///
    /// Per PMC11596295: "Support is the number of sequences that do not harbor
    /// a gap and/or unknown/ambiguous." This is a scientific requirement that
    /// applies regardless of validation mode. The mode controls *reporting*
    /// behavior (abort vs warn vs log), not encoding correctness.
    #[inline(always)]
    pub fn should_invalidate_kmer(&self, ch: u8) -> bool {
        let code = self.lookup[ch as usize];
        code == MARKER_INVALID || code == MARKER_AMBIGUOUS
    }

    /// Check if any character in a window should invalidate the k-mer
    #[inline(always)]
    pub fn window_has_invalid(&self, window: &[u8]) -> bool {
        window.iter().any(|&ch| self.should_invalidate_kmer(ch))
    }

    /// Get the encoding index for a valid character.
    ///
    /// Returns `Some(index)` only for standard alphabet characters (0-19 for
    /// protein, 0-4 for nucleotide). Returns `None` for ambiguous, gap, and
    /// invalid characters — these must be excluded from k-mer encoding per
    /// PMC11596295's definition of support.
    ///
    /// This function is mode-independent: scientific correctness requires
    /// excluding non-standard characters regardless of the validation mode.
    #[inline(always)]
    pub fn encode(&self, ch: u8) -> Option<u8> {
        let code = self.lookup[ch as usize];
        if code != MARKER_INVALID && code != MARKER_AMBIGUOUS {
            Some(code)
        } else {
            None
        }
    }

    /// Get the raw lookup value for a character
    #[inline(always)]
    pub fn lookup_raw(&self, ch: u8) -> u8 {
        self.lookup[ch as usize]
    }

    /// Get the valid characters for this alphabet
    pub fn valid_chars(&self) -> &'static [u8] {
        match self.alphabet_type {
            AlphabetType::Protein => VALID_PROTEIN_CHARS,
            AlphabetType::Nucleotide => VALID_NUCLEOTIDE_CHARS,
        }
    }

    /// Get the ambiguous characters for this alphabet
    pub fn ambiguous_chars(&self) -> &'static [u8] {
        match self.alphabet_type {
            AlphabetType::Protein => AMBIGUOUS_PROTEIN_CHARS,
            AlphabetType::Nucleotide => AMBIGUOUS_NUCLEOTIDE_CHARS,
        }
    }

    /// Check if this is a protein validator
    pub fn is_protein(&self) -> bool {
        matches!(self.alphabet_type, AlphabetType::Protein)
    }

    /// Check if this is a nucleotide validator
    pub fn is_nucleotide(&self) -> bool {
        matches!(self.alphabet_type, AlphabetType::Nucleotide)
    }

    /// Convert a character to uppercase if lowercase is allowed
    /// Returns the same character if already uppercase or lowercase not allowed
    #[inline(always)]
    pub fn normalize_case(&self, ch: u8) -> u8 {
        if self.allow_lowercase && ch.is_ascii_lowercase() {
            ch.to_ascii_uppercase()
        } else {
            ch
        }
    }

    /// Find all invalid characters in a sequence
    pub fn find_invalid_characters(&self, sequence: &[u8]) -> Vec<(usize, u8, CharacterClass)> {
        sequence
            .iter()
            .enumerate()
            .filter_map(|(pos, &ch)| {
                let class = self.classify(ch);
                match class {
                    CharacterClass::Invalid => Some((pos, ch, class)),
                    CharacterClass::Ambiguous if self.mode == ValidationMode::Strict => {
                        Some((pos, ch, class))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    /// Get a set of unique invalid characters in a sequence
    pub fn unique_invalid_characters(&self, sequence: &[u8]) -> HashSet<u8> {
        sequence
            .iter()
            .filter(|&&ch| {
                let class = self.classify(ch);
                matches!(class, CharacterClass::Invalid)
            })
            .copied()
            .collect()
    }
}

impl fmt::Debug for CharacterValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CharacterValidator")
            .field("alphabet_type", &self.alphabet_type)
            .field("mode", &self.mode)
            .field("allow_lowercase", &self.allow_lowercase)
            .finish()
    }
}

/// Builder for creating custom CharacterValidator instances
pub struct CharacterValidatorBuilder {
    alphabet_type: AlphabetType,
    mode: ValidationMode,
    allow_lowercase: bool,
    custom_valid: Option<Vec<u8>>,
    custom_ambiguous: Option<Vec<u8>>,
}

impl CharacterValidatorBuilder {
    pub fn new(alphabet_type: AlphabetType) -> Self {
        Self {
            alphabet_type,
            mode: ValidationMode::Strict,
            allow_lowercase: false,
            custom_valid: None,
            custom_ambiguous: None,
        }
    }

    pub fn protein() -> Self {
        Self::new(AlphabetType::Protein)
    }

    pub fn nucleotide() -> Self {
        Self::new(AlphabetType::Nucleotide)
    }

    pub fn mode(mut self, mode: ValidationMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn allow_lowercase(mut self, allow: bool) -> Self {
        self.allow_lowercase = allow;
        self
    }

    pub fn custom_valid_chars(mut self, chars: Vec<u8>) -> Self {
        self.custom_valid = Some(chars);
        self
    }

    pub fn custom_ambiguous_chars(mut self, chars: Vec<u8>) -> Self {
        self.custom_ambiguous = Some(chars);
        self
    }

    /// Build the CharacterValidator from the configured options.
    ///
    /// Returns `Err` if the custom alphabet exceeds the maximum supported size
    /// (253 valid characters), which would collide with internal sentinel values.
    pub fn build(self) -> Result<CharacterValidator, String> {
        if self.custom_valid.is_some() || self.custom_ambiguous.is_some() {
            let mut lookup = [MARKER_INVALID; 256];

            let valid_chars = self
                .custom_valid
                .as_deref()
                .unwrap_or(match self.alphabet_type {
                    AlphabetType::Protein => VALID_PROTEIN_CHARS,
                    AlphabetType::Nucleotide => VALID_NUCLEOTIDE_CHARS,
                });

            let valid_count = valid_chars.len();
            if valid_count >= MARKER_AMBIGUOUS as usize {
                return Err(format!(
                    "Custom alphabet has {} valid characters which would collide with \
                     marker sentinels (maximum supported: {})",
                    valid_count,
                    MARKER_AMBIGUOUS as usize - 1
                ));
            }

            for (idx, &ch) in valid_chars.iter().enumerate() {
                lookup[ch as usize] = idx as u8;
                if self.allow_lowercase {
                    lookup[ch.to_ascii_lowercase() as usize] = idx as u8;
                }
            }

            let ambiguous_chars =
                self.custom_ambiguous
                    .as_deref()
                    .unwrap_or(match self.alphabet_type {
                        AlphabetType::Protein => AMBIGUOUS_PROTEIN_CHARS,
                        AlphabetType::Nucleotide => AMBIGUOUS_NUCLEOTIDE_CHARS,
                    });

            for &ch in ambiguous_chars {
                lookup[ch as usize] = MARKER_AMBIGUOUS;
                if self.allow_lowercase {
                    lookup[ch.to_ascii_lowercase() as usize] = MARKER_AMBIGUOUS;
                }
            }

            lookup[GAP_CHAR as usize] = MARKER_AMBIGUOUS;
            lookup[GAP_CHAR_DOT as usize] = MARKER_AMBIGUOUS;

            Ok(CharacterValidator {
                lookup,
                alphabet_type: self.alphabet_type,
                mode: self.mode,
                allow_lowercase: self.allow_lowercase,
            })
        } else {
            Ok(CharacterValidator::with_options(
                self.alphabet_type,
                self.mode,
                self.allow_lowercase,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protein_validator_valid_chars() {
        let validator = CharacterValidator::protein();

        // All 20 standard amino acids should be valid
        for &ch in VALID_PROTEIN_CHARS {
            assert!(
                validator.is_valid(ch),
                "Character {} should be valid",
                ch as char
            );
            assert!(matches!(validator.classify(ch), CharacterClass::Valid(_)));
        }
    }

    #[test]
    fn test_protein_validator_ambiguous_chars() {
        let validator = CharacterValidator::protein();

        // Known ambiguous characters
        for &ch in AMBIGUOUS_PROTEIN_CHARS {
            assert!(
                !validator.is_valid(ch),
                "Character {} should not be valid",
                ch as char
            );
            assert!(matches!(validator.classify(ch), CharacterClass::Ambiguous));
        }
    }

    #[test]
    fn test_protein_validator_invalid_chars() {
        let validator = CharacterValidator::protein();

        // Completely invalid characters
        let invalid_chars = b"#*@!123456789()[]{}<>?/\\|`~";
        for &ch in invalid_chars {
            assert!(
                !validator.is_valid(ch),
                "Character {} should not be valid",
                ch as char
            );
            assert!(matches!(validator.classify(ch), CharacterClass::Invalid));
        }
    }

    #[test]
    fn test_nucleotide_validator_valid_chars() {
        let validator = CharacterValidator::nucleotide();

        for &ch in VALID_NUCLEOTIDE_CHARS {
            assert!(
                validator.is_valid(ch),
                "Character {} should be valid",
                ch as char
            );
        }
    }

    #[test]
    fn test_nucleotide_validator_ambiguous_chars() {
        let validator = CharacterValidator::nucleotide();

        for &ch in AMBIGUOUS_NUCLEOTIDE_CHARS {
            assert!(!validator.is_valid(ch));
            assert!(matches!(validator.classify(ch), CharacterClass::Ambiguous));
        }
    }

    #[test]
    fn test_gap_character() {
        let protein_validator = CharacterValidator::protein();
        let nucleotide_validator = CharacterValidator::nucleotide();

        assert!(matches!(
            protein_validator.classify(b'-'),
            CharacterClass::Gap
        ));
        assert!(matches!(
            nucleotide_validator.classify(b'-'),
            CharacterClass::Gap
        ));
    }

    #[test]
    fn test_lowercase_not_allowed_by_default() {
        let validator = CharacterValidator::protein();

        // Lowercase should be invalid by default
        assert!(matches!(validator.classify(b'a'), CharacterClass::Invalid));
        assert!(matches!(validator.classify(b'c'), CharacterClass::Invalid));
    }

    #[test]
    fn test_lowercase_allowed_when_enabled() {
        let validator =
            CharacterValidator::with_options(AlphabetType::Protein, ValidationMode::Strict, true);

        // Lowercase should be valid when enabled
        assert!(validator.is_valid(b'a'));
        assert!(validator.is_valid(b'c'));

        // Should encode to same value as uppercase
        assert_eq!(validator.encode(b'a'), validator.encode(b'A'));
    }

    #[test]
    fn test_invalidation_is_mode_independent() {
        // Per PMC11596295: "Support is the number of sequences that do not harbor
        // a gap and/or unknown/ambiguous." This applies regardless of mode.
        // All modes must invalidate k-mers containing non-standard chars.
        let strict =
            CharacterValidator::with_options(AlphabetType::Protein, ValidationMode::Strict, false);
        assert!(strict.should_invalidate_kmer(b'X')); // ambiguous
        assert!(strict.should_invalidate_kmer(b'#')); // invalid
        assert!(!strict.should_invalidate_kmer(b'A')); // valid

        let permissive = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::Permissive,
            false,
        );
        assert!(permissive.should_invalidate_kmer(b'X')); // ambiguous — MUST invalidate
        assert!(permissive.should_invalidate_kmer(b'#')); // invalid
        assert!(!permissive.should_invalidate_kmer(b'A')); // valid

        let report = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::ReportOnly,
            false,
        );
        assert!(report.should_invalidate_kmer(b'X')); // ambiguous — MUST invalidate
        assert!(report.should_invalidate_kmer(b'#')); // invalid
        assert!(!report.should_invalidate_kmer(b'A')); // valid
    }

    #[test]
    fn test_window_validation() {
        let validator = CharacterValidator::protein();

        // Valid window
        assert!(!validator.window_has_invalid(b"ACDEF"));

        // Window with ambiguous character
        assert!(validator.window_has_invalid(b"ACDXF"));

        // Window with invalid character
        assert!(validator.window_has_invalid(b"ACD#F"));

        // Window with gap
        assert!(validator.window_has_invalid(b"ACD-F"));
    }

    #[test]
    fn test_find_invalid_characters() {
        let validator = CharacterValidator::protein();

        let sequence = b"ACD#FG*HI";
        let invalid = validator.find_invalid_characters(sequence);

        assert_eq!(invalid.len(), 2);
        assert_eq!(invalid[0], (3, b'#', CharacterClass::Invalid));
        assert_eq!(invalid[1], (6, b'*', CharacterClass::Invalid));
    }

    #[test]
    fn test_unique_invalid_characters() {
        let validator = CharacterValidator::protein();

        let sequence = b"ACD#FG#HI*#";
        let unique = validator.unique_invalid_characters(sequence);

        assert_eq!(unique.len(), 2);
        assert!(unique.contains(&b'#'));
        assert!(unique.contains(&b'*'));
    }

    #[test]
    fn test_builder() {
        let validator = CharacterValidatorBuilder::protein()
            .mode(ValidationMode::Permissive)
            .allow_lowercase(true)
            .build()
            .expect("standard alphabet should always succeed");

        assert_eq!(validator.mode(), ValidationMode::Permissive);
        assert!(validator.allows_lowercase());
        assert!(validator.is_protein());
    }

    #[test]
    fn test_custom_alphabet() {
        let validator = CharacterValidatorBuilder::protein()
            .custom_valid_chars(b"ABC".to_vec())
            .custom_ambiguous_chars(b"X".to_vec())
            .build()
            .expect("small custom alphabet should succeed");

        assert!(validator.is_valid(b'A'));
        assert!(validator.is_valid(b'B'));
        assert!(validator.is_valid(b'C'));
        assert!(!validator.is_valid(b'D'));
        assert!(matches!(
            validator.classify(b'X'),
            CharacterClass::Ambiguous
        ));
    }

    #[test]
    fn test_builder_rejects_oversized_alphabet() {
        // 254+ valid chars would collide with MARKER_AMBIGUOUS sentinel
        let huge_alphabet: Vec<u8> = (0u8..=253).collect();
        let result = CharacterValidatorBuilder::protein()
            .custom_valid_chars(huge_alphabet)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_stats() {
        let stats = ValidationStats::new();

        stats.record(CharacterClass::Valid(0));
        stats.record(CharacterClass::Valid(1));
        stats.record(CharacterClass::Ambiguous);
        stats.record(CharacterClass::Invalid);
        stats.record_invalidated_kmer();

        let summary = stats.summary();
        assert_eq!(summary.total_chars, 4);
        assert_eq!(summary.valid_chars, 2);
        assert_eq!(summary.ambiguous_chars, 1);
        assert_eq!(summary.invalid_chars, 1);
        assert_eq!(summary.invalidated_kmers, 1);
    }

    #[test]
    fn test_from_alphabet_string() {
        let protein = CharacterValidator::from_alphabet_string(Some(&"protein".to_string()));
        assert!(protein.is_protein());

        let nucleotide = CharacterValidator::from_alphabet_string(Some(&"nucleotide".to_string()));
        assert!(nucleotide.is_nucleotide());

        // Default to protein
        let default = CharacterValidator::from_alphabet_string(None);
        assert!(default.is_protein());
    }
}
