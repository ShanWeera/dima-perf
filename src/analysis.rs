use hashbrown::HashMap;
use indicatif::ProgressStyle;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::alphabet::{ValidationMode, ValidationStats};
use crate::io::{
    InputSource,
    get_kmers_and_headers_encoded_columnar,
    KmerExtractionConfig,
    ParseDiagnostics,
};
use crate::entropy::calculate_entropy_encoded_at_position;
use crate::kmer::{count_kmers_encoded, decode_kmer};
use crate::models::{Results, Position, Variant, HighestEntropy};

/// Errors that can occur during the analysis pipeline.
///
/// Uses `thiserror` for structured, matchable error types with automatic
/// `Display` and `Error` trait implementations. Each variant provides
/// specific context to enable actionable CLI error messages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AnalysisError {
    /// The input file could not be opened or read.
    #[error("failed to read '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The input data failed validation (e.g. unequal MSA lengths, no sequences).
    #[error("{reason}")]
    Validation { reason: String },

    /// The analysis was cancelled by the user via the cancel token.
    #[error("analysis cancelled by user")]
    Cancelled,

    /// A binary format error occurred during deserialization or serialization.
    #[error("binary format error: {0}")]
    BinaryFormat(#[from] crate::binary::BinaryFormatError),
}

impl From<std::io::Error> for AnalysisError {
    fn from(e: std::io::Error) -> Self {
        AnalysisError::Io {
            path: PathBuf::from("<unknown>"),
            source: e,
        }
    }
}


/// Configuration for analysis with validation options
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Validation mode for character checking
    pub validation_mode: ValidationMode,
    /// Allow lowercase characters (auto-converted to uppercase)
    pub allow_lowercase: bool,
    /// Report invalid characters found during processing
    pub report_invalid: bool,
    /// Cooperative cancellation token. When set to `true`, parallel computation
    /// loops will short-circuit and return `AnalysisError::Cancelled`.
    /// Pass `None` for non-cancellable runs (CLI without signal handling, tests).
    pub cancel_token: Option<Arc<AtomicBool>>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            validation_mode: ValidationMode::default(),
            allow_lowercase: false,
            report_invalid: false,
            cancel_token: None,
        }
    }
}

impl AnalysisConfig {
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

    /// Returns true if cancellation has been requested.
    fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .is_some_and(|t| t.load(Ordering::Relaxed))
    }
    
    fn to_kmer_extraction_config(&self) -> KmerExtractionConfig {
        KmerExtractionConfig {
            validation_mode: self.validation_mode,
            allow_lowercase: self.allow_lowercase,
            report_invalid: self.report_invalid,
            cancel_token: self.cancel_token.clone(),
            expected_sequence_count: None,
        }
    }
}

// ─── Shared Pipeline Infrastructure (Fix 2.1) ────────────────────────────────
//
// Both `get_results_objs_columnar` and `get_results_objs` share identical logic
// for entropy computation, position building, and summary statistics. The only
// difference is how per-variant metadata is aggregated (columnar vs row-based).
// The `MetadataAggregator` trait abstracts over that difference, eliminating
// ~300 lines of duplication and ensuring bug fixes apply to both paths.

/// Abstracts metadata aggregation for a variant's sequence indices.
/// Implementations provide columnar or row-based lookups.
trait MetadataAggregator: Sync {
    /// Aggregate metadata counts for the given sequence indices and fields.
    /// Returns a nested map: field_name → (value → count).
    fn aggregate(
        &self,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>>;
}

/// Columnar metadata strategy: delegates to the columnar adapter's parallel aggregation.
struct ColumnarAggregator<'a> {
    adapter: &'a crate::columnar::ColumnarMetadataAdapter,
}

impl MetadataAggregator for ColumnarAggregator<'_> {
    fn aggregate(
        &self,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>> {
        self.adapter.get_columnar().aggregate_metadata_for_indices_parallel(indices, fields)
    }
}

/// Row-based metadata strategy: iterates sequence indices and accumulates counts
/// from per-sequence HashMap metadata.
struct RowAggregator<'a> {
    headers: &'a [Option<HashMap<String, String>>],
}

impl MetadataAggregator for RowAggregator<'_> {
    fn aggregate(
        &self,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>> {
        let mut result: HashMap<String, HashMap<String, usize>> = HashMap::new();
        for &idx in indices {
            let header_map = match self.headers.get(idx).and_then(|h| h.as_ref()) {
                Some(m) => m,
                None => continue,
            };
            for field in fields {
                let value = match header_map.get(field) {
                    Some(v) => v,
                    None => continue,
                };
                result
                    .entry(field.clone())
                    .or_default()
                    .entry(value.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }
        }
        result
    }
}

/// Shared entropy computation with cooperative cancellation and span-based progress.
///
/// Progress is driven by the caller's entered span (which has `indicatif.pb_show`).
/// Each rayon worker propagates the parent span context and increments via `pb_inc`.
/// When no IndicatifLayer is registered (Tauri, tests), `pb_inc` is a documented no-op.
fn compute_entropies(
    encoded_kmers: &[Vec<u64>],
    support_threshold: usize,
    config: &AnalysisConfig,
) -> Result<Vec<f64>, AnalysisError> {
    let parent = tracing::Span::current();
    encoded_kmers
        .par_iter()
        .enumerate()
        .map(|(idx, position_kmers)| {
            let _guard = parent.enter();
            if config.is_cancelled() {
                return Err(AnalysisError::Cancelled);
            }
            let entropy = calculate_entropy_encoded_at_position(
                position_kmers, &support_threshold, idx,
            );
            tracing::Span::current().pb_inc(1);
            Ok(entropy)
        })
        .collect()
}

/// Shared position building: count k-mers, decode variants, aggregate metadata,
/// and assign low-support labels.
///
/// Progress is driven by the caller's entered span. Each rayon worker propagates the
/// parent span context and increments via `pb_inc` (no-op without IndicatifLayer).
#[allow(clippy::too_many_arguments)]
fn build_positions(
    encoded_kmers: &[Vec<u64>],
    position_entropies: &[f64],
    kmer_length: usize,
    support_threshold: usize,
    is_protein: bool,
    config: &AnalysisConfig,
    aggregator: Option<&dyn MetadataAggregator>,
    fields: &[String],
) -> Result<Vec<Position>, AnalysisError> {
    let parent = tracing::Span::current();
    encoded_kmers
        .par_iter()
        .map(|position_kmers| count_kmers_encoded(position_kmers))
        .enumerate()
        .map(|(idx, position_count)| {
            let _guard = parent.enter();
            if config.is_cancelled() {
                return Err(AnalysisError::Cancelled);
            }
            tracing::Span::current().pb_inc(1);
            let pos = build_single_position(
                idx, &position_count, position_entropies,
                kmer_length, support_threshold, is_protein, aggregator, fields,
            );
            Ok(pos)
        })
        .collect()
}

/// Construct a single Position from k-mer counts at a given index.
#[allow(clippy::too_many_arguments)]
fn build_single_position(
    idx: usize,
    position_count: &HashMap<u64, (usize, Vec<usize>)>,
    position_entropies: &[f64],
    kmer_length: usize,
    support_threshold: usize,
    is_protein: bool,
    aggregator: Option<&dyn MetadataAggregator>,
    fields: &[String],
) -> Position {
    // Derive support from the count map (sum of all valid k-mer counts) — avoids
    // a redundant O(N) sentinel scan that `count_kmers_encoded` already performed.
    let support: usize = position_count.values().map(|(count, _)| count).sum();

    let mut variants: Vec<Variant> = Vec::with_capacity(position_count.len());
    variants.extend(position_count.iter().map(|(&encoded_sequence, count_data)| {
            let sequence = decode_kmer(encoded_sequence, kmer_length, is_protein);
            let mut variant = Variant {
                sequence,
                count: count_data.0,
                incidence: if support > 0 {
                    (count_data.0 as f64 / support as f64) * 100.0
                } else {
                    0.0
                },
                metadata: None,
                motif_short: None,
                motif_long: None,
            };

            if let Some(agg) = aggregator {
                if !fields.is_empty() {
                    let metadata = agg.aggregate(&count_data.1, fields);
                    // Always set Some(metadata) when fields configured — unifies
                    // semantics across both paths (Some({}) vs None distinction).
                    variant.metadata = Some(metadata);
                }
            }

            variant
        }));

    // Low-support classification per DiMA publication (Tharanga et al. 2025,
    // PMC11596295, Table 1):
    //   NS  = No Support (N = 0)
    //   LS  = Low Support (N < T)
    //   ELS = Exceptional Low Support (N = T, i.e. exactly at threshold)
    //   None = fully supported (N > T)
    let low_support_label = if support == 0 {
        Some("NS".to_owned())
    } else if support < support_threshold {
        Some("LS".to_owned())
    } else if support == support_threshold {
        Some("ELS".to_owned())
    } else {
        None
    };

    Position::new(
        idx + 1,
        position_entropies[idx],
        support,
        if variants.is_empty() { None } else { Some(&mut variants) },
        low_support_label,
    )
}

/// Compute summary statistics (average entropy, highest entropy, low support count)
/// from the finished positions list.
///
/// Guards against NaN/Inf propagation: positions with non-finite entropy are
/// excluded from averaging and max-finding. This prevents one corrupted position
/// from poisoning the entire summary (IEEE 754 NaN propagation).
fn compute_summary_stats(positions: &[Position]) -> (f64, HighestEntropy, usize) {
    // Count all positions with any low-support label (NS, LS, ELS).
    // Per DiMA publication: ELS (N=T) is a form of low support — the label
    // distinguishes it from LS (N<T) but it's still counted as low-support.
    let low_support_count = positions
        .iter()
        .filter(|p| matches!(p.low_support.as_deref(), Some("NS") | Some("LS") | Some("ELS")))
        .count();

    // ELS positions (support == threshold) are included because they received
    // full rarefaction correction and are scientifically valid per PMC11596295.
    // Only NS (no support) and LS (below threshold) are excluded.
    // Non-finite entropy values are also excluded to prevent NaN poisoning.
    let reliable_positions: Vec<&Position> = positions
        .iter()
        .filter(|p| !matches!(p.low_support.as_deref(), Some("NS") | Some("LS")))
        .filter(|p| p.entropy.is_finite())
        .collect();

    let average_entropy = if reliable_positions.is_empty() {
        0.0
    } else {
        reliable_positions.iter().map(|p| p.entropy).sum::<f64>() / reliable_positions.len() as f64
    };

    let highest = if reliable_positions.is_empty() {
        HighestEntropy { position: 0, entropy: 0.0 }
    } else {
        let best = reliable_positions
            .iter()
            .max_by(|a, b| a.entropy.partial_cmp(&b.entropy).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();
        HighestEntropy { position: best.position, entropy: best.entropy }
    };

    (average_entropy, highest, low_support_count)
}

// ─── Shared Pipeline Core ────────────────────────────────────────────────────
//
// Both analysis paths (row and columnar) perform identical post-I/O processing.
// This function eliminates the 80+ lines of duplicated logic.

#[allow(clippy::too_many_arguments)]
fn finish_analysis(
    encoded_kmers: Vec<Vec<u64>>,
    sequence_count: usize,
    kmer_length: usize,
    support_threshold: usize,
    is_protein: bool,
    query_name: String,
    config: &AnalysisConfig,
    header_format: &Option<Vec<String>>,
    metadata_fields: &Option<Vec<String>>,
    aggregator: Option<&dyn MetadataAggregator>,
    validation_stats: Option<ValidationStats>,
    diagnostics: &ParseDiagnostics,
    io_duration: std::time::Duration,
    input_size_bytes: Option<u64>,
) -> Result<(Results, Option<ValidationStats>, crate::perf::PerfReport), AnalysisError> {
    if sequence_count == 0 {
        return Err(AnalysisError::Validation {
            reason: "Input file contains no valid sequences. Ensure the file is a non-empty FASTA \
                     with at least one sequence.".to_string(),
        });
    }

    let parse_failures = diagnostics.header_parse_failures;
    if parse_failures > 0 && sequence_count > 0 {
        let failure_rate = parse_failures as f64 / sequence_count as f64;
        if failure_rate > 0.05 {
            tracing::warn!(
                failures = parse_failures,
                total = sequence_count,
                rate = format!("{:.1}%", failure_rate * 100.0),
                "high header parse failure rate — check --header-format matches your FASTA headers"
            );
        }
    }

    if config.is_cancelled() {
        return Err(AnalysisError::Cancelled);
    }

    if encoded_kmers.is_empty() {
        return Err(AnalysisError::Validation {
            reason: format!(
                "No k-mer positions produced: all {} sequences are shorter than k-mer length {}. \
                 Reduce the k-mer length or provide longer sequences.",
                sequence_count, kmer_length
            ),
        });
    }

    let entropy_start = std::time::Instant::now();
    let entropy_span = tracing::info_span!(
        "entropy",
        positions = encoded_kmers.len(),
        indicatif.pb_show = tracing::field::Empty,
    );
    entropy_span.pb_set_style(&ProgressStyle::with_template(
        "{spinner:.green} Entropy: [{bar:40.cyan/blue}] {pos}/{len} [{elapsed}<{eta}] ({per_sec})"
    ).unwrap().progress_chars("#>-"));
    entropy_span.pb_set_length(encoded_kmers.len() as u64);
    let _entropy_entered = entropy_span.entered();
    let position_entropies = compute_entropies(&encoded_kmers, support_threshold, config)?;
    drop(_entropy_entered);
    let entropy_duration = entropy_start.elapsed();

    if config.is_cancelled() {
        return Err(AnalysisError::Cancelled);
    }

    let fields: Vec<String> = match (header_format, metadata_fields) {
        (Some(hdr_fmt), Some(only)) => hdr_fmt.iter().filter(|f| only.contains(f)).cloned().collect(),
        (Some(hdr_fmt), None) => hdr_fmt.clone(),
        _ => Vec::new(),
    };

    let building_start = std::time::Instant::now();
    let building_span = tracing::info_span!(
        "positions",
        count = encoded_kmers.len(),
        indicatif.pb_show = tracing::field::Empty,
    );
    building_span.pb_set_style(&ProgressStyle::with_template(
        "{spinner:.green} Building: [{bar:40.cyan/blue}] {pos}/{len} [{elapsed}<{eta}] ({per_sec})"
    ).unwrap().progress_chars("#>-"));
    building_span.pb_set_length(encoded_kmers.len() as u64);
    let _building_entered = building_span.entered();
    let positions = build_positions(
        &encoded_kmers, &position_entropies, kmer_length, support_threshold,
        is_protein, config, aggregator, &fields,
    )?;
    drop(_building_entered);
    let building_duration = building_start.elapsed();

    let (average_entropy, highest, low_support_count) = compute_summary_stats(&positions);

    let position_count = positions.len();
    let results = Results {
        sequence_count,
        support_threshold,
        low_support_count,
        query_name,
        kmer_length,
        highest_entropy: highest,
        average_entropy,
        results: positions,
    };

    let perf_report = crate::perf::PerfReport {
        io_duration,
        entropy_duration,
        building_duration,
        output_duration: std::time::Duration::ZERO, // filled by caller
        sequence_count,
        position_count,
        input_size_bytes,
    };

    Ok((results, validation_stats, perf_report))
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Unified analysis entry point: extracts k-mers from an input source and computes
/// Shannon entropy, diversity motifs, and support classification at each position.
///
/// When `header_format` is provided, automatically uses columnar metadata storage
/// (superior cache locality and aggregation performance). When `header_format` is
/// None, metadata processing is skipped entirely for maximum throughput.
///
/// # Arguments
/// * `input` - Input source (file path or stdin)
/// * `kmer_length` - Length of k-mers to analyze
/// * `support_threshold` - Minimum support threshold for entropy calculation
/// * `query_name` - Name for the query/sample
/// * `header_format` - Optional header format for metadata extraction
/// * `alphabet` - "protein" or "nucleotide" (defaults to "protein")
/// * `header_fillna` - Value to use for missing header fields
/// * `metadata_fields` - Optional fields to include in metadata (subset of header_format)
/// * `config` - Optional AnalysisConfig for validation options
///
/// # Returns
/// `Ok((Results, Option<ValidationStats>))` on success, or `AnalysisError` if
/// the input cannot be read or fails validation.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    input: InputSource,
    kmer_length: usize,
    support_threshold: usize,
    query_name: String,
    header_format: Option<Vec<String>>,
    alphabet: Option<String>,
    header_fillna: Option<String>,
    metadata_fields: Option<Vec<String>>,
    config: Option<AnalysisConfig>,
) -> Result<(Results, Option<ValidationStats>, crate::perf::PerfReport), AnalysisError> {
    let config = config.unwrap_or_default();
    let kmer_config = config.to_kmer_extraction_config();

    let input_size_bytes = input.file_size();

    let path = match &input {
        InputSource::File(p) => p.to_string_lossy().to_string(),
        InputSource::Stdin => {
            return analyze_stdin(
                kmer_length, support_threshold, query_name, header_format,
                alphabet, header_fillna, metadata_fields, config,
            );
        }
    };

    let io_start = std::time::Instant::now();

    // Unified I/O span: drives both tracing diagnostics and progress display
    let io_span = tracing::info_span!(
        "fasta_io",
        path = %path,
        indicatif.pb_show = tracing::field::Empty,
    );
    io_span.pb_set_style(&ProgressStyle::with_template(
        "{spinner:.green} {msg}: {human_pos} seqs [{elapsed}]"
    ).unwrap());
    io_span.pb_set_message("Reading FASTA");
    let _io_span = io_span.entered();

    let (encoded_kmers, columnar_headers, sequence_count, is_protein, validation_stats, diagnostics) =
        get_kmers_and_headers_encoded_columnar(
            &path,
            &kmer_length,
            header_format.as_ref(),
            header_fillna.as_ref(),
            alphabet.as_ref(),
            Some(kmer_config),
            None,
        )?;

    drop(_io_span);
    let io_duration = io_start.elapsed();

    let aggregator: Option<ColumnarAggregator> = columnar_headers.as_ref().map(|adapter| {
        ColumnarAggregator { adapter }
    });

    finish_analysis(
        encoded_kmers, sequence_count, kmer_length, support_threshold,
        is_protein, query_name, &config, &header_format, &metadata_fields,
        aggregator.as_ref().map(|a| a as &dyn MetadataAggregator),
        validation_stats, &diagnostics,
        io_duration, input_size_bytes,
    )
}

/// Stdin analysis path: reads from standard input using needletail's streaming parser.
/// No mmap, no file size estimation. Progress is a span-based spinner (no known total).
#[allow(clippy::too_many_arguments)]
fn analyze_stdin(
    kmer_length: usize,
    support_threshold: usize,
    query_name: String,
    header_format: Option<Vec<String>>,
    alphabet: Option<String>,
    header_fillna: Option<String>,
    metadata_fields: Option<Vec<String>>,
    config: AnalysisConfig,
) -> Result<(Results, Option<ValidationStats>, crate::perf::PerfReport), AnalysisError> {
    let kmer_config = config.to_kmer_extraction_config();
    let io_start = std::time::Instant::now();

    let alphabet_type = crate::alphabet::AlphabetType::from_optional_str(
        alphabet.as_ref().map(|s| s.as_str())
    );
    let is_protein = alphabet_type == crate::alphabet::AlphabetType::Protein;

    let max_k = crate::kmer::max_kmer_length(is_protein);
    if kmer_length == 0 || kmer_length > max_k {
        return Err(AnalysisError::Validation {
            reason: format!(
                "k-mer length {} is invalid for {} alphabet (valid range: 1..={})",
                kmer_length, if is_protein { "protein" } else { "nucleotide" }, max_k
            ),
        });
    }

    let validator = crate::alphabet::CharacterValidator::with_options(
        alphabet_type,
        kmer_config.validation_mode,
        kmer_config.allow_lowercase,
    );
    let stats = if kmer_config.report_invalid {
        Some(crate::alphabet::ValidationStats::new())
    } else {
        None
    };

    // Span-based spinner for stdin I/O (no known total length)
    let io_span = tracing::info_span!(
        "fasta_io",
        path = "stdin",
        indicatif.pb_show = tracing::field::Empty,
    );
    io_span.pb_set_style(&ProgressStyle::with_template(
        "{spinner:.green} Reading stdin: {human_pos} seqs [{elapsed}]"
    ).unwrap());
    io_span.pb_set_message("Reading stdin");
    let _io_entered = io_span.entered();

    // Read all stdin into memory: stdin is non-seekable and StdinLock is !Send,
    // so we buffer it into a Cursor which satisfies needletail's Send requirement.
    use std::io::Read;
    let mut stdin_data = Vec::new();
    std::io::stdin().lock().read_to_end(&mut stdin_data)
        .map_err(|e| std::io::Error::new(e.kind(), format!("failed to read stdin: {}", e)))?;

    if stdin_data.is_empty() {
        return Err(AnalysisError::Validation {
            reason: "No data received from stdin. Pipe a FASTA file or use --input <file>.".to_string(),
        });
    }

    let cursor = std::io::Cursor::new(stdin_data);
    let mut reader = needletail::parse_fastx_reader(cursor)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("FASTA parse error: {}", e)))?;

    let mut diagnostics = crate::io::ParseDiagnostics::new();
    let (transposed_kmers, headers_vec, sequence_count) = crate::io::process_fasta_records(
        reader.as_mut(), kmer_length, &validator, header_format.as_ref(),
        header_fillna.as_ref(), stats.as_ref(), &kmer_config, &mut diagnostics,
    )?;

    drop(_io_entered);
    let io_duration = io_start.elapsed();

    // Build columnar metadata if header_format is provided
    let columnar_headers = if let Some(ref format) = header_format {
        let adapter = crate::columnar::ColumnarMetadataAdapter::from_row_metadata(
            format.clone(),
            headers_vec,
        );
        Some(adapter)
    } else {
        None
    };

    let aggregator: Option<ColumnarAggregator> = columnar_headers.as_ref().map(|adapter| {
        ColumnarAggregator { adapter }
    });

    finish_analysis(
        transposed_kmers, sequence_count, kmer_length, support_threshold,
        is_protein, query_name, &config, &header_format, &metadata_fields,
        aggregator.as_ref().map(|a| a as &dyn MetadataAggregator),
        stats, &diagnostics,
        io_duration, None, // stdin has no file size
    )
}

// Legacy aliases for backward compatibility during migration (Tauri app uses these).
// These return the old 2-tuple signature by discarding the PerfReport.
#[allow(clippy::too_many_arguments)]
#[doc(hidden)]
pub fn get_results_objs(
    path: String,
    kmer_length: usize,
    support_threshold: usize,
    query_name: String,
    header_format: Option<Vec<String>>,
    alphabet: Option<String>,
    header_fillna: Option<String>,
    metadata_fields: Option<Vec<String>>,
    config: Option<AnalysisConfig>,
) -> Result<(Results, Option<ValidationStats>), AnalysisError> {
    let (results, stats, _perf) = analyze(InputSource::File(PathBuf::from(path)), kmer_length, support_threshold, query_name, header_format, alphabet, header_fillna, metadata_fields, config)?;
    Ok((results, stats))
}

#[allow(clippy::too_many_arguments)]
#[doc(hidden)]
pub fn get_results_objs_columnar(
    path: String,
    kmer_length: usize,
    support_threshold: usize,
    query_name: String,
    header_format: Option<Vec<String>>,
    alphabet: Option<String>,
    header_fillna: Option<String>,
    metadata_fields: Option<Vec<String>>,
    config: Option<AnalysisConfig>,
) -> Result<(Results, Option<ValidationStats>), AnalysisError> {
    let (results, stats, _perf) = analyze(InputSource::File(PathBuf::from(path)), kmer_length, support_threshold, query_name, header_format, alphabet, header_fillna, metadata_fields, config)?;
    Ok((results, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_fasta(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f.flush().unwrap();
        f
    }

    fn run_analysis(fasta_content: &str, kmer_length: usize, threshold: usize) -> Result<(Results, Option<ValidationStats>), AnalysisError> {
        let fasta = write_fasta(fasta_content);
        let path = fasta.path().to_str().unwrap().to_string();
        let config = AnalysisConfig::new();
        get_results_objs(
            path, kmer_length, threshold, "test".to_string(),
            None, None, None, None, Some(config),
        )
    }

    #[test]
    fn test_analysis_produces_correct_position_count() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n>s3\nACDEFGHIKL\n",
            3, 2,
        );
        assert!(result.is_ok());
        let (results, _) = result.unwrap();
        assert_eq!(results.results.len(), 8); // 10 - 3 + 1
        assert_eq!(results.sequence_count, 3);
        assert_eq!(results.kmer_length, 3);
    }

    #[test]
    fn test_identical_sequences_zero_entropy_everywhere() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n>s3\nACDEFGHIKL\n",
            3, 2,
        );
        let (results, _) = result.unwrap();
        for pos in &results.results {
            assert_eq!(pos.entropy, 0.0,
                "Identical sequences must yield zero entropy at position {}", pos.position);
        }
        assert_eq!(results.average_entropy, 0.0);
    }

    #[test]
    fn test_diverse_sequences_positive_entropy() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nMNPQRSTVWY\n>s3\nACDEFSTVWY\n",
            3, 2,
        );
        let (results, _) = result.unwrap();
        assert!(results.average_entropy > 0.0,
            "Diverse sequences should produce positive average entropy");
    }

    #[test]
    fn test_single_variant_classified_as_index() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n",
            3, 2,
        );
        let (results, _) = result.unwrap();
        for pos in &results.results {
            let motifs = pos.diversity_motifs.as_ref().unwrap();
            assert_eq!(motifs.len(), 1);
            assert_eq!(motifs[0].motif_short.as_deref(), Some("I"));
            assert_eq!(motifs[0].incidence, 100.0);
        }
    }

    #[test]
    fn test_low_support_labeling() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n>s3\nACDEFGHIKL\n",
            3, 5,
        );
        let (results, _) = result.unwrap();
        for pos in &results.results {
            assert_eq!(pos.low_support.as_deref(), Some("LS"),
                "support={} < threshold=5 should be LS", pos.support);
        }
        assert!(results.low_support_count > 0);
    }

    #[test]
    fn test_els_labeling_when_support_equals_threshold() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n>s3\nACDEFGHIKL\n>s4\nACDEFGHIKL\n",
            3, 4,
        );
        let (results, _) = result.unwrap();
        for pos in &results.results {
            assert_eq!(pos.low_support.as_deref(), Some("ELS"),
                "support==threshold should be ELS");
        }
    }

    #[test]
    fn test_no_low_support_when_above_threshold() {
        let mut fasta = String::new();
        for i in 0..10 {
            fasta.push_str(&format!(">s{}\nACDEFGHIKL\n", i));
        }
        let result = run_analysis(&fasta, 3, 5);
        let (results, _) = result.unwrap();
        for pos in &results.results {
            assert!(pos.low_support.is_none(),
                "support=10 > threshold=5 should have no low_support label");
        }
        assert_eq!(results.low_support_count, 0);
    }

    #[test]
    fn test_cancellation_during_entropy_computation() {
        let cancel = Arc::new(AtomicBool::new(true));
        let config = AnalysisConfig::new()
            .with_cancel_token(cancel);

        let fasta = write_fasta(">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n");
        let path = fasta.path().to_str().unwrap().to_string();

        let result = get_results_objs(
            path, 3, 2, "test".to_string(),
            None, None, None, None, Some(config),
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            AnalysisError::Cancelled => {}
            other => panic!("Expected Cancelled, got {:?}", other),
        }
    }

    #[test]
    fn test_highest_entropy_position_tracked() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n>s3\nACDEFGHIKL\n",
            3, 2,
        );
        let (results, _) = result.unwrap();
        assert_eq!(results.highest_entropy.entropy, 0.0);
    }

    #[test]
    fn test_kmer_length_equals_sequence_length() {
        let result = run_analysis(">s1\nACDEF\n>s2\nACDEF\n", 5, 2);
        let (results, _) = result.unwrap();
        assert_eq!(results.results.len(), 1);
    }

    #[test]
    fn test_gaps_reduce_support() {
        let result = run_analysis(
            ">s1\nACDEFGHIKL\n>s2\nACD-FGHIKL\n>s3\nACDEFGHIKL\n",
            3, 2,
        );
        let (results, _) = result.unwrap();
        // k-mers spanning the gap at position index 3 are invalidated
        let pos2 = &results.results[1]; // position 2 = "CDE" for valid, "CD-" for invalid
        assert!(pos2.support < 3, "Gap should reduce support, got {}", pos2.support);
    }

    #[test]
    fn test_summary_stats_consistency() {
        let mut fasta = String::new();
        for i in 0..20 {
            let seq = if i % 3 == 0 { "ACDEFGHIKL" } else { "MNPQRSTVWY" };
            fasta.push_str(&format!(">s{}\n{}\n", i, seq));
        }
        let result = run_analysis(&fasta, 3, 5);
        let (results, _) = result.unwrap();

        let sum: f64 = results.results.iter().map(|p| p.entropy).sum();
        let expected_avg = sum / results.results.len() as f64;
        assert!((results.average_entropy - expected_avg).abs() < 1e-10,
            "average_entropy should be mean of all position entropies");

        let max_entropy = results.results.iter()
            .map(|p| p.entropy)
            .fold(0.0_f64, f64::max);
        assert_eq!(results.highest_entropy.entropy, max_entropy);
    }
}
