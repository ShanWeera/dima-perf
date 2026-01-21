use hashbrown::HashMap;

use crate::alphabet::CharacterValidator;

// Re-export for tests
#[cfg(test)]
use crate::alphabet::{AlphabetType, ValidationMode};

// Decoding tables for converting back to strings
const PROTEIN_CHARS: &[u8; 20] = b"ACDEFGHIKLMNPQRSTVWY";
const NUCLEOTIDE_CHARS: &[u8; 5] = b"ACGTU";

/// Encode a k-mer using the CharacterValidator for validation
/// 
/// This function validates each character using the whitelist approach
/// and returns None if any invalid character is found.
#[inline(always)]
pub fn encode_kmer_validated(kmer: &[u8], validator: &CharacterValidator) -> Option<u64> {
    let base = if validator.is_protein() { 20u64 } else { 5u64 };
    
    let mut encoded = 0u64;
    for &byte in kmer {
        match validator.encode(byte) {
            Some(code) => {
                encoded = encoded * base + code as u64;
            }
            None => return None, // Invalid or ambiguous character
        }
    }
    Some(encoded)
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

/// Decode a k-mer back to a string
pub fn decode_kmer(encoded: u64, kmer_length: usize, is_protein: bool) -> String {
    let mut result = Vec::with_capacity(kmer_length);
    let mut remaining = encoded;
    
    if is_protein {
        let base = 20u64;
        for _ in 0..kmer_length {
            let char_idx = (remaining % base) as usize;
            result.push(PROTEIN_CHARS[char_idx]);
            remaining /= base;
        }
    } else {
        let base = 5u64;
        for _ in 0..kmer_length {
            let char_idx = (remaining % base) as usize;
            result.push(NUCLEOTIDE_CHARS[char_idx]);
            remaining /= base;
        }
    }
    
    result.reverse();
    String::from_utf8(result).unwrap()
}

/// Generate k-mers from a sequence using whitelist-based character validation
/// 
/// This function uses the robust whitelist-based character validation from 
/// the alphabet module to ensure only valid biological sequences are processed.
/// 
/// # Arguments
/// * `sequence` - The sequence bytes to process
/// * `kmer_length` - Length of k-mers to generate
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
    if sequence.len() < kmer_length {
        return Vec::new();
    }

    let result_capacity = sequence.len() - kmer_length + 1;
    let mut result = Vec::with_capacity(result_capacity);

    // Process each k-mer window
    for window in sequence.windows(kmer_length) {
        if validator.window_has_invalid(window) {
            result.push(None);
        } else {
            result.push(encode_kmer_validated(window, validator));
        }
    }

    result
}

/// Batch processing version for very large sequences
/// 
/// This function processes sequences in chunks to optimize memory usage
/// and cache locality for extremely large inputs.
/// 
/// # Arguments
/// * `sequence` - The sequence bytes to process
/// * `kmer_length` - Length of k-mers to generate
/// * `validator` - CharacterValidator configured for the appropriate alphabet
/// * `batch_size` - Size of each processing batch
pub fn sliding_window_validated_batched(
    sequence: &[u8],
    kmer_length: usize,
    validator: &CharacterValidator,
    batch_size: usize,
) -> Vec<Option<u64>> {
    if sequence.len() < kmer_length {
        return Vec::new();
    }

    let total_kmers = sequence.len() - kmer_length + 1;
    let mut result = Vec::with_capacity(total_kmers);

    // Process in overlapping batches to maintain k-mer continuity
    let mut start = 0;
    while start < sequence.len() {
        let end = std::cmp::min(start + batch_size, sequence.len());
        let batch = &sequence[start..end];
        
        // Ensure we don't go beyond valid k-mer positions
        let batch_kmer_end = if end == sequence.len() {
            batch.len()
        } else {
            batch.len().saturating_sub(kmer_length - 1)
        };

        for window in batch.windows(kmer_length).take(batch_kmer_end.saturating_sub(kmer_length - 1)) {
            if validator.window_has_invalid(window) {
                result.push(None);
            } else {
                result.push(encode_kmer_validated(window, validator));
            }
        }

        // Move start position, accounting for k-mer overlap
        start += batch_size.saturating_sub(kmer_length - 1);
        if start >= sequence.len() {
            break;
        }
    }

    result
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

/// Check if a prefix string has an overlap with the start of another string
pub fn has_overlap_end(prefix: &str, next: &str) -> bool {
    let max_overlap = prefix.len().min(next.len());
    for k in (1..=max_overlap).rev() {
        if &prefix[prefix.len() - k..] == &next[..k] {
            return true;
        }
    }
    false
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
    kmers
        .iter()
        .enumerate()
        .for_each(|(idx, kmer)| {
            let entry = counts.entry(kmer).or_insert((0, vec![]));
            entry.0 += 1;
            entry.1.push(idx);
        });
    counts
}

/// Optimized k-mer counting with better memory allocation patterns
/// 
/// Returns a map from encoded k-mer to (count, indices).
pub fn count_kmers_encoded(kmers: &[u64]) -> HashMap<u64, (usize, Vec<usize>)> {
    // Pre-allocate with better capacity estimation based on expected uniqueness
    let estimated_unique = std::cmp::min(kmers.len(), kmers.len() / 4 + 100);
    let mut counts: HashMap<u64, (usize, Vec<usize>)> = HashMap::with_capacity(estimated_unique);
    
    for (idx, &kmer) in kmers.iter().enumerate() {
        match counts.get_mut(&kmer) {
            Some(entry) => {
                entry.0 += 1;
                entry.1.push(idx);
            }
            None => {
                // Pre-allocate index vector with reasonable initial capacity
                let mut indices = Vec::with_capacity(4);
                indices.push(idx);
                counts.insert(kmer, (1, indices));
            }
        }
    }
    
    counts
}

/// Transpose k-mers from sequence-oriented to position-oriented layout
pub fn transpose_kmers(kmers: &Vec<&Vec<String>>) -> Vec<Vec<String>> {
    assert!(!kmers.is_empty());
    let len = kmers[0].len();
    let mut iters: Vec<_> = kmers.into_iter().map(|n| n.into_iter()).collect();
    (0..len)
        .map(|_| {
            iters
                .iter_mut()
                .map(|n| n.next().unwrap().to_owned())
                .collect::<Vec<String>>()
        })
        .collect()
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
        assert!(result[0].is_some());  // ACD - valid
        assert!(result[1].is_none());  // CD# - invalid
        assert!(result[2].is_none());  // D#F - invalid
        assert!(result[3].is_none());  // #FG - invalid
    }

    #[test]
    fn test_invalid_character_rejection() {
        let validator = CharacterValidator::protein();
        
        // Test various invalid characters that should be rejected
        let invalid_chars = b"#*@!123456789()[]{}<>?/\\|`~";
        
        for &ch in invalid_chars {
            let kmer = [b'A', ch, b'C', b'D', b'E'];
            let result = encode_kmer_validated(&kmer, &validator);
            assert!(result.is_none(), "Character {} should be rejected", ch as char);
        }
    }

    #[test]
    fn test_lowercase_handling() {
        // Default: lowercase should be rejected
        let strict_validator = CharacterValidator::protein();
        let lowercase_kmer = b"acdef";
        assert!(encode_kmer_validated(lowercase_kmer, &strict_validator).is_none());
        
        // With allow_lowercase: should be accepted
        let permissive_validator = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::Strict,
            true,
        );
        assert!(encode_kmer_validated(lowercase_kmer, &permissive_validator).is_some());
    }

    #[test]
    fn test_nucleotide_validation() {
        let validator = CharacterValidator::nucleotide();
        
        // Valid nucleotide k-mer
        let valid = b"ACGT";
        assert!(encode_kmer_validated(valid, &validator).is_some());
        
        // Invalid character in nucleotide sequence (protein amino acid)
        let invalid = b"ACEF";  // E is not a nucleotide
        assert!(encode_kmer_validated(invalid, &validator).is_none());
        
        // Ambiguous nucleotide character
        let ambiguous = b"ACGN";  // N is ambiguous
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
        assert_eq!(result[1], "NA");  // CD# is invalid
        assert_eq!(result[2], "NA");  // D#F is invalid
        assert_eq!(result[3], "NA");  // #FG is invalid
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
        assert!(result[1].is_none());  // Contains #
    }

    #[test]
    fn test_permissive_mode() {
        // In permissive mode, ambiguous characters should NOT invalidate k-mers
        // but completely invalid characters should
        let validator = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::Permissive,
            false,
        );
        
        // Ambiguous character X should pass in permissive mode
        assert!(!validator.should_invalidate_kmer(b'X'));
        
        // But # should still fail
        assert!(validator.should_invalidate_kmer(b'#'));
    }

    #[test]
    fn test_report_mode() {
        // In report mode, nothing should invalidate k-mers
        let validator = CharacterValidator::with_options(
            AlphabetType::Protein,
            ValidationMode::ReportOnly,
            false,
        );
        
        assert!(!validator.should_invalidate_kmer(b'X'));
        assert!(!validator.should_invalidate_kmer(b'#'));
        assert!(!validator.should_invalidate_kmer(b'A'));
    }
}
