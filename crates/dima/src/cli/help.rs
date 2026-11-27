//! Help texts. Every scientific option ends with the references behind its behaviour, taken
//! from the citation registry (D9, D89, D90); `-h` shows the one-line summaries only.

use dima_core::citations::Cite;
use dima_core::config::{
    DEFAULT_FILL, DEFAULT_KMER_NUCLEOTIDE, DEFAULT_KMER_PROTEIN, DEFAULT_SUPPORT_THRESHOLD,
};

/// Footer of the long help: the publication, how to cite, and the documentation of this build
/// (that of its own version for a release, D113).
pub fn footer() -> String {
    format!(
        "Publication: Tharanga et al. 2025, Brief Bioinform 26(1):bbae607, \
         doi:10.1093/bib/bbae607; cite: dima cite; docs: {}",
        dima_core::citations::DOCS_URL
    )
}

fn refs(cites: &[Cite]) -> String {
    let list: Vec<String> = cites.iter().map(|c| c.reference()).collect();
    format!("\n\nRef: {}", list.join("; "))
}

const fn c(key: &'static str) -> Cite {
    Cite::new(key)
}

/// The shared wording of where the paper's correction applies (D9): one text for the support
/// threshold and the estimator.
const CORRECTED_WHERE_POSSIBLE: &str = "The correction needs at least five points (the full \
     data set included; a minimum taken from DiMA v5.0.9, which the paper does not state), so \
     an eligible position is corrected only when its support is at least 1.25 T (125 sequences \
     for T = 100). With support between T and 1.25 T, and in the one tiny case T = 2, N = 3, it \
     keeps its observed entropy; such positions are still eligible and in the summaries.";

/// Long description of the program.
pub fn about_long() -> String {
    format!(
        "DiMA quantifies the diversity of viral protein or nucleotide alignments: for every \
         k-mer position it counts the distinct k-mers, estimates their Shannon entropy with a \
         correction for sample size, classifies variants into Index, Major, Minor and Unique \
         motifs, and reports their incidence, optionally stratified by FASTA header metadata. \
         On request it also computes their similarity-weighted entropy (Wu 2025), which is not \
         corrected for sample size. \
         Conserved stretches (historically conserved sequences) are reported separately.{}",
        refs(&[c("tharanga2025"), dima_core::similarity::WU2025])
    )
}

pub fn kmer_length() -> String {
    format!(
        "Length k of the overlapping windows (k-mers). Default: {DEFAULT_KMER_PROTEIN} for \
         proteins (the length of peptides presented by HLA class I molecules) and \
         {DEFAULT_KMER_NUCLEOTIDE} for DNA/RNA (the codons of a 9-mer). Any value from 1 to the \
         alignment length is accepted.{}",
        refs(&[c("tharanga2025"), c("falk1991"), c("rammensee1993")])
    )
}

pub fn support_threshold() -> String {
    format!(
        "Support threshold T. A position's support N is the number of sequences with a valid \
         k-mer there. Positions with support above T are eligible: they enter the summary \
         statistics. With the paper's estimator ('dima'), an eligible position's entropy is \
         corrected for sample size where possible. {CORRECTED_WHERE_POSSIBLE} Positions with \
         support 0 (NS), below T (LS) or equal to T (ELS) are labelled and excluded from the \
         summaries; with 'dima' LS and ELS keep their observed entropy. NS has no entropy \
         (null, NA), where the paper assigns a pseudo-entropy of 0: a value that was not \
         observed is missing, not zero. Default: {DEFAULT_SUPPORT_THRESHOLD}, as in the \
         paper.{}",
        refs(&[
            c("tharanga2025").at("Methods and Fig. 2"),
            c("rubin1976"),
            c("silverman2020")
        ])
    )
}

pub fn alphabet() -> String {
    format!(
        "Sequence alphabet. 'auto' detects it from the residues of the first records with the \
         rules of Easel's esl_abc_GuessAlphabet (Easel is the sequence library of HMMER, Eddy \
         2011); when the rules do not decide, DiMA stops and asks for this option instead of \
         guessing. T and U are folded to the alphabet's letter; lower case is folded to upper \
         case. Symbols: gaps - . _, missing ~ ?, stops * $ ^, frameshift #, and ambiguity codes \
         (protein X B J Z; nucleotide R Y K M S W B D H V N, the IUPAC-IUB codes, and X and I, \
         which DiMA also reads as ambiguous) exclude the k-mers that contain them; protein U \
         (selenocysteine) and O (pyrrolysine) are residues.{}",
        refs(&[
            c("eddy2011"),
            c("cornishbowden1985"),
            c("lanlcodes"),
            c("lanlgenecutter"),
            c("bock1991"),
            c("srinivasan2002"),
        ])
    )
}

pub fn validation() -> String {
    "How characters outside the alphabet are handled. 'strict' stops at the first one with \
     its record, line and column; 'permissive' excludes the k-mers that contain them and \
     reports one aggregated warning; 'report' is permissive and also prints the table of every \
     character found. Structural problems (no FASTA, unequal lengths, NUL bytes) always stop \
     the run. Default: strict."
        .to_owned()
}

pub fn estimator() -> String {
    format!(
        "Entropy estimator. At a position, N is its support (the sequences with a valid k-mer \
         there) and D the number of distinct k-mers, the Index included. 'dima' is the paper's \
         correction: the entropies of subsamples of R% of the sequences, n = R*N/100 rounded \
         up, are regressed on 1/n and the intercept is the estimate, where R is every multiple \
         of 5 from 50 to 95 that is at least 100*T/N, plus 100%. It is the intercept expected \
         over the paper's random with-replacement subsampling, computed exactly, so results \
         are reproducible bit for bit; it is the limit of averaging many random subsets, as in \
         the original method. {CORRECTED_WHERE_POSSIBLE} Positions with support up to T keep \
         their observed entropy too. 'chao-wang-jost' (the least biased estimator in its \
         authors' comparison) and 'miller-madow' (which adds (D - 1)/(2N ln 2)) correct every \
         position with at least 2 sequences, low-support ones included; with 1 they give its \
         observed entropy, 0. 'raw' is the observed (plug-in) entropy everywhere. Default: \
         dima.{}",
        refs(&[
            c("tharanga2025").at("Fig. 2"),
            c("shannon1948"),
            c("rao1945"),
            c("blackwell1947"),
            c("khan2017"),
            c("chao2013"),
            c("miller1955"),
            c("paninski2003"),
        ])
    )
}

pub fn similarity_weighted_entropy() -> String {
    format!(
        "{}\n\n{}\n\n{} The values are written only to the results (JSON, JSONL) and the \
         positions table, and are null (NA) without this option.{}",
        dima_core::similarity::DESCRIPTION,
        dima_core::similarity::EVIDENCE,
        dima_core::similarity::sample_size(),
        refs(&dima_core::similarity::REFERENCES)
    )
}

pub fn min_coverage() -> String {
    format!(
        "Flag positions whose estimated sample coverage (the probability that one more sequence \
         carries an already-seen k-mer) is below this value, between 0 and 1. Coverage is \
         estimated with Chao and Jost's formula and, where no k-mer is seen exactly twice, with \
         the form of Chao, Wang and Jost. It is always reported; this option only adds the flag \
         and a warning.{}",
        refs(&[
            c("chao2012").at("Eq. 4a"),
            c("chao2013").at("Materials and methods")
        ])
    )
}

pub fn query_name() -> String {
    "Name of the analysed protein or gene. Default: the input file name without its FASTA and \
     compression extensions; 'Unknown Protein/DNA/RNA' for standard input, /dev/stdin and \
     process substitution (<(...))."
        .to_owned()
}

pub fn header_format() -> String {
    format!(
        "Names of the '|'-separated fields of the FASTA headers, e.g. \
         'Accession|Host|Country|Year'. Variants are then counted per value of each field. \
         Values are cleaned (invisible characters removed, white space trimmed, Unicode NFC) and \
         empty values get the fill value.{}",
        refs(&[c("tharanga2025"), c("uax15"), c("charmodnorm")])
    )
}

pub fn metadata_fields() -> String {
    "The header fields to stratify variants by (default: all fields of --header-format), \
     e.g. 'Host|Year'. Identifier-like fields (such as accessions) multiply the size of the \
     results; DiMA warns about them."
        .to_owned()
}

pub fn header_fill() -> String {
    format!(
        "Value used for empty header fields and for headers whose field count differs from \
         --header-format (permissive mode). Default: {DEFAULT_FILL}."
    )
}

pub fn output() -> String {
    "Write the results here ('-' = standard output). The format follows -O or the file \
     extension: .json, .jsonl/.ndjson or .tsv/.tab, optionally followed by .gz or .zst for \
     compression; a name without one of these extensions needs -O. A name that asks for \
     another compression (.bz2, .xz, .zip, ...) or for two, or that is a folder, is refused \
     before anything runs. Without -o or --tsv-dir (with 'dima analyze', also --hcs-output), \
     JSON goes to standard output. Files are replaced atomically, and never one that another \
     output, the run report or the input already uses."
        .to_owned()
}

pub fn output_format() -> String {
    "Format of -o: 'json' (complete results with provenance), 'jsonl' (one position per line) \
     or 'tsv' (one table; choose it with --table). Must agree with the file extension."
        .to_owned()
}

pub fn table() -> String {
    format!(
        "The table written by -O tsv: positions, variants, metadata or hcs. Tables are tidy (one \
         row per observation), tab-separated with RFC 4180 quoting, and use NA for undefined or \
         not computed values.{}",
        refs(&[c("wickham2014"), c("broman2018"), c("rfc4180")])
    )
}

pub fn tsv_dir() -> String {
    "Write all four tables to this folder (created if needed) as NAME.positions.tsv, \
     NAME.variants.tsv, NAME.metadata.tsv and NAME.hcs.tsv. NAME is the query name made safe \
     for file names: characters other than Unicode letters, digits, '.', '-' and '_' become \
     '_', leading and trailing '.' and '_' are removed, and an empty name becomes 'dima'. With \
     'dima analyze' the query name is --query-name, else the input file name without its FASTA \
     and compression extensions, and 'dima' for standard input, /dev/stdin and <(...). With \
     'dima view' it is the query name in the results ('Unknown_Protein', 'Unknown_DNA' or \
     'Unknown_RNA' for results analysed from standard input)."
        .to_owned()
}

pub fn pretty() -> String {
    "Indent JSON output (it is compact by default, and indented when written to a terminal)."
        .to_owned()
}

/// How a file of conserved sequences is named, for `--hcs-output` and `dima hcs -o`.
const HCS_NAMES: &str = "The name gives the format: .json or .tsv, optionally followed by \
     .gz or .zst. A file name with neither gets JSON, with a warning (output_format_assumed); \
     standard output and other streams get JSON. A name that asks for JSON Lines, for another \
     compression (.bz2, .xz, ...) or for two, or that is a folder, is refused.";

pub fn hcs_output() -> String {
    format!(
        "Also write the historically conserved sequences here. {HCS_NAMES} Consecutive \
         positions whose most common (Index) k-mer reaches --hcs-min-incidence are joined when \
         the k-mers overlap or abut; regions that include positions with support at most T \
         (the support threshold of the analysis) are flagged, and each region reports the \
         lowest per-position exact 95% Clopper-Pearson lower bound among its members.{}",
        refs(&[
            c("tharanga2025"),
            c("raman2020"),
            c("hu2013"),
            c("clopper1934")
        ])
    )
}

pub fn hcs_target() -> String {
    format!(
        "Write the conserved sequences here ('-' = standard output, the default). {HCS_NAMES} \
         -O chooses the format instead. Files are replaced atomically, and never the input."
    )
}

pub fn hcs_format() -> String {
    "Format of -o: 'json' (regions with their members and provenance) or 'tsv' (one row per \
     region). Default: from the file extension; a file name without one gets JSON, with a \
     warning, and standard output gets JSON. Must agree with the file extension."
        .to_owned()
}

pub fn hcs_min_incidence() -> String {
    format!(
        "Minimum Index incidence (%) of the positions that form conserved sequences, above 50 \
         and at most 100 (above 50% the Index k-mer is unique): a decimal number such as 95 or \
         99.5, with at most 9 decimal places and no % sign or exponent, compared exactly. \
         Default: 100, the paper's (every sequence with a valid k-mer has the Index).{}",
        refs(&[c("tharanga2025").at("Methods")])
    )
}

pub fn hcs_eligible_only() -> String {
    format!(
        "Build conserved sequences from positions with support above T only, where T is the \
         support threshold of the analysis (recorded in the results); positions with support \
         at most T end a region instead of being flagged.{}",
        refs(&[c("hu2013"), c("castresana2000")])
    )
}

pub fn run_report() -> String {
    format!(
        "Also write a run report here: version and build target, the command (file names \
         only), status, start and end time (UTC), threads and where their number came from, \
         platform, time per phase, CPU time, peak memory, and the size and SHA-256 of the input \
         and of every output written. It is separate from the results, which stay identical \
         from run to run, and holds no host or user names and no paths. It is always JSON, \
         compressed when the name ends in .gz or .zst; a name that asks for a TSV table, JSON \
         Lines or another compression, or that is a folder, is refused. '-' writes it to \
         standard output when no other output goes there. It is checked and opened before \
         anything runs, and written even when the run fails or is interrupted.{}",
        refs(&[c("taschuk2017").at("Rule 10"), c("khan2019"), c("leo2024")])
    )
}

pub fn threads() -> String {
    "Worker threads. Default: RAYON_NUM_THREADS if set, otherwise the CPUs this process may \
     use, limited to the CPUs granted by Slurm, PBS, SGE or LSF when running under one. Results \
     do not depend on the number of threads."
        .to_owned()
}

pub fn progress() -> String {
    "How progress is shown on standard error. 'auto': on a terminal that can redraw a line, one \
     line updated in place; elsewhere nothing, or plain lines with -v. 'plain': appended lines, \
     one per phase and the progress at most every 10 seconds (at each 10%) and at least once a \
     minute; recommended with screen readers, which read every update of a redrawn line, and \
     for log files. 'off': nothing. -q always hides progress. Without this option: 'off' when \
     DIMA_NO_PROGRESS is set to anything but an empty value, 0, false, no, off, n or f; \
     otherwise DIMA_PROGRESS (auto, plain or off; for example DIMA_PROGRESS=plain in a shell \
     profile), where an empty value counts as unset; otherwise 'auto'. Values are lower case, \
     as for every option."
        .to_owned()
}

pub fn quiet() -> String {
    "Print errors only: no warnings, notes, progress, summary line, Cite: line or --validation \
     report table. Errors, including the message of an interrupted run, are always printed. \
     Giving it twice is the same as once."
        .to_owned()
}

pub fn verbose() -> String {
    "Print more on standard error. With 'dima analyze', -v adds: the number of worker threads \
     and where it came from (a scheduler's allocation can name this host, as in \
     PE_HOSTFILE[node12]=8), a note when OMP_NUM_THREADS is lower (DiMA does not use it), \
     informational notes (note[...]), 'wrote FILE (SIZE)' for each file written and, before \
     the summary line, the time of each phase (with a note when reading the input limited the \
     run), the CPU time and the peak memory. Where standard error cannot redraw a line, -v \
     also prints the phase lines of --progress plain. -vv adds debugging messages, and prints \
     plain progress lines instead of the redrawn line on a terminal. With view, hcs, cite and \
     completions, -v adds nothing and -vv adds debugging messages. DIMA_LOG accepts a tracing \
     filter such as 'dima_core=debug', which replaces the level -q and -v choose."
        .to_owned()
}

/// One-line summaries for `-h` (`--help` shows the full texts above).
pub mod short {
    use dima_core::config::{
        DEFAULT_FILL, DEFAULT_KMER_NUCLEOTIDE, DEFAULT_KMER_PROTEIN, DEFAULT_SUPPORT_THRESHOLD,
    };

    pub const OUTPUT: &str = "Write the results here ('-' = standard output)";
    pub const OUTPUT_FORMAT: &str = "Format of -o [default: from the extension]";
    pub const TABLE: &str = "Table written by -O tsv";
    pub const TSV_DIR: &str = "Write all four TSV tables to this folder";
    pub const PRETTY: &str = "Indent JSON output";
    pub const HCS_MIN_INCIDENCE: &str =
        "Minimum Index incidence of conserved positions, above 50 and at most 100 [default: 100]";
    pub const HCS_ELIGIBLE_ONLY: &str =
        "Build conserved sequences from positions with support above T only";
    pub const HCS_TARGET: &str =
        "Write the conserved sequences here ('-' = standard output, the default)";
    pub const HCS_FORMAT: &str = "Format of -o [default: from the extension, else json]";
    pub const VALIDATION: &str =
        "How characters outside the alphabet are handled [default: strict]";
    pub const ESTIMATOR: &str = "Entropy estimator [default: dima]";
    pub const SIMILARITY_WEIGHTED_ENTROPY: &str =
        "Also compute similarity-weighted entropy (Wu 2025) and normalised entropies";
    pub const MIN_COVERAGE: &str = "Flag positions whose sample coverage is below X (0 to 1)";
    pub const QUERY_NAME: &str = "Name of the protein or gene [default: from the file name]";
    pub const HEADER_FORMAT: &str =
        "Names of the '|'-separated header fields, e.g. 'Accession|Host|Year'";
    pub const METADATA_FIELDS: &str =
        "Header fields to stratify variants by [default: all of --header-format]";
    pub const HCS_OUTPUT: &str =
        "Also write the historically conserved sequences here (.json or .tsv)";
    pub const RUN_REPORT: &str =
        "Also write a run report here (JSON: timings, resources, checksums)";
    pub const THREADS: &str = "Worker threads [default: the CPUs available to this job]";
    pub const PROGRESS: &str =
        "How progress is shown: auto, plain (screen readers, logs) or off [default: auto]";
    pub const QUIET: &str = "Print errors only";
    pub const VERBOSE: &str =
        "Print threads, notes, files written and timings (-vv: also debugging messages)";
    pub const ALPHABET: &str = "Sequence alphabet";

    pub fn kmer_length() -> String {
        format!(
            "k-mer length [default: {DEFAULT_KMER_PROTEIN} for proteins, \
             {DEFAULT_KMER_NUCLEOTIDE} for DNA/RNA]"
        )
    }

    pub fn support_threshold() -> String {
        format!("Support threshold T [default: {DEFAULT_SUPPORT_THRESHOLD}]")
    }

    pub fn header_fill() -> String {
        format!("Value for empty or mismatched header fields [default: {DEFAULT_FILL}]")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_citation_in_help_exists() {
        for text in [
            super::about_long(),
            super::kmer_length(),
            super::support_threshold(),
            super::alphabet(),
            super::estimator(),
            super::similarity_weighted_entropy(),
            super::min_coverage(),
            super::header_format(),
            super::table(),
            super::hcs_output(),
            super::hcs_min_incidence(),
            super::hcs_eligible_only(),
            super::run_report(),
        ] {
            assert!(text.contains("\n\nRef: "), "{text}");
        }
    }

    /// The support threshold and the estimator say alike where the correction applies, with
    /// the exact rule (corrected only from 1.25 T) and the source of the minimum.
    #[test]
    fn the_correction_is_described_alike() {
        for text in [super::support_threshold(), super::estimator()] {
            for phrase in [
                "at least five points",
                "a minimum taken from DiMA v5.0.9, which the paper does not state",
                "at least 1.25 T (125 sequences for T = 100)",
                "T = 2, N = 3",
                "still eligible and in the summaries",
            ] {
                assert!(text.contains(phrase), "{phrase:?} in {text}");
            }
        }
        let e = super::estimator();
        for defined in [
            "N is its support",
            "D the number of distinct k-mers",
            "n = R*N/100",
        ] {
            assert!(e.contains(defined), "{defined:?} in {e}");
        }
    }
}
