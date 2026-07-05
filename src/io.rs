use needletail::parse_fastx_reader;
use hashbrown::HashMap;
use std::fs::{File, metadata};
use std::io::{self, Write, Cursor, Read as IoRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use memmap2::Mmap;
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::alphabet::{CharacterValidator, ValidationMode, AlphabetType, ValidationStats};
use crate::kmer::{sliding_window_validated_with_stats, sliding_window_string_validated};
use crate::zero_copy::parse_header_zero_copy;
use crate::columnar::ColumnarMetadataAdapter;

// ─── InputSource Enum ────────────────────────────────────────────────────────
//
// Closed set of input sources. Enum for zero-cost static dispatch
// (per uutils/coreutils pattern, avoiding Box<dyn Read> overhead).

/// Represents the source of input data for analysis.
/// Uses an enum (closed set) instead of trait objects for zero-cost dispatch.
#[derive(Debug, Clone)]
pub enum InputSource {
    /// File path - enables mmap, file size estimation, compression detection
    File(PathBuf),
    /// Standard input - streaming only, no mmap, no size estimate
    Stdin,
}

impl InputSource {
    /// Whether this source supports memory mapping (only files on local filesystems)
    pub fn supports_mmap(&self) -> bool {
        matches!(self, InputSource::File(_))
    }

    /// File size in bytes (None for stdin or if stat fails)
    pub fn file_size(&self) -> Option<u64> {
        match self {
            InputSource::File(p) => std::fs::metadata(p).ok().map(|m| m.len()),
            InputSource::Stdin => None,
        }
    }

    /// Display-friendly name for error messages and progress reporting
    pub fn display_name(&self) -> String {
        match self {
            InputSource::File(p) => p.display().to_string(),
            InputSource::Stdin => "<stdin>".to_string(),
        }
    }

    /// Returns the path if this is a file source, None otherwise
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            InputSource::File(p) => Some(p.as_path()),
            InputSource::Stdin => None,
        }
    }
}

/// Detect compression by reading magic bytes from file header.
/// Returns true if the file uses a supported compression format (gz/bz2/xz/zst).
/// needletail handles transparent decompression for all of these.
pub fn is_compressed(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else { return false };
    let mut magic = [0u8; 4];
    let Ok(n) = file.read(&mut magic) else { return false };
    if n < 2 { return false; }
    matches!(
        &magic[..2],
        [0x1f, 0x8b]  // gzip
        | [0x42, 0x5a] // bzip2
    ) || (n >= 4 && matches!(
        &magic[..4],
        [0xfd, 0x37, 0x7a, 0x58] // xz
        | [0x28, 0xb5, 0x2f, 0xfd] // zstd
    ))
}

/// Metadata vector: per-sequence parsed header metadata (None if headers couldn't be parsed).
pub type SequenceMetadata = Vec<Option<HashMap<String, String>>>;

/// Result of k-mer extraction: (encoded_kmers, optional_metadata, sequence_count, is_protein, validation_stats, diagnostics).
pub type KmerExtractionResult = (Vec<Vec<u64>>, Option<SequenceMetadata>, usize, bool, Option<ValidationStats>, ParseDiagnostics);

/// Result of legacy extraction: (encoded_kmers, metadata, sequence_count).
pub type LegacyExtractionResult = (Vec<Vec<u64>>, SequenceMetadata, usize);

/// Result of columnar extraction: (encoded_kmers, columnar_adapter, sequence_count, is_protein, validation_stats, diagnostics).
pub type ColumnarExtractionResult = (Vec<Vec<u64>>, Option<ColumnarMetadataAdapter>, usize, bool, Option<ValidationStats>, ParseDiagnostics);

/// Result of string k-mer extraction: (string_kmers, optional_metadata, sequence_count).
pub type StringKmerResult = (Vec<Vec<String>>, Option<SequenceMetadata>, usize);

/// UTF-8 BOM bytes (EF BB BF). Some text editors (especially Windows Notepad)
/// prepend this to UTF-8 files. bio-rs FASTA parser doesn't handle it, causing
/// the first record header to be unparseable.
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Maximum allowed sequence length per record (100 MB). Sequences longer than
/// this are almost certainly not valid MSA input — they'd be whole genomes or
/// corrupt data. Rejecting them early prevents multi-GB memory allocation that
/// would OOM the process.
const MAX_SEQUENCE_LENGTH: usize = 100 * 1024 * 1024;

/// Maximum number of sequences before OOM protection triggers.
/// 10M sequences × 10K positions × 8 bytes = 800 GB — well beyond any
/// reasonable workstation. This prevents unbounded allocation from
/// malicious or accidentally large FASTA files (CWE-400).
const MAX_SEQUENCE_COUNT: usize = 10_000_000;

/// Maximum allowed FASTA header length in bytes.
/// Real-world headers are typically under 200 bytes (UniProt, NCBI, etc.).
/// 10 KB is generous for any legitimate format while preventing OOM from
/// crafted FASTA files with multi-MB headers per record (CWE-400).
const MAX_HEADER_LENGTH: usize = 10 * 1024;

/// Strip UTF-8 BOM from the start of a byte slice if present.
#[inline]
fn strip_bom(data: &[u8]) -> &[u8] {
    if data.starts_with(UTF8_BOM) {
        &data[3..]
    } else {
        data
    }
}


/// Write content to a file, propagating the real OS error on failure.
///
/// Uses atomic write semantics: writes to a temporary file and renames
/// to the final path, ensuring no partial writes on crash/interruption.
pub fn save_file(content: &str, path: &str) -> Result<(), io::Error> {
    use std::path::Path;
    atomic_write(Path::new(path), |writer| {
        writer.write_all(content.as_bytes())
    })
}

/// Atomic file write: writes via a `tempfile::NamedTempFile` in the same directory,
/// then persists (renames) to the final path.
///
/// Guarantees:
/// - No partial writes visible at `path` on crash
/// - Data is fsync'd before rename (durable on power loss)
/// - Temp file auto-cleaned on drop if write fails (no orphans)
/// - No collision risk between concurrent writes (random temp name)
/// - Works cross-platform (tempfile handles Windows rename semantics)
pub fn atomic_write(
    path: &std::path::Path,
    write_fn: impl FnOnce(&mut std::io::BufWriter<&File>) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(parent)?;
    let mut writer = std::io::BufWriter::new(tmp.as_file());

    write_fn(&mut writer)?;

    writer.flush()?;
    writer.into_inner()
        .map_err(|e| io::Error::other(e.to_string()))?
        .sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Per-run diagnostics collected during FASTA processing.
/// Replaces the former global `AtomicUsize` counter, making analysis
/// runs fully independent and testable in parallel.
#[derive(Debug, Clone, Default)]
pub struct ParseDiagnostics {
    pub header_parse_failures: usize,
    pub skipped_records: usize,
}

impl ParseDiagnostics {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Scalar header parsing function with error counting.
/// Returns parsed metadata on success, or empty HashMap on malformed headers.
/// Increments the provided diagnostics counter (no global state).
pub fn parse_header(
    header: &str,
    format: &[String],
    fill_na: &str,
    diagnostics: &mut ParseDiagnostics,
) -> HashMap<String, String> {
    match parse_header_zero_copy(header, format, fill_na) {
        Ok(result) => result,
        Err(e) => {
            if diagnostics.header_parse_failures < 5 {
                tracing::warn!(error = e.trim(), "skipping malformed header");
            }
            diagnostics.header_parse_failures += 1;
            HashMap::new()
        }
    }
}

/// Internal header parsing with per-run diagnostics tracking.
/// Increments `diagnostics.header_parse_failures` on malformed headers
/// and emits a warning for the first 5 occurrences.
fn parse_header_internal(
    header: &str,
    format: &[String],
    fill_na: &str,
    diagnostics: &mut ParseDiagnostics,
) -> HashMap<String, String> {
    match parse_header_zero_copy(header, format, fill_na) {
        Ok(result) => result,
        Err(e) => {
            if diagnostics.header_parse_failures < 5 {
                tracing::warn!(error = e.trim(), "skipping malformed header");
            }
            diagnostics.header_parse_failures += 1;
            HashMap::new()
        }
    }
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
    /// Cooperative cancellation token (checked every N sequences during I/O)
    pub cancel_token: Option<Arc<AtomicBool>>,
    /// Hint for expected number of sequences (used to pre-allocate k-mer matrix columns,
    /// reducing reallocations during the transpose-on-build phase)
    pub expected_sequence_count: Option<usize>,
}

impl Default for KmerExtractionConfig {
    fn default() -> Self {
        Self {
            validation_mode: ValidationMode::default(),
            allow_lowercase: false,
            report_invalid: false,
            cancel_token: None,
            expected_sequence_count: None,
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

    pub fn with_cancel_token(mut self, token: Arc<AtomicBool>) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// Check if cancellation has been requested.
    fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .is_some_and(|t| t.load(Ordering::Relaxed))
    }
}

/// Extract k-mers and headers from a FASTA file with whitelist-based validation
/// 
/// This function uses a whitelist-based character validation approach that rejects 
/// any character not in the valid biological alphabet (20 amino acids or 5 nucleotides).
/// 
/// # Arguments
/// * `path` - Path to the FASTA file
/// * `kmer_length` - Length of k-mers to generate
/// * `header_format` - Optional header format for metadata extraction
/// * `header_fillna` - Value to use for missing header fields
/// * `alphabet` - "protein" or "nucleotide" (defaults to "protein")
/// * `config` - Optional KmerExtractionConfig for validation options
/// * `expected_count` - Optional expected sequence count for progress bar
/// 
/// # Returns
/// * `Vec<Vec<u64>>` - Transposed encoded k-mers (position-oriented)
/// * `Option<Vec<Option<HashMap<String, String>>>>` - Headers (if format provided)
/// * `usize` - Sequence count
/// * `bool` - Is protein flag
/// * `Option<ValidationStats>` - Validation statistics (if reporting enabled)
pub fn get_kmers_and_headers_validated(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    config: Option<KmerExtractionConfig>,
    expected_count: Option<usize>,
) -> Result<KmerExtractionResult, std::io::Error> {
    let mut config = config.unwrap_or_default();
    // Propagate expected_count hint into config for inner functions to use
    if config.expected_sequence_count.is_none() {
        config.expected_sequence_count = expected_count;
    }
    
    let alphabet_type = AlphabetType::from_optional_str(alphabet.map(|s| s.as_str()));
    let is_protein = alphabet_type == AlphabetType::Protein;

    // Library-level precondition: k-mer length must be encodable in u64.
    // Without this check, all k-mers silently encode to None (overflow via
    // checked_mul), producing all-NS results with no error message.
    let max_k = crate::kmer::max_kmer_length(is_protein);
    if *kmer_length == 0 || *kmer_length > max_k {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "k-mer length {} is invalid for {} alphabet (valid range: 1..={})",
                kmer_length, if is_protein { "protein" } else { "nucleotide" }, max_k
            ),
        ));
    }
    
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

    let mut diagnostics = ParseDiagnostics::new();

    let (transposed_kmers, headers_vec, sequence_count) = if use_mmap {
        match try_mmap_processing(
            path,
            kmer_length,
            &validator,
            header_format,
            header_fillna,
            stats.as_ref(),
            &config,
            &mut diagnostics,
        ) {
            Ok(result) => result,
            Err(e) if is_retriable_io_error(&e) => {
                // Only fallback for actual mmap/open failures, NOT validation errors.
                // Validation errors (InvalidData) would just re-fail after re-reading.
                tracing::warn!("Memory mapping failed ({}), falling back to buffered I/O", e);
                process_with_buffered_io(
                    path,
                    kmer_length,
                    &validator,
                    header_format,
                    header_fillna,
                    stats.as_ref(),
                    &config,
                    &mut diagnostics,
                )?
            }
            Err(e) => return Err(e), // Validation errors propagate immediately
        }
    } else {
        process_with_buffered_io(
            path,
            kmer_length,
            &validator,
            header_format,
            header_fillna,
            stats.as_ref(),
            &config,
            &mut diagnostics,
        )?
    };

    // Validate header format match rate: if a header format was specified but
    // >5% of headers produced empty metadata, the format likely doesn't match
    // the actual header structure — fail with an actionable error message.
    if header_format.is_some() && sequence_count > 0 {
        let empty_count = headers_vec.iter()
            .filter(|h| h.as_ref().map_or(true, |m| m.is_empty()))
            .count();
        let mismatch_rate = empty_count as f64 / sequence_count as f64;
        if mismatch_rate > 0.05 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Header format mismatch: {}/{} sequences ({:.1}%) produced no metadata. \
                     The declared header format likely doesn't match the FASTA headers. \
                     Expected pipe-delimited fields matching: {:?}",
                    empty_count,
                    sequence_count,
                    mismatch_rate * 100.0,
                    header_format.unwrap(),
                ),
            ));
        } else if empty_count > 0 {
            tracing::warn!(
                mismatched = empty_count,
                total = sequence_count,
                "some headers did not match declared format (below 5% threshold, proceeding)"
            );
        }
    }

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    Ok((transposed_kmers, headers, sequence_count, is_protein, stats, diagnostics))
}

/// Core record-processing loop shared by all I/O paths (mmap, buffered, stdin).
/// Encapsulates: header parsing, k-mer extraction, MSA validation, cancellation
/// checks, and progress reporting. Only the I/O source setup differs between callers.
#[allow(clippy::too_many_arguments)]
pub fn process_fasta_records(
    reader: &mut (dyn needletail::FastxReader + '_),
    kmer_length: usize,
    validator: &CharacterValidator,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    stats: Option<&ValidationStats>,
    config: &KmerExtractionConfig,
    diagnostics: &mut ParseDiagnostics,
) -> io::Result<LegacyExtractionResult> {
    let mut transposed_kmers: Vec<Vec<u64>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;
    let mut expected_kmer_count: Option<usize> = None;

    while let Some(record_result) = reader.next() {
        let record = match record_result {
            Ok(r) => r,
            Err(e) => {
                diagnostics.skipped_records += 1;
                tracing::warn!(after_seq = sequence_count, error = %e, "skipping malformed FASTA record");
                continue;
            }
        };
        sequence_count += 1;
        // Increment the parent span's progress bar (no-op without IndicatifLayer)
        tracing::Span::current().pb_inc(1);

        if sequence_count % 1000 == 0 && config.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "cancelled during FASTA reading",
            ));
        }

        if sequence_count > MAX_SEQUENCE_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Sequence count ({}) exceeds maximum ({}). \
                     This is likely not a valid MSA or is too large for available memory.",
                    sequence_count, MAX_SEQUENCE_COUNT,
                ),
            ));
        }

        let raw_id = record.id();
        if raw_id.len() > MAX_HEADER_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Sequence {} has a header of {} bytes — exceeds maximum ({} KB). \
                     This is likely malformed or adversarial input.",
                    sequence_count, raw_id.len(), MAX_HEADER_LENGTH / 1024,
                ),
            ));
        }
        let sequence_bytes = record.raw_seq();
        let record_id = String::from_utf8_lossy(raw_id);
        if sequence_bytes.len() > MAX_SEQUENCE_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Sequence {} ('{}') is {} bytes — exceeds maximum ({} MB). Not valid MSA input.",
                    sequence_count, record_id, sequence_bytes.len(), MAX_SEQUENCE_LENGTH / (1024 * 1024),
                ),
            ));
        }
        let encoded_kmers = sliding_window_validated_with_stats(
            sequence_bytes,
            kmer_length,
            validator,
            stats,
        );

        match expected_kmer_count {
            None => {
                expected_kmer_count = Some(encoded_kmers.len());
                let capacity = config.expected_sequence_count.unwrap_or(1024);
                transposed_kmers = vec![Vec::with_capacity(capacity); encoded_kmers.len()];
            }
            Some(expected) if encoded_kmers.len() != expected => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "MSA validation failed: sequence {} ('{}') produces {} k-mer positions \
                         but expected {} (defined by sequence 1). All sequences in a Multiple \
                         Sequence Alignment must have equal length. Ensure your input file is \
                         a properly aligned MSA.",
                        sequence_count, record_id, encoded_kmers.len(), expected,
                    ),
                ));
            }
            _ => {}
        }

        for (i, encoded_kmer) in encoded_kmers.into_iter().enumerate() {
            transposed_kmers[i].push(encoded_kmer.unwrap_or(u64::MAX));
        }

        if let Some(headers_components) = header_format {
            let fixed_header = String::from_utf8_lossy(raw_id).to_string();
            let fill = header_fillna.map(|s| s.as_str()).unwrap_or("Unknown");
            headers_vec.push(Some(parse_header_internal(&fixed_header, headers_components, fill, diagnostics)));
        }
    }

    Ok((transposed_kmers, headers_vec, sequence_count))
}

/// Memory-mapped FASTA processing with CharacterValidator.
/// Uses needletail's zero-copy parser over the mmap'd byte slice for maximum throughput.
#[allow(clippy::too_many_arguments)]
fn try_mmap_processing(
    path: &String,
    kmer_length: &usize,
    validator: &CharacterValidator,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    stats: Option<&ValidationStats>,
    config: &KmerExtractionConfig,
    diagnostics: &mut ParseDiagnostics,
) -> io::Result<LegacyExtractionResult> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    let data = strip_bom(&mmap[..]);
    let cursor = Cursor::new(data);
    let mut reader = parse_fastx_reader(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("FASTA parse error: {}", e)))?;

    process_fasta_records(
        reader.as_mut(), *kmer_length, validator, header_format, header_fillna,
        stats, config, diagnostics,
    )
}

/// Buffered I/O FASTA processing with CharacterValidator.
/// Falls back to this when mmap is unavailable (stdin, NFS, FUSE, etc.).
#[allow(clippy::too_many_arguments)]
fn process_with_buffered_io(
    path: &String,
    kmer_length: &usize,
    validator: &CharacterValidator,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    stats: Option<&ValidationStats>,
    config: &KmerExtractionConfig,
    diagnostics: &mut ParseDiagnostics,
) -> Result<LegacyExtractionResult, std::io::Error> {
    let mut file = File::open(path).map_err(|e| {
        std::io::Error::new(e.kind(), format!("Failed to open FASTA file '{}': {}", path, e))
    })?;
    // Handle UTF-8 BOM: read first 3 bytes, skip if BOM, else seek back
    let mut bom_buf = [0u8; 3];
    let bytes_read = file.read(&mut bom_buf).unwrap_or(0);
    if !(bytes_read >= 3 && bom_buf == *UTF8_BOM) {
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));
    }

    let buf_reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let mut reader = parse_fastx_reader(buf_reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("FASTA parse error: {}", e)))?;

    process_fasta_records(
        reader.as_mut(), *kmer_length, validator, header_format, header_fillna,
        stats, config, diagnostics,
    )
}

/// Extract k-mers and headers with columnar metadata storage
/// 
/// Same as `get_kmers_and_headers_validated` but returns metadata in columnar format
/// for improved cache locality and performance.
pub fn get_kmers_and_headers_encoded_columnar(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    config: Option<KmerExtractionConfig>,
    expected_count: Option<usize>,
) -> Result<ColumnarExtractionResult, std::io::Error> {
    let (kmers, row_headers, sequence_count, is_protein, stats, diagnostics) = get_kmers_and_headers_validated(
        path, kmer_length, header_format, header_fillna, alphabet, config, expected_count
    )?;
    
    let columnar_headers = if let (Some(headers), Some(format)) = (row_headers, header_format) {
        // Use non-indexing variant: the CLI analysis path never queries indices.
        // Indices remain available via from_row_metadata_with_indexing for Tauri/API consumers.
        let adapter = ColumnarMetadataAdapter::from_row_metadata(format.clone(), headers);
        Some(adapter)
    } else {
        None
    };
    
    Ok((kmers, columnar_headers, sequence_count, is_protein, stats, diagnostics))
}

/// Determine if an I/O error is a transient mmap-related failure that
/// warrants falling back to buffered I/O (e.g., permission denied, filesystem
/// doesn't support mmap). Validation errors (InvalidData) should NOT be retried
/// because re-reading the same file will produce the same validation failure.
fn is_retriable_io_error(e: &io::Error) -> bool {
    // InvalidData/InvalidInput: re-reading the same file won't fix validation errors.
    // Interrupted: deliberate application cancellation (Ctrl-C) must propagate immediately,
    // not trigger a full file re-read via buffered I/O fallback.
    !matches!(
        e.kind(),
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput | io::ErrorKind::Interrupted
    )
}

/// Decide whether to use memory-mapped I/O based on file size and compression.
///
/// Files >= 10MB benefit from mmap (avoids copy through page cache).
/// Smaller files use buffered I/O (lower syscall overhead for small reads).
/// Compressed files NEVER use mmap (compressed data must be streamed through decompressor).
/// The `DIMA_FORCE_MMAP` env var overrides for testing: "1" forces mmap, "0" forces buffered.
fn should_use_memory_mapping(path: &String) -> bool {
    // Compressed files cannot be mmap'd (data must be decompressed sequentially)
    if is_compressed(Path::new(path)) {
        return false;
    }

    match std::env::var("DIMA_FORCE_MMAP") {
        Ok(ref val) if val == "1" => return true,
        Ok(ref val) if val == "0" => return false,
        _ => {}
    }

    let file_size = match metadata(path) {
        Ok(meta) => meta.len(),
        Err(_) => return false,
    };

    const MMAP_THRESHOLD: u64 = 10 * 1024 * 1024; // 10MB
    file_size >= MMAP_THRESHOLD
}

/// String-based k-mer extraction with CharacterValidator.
/// 
/// Returns k-mers as strings instead of encoded values.
/// Currently unused by the main pipeline (which uses encoded k-mers for performance),
/// but retained as a public API for library consumers who need string-level access.
#[allow(dead_code)]
pub fn get_kmers_and_headers_string_validated(
    path: &String,
    kmer_length: &usize,
    header_format: Option<&Vec<String>>,
    header_fillna: Option<&String>,
    alphabet: Option<&String>,
    config: Option<KmerExtractionConfig>,
    expected_count: Option<usize>,
) -> Result<StringKmerResult, std::io::Error> {
    let config = config.unwrap_or_default();
    
    let alphabet_type = AlphabetType::from_optional_str(alphabet.map(|s| s.as_str()));
    
    let validator = CharacterValidator::with_options(
        alphabet_type,
        config.validation_mode,
        config.allow_lowercase,
    );

    let mut transposed_kmers: Vec<Vec<String>> = Vec::new();
    let mut headers_vec: Vec<Option<HashMap<String, String>>> = Vec::new();
    let mut sequence_count: usize = 0;
    let mut diagnostics = ParseDiagnostics::new();

    let mut file = File::open(path).map_err(|e| {
        std::io::Error::new(e.kind(), format!("Failed to open FASTA file '{}': {}", path, e))
    })?;
    // Strip UTF-8 BOM if present (common from Windows editors)
    let mut bom_buf = [0u8; 3];
    let bytes_read = file.read(&mut bom_buf).unwrap_or(0);
    if !(bytes_read >= 3 && bom_buf == *UTF8_BOM) {
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));
    }
    let buf_reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let mut reader = parse_fastx_reader(buf_reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("FASTA parse error: {}", e)))?;

    while let Some(record_result) = reader.next() {
        let record = match record_result {
            Ok(r) => r,
            Err(e) => {
                diagnostics.skipped_records += 1;
                tracing::warn!(after_seq = sequence_count, error = %e, "skipping malformed FASTA record");
                continue;
            }
        };
        sequence_count += 1;
        tracing::Span::current().pb_inc(1);

        let raw_id = record.id();
        if raw_id.len() > MAX_HEADER_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Sequence {} has a header of {} bytes — exceeds maximum ({} KB). \
                     This is likely malformed or adversarial input.",
                    sequence_count, raw_id.len(), MAX_HEADER_LENGTH / 1024,
                ),
            ));
        }
        let sequence_bytes = record.raw_seq();
        let record_id = String::from_utf8_lossy(raw_id);
        if sequence_bytes.len() > MAX_SEQUENCE_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Sequence {} ('{}') is {} bytes — exceeds maximum ({} MB). Not valid MSA input.",
                    sequence_count, record_id, sequence_bytes.len(), MAX_SEQUENCE_LENGTH / (1024 * 1024),
                ),
            ));
        }
        let sequence = match std::str::from_utf8(sequence_bytes) {
            Ok(s) => s.to_owned(),
            Err(_) => {
                diagnostics.skipped_records += 1;
                tracing::warn!(sequence = sequence_count, "skipping record with invalid UTF-8 sequence");
                continue;
            }
        };
        let kmers = sliding_window_string_validated(&sequence, *kmer_length, &validator);

        if transposed_kmers.is_empty() {
            transposed_kmers = vec![Vec::with_capacity(expected_count.unwrap_or(1024)); kmers.len()];
        }

        for (i, k) in kmers.into_iter().enumerate() {
            transposed_kmers[i].push(k);
        }

        if let Some(headers_components) = header_format {
            let fixed_header = String::from_utf8_lossy(raw_id).to_string();
            let fill = header_fillna.map(|s| s.as_str()).unwrap_or("Unknown");
            headers_vec.push(Some(parse_header_internal(&fixed_header, headers_components, fill, &mut diagnostics)));
        }
    }

    // NOTE: "NA" k-mers are NOT removed here. They are retained so that the indices
    // into each position's Vec remain aligned with headers_vec (sequence order).
    // Downstream counting functions (count_kmers) should skip "NA" entries.

    let headers: Option<Vec<Option<HashMap<String, String>>>> = if header_format.is_none() {
        None
    } else {
        Some(headers_vec)
    };

    Ok((transposed_kmers, headers, sequence_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper: write FASTA content to a temp file, return path string and file handle.
    fn write_fasta(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f.flush().unwrap();
        f
    }

    fn default_config() -> KmerExtractionConfig {
        KmerExtractionConfig::new()
    }

    // ─── Basic FASTA Reading ─────────────────────────────────────────────────

    #[test]
    fn test_basic_protein_fasta_extraction() {
        let fasta = write_fasta(">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_ok());
        let (encoded, _meta, seq_count, is_protein, _stats, _diag) = result.unwrap();

        assert_eq!(seq_count, 2);
        assert!(is_protein);
        // 10 chars - 3 + 1 = 8 positions
        assert_eq!(encoded.len(), 8);
        // Each position should have 2 k-mers (one per sequence)
        assert_eq!(encoded[0].len(), 2);
    }

    #[test]
    fn test_basic_nucleotide_fasta_extraction() {
        let fasta = write_fasta(">s1\nACGTACGTACGT\n>s2\nACGTACGTACGT\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 4usize;
        let alphabet = "nucleotide".to_string();

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, Some(&alphabet), Some(default_config()), None,
        );
        assert!(result.is_ok());
        let (encoded, _, seq_count, is_protein, _, _) = result.unwrap();

        assert_eq!(seq_count, 2);
        assert!(!is_protein);
        assert_eq!(encoded.len(), 9); // 12 - 4 + 1 = 9
    }

    // ─── K-mer Length Validation ─────────────────────────────────────────────

    #[test]
    fn test_kmer_length_zero_rejected() {
        let fasta = write_fasta(">s1\nACDEF\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 0usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid"));
    }

    #[test]
    fn test_kmer_length_exceeds_protein_max_rejected() {
        let fasta = write_fasta(">s1\nACDEFGHIKLMNPQRS\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 15usize; // max for protein is 14

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid"));
    }

    #[test]
    fn test_kmer_length_exceeds_nucleotide_max_rejected() {
        let fasta = write_fasta(">s1\nACGTACGTACGTACGTACGTACGTACGTACGT\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 28usize; // max for nucleotide is 27
        let alphabet = "nucleotide".to_string();

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, Some(&alphabet), Some(default_config()), None,
        );
        assert!(result.is_err());
    }

    // ─── MSA Validation ─────────────────────────────────────────────────────

    #[test]
    fn test_unequal_length_sequences_rejected() {
        let fasta = write_fasta(">s1\nACDEFGHIKL\n>s2\nACDEF\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("equal length") || err_msg.contains("length"),
            "Expected MSA length error, got: {}", err_msg
        );
    }

    #[test]
    fn test_single_sequence_accepted() {
        let fasta = write_fasta(">only\nACDEFGHIKL\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_ok());
        let (_, _, seq_count, _, _, _) = result.unwrap();
        assert_eq!(seq_count, 1);
    }

    // ─── Invalid Character Handling ─────────────────────────────────────────

    #[test]
    fn test_gap_characters_produce_sentinels() {
        // Gaps should produce u64::MAX sentinel values at affected positions
        let fasta = write_fasta(">s1\nACD-FGHIKL\n>s2\nACDEFGHIKL\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_ok());
        let (encoded, _, _, _, _, _) = result.unwrap();

        // Position 2 (k-mer "D-F") should be invalid for seq1 → u64::MAX
        // Positions 1,2,3 are affected by the gap at index 3
        let has_sentinel = encoded.iter().any(|col| col.contains(&u64::MAX));
        assert!(has_sentinel, "Gap should produce sentinel values");
    }

    #[test]
    fn test_validation_stats_reported_when_enabled() {
        let fasta = write_fasta(">s1\nACD-FGHIKL\n>s2\nACDEFGHIKL\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;
        let config = KmerExtractionConfig::new().with_report_invalid(true);

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(config), None,
        );
        assert!(result.is_ok());
        let (_, _, _, _, stats, _) = result.unwrap();
        assert!(stats.is_some(), "Validation stats should be present when reporting enabled");
    }

    // ─── BOM Handling ────────────────────────────────────────────────────────

    #[test]
    fn test_utf8_bom_stripped_correctly() {
        let mut f = NamedTempFile::new().unwrap();
        // Write UTF-8 BOM followed by FASTA content
        f.write_all(b"\xEF\xBB\xBF>seq1\nACDEFGHIKL\n>seq2\nACDEFGHIKL\n").unwrap();
        f.flush().unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(default_config()), None,
        );
        assert!(result.is_ok(), "BOM should be transparently handled");
        let (_, _, seq_count, _, _, _) = result.unwrap();
        assert_eq!(seq_count, 2);
    }

    // ─── Cancellation ────────────────────────────────────────────────────────

    #[test]
    fn test_cancellation_during_io_returns_interrupted() {
        // Pre-signal cancellation before starting I/O
        let cancel = Arc::new(AtomicBool::new(true));
        let config = KmerExtractionConfig::new().with_cancel_token(cancel);

        // Need enough sequences to trigger the per-1000 check
        let mut fasta_content = String::new();
        for i in 0..1100 {
            fasta_content.push_str(&format!(">s{}\nACDEFGHIKL\n", i));
        }
        let fasta = write_fasta(&fasta_content);
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, None, None, None, Some(config), None,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }

    // ─── Header Parsing ──────────────────────────────────────────────────────

    #[test]
    fn test_header_metadata_extraction() {
        let fasta = write_fasta(">sample1|treated|rep1\nACDEFGHIKL\n>sample2|control|rep2\nACDEFGHIKL\n");
        let path = fasta.path().to_str().unwrap().to_string();
        let kmer_len = 3usize;
        let format = vec!["id".to_string(), "condition".to_string(), "replicate".to_string()];
        let fillna = "NA".to_string();

        let result = get_kmers_and_headers_validated(
            &path, &kmer_len, Some(&format), Some(&fillna), None, Some(default_config()), None,
        );
        assert!(result.is_ok());
        let (_, meta, _, _, _, _) = result.unwrap();
        let meta = meta.unwrap();
        assert_eq!(meta.len(), 2);

        let first = meta[0].as_ref().unwrap();
        assert_eq!(first.get("id").unwrap(), "sample1");
        assert_eq!(first.get("condition").unwrap(), "treated");
    }

    // ─── Atomic Write ────────────────────────────────────────────────────────

    #[test]
    fn test_atomic_write_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.json");

        let result = atomic_write(&path, |writer| {
            writer.write_all(b"hello world")
        });
        assert!(result.is_ok());
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
    }

    #[test]
    fn test_atomic_write_failure_leaves_no_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("should_not_exist.json");

        let result = atomic_write(&path, |_writer| {
            Err(io::Error::other("simulated failure"))
        });
        assert!(result.is_err());
        assert!(!path.exists(), "Failed atomic_write should not leave a file");
    }
}
