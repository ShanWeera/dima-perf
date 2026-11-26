//! The workspace filters: which positions the chart and the position table show.
//!
//! Filters are null-aware (G33, G44): the value range applies to the value of the metric the
//! chart plots, and only at positions that have one, so positions without usable support
//! (`null`, D34) are never dropped by it and stay visible as bands; whether low-support
//! positions are shown at all is the separate "low support" switch, which treats NS, LS and ELS
//! alike (G43). The filter does not store the metric: the session passes it in, and a switch of
//! metric resets the value range to the new metric's full range.

use dima_core::model::{Motif, PositionResult, Results};

use crate::charts::metric::ChartMetric;

/// The motif classes, in display order.
pub const MOTIFS: [Motif; 4] = [Motif::Index, Motif::Major, Motif::Minor, Motif::Unique];

/// Filter settings.
#[derive(Clone, Debug, PartialEq)]
pub struct FilterState {
    /// Positions shown, inclusive.
    pub position_range: (u64, u64),
    /// Range of the plotted metric's value shown, inclusive (positions without a value are not
    /// affected).
    pub value_range: (f64, f64),
    /// Keep positions with at least one variant of a checked motif; all or none checked
    /// means no motif filter.
    pub motifs: [bool; 4],
    /// Show positions with support ≤ T (NS, LS, ELS).
    pub include_low_support: bool,
}

/// The bounds the filter controls offer for a dataset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// First and last position.
    pub positions: (u64, u64),
    /// Largest entropy of any position (0 when none has one).
    pub max_entropy: f64,
    /// Largest similarity-weighted entropy of any position (0 when none has one, as when it
    /// was not computed).
    pub max_similarity_weighted: f64,
}

impl Bounds {
    /// Computes the bounds once per dataset.
    #[must_use]
    pub fn of(results: &Results) -> Self {
        let first = results.results.first().map_or(1, |p| p.position);
        let last = results.results.last().map_or(1, |p| p.position);
        let max = |metric: ChartMetric| {
            results
                .results
                .iter()
                .filter_map(|p| metric.value(p))
                .filter(|v| v.is_finite())
                .fold(0.0_f64, f64::max)
        };
        Self {
            positions: (first, last),
            max_entropy: max(ChartMetric::Entropy),
            max_similarity_weighted: max(ChartMetric::SimilarityWeighted),
        }
    }

    /// The largest value of a metric.
    #[must_use]
    pub const fn max_for(&self, metric: ChartMetric) -> f64 {
        match metric {
            ChartMetric::Entropy => self.max_entropy,
            ChartMetric::SimilarityWeighted => self.max_similarity_weighted,
        }
    }
}

impl FilterState {
    /// Shows everything, with the full value range of `metric`.
    #[must_use]
    pub fn all(bounds: Bounds, metric: ChartMetric) -> Self {
        Self {
            position_range: bounds.positions,
            value_range: (0.0, bounds.max_for(metric)),
            motifs: [true; 4],
            include_low_support: true,
        }
    }

    /// `true` when the filter keeps every position of a dataset with these bounds, plotted as
    /// `metric`.
    #[must_use]
    pub fn is_everything(&self, bounds: Bounds, metric: ChartMetric) -> bool {
        *self == Self::all(bounds, metric)
    }

    /// Whether one position passes, its value being that of `metric`.
    #[must_use]
    pub fn keeps(&self, p: &PositionResult, metric: ChartMetric) -> bool {
        let (lo, hi) = self.position_range;
        if p.position < lo || p.position > hi {
            return false;
        }
        if let Some(v) = metric.value(p) {
            let (vmin, vmax) = self.value_range;
            // Non-finite values cannot be compared meaningfully; they are kept (the library
            // never writes them, and hiding them would hide the problem).
            if v.is_finite() && (v < vmin || v > vmax) {
                return false;
            }
        }
        if !self.include_low_support && p.low_support.is_some() {
            return false;
        }
        let checked = self.motifs.iter().filter(|m| **m).count();
        if checked > 0 && checked < MOTIFS.len() {
            let wanted = |m: Motif| MOTIFS.iter().zip(self.motifs).any(|(x, on)| on && *x == m);
            if !p.diversity_motifs.iter().any(|v| wanted(v.motif)) {
                return false;
            }
        }
        true
    }

    /// Indices (into `results.results`) of the positions that pass, their values being those
    /// of `metric`, in position order. An inverted range keeps nothing.
    #[must_use]
    pub fn apply(&self, results: &Results, metric: ChartMetric) -> Vec<usize> {
        let ordered = matches!(
            self.value_range.0.partial_cmp(&self.value_range.1),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        );
        if self.position_range.0 > self.position_range.1 || !ordered {
            return Vec::new();
        }
        results
            .results
            .iter()
            .enumerate()
            .filter(|(_, p)| self.keeps(p, metric))
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::series::tests::{position, results_of};

    const E: ChartMetric = ChartMetric::Entropy;
    const W: ChartMetric = ChartMetric::SimilarityWeighted;

    fn data() -> Results {
        results_of(vec![
            position(1, Some(0.5), 200, 100),
            position(2, None, 0, 100),
            position(3, Some(1.5), 50, 100),
            position(4, Some(0.1), 100, 100),
            position(5, Some(1.0), 300, 100),
        ])
    }

    #[test]
    fn default_shows_everything_including_no_support() {
        let r = data();
        let b = Bounds::of(&r);
        assert_eq!(b.positions, (1, 5));
        assert!((b.max_entropy - 1.5).abs() < 1e-12);
        assert_eq!(b.max_similarity_weighted, 0.0, "not computed");
        let f = FilterState::all(b, E);
        assert_eq!(f.apply(&r, E), vec![0, 1, 2, 3, 4]);
        assert!(f.is_everything(b, E));
    }

    #[test]
    fn entropy_range_is_null_aware() {
        let r = data();
        let mut f = FilterState::all(Bounds::of(&r), E);
        f.value_range = (0.4, 1.2);
        // NS (no entropy) stays; 1.5 and 0.1 go.
        assert_eq!(f.apply(&r, E), vec![0, 1, 4]);
    }

    /// The value range applies to the plotted metric, null-aware alike, and each metric has
    /// its own full range.
    #[test]
    fn value_ranges_follow_the_metric() {
        let mut r = data();
        for (p, v) in r.results.iter_mut().zip([0.2, f64::NAN, 0.75, 0.05, 0.5]) {
            p.similarity_weighted_entropy = (p.support > 0).then_some(v);
        }
        let b = Bounds::of(&r);
        assert_eq!((b.max_for(E), b.max_for(W)), (1.5, 0.75));
        let all = FilterState::all(b, W);
        assert_eq!(all.value_range, (0.0, 0.75));
        assert!(all.is_everything(b, W));
        assert!(!all.is_everything(b, E), "another metric's full range");
        assert_eq!(all.apply(&r, W), vec![0, 1, 2, 3, 4]);
        let mut f = all.clone();
        f.value_range = (0.1, 0.6);
        // Position 2 has no value (NS), position 4's 0.05 and 3's 0.75 go.
        assert_eq!(f.apply(&r, W), vec![0, 1, 4]);
        // The same range read as entropy keeps other positions (0.1 at position 4 is in it).
        assert_eq!(f.apply(&r, E), vec![0, 1, 3]);
        f.value_range = (0.0, 0.3);
        assert_eq!(f.apply(&r, W), vec![0, 1, 3]);
        assert_eq!(f.apply(&r, E), vec![1, 3]);
    }

    #[test]
    fn low_support_switch_groups_ns_ls_and_els() {
        let r = data();
        let mut f = FilterState::all(Bounds::of(&r), E);
        f.include_low_support = false;
        assert_eq!(f.apply(&r, E), vec![0, 4], "NS, LS and ELS are all hidden");
    }

    #[test]
    fn motif_filter_and_inverted_ranges() {
        let r = data();
        let mut f = FilterState::all(Bounds::of(&r), E);
        f.motifs = [false, false, false, true];
        assert!(f.apply(&r, E).is_empty(), "no Unique variants in this data");
        f.motifs = [false; 4];
        assert_eq!(f.apply(&r, E).len(), 5, "none checked = no motif filter");
        f.position_range = (4, 2);
        assert!(f.apply(&r, E).is_empty());
        f.position_range = (1, 5);
        f.value_range = (1.0, 0.0);
        assert!(f.apply(&r, E).is_empty());
        f.value_range = (f64::NAN, 1.0);
        assert!(f.apply(&r, E).is_empty(), "NaN bounds keep nothing");
    }
}
