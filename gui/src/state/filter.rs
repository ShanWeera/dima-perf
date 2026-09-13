//! Filter state for the workspace view.
//!
//! Controls which positions appear in the entropy chart and position explorer.
//! Matches the behavior of `ui/src/lib/filters.ts` (including Fix 6.23).

use dima_lib::Results;

/// Motif type classification, matching dima_lib's Variant.motif_short values.
#[derive(Debug, Clone, PartialEq)]
pub enum MotifType {
    Index,
    Major,
    Minor,
    Unique,
}

impl MotifType {
    /// Maps to the exact strings stored in dima_lib's Variant.motif_short field.
    pub fn as_str(&self) -> &str {
        match self {
            MotifType::Index => "I",
            MotifType::Major => "Ma",
            MotifType::Minor => "Mi",
            MotifType::Unique => "U",
        }
    }

    /// Display name for the UI.
    pub fn display_name(&self) -> &str {
        match self {
            MotifType::Index => "Index",
            MotifType::Major => "Major",
            MotifType::Minor => "Minor",
            MotifType::Unique => "Unique",
        }
    }
}

const ALL_MOTIF_TYPES_COUNT: usize = 4;

/// Filter configuration for the workspace view.
#[derive(Debug, Clone)]
pub struct FilterState {
    /// 1-based inclusive position range
    pub position_range: (usize, usize),
    /// Min/max entropy range
    pub entropy_range: (f64, f64),
    /// Which motif types to include
    pub motif_types: Vec<MotifType>,
    /// Whether to include positions with low support (NS/LS)
    pub include_low_support: bool,
}

impl FilterState {
    /// Create default filter state that shows ALL positions.
    ///
    /// Equivalent to TypeScript's DEFAULT_FILTERS where positionRange=null,
    /// entropyRange=null, motifTypes=ALL, includeLowSupport=true.
    ///
    /// IMPORTANT: entropy upper bound must be computed from ALL positions, not
    /// just reliable ones. Results.highest_entropy excludes NS/LS positions
    /// (analysis.rs compute_summary_stats), so an LS position could have
    /// entropy > highest_entropy.entropy and would be incorrectly filtered out.
    pub fn default_for(results: &Results) -> Self {
        let last_position = results.results.last().map(|p| p.position).unwrap_or(1);
        let max_entropy_all_positions = results
            .results
            .iter()
            .map(|p| p.entropy)
            .filter(|e| e.is_finite())
            .fold(0.0_f64, f64::max);
        FilterState {
            position_range: (1, last_position),
            entropy_range: (0.0, max_entropy_all_positions),
            motif_types: vec![
                MotifType::Index,
                MotifType::Major,
                MotifType::Minor,
                MotifType::Unique,
            ],
            include_low_support: true,
        }
    }

    /// Apply filters to produce a list of position indices.
    /// Returns indices into the UNFILTERED results.results Vec.
    /// Matches the behavior in ui/src/lib/filters.ts (including Fix 6.23).
    pub fn apply(&self, results: &Results) -> Vec<usize> {
        // Inverted range check: if from > to, return empty (matches TS behavior)
        if self.position_range.0 > self.position_range.1 {
            return Vec::new();
        }
        if self.entropy_range.0 > self.entropy_range.1 {
            return Vec::new();
        }

        // Motif filter: active only when motif_types is non-empty AND not all
        // types selected. Fix 6.23: empty motif_types = "no filter" (user
        // unchecked all = show everything), NOT "exclude everything".
        let motif_filter_active =
            !self.motif_types.is_empty() && self.motif_types.len() < ALL_MOTIF_TYPES_COUNT;

        results
            .results
            .iter()
            .enumerate()
            .filter(|(_, pos)| {
                // Position range check (1-based)
                if pos.position < self.position_range.0 || pos.position > self.position_range.1 {
                    return false;
                }
                // Entropy range check (handle NaN/Inf per Fix 5.63)
                if !pos.entropy.is_finite()
                    || pos.entropy < self.entropy_range.0
                    || pos.entropy > self.entropy_range.1
                {
                    return false;
                }
                // Low support: only exclude NS and LS, NOT ELS.
                // ELS (support == threshold) is scientifically valid per PMC11596295.
                if !self.include_low_support {
                    if let Some(ref tag) = pos.low_support {
                        if tag == "NS" || tag == "LS" {
                            return false;
                        }
                    }
                }
                // Motif type filter: check if ANY variant has a matching motif_short
                if motif_filter_active {
                    let variants = match &pos.diversity_motifs {
                        Some(v) => v,
                        None => return false,
                    };
                    let has_match = variants.iter().any(|v| {
                        v.motif_short
                            .as_deref()
                            .is_some_and(|ms| self.motif_types.iter().any(|mt| mt.as_str() == ms))
                    });
                    if !has_match {
                        return false;
                    }
                }
                true
            })
            .map(|(idx, _)| idx)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dima_lib::{HighestEntropy, Position, Results, Variant};

    fn make_results(positions: Vec<Position>) -> Results {
        Results {
            sequence_count: 100,
            support_threshold: 10,
            low_support_count: 0,
            query_name: "test".to_string(),
            kmer_length: 3,
            highest_entropy: HighestEntropy {
                position: 1,
                entropy: 0.0,
            },
            average_entropy: 0.0,
            results: positions,
        }
    }

    fn make_position(pos: usize, entropy: f64, low_support: Option<&str>) -> Position {
        Position {
            position: pos,
            entropy,
            support: 100,
            low_support: low_support.map(|s| s.to_string()),
            diversity_motifs: Some(vec![Variant {
                sequence: "ABC".to_string(),
                count: 100,
                incidence: 100.0,
                motif_short: Some("I".to_string()),
                motif_long: Some("Index".to_string()),
                metadata: None,
            }]),
            distinct_variants_count: 1,
            distinct_variants_incidence: 0.0,
            total_variants_incidence: 0.0,
        }
    }

    #[test]
    fn test_default_for_includes_all_positions() {
        let results = make_results(vec![
            make_position(1, 0.5, None),
            make_position(2, 1.0, None),
            make_position(3, 1.5, None),
        ]);
        let filter = FilterState::default_for(&results);
        let indices = filter.apply(&results);
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_default_for_empty_results() {
        let results = make_results(vec![]);
        let filter = FilterState::default_for(&results);
        assert_eq!(filter.position_range, (1, 1));
        assert_eq!(filter.entropy_range, (0.0, 0.0));
    }

    #[test]
    fn test_inverted_position_range_returns_empty() {
        let results = make_results(vec![make_position(1, 0.5, None)]);
        let filter = FilterState {
            position_range: (5, 1),
            entropy_range: (0.0, 10.0),
            motif_types: vec![MotifType::Index],
            include_low_support: true,
        };
        assert!(filter.apply(&results).is_empty());
    }

    #[test]
    fn test_inverted_entropy_range_returns_empty() {
        let results = make_results(vec![make_position(1, 0.5, None)]);
        let filter = FilterState {
            position_range: (1, 10),
            entropy_range: (5.0, 1.0),
            motif_types: vec![MotifType::Index],
            include_low_support: true,
        };
        assert!(filter.apply(&results).is_empty());
    }

    #[test]
    fn test_low_support_ns_ls_excluded_but_els_kept() {
        let results = make_results(vec![
            make_position(1, 0.5, Some("NS")),
            make_position(2, 0.5, Some("LS")),
            make_position(3, 0.5, Some("ELS")),
            make_position(4, 0.5, None),
        ]);
        let filter = FilterState {
            position_range: (1, 10),
            entropy_range: (0.0, 10.0),
            motif_types: vec![
                MotifType::Index,
                MotifType::Major,
                MotifType::Minor,
                MotifType::Unique,
            ],
            include_low_support: false,
        };
        let indices = filter.apply(&results);
        // NS and LS excluded, ELS and None kept
        assert_eq!(indices, vec![2, 3]);
    }

    #[test]
    fn test_empty_motif_types_means_no_filter_fix_623() {
        let results = make_results(vec![make_position(1, 0.5, None)]);
        let filter = FilterState {
            position_range: (1, 10),
            entropy_range: (0.0, 10.0),
            motif_types: vec![], // Empty = no filter, NOT exclude all
            include_low_support: true,
        };
        assert_eq!(filter.apply(&results), vec![0]);
    }

    #[test]
    fn test_nan_entropy_filtered_out() {
        let results = make_results(vec![
            make_position(1, f64::NAN, None),
            make_position(2, 0.5, None),
        ]);
        let filter = FilterState::default_for(&results);
        let indices = filter.apply(&results);
        // NaN position should be excluded
        assert_eq!(indices, vec![1]);
    }

    #[test]
    fn test_motif_type_as_str_matches_lib_values() {
        assert_eq!(MotifType::Index.as_str(), "I");
        assert_eq!(MotifType::Major.as_str(), "Ma");
        assert_eq!(MotifType::Minor.as_str(), "Mi");
        assert_eq!(MotifType::Unique.as_str(), "U");
    }
}
