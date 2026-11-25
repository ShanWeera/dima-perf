//! Run-level summaries (D11, D32, D42, D43, D88).
//!
//! * The **eligible summary** (top-level keys) covers positions with support above the
//!   threshold only, whose entropy the paper's correction applies to: mean, lowest and highest
//!   (first position plus the number of tied positions), and position counts.
//! * The **all-positions summary** reproduces DiMA v5.0.9's averaging rule (every position, NS
//!   counted as 0 bits, LS/ELS with their reported entropy: the observed entropy with the
//!   `dima` and `raw` estimators) for comparison with earlier results.
//! * The **coverage summary** describes the estimated sample coverage (Chao & Jost 2012).
//! * The **similarity-weighted summary** (with `--similarity-weighted-entropy`) covers the
//!   eligible positions' similarity-weighted entropy (Wu 2025) and their mean observed entropy,
//!   so the two can be compared like for like; the normalised values are not summarised.

use crate::model::{
    AllPositionsSummary, CoverageSummary, Extremum, PositionResult, SimilarityWeightedSummary,
    SupportLabel, SupportLabelCounts,
};
use crate::numeric::Neumaier;

/// Text of the all-positions averaging rule, recorded in the output.
pub const ALL_POSITIONS_RULE: &str = "mean over all positions with positions without support \
    counted as 0 bits and low-support positions with their reported entropy (DiMA v5.0.9's \
    rule; v5.0.9 reported the last maximum with a 0-based position, here the first, 1-based)";

/// Counts of positions per support class.
#[must_use]
pub fn label_counts(positions: &[PositionResult]) -> SupportLabelCounts {
    let mut c = SupportLabelCounts::default();
    for p in positions {
        match p.low_support {
            Some(SupportLabel::NoSupport) => c.no_support += 1,
            Some(SupportLabel::Low) => c.low += 1,
            Some(SupportLabel::EqualToThreshold) => c.equal_to_threshold += 1,
            None => c.eligible += 1,
        }
    }
    c
}

/// Mean (compensated sum) and extrema (first position, tie count) of `(position, value)` pairs
/// in position order; all `None` when empty.
#[must_use]
pub fn describe(values: &[(u64, f64)]) -> (Option<f64>, Option<Extremum>, Option<Extremum>) {
    let Some(&(first_pos, first_val)) = values.first() else {
        return (None, None, None);
    };
    let mut sum = Neumaier::new();
    let mut low = Extremum {
        position: first_pos,
        entropy: first_val,
        tied_position_count: 0,
    };
    let mut high = low;
    for &(pos, v) in values {
        sum.add(v);
        if v < low.entropy {
            low = Extremum {
                position: pos,
                entropy: v,
                tied_position_count: 0,
            };
        }
        if v > high.entropy {
            high = Extremum {
                position: pos,
                entropy: v,
                tied_position_count: 0,
            };
        }
    }
    low.tied_position_count = values.iter().filter(|&&(_, v)| v == low.entropy).count() as u64;
    high.tied_position_count = values.iter().filter(|&&(_, v)| v == high.entropy).count() as u64;
    let mean = sum.total() / values.len() as f64;
    (Some(mean), Some(low), Some(high))
}

/// The eligible summary: mean, lowest and highest entropy over positions with support > T.
#[must_use]
pub fn eligible(positions: &[PositionResult]) -> (Option<f64>, Option<Extremum>, Option<Extremum>) {
    let values: Vec<(u64, f64)> = positions
        .iter()
        .filter(|p| p.low_support.is_none())
        .filter_map(|p| p.entropy.map(|e| (p.position, e)))
        .collect();
    describe(&values)
}

/// The all-positions summary (DiMA v5.0.9's rule).
#[must_use]
pub fn all_positions(positions: &[PositionResult]) -> AllPositionsSummary {
    let values: Vec<(u64, f64)> = positions
        .iter()
        .map(|p| (p.position, p.entropy.unwrap_or(0.0)))
        .collect();
    let (average_entropy, lowest_entropy, highest_entropy) = describe(&values);
    AllPositionsSummary {
        rule: ALL_POSITIONS_RULE.to_owned(),
        position_count: values.len() as u64,
        average_entropy,
        lowest_entropy,
        highest_entropy,
    }
}

/// The similarity-weighted summary over eligible positions (support > T): mean, lowest and
/// highest similarity-weighted entropy, and the mean observed entropy of the same positions.
#[must_use]
pub fn similarity_weighted(positions: &[PositionResult]) -> SimilarityWeightedSummary {
    let eligible: Vec<&PositionResult> = positions
        .iter()
        .filter(|p| p.low_support.is_none())
        .collect();
    let weighted: Vec<(u64, f64)> = eligible
        .iter()
        .filter_map(|p| p.similarity_weighted_entropy.map(|e| (p.position, e)))
        .collect();
    let (average_entropy, lowest_entropy, highest_entropy) = describe(&weighted);
    let observed: Vec<(u64, f64)> = eligible
        .iter()
        .filter_map(|p| p.observed_entropy.map(|e| (p.position, e)))
        .collect();
    SimilarityWeightedSummary {
        position_count: weighted.len() as u64,
        average_entropy,
        lowest_entropy,
        highest_entropy,
        average_observed_entropy: describe(&observed).0,
    }
}

/// The sample-coverage summary.
#[must_use]
pub fn coverage(positions: &[PositionResult], min_coverage: Option<f64>) -> CoverageSummary {
    let values: Vec<f64> = positions.iter().filter_map(|p| p.sample_coverage).collect();
    let mut sum = Neumaier::new();
    values.iter().for_each(|&v| sum.add(v));
    CoverageSummary {
        evaluated: values.len() as u64,
        min: values.iter().copied().reduce(f64::min),
        mean: (!values.is_empty()).then(|| sum.total() / values.len() as f64),
        min_coverage,
        below_min_count: min_coverage.map(|m| values.iter().filter(|&&v| v < m).count() as u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EntropyMethod;

    fn pos(
        position: u64,
        label: Option<SupportLabel>,
        entropy: Option<f64>,
        coverage: Option<f64>,
    ) -> PositionResult {
        PositionResult {
            position,
            low_support: label,
            support: 0,
            distinct_variants_count: 0,
            entropy,
            observed_entropy: entropy,
            entropy_method: EntropyMethod::PlugIn,
            entropy_fallback: None,
            distinct_variants_incidence: None,
            total_variants_incidence: None,
            sample_coverage: coverage,
            below_min_coverage: None,
            similarity_weighted_entropy: entropy.map(|e| e / 2.0),
            normalised_observed_entropy: None,
            normalised_similarity_weighted_entropy: None,
            diversity_motifs: Vec::new(),
        }
    }

    #[test]
    fn eligible_summary_uses_first_position_and_ties() {
        let ps = vec![
            pos(1, None, Some(1.0), Some(0.9)),
            pos(2, Some(SupportLabel::NoSupport), None, None),
            pos(3, None, Some(2.0), Some(0.8)),
            pos(4, Some(SupportLabel::Low), Some(5.0), Some(0.5)),
            pos(5, None, Some(2.0), Some(1.0)),
            pos(6, None, Some(1.0), None),
        ];
        let (mean, low, high) = eligible(&ps);
        assert_eq!(mean, Some(1.5));
        assert_eq!(
            low,
            Some(Extremum {
                position: 1,
                entropy: 1.0,
                tied_position_count: 2
            })
        );
        assert_eq!(
            high,
            Some(Extremum {
                position: 3,
                entropy: 2.0,
                tied_position_count: 2
            })
        );
        let all = all_positions(&ps);
        assert_eq!(all.position_count, 6);
        assert_eq!(all.average_entropy, Some(11.0 / 6.0));
        assert_eq!(
            all.lowest_entropy.unwrap().position,
            2,
            "NS counts as 0 bits"
        );
        assert_eq!(all.highest_entropy.unwrap().position, 4);
        let c = label_counts(&ps);
        assert_eq!(
            (c.no_support, c.low, c.equal_to_threshold, c.eligible),
            (1, 1, 0, 4)
        );
        let cov = coverage(&ps, Some(0.85));
        assert_eq!(cov.evaluated, 4);
        assert_eq!(cov.min, Some(0.5));
        assert_eq!(cov.below_min_count, Some(2));
        assert!((cov.mean.unwrap() - 0.8).abs() < 1e-15);
        assert_eq!(coverage(&ps, None).below_min_count, None);
    }

    #[test]
    fn empty_summaries_are_null() {
        assert_eq!(describe(&[]), (None, None, None));
        let ps = vec![pos(1, Some(SupportLabel::NoSupport), None, None)];
        assert_eq!(eligible(&ps), (None, None, None));
        let cov = coverage(&ps, None);
        assert_eq!((cov.evaluated, cov.min, cov.mean), (0, None, None));
        assert_eq!(all_positions(&[]).average_entropy, None);
    }

    #[test]
    fn similarity_weighted_summary_covers_eligible_positions_only() {
        let ps = vec![
            pos(1, None, Some(1.0), None),
            pos(2, Some(SupportLabel::Low), Some(3.0), None),
            pos(3, None, Some(2.0), None),
            pos(4, Some(SupportLabel::NoSupport), None, None),
        ];
        let s = similarity_weighted(&ps);
        assert_eq!(s.position_count, 2);
        assert_eq!(s.average_entropy, Some(0.75));
        assert_eq!(s.lowest_entropy.unwrap().position, 1);
        assert_eq!(s.highest_entropy.unwrap().entropy, 1.0);
        assert_eq!(s.average_observed_entropy, Some(1.5));
        let none = similarity_weighted(&ps[1..2]);
        assert_eq!(none.position_count, 0);
        assert_eq!(none.average_entropy, None);
        assert_eq!(none.average_observed_entropy, None);
    }
}
