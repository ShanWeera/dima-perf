//! Tooltips of the analysis options and chart encodings. Each one that describes scientific
//! behaviour ends with its references, taken from the library's citation registry (D9, D89,
//! D90): short author–year form with the specific figure or rule, never a hard-coded string.

use dima_core::citations::Cite;
use dima_core::config::{
    DEFAULT_FILL, DEFAULT_KMER_NUCLEOTIDE, DEFAULT_KMER_PROTEIN, DEFAULT_SUPPORT_THRESHOLD,
    Estimator,
};

use crate::charts::metric::ChartMetric;

/// `"\n\nRef: A 2020; B 2021"` for the given citations.
#[must_use]
pub fn refs(cites: &[Cite]) -> String {
    let list: Vec<String> = cites.iter().map(|c| c.short()).collect();
    format!("\n\nRef: {}", list.join("; "))
}

const fn c(key: &'static str) -> Cite {
    Cite::new(key)
}

/// An export (its tooltip), with the references of the format as in the command line's help; a
/// figure is named by the metric the chart shows.
#[must_use]
pub fn export(kind: crate::workers::export::ExportKind, metric: ChartMetric) -> String {
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
             quoting, NA for undefined or not computed values.{}",
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
        K::Png => format!(
            "The {} chart as shown (filters and zoom), downsampled to the image width.",
            metric.in_sentence()
        ),
        K::Svg => format!(
            "The {} chart as shown (filters and zoom), every point, editable text.",
            metric.in_sentence()
        ),
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

/// Similarity-weighted entropy: the texts the command-line help shows (shared, so the two never
/// drift apart), then where the app shows the values.
#[must_use]
pub fn similarity_weighted_entropy() -> String {
    use dima_core::similarity::{DESCRIPTION, EVIDENCE, REFERENCES, sample_size};
    format!(
        "{DESCRIPTION}\n\n{EVIDENCE}\n\n{} With this option the chart can show \
         similarity-weighted entropy (Metric, above the chart), and the position table, the \
         facts of the selected position, the exported results (JSON, JSON Lines) and the \
         exported positions table hold the values; without it they are not computed.{}",
        sample_size(),
        refs(&REFERENCES)
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

/// The chart's encoding (legend tooltip) for a run's threshold and estimator, and the metric
/// shown.
#[must_use]
pub fn chart_encoding(t: u64, estimator: Estimator, metric: ChartMetric) -> String {
    let lines = match (metric, estimator) {
        (ChartMetric::SimilarityWeighted, _) => format!(
            "Solid blue: similarity-weighted entropy at positions with support > {t}, which \
             enter its summary. Dashed vermillion: the same at positions with support \
             \u{2264} {t} (LS, ELS), from fewer sequences and left out of the summary. No \
             similarity-weighted value is corrected for sample size."
        ),
        (ChartMetric::Entropy, Estimator::Dima) => format!(
            "Solid blue: corrected entropy at positions with support > {t}. Dashed vermillion: \
             the observed (uncorrected) entropy at positions with support \u{2264} {t} (LS, ELS)."
        ),
        (
            ChartMetric::Entropy,
            Estimator::ChaoWangJost | Estimator::MillerMadow | Estimator::Raw,
        ) => format!(
            "Solid blue: positions with support > {t}, which enter the summaries. Dashed \
             vermillion: positions with support \u{2264} {t} (LS, ELS), estimated the same way \
             from fewer sequences and left out of the summaries."
        ),
    };
    const ENCODING: [Cite; 6] = [
        c("tharanga2025"),
        c("wong2011"),
        c("weiskopf2022"),
        c("kerpedjiev2018"),
        c("kent2010"),
        c("jugel2014"),
    ];
    let cites = match metric {
        ChartMetric::Entropy => ENCODING.to_vec(),
        ChartMetric::SimilarityWeighted => {
            [&[dima_core::similarity::WU2025], ENCODING.as_slice()].concat()
        }
    };
    format!(
        "{lines} Hatched bands: positions without usable support, which have no {} (they \
         are never drawn as 0). The line breaks there and where filters hide positions. Long \
         ranges are drawn with M4 downsampling, which keeps every peak and trough.{}",
        metric.in_sentence(),
        refs(&cites)
    )
}

/// An option of the chart's metric switch: what it plots, and what a switch keeps.
#[must_use]
pub fn chart_metric(metric: ChartMetric) -> String {
    const SWITCH: &str = "Switching keeps the zoom and the selection, and resets the value range \
                          of the filters to the full range.";
    match metric {
        ChartMetric::Entropy => format!(
            "Plot each position's entropy, from the run's estimator: the values of the summaries \
             above. {SWITCH}"
        ),
        ChartMetric::SimilarityWeighted => format!(
            "Plot each position's similarity-weighted entropy, in which near-identical variants \
             count for less than unrelated ones; it is not corrected for sample size. \
             {SWITCH}{}",
            refs(&[dima_core::similarity::WU2025])
        ),
    }
}

/// The sources of a statement about the normalised entropies: their definition, and their
/// dependence on sample size.
fn normalised_refs() -> String {
    use dima_core::similarity::{GREGORI2014, WU2025};
    refs(&[WU2025, GREGORI2014])
}

/// The position fact "Similarity-weighted entropy".
#[must_use]
pub fn similarity_weighted_fact() -> String {
    format!(
        "Entropy in which each variant counts for less the more it resembles the other variants \
         at this position, in bits: 0 with one distinct k-mer, and never more than the observed \
         entropy. Not corrected for sample size.{}",
        refs(&[dima_core::similarity::WU2025])
    )
}

/// The position fact "Normalised observed entropy".
#[must_use]
pub fn normalised_observed_fact() -> String {
    format!(
        "Observed entropy divided by log2 of the number of distinct k-mers here (the Index \
         included): between 0 and 1, undefined with one distinct k-mer. Values marked (low \
         support) come from support \u{2264} T: they are less reliable and can be higher or \
         lower than with more sequences. Where every sequence carries a different k-mer the \
         value is 1.0000 by arithmetic alone.{}",
        normalised_refs()
    )
}

/// The position fact "Normalised similarity-weighted entropy".
#[must_use]
pub fn normalised_similarity_weighted_fact() -> String {
    format!(
        "Similarity-weighted entropy divided by log2 of the number of distinct k-mers here (the \
         Index included): between 0 and 1, undefined with one distinct k-mer. Values marked (low \
         support) come from support \u{2264} T: they are less reliable and can be higher or \
         lower than with more sequences.{}",
        normalised_refs()
    )
}

/// The summary chip of the mean entropy: the eligible positions it averages and the chart line
/// that shows it, or, with no eligible position, why there is neither.
#[must_use]
pub fn mean_chip(eligible: u64, has_mean: bool) -> String {
    if has_mean {
        format!(
            "Mean entropy over {} (support > T), each position counted once: the eligible mean, \
             the line labelled eligible mean on the chart when it shows entropy",
            dima_core::human::plural(eligible, "eligible position", "eligible positions")
        )
    } else {
        "No position has support above T, so there is no mean entropy and no eligible-mean line \
         on the chart"
            .to_owned()
    }
}

/// The summary chip of similarity-weighted entropy: the eligible positions it averages, the
/// observed mean beside it and the chart line that shows it, or, with no eligible position, why
/// there is neither.
#[must_use]
pub fn similarity_weighted_mean(eligible: u64, has_mean: bool) -> String {
    let what = if has_mean {
        format!(
            "Mean similarity-weighted entropy over {}, each position counted once, and in \
             parentheses the mean observed (uncorrected) entropy of the same positions, for a \
             like-for-like comparison. The first number is the line labelled eligible mean on \
             the chart when it shows similarity-weighted entropy.",
            dima_core::human::plural(eligible, "eligible position", "eligible positions")
        )
    } else {
        "No position has support above T, so there is no mean similarity-weighted entropy and no \
         eligible-mean line on the chart."
            .to_owned()
    };
    format!("{what}{}", refs(&[dima_core::similarity::WU2025]))
}

/// The chart's "eligible mean" legend entry: which positions the mean line covers, and that it
/// is the run's value (the mean chip's, or the first number of the similarity-weighted chip),
/// which zooming and the filters do not change (a filter can rescale the value axis, and so move
/// the line, but not change its value).
#[must_use]
pub fn eligible_mean(metric: ChartMetric, threshold: u64, eligible: u64) -> String {
    let chip = match metric {
        ChartMetric::Entropy => "the mean chip above",
        ChartMetric::SimilarityWeighted => {
            "the first number of the similarity-weighted mean chip above"
        }
    };
    format!(
        "Eligible mean: the average {} over {} (support > {threshold}), each position counted \
         once. It is the run's value, the one {chip} shows: zooming and the filters do not \
         change it. Positions with support \u{2264} {threshold} are not in it.{}",
        metric.in_sentence(),
        dima_core::human::plural(eligible, "eligible position", "eligible positions"),
        refs(&[c("tharanga2025").at("Results")])
    )
}

/// The filter bar's value range, for the metric shown.
#[must_use]
pub fn value_range(metric: ChartMetric) -> String {
    let m = metric.in_sentence();
    format!(
        "Show positions whose {m} is in this range (bits). Positions without usable support \
         have no {m} and are not affected."
    )
}

#[cfg(test)]
mod tests {
    use super::{ChartMetric, Estimator};

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
            super::similarity_weighted_entropy(),
            super::min_coverage(),
            super::chart_encoding(100, Estimator::Dima, ChartMetric::Entropy),
            super::chart_encoding(100, Estimator::Raw, ChartMetric::Entropy),
            super::chart_encoding(100, Estimator::Dima, ChartMetric::SimilarityWeighted),
            super::chart_metric(ChartMetric::SimilarityWeighted),
            super::similarity_weighted_fact(),
            super::normalised_observed_fact(),
            super::normalised_similarity_weighted_fact(),
            super::similarity_weighted_mean(1345, true),
            super::similarity_weighted_mean(0, false),
            super::eligible_mean(ChartMetric::Entropy, 100, 1345),
            super::eligible_mean(ChartMetric::SimilarityWeighted, 100, 1345),
        ] {
            assert!(text.contains("\n\nRef: "), "{text}");
        }
        assert!(super::support_threshold().contains("Tharanga et al. 2025, Methods"));
        // The entropy texts are those of 1.0.0; similarity-weighted entropy cites Wu first and
        // never calls itself corrected.
        let entropy = super::chart_encoding(100, Estimator::Dima, ChartMetric::Entropy);
        assert!(
            entropy.starts_with("Solid blue: corrected entropy at positions with support > 100.")
        );
        assert!(entropy.contains("which have no entropy (they are never drawn as 0)"));
        assert!(
            entropy.contains("Ref: Tharanga et al. 2025; Wong 2011"),
            "{entropy}"
        );
        let swe = super::chart_encoding(100, Estimator::Dima, ChartMetric::SimilarityWeighted);
        assert!(!swe.contains("corrected entropy"), "{swe}");
        assert!(swe.contains("No similarity-weighted value is corrected for sample size"));
        assert!(swe.contains("Ref: Wu 2025, Section 2.1 and Supplementary File 1; Tharanga"));
        assert_eq!(
            super::value_range(ChartMetric::Entropy),
            "Show positions whose entropy is in this range (bits). Positions without usable \
             support have no entropy and are not affected."
        );
        // The command line's texts, word for word.
        let swe = super::similarity_weighted_entropy();
        for shared in [
            dima_core::similarity::DESCRIPTION.to_owned(),
            dima_core::similarity::EVIDENCE.to_owned(),
            dima_core::similarity::sample_size(),
        ] {
            assert!(swe.contains(&shared), "{swe}");
        }
        assert!(
            swe.contains("Wu 2025, Section 2.1 and Supplementary File 1"),
            "{swe}"
        );
        for kind in crate::workers::export::ExportKind::ALL {
            for m in ChartMetric::ALL {
                assert!(!super::export(kind, m).is_empty());
            }
        }
        use crate::workers::export::ExportKind as K;
        for kind in [K::TsvFolder, K::HcsJson, K::HcsTsv, K::RunReport] {
            assert!(
                super::export(kind, ChartMetric::Entropy).contains("\n\nRef: "),
                "{kind:?}"
            );
        }
        assert_eq!(
            super::export(K::Png, ChartMetric::Entropy),
            "The entropy chart as shown (filters and zoom), downsampled to the image width."
        );
        assert!(
            super::export(K::Svg, ChartMetric::SimilarityWeighted)
                .starts_with("The similarity-weighted entropy chart as shown")
        );
    }

    /// The mean line's legend tooltip names the metric, the positions it covers and the chip
    /// that shows the same value, and says that zooming and filtering do not change it.
    #[test]
    fn the_eligible_mean_tooltip() {
        assert_eq!(
            super::eligible_mean(ChartMetric::Entropy, 100, 1345),
            "Eligible mean: the average entropy over 1345 eligible positions (support > 100), \
             each position counted once. It is the run's value, the one the mean chip above \
             shows: zooming and the filters do not change it. Positions with support \u{2264} \
             100 are not in it.\n\nRef: Tharanga et al. 2025, Results"
        );
        let swe = super::eligible_mean(ChartMetric::SimilarityWeighted, 30, 1);
        assert!(
            swe.starts_with(
                "Eligible mean: the average similarity-weighted entropy over 1 eligible position \
                 (support > 30),"
            ),
            "{swe}"
        );
        assert!(
            swe.contains("the first number of the similarity-weighted mean chip above"),
            "{swe}"
        );
        assert!(swe.contains("support \u{2264} 30 are not in it."), "{swe}");
    }

    /// The two mean chips' tooltips name the chart line that shows their value, count the
    /// positions in words that fit one, and say why there is neither mean nor line without an
    /// eligible position.
    #[test]
    fn the_mean_chip_tooltips() {
        assert_eq!(
            super::mean_chip(1345, true),
            "Mean entropy over 1345 eligible positions (support > T), each position counted \
             once: the eligible mean, the line labelled eligible mean on the chart when it shows \
             entropy"
        );
        assert!(
            super::mean_chip(1, true).starts_with("Mean entropy over 1 eligible position ("),
            "{}",
            super::mean_chip(1, true)
        );
        assert_eq!(
            super::mean_chip(0, false),
            "No position has support above T, so there is no mean entropy and no eligible-mean \
             line on the chart"
        );
        let swe = super::similarity_weighted_mean(1345, true);
        assert!(
            swe.starts_with("Mean similarity-weighted entropy over 1345 eligible positions,"),
            "{swe}"
        );
        assert!(
            swe.contains(
                "The first number is the line labelled eligible mean on the chart when it shows \
                 similarity-weighted entropy."
            ),
            "{swe}"
        );
        let none = super::similarity_weighted_mean(0, false);
        assert!(
            none.starts_with(
                "No position has support above T, so there is no mean similarity-weighted entropy"
            ),
            "{none}"
        );
    }
}
