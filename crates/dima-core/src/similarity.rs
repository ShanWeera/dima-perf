//! Similarity-weighted entropy of the k-mers at one position (Wu 2025).
//!
//! Shannon entropy treats two k-mers that differ at one site exactly like two unrelated ones.
//! Wu (2025, *Virus Evolution* 11:veaf029, Section 2.1 and Supplementary File 1) weights each
//! k-mer's entropy term by how dissimilar it is to the rest of the population:
//!
//! ```text
//! f_i = c_i / N,   s_ij = (k − d_ij) / k,   S_i = Σ_{j≠i} f_j · s_ij,
//! H_sim = Σ_i (−f_i log₂ f_i) · (1 − S_i)
//! ```
//!
//! where c_i is the number of sequences carrying the i-th distinct k-mer, N the support and d_ij
//! the number of sites at which two k-mers differ (their Hamming distance), so s_ij is the
//! proportion of identical residues (Grishin 1995, p. 675, for amino acids).
//!
//! # Computation without pairs
//!
//! Wu sums over every pair of distinct k-mers, which is quadratic in the number of distinct
//! k-mers D. The same sums come from counting residues per site. With c_p(a) the number of
//! sequences whose k-mer has residue `a` at offset p,
//!
//! ```text
//! Σ_p c_p(x_ip) = Σ_j c_j · (k − d_ij)     (every sequence counted once per identical site),
//! a_i = Σ_p c_p(x_ip) − k·c_i = k · Σ_{j≠i} c_j · s_ij,
//! 1 − S_i = (k·N − a_i) / (k·N),
//! ```
//!
//! the column-profile form of sum-of-pairs scores of Gotoh (1994, *Bioinformatics* 10:379,
//! eqs. (1) and (4a), with the self-pairs removed as in eq. (9b)), taken with an identity score
//! and one sequence against the whole position. The numerators k·N − a_i are exact integers
//! (`u128`; k·N < 2¹²⁸), summed exactly per count class, and each class contributes
//! `term(c, N) · (g_c / (k·N))` to a compensated sum in the same class order as observed
//! entropy. Time O(D·k), memory 208·k bytes per position being computed (a 26 × k table of
//! `u64`).
//!
//! # Bounds and exactness
//!
//! The weight 1 − S_i = f_i + Σ_j f_j·d_ij/k lies in [f_i, 1]; distinct k-mers differ at least
//! at one site, so it is at least f_i + (1 − f_i)/k, and H_sim lies between
//! Σ_i (−f_i log₂ f_i)(f_i + (1 − f_i)/k) and the observed entropy H. When every weight is 1
//! (k = 1, or every pair of k-mers differs at every site) the class sums are f_c·k·N and the
//! result equals [`crate::entropy::plug_in`] bit for bit as long as f_c·k·N ≤ 2⁵³ for every
//! class (Goldberg 1991, §2.1.4: conversion between integer and floating point and division
//! are correctly rounded, so integers up to 2⁵³ convert exactly and their quotient is the
//! correctly rounded value). Weights of 1 force all pairs to differ at the first site, so D ≤ 26 and
//! the bound holds whenever k·N ≤ 3.46 × 10¹⁴ (4 × 10¹⁴ for the 22 canonical amino-acid
//! letters).

use crate::entropy::{CountProfile, term};
use crate::error::{DimaError, Result};
use crate::numeric::Neumaier;

/// Letters a canonical k-mer can hold: `A`–`Z`.
const LETTERS: usize = 26;

/// Similarity-weighted entropy H_sim, in bits, of the distinct k-mers at one position, each with
/// the number of sequences that carry it (Wu 2025, Section 2.1 and Supplementary File 1).
///
/// The k-mers may come in any order and must be distinct (the engine counts each distinct k-mer
/// once); the result does not depend on their order. `None` when there are no k-mers; `0` for
/// a single distinct k-mer. Normalise with [`crate::entropy::normalised`] for H_nsim.
///
/// # Errors
/// [`DimaError::Internal`] for input no position can produce: a zero count, an empty k-mer,
/// k-mers of different lengths, a byte outside `A`–`Z`, or an exact sum beyond `u128` (it would
/// take more than 10¹⁹ table updates). [`DimaError::Overflow`] if the counts sum beyond
/// `u64::MAX`.
pub fn similarity_weighted_entropy<'a, I>(variants: I) -> Result<Option<f64>>
where
    I: IntoIterator<Item = (&'a [u8], u64)>,
{
    let variants: Vec<(&[u8], u64)> = variants.into_iter().collect();
    let Some(&(first, _)) = variants.first() else {
        return Ok(None);
    };
    let k = first.len();
    validate(&variants, k)?;
    let profile = CountProfile::from_counts(variants.iter().map(|&(_, c)| c))?;
    if profile.distinct() == 1 {
        return Ok(Some(0.0));
    }
    let sums = class_sums(&variants, k, &profile)?;
    let n = profile.support();
    let denominator = (k as u128 * u128::from(n)) as f64;
    let mut h = Neumaier::new();
    for (&(count, _), &g) in profile.frequencies().iter().zip(&sums) {
        h.add(term(count, n) * (g as f64 / denominator));
    }
    Ok(Some(h.total()))
}

/// Checks what [`similarity_weighted_entropy`] requires of its input.
fn validate(variants: &[(&[u8], u64)], k: usize) -> Result<()> {
    if k == 0 {
        return Err(DimaError::internal(
            "similarity-weighted entropy of empty k-mers",
        ));
    }
    for (i, &(kmer, count)) in variants.iter().enumerate() {
        if count == 0 {
            return Err(DimaError::internal(format!(
                "similarity-weighted entropy: k-mer {i} has a count of 0"
            )));
        }
        if kmer.len() != k {
            return Err(DimaError::internal(format!(
                "similarity-weighted entropy: k-mer {i} has length {}, not {k}",
                kmer.len()
            )));
        }
        if let Some(b) = kmer.iter().find(|b| !b.is_ascii_uppercase()) {
            return Err(DimaError::internal(format!(
                "similarity-weighted entropy: k-mer {i} holds byte {b:#04x}, not a letter A-Z"
            )));
        }
    }
    Ok(())
}

/// The exact numerators of the weights, summed per count class of `profile` (in its order):
/// g_c = Σ_{i: c_i = c} (k·N − a_i), with a_i = k · Σ_{j≠i} c_j · s_ij counted per site.
pub(crate) fn class_sums(
    variants: &[(&[u8], u64)],
    k: usize,
    profile: &CountProfile,
) -> Result<Vec<u128>> {
    // Residue counts per site; each entry is at most N, which fits in a u64.
    let mut table = vec![0u64; LETTERS * k];
    for &(kmer, count) in variants {
        for (p, &b) in kmer.iter().enumerate() {
            table[p * LETTERS + usize::from(b - b'A')] += count;
        }
    }
    let n = u128::from(profile.support());
    let k_wide = k as u128;
    let kn = k_wide * n;
    let classes = profile.frequencies();
    let mut sums = vec![0u128; classes.len()];
    for &(kmer, count) in variants {
        let identical: u128 = kmer
            .iter()
            .enumerate()
            .map(|(p, &b)| u128::from(table[p * LETTERS + usize::from(b - b'A')]))
            .sum();
        let own = k_wide * u128::from(count);
        if identical < own || identical > kn {
            return Err(DimaError::internal(format!(
                "similarity-weighted entropy: {identical} identical sites outside [{own}, {kn}]"
            )));
        }
        let class = classes
            .binary_search_by_key(&count, |&(c, _)| c)
            .map_err(|_| DimaError::internal("similarity-weighted entropy: count without class"))?;
        sums[class] = sums[class]
            .checked_add(kn - (identical - own))
            .ok_or_else(|| DimaError::internal("similarity-weighted entropy: sum beyond u128"))?;
    }
    Ok(sums)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::entropy::plug_in;

    fn h_sim(variants: &[(&str, u64)]) -> f64 {
        similarity_weighted_entropy(variants.iter().map(|&(s, c)| (s.as_bytes(), c)))
            .unwrap()
            .unwrap()
    }

    fn observed(counts: impl IntoIterator<Item = u64>) -> f64 {
        plug_in(&CountProfile::from_counts(counts).unwrap()).unwrap()
    }

    /// Every k-mer of length `k` over `alphabet` whose first `m` sites are free and the rest
    /// fixed to the alphabet's first letter, one sequence each: the saturated pools of Wu's
    /// Figs. 4 and 5c.
    fn saturated(m: u32, k: usize, alphabet: &[u8]) -> Vec<(Vec<u8>, u64)> {
        let base = alphabet.len();
        (0..base.pow(m))
            .map(|mut x| {
                let mut kmer = vec![alphabet[0]; k];
                for site in kmer.iter_mut().take(m as usize) {
                    *site = alphabet[x % base];
                    x /= base;
                }
                (kmer, 1)
            })
            .collect()
    }

    fn of(pool: &[(Vec<u8>, u64)]) -> f64 {
        similarity_weighted_entropy(pool.iter().map(|(s, c)| (s.as_slice(), *c)))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn closed_forms_of_saturated_pools_are_exact() {
        // m saturated sites of k at equal frequencies: weight 4^-m + 3m/(4k), H = 2m bits.
        for k in 1..=7usize {
            let pool = saturated(k as u32, k, b"ACGT");
            let w = 0.25f64.powi(k as i32) + 0.75;
            assert_eq!(
                of(&pool).to_bits(),
                (2.0 * k as f64 * w).to_bits(),
                "k = {k}"
            );
        }
        // Wu's Fig. 5c pool: four free sites in a 6-mer, H_sim = 129/32.
        let pool = saturated(4, 6, b"ACGU");
        assert_eq!(of(&pool).to_bits(), (129.0f64 / 32.0).to_bits());
    }

    #[test]
    fn small_cases() {
        assert_eq!(
            similarity_weighted_entropy(std::iter::empty::<(&[u8], u64)>()).unwrap(),
            None
        );
        assert_eq!(h_sim(&[("ACGT", 7)]), 0.0);
        // Two equal-count k-mers at distance d: (k + d) / (2k).
        assert_eq!(h_sim(&[("IAK", 4), ("LAK", 4)]), 2.0 / 3.0);
        assert_eq!(h_sim(&[("AAAA", 3), ("CCAA", 3)]), 0.75);
    }

    #[test]
    fn invalid_input_is_an_internal_error() {
        let bad: [&[(&str, u64)]; 5] = [
            &[("AC", 0)],
            &[("", 1)],
            &[("AC", 1), ("ACG", 1)],
            &[("Ac", 1)],
            &[("A-", 1)],
        ];
        for case in bad {
            let e = similarity_weighted_entropy(case.iter().map(|&(s, c)| (s.as_bytes(), c)))
                .unwrap_err();
            assert!(matches!(e, DimaError::Internal { .. }), "{case:?}: {e}");
        }
    }

    #[test]
    fn largest_exact_counts_give_observed_entropy() {
        // Three single-letter k-mers whose class sum 3c·N is just below 2^53: still bitwise.
        let c = 1_000_799_917_193_443u64;
        assert_eq!(9 * c, (1u64 << 53) - 5);
        let h = similarity_weighted_entropy([(&b"A"[..], c), (b"C", c), (b"G", c)])
            .unwrap()
            .unwrap();
        assert_eq!(h.to_bits(), observed([c, c, c]).to_bits());
    }

    #[test]
    fn very_long_kmers() {
        let k = 100_000;
        let a = vec![b'A'; k];
        let mut b = a.clone();
        b[k / 2] = b'C';
        let h = similarity_weighted_entropy([(a.as_slice(), 3), (b.as_slice(), 1)])
            .unwrap()
            .unwrap();
        let h_obs = observed([3, 1]);
        assert!(h.is_finite() && h > 0.0 && h < h_obs);
    }

    /// Distinct k-mers over a small alphabet with positive counts.
    fn population() -> impl Strategy<Value = (usize, Vec<(Vec<u8>, u64)>)> {
        (1usize..12).prop_flat_map(|k| {
            let kmer =
                proptest::collection::vec(prop_oneof![Just(b'A'), Just(b'C'), Just(b'G')], k);
            (
                Just(k),
                proptest::collection::btree_map(kmer, 1u64..40, 1..20)
                    .prop_map(|m| m.into_iter().collect::<Vec<_>>()),
            )
        })
    }

    fn naive_class_sums(pop: &[(Vec<u8>, u64)], k: usize) -> Vec<(u64, u128)> {
        let n: u64 = pop.iter().map(|(_, c)| c).sum();
        let kn = k as u128 * u128::from(n);
        let mut sums = std::collections::BTreeMap::new();
        for (i, (x, ci)) in pop.iter().enumerate() {
            let mut a = 0u128;
            for (j, (y, cj)) in pop.iter().enumerate() {
                if i != j {
                    let same = x.iter().zip(y).filter(|(p, q)| p == q).count();
                    a += u128::from(*cj) * same as u128;
                }
            }
            *sums.entry(*ci).or_insert(0u128) += kn - a;
        }
        sums.into_iter().collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        #[test]
        fn class_sums_equal_the_pairwise_sums((k, pop) in population()) {
            let view: Vec<(&[u8], u64)> = pop.iter().map(|(s, c)| (s.as_slice(), *c)).collect();
            let profile = CountProfile::from_counts(view.iter().map(|&(_, c)| c)).unwrap();
            let got = class_sums(&view, k, &profile).unwrap();
            let want = naive_class_sums(&pop, k);
            prop_assert_eq!(got.len(), want.len());
            for ((&(c, _), g), (wc, w)) in profile.frequencies().iter().zip(&got).zip(&want) {
                prop_assert_eq!(c, *wc);
                prop_assert_eq!(*g, *w);
            }
        }

        #[test]
        fn bounds_order_and_scaling((k, pop) in population(), shift in 0u32..4, rotate in 0usize..20) {
            let h = of(&pop);
            let counts: Vec<u64> = pop.iter().map(|(_, c)| *c).collect();
            let n: u64 = counts.iter().sum();
            let h_obs = observed(counts.iter().copied());
            let lower: f64 = counts.iter().map(|&c| {
                let f = c as f64 / n as f64;
                -f * f.log2() * (f + (1.0 - f) / k as f64)
            }).sum();
            prop_assert!(h <= h_obs * (1.0 + 1e-12) + 1e-300);
            prop_assert!(h >= lower * (1.0 - 1e-12));
            // Any order of the k-mers gives the same bits.
            let mut turned = pop.clone();
            turned.rotate_left(rotate % pop.len());
            turned.reverse();
            prop_assert_eq!(of(&turned).to_bits(), h.to_bits());
            // Multiplying every count by a power of two changes nothing.
            let scaled: Vec<(Vec<u8>, u64)> = pop.iter().map(|(s, c)| (s.clone(), c << shift)).collect();
            prop_assert_eq!(of(&scaled).to_bits(), h.to_bits());
            // Permuting the sites changes no distance.
            let permuted: Vec<(Vec<u8>, u64)> = pop.iter().map(|(s, c)| {
                let mut s = s.clone();
                s.rotate_left(rotate % k);
                (s, *c)
            }).collect();
            prop_assert_eq!(of(&permuted).to_bits(), h.to_bits());
        }

        #[test]
        fn single_sites_and_fully_different_kmers_give_observed_entropy(
            counts in proptest::collection::vec(1u64..1000, 1..20),
            k in 1usize..30,
        ) {
            // Distinct letters at every site: every pair differs everywhere, every weight is 1.
            let pop: Vec<(Vec<u8>, u64)> = counts.iter().enumerate()
                .map(|(i, &c)| (vec![b'A' + i as u8; k], c))
                .collect();
            prop_assert_eq!(of(&pop).to_bits(), observed(counts.iter().copied()).to_bits());
        }

        #[test]
        fn two_equal_variants_at_distance_d(k in 1usize..200, d_seed in 0usize..200, c in 1u64..1_000_000) {
            let d = 1 + d_seed % k;
            let a = vec![b'A'; k];
            let mut b = a.clone();
            for site in b.iter_mut().take(d) {
                *site = b'C';
            }
            let h = similarity_weighted_entropy([(a.as_slice(), c), (b.as_slice(), c)]).unwrap().unwrap();
            prop_assert_eq!(h.to_bits(), ((k + d) as f64 / (2 * k) as f64).to_bits());
        }
    }
}
