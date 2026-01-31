//! FASTA Validation Commands
//!
//! Commands for validating FASTA files and detecting header formats.

use rand::seq::SliceRandom;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

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

/// Validate a FASTA file
#[tauri::command]
pub async fn validate_fasta(
    path: String,
    _alphabet: Option<String>, // Reserved for explicit alphabet validation
) -> Result<FastaValidation, String> {
    let file_path = Path::new(&path);

    if !file_path.exists() {
        return Ok(FastaValidation {
            is_valid: false,
            sequence_count: 0,
            sequence_length: None,
            sample_headers: vec![],
            detected_alphabet: "unknown".to_string(),
            errors: vec![ValidationError {
                error_type: "file_not_found".to_string(),
                message: format!("File not found: {}", path),
                line_number: None,
            }],
            file_size_bytes: 0,
            file_modified_at: None,
        });
    }

    // Get file size and modification time
    let metadata = std::fs::metadata(file_path).map_err(|e| e.to_string())?;
    let file_size_bytes = metadata.len();
    let file_modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| {
            use chrono::{DateTime, Utc};
            let datetime: DateTime<Utc> = t.into();
            Some(datetime.to_rfc3339())
        });

    // Open file
    let file = File::open(file_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut headers: Vec<String> = Vec::new();
    let mut sequences: Vec<String> = Vec::new();
    let mut current_sequence = String::new();
    let mut _line_number = 0; // Tracked for potential future error context
    let mut errors: Vec<ValidationError> = Vec::new();

    for line_result in reader.lines() {
        _line_number += 1;
        let line = line_result.map_err(|e| e.to_string())?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('>') {
            // Save previous sequence if exists
            if !current_sequence.is_empty() {
                sequences.push(current_sequence.clone());
                current_sequence.clear();
            }
            headers.push(trimmed[1..].to_string());
        } else {
            current_sequence.push_str(trimmed);
        }

        // Limit scanning for very large files
        if sequences.len() >= 1000 {
            break;
        }
    }

    // Don't forget the last sequence
    if !current_sequence.is_empty() {
        sequences.push(current_sequence);
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

    // Sample sequences for validation (first 10 + 90 random)
    let mut sample_indices: Vec<usize> = (0..sequences.len().min(10)).collect();
    if sequences.len() > 10 {
        let remaining: Vec<usize> = (10..sequences.len()).collect();
        let mut rng = rand::thread_rng();
        let additional: Vec<usize> = remaining
            .choose_multiple(&mut rng, 90.min(remaining.len()))
            .cloned()
            .collect();
        sample_indices.extend(additional);
    }

    // Check sequence lengths (MSA validation)
    let first_length = sequences.first().map(|s| s.len());
    let mut length_mismatches: Vec<(usize, usize, usize)> = Vec::new();

    for &idx in &sample_indices {
        if let Some(seq) = sequences.get(idx) {
            if let Some(expected) = first_length {
                if seq.len() != expected {
                    length_mismatches.push((idx + 1, seq.len(), expected));
                }
            }
        }
    }

    // Add length mismatch errors
    for (seq_num, actual, expected) in length_mismatches.iter().take(5) {
        errors.push(ValidationError {
            error_type: "length_mismatch".to_string(),
            message: format!(
                "Sequence {} has length {}, expected {}",
                seq_num, actual, expected
            ),
            line_number: None,
        });
    }

    // Detect alphabet
    let detected_alphabet = detect_alphabet(&sequences);

    // Get sample headers (first 3)
    let sample_headers: Vec<String> = headers.iter().take(3).cloned().collect();

    // Estimate total sequence count from file size
    let avg_seq_length = first_length.unwrap_or(1000);
    let avg_header_length = 50;
    let estimated_total = if sequences.len() >= 1000 {
        // Extrapolate from file size
        let bytes_per_seq = avg_seq_length + avg_header_length + 2; // +2 for newlines
        (file_size_bytes as usize / bytes_per_seq).max(sequences.len())
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

/// Detect whether sequences are protein or nucleotide
fn detect_alphabet(sequences: &[String]) -> String {
    let nucleotides: HashSet<char> = "ACGTUN".chars().collect();
    let mut nucleotide_count = 0;
    let mut total_count = 0;

    for seq in sequences.iter().take(10) {
        for c in seq.to_uppercase().chars() {
            if c != '-' && c != '*' {
                total_count += 1;
                if nucleotides.contains(&c) {
                    nucleotide_count += 1;
                }
            }
        }
    }

    if total_count == 0 {
        return "unknown".to_string();
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

/// Detect header format from sample headers
#[tauri::command]
pub async fn detect_header_format(path: String) -> Result<HeaderFormatDetection, String> {
    // Read first few headers
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut headers: Vec<String> = Vec::new();
    for line_result in reader.lines() {
        let line = line_result.map_err(|e| e.to_string())?;
        let trimmed = line.trim();
        if trimmed.starts_with('>') {
            headers.push(trimmed[1..].to_string());
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
        if counts.iter().all(|&c| c == counts[0]) && counts[0] > 1 {
            if counts[0] > best_field_count {
                best_field_count = counts[0];
                best_delimiter = Some(delimiter);
            }
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

    // Suggest common field names
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
