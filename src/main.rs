use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use dima::get_results_objs;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Alphabet {
    Protein,
    Nucleotide,
}

#[derive(Parser, Debug)]
#[command(name = "dima")]
#[command(about = "DiMA - Diversity Motif Analyser", long_about = None)]
struct Cli {
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

    /// Only compute entropies and supports, omit variant lists and metadata
    #[arg(long = "summary-only")]
    summary_only: bool,

    /// Number of Rayon worker threads (defaults to number of CPUs)
    #[arg(long = "threads")]
    threads: Option<usize>,
}

fn main() {
    let cli = Cli::parse();

    if let Some(n_threads) = cli.threads {
        let _ = rayon::ThreadPoolBuilder::new().num_threads(n_threads).build_global();
    }

    let header_format = if cli.no_metadata || cli.summary_only {
        None
    } else {
        cli.header_format
            .as_ref()
            .map(|s| s.split('|').map(|v| v.trim().to_string()).collect())
    };

    let metadata_fields = if cli.no_metadata || cli.summary_only {
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

    let results = get_results_objs(
        cli.input.to_string_lossy().to_string(),
        cli.kmer_length,
        cli.support_threshold,
        cli.query_name,
        header_format,
        alphabet,
        Some(cli.header_fillna),
        metadata_fields,
        cli.summary_only,
    );

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
    } else {
        println!("{}", results);
    }
} 