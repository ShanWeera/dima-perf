//! Tooltips of the analysis options and chart encodings. Each one that describes scientific
//! behaviour ends with its references, taken from the library's citation registry (D9, D89,
//! D90): short author–year form with the specific figure or rule, never a hard-coded string.

use dima_core::citations::Cite;
use dima_core::config::{
    DEFAULT_FILL, DEFAULT_KMER_NUCLEOTIDE, DEFAULT_KMER_PROTEIN, DEFAULT_SUPPORT_THRESHOLD,
    Estimator,
};

/// `"\n\nRef: A 2020; B 2021"` for the given citations.
#[must_use]
pub fn refs(cites: &[Cite]) -> String {
    let list: Vec<String> = cites.iter().map(|c| c.short()).collect();
    format!("\n\nRef: {}", list.join("; "))
}

const fn c(key: &'static str) -> Cite {
    Cite::new(key)
}

/// An export (its tooltip), with the references of the format as in the command line's help.
#[must_use]
pub fn export(kind: crate::workers::export::ExportKind) -> String {
    use crate::workers::export::ExportKind as K;
    const TABLES: [Cite; 3] = [c("wickham2014"), c("broman2018"), c("rfc4180")];
    const HCS: [Cite; 4] = [
        c("tharanga2025"),
        c("raman2020"),
        c("hu2013"),
        c("clopper1934"),
    ];
    match kind {
        K::ResultsJson => "The complete results with provenance, as written by `dima analyze`. \
                           Name the file .json.gz or .json.zst to compress it. The app can open \
                           it again."
            .to_owned(),
        K::Jsonl => "One position per line; provenance and summaries are in the JSON.".to_owned(),
        K::TsvFolder => format!(
            "Tidy tables <name>.positions/variants/metadata/hcs.tsv, as `dima analyze \
             --tsv-dir` writes them: one row per observation, tab-separated with RFC 4180 \
             quoting, NA for undefined values.{}",
            refs(&TABLES)
        ),
        K::HcsJson => format!(
            "The conserved regions at the current threshold, with their flags and the 95% \
             Clopper-Pearson lower bound of their conservation.{}",
            refs(&HCS)
        ),
        K::HcsTsv => format!(
            "The conserved regions at the current threshold, one row per region.{}",
            refs(&[HCS.as_slice(), TABLES.as_slice()].concat())
        ),
        K::Png => "The entropy chart as shown (filters and zoom), downsampled to the image \
                   width."
            .to_owned(),
        K::Svg => {
            "The entropy chart as shown (filters and zoom), every point, editable text.".to_owned()
        }
        K::RunReport => format!(
            "How the analysis ran: version, equivalent command, threads, phase times, platform, \
             and the size and SHA-256 of the input and of the exported files. No paths, user or \
             host names.{}",
            refs(&[c("taschuk2017").at("Rule 10"), c("khan2019"), c("leo2024")])
        ),
        K::Methods => {
            "A methods paragraph for this analysis and the references it used, as `dima cite \
             --from` prints them."
                .to_owned()
        }
        K::Bibtex => "The references this analysis used, for a reference manager.".to_owned(),
    }
}

/// Alphabet.
#[must_use]
pub fn alphabet() -> String {
    format!(
        "Sequence alphabet. Auto detects it from the first records with Easel's rules (as in \
         HMMER), which make no false calls and otherwise ask you to choose. T and U are folded to \
         the alphabet's letter and lower case to upper case. Gaps (- . _), missing data (~ ?), \
         stops (* $ ^), frameshifts (#) and ambiguity codes exclude the k-mers that contain them; \
         protein U and O are residues.{}",
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

/// k-mer length.
#[must_use]
pub fn kmer_length() -> String {
    format!(
        "Length k of the overlapping windows. Default: {DEFAULT_KMER_PROTEIN} for proteins (the \
         length of peptides presented by HLA class I) and {DEFAULT_KMER_NUCLEOTIDE} for DNA/RNA \
         (the codons of a 9-mer). Any value from 1 to the alignment length.{}",
        refs(&[c("tharanga2025"), c("falk1991"), c("rammensee1993")])
    )
}

/// Support threshold.
#[must_use]
pub fn support_threshold() -> String {
    format!(
        "Support threshold T. A position's support is the number of sequences with a valid k-mer \
         there. Positions with support above T are eligible: they enter the summaries, and the \
         DiMA estimator corrects their entropy for sample size. Support 0 (NS), below T (LS) and \
         equal to T (ELS) are labelled and excluded from the summaries; the DiMA estimator \
         reports the observed entropy at LS and ELS. NS has no entropy, where the paper assigns \
         a pseudo-entropy of 0: a value that was not observed is missing, not zero. Default: \
         {DEFAULT_SUPPORT_THRESHOLD}, as in the paper.{}",
        refs(&[
            c("tharanga2025").at("Methods"),
            c("rubin1976"),
            c("silverman2020")
        ])
    )
}

/// Query name.
#[must_use]
pub fn query_name() -> String {
    "Name of the analysed protein or gene, used in titles and file names. Default: the file \
     name without its FASTA and compression extensions."
        .to_owned()
}

/// Header format.
#[must_use]
pub fn header_format() -> String {
    format!(
        "Name the '|'-separated fields of the FASTA headers to count every variant per value of \
         each stratified field (for example per host or per year). Values are cleaned (invisible \
         characters removed, white space trimmed, Unicode NFC); empty values get the fill value. \
         Identifier-like fields (all values different) multiply the size of the results.{}",
        refs(&[c("tharanga2025"), c("uax15"), c("charmodnorm")])
    )
}

/// HCS threshold.
#[must_use]
pub fn hcs_threshold() -> String {
    format!(
        "Historically conserved sequences: consecutive positions whose most common (Index) k-mer \
         reaches this incidence, joined where the k-mers overlap or abut. Above 50% and at most \
         100% (compared exactly). Regions that include positions with support \u{2264} T are \
         flagged (*), and each reports the exact 95% Clopper\u{2013}Pearson lower bound of its \
         conservation. Default: 100%, the paper's (every sequence with a valid k-mer has the \
         Index).{}",
        refs(&[
            c("tharanga2025"),
            c("raman2020"),
            c("hu2013"),
            c("clopper1934"),
        ])
    )
}

/// Eligible-only HCS.
#[must_use]
pub fn hcs_eligible_only() -> String {
    format!(
        "Build conserved regions from positions with support above T only; positions with \
         support \u{2264} T end a region instead of being flagged.{}",
        refs(&[c("hu2013"), c("castresana2000")])
    )
}

/// Estimator.
#[must_use]
pub fn estimator() -> String {
    format!(
        "Entropy estimator. DiMA is the paper's correction: entropies of subsamples are regressed \
         on 1/n and the intercept is the estimate, computed as the exact expectation of the \
         paper's random subsampling (reproducible; the limit of averaging many random subsets, \
         as in the original method); positions \
         with support \u{2264} T keep the observed entropy, as in the paper. \
         Chao\u{2013}Wang\u{2013}Jost (the least biased in its authors' comparison) and \
         Miller\u{2013}Madow (which adds (D \u{2212} 1)/(2N ln 2)) apply to every position with \
         at least 2 sequences; Raw is the observed entropy everywhere.{}",
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

/// Validation mode.
#[must_use]
pub fn validation() -> String {
    "How characters outside the alphabet are handled. Strict stops at the first one with its \
     record, line and column; Permissive excludes the k-mers that contain them and reports one \
     warning; Report also lists every character found (Diagnostics). Structural problems \
     (unequal lengths, NUL bytes) always stop the run."
        .to_owned()
}

/// Minimum coverage.
#[must_use]
pub fn min_coverage() -> String {
    format!(
        "Flag positions whose estimated sample coverage (the probability that one more sequence \
         carries an already seen k-mer) is below this value. Coverage is always reported; this \
         adds the flag, a chart mark and a warning.{}",
        refs(&[c("chao2012").at("Eq. 4a")])
    )
}

/// Fill value.
#[must_use]
pub fn fill() -> String {
    format!(
        "Value for empty header fields and, in permissive mode, for headers whose field count \
         differs from the format. Must not be empty. Default: {DEFAULT_FILL}."
    )
}

/// Threads.
#[must_use]
pub fn threads() -> String {
    "Worker threads for the analysis. Default: all CPUs but one, so the app stays responsive. \
     Results do not depend on the number of threads."
        .to_owned()
}

/// The chart's encoding (legend tooltip) for a run's threshold and estimator.
#[must_use]
pub fn chart_encoding(t: u64, estimator: Estimator) -> String {
    let lines = match estimator {
        Estimator::Dima => format!(
            "Solid blue: corrected entropy at positions with support > {t}. Dashed vermillion: \
             the observed (uncorrected) entropy at positions with support \u{2264} {t} (LS, ELS)."
        ),
        Estimator::ChaoWangJost | Estimator::MillerMadow | Estimator::Raw => format!(
            "Solid blue: positions with support > {t}, which enter the summaries. Dashed \
             vermillion: positions with support \u{2264} {t} (LS, ELS), estimated the same way \
             from fewer sequences and left out of the summaries."
        ),
    };
    format!(
        "{lines} Hatched bands: positions without usable support, which have no entropy (they \
         are never drawn as 0). The line breaks there and where filters hide positions. Long \
         ranges are drawn with M4 downsampling, which keeps every peak and trough.{}",
        refs(&[
            c("tharanga2025"),
            c("wong2011"),
            c("weiskopf2022"),
            c("kerpedjiev2018"),
            c("kent2010"),
            c("jugel2014"),
        ])
    )
}

#[cfg(test)]
mod tests {
    use super::Estimator;

    #[test]
    fn every_citation_exists() {
        // `Cite::short` panics on an unknown key, so building every text checks them all.
        for text in [
            super::alphabet(),
            super::kmer_length(),
            super::support_threshold(),
            super::header_format(),
            super::hcs_threshold(),
            super::hcs_eligible_only(),
            super::estimator(),
            super::min_coverage(),
            super::chart_encoding(100, Estimator::Dima),
            super::chart_encoding(100, Estimator::Raw),
        ] {
            assert!(text.contains("\n\nRef: "), "{text}");
        }
        assert!(super::support_threshold().contains("Tharanga et al. 2025, Methods"));
        for kind in crate::workers::export::ExportKind::ALL {
            assert!(!super::export(kind).is_empty());
        }
        use crate::workers::export::ExportKind as K;
        for kind in [K::TsvFolder, K::HcsJson, K::HcsTsv, K::RunReport] {
            assert!(super::export(kind).contains("\n\nRef: "), "{kind:?}");
        }
    }
}
