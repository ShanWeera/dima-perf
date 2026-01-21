use std::path::PathBuf;

use clap::{Parser, ValueEnum, Subcommand};

use dima::{
    ValidationMode,
    get_results_objs_validated, 
    get_results_objs_columnar_validated,
    AnalysisConfig,
};

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
#[command(about = "DiMA - Diversity Motif Analyser", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze FASTA file and generate diversity motif results
    Analyze(AnalyzeArgs),
    /// Convert binary format back to JSON (deflate/decompress)
    Deflate(DeflateArgs),
}

#[derive(Parser, Debug)]
struct AnalyzeArgs {
    /// Path to the FASTA file
    #[arg(short = 'i', long = "input", value_name = "FASTA")]
    input: PathBuf,

    /// K-mer length
    #[arg(short = 'k', long = "kmer", default_value_t = 9)]
    kmer_length: usize,

    /// Support threshold
    #[arg(short = 't', long = "threshold", default_value_t = 30)]
    support_threshold: usize,

    /// Query/sample name
    #[arg(short = 'n', long = "name", default_value = "Unknown Protein")]
    query_name: String,

    /// Header format fields separated by '|', e.g., "country|date|patient"
    #[arg(long = "header-format")]
    header_format: Option<String>,

    /// Restrict metadata to these fields (subset of header-format), separated by '|'
    #[arg(long = "metadata-fields")]
    metadata_fields: Option<String>,

    /// Fill NA for empty header fields
    #[arg(long = "header-fillna", default_value = "Unknown")]
    header_fillna: String,

    /// Alphabet: protein or nucleotide
    #[arg(long = "alphabet", value_enum, default_value_t = Alphabet::Protein)]
    alphabet: Alphabet,

    /// Output file to write JSON results. If not provided, prints to stdout
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Only print HCS (highly conserved sequences) instead of full JSON
    #[arg(long = "hcs")]
    hcs_only: bool,

    /// HCS incidence threshold percentage (0-100). Only used when --hcs is passed
    #[arg(long = "hcs-threshold")]
    hcs_threshold: Option<f32>,

    /// Disable per-variant metadata aggregation to improve speed and memory
    #[arg(long = "no-metadata")]
    no_metadata: bool,

    /// Number of Rayon worker threads (defaults to number of CPUs)
    #[arg(long = "threads")]
    threads: Option<usize>,

    /// Use columnar metadata storage for improved performance
    #[arg(long = "columnar")]
    columnar: bool,

    /// Enable metadata indexing for 80-95% faster lookups
    #[arg(long = "indexing")]
    indexing: bool,

    /// Use binary format for 50-70% faster I/O (output file will have .dima extension)
    #[arg(long = "binary")]
    binary: bool,

    /// Binary format compression level (0=none, 1=lz4, 2=zstd)
    #[arg(long = "compression", default_value = "1")]
    compression: u8,

    // =========================================================================
    // Character Validation Options
    // =========================================================================

    /// Character validation mode for k-mer generation.
    /// 
    /// - strict: Only accept valid alphabet characters (20 amino acids or 4/5 nucleotides).
    ///           This is the RECOMMENDED mode for scientific accuracy. Invalid characters
    ///           like #, *, @, numbers will cause k-mers to be marked as NA.
    /// 
    /// - permissive: Accept valid + known ambiguous characters (X, B, N, etc.).
    ///               Only completely invalid characters (#, *, etc.) cause NA k-mers.
    /// 
    /// - report: Accept all characters but report invalid ones found.
    ///           Useful for data quality assessment.
    #[arg(long = "validation", value_enum, default_value_t = ValidationModeArg::Strict)]
    validation: ValidationModeArg,

    /// Allow lowercase characters in sequences.
    /// When enabled, lowercase letters (a-z) are automatically converted to uppercase.
    /// By default, lowercase characters are treated as invalid.
    #[arg(long = "allow-lowercase")]
    allow_lowercase: bool,

    /// Report statistics about invalid characters found during processing.
    /// Shows counts of valid, ambiguous, gap, and invalid characters.
    #[arg(long = "report-invalid")]
    report_invalid: bool,
}

#[derive(Parser, Debug)]
struct DeflateArgs {
    /// Path to the binary (.dima) file to decompress
    #[arg(short = 'i', long = "input", value_name = "BINARY")]
    input: PathBuf,

    /// Output JSON file path. If not provided, prints to stdout
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Disable pretty printing (compact JSON output)
    #[arg(long = "no-pretty")]
    no_pretty: bool,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze(args) => run_analyze(args),
        Commands::Deflate(args) => run_deflate(args),
    }
}

fn run_analyze(cli: AnalyzeArgs) {
    if let Some(n_threads) = cli.threads {
        let _ = rayon::ThreadPoolBuilder::new().num_threads(n_threads).build_global();
    }

    let header_format = if cli.no_metadata {
        None
    } else {
        cli.header_format
            .as_ref()
            .map(|s| s.split('|').map(|v| v.trim().to_string()).collect())
    };

    let metadata_fields = if cli.no_metadata {
        None
    } else {
        cli.metadata_fields
            .as_ref()
            .map(|s| s.split('|').map(|v| v.trim().to_string()).collect())
    };

    let alphabet = match cli.alphabet {
        Alphabet::Protein => Some("protein".to_string()),
        Alphabet::Nucleotide => Some("nucleotide".to_string()),
    };

    // Build analysis config from CLI options
    let analysis_config = AnalysisConfig::new()
        .with_validation_mode(cli.validation.into())
        .with_allow_lowercase(cli.allow_lowercase)
        .with_report_invalid(cli.report_invalid);

    // Run analysis with validation
    let (results, validation_stats) = if cli.columnar {
        get_results_objs_columnar_validated(
            cli.input.to_string_lossy().to_string(),
            cli.kmer_length,
            cli.support_threshold,
            cli.query_name,
            header_format,
            alphabet,
            Some(cli.header_fillna),
            metadata_fields,
            Some(analysis_config),
        )
    } else {
        get_results_objs_validated(
            cli.input.to_string_lossy().to_string(),
            cli.kmer_length,
            cli.support_threshold,
            cli.query_name,
            header_format,
            alphabet,
            Some(cli.header_fillna),
            metadata_fields,
            Some(analysis_config),
        )
    };

    // Report validation statistics if requested
    if cli.report_invalid {
        if let Some(stats) = validation_stats {
            let summary = stats.summary();
            eprintln!("\n{}", summary);
            
            // Warn if invalid characters were found
            if summary.invalid_chars > 0 {
                eprintln!("\nWarning: {} invalid characters were found in the input sequences.", 
                         summary.invalid_chars);
                eprintln!("These characters are not part of the standard {} alphabet.",
                         if cli.alphabet == Alphabet::Protein { "protein (20 amino acids)" } else { "nucleotide (ACGTU)" });
                eprintln!("K-mers containing invalid characters were marked as NA.\n");
            }
        }
    }

    if cli.hcs_only {
        match results.get_hcs(
            cli.output.as_ref().map(|p| p.to_string_lossy().to_string()),
            cli.hcs_threshold,
        ) {
            Ok(hcs) => {
                if cli.output.is_none() {
                    println!("{}", serde_json::to_string_pretty(&hcs).unwrap());
                }
            }
            Err(e) => {
                eprintln!("Error writing HCS: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(out) = cli.output {
        if cli.binary {
            // Use binary format
            let binary_path = if out.extension().is_some() {
                out.with_extension("dima")
            } else {
                out.with_extension("dima")
            };
            
            let compression_type = match cli.compression {
                0 => dima::binary::CompressionType::None,
                1 => dima::binary::CompressionType::Lz4,
                2 => dima::binary::CompressionType::Zstd,
                _ => {
                    eprintln!("Invalid compression type: {}. Use 0=none, 1=lz4, 2=zstd", cli.compression);
                    std::process::exit(1);
                }
            };
            
            let config = dima::binary::BinaryFormatConfig {
                compression: compression_type,
                compression_level: 1,
                string_interning: true,
                buffer_size: 64 * 1024,
                validate_checksums: true,
            };
            
            match results.to_binary(binary_path.to_string_lossy().to_string(), Some(config)) {
                Ok(()) => {
                    println!("Results saved in binary format to: {}", binary_path.display());
                }
                Err(e) => {
                    eprintln!("Error writing binary output: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            // Use JSON format
            match results.to_json(Some(out.to_string_lossy().to_string())) {
                Ok(json) => {
                    if false {
                        println!("{}", json);
                    }
                }
                Err(e) => {
                    eprintln!("Error writing output: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        if cli.binary {
            eprintln!("Binary format requires an output file. Use --output <file>");
            std::process::exit(1);
        }
        println!("{}", results);
    }
}

fn run_deflate(args: DeflateArgs) {
    // Validate input file extension
    if let Some(extension) = args.input.extension() {
        if extension != "dima" {
            eprintln!("Warning: Input file does not have .dima extension. Expected binary format.");
        }
    } else {
        eprintln!("Warning: Input file has no extension. Expected .dima binary format.");
    }

    // Check if input file exists
    if !args.input.exists() {
        eprintln!("Error: Input file '{}' does not exist.", args.input.display());
        std::process::exit(1);
    }

    // Read binary format
    let results = match dima::models::Results::from_binary(args.input.to_string_lossy().to_string()) {
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error reading binary file '{}': {}", args.input.display(), e);
            eprintln!("Make sure the file is a valid .dima binary format file.");
            std::process::exit(1);
        }
    };

    // Convert to JSON
    let json_output = if !args.no_pretty {
        match serde_json::to_string_pretty(&results) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error serializing to JSON: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match serde_json::to_string(&results) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error serializing to JSON: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Output JSON
    if let Some(output_path) = args.output {
        match std::fs::write(&output_path, &json_output) {
            Ok(()) => {
                println!("Binary file '{}' successfully converted to JSON: '{}'", 
                        args.input.display(), output_path.display());
                
                // Show file size comparison
                if let (Ok(binary_metadata), Ok(json_metadata)) = (
                    std::fs::metadata(&args.input),
                    std::fs::metadata(&output_path)
                ) {
                    let binary_size = binary_metadata.len();
                    let json_size = json_metadata.len();
                    let ratio = json_size as f64 / binary_size as f64;
                    
                    println!("File size comparison:");
                    println!("  Binary (.dima): {} bytes", binary_size);
                    println!("  JSON:           {} bytes", json_size);
                    println!("  Expansion ratio: {:.2}x", ratio);
                }
            }
            Err(e) => {
                eprintln!("Error writing JSON file '{}': {}", output_path.display(), e);
                std::process::exit(1);
            }
        }
    } else {
        // Print to stdout
        println!("{}", json_output);
    }
}
