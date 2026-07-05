//! FASTA Validation Commands
//!
//! Commands for validating FASTA files and detecting header formats.
//! Includes protection against symlinks, FIFOs, binary files, and excessively large files.
//! Supports cooperative cancellation via an AtomicBool flag (Fix 4.29).

use crate::error::AppError;
use crate::state::AppState;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;

/// Maximum file size we'll validate (2 GB)
const MAX_VALIDATE_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// Maximum number of sequences to scan for validation
const MAX_SCAN_SEQUENCES: usize = 1000;

/// Maximum line length before we suspect binary content
const MAX_LINE_LENGTH: usize = 1_000_000;

/// Result of FASTA file validation
#[derive(Debug, Serialize)]
pub struct FastaValidation {
    pub is_valid: bool,
    pub sequence_count: usize,
    pub sequence_length: Option<usize>,
    pub sample_headers: Vec<String>,
    pub detected_alphabet: String,
    pub errors: Vec<ValidationError>,
    pub file_size_bytes: u64,
    pub file_modified_at: Option<String>,
}

/// Validation error with location info
#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub error_type: String,
    pub message: String,
    pub line_number: Option<usize>,
}

/// How often (in lines) to check the cancellation flag during validation.
/// Balancing between responsiveness (lower = faster cancel) and overhead (higher = less
/// atomic load cost). 10,000 lines keeps overhead negligible while giving sub-second
/// cancellation on typical FASTA files.
const CANCEL_CHECK_INTERVAL: usize = 10_000;

/// Validate a FASTA file.
/// Delegates all blocking std::fs I/O to a background thread via spawn_blocking
/// to avoid stalling the Tokio async runtime. (Fix 4.13)
/// Supports cooperative cancellation via the validation_cancel_flag in AppState. (Fix 4.29)
#[tauri::command]
pub async fn validate_fasta(
    path: String,
    _alphabet: Option<String>,
    state: State<'_, AppState>,
) -> Result<FastaValidation, AppError> {
    // Reset the cancel flag at the start of each new validation so a stale `true`
    // from a prior cancelled run doesn't immediately abort this one.
    state.reset_validation_cancel();
    let cancel_flag = state.validation_cancel_flag.clone();

    tokio::task::spawn_blocking(move || validate_fasta_blocking(&path, &cancel_flag))
        .await
        .map_err(|e| AppError::InternalError(format!("Validation task failed: {}", e)))?
}

/// Cancel the current validation task (if running). (Fix 4.29)
/// The frontend calls this when the user selects a different file or navigates away,
/// preventing wasted CPU/IO on abandoned validation of large files.
#[tauri::command]
pub async fn cancel_validation(state: State<'_, AppState>) -> Result<(), AppError> {
    state.request_validation_cancel();
    Ok(())
}

/// Test-accessible wrapper that validates without a cancel flag (uses a no-op flag).
/// Allows integration tests to exercise the full validation logic without needing
/// a Tauri `State<AppState>` injection.
#[cfg(test)]
pub fn validate_fasta_blocking_public(path: &str) -> Result<FastaValidation, AppError> {
    let no_cancel = Arc::new(AtomicBool::new(false));
    validate_fasta_blocking(path, &no_cancel)
}

fn validate_fasta_blocking(
    path: &str,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<FastaValidation, AppError> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "file_not_found".to_string(),
                message: format!("File not found: {}", file_path.display()),
                line_number: None,
            }],
            file_size_bytes: 0,
            file_modified_at: None,
        });
    }

    // Security: check that target is a regular file (not symlink to device, FIFO, etc.)
    let metadata = fs::metadata(file_path)?;
    if !metadata.is_file() {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "not_regular_file".to_string(),
                message: "Path does not point to a regular file".to_string(),
                line_number: None,
            }],
            file_size_bytes: 0,
            file_modified_at: None,
        });
    }

    let file_size_bytes = metadata.len();

    if file_size_bytes > MAX_VALIDATE_FILE_SIZE {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "file_too_large".to_string(),
                message: format!(
                    "File exceeds maximum size ({} GB limit)",
                    MAX_VALIDATE_FILE_SIZE / (1024 * 1024 * 1024)
                ),
                line_number: None,
            }],
            file_size_bytes,
            file_modified_at: None,
        });
    }

    let file_modified_at = crate::project::file_mtime_fingerprint(&metadata);

    // Open file and handle BOM
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // Check for and skip UTF-8 BOM (EF BB BF)
    let mut bom_buf = [0u8; 3];
    let bom_read = reader.read(&mut bom_buf)?;
    let has_bom = bom_read == 3 && bom_buf == [0xEF, 0xBB, 0xBF];
    if !has_bom && bom_read > 0 {
        // Not BOM — need to re-open to not lose these bytes
        drop(reader);
        let file = File::open(file_path)?;
        reader = BufReader::new(file);
    }

    // Check first bytes for binary content (null bytes indicate non-text)
    let first_check: Vec<u8> = if has_bom {
        let mut buf = vec![0u8; 512];
        let n = reader.read(&mut buf)?;
        buf.truncate(n);
        // Re-open after BOM skip
        drop(reader);
        let file = File::open(file_path)?;
        reader = BufReader::new(file);
        // Skip BOM again
        let mut skip = [0u8; 3];
        let _ = reader.read(&mut skip);
        buf
    } else {
        // No BOM detected — read first 512 bytes from a temporary reader, then
        // re-open from the start for full parsing (no BOM bytes to skip).
        drop(reader);
        let file = File::open(file_path)?;
        let mut temp_reader = BufReader::new(file);
        let mut buf = vec![0u8; 512];
        let n = temp_reader.read(&mut buf)?;
        buf.truncate(n);
        drop(temp_reader);
        let file = File::open(file_path)?;
        reader = BufReader::new(file);
        buf
    };

    if first_check.contains(&0) {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "binary_file".to_string(),
                message: "File appears to be binary (contains null bytes). FASTA files must be plain text.".to_string(),
                line_number: None,
            }],
            file_size_bytes,
            file_modified_at: file_modified_at.clone(),
        });
    }

    let mut headers: Vec<String> = Vec::new();
    let mut sequences: Vec<String> = Vec::new();
    let mut current_sequence = String::new();
    let mut line_number: usize = 0;
    let mut errors: Vec<ValidationError> = Vec::new();
    let mut found_header = false;

    // Use an explicit iterator so we can resume reading from the same position
    // after the sampling loop breaks (for the streaming length-check pass).
    let mut lines = reader.lines();

    for line_result in lines.by_ref() {
        line_number += 1;

        // Cooperative cancellation check every CANCEL_CHECK_INTERVAL lines (Fix 4.29).
        // Avoids per-line atomic load overhead while keeping cancel latency under ~100ms
        // for typical line lengths.
        if line_number.is_multiple_of(CANCEL_CHECK_INTERVAL) && cancel_flag.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled("Validation cancelled".to_string()));
        }

        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                errors.push(ValidationError {
                    error_type: "read_error".to_string(),
                    message: format!("Failed to read line: {}", e),
                    line_number: Some(line_number),
                });
                break;
            }
        };

        // Guard against excessively long lines (likely binary)
        if line.len() > MAX_LINE_LENGTH {
            errors.push(ValidationError {
                error_type: "line_too_long".to_string(),
                message: format!(
                    "Line exceeds {} characters — file may be binary or corrupt",
                    MAX_LINE_LENGTH
                ),
                line_number: Some(line_number),
            });
            break;
        }

        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Skip comment lines (semicolons)
        if trimmed.starts_with(';') {
            continue;
        }

        if let Some(header_content) = trimmed.strip_prefix('>') {
            found_header = true;
            // Save previous sequence if exists (take avoids clone + clear overhead)
            if !current_sequence.is_empty() {
                sequences.push(std::mem::take(&mut current_sequence));
            } else if !headers.is_empty() {
                // Empty sequence between consecutive headers (previous header had no sequence data).
                // Check !headers.is_empty() (not > 1) because the first push hasn't happened yet.
                errors.push(ValidationError {
                    error_type: "empty_sequence".to_string(),
                    message: format!("Empty sequence for header at line {}", line_number - 1),
                    line_number: Some(line_number - 1),
                });
            }
            headers.push(header_content.to_string());
        } else {
            // Cap sequence accumulation at 100MB to prevent OOM from single-record
            // FASTA files (entire file in one String). Only the first portion is needed
            // for alphabet detection; the total length is tracked for MSA validation.
            const MAX_SEQUENCE_ACCUMULATE: usize = 100 * 1024 * 1024;
            if current_sequence.len() < MAX_SEQUENCE_ACCUMULATE {
                current_sequence.push_str(trimmed);
            }
        }

        // After collecting enough sequences for alphabet detection, switch to
        // length-only streaming to check MSA alignment across ALL sequences.
        // This eliminates the blind spot where sequences after MAX_SCAN_SEQUENCES
        // could have different lengths without being caught.
        if sequences.len() >= MAX_SCAN_SEQUENCES {
            break;
        }
    }

    // Don't forget the last sequence
    if !current_sequence.is_empty() {
        sequences.push(std::mem::take(&mut current_sequence));
    }

    // Continue reading the rest of the file in streaming mode (length-only)
    // to catch MSA length mismatches beyond the sampling window. Only tracks
    // sequence length without storing content — O(1) memory per sequence.
    let expected_length = sequences.first().map(|s| s.len());
    let mut total_sequences_scanned = sequences.len();
    let mut streaming_length_mismatches: Vec<(usize, usize, usize)> = Vec::new();

    if sequences.len() >= MAX_SCAN_SEQUENCES {
        let mut tail_seq_len: usize = 0;
        let mut in_sequence = !current_sequence.is_empty();
        let mut streaming_line_count: usize = 0;

        for line_result in lines {
            streaming_line_count += 1;

            // Cancellation check during the streaming tail pass (Fix 4.29)
            if streaming_line_count.is_multiple_of(CANCEL_CHECK_INTERVAL)
                && cancel_flag.load(Ordering::Relaxed)
            {
                return Err(AppError::Cancelled("Validation cancelled".to_string()));
            }

            let line = match line_result {
                Ok(l) => l,
                Err(_) => break,
            };
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }
            if trimmed.starts_with('>') {
                if in_sequence && tail_seq_len > 0 {
                    total_sequences_scanned += 1;
                    if let Some(expected) = expected_length {
                        if tail_seq_len != expected && streaming_length_mismatches.len() < 5 {
                            streaming_length_mismatches.push((
                                total_sequences_scanned,
                                tail_seq_len,
                                expected,
                            ));
                        }
                    }
                }
                tail_seq_len = 0;
                in_sequence = true;
            } else {
                tail_seq_len += trimmed.len();
            }
        }
        // Final trailing sequence
        if in_sequence && tail_seq_len > 0 {
            total_sequences_scanned += 1;
            if let Some(expected) = expected_length {
                if tail_seq_len != expected && streaming_length_mismatches.len() < 5 {
                    streaming_length_mismatches.push((
                        total_sequences_scanned,
                        tail_seq_len,
                        expected,
                    ));
                }
            }
        }
    }

    // Validate we have at least one header line (prevents accepting non-FASTA text)
    if !found_header {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "no_headers".to_string(),
                message: "No FASTA header lines found (lines starting with '>'). This does not appear to be a FASTA file.".to_string(),
                line_number: None,
            }],
            file_size_bytes,
            file_modified_at: file_modified_at.clone(),
        });
    }

    // Validate we have sequences
    if sequences.is_empty() {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "no_sequences".to_string(),
                message: "No sequences found in file".to_string(),
                line_number: None,
            }],
            file_size_bytes,
            file_modified_at: file_modified_at.clone(),
        });
    }

    // Check sequence lengths (MSA validation — all must be equal)
    let first_length = sequences.first().map(|s| s.len());
    let mut length_mismatches: Vec<(usize, usize, usize)> = Vec::new();

    for (idx, seq) in sequences.iter().enumerate() {
        if let Some(expected) = first_length {
            if seq.len() != expected {
                length_mismatches.push((idx + 1, seq.len(), expected));
            }
        }
    }

    // Merge mismatches from both the sampled window and streaming tail
    let all_mismatches: Vec<(usize, usize, usize)> = length_mismatches
        .into_iter()
        .chain(streaming_length_mismatches)
        .collect();

    for (seq_num, actual, expected) in all_mismatches.iter().take(5) {
        errors.push(ValidationError {
            error_type: "length_mismatch".to_string(),
            message: format!(
                "Sequence {} has length {}, expected {} (all sequences in an MSA must be equal length)",
                seq_num, actual, expected
            ),
            line_number: None,
        });
    }
    if all_mismatches.len() > 5 {
        errors.push(ValidationError {
            error_type: "length_mismatch".to_string(),
            message: format!("...and {} more length mismatches", all_mismatches.len() - 5),
            line_number: None,
        });
    }

    // Detect alphabet
    let detected_alphabet = detect_alphabet(&sequences);

    // Get sample headers (first 3)
    let sample_headers: Vec<String> = headers.iter().take(3).cloned().collect();

    // If we streamed through the whole file, we have an exact count.
    // Otherwise (file small enough to fully scan), use the sampled count.
    let estimated_total = if total_sequences_scanned > sequences.len() {
        total_sequences_scanned
    } else if sequences.len() >= MAX_SCAN_SEQUENCES {
        // Fallback: estimate from file size if streaming didn't run (shouldn't happen)
        let sample_count = sequences.len().min(headers.len());
        let scanned_bytes: u64 = sequences
            .iter()
            .zip(headers.iter())
            .take(sample_count)
            .map(|(seq, hdr)| (seq.len() + hdr.len() + 3) as u64)
            .sum();
        let avg_bytes_per_seq = scanned_bytes / (sample_count as u64).max(1);
        file_size_bytes
            .checked_div(avg_bytes_per_seq)
            .map(|v| v as usize)
            .unwrap_or(sequences.len())
    } else {
        sequences.len()
    };

    let is_valid = errors.is_empty();

    Ok(FastaValidation {
        is_valid,
        sequence_count: estimated_total,
        sequence_length: first_length,
        sample_headers,
        detected_alphabet,
        errors,
        file_size_bytes,
        file_modified_at,
    })
}

/// Detect whether sequences are protein or nucleotide.
///
/// Strategy: if exclusive protein letters (those that NEVER appear in nucleotide
/// sequences) are present at > 1% frequency, classify as protein regardless of
/// nucleotide ratio. This prevents misclassifying protein MSAs rich in Ala/Gly/Thr/Cys
/// which share letters with DNA (A, G, T, C). IUPAC ambiguity codes are included
/// in the extended nucleotide set for accurate detection of ambiguous DNA/RNA MSAs.
fn detect_alphabet(sequences: &[String]) -> String {
    // Extended nucleotide character set (includes IUPAC ambiguity codes)
    let nucleotides: HashSet<char> = "ACGTUNRYSWKMBDHV".chars().collect();
    // Letters that appear ONLY in protein sequences — never in nucleotide
    let exclusive_protein: HashSet<char> = "EFIJLPQXZ".chars().collect();

    let mut nucleotide_count: usize = 0;
    let mut exclusive_protein_count: usize = 0;
    let mut total_count: usize = 0;

    for seq in sequences.iter().take(10) {
        for c in seq.to_uppercase().chars() {
            if c == '-' || c == '.' || c == '*' {
                continue;
            }
            total_count += 1;
            if nucleotides.contains(&c) {
                nucleotide_count += 1;
            }
            if exclusive_protein.contains(&c) {
                exclusive_protein_count += 1;
            }
        }
    }

    if total_count == 0 {
        return "unknown".to_string();
    }

    // If exclusive protein letters are present at > 1%, it's definitely protein
    let exclusive_protein_ratio = exclusive_protein_count as f64 / total_count as f64;
    if exclusive_protein_ratio > 0.01 {
        return "protein".to_string();
    }

    let nucleotide_ratio = nucleotide_count as f64 / total_count as f64;
    if nucleotide_ratio > 0.9 {
        "nucleotide".to_string()
    } else {
        "protein".to_string()
    }
}

/// Detected header format
#[derive(Debug, Serialize)]
pub struct HeaderFormatDetection {
    pub detected_format: Option<String>,
    pub detected_delimiter: Option<String>,
    pub field_count: usize,
    pub sample_parsed: Vec<ParsedHeader>,
    pub suggested_fields: Vec<String>,
}

/// A parsed header with detected fields
#[derive(Debug, Serialize)]
pub struct ParsedHeader {
    pub raw: String,
    pub fields: Vec<String>,
}

/// Detect header format from sample headers.
/// Runs on a blocking thread and applies the same security checks as validate_fasta. (Fix 4.40)
#[tauri::command]
pub async fn detect_header_format(path: String) -> Result<HeaderFormatDetection, AppError> {
    tokio::task::spawn_blocking(move || detect_header_format_blocking(&path))
        .await
        .map_err(|e| AppError::InternalError(format!("Header detection task failed: {}", e)))?
}

fn detect_header_format_blocking(path: &str) -> Result<HeaderFormatDetection, AppError> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(AppError::NotFound("File not found".to_string()));
    }

    let meta = fs::metadata(file_path)?;
    if !meta.is_file() {
        return Err(AppError::ValidationError(
            "Path does not point to a regular file".to_string(),
        ));
    }

    // Apply the same size cap as validate_fasta to prevent resource exhaustion
    if meta.len() > MAX_VALIDATE_FILE_SIZE {
        return Err(AppError::ValidationError(format!(
            "File exceeds maximum size ({} GB limit)",
            MAX_VALIDATE_FILE_SIZE / (1024 * 1024 * 1024)
        )));
    }

    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // Skip BOM if present
    let mut bom_buf = [0u8; 3];
    let bom_read = reader.read(&mut bom_buf).unwrap_or(0);
    if !(bom_read == 3 && bom_buf == [0xEF, 0xBB, 0xBF]) {
        // Not BOM — re-open
        drop(reader);
        let file = File::open(file_path)?;
        reader = BufReader::new(file);
    }

    let mut headers: Vec<String> = Vec::new();
    for line_result in reader.lines() {
        let line = line_result?;
        let trimmed = line.trim();
        if let Some(header_content) = trimmed.strip_prefix('>') {
            headers.push(header_content.to_string());
            if headers.len() >= 5 {
                break;
            }
        }
    }

    if headers.is_empty() {
        return Ok(HeaderFormatDetection {
            detected_format: None,
            detected_delimiter: None,
            field_count: 0,
            sample_parsed: vec![],
            suggested_fields: vec![],
        });
    }

    // Try to detect delimiter and field count
    let delimiters = ['|', '\t', ';', ','];
    let mut best_delimiter: Option<char> = None;
    let mut best_field_count = 0;

    for delimiter in delimiters {
        let counts: Vec<usize> = headers.iter().map(|h| h.split(delimiter).count()).collect();
        if counts.iter().all(|&c| c == counts[0]) && counts[0] > 1 && counts[0] > best_field_count {
            best_field_count = counts[0];
            best_delimiter = Some(delimiter);
        }
    }

    let (detected_format, sample_parsed) = if let Some(delim) = best_delimiter {
        let format = (0..best_field_count)
            .map(|i| format!("field{}", i + 1))
            .collect::<Vec<_>>()
            .join(&delim.to_string());

        let parsed: Vec<ParsedHeader> = headers
            .iter()
            .take(3)
            .map(|h| ParsedHeader {
                raw: h.clone(),
                fields: h.split(delim).map(|s| s.to_string()).collect(),
            })
            .collect();

        (Some(format), parsed)
    } else {
        let parsed: Vec<ParsedHeader> = headers
            .iter()
            .take(3)
            .map(|h| ParsedHeader {
                raw: h.clone(),
                fields: vec![h.clone()],
            })
            .collect();

        (None, parsed)
    };

    let suggested_fields = vec![
        "accession".to_string(),
        "country".to_string(),
        "date".to_string(),
        "host".to_string(),
        "strain".to_string(),
        "isolate".to_string(),
    ];

    Ok(HeaderFormatDetection {
        detected_format,
        detected_delimiter: best_delimiter.map(|c| c.to_string()),
        field_count: best_field_count.max(1),
        sample_parsed,
        suggested_fields,
    })
}
