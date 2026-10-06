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

/// Columns of the position table.
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
    /// Names, in column order.
    pub const NAMES: [&str; 6] = [
        "Position",
        "Entropy",
        "Support",
        "Class",
        "Variants %",
        "Top variant",
    ];
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
        c::SUPPORT => apply(a.support.cmp(&b.support), d),
        c::CLASS => apply(class_rank(a).cmp(&class_rank(b)), d),
        c::TOTAL_VARIANTS => cmp_opt_f64(a.total_variants_incidence, b.total_variants_incidence, d),
        c::TOP_VARIANT => {
            let top = |p: &PositionResult| p.diversity_motifs.first().map(|v| v.sequence.clone());
            match (top(a), top(b)) {
                (Some(x), Some(y)) => apply(x.cmp(&y), d),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        }
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
}
