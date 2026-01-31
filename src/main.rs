use std::path::PathBuf;

use clap::{Parser, ValueEnum, Subcommand};

use dima_lib::{
    ValidationMode,
    get_results_objs, 
    get_results_objs_columnar,
    AnalysisConfig,
};

mod help;

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
    #[arg(short = 'i', long = "input", value_name = "FASTA",
          help = help::analyze::INPUT_HELP,
          long_help = help::analyze::INPUT_LONG_HELP)]
    input: PathBuf,

    #[arg(short = 'k', long = "kmer", default_value_t = 9,
          help = help::analyze::KMER_HELP,
          long_help = help::analyze::KMER_LONG_HELP)]
    kmer_length: usize,

    #[arg(short = 't', long = "threshold", default_value_t = 30,
          help = help::analyze::THRESHOLD_HELP,
          long_help = help::analyze::THRESHOLD_LONG_HELP)]
    support_threshold: usize,

    #[arg(short = 'n', long = "name", default_value = "Unknown Protein",
          help = help::analyze::NAME_HELP,
          long_help = help::analyze::NAME_LONG_HELP)]
    query_name: String,

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

    #[arg(long = "hcs",
          help = help::analyze::HCS_HELP,
          long_help = help::analyze::HCS_LONG_HELP)]
    hcs_only: bool,

    #[arg(long = "hcs-threshold",
          help = help::analyze::HCS_THRESHOLD_HELP,
          long_help = help::analyze::HCS_THRESHOLD_LONG_HELP)]
    hcs_threshold: Option<f32>,

    #[arg(long = "threads",
          help = help::analyze::THREADS_HELP,
          long_help = help::analyze::THREADS_LONG_HELP)]
    threads: Option<usize>,

    #[arg(long = "columnar",
          help = help::analyze::COLUMNAR_HELP,
          long_help = help::analyze::COLUMNAR_LONG_HELP)]
    columnar: bool,

    #[arg(long = "indexing",
          help = help::analyze::INDEXING_HELP,
          long_help = help::analyze::INDEXING_LONG_HELP)]
    indexing: bool,

    #[arg(long = "binary",
          help = help::analyze::BINARY_HELP,
          long_help = help::analyze::BINARY_LONG_HELP)]
    binary: bool,

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
}

#[derive(Parser, Debug)]
struct DeflateArgs {
    #[arg(short = 'i', long = "input", value_name = "BINARY",
          help = help::deflate::INPUT_HELP,
          long_help = help::deflate::INPUT_LONG_HELP)]
    input: PathBuf,

    #[arg(short = 'o', long = "output",
          help = help::deflate::OUTPUT_HELP,
          long_help = help::deflate::OUTPUT_LONG_HELP)]
    output: Option<PathBuf>,

    #[arg(long = "no-pretty",
          help = help::deflate::NO_PRETTY_HELP,
          long_help = help::deflate::NO_PRETTY_LONG_HELP)]
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

    // Parse header format if provided (None disables metadata processing)
    let header_format = cli.header_format
        .as_ref()
        .map(|s| s.split('|').map(|v| v.trim().to_string()).collect());

    // Parse metadata fields filter (only relevant if header_format is provided)
    let metadata_fields = cli.metadata_fields
        .as_ref()
        .map(|s| s.split('|').map(|v| v.trim().to_string()).collect());

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
        get_results_objs_columnar(
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
        get_results_objs(
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
                0 => dima_lib::binary::CompressionType::None,
                1 => dima_lib::binary::CompressionType::Lz4,
                2 => dima_lib::binary::CompressionType::Zstd,
                _ => {
                    eprintln!("Invalid compression type: {}. Use 0=none, 1=lz4, 2=zstd", cli.compression);
                    std::process::exit(1);
                }
            };
            
            let config = dima_lib::binary::BinaryFormatConfig {
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
    let results = match dima_lib::models::Results::from_binary(args.input.to_string_lossy().to_string()) {
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
