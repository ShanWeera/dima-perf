//! Support classes, diversity motifs and incidences of a position (Tharanga et al. 2025;
//! motif definitions after Hu et al. 2013 and Khan et al. 2008).

use crate::model::{Motif, SupportLabel};

/// Low-support class of a position: NS (N = 0), LS (0 < N < T), ELS (N = T); `None` for
/// eligible positions (N > T).
#[must_use]
pub const fn support_label(support: u64, threshold: u64) -> Option<SupportLabel> {
    if support == 0 {
        Some(SupportLabel::NoSupport)
    } else if support < threshold {
        Some(SupportLabel::Low)
    } else if support == threshold {
        Some(SupportLabel::EqualToThreshold)
    } else {
        None
    }
}

/// `count / support × 100`, rounded once (`support` must be positive).
#[must_use]
pub fn incidence(count: u64, support: u64) -> f64 {
    debug_assert!(support > 0);
    (u128::from(count) * 100) as f64 / support as f64
}

/// Classifies the variants of a position, given their counts in descending order:
///
/// * **Index**: the highest count, if above 1 (all variants tied at the highest count are Index;
///   Tharanga et al. 2025: "a position may exhibit more than one index or major variant");
/// * **Unique**: count 1;
/// * **Major**: the highest count among the rest (ties are all Major);
/// * **Minor**: everything else.
#[must_use]
pub fn classify_motifs(counts_desc: &[u64]) -> Vec<Motif> {
    debug_assert!(
        counts_desc.windows(2).all(|w| w[0] >= w[1]),
        "counts must be descending"
    );
    let Some(&max) = counts_desc.first() else {
        return Vec::new();
    };
    let major = counts_desc.iter().copied().find(|&c| c != max && c > 1);
    counts_desc
        .iter()
        .map(|&c| {
            if c == 1 {
                Motif::Unique
            } else if c == max {
                Motif::Index
            } else if Some(c) == major {
                Motif::Major
            } else {
                Motif::Minor
            }
        })
        .collect()
}

/// Diversity statistics of a position derived from its motifs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Diversity {
    /// Number of distinct non-Index variants (DiMA's `distinct_variants_count`).
    pub distinct_variants_count: u64,
    /// Non-Index variant types per 100 non-Index sequences; `None` when every sequence carries
    /// an Index variant (0/0).
    pub distinct_variants_incidence: Option<f64>,
    /// Non-Index sequences per 100 supporting sequences; `None` without support.
    pub total_variants_incidence: Option<f64>,
}

/// Computes the diversity statistics from counts and their motifs.
#[must_use]
pub fn diversity(counts: &[u64], motifs: &[Motif]) -> Diversity {
    debug_assert_eq!(counts.len(), motifs.len());
    let support: u64 = counts.iter().sum();
    let (mut types, mut reads) = (0u64, 0u64);
    for (&c, &m) in counts.iter().zip(motifs) {
        if m != Motif::Index {
            types += 1;
            reads += c;
        }
    }
    Diversity {
        distinct_variants_count: types,
        distinct_variants_incidence: (reads > 0).then(|| incidence(types, reads)),
        total_variants_incidence: (support > 0).then(|| incidence(reads, support)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Motif::{Index as I, Major as Ma, Minor as Mi, Unique as U};

    #[test]
    fn labels_at_threshold_boundaries() {
        assert_eq!(support_label(0, 100), Some(SupportLabel::NoSupport));
        assert_eq!(support_label(1, 100), Some(SupportLabel::Low));
        assert_eq!(support_label(99, 100), Some(SupportLabel::Low));
        assert_eq!(
            support_label(100, 100),
            Some(SupportLabel::EqualToThreshold)
        );
        assert_eq!(support_label(101, 100), None);
        assert_eq!(support_label(0, 1), Some(SupportLabel::NoSupport));
        assert_eq!(support_label(1, 1), Some(SupportLabel::EqualToThreshold));
    }

    #[test]
    fn motifs_position_25() {
        // MERS-CoV spike position 25 (Tharanga et al. 2025, Fig. 1C).
        let counts = [129, 14, 9, 6, 4, 4, 1, 1, 1, 1];
        let m = classify_motifs(&counts);
        assert_eq!(m, [I, Ma, Mi, Mi, Mi, Mi, U, U, U, U]);
        let d = diversity(&counts, &m);
        assert_eq!(d.distinct_variants_count, 9);
        assert!((d.distinct_variants_incidence.unwrap() - 900.0 / 41.0).abs() < 1e-12);
        assert!((d.total_variants_incidence.unwrap() - 4100.0 / 170.0).abs() < 1e-12);
    }

    #[test]
    fn motif_ties_and_edge_cases() {
        assert_eq!(classify_motifs(&[]), Vec::<Motif>::new());
        assert_eq!(classify_motifs(&[5]), [I]);
        assert_eq!(classify_motifs(&[1]), [U]);
        assert_eq!(classify_motifs(&[1, 1, 1]), [U, U, U]);
        assert_eq!(classify_motifs(&[3, 3, 2, 2, 1]), [I, I, Ma, Ma, U]);
        assert_eq!(classify_motifs(&[2, 2, 1]), [I, I, U]);
        assert_eq!(classify_motifs(&[5, 2, 1]), [I, Ma, U]);
        assert_eq!(classify_motifs(&[7, 4, 3, 3]), [I, Ma, Mi, Mi]);
    }

    #[test]
    fn diversity_nulls() {
        // Fully conserved: total 0, distinct undefined (0/0).
        let d = diversity(&[50], &[I]);
        assert_eq!(d.distinct_variants_count, 0);
        assert_eq!(d.total_variants_incidence, Some(0.0));
        assert_eq!(d.distinct_variants_incidence, None);
        // No support.
        let d = diversity(&[], &[]);
        assert_eq!(d.total_variants_incidence, None);
        assert_eq!(d.distinct_variants_incidence, None);
        // All unique: no Index, everything is variation.
        let d = diversity(&[1, 1, 1, 1], &[U, U, U, U]);
        assert_eq!(d.distinct_variants_count, 4);
        assert_eq!(d.total_variants_incidence, Some(100.0));
        assert_eq!(d.distinct_variants_incidence, Some(100.0));
    }

    #[test]
    fn incidence_is_correctly_rounded() {
        assert_eq!(incidence(1, 3), 100.0 / 3.0);
        assert_eq!(incidence(129, 170), 12900.0 / 170.0);
        assert_eq!(incidence(u64::MAX, u64::MAX), 100.0);
    }
}
