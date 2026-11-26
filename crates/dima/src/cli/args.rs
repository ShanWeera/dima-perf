//! Command-line syntax (D99): paper terms and conventions of domain tools.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use super::help;

#[derive(Debug, Parser)]
#[command(
    name = "dima",
    version,
    about = "DiMA: sequence diversity dynamics analyser for viruses",
    long_about = help::about_long(),
    after_long_help = help::footer(),
    propagate_version = true,
    max_term_width = 100,
    subcommand_required = false,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Print the licences of DiMA and its components, and exit.
    #[arg(long, exclusive = true)]
    pub licenses: bool,

    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Global options")]
pub struct GlobalArgs {
    #[command(flatten)]
    pub color: colorchoice_clap::Color,

    #[arg(short, long, global = true, action = clap::ArgAction::Count, conflicts_with = "verbose", help = help::short::QUIET, long_help = help::quiet())]
    pub quiet: u8,

    #[arg(short, long, global = true, action = clap::ArgAction::Count, help = help::short::VERBOSE, long_help = help::verbose())]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Analyse an alignment.
    #[command(alias = "analyse")]
    Analyze(Box<AnalyzeArgs>),
    /// Convert results (JSON) to JSON, JSONL or TSV.
    View(ViewArgs),
    /// Compute historically conserved sequences from results.
    Hcs(HcsArgs),
    /// Print how to cite DiMA (and, with --from, the references of a run).
    Cite(CiteArgs),
    /// Print a shell completion script.
    Completions {
        /// The shell.
        shell: clap_complete::Shell,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum AlphabetArg {
    /// Detect from the data.
    Auto,
    /// Amino acids.
    #[value(alias = "aa", alias = "amino-acid")]
    Protein,
    /// DNA.
    #[value(alias = "nucleotide")]
    Dna,
    /// RNA.
    Rna,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ValidationArg {
    /// Stop at the first character outside the alphabet.
    Strict,
    /// Exclude k-mers with such characters and warn.
    Permissive,
    /// Permissive, and print the character table.
    #[value(alias = "report-only")]
    Report,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum EstimatorArg {
    /// The paper's correction (exact expectation).
    Dima,
    /// The estimator of Chao et al. (2013).
    ChaoWangJost,
    /// Miller-Madow.
    MillerMadow,
    /// Observed (plug-in) entropy.
    #[value(alias = "plug-in")]
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum FormatArg {
    /// Complete results.
    Json,
    /// One position per line.
    Jsonl,
    /// One table (see --table).
    Tsv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum HcsFormatArg {
    /// Regions with members and provenance.
    Json,
    /// One row per region.
    Tsv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum TableArg {
    /// One row per position.
    Positions,
    /// One row per variant.
    Variants,
    /// One row per variant and header value.
    Metadata,
    /// One row per conserved region.
    Hcs,
}

/// How progress is shown on standard error (D109).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ProgressArg {
    /// On a terminal, one line redrawn in place; elsewhere nothing, or plain lines with -v.
    Auto,
    /// Appended lines: one per phase, and the progress at most every 10 seconds (each 10%),
    /// at least once a minute. For screen readers and log files.
    Plain,
    /// Nothing.
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CiteFormat {
    /// Plain text.
    Text,
    /// BibTeX.
    Bibtex,
    /// RIS.
    Ris,
    /// Citation File Format (CITATION.cff).
    Cff,
}

/// Output options shared by `analyze` and `view`.
#[derive(Debug, Args)]
pub struct OutputArgs {
    #[arg(short, long, value_name = "PATH", help = help::short::OUTPUT, long_help = help::output())]
    pub output: Option<PathBuf>,

    #[arg(short = 'O', long, value_name = "FORMAT", help = help::short::OUTPUT_FORMAT, long_help = help::output_format())]
    pub output_format: Option<FormatArg>,

    #[arg(long, value_name = "TABLE", help = help::short::TABLE, long_help = help::table())]
    pub table: Option<TableArg>,

    #[arg(long, value_name = "DIR", help = help::short::TSV_DIR, long_help = help::tsv_dir())]
    pub tsv_dir: Option<PathBuf>,

    #[arg(long, help = help::short::PRETTY, long_help = help::pretty())]
    pub pretty: bool,
}

/// HCS options shared by `analyze` and `hcs`.
#[derive(Debug, Args)]
pub struct HcsOptions {
    #[arg(long, value_name = "PERCENT", help = help::short::HCS_MIN_INCIDENCE, long_help = help::hcs_min_incidence())]
    pub hcs_min_incidence: Option<String>,

    #[arg(long, help = help::short::HCS_ELIGIBLE_ONLY, long_help = help::hcs_eligible_only())]
    pub hcs_eligible_only: bool,
}

#[derive(Debug, Args)]
#[command(after_long_help = help::footer())]
pub struct AnalyzeArgs {
    /// Aligned FASTA file (plain, .gz, .bz2 or .zst; '-' or omitted = standard input).
    #[arg(help_heading = "Input", short, long, value_name = "FASTA")]
    pub input: Option<PathBuf>,

    #[arg(help_heading = "Method", short, long, value_name = "K", value_parser = parse_count, help = help::short::kmer_length(), long_help = help::kmer_length())]
    pub kmer_length: Option<u64>,

    #[arg(help_heading = "Method", short, long, value_name = "T", value_parser = parse_count, help = help::short::support_threshold(), long_help = help::support_threshold())]
    pub support_threshold: Option<u64>,

    #[arg(help_heading = "Input", short, long, value_enum, value_name = "ALPHABET", default_value_t = AlphabetArg::Auto, help = help::short::ALPHABET, long_help = help::alphabet())]
    pub alphabet: AlphabetArg,

    #[arg(help_heading = "Input", long, value_enum, value_name = "MODE", help = help::short::VALIDATION, long_help = help::validation())]
    pub validation: Option<ValidationArg>,

    #[arg(help_heading = "Method", long, value_enum, value_name = "ESTIMATOR", help = help::short::ESTIMATOR, long_help = help::estimator())]
    pub estimator: Option<EstimatorArg>,

    #[arg(help_heading = "Method", long, help = help::short::SIMILARITY_WEIGHTED_ENTROPY, long_help = help::similarity_weighted_entropy())]
    pub similarity_weighted_entropy: bool,

    #[arg(help_heading = "Method", long, value_name = "X", value_parser = parse_unit_interval, help = help::short::MIN_COVERAGE, long_help = help::min_coverage())]
    pub min_coverage: Option<f64>,

    #[arg(help_heading = "Metadata", long, alias = "name", value_name = "NAME", help = help::short::QUERY_NAME, long_help = help::query_name())]
    pub query_name: Option<String>,

    #[arg(help_heading = "Metadata", long, value_name = "FIELDS", help = help::short::HEADER_FORMAT, long_help = help::header_format())]
    pub header_format: Option<String>,

    #[arg(help_heading = "Metadata", long, value_name = "FIELDS", help = help::short::METADATA_FIELDS, long_help = help::metadata_fields())]
    pub metadata_fields: Option<String>,

    #[arg(help_heading = "Metadata", long = "header-fillna", value_name = "VALUE", help = help::short::header_fill(), long_help = help::header_fill())]
    pub header_fill: Option<String>,

    #[command(flatten, next_help_heading = "Output")]
    pub output: OutputArgs,

    #[arg(help_heading = "Conserved sequences", long, value_name = "PATH", help = help::short::HCS_OUTPUT, long_help = help::hcs_output())]
    pub hcs_output: Option<PathBuf>,

    #[command(flatten, next_help_heading = "Conserved sequences")]
    pub hcs: HcsOptions,

    #[arg(help_heading = "Output", long, value_name = "PATH", help = help::short::RUN_REPORT, long_help = help::run_report())]
    pub run_report: Option<PathBuf>,

    #[arg(help_heading = "Run", short, long, value_name = "N", value_parser = parse_threads, help = help::short::THREADS, long_help = help::threads())]
    pub threads: Option<NonZeroUsize>,

    #[arg(help_heading = "Run", long, value_enum, value_name = "WHEN", default_value_t = ProgressArg::Auto, env = "DIMA_PROGRESS", help = help::short::PROGRESS, long_help = help::progress())]
    pub progress: ProgressArg,

    /// The same as `--progress off` (kept for scripts written before `--progress`).
    #[arg(long, hide = true, env = "DIMA_NO_PROGRESS", value_parser = clap::builder::FalseyValueParser::new())]
    pub no_progress: bool,
}

impl AnalyzeArgs {
    /// The progress display asked for (`--no-progress` means `off`).
    #[must_use]
    pub const fn progress(&self) -> ProgressArg {
        if self.no_progress {
            ProgressArg::Off
        } else {
            self.progress
        }
    }
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    /// Results written by `dima analyze` (JSON, optionally .gz or .zst; '-' = standard input).
    #[arg(short, long, value_name = "RESULTS")]
    pub input: PathBuf,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Debug, Args)]
pub struct HcsArgs {
    /// Results written by `dima analyze` (JSON, optionally .gz or .zst; '-' = standard input).
    #[arg(short, long, value_name = "RESULTS")]
    pub input: PathBuf,

    #[command(flatten)]
    pub hcs: HcsOptions,

    /// Write the regions here ('-' = standard output, the default).
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Format (default: from the extension, else JSON).
    #[arg(short = 'O', long, value_name = "FORMAT")]
    pub output_format: Option<HcsFormatArg>,

    /// Indent JSON output.
    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Args)]
pub struct CiteArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = CiteFormat::Text)]
    pub format: CiteFormat,

    /// Results of a run, and/or the conserved sequences computed from it (repeat the option):
    /// print a methods text and the references of the methods used.
    #[arg(long, value_name = "FILE", action = clap::ArgAction::Append)]
    pub from: Vec<PathBuf>,
}

/// A whole number of at least 1 (k, T).
fn parse_count(s: &str) -> Result<u64, String> {
    match s.trim().parse::<u64>() {
        Ok(v) if v >= 1 => Ok(v),
        _ => Err(format!("{s:?} is not a whole number of at least 1")),
    }
}

/// A thread count of at least 1.
fn parse_threads(s: &str) -> Result<NonZeroUsize, String> {
    s.trim()
        .parse::<usize>()
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| format!("{s:?} is not a whole number of at least 1"))
}

fn parse_unit_interval(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("{s:?} is not a number"))?;
    if (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err(format!("{v} is outside [0, 1]"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn aliases_and_ranges() {
        let parse = |args: &[&str]| Cli::try_parse_from(args);
        let c = parse(&[
            "dima",
            "analyse",
            "-a",
            "nucleotide",
            "--estimator",
            "plug-in",
            "--validation",
            "report-only",
            "--name",
            "X",
        ])
        .unwrap();
        let Some(Command::Analyze(a)) = c.command else {
            panic!()
        };
        assert_eq!(a.alphabet, AlphabetArg::Dna);
        assert_eq!(a.estimator, Some(EstimatorArg::Raw));
        assert_eq!(a.validation, Some(ValidationArg::Report));
        assert_eq!(a.query_name.as_deref(), Some("X"));
        assert!(parse(&["dima", "analyze", "-k", "0"]).is_err());
        assert!(parse(&["dima", "analyze", "-s", "0"]).is_err());
        assert!(parse(&["dima", "analyze", "--min-coverage", "1.5"]).is_err());
        assert!(parse(&["dima", "analyze", "-t", "0"]).is_err());
        assert!(parse(&["dima", "-q", "-v", "analyze"]).is_err());
        let c = parse(&["dima", "analyze", "-qq"]).unwrap();
        assert_eq!(c.global.quiet, 2);
    }
}
