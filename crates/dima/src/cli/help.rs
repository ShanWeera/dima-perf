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

/// Long description of the program.
pub fn about_long() -> String {
    format!(
        "DiMA quantifies the diversity of viral protein or nucleotide alignments: for every \
         k-mer position it counts the distinct k-mers, estimates their Shannon entropy with a \
         correction for sample size, classifies variants into Index, Major, Minor and Unique \
         motifs, and reports their incidence, optionally stratified by FASTA header metadata. \
         Conserved stretches (historically conserved sequences) are reported separately.{}",
        refs(&[c("tharanga2025")])
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
        "Support threshold T. A position's support is the number of sequences with a valid \
         k-mer there. Positions with support above T are eligible: they enter the summary \
         statistics, and the 'dima' estimator corrects their entropy for sample size. Positions \
         with support 0 (NS), below T (LS) or equal to T (ELS) are labelled and excluded from \
         the summaries; with 'dima' LS and ELS keep their observed entropy. NS has no entropy \
         (null, NA), where the paper assigns a pseudo-entropy of 0: a value that was not \
         observed is missing, not zero. Default: {DEFAULT_SUPPORT_THRESHOLD}, as in the \
         paper.{}",
        refs(&[
            c("tharanga2025").at("Methods"),
            c("rubin1976"),
            c("silverman2020")
        ])
    )
}

pub fn alphabet() -> String {
    format!(
        "Sequence alphabet. 'auto' detects it from the first records with Easel's rules (as \
         in HMMER), which make no false calls and otherwise ask for this option. T and U are \
         folded to the alphabet's letter; lower case is folded to upper case. Symbols: gaps \
         - . _, missing ~ ?, stops * $ ^, frameshift #, and ambiguity codes (protein X B J Z; \
         nucleotide R Y K M S W B D H V N X I) exclude the k-mers that contain them; protein \
         U (selenocysteine) and O (pyrrolysine) are residues.{}",
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
        "Entropy estimator. 'dima' is the paper's correction: entropies of subsamples of R% of \
         the sequences (R is every multiple of 5 from 50 to 95 that is at least 100*T/N, plus \
         100%) are regressed on 1/n and the intercept is the estimate; positions with support \
         up to T keep the observed entropy. It is the intercept expected over the paper's \
         random with-replacement subsampling, computed exactly, so results are reproducible bit \
         for bit; it is the limit of averaging many random subsets, as in the original method. \
         'chao-wang-jost' (the least biased estimator in its authors' comparison) and \
         'miller-madow' (which adds (D - 1)/(2N ln 2)) apply to every position with at least \
         2 sequences; 'raw' is the observed (plug-in) entropy everywhere. Default: dima.{}",
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

pub fn min_coverage() -> String {
    format!(
        "Flag positions whose estimated sample coverage (the probability that one more sequence \
         carries an already-seen k-mer) is below this value, between 0 and 1. Coverage is always \
         reported; this only adds the flag and a warning.{}",
        refs(&[c("chao2012").at("Eq. 4a")])
    )
}

pub fn query_name() -> String {
    "Name of the analysed protein or gene. Default: the input file name without its FASTA and \
     compression extensions; 'Unknown Protein/DNA/RNA' for standard input."
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
     compression. Without -o or --tsv-dir (with 'dima analyze', also --hcs-output), JSON goes \
     to standard output. Files are replaced atomically."
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
         row per observation), tab-separated with RFC 4180 quoting, and use NA for undefined \
         values.{}",
        refs(&[c("wickham2014"), c("broman2018"), c("rfc4180")])
    )
}

pub fn tsv_dir() -> String {
    "Write all four tables to this directory (created if needed) as \
     <name>.positions.tsv, <name>.variants.tsv, <name>.metadata.tsv and <name>.hcs.tsv."
        .to_owned()
}

pub fn pretty() -> String {
    "Indent JSON output (it is compact by default, and indented when written to a terminal)."
        .to_owned()
}

pub fn hcs_output() -> String {
    format!(
        "Also write the historically conserved sequences here (.json or .tsv, optionally .gz or \
         .zst). Consecutive positions whose most common (Index) k-mer reaches \
         --hcs-min-incidence are joined when the k-mers overlap or abut; regions that include \
         positions with support at most T are flagged, and each region reports the exact 95% \
         Clopper-Pearson lower bound of its conservation.{}",
        refs(&[
            c("tharanga2025"),
            c("raman2020"),
            c("hu2013"),
            c("clopper1934")
        ])
    )
}

pub fn hcs_min_incidence() -> String {
    format!(
        "Minimum Index incidence (%) of the positions that form conserved sequences, above 50 \
         and at most 100 (above 50% the Index k-mer is unique). Compared exactly. Default: 100, \
         the paper's (every sequence with a valid k-mer has the Index).{}",
        refs(&[c("tharanga2025").at("Methods")])
    )
}

pub fn hcs_eligible_only() -> String {
    format!(
        "Build conserved sequences from positions with support above T only; positions with \
         support at most T end a region instead of being flagged.{}",
        refs(&[c("hu2013"), c("castresana2000")])
    )
}

pub fn run_report() -> String {
    format!(
        "Also write a run report here (JSON): version and build target, the command (file names \
         only), status, start and end time (UTC), threads and where their number came from, \
         platform, time per phase, CPU time, peak memory, and the size and SHA-256 of the input \
         and every output. It is separate from the results, which stay identical from run to \
         run, and holds no host or user names and no paths.{}",
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
     for log files. 'off': nothing. -q always hides progress. Also DIMA_PROGRESS (for example \
     DIMA_PROGRESS=plain in a shell profile)."
        .to_owned()
}

pub fn quiet() -> String {
    "Print errors only (no warnings, no summary line).".to_owned()
}

pub fn verbose() -> String {
    "Print progress milestones and a performance report (-v); debugging details (-vv). \
     DIMA_LOG accepts a tracing filter such as 'dima_core=debug'."
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
    pub const TSV_DIR: &str = "Write all four TSV tables to this directory";
    pub const PRETTY: &str = "Indent JSON output";
    pub const HCS_MIN_INCIDENCE: &str =
        "Minimum Index incidence of conserved positions, above 50 and at most 100 [default: 100]";
    pub const HCS_ELIGIBLE_ONLY: &str =
        "Build conserved sequences from positions with support above T only";
    pub const VALIDATION: &str =
        "How characters outside the alphabet are handled [default: strict]";
    pub const ESTIMATOR: &str = "Entropy estimator [default: dima]";
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
    pub const PROGRESS: &str = "How progress is shown: auto, plain (screen readers, logs) or off";
    pub const QUIET: &str = "Print errors only";
    pub const VERBOSE: &str = "Print progress milestones and a performance report (-vv: debug)";
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
}
