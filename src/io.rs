use bio::io::fasta;
use hashbrown::HashMap;
use rayon::prelude::*;
use std::fs::{File, metadata};
use std::io::{self, Write, Cursor};
use memmap2::Mmap;

use crate::alphabet::{CharacterValidator, ValidationMode, AlphabetType, ValidationStats};
use crate::kmer::{sliding_window_validated, sliding_window_string_validated};
use crate::zero_copy::parse_header_zero_copy;
use crate::columnar::ColumnarMetadataAdapter;

// Re-export deprecated functions for backward compatibility
#[allow(deprecated)]
use crate::kmer::{sliding_window, sliding_window_encoded};


pub fn save_file(content: &str, path: &str) -> Result<(), io::Error> {
    if let Ok(mut f) = File::create(path) {
        if f.write_all(content.as_bytes()).is_ok() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Cannot write to file."))
        }
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, "Unable to create on disk."))
    }
}

/// Original scalar header parsing function (maintained for backward compatibility)
pub fn parse_header_scalar(
    header: &String,
    format: &Vec<String>,
    fill_na: &String,
) -> HashMap<String, String> {
    let metadata = header
        .split("|")
        .map(|component| {
            return if !component.is_empty() {
                component.trim()
            } else {
                if !fill_na.is_empty() {
                    return fill_na.as_str();
                }
                component
            };
        })
        .collect::<Vec<&str>>();

    assert_eq!(
        metadata.iter().filter(|item| item.len() == 0).count(),
        0,
        "\n\nThe FASTA header looks invalid:\n\tFormat: {}\n\tHeader: {}\n\n",
        format.join("|"),
        header
    );

    assert_eq!(
        metadata.len(),
        format.len(),
        "\n\nThe header format provided does not match the header:\n\tFormat: {}\n\tHeader: {}\n\n",
        format.join("|"),
        header
    );

    format
        .iter()
        .enumerate()
        .map(|(idx, item)| (item.to_string(), metadata[idx].to_owned()))
        .collect::<HashMap<String, String>>()
}

/// Production-grade zero-copy header parsing with SIMD acceleration
pub fn parse_header(
    header: &String,
    format: &Vec<String>,
    fill_na: &String,
) -> HashMap<String, String> {
    parse_header_zero_copy(header, format, fill_na)
}

/// Configuration for k-mer extraction with validation options
#[derive(Debug, Clone)]
pub struct KmerExtractionConfig {
    /// Validation mode (strict, permissive, report-only)
    pub validation_mode: ValidationMode,
    /// Allow lowercase characters (auto-converted to uppercase)
    pub allow_lowercase: bool,
    /// Report invalid characters found during processing
    pub report_invalid: bool,
}

impl Default for KmerExtractionConfig {
    fn default() -> Self {
        Self {
            validation_mode: ValidationMode::Strict,
            allow_lowercase: false,
            report_invalid: false,
        }
    }
}

impl KmerExtractionConfig {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_validation_mode(mut self, mode: ValidationMode) -> Self {
        self.validation_mode = mode;
        self
    }
    
    pub fn with_allow_lowercase(mut self, allow: bool) -> Self {
        self.allow_lowercase = allow;
        self
    }
    
    pub fn with_report_invalid(mut self, report: bool) -> Self {
        self.report_invalid = report;
        self
    }
}

/// New validated version of get_kmers_and_headers_encoded using CharacterValidator
/// 
/// This is the recommended function for k-mer extraction. It uses a whitelist-based
/// character validation approach that rejects any character not in the valid 
/// biological alphabet (20 amino acids or 4/5 nucleotides).
/// 
/// # Arguments
/// * `path` - Path to the FASTA file
/// * `kmer_length` - Length of k-mers to generate
/// * `header_format` - Optional header format for metadata extraction
/// * `header_fillna` - Value to use for missing header fields
/// * `alphabet` - "protein" or "nucleotide" (defaults to "protein")
/// * `config` - Optional KmerExtractionConfig for validation options
/// * `expected_count` - Optional expected sequence count for progress bar
pub fn get_kmers_and_headers_validated(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    config: Option<KmerExtractionConfig>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<u64>>,                                    // transposed encoded kmers
    Option<Vec<Option<HashMap<String, String>>>>,    // headers
    usize,                                           // sequence count
    bool,                                            // is_protein flag
    Option<ValidationStats>,                         // validation statistics (if reporting enabled)
) {
    let config = config.unwrap_or_default();
    
    // Create validator based on alphabet and config
    let is_protein = alphabet.map(|s| s == "protein").unwrap_or(true);
    let alphabet_type = if is_protein { AlphabetType::Protein } else { AlphabetType::Nucleotide };
    
    let validator = CharacterValidator::with_options(
        alphabet_type,
        config.validation_mode,
        config.allow_lowercase,
    );
    
    let stats = if config.report_invalid {
        Some(ValidationStats::new())
    } else {
        None
    };

    // Determine optimal I/O strategy
    let use_mmap = should_use_memory_mapping(path);
    
    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let pb = if show_progress {
        match expected_count {
            Some(len) => {
                let pb = indicatif::ProgressBar::new(len as u64);
                let template = if use_mmap {
                    "[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA (Memory-Mapped, Validated)"
                } else {
                    "[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA (Buffered I/O, Validated)"
                };
                pb.set_style(indicatif::ProgressStyle::with_template(template)
                    .unwrap()
                    .progress_chars("##-"));
                Some(pb)
            }
            None => {
                let pb = indicatif::ProgressBar::new_spinner();
                let message = if use_mmap {
                    "Reading FASTA (Memory-Mapped, Validated)..."
                } else {
                    "Reading FASTA (Buffered I/O, Validated)..."
                };
                pb.set_message(message);
                pb.enable_steady_tick(std::time::Duration::from_millis(1));
                Some(pb)
            }
        }
    } else { None };

    let (transposed_kmers, headers_vec, sequence_count) = if use_mmap {
        match try_mmap_processing_validated(
            path,
            kmer_length,
            &validator,
            header_format,
            header_fillna,
            &pb,
        ) {
            Ok(result) => result,
            Err(_) => {
                if let Some(ref pb) = pb {
                    pb.set_message("Memory mapping failed, using buffered I/O...");
                }
                process_with_buffered_io_validated(
                    path,
                    kmer_length,
                    &validator,
                    header_format,
                    header_fillna,
                    &pb,
                )
            }
        }
    } else {
        process_with_buffered_io_validated(
            path,
            kmer_length,
            &validator,
            header_format,
            header_fillna,
            &pb,
        )
    };

    if let Some(pb) = pb { pb.finish_and_clear(); }

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    (transposed_kmers, headers, sequence_count, is_protein, stats)
}

/// Memory-mapped FASTA processing with CharacterValidator
fn try_mmap_processing_validated(
    path: &String,
    kmer_length: &usize,
    validator: &CharacterValidator,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    pb: &Option<indicatif::ProgressBar>,
) -> io::Result<(Vec<Vec<u64>>, Vec<Option<HashMap<String, String>>>, usize)> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    
    let mut transposed_kmers: Vec<Vec<u64>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let cursor = Cursor::new(&mmap[..]);
    let reader = fasta::Reader::new(cursor);

    for record_result in reader.records() {
        let record = record_result?;
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let sequence_bytes = record.seq();
        let encoded_kmers = sliding_window_validated(
            sequence_bytes,
            *kmer_length,
            validator,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); encoded_kmers.len()];
        }

        for (i, encoded_kmer) in encoded_kmers.into_iter().enumerate() {
            if let Some(kmer) = encoded_kmer {
                transposed_kmers[i].push(kmer);
            }
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record.desc() {
                [record.id(), desc].join(" ")
            } else {
                record.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    Ok((transposed_kmers, headers_vec, sequence_count))
}

/// Buffered I/O FASTA processing with CharacterValidator
fn process_with_buffered_io_validated(
    path: &String,
    kmer_length: &usize,
    validator: &CharacterValidator,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    pb: &Option<indicatif::ProgressBar>,
) -> (Vec<Vec<u64>>, Vec<Option<HashMap<String, String>>>, usize) {
    let mut transposed_kmers: Vec<Vec<u64>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let file = File::open(path).expect("Failed to read FASTA file");
    let reader = fasta::Reader::with_capacity(64 * 1024, file);

    for record in reader.records() {
        let record_unwrapped = record.as_ref().unwrap();
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let sequence_bytes = record_unwrapped.seq();
        let encoded_kmers = sliding_window_validated(
            sequence_bytes,
            *kmer_length,
            validator,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); encoded_kmers.len()];
        }

        for (i, encoded_kmer) in encoded_kmers.into_iter().enumerate() {
            if let Some(kmer) = encoded_kmer {
                transposed_kmers[i].push(kmer);
            }
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record_unwrapped.desc() {
                [record_unwrapped.id(), desc].join(" ")
            } else {
                record_unwrapped.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    (transposed_kmers, headers_vec, sequence_count)
}

/// Columnar metadata version with CharacterValidator
pub fn get_kmers_and_headers_encoded_columnar_validated(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    config: Option<KmerExtractionConfig>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<u64>>,
    Option<ColumnarMetadataAdapter>,
    usize,
    bool,
    Option<ValidationStats>,
) {
    let (kmers, row_headers, sequence_count, is_protein, stats) = get_kmers_and_headers_validated(
        path, kmer_length, header_format, header_fillna, alphabet, config, expected_count
    );
    
    let columnar_headers = if let (Some(headers), Some(format)) = (row_headers, header_format) {
        let adapter = ColumnarMetadataAdapter::from_row_metadata_with_indexing(format.clone(), headers);
        Some(adapter)
    } else {
        None
    };
    
    (kmers, columnar_headers, sequence_count, is_protein, stats)
}

// ============================================================================
// Legacy functions for backward compatibility
// These use the old blacklist approach but are maintained for existing code
// ============================================================================

/// Columnar metadata version of get_kmers_and_headers_encoded (legacy)
#[deprecated(
    since = "2.0.0",
    note = "Use get_kmers_and_headers_encoded_columnar_validated for robust whitelist-based validation"
)]
pub fn get_kmers_and_headers_encoded_columnar(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<u64>>,
    Option<ColumnarMetadataAdapter>,
    usize,
    bool,
) {
    #[allow(deprecated)]
    let (kmers, row_headers, sequence_count, is_protein) = get_kmers_and_headers_encoded(
        path, kmer_length, header_format, header_fillna, alphabet, expected_count
    );
    
    let columnar_headers = if let (Some(headers), Some(format)) = (row_headers, header_format) {
        let adapter = ColumnarMetadataAdapter::from_row_metadata_with_indexing(format.clone(), headers);
        Some(adapter)
    } else {
        None
    };
    
    (kmers, columnar_headers, sequence_count, is_protein)
}

/// Legacy get_kmers_and_headers_encoded with blacklist approach
/// 
/// DEPRECATED: This function uses a blacklist approach which may allow invalid
/// characters like #, *, @, etc. to pass through. Use get_kmers_and_headers_validated instead.
#[deprecated(
    since = "2.0.0",
    note = "Use get_kmers_and_headers_validated for robust whitelist-based validation"
)]
pub fn get_kmers_and_headers_encoded(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<u64>>,
    Option<Vec<Option<HashMap<String, String>>>>,
    usize,
    bool,
) {
    let protein_ambiguous_chars = vec![b'-', b'X', b'B', b'J', b'Z', b'O', b'U'];
    let nucleotide_ambiguous_chars = vec![b'-', b'R', b'Y', b'K', b'M', b'S', b'W', b'B', b'D', b'H', b'V', b'N'];

    let is_protein = if let Some(residue_alphabet) = alphabet {
        residue_alphabet == "protein"
    } else {
        true
    };

    let illegal_chars = if is_protein { protein_ambiguous_chars } else { nucleotide_ambiguous_chars };

    let use_mmap = should_use_memory_mapping(path);
    
    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let pb = if show_progress {
        match expected_count {
            Some(len) => {
                let pb = indicatif::ProgressBar::new(len as u64);
                let template = if use_mmap {
                    "[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA (Memory-Mapped)"
                } else {
                    "[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA (Buffered I/O)"
                };
                pb.set_style(indicatif::ProgressStyle::with_template(template)
                    .unwrap()
                    .progress_chars("##-"));
                Some(pb)
            }
            None => {
                let pb = indicatif::ProgressBar::new_spinner();
                let message = if use_mmap {
                    "Reading FASTA (Memory-Mapped)..."
                } else {
                    "Reading FASTA (Buffered I/O)..."
                };
                pb.set_message(message);
                pb.enable_steady_tick(std::time::Duration::from_millis(1));
                Some(pb)
            }
        }
    } else { None };

    #[allow(deprecated)]
    let (transposed_kmers, headers_vec, sequence_count) = if use_mmap {
        match try_mmap_processing_legacy(
            path,
            kmer_length,
            &illegal_chars,
            is_protein,
            header_format,
            header_fillna,
            &pb,
        ) {
            Ok(result) => result,
            Err(_) => {
                if let Some(ref pb) = pb {
                    pb.set_message("Memory mapping failed, using buffered I/O...");
                }
                process_with_buffered_io_legacy(
                    path,
                    kmer_length,
                    &illegal_chars,
                    is_protein,
                    header_format,
                    header_fillna,
                    &pb,
                )
            }
        }
    } else {
        process_with_buffered_io_legacy(
            path,
            kmer_length,
            &illegal_chars,
            is_protein,
            header_format,
            header_fillna,
            &pb,
        )
    };

    if let Some(pb) = pb { pb.finish_and_clear(); }

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    (transposed_kmers, headers, sequence_count, is_protein)
}

// Intelligent decision making for I/O strategy
fn should_use_memory_mapping(path: &String) -> bool {
    let file_size = match metadata(path) {
        Ok(meta) => meta.len(),
        Err(_) => return false,
    };

    const SMALL_FILE_THRESHOLD: u64 = 10 * 1024 * 1024;
    const LARGE_FILE_THRESHOLD: u64 = 100 * 1024 * 1024;

    match file_size {
        size if size < SMALL_FILE_THRESHOLD => false,
        size if size > LARGE_FILE_THRESHOLD => true,
        _ => get_available_memory_gb() > 4.0
    }
}

fn get_available_memory_gb() -> f64 {
    match std::env::var("DIMA_FORCE_MMAP") {
        Ok(val) if val == "1" => 999.0,
        Ok(val) if val == "0" => 0.0,
        _ => 8.0,
    }
}

// Legacy memory-mapped processing
#[allow(deprecated)]
fn try_mmap_processing_legacy(
    path: &String,
    kmer_length: &usize,
    illegal_chars: &[u8],
    is_protein: bool,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    pb: &Option<indicatif::ProgressBar>,
) -> io::Result<(Vec<Vec<u64>>, Vec<Option<HashMap<String, String>>>, usize)> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    
    let mut transposed_kmers: Vec<Vec<u64>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let cursor = Cursor::new(&mmap[..]);
    let reader = fasta::Reader::new(cursor);

    for record_result in reader.records() {
        let record = record_result?;
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let sequence_bytes = record.seq();
        #[allow(deprecated)]
        let encoded_kmers = sliding_window_encoded(
            sequence_bytes,
            *kmer_length,
            is_protein,
            illegal_chars,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); encoded_kmers.len()];
        }

        for (i, encoded_kmer) in encoded_kmers.into_iter().enumerate() {
            if let Some(kmer) = encoded_kmer {
                transposed_kmers[i].push(kmer);
            }
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record.desc() {
                [record.id(), desc].join(" ")
            } else {
                record.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    Ok((transposed_kmers, headers_vec, sequence_count))
}

// Legacy buffered I/O processing
#[allow(deprecated)]
fn process_with_buffered_io_legacy(
    path: &String,
    kmer_length: &usize,
    illegal_chars: &[u8],
    is_protein: bool,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    pb: &Option<indicatif::ProgressBar>,
) -> (Vec<Vec<u64>>, Vec<Option<HashMap<String, String>>>, usize) {
    let mut transposed_kmers: Vec<Vec<u64>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let file = File::open(path).expect("Failed to read FASTA file");
    let reader = fasta::Reader::with_capacity(64 * 1024, file);

    for record in reader.records() {
        let record_unwrapped = record.as_ref().unwrap();
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let sequence_bytes = record_unwrapped.seq();
        #[allow(deprecated)]
        let encoded_kmers = sliding_window_encoded(
            sequence_bytes,
            *kmer_length,
            is_protein,
            illegal_chars,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); encoded_kmers.len()];
        }

        for (i, encoded_kmer) in encoded_kmers.into_iter().enumerate() {
            if let Some(kmer) = encoded_kmer {
                transposed_kmers[i].push(kmer);
            }
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record_unwrapped.desc() {
                [record_unwrapped.id(), desc].join(" ")
            } else {
                record_unwrapped.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    (transposed_kmers, headers_vec, sequence_count)
}

/// Legacy string-based function for backward compatibility
#[deprecated(
    since = "2.0.0",
    note = "Use get_kmers_and_headers_validated for robust whitelist-based validation"
)]
pub fn get_kmers_and_headers(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<String>>,
    Option<Vec<Option<HashMap<String, String>>>>,
    usize,
) {
    let protein_ambiguous_chars = vec!['-', 'X', 'B', 'J', 'Z', 'O', 'U'];
    let nucleotide_ambiguous_chars = vec!['-', 'R', 'Y', 'K', 'M', 'S', 'W', 'B', 'D', 'H', 'V', 'N'];

    let illegal_chars = if let Some(residue_alphabet) = alphabet {
        if residue_alphabet == "protein" { protein_ambiguous_chars } else { nucleotide_ambiguous_chars }
    } else { protein_ambiguous_chars };

    let mut transposed_kmers: Vec<Vec<String>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let pb = if show_progress {
        match expected_count {
            Some(len) => {
                let pb = indicatif::ProgressBar::new(len as u64);
                pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA")
                    .unwrap()
                    .progress_chars("##-"));
                Some(pb)
            }
            None => {
                let pb = indicatif::ProgressBar::new_spinner();
                pb.set_message("Reading FASTA...");
                pb.enable_steady_tick(std::time::Duration::from_millis(1));
                Some(pb)
            }
        }
    } else { None };

    let file = File::open(path).expect("Failed to read FASTA file");
    let reader = fasta::Reader::with_capacity(64 * 1024, file);

    for record in reader.records() {
        let record_unwrapped = record.as_ref().unwrap();
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        #[allow(deprecated)]
        let kmers = sliding_window(
            &String::from_utf8(Vec::from(record_unwrapped.seq())).unwrap(),
            &kmer_length,
            &illegal_chars,
        );

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); kmers.len()];
        }

        for (i, k) in kmers.into_iter().enumerate() {
            transposed_kmers[i].push(k);
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record_unwrapped.desc() {
                [record_unwrapped.id(), desc].join(" ")
            } else {
                record_unwrapped.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    if let Some(pb) = pb { pb.finish_and_clear(); }

    transposed_kmers
        .par_iter_mut()
        .for_each(|kmer_position| kmer_position.retain(|kmer| kmer != "NA"));

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    (transposed_kmers, headers, sequence_count)
}

/// New string-based function with CharacterValidator
pub fn get_kmers_and_headers_string_validated(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    config: Option<KmerExtractionConfig>,
    expected_count: Option<usize>,
) -> (
    Vec<Vec<String>>,
    Option<Vec<Option<HashMap<String, String>>>>,
    usize,
) {
    let config = config.unwrap_or_default();
    
    let is_protein = alphabet.map(|s| s == "protein").unwrap_or(true);
    let alphabet_type = if is_protein { AlphabetType::Protein } else { AlphabetType::Nucleotide };
    
    let validator = CharacterValidator::with_options(
        alphabet_type,
        config.validation_mode,
        config.allow_lowercase,
    );

    let mut transposed_kmers: Vec<Vec<String>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;

    let show_progress = std::env::var("PROGRESS").ok().as_deref() != Some("0");
    let pb = if show_progress {
        match expected_count {
            Some(len) => {
                let pb = indicatif::ProgressBar::new(len as u64);
                pb.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] {bar:40.magenta/blue} {pos}/{len} Reading FASTA (Validated)")
                    .unwrap()
                    .progress_chars("##-"));
                Some(pb)
            }
            None => {
                let pb = indicatif::ProgressBar::new_spinner();
                pb.set_message("Reading FASTA (Validated)...");
                pb.enable_steady_tick(std::time::Duration::from_millis(1));
                Some(pb)
            }
        }
    } else { None };

    let file = File::open(path).expect("Failed to read FASTA file");
    let reader = fasta::Reader::with_capacity(64 * 1024, file);

    for record in reader.records() {
        let record_unwrapped = record.as_ref().unwrap();
        sequence_count += 1;
        if let Some(ref pb) = pb { pb.inc(1); }

        let sequence = String::from_utf8(Vec::from(record_unwrapped.seq())).unwrap();
        let kmers = sliding_window_string_validated(&sequence, *kmer_length, &validator);

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(1024); kmers.len()];
        }

        for (i, k) in kmers.into_iter().enumerate() {
            transposed_kmers[i].push(k);
        }

        if let Some(headers_components) = header_format {
            let fixed_header: String = if let Some(desc) = record_unwrapped.desc() {
                [record_unwrapped.id(), desc].join(" ")
            } else {
                record_unwrapped.id().to_string()
            };

            if let Some(fill_na) = header_fillna {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, fill_na)));
            } else {
                headers_vec.push(Some(parse_header(&fixed_header, headers_components, &"Unknown".to_string())));
            }
        }
    }

    if let Some(pb) = pb { pb.finish_and_clear(); }

    // Filter out "NA" k-mers
    transposed_kmers
        .par_iter_mut()
        .for_each(|kmer_position| kmer_position.retain(|kmer| kmer != "NA"));

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    (transposed_kmers, headers, sequence_count)
}
