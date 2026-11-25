//! What the entropy chart plots (D87): the run's entropy or, when the run computed it,
//! similarity-weighted entropy (Wu 2025, *Virus Evolution* 11:veaf029, Section 2.1). The metric
//! decides the plotted value, the mean line, the titles and the value axis, and the names of
//! exported figures; how the chart shows support (solid, dashed, hatched) is the same for both.
//! Charts never read the application state: the session passes its metric in.

use dima_core::human::{
    plural,
    separators::{GROUP, ITEM},
};
use dima_core::model::{PositionResult, Results};

/// The value the chart plots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChartMetric {
    /// The run's entropy (`entropy`: corrected where its estimator corrects it).
    #[default]
    Entropy,
    /// Similarity-weighted entropy (`similarity_weighted_entropy`).
    SimilarityWeighted,
}

impl ChartMetric {
    /// Both metrics, in the order of the chart's switch.
    pub const ALL: [Self; 2] = [Self::Entropy, Self::SimilarityWeighted];

    /// The value plotted at a position, in bits; `None` where it has none (no support, or not
    /// computed).
    #[must_use]
    pub const fn value(self, p: &PositionResult) -> Option<f64> {
        match self {
            Self::Entropy => p.entropy,
            Self::SimilarityWeighted => p.similarity_weighted_entropy,
        }
    }

    /// The mean over the eligible positions, as the summaries give it (the mean line).
    #[must_use]
    pub fn mean(self, r: &Results) -> Option<f64> {
        match self {
            Self::Entropy => r.average_entropy,
            Self::SimilarityWeighted => r
                .similarity_weighted_entropy_summary
                .as_ref()
                .and_then(|s| s.average_entropy),
        }
    }

    /// `true` when the results hold the metric's values: always for entropy; for
    /// similarity-weighted entropy when the run computed it (the library's presence rules then
    /// give a value at every supported position).
    #[must_use]
    pub const fn available(self, r: &Results) -> bool {
        match self {
            Self::Entropy => true,
            Self::SimilarityWeighted => r.run.parameters.similarity_weighted_entropy.value,
        }
    }

    /// The name as a heading, a switch option and a filter label.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Entropy => "Entropy",
            Self::SimilarityWeighted => "Similarity-weighted entropy",
        }
    }

    /// The name inside a sentence.
    #[must_use]
    pub const fn in_sentence(self) -> &'static str {
        match self {
            Self::Entropy => "entropy",
            Self::SimilarityWeighted => "similarity-weighted entropy",
        }
    }

    /// The title of the value axis on screen.
    #[must_use]
    pub const fn axis_title(self) -> &'static str {
        match self {
            Self::Entropy => "Entropy (bits)",
            Self::SimilarityWeighted => "Similarity-weighted entropy (bits)",
        }
    }

    /// The title of the value axis of exported figures.
    #[must_use]
    pub const fn figure_axis_title(self) -> &'static str {
        match self {
            Self::Entropy => "Shannon entropy (bits)",
            Self::SimilarityWeighted => "Similarity-weighted entropy (bits)",
        }
    }

    /// The title of an exported figure of the query's results.
    #[must_use]
    pub fn figure_title(self, query: &str) -> String {
        let what = match self {
            Self::Entropy => "Shannon entropy",
            Self::SimilarityWeighted => "Similarity-weighted entropy",
        };
        format!("{query} \u{2014} {what} per k-mer position")
    }

    /// The subtitle of an exported figure: the run's facts (the estimator only for entropy,
    /// the one metric it applies to) and how many positions are shown.
    #[must_use]
    pub fn figure_subtitle(self, r: &Results, shown: usize) -> String {
        let estimator = match self {
            Self::Entropy => format!(
                "{ITEM}estimator {}",
                r.run.parameters.estimator.value.as_str()
            ),
            Self::SimilarityWeighted => String::new(),
        };
        format!(
            "{}{ITEM}k = {}{ITEM}T = {}{estimator}{GROUP}{shown} of {} positions shown{GROUP}DiMA {}",
            plural(r.sequence_count, "sequence", "sequences"),
            r.kmer_length,
            r.support_threshold,
            r.position_count,
            r.run.tool.version
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures;

    #[test]
    fn values_means_and_availability() {
        let off = fixtures::mers();
        let on = fixtures::mers_swe();
        assert!(ChartMetric::Entropy.available(&off));
        assert!(!ChartMetric::SimilarityWeighted.available(&off));
        assert!(ChartMetric::SimilarityWeighted.available(&on));
        let p = &on.results[24];
        assert_eq!(p.position, 25);
        assert_eq!(ChartMetric::Entropy.value(p), p.entropy);
        assert_eq!(
            ChartMetric::SimilarityWeighted.value(p),
            p.similarity_weighted_entropy
        );
        assert_eq!(ChartMetric::Entropy.mean(&on), on.average_entropy);
        let mean = ChartMetric::SimilarityWeighted.mean(&on).unwrap();
        assert!((mean - 0.0363).abs() < 5e-5, "{mean}");
        assert_eq!(ChartMetric::SimilarityWeighted.mean(&off), None);
    }

    /// The entropy texts are those of 1.0.0, byte for byte; similarity-weighted entropy has no
    /// estimator.
    #[test]
    fn titles_and_subtitles() {
        let r = fixtures::mers_swe();
        assert_eq!(
            ChartMetric::Entropy.figure_title("q"),
            "q \u{2014} Shannon entropy per k-mer position"
        );
        assert_eq!(
            ChartMetric::SimilarityWeighted.figure_title("q"),
            "q \u{2014} Similarity-weighted entropy per k-mer position"
        );
        let version = &r.run.tool.version;
        assert_eq!(
            ChartMetric::Entropy.figure_subtitle(&r, 12),
            format!(
                "191 sequences, k = 9, T = 100, estimator dima; 12 of 1345 positions shown; \
                 DiMA {version}"
            )
        );
        assert_eq!(
            ChartMetric::SimilarityWeighted.figure_subtitle(&r, 12),
            format!("191 sequences, k = 9, T = 100; 12 of 1345 positions shown; DiMA {version}")
        );
    }
}
