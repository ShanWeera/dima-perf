//! Table sorting. Comparisons are total (`f64::total_cmp`, never a panicking `partial_cmp`
//! unwrap; G10) and null-aware: undefined values (NS entropy, D42 nulls) sort last in both
//! directions, so they never interleave with numbers.

use std::cmp::Ordering;

use dima_core::model::{PositionResult, Variant};

/// Sort direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

impl Direction {
    /// The other direction.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    /// Header suffix.
    #[must_use]
    pub const fn arrow(self) -> &'static str {
        match self {
            Self::Ascending => " \u{23F6}",
            Self::Descending => " \u{23F7}",
        }
    }
}

/// A sorted column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableSort {
    /// Column index.
    pub column: usize,
    /// Direction.
    pub direction: Direction,
}

impl TableSort {
    /// The sort after clicking `column`: toggles the direction of the sorted column, otherwise
    /// sorts the new column ascending.
    #[must_use]
    pub fn clicked(current: Option<Self>, column: usize) -> Self {
        match current {
            Some(s) if s.column == column => Self {
                column,
                direction: s.direction.toggled(),
            },
            _ => Self {
                column,
                direction: Direction::Ascending,
            },
        }
    }
}

/// Compares optional floats: values by `total_cmp` in `direction`, `None` always last.
#[must_use]
pub fn cmp_opt_f64(a: Option<f64>, b: Option<f64>, direction: Direction) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => apply(x.total_cmp(&y), direction),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn apply(o: Ordering, d: Direction) -> Ordering {
    match d {
        Direction::Ascending => o,
        Direction::Descending => o.reverse(),
    }
}

/// The sort ids of the position table's columns (the panel decides which are shown and in
/// what order; an id is never reused, so a remembered sort keeps its meaning).
pub mod position_columns {
    /// Position.
    pub const POSITION: usize = 0;
    /// Entropy.
    pub const ENTROPY: usize = 1;
    /// Support.
    pub const SUPPORT: usize = 2;
    /// Support class.
    pub const CLASS: usize = 3;
    /// Total-variants incidence.
    pub const TOTAL_VARIANTS: usize = 4;
    /// Most common variant.
    pub const TOP_VARIANT: usize = 5;
    /// Similarity-weighted entropy (shown right after entropy when computed).
    pub const SIMILARITY_WEIGHTED: usize = 6;
}

/// Orders two positions by a column; ties keep position order (stable and deterministic).
#[must_use]
pub fn compare_positions(a: &PositionResult, b: &PositionResult, sort: TableSort) -> Ordering {
    use position_columns as c;
    let d = sort.direction;
    let class_rank = |p: &PositionResult| {
        p.low_support.map_or(0, |l| match l {
            dima_core::model::SupportLabel::EqualToThreshold => 1,
            dima_core::model::SupportLabel::Low => 2,
            dima_core::model::SupportLabel::NoSupport => 3,
        })
    };
    let o = match sort.column {
        c::ENTROPY => cmp_opt_f64(a.entropy, b.entropy, d),
        c::SIMILARITY_WEIGHTED => cmp_opt_f64(
            a.similarity_weighted_entropy,
            b.similarity_weighted_entropy,
            d,
        ),
        c::SUPPORT => apply(a.support.cmp(&b.support), d),
        c::CLASS => apply(class_rank(a).cmp(&class_rank(b)), d),
        c::TOTAL_VARIANTS => cmp_opt_f64(a.total_variants_incidence, b.total_variants_incidence, d),
        // Borrowed, never cloned: a sort makes O(n log n) comparisons.
        c::TOP_VARIANT => match (a.diversity_motifs.first(), b.diversity_motifs.first()) {
            (Some(x), Some(y)) => apply(x.sequence.cmp(&y.sequence), d),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        c::POSITION => apply(a.position.cmp(&b.position), d),
        _ => Ordering::Equal,
    };
    o.then(a.position.cmp(&b.position))
}

/// Columns of the variant table.
pub mod variant_columns {
    /// Count.
    pub const COUNT: usize = 0;
    /// Incidence.
    pub const INCIDENCE: usize = 1;
    /// Motif.
    pub const MOTIF: usize = 2;
    /// Sequence.
    pub const SEQUENCE: usize = 3;
    /// Names, in column order.
    pub const NAMES: [&str; 4] = ["Count", "Incidence", "Motif", "Sequence"];
}

/// Orders two variants by a column; ties keep the library order (by index).
#[must_use]
pub fn compare_variants(a: (usize, &Variant), b: (usize, &Variant), sort: TableSort) -> Ordering {
    use variant_columns as c;
    let d = sort.direction;
    let rank = |v: &Variant| match v.motif {
        dima_core::model::Motif::Index => 0,
        dima_core::model::Motif::Major => 1,
        dima_core::model::Motif::Minor => 2,
        dima_core::model::Motif::Unique => 3,
    };
    let o = match sort.column {
        c::COUNT => apply(a.1.count.cmp(&b.1.count), d),
        c::INCIDENCE => apply(a.1.incidence.total_cmp(&b.1.incidence), d),
        c::MOTIF => apply(rank(a.1).cmp(&rank(b.1)), d),
        c::SEQUENCE => apply(a.1.sequence.cmp(&b.1.sequence), d),
        _ => Ordering::Equal,
    };
    o.then(a.0.cmp(&b.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::series::tests::position;

    #[test]
    fn nulls_sort_last_in_both_directions_and_nan_never_panics() {
        let mut v = [Some(1.0), None, Some(f64::NAN), Some(-0.5), None, Some(3.0)];
        v.sort_by(|a, b| cmp_opt_f64(*a, *b, Direction::Ascending));
        assert_eq!(v[0], Some(-0.5));
        assert!(v[4].is_none() && v[5].is_none());
        v.sort_by(|a, b| cmp_opt_f64(*a, *b, Direction::Descending));
        assert!(v[4].is_none() && v[5].is_none());
    }

    #[test]
    fn positions_by_entropy_keep_ns_last() {
        let ps = [
            position(1, Some(0.5), 200, 100),
            position(2, None, 0, 100),
            position(3, Some(1.5), 200, 100),
        ];
        let mut idx = vec![0, 1, 2];
        let sort = TableSort {
            column: position_columns::ENTROPY,
            direction: Direction::Descending,
        };
        idx.sort_by(|&a, &b| compare_positions(&ps[a], &ps[b], sort));
        assert_eq!(idx, vec![2, 0, 1]);
        let s = TableSort::clicked(Some(sort), position_columns::ENTROPY);
        assert_eq!(s.direction, Direction::Ascending);
        let s = TableSort::clicked(Some(sort), position_columns::SUPPORT);
        assert_eq!(
            (s.column, s.direction),
            (position_columns::SUPPORT, Direction::Ascending)
        );
    }

    /// Similarity-weighted entropy sorts like entropy: values in order, missing ones last in
    /// both directions.
    #[test]
    fn positions_by_similarity_weighted_entropy_keep_missing_values_last() {
        let swe = |pos, v| {
            let mut p = position(pos, Some(1.0), 200, 100);
            p.similarity_weighted_entropy = v;
            p
        };
        let ps = [
            swe(1, Some(0.3)),
            swe(2, None),
            swe(3, Some(0.1)),
            swe(4, Some(0.3)),
        ];
        for (direction, want) in [
            (Direction::Ascending, vec![2, 0, 3, 1]),
            (Direction::Descending, vec![0, 3, 2, 1]),
        ] {
            let sort = TableSort {
                column: position_columns::SIMILARITY_WEIGHTED,
                direction,
            };
            let mut idx: Vec<usize> = (0..ps.len()).collect();
            idx.sort_by(|&a, &b| compare_positions(&ps[a], &ps[b], sort));
            assert_eq!(idx, want, "{direction:?}: ties keep position order");
        }
    }

    /// The Top-variant order as it was computed before it borrowed: from owned copies of the
    /// two top variants.
    fn owned_top_variant_order(a: &PositionResult, b: &PositionResult, d: Direction) -> Ordering {
        let top = |p: &PositionResult| p.diversity_motifs.first().map(|v| v.sequence.clone());
        let o = match (top(a), top(b)) {
            (Some(x), Some(y)) => apply(x.cmp(&y), d),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        o.then(a.position.cmp(&b.position))
    }

    proptest::proptest! {
        /// Sorting by the top variant gives the order of the owned-string comparison: ties (a
        /// two-letter alphabet), positions without variants and both directions.
        #[test]
        fn top_variant_order_matches_owned_strings(
            tops in proptest::collection::vec(
                proptest::option::of(proptest::string::string_regex("[AC]{1,3}").unwrap()),
                0..40,
            ),
            descending in proptest::bool::ANY,
        ) {
            let ps: Vec<PositionResult> = tops
                .iter()
                .zip(1u64..)
                .map(|(top, pos)| {
                    let mut p = position(pos, Some(0.5), 200, 100);
                    match top {
                        // A second variant, which must not count.
                        Some(seq) => {
                            p.diversity_motifs[0].sequence.clone_from(seq);
                            let mut other = p.diversity_motifs[0].clone();
                            other.sequence = "Z".into();
                            p.diversity_motifs.push(other);
                        }
                        None => p.diversity_motifs.clear(),
                    }
                    p
                })
                .collect();
            let direction = if descending { Direction::Descending } else { Direction::Ascending };
            let sort = TableSort { column: position_columns::TOP_VARIANT, direction };
            for a in &ps {
                for b in &ps {
                    proptest::prop_assert_eq!(
                        compare_positions(a, b, sort),
                        owned_top_variant_order(a, b, direction)
                    );
                }
            }
            let mut got: Vec<usize> = (0..ps.len()).collect();
            got.sort_by(|&a, &b| compare_positions(&ps[a], &ps[b], sort));
            let mut want: Vec<usize> = (0..ps.len()).collect();
            want.sort_by(|&a, &b| owned_top_variant_order(&ps[a], &ps[b], direction));
            proptest::prop_assert_eq!(got, want);
        }
    }
}
