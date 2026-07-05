use hashbrown::HashMap;

use crate::alphabet::{
    CharacterValidator, ValidationStats, VALID_NUCLEOTIDE_CHARS, VALID_PROTEIN_CHARS,
};

// Re-export for tests
#[cfg(test)]
use crate::alphabet::{AlphabetType, ValidationMode};

/// Encode a k-mer using the CharacterValidator for validation.
///
/// Returns `Some(encoded_value)` only if ALL characters in the k-mer are
/// standard valid alphabet characters (no gaps, no ambiguous, no invalid).
/// Per PMC11596295: support counts only sequences without gaps/ambiguous chars,
/// so k-mers containing such characters are excluded from diversity analysis.
///
/// Returns `None` if:
/// - Any character cannot be encoded (gap, ambiguous, or invalid)
/// - Integer overflow would occur (k-mer too long for the alphabet's base)
#[inline(always)]
pub fn encode_kmer_validated(kmer: &[u8], validator: &CharacterValidator) -> Option<u64> {
    let base = encoding_base(validator);

    let mut encoded = 0u64;
    for &byte in kmer {
        match validator.encode(byte) {
            Some(code) => {
                encoded = encoded.checked_mul(base)?.checked_add(code as u64)?;
            }
            None => {
                return None;
            }
        }
    }
    Some(encoded)
}

/// Get the encoding base for the given validator.
///
/// Always uses the standard alphabet size (20 for protein, 5 for nucleotide)
/// since only standard characters are ever encoded. The validation mode does
/// not affect encoding — it only controls reporting behavior.
#[inline(always)]
fn encoding_base(validator: &CharacterValidator) -> u64 {
    if validator.is_protein() {
        20u64
    } else {
        5u64
    }
}

/// Maximum k-mer length that can be encoded in a u64 for the given alphabet.
///
/// Beyond this length, `encode_kmer_validated` will overflow and return `None`
/// for ALL k-mers, resulting in support=0 at every position (silent failure).
/// Callers should validate k-mer length before starting analysis.
///
/// Values: protein = 14 (20^14 < 2^64 < 20^15), nucleotide = 27 (5^27 < 2^64 < 5^28)
pub fn max_kmer_length(is_protein: bool) -> usize {
    if is_protein {
        14
    } else {
        27
    }
}

/// Encode a k-mer for the specified alphabet type
///
/// Creates a temporary validator internally. For better performance when
/// processing many k-mers, use `encode_kmer_validated` with a pre-created validator.
#[inline(always)]
pub fn encode_kmer(kmer: &[u8], is_protein: bool) -> Option<u64> {
    let validator = if is_protein {
        CharacterValidator::protein()
    } else {
        CharacterValidator::nucleotide()
    };
    encode_kmer_validated(kmer, &validator)
}

/// Decode an encoded k-mer back to its string representation.
///
/// Uses base-20 for protein and base-5 for nucleotide, matching the encoding
/// in `encode_kmer_validated`. Only standard alphabet characters are produced.
pub fn decode_kmer(encoded: u64, kmer_length: usize, is_protein: bool) -> String {
    // Build the string directly in correct order by filling from the end,
    // avoiding an O(k) reverse pass on the result buffer.
    let mut result = vec![0u8; kmer_length];
    let mut remaining = encoded;

    let (base, chars): (u64, &[u8]) = if is_protein {
        (20, VALID_PROTEIN_CHARS)
    } else {
        (5, VALID_NUCLEOTIDE_CHARS)
    };

    for i in (0..kmer_length).rev() {
        let char_idx = (remaining % base) as usize;
        result[i] = if char_idx < chars.len() {
            chars[char_idx]
        } else {
            b'?'
        };
        remaining /= base;
    }

    // Safety: VALID_PROTEIN_CHARS and VALID_NUCLEOTIDE_CHARS are ASCII, and b'?' is ASCII.
    unsafe { String::from_utf8_unchecked(result) }
}

/// Generate k-mers from a sequence using whitelist-based character validation.
///
/// This function uses the robust whitelist-based character validation from
/// the alphabet module to ensure only valid biological sequences are processed.
///
/// # Arguments
/// * `sequence` - The sequence bytes to process
/// * `kmer_length` - Length of k-mers to generate (must be > 0)
/// * `validator` - CharacterValidator configured for the appropriate alphabet
///
/// # Returns
/// Vector of Option<u64> where None indicates an invalid k-mer (contains
/// invalid or ambiguous characters depending on validation mode)
pub fn sliding_window_validated(
    sequence: &[u8],
    kmer_length: usize,
    validator: &CharacterValidator,
) -> Vec<Option<u64>> {
    sliding_window_validated_with_stats(sequence, kmer_length, validator, None)
}

/// Generate k-mers with optional ValidationStats recording.
///
/// When `stats` is `Some`, every character classification and every invalidated
/// k-mer is recorded — enabling the `--report-invalid` CLI flag to report
/// accurate statistics.
pub fn sliding_window_validated_with_stats(
    sequence: &[u8],
    kmer_length: usize,
    validator: &CharacterValidator,
    stats: Option<&ValidationStats>,
) -> Vec<Option<u64>> {
    // Guard against kmer_length == 0 which would panic in `windows(0)`
    if kmer_length == 0 || sequence.len() < kmer_length {
        return Vec::new();
    }

    let result_capacity = sequence.len() - kmer_length + 1;
    let mut result = Vec::with_capacity(result_capacity);

    // Record character classifications for all characters in the sequence
    if let Some(stats) = stats {
        for &ch in sequence {
            stats.record(validator.classify(ch));
        }
    }

    // Single-pass: validate AND encode in one iteration per window.
    // Previously this was two passes: window_has_invalid() then encode_kmer_validated(),
    // doubling character classifications (~108M redundant ops for typical runs).
    let base = encoding_base(validator);
    for window in sequence.windows(kmer_length) {
        match encode_kmer_single_pass(window, base, validator) {
            Some(encoded) => result.push(Some(encoded)),
            None => {
                if let Some(stats) = stats {
                    stats.record_invalidated_kmer();
                }
                result.push(None);
            }
        }
    }

    result
}

/// Single-pass k-mer validation and encoding.
/// Returns `None` on first invalid character (early exit — no wasted work).
/// Combines the logic of `window_has_invalid` + `encode_kmer_validated` into one pass.
#[inline(always)]
fn encode_kmer_single_pass(
    window: &[u8],
    base: u64,
    validator: &CharacterValidator,
) -> Option<u64> {
    let mut encoded = 0u64;
    for &byte in window {
        match validator.encode(byte) {
            Some(code) => {
                encoded = encoded.checked_mul(base)?.checked_add(code as u64)?;
            }
            None => return None,
        }
    }
    Some(encoded)
}

/// Convenience function for sliding window with default validation
///
/// Creates a validator internally based on `is_protein` flag.
/// For better performance when processing many sequences, create a
/// `CharacterValidator` once and use `sliding_window_validated` directly.
pub fn sliding_window_encoded_safe(
    sequence: &[u8],
    kmer_length: usize,
    is_protein: bool,
) -> Vec<Option<u64>> {
    let validator = if is_protein {
        CharacterValidator::protein()
    } else {
        CharacterValidator::nucleotide()
    };
    sliding_window_validated(sequence, kmer_length, &validator)
}

/// String-based sliding window with CharacterValidator
///
/// Returns strings instead of encoded values. K-mers containing invalid
/// characters are returned as "NA".
pub fn sliding_window_string_validated(
    sequence: &str,
    kmer_length: usize,
    validator: &CharacterValidator,
) -> Vec<String> {
    if kmer_length == 0 {
        return Vec::new();
    }
    let bytes = sequence.as_bytes();
    if bytes.len() < kmer_length {
        return Vec::new();
    }

    bytes
        .windows(kmer_length)
        .map(|window| {
            if validator.window_has_invalid(window) {
                String::from("NA")
            } else {
                // Safe because we're working with ASCII biological sequences
                unsafe { std::str::from_utf8_unchecked(window).to_string() }
            }
        })
        .collect()
}

/// Count k-mers and track their indices (string version)
pub fn count_kmers<'a>(kmers: &'a [Box<str>]) -> HashMap<&'a str, (usize, Vec<usize>)> {
    let mut counts: HashMap<&'a str, (usize, Vec<usize>)> = HashMap::new();
    kmers.iter().enumerate().for_each(|(idx, kmer)| {
        let entry = counts.entry(kmer).or_insert((0, vec![]));
        entry.0 += 1;
        entry.1.push(idx);
    });
    counts
}

/// Optimized k-mer counting with better memory allocation patterns
///
/// Returns a map from encoded k-mer to (count, indices).
/// Count occurrences of each encoded k-mer in a position column.
/// Skips u64::MAX sentinel values (invalid k-mers) while preserving their index slots
/// so that returned indices correctly map back to sequence headers.
pub fn count_kmers_encoded(kmers: &[u64]) -> HashMap<u64, (usize, Vec<usize>)> {
    let estimated_unique = std::cmp::min(kmers.len(), kmers.len() / 4 + 100);
    let mut counts: HashMap<u64, (usize, Vec<usize>)> = HashMap::with_capacity(estimated_unique);

    for (idx, &kmer) in kmers.iter().enumerate() {
        // u64::MAX is the sentinel for invalid/skipped k-mers — don't count them
        if kmer == u64::MAX {
            continue;
        }
        match counts.get_mut(&kmer) {
            Some(entry) => {
                entry.0 += 1;
                entry.1.push(idx);
            }
            None => {
                let mut indices = Vec::with_capacity(4);
                indices.push(idx);
                counts.insert(kmer, (1, indices));
            }
        }
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_based_encoding() {
        let validator = CharacterValidator::protein();

        // Valid k-mer should encode successfully
        let kmer = b"ACDEF";
        assert!(encode_kmer_validated(kmer, &validator).is_some());

        // K-mer with invalid character should fail
        let invalid_kmer = b"ACD#F";
        assert!(encode_kmer_validated(invalid_kmer, &validator).is_none());

        // K-mer with ambiguous character should fail
        let ambiguous_kmer = b"ACDXF";
        assert!(encode_kmer_validated(ambiguous_kmer, &validator).is_none());
    }

    #[test]
    fn test_sliding_window_validated() {
        let validator = CharacterValidator::protein();

        // Valid sequence
        let sequence = b"ACDEFG";
        let result = sliding_window_validated(sequence, 3, &validator);
        assert_eq!(result.len(), 4);
        assert!(result.iter().all(|x| x.is_some()));

        // Sequence with invalid character
        let sequence_invalid = b"ACD#FG";
        let result = sliding_window_validated(sequence_invalid, 3, &validator);
        assert_eq!(result.len(), 4);
        // K-mers containing # should be None
        assert!(result[0].is_some()); // ACD - valid
        assert!(result[1].is_none()); // CD# - invalid
        assert!(result[2].is_none()); // D#F - invalid
        assert!(result[3].is_none()); // #FG - invalid
    }

    #[test]
    fn test_invalid_character_rejection() {
        let validator = CharacterValidator::protein();

        // Test various invalid characters that should be rejected
        let invalid_chars = b"#*@!123456789()[]{}<>?/\\|`~";

        for &ch in invalid_chars {
            let kmer = [b'A', ch, b'C', b'D', b'E'];
            let result = encode_kmer_validated(&kmer, &validator);
            assert!(
                result.is_none(),
                "Character {} should be rejected",
                ch as char
            );
        }
    }

    #[test]
    fn test_lowercase_handling() {
        // Default: lowercase should be rejected
        let strict_validator = CharacterValidator::protein();
        let lowercase_kmer = b"acdef";
        assert!(encode_kmer_validated(lowercase_kmer, &strict_validator).is_none());

        // With allow_lowercase: should be accepted
        let permissive_validator =
            CharacterValidator::with_options(AlphabetType::Protein, ValidationMode::Strict, true);
        assert!(encode_kmer_validated(lowercase_kmer, &permissive_validator).is_some());
    }

    #[test]
    fn test_nucleotide_validation() {
        let validator = CharacterValidator::nucleotide();

        // Valid nucleotide k-mer
        let valid = b"ACGT";
        assert!(encode_kmer_validated(valid, &validator).is_some());

        // Invalid character in nucleotide sequence (protein amino acid)
        let invalid = b"ACEF"; // E is not a nucleotide
        assert!(encode_kmer_validated(invalid, &validator).is_none());

        // Ambiguous nucleotide character
        let ambiguous = b"ACGN"; // N is ambiguous
        assert!(encode_kmer_validated(ambiguous, &validator).is_none());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let validator = CharacterValidator::protein();
        let original = b"ACDEFG";
        let kmer_length = 6;

        if let Some(encoded) = encode_kmer_validated(original, &validator) {
            let decoded = decode_kmer(encoded, kmer_length, true);
            assert_eq!(decoded.as_bytes(), original);
        } else {
            panic!("Failed to encode k-mer");
        }
    }

    #[test]
    fn test_string_validated_sliding_window() {
        let validator = CharacterValidator::protein();

        let sequence = "ACDEFG";
        let result = sliding_window_string_validated(sequence, 3, &validator);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "ACD");
        assert_eq!(result[1], "CDE");
        assert_eq!(result[2], "DEF");
        assert_eq!(result[3], "EFG");

        // With invalid character
        let sequence_invalid = "ACD#FG";
        let result = sliding_window_string_validated(sequence_invalid, 3, &validator);
        assert_eq!(result[0], "ACD");
        assert_eq!(result[1], "NA"); // CD# is invalid
        assert_eq!(result[2], "NA"); // D#F is invalid
        assert_eq!(result[3], "NA"); // #FG is invalid
    }

    #[test]
    fn test_safe_encoded_function() {
        // Test the convenience function that uses whitelist internally
        let sequence = b"ACDEFG";
        let result = sliding_window_encoded_safe(sequence, 3, true);
        assert_eq!(result.len(), 4);
        assert!(result.iter().all(|x| x.is_some()));

        // Invalid characters should be rejected
        let sequence_invalid = b"ACD#FG";
        let result = sliding_window_encoded_safe(sequence_invalid, 3, true);
        assert!(result[1].is_none()); // Contains #
    }

    #[test]
    fn test_permissive_mode_still_invalidates_ambiguous() {
        // Per PMC11596295: support excludes gaps/ambiguous regardless of mode.
        // Permissive mode only affects reporting behavior, not encoding.
        let validator = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::Permissive,
            false,
        );

        // Ambiguous character X MUST invalidate k-mers (paper requirement)
        assert!(validator.should_invalidate_kmer(b'X'));

        // Invalid chars also invalidate
        assert!(validator.should_invalidate_kmer(b'#'));

        // Valid chars never invalidate
        assert!(!validator.should_invalidate_kmer(b'A'));
    }

    #[test]
    fn test_report_mode_still_invalidates_ambiguous() {
        // ReportOnly mode also invalidates ambiguous/invalid chars for encoding.
        // The "report only" aspect is about error handling, not encoding.
        let validator = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::ReportOnly,
            false,
        );

        assert!(validator.should_invalidate_kmer(b'X'));
        assert!(validator.should_invalidate_kmer(b'#'));
        assert!(!validator.should_invalidate_kmer(b'A'));
    }
}
