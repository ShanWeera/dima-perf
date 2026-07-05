use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use anyhow::{ensure, Context};
use clap::{CommandFactory, Parser, ValueEnum, Subcommand};
use tracing_indicatif::IndicatifLayer;
use tracing_indicatif::filter::{IndicatifFilter, hide_indicatif_span_fields};
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use dima_lib::{
    ValidationMode,
    InputSource,
    OutputType,
    analyze,
    resolve_output_type,
    write_results_to_output,
    AnalysisConfig,
    AnalysisError,
};

mod help;

/// Semantic exit codes following Unix conventions.
/// EXIT_CANCELLED uses 128+SIGINT(2) = 130 per POSIX practice.
mod exit_codes {
    pub const SUCCESS: u8 = 0;
    pub const RUNTIME_ERROR: u8 = 1;
    pub const USAGE_ERROR: u8 = 2;
    pub const IO_ERROR: u8 = 3;
    pub const CANCELLED: u8 = 130;
}

/// Wrapper for argument validation errors so they can be distinguished from
/// runtime errors in the exit code mapping. anyhow::ensure! produces generic
/// anyhow::Error with no distinct type — wrapping in UsageError allows
/// downcast_ref::<UsageError>() to identify them.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct UsageError(String);

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum Alphabet {
    Protein,
    Nucleotide,
}

/// Validation mode for character checking
#[derive(Copy, Clone, Debug, ValueEnum, Default)]
pub enum ValidationModeArg {
    /// Only accept valid alphabet characters (whitelist approach) - RECOMMENDED
    #[default]
    Strict,
    /// Accept valid + ambiguous characters, reject only completely invalid
    Permissive,
    /// Accept all characters but report invalid ones
    Report,
}

impl From<ValidationModeArg> for ValidationMode {
    fn from(arg: ValidationModeArg) -> Self {
        match arg {
            ValidationModeArg::Strict => ValidationMode::Strict,
            ValidationModeArg::Permissive => ValidationMode::Permissive,
            ValidationModeArg::Report => ValidationMode::ReportOnly,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "dima")]
#[command(version)]
#[command(about = "DiMA - Diversity Motif Analyser", long_about = None)]
#[command(after_help = "\
Quick start:\n  \
  dima analyze -i aligned.fasta -o results.json\n  \
  dima analyze -i aligned.fasta -O dima -o results.dima\n  \
  dima view -i results.dima -o results.json\n  \
  dima view -i results.dima -O tsv -o results.tsv\n\n\
Run 'dima <command> --help' for detailed usage.")]
#[command(after_long_help = "\
ENVIRONMENT VARIABLES:\n  \
  DIMA_FORCE_MMAP=1    Force memory-mapped I/O regardless of file size\n  \
  DIMA_FORCE_MMAP=0    Force buffered I/O regardless of file size\n  \
  RAYON_NUM_THREADS=N  Set thread pool size (alternative to --threads)\n  \
  NO_COLOR=1           Disable colored output (https://no-color.org/)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze FASTA file and generate diversity motif results
    Analyze(AnalyzeArgs),
    /// View/convert binary .dima files to other formats
    View(ViewArgs),
    /// Generate shell completions for tab-completion support
    Completions(CompletionsArgs),
}

#[derive(Parser, Debug)]
struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    shell: clap_complete::Shell,
}

/// Shared output format arguments used by both `analyze` and `view` commands.
/// Follows BCFtools convention: -O/--output-type for format selection.
#[derive(clap::Args, Debug)]
struct OutputArgs {
    /// Output format. If not specified, inferred from output file extension
    /// (.dima → dima, .tsv → tsv, .jsonl → jsonl, else json).
    /// Follows samtools convention: explicit -O always wins over extension.
    #[arg(short = 'O', long = "output-type", value_enum)]
    output_type: Option<OutputType>,

    /// Omit TSV header row (only applies to -O tsv)
    #[arg(long = "no-header")]
    no_header: bool,
}

#[derive(Parser, Debug)]
struct AnalyzeArgs {
    /// Path to input FASTA file. Use '-' for stdin, or omit when piping data.
    #[arg(short = 'i', long = "input", value_name = "FASTA",
          help = help::analyze::INPUT_HELP,
          long_help = help::analyze::INPUT_LONG_HELP)]
    input: Option<PathBuf>,

    #[arg(short = 'k', long = "kmer",
          help = help::analyze::KMER_HELP,
          long_help = help::analyze::KMER_LONG_HELP)]
    kmer_length: Option<usize>,

    #[arg(short = 't', long = "threshold", default_value_t = 100,
          help = help::analyze::THRESHOLD_HELP,
          long_help = help::analyze::THRESHOLD_LONG_HELP)]
    support_threshold: usize,

    #[arg(short = 'n', long = "name",
          help = help::analyze::NAME_HELP,
          long_help = help::analyze::NAME_LONG_HELP)]
    query_name: Option<String>,

    #[arg(long = "header-format",
          help = help::analyze::HEADER_FORMAT_HELP,
          long_help = help::analyze::HEADER_FORMAT_LONG_HELP)]
    header_format: Option<String>,

    #[arg(long = "metadata-fields",
          help = help::analyze::METADATA_FIELDS_HELP,
          long_help = help::analyze::METADATA_FIELDS_LONG_HELP)]
    metadata_fields: Option<String>,

    #[arg(long = "header-fillna", default_value = "Unknown",
          help = help::analyze::HEADER_FILLNA_HELP,
          long_help = help::analyze::HEADER_FILLNA_LONG_HELP)]
    header_fillna: String,

    #[arg(long = "alphabet", value_enum, default_value_t = Alphabet::Protein,
          help = help::analyze::ALPHABET_HELP,
          long_help = help::analyze::ALPHABET_LONG_HELP)]
    alphabet: Alphabet,

    #[arg(short = 'o', long = "output",
          help = help::analyze::OUTPUT_HELP,
          long_help = help::analyze::OUTPUT_LONG_HELP)]
    output: Option<PathBuf>,

    #[arg(long = "hcs-output", value_name = "HCS_FILE",
          help = help::analyze::HCS_OUTPUT_HELP,
          long_help = help::analyze::HCS_OUTPUT_LONG_HELP)]
    hcs_output: Option<PathBuf>,

    #[arg(long = "hcs-threshold",
          help = help::analyze::HCS_THRESHOLD_HELP,
          long_help = help::analyze::HCS_THRESHOLD_LONG_HELP)]
    hcs_threshold: Option<f64>,

    #[arg(long = "threads",
          help = help::analyze::THREADS_HELP,
          long_help = help::analyze::THREADS_LONG_HELP)]
    threads: Option<usize>,

    /// Compression type for -O dima output (0=none, 1=lz4, 2=zstd)
    #[arg(long = "compression", default_value = "1",
          help = help::analyze::COMPRESSION_HELP,
          long_help = help::analyze::COMPRESSION_LONG_HELP)]
    compression: u8,

    #[arg(long = "validation", value_enum, default_value_t = ValidationModeArg::Strict,
          help = help::analyze::VALIDATION_HELP,
          long_help = help::analyze::VALIDATION_LONG_HELP)]
    validation: ValidationModeArg,

    #[arg(long = "allow-lowercase",
          help = help::analyze::ALLOW_LOWERCASE_HELP,
          long_help = help::analyze::ALLOW_LOWERCASE_LONG_HELP)]
    allow_lowercase: bool,

    #[arg(long = "report-invalid",
          help = help::analyze::REPORT_INVALID_HELP,
          long_help = help::analyze::REPORT_INVALID_LONG_HELP)]
    report_invalid: bool,

    /// Force disk-backed mode for large datasets (auto-detected by default)
    #[arg(long = "low-memory",
          help = "Force disk-backed matrix storage (auto-detected by default)")]
    low_memory: bool,

    /// Force RAM-only mode even when matrix exceeds available memory
    #[arg(long = "force-ram",
          help = "Force RAM mode even for large datasets (may cause OOM)")]
    force_ram: bool,

    /// Directory for temporary matrix files in disk-backed mode
    #[arg(long = "temp-dir", value_name = "DIR",
          help = "Temp directory for disk-backed mode (default: $TMPDIR or input dir)")]
    temp_dir: Option<PathBuf>,

    #[command(flatten)]
    output_args: OutputArgs,

    /// Control output verbosity (-q quiet, -v info, -vv debug, -vvv trace).
    /// Default shows warnings and progress bars. -q suppresses everything.
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,
}

#[derive(Parser, Debug)]
struct ViewArgs {
    /// Path to the binary .dima file to view/convert
    #[arg(short = 'i', long = "input", value_name = "DIMA_FILE",
          help = help::view::INPUT_HELP,
          long_help = help::view::INPUT_LONG_HELP)]
    input: PathBuf,

    /// Output file path (prints to stdout if not specified)
    #[arg(short = 'o', long = "output",
          help = help::view::OUTPUT_HELP,
          long_help = help::view::OUTPUT_LONG_HELP)]
    output: Option<PathBuf>,

    /// Compression type for -O dima output (0=none, 1=lz4, 2=zstd)
    #[arg(long = "compression", default_value = "1",
          help = "Compression for -O dima re-encoding (0=none, 1=lz4, 2=zstd)")]
    compression: u8,

    #[command(flatten)]
    output_args: OutputArgs,

    /// Control output verbosity (-q quiet, -v info, -vv debug).
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,
}

/// Initialize the tracing subscriber stack with optional indicatif progress bar integration.
///
/// Uses per-layer filtering: EnvFilter controls fmt_layer verbosity independently,
/// while IndicatifFilter controls which spans display progress bars. This ensures
/// progress bars work at ANY verbosity level (they only need `indicatif.pb_show`),
/// while log output respects the user's `-v`/`-q` settings.
///
/// When `show_progress` is false (quiet mode or non-terminal), the IndicatifLayer is
/// not registered at all — all `pb_inc`/`pb_set_*` calls become documented no-ops.
fn init_tracing(
    verbosity: &clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,
    show_progress: bool,
) {
    let use_ansi = std::io::stderr().is_terminal()
        && std::env::var("NO_COLOR").map_or(true, |v| v.is_empty());

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            let level = verbosity.tracing_level_filter();
            tracing_subscriber::EnvFilter::new(level.to_string())
        });

    if show_progress {
        // Full progress bar support: IndicatifLayer manages a coordinated MultiProgress
        // at 20Hz draw rate. Only spans with `indicatif.pb_show` field get bars.
        let indicatif_layer = IndicatifLayer::new()
            .with_span_field_formatter(hide_indicatif_span_fields(DefaultFields::new()));

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(indicatif_layer.get_stderr_writer())
            .with_target(false)
            .with_ansi(use_ansi)
            .with_level(true);

        tracing_subscriber::registry()
            .with(fmt_layer.with_filter(filter))
            .with(indicatif_layer.with_filter(IndicatifFilter::new(false)))
            .init();
    } else {
        // No progress bars: plain log output to stderr (must explicitly set writer
        // because tracing-subscriber defaults to stdout)
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(false)
            .with_ansi(use_ansi)
            .with_level(true);

        tracing_subscriber::registry()
            .with(fmt_layer.with_filter(filter))
            .init();
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Analyze(args) => run_analyze(args),
        Commands::View(args) => run_view(args),
        Commands::Completions(args) => {
            clap_complete::generate(
                args.shell,
                &mut Cli::command(),
                "dima",
                &mut std::io::stdout(),
            );
            return ExitCode::from(exit_codes::SUCCESS);
        }
    };

    match result {
        Ok(()) => ExitCode::from(exit_codes::SUCCESS),
        Err(e) => {
            // Map specific error types to semantic exit codes
            let code = if let Some(analysis_err) = e.downcast_ref::<AnalysisError>() {
                match analysis_err {
                    AnalysisError::Cancelled => exit_codes::CANCELLED,
                    AnalysisError::Validation { .. } => exit_codes::USAGE_ERROR,
                    AnalysisError::Io { .. } => exit_codes::IO_ERROR,
                    AnalysisError::BinaryFormat(_) => exit_codes::IO_ERROR,
                    _ => exit_codes::RUNTIME_ERROR,
                }
            } else if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                match io_err.kind() {
                    std::io::ErrorKind::Interrupted => exit_codes::CANCELLED,
                    _ => exit_codes::IO_ERROR,
                }
            } else if e.downcast_ref::<UsageError>().is_some() {
                exit_codes::USAGE_ERROR
            } else {
                exit_codes::RUNTIME_ERROR
            };

            // Cancelled gets a short message (user already saw "Interrupted...")
            if code == exit_codes::CANCELLED {
                tracing::warn!("analysis cancelled by user");
            } else {
                // Use eprintln for top-level errors to ensure error messages appear
                // regardless of tracing subscriber configuration and to produce stable
                // output format for integration tests and scripts.
                eprintln!("Error: {:#}", e);
            }

            ExitCode::from(code)
        }
    }
}

// ─── Fail-Fast Argument Validation ──────────────────────────────────────────
//
// All cheap checks run BEFORE any I/O or computation, preventing wasted CPU
// time on invalid arguments. Per Fail-Fast principle (ArchMan, "Core Design
// and Programming Principles"): detect invalid state at the boundary.

fn validate_analyze_args(args: &AnalyzeArgs, resolved_output_type: OutputType) -> anyhow::Result<()> {
    // Input file must exist and be a regular file (only checked for file paths, not stdin)
    if let Some(ref input) = args.input {
        if input.as_os_str() != "-" {
            ensure!(
                input.exists(),
                "input file does not exist: {}",
                input.display()
            );
            ensure!(
                input.is_file(),
                "input path is not a regular file: {}",
                input.display()
            );
        }
    }

    // Binary output requires a file path (cannot stream binary to stdout)
    if resolved_output_type == OutputType::Dima && args.output.is_none() {
        anyhow::bail!("--output required for -O dima (cannot stream binary to stdout)");
    }

    // Output directory must exist
    if let Some(ref out) = args.output {
        if let Some(parent) = out.parent() {
            ensure!(
                parent.as_os_str().is_empty() || parent.exists(),
                "output directory '{}' does not exist",
                parent.display()
            );
        }
    }

    // HCS output directory must exist (same validation as --output)
    if let Some(ref hcs_out) = args.hcs_output {
        if let Some(parent) = hcs_out.parent() {
            ensure!(
                parent.as_os_str().is_empty() || parent.exists(),
                "HCS output directory '{}' does not exist",
                parent.display()
            );
        }
    }

    // Compression range validation
    ensure!(
        args.compression <= 2,
        "invalid --compression: {}. Use 0=none, 1=lz4, 2=zstd",
        args.compression
    );

    // Support threshold must be at least 1
    ensure!(
        args.support_threshold > 0,
        "--threshold must be >= 1"
    );

    // K-mer length validation (if explicitly provided)
    if let Some(k) = args.kmer_length {
        ensure!(k >= 1, "--kmer must be >= 1");

        let is_protein = args.alphabet != Alphabet::Nucleotide;
        let max_k = dima_lib::kmer::max_kmer_length(is_protein);
        ensure!(
            k <= max_k,
            "--kmer={} exceeds maximum for {} ({}) — limited by u64 encoding",
            k,
            if is_protein { "protein" } else { "nucleotide" },
            max_k
        );
    }

    // HCS threshold range
    if let Some(thresh) = args.hcs_threshold {
        ensure!(
            (0.0..=100.0).contains(&thresh) && !thresh.is_nan(),
            "--hcs-threshold must be between 0.0 and 100.0 (got {})",
            thresh
        );
    }

    // Thread count must be >= 1
    if let Some(n) = args.threads {
        ensure!(n >= 1, "--threads must be >= 1");
    }

    // Header format cannot be empty and must have non-empty field names
    if let Some(ref fmt) = args.header_format {
        ensure!(
            !fmt.trim().is_empty(),
            "--header-format cannot be empty. Provide pipe-delimited field names."
        );

        let fields: Vec<&str> = fmt.split('|').map(|f| f.trim()).collect();

        ensure!(
            fields.iter().all(|f| !f.is_empty()),
            "--header-format contains empty field name(s). Each field between '|' \
             delimiters must be non-empty. Got: {:?}",
            fmt
        );

        let unique: std::collections::HashSet<&str> = fields.iter().copied().collect();
        ensure!(
            unique.len() == fields.len(),
            "--header-format contains duplicate field name(s). \
             Duplicates cause silent data corruption in columnar storage. Fields: {:?}",
            fields
        );
    }

    // ─── No-op flag combination warnings ─────────────────────────────────────
    if args.metadata_fields.is_some() && args.header_format.is_none() {
        tracing::warn!("--metadata-fields has no effect without --header-format");
    }
    if args.hcs_threshold.is_some() && args.hcs_output.is_none() {
        tracing::warn!("--hcs-threshold has no effect without --hcs-output");
    }
    if args.compression != 1 && resolved_output_type != OutputType::Dima {
        tracing::warn!("--compression has no effect without -O dima");
    }
    if args.output_args.no_header && resolved_output_type != OutputType::Tsv {
        tracing::warn!("--no-header has no effect without -O tsv");
    }

    Ok(())
}

fn run_analyze(cli: AnalyzeArgs) -> anyhow::Result<()> {
    // Compute quiet FIRST — needed to decide whether to register IndicatifLayer
    let is_quiet = cli.verbosity.is_silent()
        || cli.verbosity.tracing_level_filter() <= tracing::level_filters::LevelFilter::ERROR;
    let show_progress = !is_quiet && std::io::stderr().is_terminal();

    init_tracing(&cli.verbosity, show_progress);

    // Resolve output format before validation (validation needs the resolved type)
    let output_type = resolve_output_type(
        cli.output_args.output_type,
        cli.output.as_deref(),
    );

    // Fail-fast: all cheap validation before any I/O.
    validate_analyze_args(&cli, output_type)
        .map_err(|e| anyhow::anyhow!(UsageError(format!("{:#}", e))))?;

    // Alphabet-aware k-mer default per DiMA publication (Tharanga et al. 2025):
    // protein=9 (nonamer), nucleotide=27 (9 aa × 3 nt/codon)
    let kmer_length = cli.kmer_length.unwrap_or(match cli.alphabet {
        Alphabet::Nucleotide => 27,
        Alphabet::Protein => 9,
    });

    let is_protein = cli.alphabet != Alphabet::Nucleotide;

    // Alphabet-aware default query name
    let query_name = cli.query_name.unwrap_or_else(|| {
        if is_protein { "Unknown Protein".to_string() } else { "Unknown Nucleotide".to_string() }
    });

    // Configure thread pool if explicitly requested
    if let Some(n_threads) = cli.threads {
        if let Err(e) = rayon::ThreadPoolBuilder::new().num_threads(n_threads).build_global() {
            tracing::warn!(threads = n_threads, error = %e, "failed to set thread count");
        }
    }

    // Parse header format if provided (None disables metadata processing)
    let header_format: Option<Vec<String>> = cli.header_format
        .as_ref()
        .map(|s| s.split('|').map(|v| v.trim().to_string()).collect());

    // Parse metadata fields filter
    let metadata_fields: Option<Vec<String>> = cli.metadata_fields
        .as_ref()
        .map(|s| s.split('|').map(|v| v.trim().to_string()).collect());

    let alphabet = match cli.alphabet {
        Alphabet::Protein => Some("protein".to_string()),
        Alphabet::Nucleotide => Some("nucleotide".to_string()),
    };

    // Cooperative cancellation via Ctrl-C.
    // Double-Ctrl-C forces immediate exit for unresponsive situations.
    let cancel_token = Arc::new(AtomicBool::new(false));
    let ctrl_c_count = Arc::new(AtomicU8::new(0));
    {
        let token = cancel_token.clone();
        let count = ctrl_c_count.clone();
        ctrlc::set_handler(move || {
            let prev = count.fetch_add(1, Ordering::SeqCst);
            if prev == 0 {
                eprintln!("\nInterrupted — requesting graceful cancellation...");
                token.store(true, Ordering::SeqCst);
            } else {
                eprintln!("\nForce quit.");
                std::process::exit(exit_codes::CANCELLED as i32);
            }
        }).unwrap_or_else(|e| eprintln!("Warning: could not set signal handler: {}", e));
    }

    let analysis_config = AnalysisConfig::new()
        .with_validation_mode(cli.validation.into())
        .with_allow_lowercase(cli.allow_lowercase)
        .with_report_invalid(cli.report_invalid)
        .with_cancel_token(cancel_token);

    // Determine input source: explicit path, explicit '-' for stdin, or auto-detect piped stdin
    let input_source = match cli.input.as_ref() {
        Some(path) if path.as_os_str() == "-" => InputSource::Stdin,
        Some(path) => InputSource::File(path.clone()),
        None if !std::io::stdin().is_terminal() => InputSource::Stdin,
        None => {
            return Err(anyhow::anyhow!(UsageError(
                "No input specified. Use --input <file> or pipe data via stdin.".into()
            )));
        }
    };
    let (results, validation_stats, mut perf_report) = analyze(
        input_source,
        kmer_length,
        cli.support_threshold,
        query_name,
        header_format,
        alphabet,
        Some(cli.header_fillna),
        metadata_fields,
        Some(analysis_config),
    ).map_err(|e| -> anyhow::Error { e.into() })?;

    // Post-analysis UX warning: threshold exceeds actual sequence count means
    // all positions are Low Support/ELS and rarefaction was skipped (raw Shannon only).
    if !is_quiet && cli.support_threshold > results.sequence_count {
        tracing::warn!(
            threshold = cli.support_threshold,
            sequences = results.sequence_count,
            "threshold exceeds sequence count — all positions marked Low Support"
        );
    }

    // Report validation statistics if requested
    if cli.report_invalid && !is_quiet {
        if let Some(stats) = validation_stats {
            let summary = stats.summary();
            tracing::info!("{}", summary);
            
            if summary.invalid_chars > 0 {
                tracing::warn!(
                    invalid_chars = summary.invalid_chars,
                    alphabet = if is_protein { "protein (20 amino acids)" } else { "nucleotide (ACGTU)" },
                    "invalid characters found — k-mers containing them marked as NA"
                );
            }
        }
    }

    // Write HCS output if --hcs-output is specified
    if let Some(ref hcs_path) = cli.hcs_output {
        results.get_hcs(
            Some(hcs_path.to_string_lossy().to_string()),
            cli.hcs_threshold,
        ).with_context(|| format!("failed to write HCS to '{}'", hcs_path.display()))?;
        if !is_quiet {
            tracing::info!(path = %hcs_path.display(), "HCS results saved");
        }
    }

    // Write output using the resolved output type (timed for perf report)
    let output_start = std::time::Instant::now();
    match output_type {
        OutputType::Dima => {
            let out = cli.output.as_ref().unwrap(); // validated: Dima requires -o
            let compression_type = match cli.compression {
                0 => dima_lib::binary::CompressionType::None,
                1 => dima_lib::binary::CompressionType::Lz4,
                2 => dima_lib::binary::CompressionType::Zstd,
                _ => unreachable!("validated"),
            };
            let config = dima_lib::binary::BinaryFormatConfig {
                compression: compression_type,
                compression_level: 1,
                string_interning: true,
                validate_checksums: true,
            };
            results.to_binary(out.to_string_lossy().to_string(), Some(config))
                .with_context(|| format!("failed to write binary output to '{}'", out.display()))?;
            if !is_quiet {
                tracing::info!(path = %out.display(), format = "dima", "results saved");
            }
        }
        other => {
            write_results_to_output(
                &results,
                cli.output.as_deref(),
                other,
                cli.output_args.no_header,
            ).context("failed to write output")?;
        }
    }
    let output_duration = output_start.elapsed();
    perf_report.output_duration = output_duration;

    // Post-analysis summary: always shown unless --quiet
    if !is_quiet {
        let total = perf_report.total_duration();
        eprintln!(
            "analysis complete: {} sequences \u{00d7} {} positions in {:.2}s (avg entropy: {:.4})",
            results.sequence_count,
            results.results.len(),
            total.as_secs_f64(),
            results.average_entropy,
        );
    }

    // Verbose performance report: printed at -v level (Info or higher)
    let is_verbose = cli.verbosity.tracing_level_filter() >= tracing::level_filters::LevelFilter::INFO;
    if is_verbose {
        perf_report.print();
    }

    Ok(())
}

fn run_view(args: ViewArgs) -> anyhow::Result<()> {
    init_tracing(&args.verbosity, false);

    // Resolve output format before validation
    let output_type = resolve_output_type(
        args.output_args.output_type,
        args.output.as_deref(),
    );

    validate_view_args(&args, output_type)
        .map_err(|e| anyhow::anyhow!(UsageError(format!("{:#}", e))))?;

    // No-op flag warnings (use the resolved output type)
    if args.compression != 1 && output_type != OutputType::Dima {
        tracing::warn!("--compression has no effect without -O dima");
    }
    if args.output_args.no_header && output_type != OutputType::Tsv {
        tracing::warn!("--no-header has no effect without -O tsv");
    }

    // Extension warning (informational, not blocking)
    if let Some(ext) = args.input.extension() {
        if ext != "dima" {
            tracing::warn!(path = %args.input.display(), "input file does not have .dima extension");
        }
    } else {
        tracing::warn!(path = %args.input.display(), "input file has no extension, expected .dima");
    }

    // Read binary format
    let results = dima_lib::Results::from_binary(args.input.to_string_lossy().to_string())
        .with_context(|| format!(
            "failed to read binary file '{}'. Ensure it is a valid .dima format file",
            args.input.display()
        ))?;

    // Dispatch to output format
    match output_type {
        OutputType::Dima => {
            // Re-encode with (potentially different) compression
            let compression_type = match args.compression {
                0 => dima_lib::binary::CompressionType::None,
                1 => dima_lib::binary::CompressionType::Lz4,
                2 => dima_lib::binary::CompressionType::Zstd,
                _ => unreachable!("validated"),
            };
            let config = dima_lib::binary::BinaryFormatConfig {
                compression: compression_type,
                compression_level: 1,
                string_interning: true,
                validate_checksums: true,
            };
            let out = args.output.unwrap(); // validated: -O dima requires -o
            results.to_binary(out.to_string_lossy().to_string(), Some(config))
                .with_context(|| format!("failed to write to '{}'", out.display()))?;
            tracing::info!(input = %args.input.display(), output = %out.display(), "re-encoded");
        }
        other => {
            write_results_to_output(
                &results,
                args.output.as_deref(),
                other,
                args.output_args.no_header,
            ).context("failed to write output")?;
        }
    }

    Ok(())
}

fn validate_view_args(args: &ViewArgs, resolved_output_type: OutputType) -> anyhow::Result<()> {
    ensure!(
        args.input.exists(),
        "input file '{}' does not exist",
        args.input.display()
    );
    ensure!(
        args.input.is_file(),
        "input path '{}' is not a regular file",
        args.input.display()
    );
    ensure!(
        args.compression <= 2,
        "invalid --compression: {}. Use 0=none, 1=lz4, 2=zstd",
        args.compression
    );

    // Binary output requires a file path (cannot stream binary to stdout)
    if resolved_output_type == OutputType::Dima && args.output.is_none() {
        anyhow::bail!("--output required for -O dima (cannot stream binary to stdout)");
    }

    // Output directory must exist
    if let Some(ref out) = args.output {
        if let Some(parent) = out.parent() {
            ensure!(
                parent.as_os_str().is_empty() || parent.exists(),
                "output directory '{}' does not exist",
                parent.display()
            );
        }
    }

    Ok(())
}
