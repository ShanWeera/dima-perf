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

impl Command {
    /// The subcommand's name on the command line.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Analyze(_) => "analyze",
            Self::View(_) => "view",
            Self::Hcs(_) => "hcs",
            Self::Cite(_) => "cite",
            Self::Completions { .. } => "completions",
        }
    }
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

    /// The progress display; `None` leaves it to the environment ([`AnalyzeArgs::progress`]).
    #[arg(help_heading = "Run", long, value_enum, value_name = "WHEN", overrides_with = "no_progress", help = help::short::PROGRESS, long_help = help::progress())]
    pub progress: Option<ProgressArg>,

    /// The same as `--progress off` (kept for scripts written before `--progress`).
    #[arg(long, hide = true, overrides_with = "progress")]
    pub no_progress: bool,
}

impl AnalyzeArgs {
    /// The progress display asked for (D109): `--progress` or `--no-progress` (`off`),
    /// whichever is given last; otherwise `off` when `DIMA_NO_PROGRESS` is set to a true value
    /// (the older variable keeps its meaning); otherwise `DIMA_PROGRESS`, where an empty value
    /// counts as unset; otherwise `auto`. `env` reads a variable. Only `dima analyze` shows
    /// progress, so only it reads the variables.
    ///
    /// # Errors
    /// A usage error when `DIMA_PROGRESS` holds anything but `auto`, `plain` or `off`. The value
    /// is not repeated: messages end up in run reports, which never hold environment values
    /// (D103).
    pub fn progress(
        &self,
        env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
    ) -> dima_core::Result<ProgressArg> {
        if let Some(p) = self.progress {
            return Ok(p);
        }
        if self.no_progress || env("DIMA_NO_PROGRESS").is_some_and(|v| truthy(&v)) {
            return Ok(ProgressArg::Off);
        }
        match env("DIMA_PROGRESS") {
            None => Ok(ProgressArg::Auto),
            Some(v) if v.is_empty() => Ok(ProgressArg::Auto),
            Some(v) => v
                .to_str()
                .and_then(|v| ProgressArg::from_str(v, false).ok())
                .ok_or_else(|| {
                    dima_core::DimaError::usage(
                        "DIMA_PROGRESS is set, but not to a progress mode (auto, plain or off)",
                    )
                    .with_hint("set it to auto, plain or off, in lower case, or unset it")
                }),
        }
    }
}

/// A variable that is set and not false: anything but an empty value or `0`, `false`, `no`,
/// `off`, `n` or `f` in any letter case (the rule of clap's `FalseyValueParser`, which read
/// `DIMA_NO_PROGRESS` before).
fn truthy(v: &std::ffi::OsStr) -> bool {
    const FALSE: [&str; 6] = ["n", "no", "f", "false", "off", "0"];
    match v.to_str() {
        Some(s) => !s.is_empty() && !FALSE.iter().any(|f| s.eq_ignore_ascii_case(f)),
        None => true,
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

    #[arg(short, long, value_name = "PATH", help = help::short::HCS_TARGET, long_help = help::hcs_target())]
    pub output: Option<PathBuf>,

    #[arg(short = 'O', long, value_name = "FORMAT", help = help::short::HCS_FORMAT, long_help = help::hcs_format())]
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

    /// Results of a run, and/or the conserved sequences computed from them (give the option
    /// twice; '-' reads standard input): print a methods text and the references of the methods
    /// used, or with --format bibtex, ris or cff the references only. Both files must come
    /// from one run: conserved sequences of another input, k or T are refused.
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

    /// B7: the option (the last of --progress and --no-progress), then DIMA_NO_PROGRESS, then
    /// DIMA_PROGRESS (empty = unset), then auto.
    #[test]
    fn progress_order() {
        let analyze = |args: &[&str]| {
            let all: Vec<&str> = ["dima", "analyze"].iter().chain(args).copied().collect();
            match Cli::try_parse_from(all).unwrap().command {
                Some(Command::Analyze(a)) => a,
                _ => panic!(),
            }
        };
        let with = |args: &[&str], env: &[(&str, &str)]| {
            let env: Vec<(String, std::ffi::OsString)> = env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).into()))
                .collect();
            analyze(args).progress(&|k| {
                env.iter()
                    .find(|(name, _)| name == k)
                    .map(|(_, v)| v.clone())
            })
        };
        use ProgressArg::{Auto, Off, Plain};
        assert_eq!(with(&[], &[]).unwrap(), Auto);
        assert_eq!(with(&[], &[("DIMA_PROGRESS", "")]).unwrap(), Auto);
        assert_eq!(with(&[], &[("DIMA_PROGRESS", "plain")]).unwrap(), Plain);
        assert!(with(&[], &[("DIMA_PROGRESS", "PLAIN")]).is_err());
        assert!(with(&[], &[("DIMA_PROGRESS", "x")]).is_err());
        for no in ["1", "yes", "anything"] {
            assert_eq!(
                with(&[], &[("DIMA_NO_PROGRESS", no), ("DIMA_PROGRESS", "plain")]).unwrap(),
                Off,
                "{no}"
            );
        }
        for not in ["", "0", "false", "No", "OFF", "n", "f"] {
            assert_eq!(
                with(
                    &[],
                    &[("DIMA_NO_PROGRESS", not), ("DIMA_PROGRESS", "plain")]
                )
                .unwrap(),
                Plain,
                "{not:?}"
            );
        }
        // An explicit option beats both variables, even an invalid one.
        let env = [("DIMA_NO_PROGRESS", "1"), ("DIMA_PROGRESS", "x")];
        assert_eq!(with(&["--progress", "plain"], &env).unwrap(), Plain);
        assert_eq!(with(&["--no-progress"], &[]).unwrap(), Off);
        assert_eq!(
            with(&["--progress", "plain", "--no-progress"], &[]).unwrap(),
            Off
        );
        assert_eq!(
            with(&["--no-progress", "--progress", "plain"], &[]).unwrap(),
            Plain
        );
        let e = with(&[], &[("DIMA_PROGRESS", "sparkles")]).unwrap_err();
        assert_eq!(e.class().code(), 2);
        assert!(!e.to_string().contains("sparkles"), "{e}");
    }
}
