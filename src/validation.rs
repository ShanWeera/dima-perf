//! FASTA Validation Module (Shared)
//!
//! Validates FASTA files for use with `dima_lib::analyze()`. Combines structural
//! validation, alphabet detection, and header format detection into a single file
//! read for efficiency.
//!
//! Part of the public `dima_lib` API, used by the native GUI app to pre-validate
//! FASTA files before analysis (SRP: validation is a domain concern, not a UI
//! concern).
//!
//! Security: guards against non-regular files (directories, FIFOs, devices),
//! binary files, and BOM markers. Warns on large files (>500MB on disk).
//! Supports transparent decompression of gzip, bzip2, xz, and zstd files.
//! Symlinks are followed transparently — the target file is what gets validated.
//! Supports cooperative cancellation via `AtomicBool`.
//!
//! MSRV note: Uses `line_number % CANCEL_CHECK_INTERVAL == 0` instead of
//! `usize::is_multiple_of()` (stabilized in Rust 1.87) because dima_lib's
//! MSRV is 1.81.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use bzip2::bufread::MultiBzDecoder;
use flate2::bufread::MultiGzDecoder;
use liblzma::bufread::XzDecoder;

use crate::alphabet::AlphabetType;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Maximum line length before we suspect binary content. FASTA lines should
/// rarely exceed this; hitting it strongly suggests a binary file.
const MAX_LINE_LENGTH: usize = 1_000_000;

/// How often (in lines) to check the cancellation flag during validation.
/// 10,000 lines keeps overhead negligible while giving sub-second cancellation
/// on typical FASTA files.
const CANCEL_CHECK_INTERVAL: usize = 10_000;

/// Number of headers to sample for preview display in the UI.
const SAMPLE_HEADER_COUNT: usize = 3;

/// Number of sequences used for alphabet auto-detection.
const ALPHABET_DETECTION_SEQUENCES: usize = 10;

/// Number of headers to analyze for delimiter detection.
const HEADER_FORMAT_DETECTION_COUNT: usize = 5;

/// Characters that appear exclusively in protein sequences (not nucleotide).
/// If more than 1% of characters are from this set, the file is classified
/// as protein. Based on IUPAC amino acid codes.
const EXCLUSIVE_PROTEIN_CHARS: &[u8] = b"EFILPQefilpq";

// Magic bytes for compression format detection (same values as needletail).
const GZ_MAGIC: [u8; 2] = [0x1F, 0x8B];
const BZ_MAGIC: [u8; 2] = [0x42, 0x5A];
const XZ_MAGIC: [u8; 2] = [0xFD, 0x37];
const ZST_MAGIC: [u8; 2] = [0x28, 0xB5];

// ─── Public Types ───────────────────────────────────────────────────────────

/// Result of validating a FASTA file. Contains all information needed by the
/// GUI's Setup view to auto-populate configuration and display file metadata.
#[derive(Debug)]
pub struct FastaValidationResult {
    /// Number of sequences found in the file
    pub sequence_count: usize,
    /// Auto-detected alphabet type. `None` if detection failed (e.g., no sequences
    /// or ambiguous character composition). Maps to AlphabetType enum rather than
    /// a string for type safety.
    pub detected_alphabet: Option<AlphabetType>,
    /// Length of aligned sequences (all same length). `None` if sequences have
    /// different lengths (not an MSA). Used to compute `total_positions` for
    /// progress reporting: `alignment_length - kmer_length + 1`.
    pub alignment_length: Option<usize>,
    /// Whether all sequences have the same length (valid MSA)
    pub is_aligned: bool,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// File modification time for TOCTOU fingerprint. Raw `SystemTime` rather
    /// than formatted string -- the caller formats as needed. `None` if the
    /// file metadata is unavailable (e.g., some virtual filesystems).
    pub file_modified_at: Option<SystemTime>,
    /// Auto-detected header format (delimiter and field structure)
    pub header_format: Option<HeaderFormat>,
    /// First few headers for UI preview display
    pub sample_headers: Vec<String>,
    /// Non-fatal issues found during validation
    pub warnings: Vec<ValidationWarning>,
    /// Fatal errors that prevent analysis
    pub errors: Vec<ValidationError>,
}

impl FastaValidationResult {
    /// Whether the file is valid for analysis (no fatal errors)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Auto-detected header format describing delimiter and field structure.
#[derive(Debug, Clone)]
pub struct HeaderFormat {
    /// Delimiter character ('|', '\t', ';', or ',')
    pub delimiter: char,
    /// Number of fields per header when split by this delimiter
    pub field_count: usize,
    /// Reconstructed format string (e.g., "field1|field2|field3")
    pub format_string: String,
}

/// Non-fatal validation issues. These don't prevent analysis but the user
/// should be informed.
#[derive(Debug)]
pub enum ValidationWarning {
    /// File is larger than expected for typical FASTA
    LargeFileSize(u64),
    /// Sequences have unequal lengths (not a valid MSA for sliding-window analysis)
    UnequalSequenceLengths {
        expected: usize,
        found: usize,
        at_sequence: usize,
    },
    /// A sequence has zero length
    EmptySequence { at_sequence: usize },
}

/// Fatal validation errors that prevent analysis.
#[derive(Debug)]
pub enum ValidationError {
    /// File does not exist at the given path
    FileNotFound(String),
    /// Path exists but is not a regular file (e.g., directory, FIFO, device)
    NotRegularFile,
    /// File appears to contain binary (non-text) content
    BinaryFile,
    /// No FASTA headers (lines starting with '>') found
    NoHeaders,
    /// Headers found but no sequence data
    NoSequences,
    /// I/O error during file reading
    ReadError { line: usize, message: String },
    /// Validation was cancelled by the user
    Cancelled,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::FileNotFound(path) => write!(f, "file not found: {}", path),
            ValidationError::NotRegularFile => write!(f, "path is not a regular file"),
            ValidationError::BinaryFile => write!(f, "file appears to contain binary content"),
            ValidationError::NoHeaders => write!(f, "no FASTA headers found"),
            ValidationError::NoSequences => write!(f, "FASTA headers found but no sequence data"),
            ValidationError::ReadError { line, message } => {
                write!(f, "read error at line {}: {}", line, message)
            }
            ValidationError::Cancelled => write!(f, "validation cancelled"),
        }
    }
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Validate a FASTA file for use with `dima_lib::analyze()`.
///
/// Performs structural validation, alphabet detection, and header format detection
/// in a single file read. Used by the native GUI app for pre-analysis file
/// validation and available as part of the public `dima_lib` API.
///
/// # Arguments
/// * `path` - Path to the FASTA file
/// * `cancel_flag` - Optional cooperative cancellation flag (checked every ~10K lines)
///
/// # Returns
/// `Ok(FastaValidationResult)` with validation details, or `Err` for I/O failures
/// that prevent even opening the file.
pub fn validate_fasta(
    path: &Path,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<FastaValidationResult, std::io::Error> {
    let mut result = FastaValidationResult {
        sequence_count: 0,
        detected_alphabet: None,
        alignment_length: None,
        is_aligned: true,
        file_size_bytes: 0,
        file_modified_at: None,
        header_format: None,
        sample_headers: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    // ── Pre-flight checks (before opening file) ──

    if !path.exists() {
        result
            .errors
            .push(ValidationError::FileNotFound(path.display().to_string()));
        return Ok(result);
    }

    let metadata = fs::metadata(path)?;

    if !metadata.is_file() {
        result.errors.push(ValidationError::NotRegularFile);
        return Ok(result);
    }

    result.file_size_bytes = metadata.len();
    result.file_modified_at = metadata.modified().ok();

    if result.file_size_bytes > 500 * 1024 * 1024 {
        result
            .warnings
            .push(ValidationWarning::LargeFileSize(result.file_size_bytes));
    }

    // ── Read and validate file content ──
    // open_maybe_compressed() peeks at magic bytes and wraps the reader
    // in the appropriate decompression decoder if needed. Uncompressed
    // files pass through with negligible overhead (one extra BufReader layer).

    let file = File::open(path)?;
    let buf_reader = BufReader::new(file);
    let reader = open_maybe_compressed(buf_reader)?;

    let scan_result = scan_fasta_content(reader, cancel_flag);

    match scan_result {
        Ok(scan) => {
            if scan.cancelled {
                result.errors.push(ValidationError::Cancelled);
                return Ok(result);
            }
            if scan.is_binary {
                result.errors.push(ValidationError::BinaryFile);
                return Ok(result);
            }

            result.sequence_count = scan.sequence_count;
            result.sample_headers = scan.sample_headers.clone();

            if scan.header_count == 0 {
                result.errors.push(ValidationError::NoHeaders);
                return Ok(result);
            }
            if scan.sequence_count == 0 {
                result.errors.push(ValidationError::NoSequences);
                return Ok(result);
            }

            // Check alignment (all sequences same length)
            populate_alignment_info(&mut result, &scan);

            // Detect alphabet from sampled sequence content
            result.detected_alphabet = detect_alphabet(&scan.sampled_sequence_content);

            // Detect header format from sampled headers
            result.header_format = detect_header_format(&scan.all_headers_for_format);
        }
        Err(e) => {
            result.errors.push(ValidationError::ReadError {
                line: 0,
                message: e.to_string(),
            });
        }
    }

    Ok(result)
}

// ─── Internal Helpers ───────────────────────────────────────────────────────

/// Detect compression format from magic bytes and return a reader that
/// transparently decompresses. Uses `bufread` variants to avoid
/// double-buffering since the caller already provides a `BufReader`.
/// Multi-stream decoders are used for gzip, bzip2, and xz to handle
/// concatenated streams (common in bioinformatics, e.g. bgzip, pbzip2).
///
/// The `BufReader::fill_buf()` peek does not consume bytes -- the bytes
/// remain in the internal buffer and are re-read by the decoder when it
/// parses the compression header.
fn open_maybe_compressed(
    mut buf_reader: BufReader<File>,
) -> Result<BufReader<Box<dyn Read>>, std::io::Error> {
    let magic: [u8; 2] = {
        let buf = buf_reader.fill_buf()?;
        if buf.len() >= 2 {
            [buf[0], buf[1]]
        } else {
            // File is 0 or 1 bytes -- cannot be compressed. Pass through.
            [0, 0]
        }
    };

    let reader: Box<dyn Read> = match magic {
        GZ_MAGIC => Box::new(MultiGzDecoder::new(buf_reader)),
        BZ_MAGIC => Box::new(MultiBzDecoder::new(buf_reader)),
        XZ_MAGIC => Box::new(XzDecoder::new_multi_decoder(buf_reader)),
        ZST_MAGIC => Box::new(zstd::Decoder::with_buffer(buf_reader)?),
        _ => Box::new(buf_reader), // Uncompressed -- pass through
    };

    Ok(BufReader::new(reader))
}

/// Raw data collected from scanning the FASTA file content.
struct FastaScanResult {
    header_count: usize,
    sequence_count: usize,
    sample_headers: Vec<String>,
    /// First N headers for format detection (may overlap with sample_headers)
    all_headers_for_format: Vec<String>,
    /// Concatenated sequence content from first N sequences for alphabet detection
    sampled_sequence_content: Vec<u8>,
    /// (sequence_index, sequence_length) for alignment checking
    sequence_lengths: Vec<(usize, usize)>,
    is_binary: bool,
    cancelled: bool,
}

/// Scan FASTA file content in a single pass, collecting headers, sequence
/// lengths, and sampled content for alphabet detection.
fn scan_fasta_content<R: Read>(
    reader: BufReader<R>,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<FastaScanResult, std::io::Error> {
    let mut scan = FastaScanResult {
        header_count: 0,
        sequence_count: 0,
        sample_headers: Vec::new(),
        all_headers_for_format: Vec::new(),
        sampled_sequence_content: Vec::new(),
        sequence_lengths: Vec::new(),
        is_binary: false,
        cancelled: false,
    };

    let mut line_number: usize = 0;
    let mut current_seq_length: usize = 0;
    let mut in_sequence = false;
    let mut first_line = true;

    for line_result in reader.lines() {
        line_number += 1;

        // Cooperative cancellation check (every CANCEL_CHECK_INTERVAL lines).
        // Uses modulo instead of is_multiple_of() for MSRV 1.81 compatibility.
        if line_number % CANCEL_CHECK_INTERVAL == 0 {
            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    scan.cancelled = true;
                    return Ok(scan);
                }
            }
        }

        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                // Non-UTF-8 content strongly suggests binary file
                if e.kind() == std::io::ErrorKind::InvalidData {
                    scan.is_binary = true;
                    return Ok(scan);
                }
                return Err(e);
            }
        };

        // Guard against extremely long lines (binary file indicator)
        if line.len() > MAX_LINE_LENGTH {
            scan.is_binary = true;
            return Ok(scan);
        }

        // Check for null bytes (binary content indicator)
        if line.as_bytes().contains(&0) {
            scan.is_binary = true;
            return Ok(scan);
        }

        // Handle BOM on first line
        let line = if first_line {
            first_line = false;
            line.strip_prefix('\u{FEFF}').unwrap_or(&line).to_string()
        } else {
            line
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(header) = trimmed.strip_prefix('>') {
            // Flush previous sequence — only count as a "sequence" if it had
            // actual content. A header without content is NOT a sequence; this
            // makes the NoSequences error variant reachable for files with
            // headers but zero-length sequence data.
            if in_sequence {
                scan.sequence_lengths
                    .push((scan.header_count - 1, current_seq_length));
                if current_seq_length > 0 {
                    scan.sequence_count += 1;
                }
                current_seq_length = 0;
            }

            scan.header_count += 1;
            in_sequence = true;

            let header_text = header.trim().to_string();

            // Collect sample headers for UI preview
            if scan.sample_headers.len() < SAMPLE_HEADER_COUNT {
                scan.sample_headers.push(header_text.clone());
            }

            // Collect headers for format detection
            if scan.all_headers_for_format.len() < HEADER_FORMAT_DETECTION_COUNT {
                scan.all_headers_for_format.push(header_text);
            }
        } else if in_sequence {
            current_seq_length += trimmed.len();

            // Sample sequence content for alphabet detection (first N sequences only)
            if scan.sequence_count < ALPHABET_DETECTION_SEQUENCES {
                scan.sampled_sequence_content
                    .extend_from_slice(trimmed.as_bytes());
            }
        }
    }

    // Flush last sequence — only count if it has actual content
    if in_sequence {
        scan.sequence_lengths
            .push((scan.header_count - 1, current_seq_length));
        if current_seq_length > 0 {
            scan.sequence_count += 1;
        }
    }

    Ok(scan)
}

/// Populate alignment information from scanned sequence lengths.
fn populate_alignment_info(result: &mut FastaValidationResult, scan: &FastaScanResult) {
    if scan.sequence_lengths.is_empty() {
        result.is_aligned = false;
        return;
    }

    // Check for empty sequences
    for &(idx, len) in &scan.sequence_lengths {
        if len == 0 {
            result.warnings.push(ValidationWarning::EmptySequence {
                at_sequence: idx + 1,
            });
        }
    }

    // Check alignment: all non-zero-length sequences must have the same length
    let non_empty_lengths: Vec<&(usize, usize)> = scan
        .sequence_lengths
        .iter()
        .filter(|(_, len)| *len > 0)
        .collect();

    if non_empty_lengths.is_empty() {
        result.is_aligned = false;
        return;
    }

    let reference_length = non_empty_lengths[0].1;
    for &(idx, len) in &non_empty_lengths[1..] {
        if *len != reference_length {
            result.is_aligned = false;
            result
                .warnings
                .push(ValidationWarning::UnequalSequenceLengths {
                    expected: reference_length,
                    found: *len,
                    at_sequence: idx + 1,
                });
            // Report only the first mismatch to avoid flooding warnings
            break;
        }
    }

    if result.is_aligned && reference_length > 0 {
        result.alignment_length = Some(reference_length);
    }
}

/// Detect alphabet type from sampled sequence content.
///
/// Uses character frequency analysis: if more than 1% of characters are from
/// the exclusive protein character set (E, F, I, L, P, Q), the file is classified
/// as protein. Otherwise nucleotide.
fn detect_alphabet(content: &[u8]) -> Option<AlphabetType> {
    if content.is_empty() {
        return None;
    }

    // Count characters, ignoring gaps and non-alphabetic characters
    let mut total_chars: usize = 0;
    let mut protein_exclusive_count: usize = 0;

    let protein_set: HashSet<u8> = EXCLUSIVE_PROTEIN_CHARS.iter().copied().collect();

    for &byte in content {
        // Skip gaps and non-alphabetic characters
        if !byte.is_ascii_alphabetic() {
            continue;
        }

        total_chars += 1;
        if protein_set.contains(&byte) {
            protein_exclusive_count += 1;
        }
    }

    if total_chars == 0 {
        return None;
    }

    let protein_fraction = protein_exclusive_count as f64 / total_chars as f64;

    if protein_fraction > 0.01 {
        Some(AlphabetType::Protein)
    } else {
        Some(AlphabetType::Nucleotide)
    }
}

/// Detect header format (delimiter and field structure) from sampled headers.
///
/// Checks common delimiters (pipe, tab, semicolon, comma) and picks the one
/// that produces a consistent field count across all sampled headers.
fn detect_header_format(headers: &[String]) -> Option<HeaderFormat> {
    if headers.is_empty() {
        return None;
    }

    let delimiters = ['|', '\t', ';', ','];

    for &delim in &delimiters {
        let field_counts: Vec<usize> = headers.iter().map(|h| h.split(delim).count()).collect();

        // All headers must produce the same field count, and it must be > 1
        // (a single field means the delimiter wasn't found)
        let first_count = field_counts[0];
        if first_count > 1 && field_counts.iter().all(|&c| c == first_count) {
            // Build format string from field names of the first header
            let fields: Vec<&str> = headers[0].split(delim).collect();
            let format_string = fields.join(&delim.to_string());

            return Some(HeaderFormat {
                delimiter: delim,
                field_count: first_count,
                format_string,
            });
        }
    }

    None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_fasta(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_valid_aligned_protein_fasta() {
        let fasta = write_temp_fasta(">seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n>seq3\nACDEFGHIKL\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 3);
        assert!(result.is_aligned);
        assert_eq!(result.alignment_length, Some(10));
        // F, I, L are exclusive protein chars -- should detect protein
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Protein));
    }

    #[test]
    fn test_valid_nucleotide_fasta() {
        let fasta = write_temp_fasta(">seq1\nACGTACGTAC\n>seq2\nACGTACGTAC\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Nucleotide));
    }

    #[test]
    fn test_unequal_lengths_detected() {
        let fasta = write_temp_fasta(">seq1\nACDEFGHIKL\n>seq2\nACDEF\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(result.is_valid()); // Unequal length is a warning, not an error
        assert!(!result.is_aligned);
        assert!(result.alignment_length.is_none());
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_empty_file_no_headers() {
        let fasta = write_temp_fasta("");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(!result.is_valid());
        assert!(matches!(result.errors[0], ValidationError::NoHeaders));
    }

    #[test]
    fn test_headers_only_no_sequences() {
        let fasta = write_temp_fasta(">seq1\n>seq2\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        // Headers exist but no sequence content — NoSequences error
        assert!(!result.is_valid());
        assert_eq!(result.sequence_count, 0);
        assert!(matches!(result.errors[0], ValidationError::NoSequences));
    }

    #[test]
    fn test_file_not_found() {
        let result = validate_fasta(Path::new("/nonexistent/path.fasta"), None).unwrap();
        assert!(!result.is_valid());
        assert!(matches!(result.errors[0], ValidationError::FileNotFound(_)));
    }

    #[test]
    fn test_binary_file_detection_null_bytes() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b">seq1\nACDE\x00FGHIKL\n").unwrap();
        f.flush().unwrap();
        let result = validate_fasta(f.path(), None).unwrap();
        assert!(!result.is_valid());
        assert!(matches!(result.errors[0], ValidationError::BinaryFile));
    }

    #[test]
    fn test_cancellation() {
        let fasta = write_temp_fasta(">seq1\nACDEFGHIKL\n");
        let cancel = Arc::new(AtomicBool::new(true));
        let result = validate_fasta(fasta.path(), Some(&cancel)).unwrap();
        // With only a few lines, cancellation may or may not trigger depending
        // on the check interval. The key is it doesn't crash.
        let _ = result;
    }

    #[test]
    fn test_bom_handling() {
        let fasta = write_temp_fasta("\u{FEFF}>seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
    }

    #[test]
    fn test_header_format_detection_pipe() {
        let fasta =
            write_temp_fasta(">id1|country|host\nACDEFGHIKL\n>id2|country|host\nACDEFGHIKL\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(result.is_valid());
        let fmt = result.header_format.unwrap();
        assert_eq!(fmt.delimiter, '|');
        assert_eq!(fmt.field_count, 3);
    }

    #[test]
    fn test_sample_headers_collected() {
        let fasta = write_temp_fasta(
            ">alpha\nACDEFGHIKL\n>beta\nACDEFGHIKL\n>gamma\nACDEFGHIKL\n>delta\nACDEFGHIKL\n",
        );
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert_eq!(result.sample_headers.len(), 3); // Capped at SAMPLE_HEADER_COUNT
        assert_eq!(result.sample_headers[0], "alpha");
        assert_eq!(result.sample_headers[1], "beta");
        assert_eq!(result.sample_headers[2], "gamma");
    }

    #[test]
    fn test_detect_alphabet_empty_content() {
        assert_eq!(detect_alphabet(&[]), None);
    }

    #[test]
    fn test_detect_alphabet_pure_nucleotide() {
        assert_eq!(
            detect_alphabet(b"ACGTACGTACGT"),
            Some(AlphabetType::Nucleotide)
        );
    }

    #[test]
    fn test_detect_alphabet_protein_chars() {
        // Contains F, I, L which are exclusive protein chars
        assert_eq!(
            detect_alphabet(b"ACDEFGHIKLMNPQRSTVWY"),
            Some(AlphabetType::Protein)
        );
    }

    #[test]
    fn test_detect_header_format_no_delimiter() {
        let headers = vec!["simple_header".to_string(), "another_header".to_string()];
        assert!(detect_header_format(&headers).is_none());
    }

    #[test]
    fn test_detect_header_format_inconsistent_fields() {
        let headers = vec![
            "a|b|c".to_string(),
            "d|e".to_string(), // Different field count
        ];
        // Pipe detection should fail due to inconsistent counts
        let fmt = detect_header_format(&headers);
        assert!(fmt.is_none());
    }

    #[test]
    fn test_detect_header_format_tab_delimiter() {
        let headers = vec![
            "id\tcountry\thost".to_string(),
            "id2\tusa\thuman".to_string(),
        ];
        let fmt = detect_header_format(&headers).unwrap();
        assert_eq!(fmt.delimiter, '\t');
        assert_eq!(fmt.field_count, 3);
    }

    // ── Compressed FASTA tests ──────────────────────────────────────────────
    // Each test creates a compressed in-memory FASTA, writes it to a temp file,
    // and validates it through the same validate_fasta() entry point used by
    // both the CLI and GUI. This exercises the full open_maybe_compressed()
    // pipeline including magic-byte detection.

    #[test]
    fn test_gzip_compressed_fasta() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let fasta_content = b">seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(fasta_content).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&compressed).unwrap();
        f.flush().unwrap();

        let result = validate_fasta(f.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Protein));
    }

    #[test]
    fn test_bzip2_compressed_fasta() {
        use bzip2::write::BzEncoder;
        use bzip2::Compression;

        let fasta_content = b">seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n";
        let mut encoder = BzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(fasta_content).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&compressed).unwrap();
        f.flush().unwrap();

        let result = validate_fasta(f.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Protein));
    }

    #[test]
    fn test_xz_compressed_fasta() {
        use liblzma::write::XzEncoder;

        let fasta_content = b">seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n";
        let mut encoder = XzEncoder::new(Vec::new(), 6);
        encoder.write_all(fasta_content).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&compressed).unwrap();
        f.flush().unwrap();

        let result = validate_fasta(f.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Protein));
    }

    #[test]
    fn test_zstd_compressed_fasta() {
        let fasta_content = b">seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n";
        let compressed = zstd::encode_all(&fasta_content[..], 3).unwrap();

        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&compressed).unwrap();
        f.flush().unwrap();

        let result = validate_fasta(f.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Protein));
    }

    #[test]
    fn test_uncompressed_fasta_still_works() {
        // Regression test: ensure the decompression layer doesn't break
        // plain text FASTA files (which start with '>' = 0x3E, not matching
        // any compression magic bytes).
        let fasta = write_temp_fasta(">seq1\nACGTACGTAC\n>seq2\nACGTACGTAC\n");
        let result = validate_fasta(fasta.path(), None).unwrap();
        assert!(result.is_valid());
        assert_eq!(result.sequence_count, 2);
        assert_eq!(result.detected_alphabet, Some(AlphabetType::Nucleotide));
    }
}
