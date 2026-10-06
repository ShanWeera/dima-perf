//! The workspace filters: which positions the chart and the position table show.
//!
//! Filters are null-aware (G33, G44): the entropy range applies only to positions that have an
//! entropy, so positions without usable support (entropy `null`, D34) are never dropped by it
//! and stay visible as bands; whether low-support positions are shown at all is the separate
//! "low support" switch, which treats NS, LS and ELS alike (G43).

use dima_core::model::{Motif, PositionResult, Results};

/// The motif classes, in display order.
pub const MOTIFS: [Motif; 4] = [Motif::Index, Motif::Major, Motif::Minor, Motif::Unique];

/// Filter settings.
#[derive(Clone, Debug, PartialEq)]
pub struct FilterState {
    /// Positions shown, inclusive.
    pub position_range: (u64, u64),
    /// Entropy range shown, inclusive (positions without entropy are not affected).
    pub entropy_range: (f64, f64),
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
}

impl Bounds {
    /// Computes the bounds once per dataset.
    #[must_use]
    pub fn of(results: &Results) -> Self {
        let first = results.results.first().map_or(1, |p| p.position);
        let last = results.results.last().map_or(1, |p| p.position);
        let max_entropy = results
            .results
            .iter()
            .filter_map(|p| p.entropy)
            .filter(|e| e.is_finite())
            .fold(0.0_f64, f64::max);
        Self {
            positions: (first, last),
            max_entropy,
        }
    }
}

impl FilterState {
    /// Shows everything.
    #[must_use]
    pub fn all(bounds: Bounds) -> Self {
        Self {
            position_range: bounds.positions,
            entropy_range: (0.0, bounds.max_entropy),
            motifs: [true; 4],
            include_low_support: true,
        }
    }

    /// `true` when the filter keeps every position of a dataset with these bounds.
    #[must_use]
    pub fn is_everything(&self, bounds: Bounds) -> bool {
        *self == Self::all(bounds)
    }

    /// Whether one position passes.
    #[must_use]
    pub fn keeps(&self, p: &PositionResult) -> bool {
        let (lo, hi) = self.position_range;
        if p.position < lo || p.position > hi {
            return false;
        }
        if let Some(e) = p.entropy {
            let (emin, emax) = self.entropy_range;
            // Non-finite values cannot be compared meaningfully; they are kept (the library
            // never writes them, and hiding them would hide the problem).
            if e.is_finite() && (e < emin || e > emax) {
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

    /// Indices (into `results.results`) of the positions that pass, in position order. An
    /// inverted range keeps nothing.
    #[must_use]
    pub fn apply(&self, results: &Results) -> Vec<usize> {
        let ordered = matches!(
            self.entropy_range.0.partial_cmp(&self.entropy_range.1),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        );
        if self.position_range.0 > self.position_range.1 || !ordered {
            return Vec::new();
        }
        results
            .results
            .iter()
            .enumerate()
            .filter(|(_, p)| self.keeps(p))
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::series::tests::{position, results_of};

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
        let f = FilterState::all(b);
        assert_eq!(f.apply(&r), vec![0, 1, 2, 3, 4]);
        assert!(f.is_everything(b));
    }

    #[test]
    fn entropy_range_is_null_aware() {
        let r = data();
        let mut f = FilterState::all(Bounds::of(&r));
        f.entropy_range = (0.4, 1.2);
        // NS (no entropy) stays; 1.5 and 0.1 go.
        assert_eq!(f.apply(&r), vec![0, 1, 4]);
    }

    #[test]
    fn low_support_switch_groups_ns_ls_and_els() {
        let r = data();
        let mut f = FilterState::all(Bounds::of(&r));
        f.include_low_support = false;
        assert_eq!(f.apply(&r), vec![0, 4], "NS, LS and ELS are all hidden");
    }

    #[test]
    fn motif_filter_and_inverted_ranges() {
        let r = data();
        let mut f = FilterState::all(Bounds::of(&r));
        f.motifs = [false, false, false, true];
        assert!(f.apply(&r).is_empty(), "no Unique variants in this data");
        f.motifs = [false; 4];
        assert_eq!(f.apply(&r).len(), 5, "none checked = no motif filter");
        f.position_range = (4, 2);
        assert!(f.apply(&r).is_empty());
        f.position_range = (1, 5);
        f.entropy_range = (1.0, 0.0);
        assert!(f.apply(&r).is_empty());
        f.entropy_range = (f64::NAN, 1.0);
        assert!(f.apply(&r).is_empty(), "NaN bounds keep nothing");
    }
}
