//! Shannon entropy of the k-mer distribution at a position, and its bias-corrected estimators.
//!
//! The estimators work on the *counts of counts* of a position ([`CountProfile`]): how many
//! distinct k-mers were seen once, twice, …. That is all they need, it is tiny, and it makes the
//! cost independent of the k-mer length. Similarity-weighted entropy also needs the k-mers
//! themselves (`crate::similarity`); [`normalised`] divides an entropy by log₂ of the number
//! of distinct k-mers.
//!
//! * [`plug_in`]: the observed (maximum-likelihood) entropy H = −Σ pᵢ log₂ pᵢ (Shannon 1948),
//!   biased downwards by about (D − 1)/(2N ln 2) bits (Miller 1955; Paninski 2003).
//! * [`dima_corrected`]: the paper's correction (Tharanga et al. 2025, Fig. 2) — entropies of
//!   subsamples of R % of the sequences are regressed on 1/n and the intercept (n → ∞) is the
//!   estimate. The paper draws one random with-replacement subsample per R; because the OLS
//!   intercept is linear in the responses, its expectation is the intercept fitted to the
//!   *expected* subsample entropies, which are computed here exactly (no random numbers). It is
//!   the conditional expectation of the random intercept given the data: the same mean and no
//!   larger variance (Rao 1945; Blackwell 1947), the limit of averaging many random subsets as
//!   DiMA's predecessors did (Khan et al. 2008, 2017; Miotto et al. 2008), and identical inputs
//!   always give identical outputs. The expected intercept is never negative (see
//!   [`dima_corrected`]), so the paper's negative-intercept rule never applies to it; DiMA v5.0.9
//!   applies it to each random run instead, which raises its long-run average at near-conserved
//!   positions.
//!
//! Every reported entropy is checked to be a finite number of at least 0 bits ([`estimate`]):
//! the estimators guarantee it, so a violation is a bug and stops the run as an internal error
//! instead of being replaced by another value (Wilson et al. 2014, Practice 5a; Hunter-Zinck et
//! al. 2021, Rule 5).
//! * [`miller_madow`]: H + (D − 1)/(2N ln 2) (Miller 1955).
//! * [`chao_wang_jost`]: Chao et al. (2013), the least biased estimator in their
//!   comparison.
//! * [`sample_coverage`]: estimated sample coverage (Chao & Jost 2012), the probability that
//!   the next sequence carries an already-seen k-mer.

use std::f64::consts::LN_2;

use crate::config::Estimator;
use crate::error::{DimaError, Result};
use crate::model::{EntropyFallback, EntropyMethod};
use crate::numeric::{Neumaier, binomial_walk, digamma_difference, lerch_phi1, ols_intercept};

/// Minimum number of rarefaction points for the regression: DiMA v5.0.9's (`calculate_entropy`);
/// the paper states none.
pub const RAREFACTION_MIN_POINTS: usize = 5;
/// Lowest subsample percentage of the rarefaction grid (Tharanga et al. 2025, Fig. 2).
pub const RAREFACTION_START: u64 = 50;
/// Step of the rarefaction grid, in per cent: Fig. 2 ends "…, 90, 95, 100%"; DiMA v5.0.9 steps
/// by 5 from the start.
pub const RAREFACTION_STEP: u64 = 5;

/// Counts of counts of the k-mers at one position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountProfile {
    support: u64,
    distinct: u64,
    /// `(count, number of distinct k-mers with that count)`, ascending by count.
    frequencies: Vec<(u64, u64)>,
}

impl CountProfile {
    /// Builds the profile from per-k-mer counts (zeros are ignored).
    ///
    /// # Errors
    /// [`DimaError::Overflow`] if the counts sum beyond `u64::MAX`.
    pub fn from_counts(counts: impl IntoIterator<Item = u64>) -> Result<Self> {
        let mut v: Vec<u64> = counts.into_iter().filter(|&c| c > 0).collect();
        v.sort_unstable();
        let mut support = 0u64;
        let mut frequencies: Vec<(u64, u64)> = Vec::new();
        for c in &v {
            support = support.checked_add(*c).ok_or(DimaError::Overflow {
                what: "the support",
            })?;
            match frequencies.last_mut() {
                Some((last, f)) if last == c => *f += 1,
                _ => frequencies.push((*c, 1)),
            }
        }
        Ok(Self {
            support,
            distinct: v.len() as u64,
            frequencies,
        })
    }

    /// Support *N*: the number of sequences with a valid k-mer.
    #[must_use]
    pub const fn support(&self) -> u64 {
        self.support
    }

    /// Number of distinct k-mers *D*.
    #[must_use]
    pub const fn distinct(&self) -> u64 {
        self.distinct
    }

    /// `(count, multiplicity)` pairs, ascending by count.
    #[must_use]
    pub fn frequencies(&self) -> &[(u64, u64)] {
        &self.frequencies
    }

    /// Number of distinct k-mers seen exactly `count` times (f₁ for 1, f₂ for 2, …).
    #[must_use]
    pub fn with_count(&self, count: u64) -> u64 {
        self.frequencies
            .binary_search_by_key(&count, |&(c, _)| c)
            .map_or(0, |i| self.frequencies[i].1)
    }
}

/// The result of estimating the entropy of one position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    /// The estimate in bits; `None` without support.
    pub entropy: Option<f64>,
    /// The observed (plug-in) entropy in bits; `None` without support.
    pub observed: Option<f64>,
    /// The method that produced `entropy`.
    pub method: EntropyMethod,
    /// Why the requested estimator was not used, if it was not.
    pub fallback: Option<EntropyFallback>,
}

/// Estimates the entropy of a position with the requested estimator, applying the documented
/// fallbacks (support ≤ T for `dima`; N = 1 for Miller–Madow and Chao–Wang–Jost).
///
/// # Errors
/// [`DimaError::Internal`] if a reported value is not a finite number of at least 0 bits (it
/// cannot be for valid counts; a violation is a bug).
pub fn estimate(profile: &CountProfile, estimator: Estimator, threshold: u64) -> Result<Estimate> {
    let Some(observed) = plug_in(profile) else {
        return Ok(Estimate {
            entropy: None,
            observed: None,
            method: EntropyMethod::NotEstimable,
            fallback: None,
        });
    };
    let plug = |fallback| Estimate {
        entropy: Some(observed),
        observed: Some(observed),
        method: EntropyMethod::PlugIn,
        fallback,
    };
    let corrected = |value: f64, method| Estimate {
        entropy: Some(value),
        observed: Some(observed),
        method,
        fallback: None,
    };
    let e = match estimator {
        Estimator::Raw => plug(None),
        Estimator::Dima => match dima_corrected(profile, threshold) {
            Ok(v) => corrected(v, EntropyMethod::Dima),
            Err(reason) => plug(Some(reason)),
        },
        Estimator::MillerMadow => match miller_madow(profile) {
            Some(v) => corrected(v, EntropyMethod::MillerMadow),
            None => plug(Some(EntropyFallback::SingleObservation)),
        },
        Estimator::ChaoWangJost => match chao_wang_jost(profile) {
            Some(v) => corrected(v, EntropyMethod::ChaoWangJost),
            None => plug(Some(EntropyFallback::SingleObservation)),
        },
    };
    reportable(e, profile, threshold)
}

/// The postcondition of [`estimate`]: every reported entropy is a finite number of at least 0
/// bits. The estimators guarantee it (see [`dima_corrected`] for the paper's correction), so a
/// violation is a bug, never a property of the data.
fn reportable(e: Estimate, profile: &CountProfile, threshold: u64) -> Result<Estimate> {
    let valid = |v: Option<f64>| v.is_none_or(|x| (0.0..f64::INFINITY).contains(&x));
    if valid(e.entropy) && valid(e.observed) {
        Ok(e)
    } else {
        Err(DimaError::internal(format!(
            "entropy {:?} (observed {:?}) from {:?} is not a finite number of at least 0 bits \
             (support {}, threshold {threshold}, counts of counts {:?})",
            e.entropy,
            e.observed,
            e.method,
            profile.support(),
            &profile.frequencies()[..profile.frequencies().len().min(8)],
        )))
    }
}

/// Plug-in Shannon entropy in bits, H = −Σ (c/N) log₂(c/N) (Shannon 1948); `None` when N = 0.
#[must_use]
pub fn plug_in(profile: &CountProfile) -> Option<f64> {
    let n = profile.support;
    if n == 0 {
        return None;
    }
    let mut h = Neumaier::new();
    for &(c, f) in &profile.frequencies {
        if c != n {
            h.add(f as f64 * term(c, n));
        }
    }
    Some(h.total())
}

/// One k-mer's term of the plug-in entropy, −p log₂ p with p = `count` / `support`, evaluated
/// exactly as [`plug_in`] does, so sums built from it match [`plug_in`] bit for bit.
pub(crate) fn term(count: u64, support: u64) -> f64 {
    let p = count as f64 / support as f64;
    -p * libm::log2(p)
}

/// An entropy divided by log₂ D, the largest observed entropy possible with D distinct k-mers:
/// x = H / log₂ D (Wu 2025, Section 2.1 and Supplementary File 1, H_n and H_nsim; the form of
/// Pielou's J′ with the observed D). `None` when D ≤ 1, where the maximum is 0 and the ratio is
/// undefined.
///
/// A ratio that rounding put just above 1 is capped at 1: the entropy and log₂ D each carry
/// rounding (the logarithm need not be correctly rounded; Goldberg 1991, §2.1.4), so a quotient
/// that is 1 in exact arithmetic can be a few units in the last place above it.
///
/// # Errors
/// [`DimaError::Internal`] if the ratio is not a number in [0, 1 + 10⁻¹²]: the entropies given
/// to it never exceed log₂ D, so a violation is a bug.
pub fn normalised(entropy: f64, distinct: u64) -> Result<Option<f64>> {
    if distinct <= 1 {
        return Ok(None);
    }
    let x = entropy / libm::log2(distinct as f64);
    if !(0.0..=1.0 + 1e-12).contains(&x) {
        return Err(DimaError::internal(format!(
            "normalised entropy {x:?} (entropy {entropy:?}, {distinct} distinct k-mers) is not a \
             number between 0 and 1"
        )));
    }
    Ok(Some(if x == 0.0 { 0.0 } else { x.min(1.0) }))
}

/// The paper's rarefaction grid for support `n` and threshold `t` (Fig. 2), with the step and
/// start rounding of DiMA v5.0.9: R % from max(50, ⌈100·T/N⌉ rounded up to a multiple of 5) to
/// 95 in steps of 5 (the 100% point is added separately). Empty when ⌈100·T/N⌉ > 95.
#[must_use]
pub fn rarefaction_grid(n: u64, t: u64) -> Vec<u64> {
    if n == 0 {
        return Vec::new();
    }
    // ⌈100·T/N⌉ in exact integer arithmetic (D55).
    let x = (u128::from(t) * 100).div_ceil(u128::from(n));
    let start = if x < u128::from(RAREFACTION_START) {
        u128::from(RAREFACTION_START)
    } else {
        x.div_ceil(u128::from(RAREFACTION_STEP)) * u128::from(RAREFACTION_STEP)
    };
    #[allow(clippy::cast_possible_truncation)] // start ≤ 100 here or the range is empty.
    let start = start.min(100) as u64;
    (start..100).step_by(RAREFACTION_STEP as usize).collect()
}

/// The smallest support at which the `dima` estimator corrects a position's entropy with
/// support threshold `threshold` (T ≥ 1): ⌈1.25·T⌉, below which the grid has fewer than
/// [`RAREFACTION_MIN_POINTS`] points (`too_few_points`), except T = 2, where support 3 gives
/// equal subsample sizes (`degenerate_regression`) and 4 is the smallest. Messages and help
/// texts state the rule with it; a unit test checks it against [`dima_corrected`].
#[must_use]
pub const fn min_corrected_support(threshold: u64) -> u64 {
    if threshold == 2 {
        return 4;
    }
    // ⌈5T/4⌉ = ⌈1.25·T⌉ in exact integer arithmetic, saturating for absurd thresholds.
    let s = (threshold as u128 * 5).div_ceil(4);
    if s > u64::MAX as u128 {
        u64::MAX
    } else {
        s as u64
    }
}

/// The largest support threshold T ≥ 1 with which the `dima` estimator corrects a position of
/// support `support`: ⌊0.8·N⌋ (the inverse of [`min_corrected_support`]), 1 for N = 3, and
/// `None` for N ≤ 1, which no threshold makes eligible.
#[must_use]
pub const fn max_correcting_threshold(support: u64) -> Option<u64> {
    match support {
        0 | 1 => None,
        3 => Some(1),
        // ⌊4N/5⌋ = ⌊0.8·N⌋ in exact integer arithmetic; at most N, so it fits.
        n => Some((n as u128 * 4 / 5) as u64),
    }
}

/// Subsample size ⌈R·N/100⌉ for R % of `n` sequences (Fig. 2, "Upperbound").
#[must_use]
pub fn subsample_size(n: u64, percent: u64) -> u64 {
    #[allow(clippy::cast_possible_truncation)] // ≤ n.
    let size = (u128::from(n) * u128::from(percent)).div_ceil(100) as u64;
    size
}

/// Expected plug-in entropy (bits) of a with-replacement subsample of `size` sequences drawn
/// from the position's empirical k-mer distribution: `E[H(Multinomial(size, p̂))]`.
///
/// By linearity, `E[H] = Σ_k E[h(X_k)]` with `X_k ~ Binomial(size, c_k/N)` and
/// h(x) = −(x/size) log₂(x/size); each binomial expectation is evaluated exactly (to 10⁻¹⁸ of
/// the probability mass) by walking the pmf from its mode.
#[must_use]
pub fn expected_subsample_entropy(profile: &CountProfile, size: u64) -> f64 {
    let n = profile.support;
    if size <= 1 || n == 0 {
        return 0.0;
    }
    let sf = size as f64;
    let h = |x: u64| {
        if x == 0 {
            0.0
        } else {
            let q = x as f64 / sf;
            -q * libm::log2(q)
        }
    };
    let mut total = Neumaier::new();
    for &(c, f) in &profile.frequencies {
        if c == n {
            continue; // p = 1: every subsample is pure, h = 0.
        }
        let p = c as f64 / n as f64;
        let mut e = 0.0;
        let w = binomial_walk(size, p, |x, w| e += w * h(x));
        total.add(f as f64 * e / w);
    }
    total.total()
}

/// The `dima` estimate in bits: the exact expectation of the paper's procedure (Fig. 2).
///
/// The intercept is never negative: the expected subsample entropy never decreases with the
/// subsample size n (the frequencies of n + 1 draws average those of their n + 1 subsets of n
/// draws, and entropy is concave) and never exceeds the observed entropy (Jensen's inequality:
/// a with-replacement subsample's expected frequencies are the observed ones), so the points
/// never rise as 1/n grows, the least-squares slope is at most 0, and the intercept is at
/// least the mean of the points, which is at least 0. The paper's fallback for a negative
/// intercept therefore never applies; [`estimate`] checks the result like every other.
///
/// # Errors
/// The reason the paper's procedure reports the observed entropy instead: support N ≤ T,
/// fewer than five points, or a degenerate regression.
pub fn dima_corrected(profile: &CountProfile, threshold: u64) -> Result<f64, EntropyFallback> {
    let n = profile.support;
    if n <= threshold {
        return Err(EntropyFallback::SupportAtOrBelowThreshold);
    }
    let grid = rarefaction_grid(n, threshold);
    if grid.len() + 1 < RAREFACTION_MIN_POINTS {
        return Err(EntropyFallback::TooFewPoints);
    }
    let observed = plug_in(profile).ok_or(EntropyFallback::SupportAtOrBelowThreshold)?;
    let mut xs = Vec::with_capacity(grid.len() + 1);
    let mut ys = Vec::with_capacity(grid.len() + 1);
    for r in grid {
        let size = subsample_size(n, r);
        xs.push(1.0 / size as f64);
        ys.push(expected_subsample_entropy(profile, size));
    }
    // The 100% point is the full data set itself (not resampled), as in the paper.
    xs.push(1.0 / n as f64);
    ys.push(observed);
    match ols_intercept(&xs, &ys) {
        None => Err(EntropyFallback::DegenerateRegression),
        Some(b0) => Ok(if b0 == 0.0 { 0.0 } else { b0 }),
    }
}

/// Miller–Madow estimate in bits, H + (D − 1)/(2N ln 2) (Miller 1955); `None` when N < 2.
#[must_use]
pub fn miller_madow(profile: &CountProfile) -> Option<f64> {
    let n = profile.support;
    if n < 2 {
        return None;
    }
    let h = plug_in(profile)?;
    Some(h + (profile.distinct - 1) as f64 / (2.0 * n as f64 * LN_2))
}

/// The coverage-adjustment constant A of Chao et al. (2013), shared with
/// [`sample_coverage`]: 2f₂/((N−1)f₁ + 2f₂) if f₂ > 0, else 2/((N−1)(f₁−1) + 2) if f₁ > 0, else 1.
fn coverage_constant(n: u64, f1: u64, f2: u64) -> f64 {
    let n1 = u128::from(n - 1);
    if f2 > 0 {
        (2 * u128::from(f2)) as f64 / (n1 * u128::from(f1) + 2 * u128::from(f2)) as f64
    } else if f1 > 0 {
        2.0 / (n1 * u128::from(f1 - 1) + 2) as f64
    } else {
        1.0
    }
}

/// Chao–Wang–Jost (2013) entropy estimate in bits; `None` when N < 2.
///
/// Ĥ = Σ_{1≤c<N} f_c (c/N)(ψ(N) − ψ(c)) + (f₁/N)(1−A)^{1−N}(−ln A − Σ_{r=1}^{N−1}(1−A)^r/r).
/// Because −ln A = Σ_{r≥1}(1−A)^r/r, the second term equals (f₁/N)(1−A)·Φ(1−A, 1, N), where
/// Φ(1−A, 1, N) = Σ_{j≥0}(1−A)^j/(N+j) is the Lerch transcendent, evaluated exactly (up to
/// rounding) in a time independent of N (`numeric::lerch_phi1`), so (1−A)^{1−N} is never formed.
#[must_use]
pub fn chao_wang_jost(profile: &CountProfile) -> Option<f64> {
    let n = profile.support;
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mut part1 = Neumaier::new();
    for &(c, f) in &profile.frequencies {
        if c < n {
            part1.add(f as f64 * (c as f64 / nf) * digamma_difference(n, c));
        }
    }
    let f1 = profile.with_count(1);
    let f2 = profile.with_count(2);
    let a = coverage_constant(n, f1, f2);
    let part2 = if f1 == 0 || a >= 1.0 {
        0.0
    } else {
        f1 as f64 / nf * (1.0 - a) * lerch_phi1(n, a)
    };
    let h = (part1.total() + part2) / LN_2;
    Some(if h == 0.0 { 0.0 } else { h })
}

/// Estimated sample coverage Ĉ = 1 − (f₁/N)·A' (Chao & Jost 2012, Eq. 4a), where
/// A' = (N−1)f₁/((N−1)f₁ + 2f₂), or (N−1)(f₁−1)/((N−1)(f₁−1) + 2) when f₂ = 0; 1 when f₁ = 0.
/// `None` when N < 2.
#[must_use]
pub fn sample_coverage(profile: &CountProfile) -> Option<f64> {
    let n = profile.support;
    if n < 2 {
        return None;
    }
    let f1 = profile.with_count(1);
    if f1 == 0 {
        return Some(1.0);
    }
    let f2 = profile.with_count(2);
    let n1 = u128::from(n - 1);
    let ratio = if f2 > 0 {
        (n1 * u128::from(f1)) as f64 / (n1 * u128::from(f1) + 2 * u128::from(f2)) as f64
    } else {
        (n1 * u128::from(f1 - 1)) as f64 / (n1 * u128::from(f1 - 1) + 2) as f64
    };
    Some(1.0 - f1 as f64 / n as f64 * ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(counts: &[u64]) -> CountProfile {
        CountProfile::from_counts(counts.iter().copied()).unwrap()
    }

    /// MERS-CoV spike position 25 (Tharanga et al. 2025, Fig. 1C dataset).
    const POS25: [u64; 10] = [129, 14, 9, 6, 4, 4, 1, 1, 1, 1];

    /// Agreement with a reference value by the rule of the integration tests
    /// (`tests/common/mod.rs`): within a relative 1e-13, so a zero must be exactly zero.
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-13 * b.abs()
    }

    #[test]
    fn profile_basics() {
        let p = profile(&[3, 0, 1, 1, 2, 3, 3]);
        assert_eq!(p.support(), 13);
        assert_eq!(p.distinct(), 6);
        assert_eq!(p.frequencies(), &[(1, 2), (2, 1), (3, 3)]);
        assert_eq!(p.with_count(1), 2);
        assert_eq!(p.with_count(4), 0);
        assert!(matches!(
            CountProfile::from_counts([u64::MAX, 1]),
            Err(DimaError::Overflow { .. })
        ));
    }

    /// The estimators at position 25, against the reference implementation's values for
    /// `positions[24]` of `dima_reference.py analyze -i samples/mers_spike_aa.fasta` (with `-s 30`
    /// for T = 30; the same values as `MERS_AA_T30_POS25` and `MERS_AA_T100_POS25` in
    /// `tests/paper_expected/mod.rs`). Each is also the exact value correctly rounded.
    #[test]
    fn golden_values_position_25() {
        let p = profile(&POS25);
        assert_eq!(p.support(), 170);
        assert!(close(plug_in(&p).unwrap(), 1.422_388_672_162_913_5));
        assert!(close(
            dima_corrected(&p, 30).unwrap(),
            1.445_053_291_028_629_3
        ));
        assert!(close(
            dima_corrected(&p, 100).unwrap(),
            1.461_701_237_532_242_2
        ));
        assert!(close(miller_madow(&p).unwrap(), 1.460_577_658_539_386_1));
        assert!(close(chao_wang_jost(&p).unwrap(), 1.490_071_840_297_816));
        // 1 − (4/170)·(507/509), exactly 42251/43265.
        assert!(close(sample_coverage(&p).unwrap(), 42_251.0 / 43_265.0));
    }

    #[test]
    fn grid_follows_figure_2() {
        assert_eq!(
            rarefaction_grid(170, 30),
            vec![50, 55, 60, 65, 70, 75, 80, 85, 90, 95]
        );
        // ⌈100·100/170⌉ = 59 → 60.
        assert_eq!(
            rarefaction_grid(170, 100),
            vec![60, 65, 70, 75, 80, 85, 90, 95]
        );
        // Exact integer ceiling: T = 55, N = 100 → 55 (a float computation gives 56 → 60).
        assert_eq!(rarefaction_grid(100, 55)[0], 55);
        // ⌈100·100/101⌉ = 100 → empty.
        assert!(rarefaction_grid(101, 100).is_empty());
        assert_eq!(rarefaction_grid(125, 100), vec![80, 85, 90, 95]);
        assert_eq!(subsample_size(170, 50), 85);
        assert_eq!(subsample_size(171, 50), 86);
        assert_eq!(
            subsample_size(u64::MAX, 95),
            (u128::from(u64::MAX) * 95).div_ceil(100) as u64
        );
    }

    #[test]
    fn dima_fallbacks() {
        let p = profile(&POS25);
        assert_eq!(
            dima_corrected(&p, 170),
            Err(EntropyFallback::SupportAtOrBelowThreshold)
        );
        assert_eq!(
            dima_corrected(&p, 500),
            Err(EntropyFallback::SupportAtOrBelowThreshold)
        );
        // N = 125, T = 100: x = 80 → 4 grid points + 100% = 5 points: estimated.
        let q = profile(&[100, 20, 5]);
        assert!(dima_corrected(&q, 100).is_ok());
        // N = 120, T = 100: x = 84 → 85, 90, 95 + 100% = 4 points: too few.
        let q = profile(&[100, 15, 5]);
        assert_eq!(dima_corrected(&q, 100), Err(EntropyFallback::TooFewPoints));
        // A conserved position: every expected entropy is 0, intercept 0 (not negative).
        assert_eq!(dima_corrected(&profile(&[500]), 100), Ok(0.0));
        let e = estimate(&profile(&[500]), Estimator::Dima, 100).unwrap();
        assert_eq!(e.entropy, Some(0.0));
        assert!(e.entropy.unwrap().is_sign_positive());
    }

    /// The rule that messages and help texts state, against the estimator itself: with T ≥ 1,
    /// a position of support N > T is corrected exactly when N ≥ ⌈1.25·T⌉, except T = 2, N = 3
    /// (equal subsample sizes); `max_correcting_threshold` is its inverse.
    #[test]
    fn the_correction_rule_is_exact() {
        // The fallback depends on N and T only; two variants keep the entropy above 0.
        let two = |n: u64| profile(&[n - 1, 1]);
        let corrected = |n: u64, t: u64| dima_corrected(&two(n), t).is_ok();
        for t in 1..=300u64 {
            let min = min_corrected_support(t);
            for n in t + 1..=2 * t + 3 {
                assert_eq!(corrected(n, t), n >= min, "T = {t}, N = {n}");
                if n > t && n < min {
                    let reason = dima_corrected(&two(n), t).unwrap_err();
                    let expected = if t == 2 {
                        EntropyFallback::DegenerateRegression
                    } else {
                        EntropyFallback::TooFewPoints
                    };
                    assert_eq!(reason, expected, "T = {t}, N = {n}");
                }
            }
        }
        for n in 0..=400u64 {
            match max_correcting_threshold(n) {
                None => assert!(n <= 1, "N = {n}"),
                Some(t) => {
                    assert!(t >= 1 && corrected(n, t), "N = {n}, T = {t}");
                    assert!(!corrected(n, t + 1), "N = {n}, T = {}", t + 1);
                }
            }
        }
        assert_eq!(
            [1, 2, 20, 25, 30, 100].map(min_corrected_support),
            [2, 4, 25, 32, 38, 125]
        );
        assert_eq!(max_correcting_threshold(105), Some(84));
        assert_eq!(max_correcting_threshold(26), Some(20));
        assert_eq!(min_corrected_support(u64::MAX), u64::MAX);
        assert_eq!(
            max_correcting_threshold(u64::MAX),
            Some(14_757_395_258_967_641_292)
        );
    }

    #[test]
    fn estimate_methods_and_fallbacks() {
        let empty = profile(&[]);
        let e = estimate(&empty, Estimator::Dima, 100).unwrap();
        assert_eq!(e.entropy, None);
        assert_eq!(e.observed, None);
        assert_eq!(e.method, EntropyMethod::NotEstimable);
        let one = profile(&[1]);
        for est in [Estimator::MillerMadow, Estimator::ChaoWangJost] {
            let e = estimate(&one, est, 100).unwrap();
            assert_eq!(e.entropy, Some(0.0));
            assert_eq!(e.method, EntropyMethod::PlugIn);
            assert_eq!(e.fallback, Some(EntropyFallback::SingleObservation));
        }
        let e = estimate(&one, Estimator::Dima, 100).unwrap();
        assert_eq!(e.fallback, Some(EntropyFallback::SupportAtOrBelowThreshold));
        let p = profile(&POS25);
        let e = estimate(&p, Estimator::Raw, 100).unwrap();
        assert_eq!((e.method, e.fallback), (EntropyMethod::PlugIn, None));
        assert_eq!(e.entropy, e.observed);
        let e = estimate(&p, Estimator::Dima, 30).unwrap();
        assert_eq!(e.method, EntropyMethod::Dima);
        assert!(e.entropy.unwrap() > e.observed.unwrap());
    }

    /// E[H_n] equals brute-force enumeration over all multinomial outcomes.
    #[test]
    fn expected_subsample_entropy_matches_enumeration() {
        fn enumerate(probs: &[f64], n: u64) -> f64 {
            // Recursive enumeration of all count vectors summing to n.
            fn rec(probs: &[f64], n: u64, left: u64, counts: &mut Vec<u64>, acc: &mut f64) {
                if counts.len() + 1 == probs.len() {
                    counts.push(left);
                    let mut logp = libm::lgamma(n as f64 + 1.0);
                    let mut h = 0.0;
                    for (&c, &p) in counts.iter().zip(probs) {
                        logp += c as f64 * p.ln() - libm::lgamma(c as f64 + 1.0);
                        if c > 0 {
                            let q = c as f64 / n as f64;
                            h -= q * q.log2();
                        }
                    }
                    *acc += logp.exp() * h;
                    counts.pop();
                    return;
                }
                for c in 0..=left {
                    counts.push(c);
                    rec(probs, n, left - c, counts, acc);
                    counts.pop();
                }
            }
            let mut acc = 0.0;
            rec(probs, n, n, &mut Vec::new(), &mut acc);
            acc
        }
        for counts in [
            &[3u64, 1][..],
            &[2, 2, 1],
            &[5, 3, 2, 1, 1],
            &[1, 1, 1, 1],
            &[7, 4],
        ] {
            let p = profile(counts);
            let total: u64 = counts.iter().sum();
            let probs: Vec<f64> = counts.iter().map(|&c| c as f64 / total as f64).collect();
            for size in 1..=12 {
                let got = expected_subsample_entropy(&p, size);
                let want = enumerate(&probs, size);
                assert!(
                    (got - want).abs() < 1e-12,
                    "{counts:?} n={size}: {got} vs {want}"
                );
            }
        }
        // E[H_1] = 0 and a pure position stays 0.
        assert_eq!(expected_subsample_entropy(&profile(&POS25), 1), 0.0);
        assert_eq!(expected_subsample_entropy(&profile(&[50]), 30), 0.0);
    }

    #[test]
    fn miller_madow_and_cwj_edge_cases() {
        // All identical: every estimator gives 0.
        let p = profile(&[40]);
        assert_eq!(miller_madow(&p), Some(0.0));
        assert_eq!(chao_wang_jost(&p), Some(0.0));
        assert_eq!(sample_coverage(&p), Some(1.0));
        // f1 = 1, f2 = 0: A = 1, part 2 vanishes.
        let p = profile(&[10, 1]);
        let part1 = (10.0 / 11.0 * digamma_difference(11, 10)
            + 1.0 / 11.0 * digamma_difference(11, 1))
            / LN_2;
        assert!(close(chao_wang_jost(&p).unwrap(), part1));
        // All singletons (f1 = N): defined and above the plug-in value log2(N).
        let p = profile(&[1; 50]);
        assert!(chao_wang_jost(&p).unwrap() > 50f64.log2());
        assert!(sample_coverage(&p).unwrap() < 0.05);
        // Profiles around A·N ≈ 9 are finite and positive.
        for (f1, f2) in [(30u64, 3u64), (20, 1), (200, 50), (3, 1)] {
            let mut counts = vec![1; f1 as usize];
            counts.extend(std::iter::repeat_n(2, f2 as usize));
            counts.push(1000);
            let v = chao_wang_jost(&profile(&counts)).unwrap();
            assert!(v.is_finite() && v > 0.0);
        }
        // N = 17.5 million with A·N ≈ 8.9, where summing the term as written loses 8 digits; the
        // value (mpmath at 50 digits, and the reference implementation) is 0.014173273093526109.
        let mut counts = vec![17_490_100u64];
        counts.extend(std::iter::repeat_n(1, 1000));
        counts.extend(std::iter::repeat_n(2, 4450));
        let v = chao_wang_jost(&profile(&counts)).unwrap();
        assert!(
            (v - 0.014_173_273_093_526_109).abs() <= 2.0 * f64::EPSILON * v,
            "{v}"
        );
        // Large N is stable and fast.
        let mut big = vec![1_000_000u64, 50_000, 3, 2, 2];
        big.extend(std::iter::repeat_n(1, 1000));
        let p = profile(&big);
        let c = chao_wang_jost(&p).unwrap();
        let h = plug_in(&p).unwrap();
        assert!(c > h && c.is_finite());
    }

    #[test]
    fn coverage_cases() {
        assert_eq!(sample_coverage(&profile(&[1])), None);
        assert_eq!(sample_coverage(&profile(&[])), None);
        // f2 = 0 branch: N = 4, f1 = 1 → A' = 0 → Ĉ = 1.
        assert_eq!(
            sample_coverage(&profile(&[3, 1])).map(|c| c.to_bits()),
            Some(1.0f64.to_bits())
        );
        let c = sample_coverage(&profile(&[3, 1, 1])).unwrap();
        // N = 5, f1 = 2, f2 = 0: 1 − (2/5)·(4·1/(4·1+2)) = 1 − 0.4·(2/3).
        assert!(close(c, 1.0 - 0.4 * (2.0 / 3.0)));
    }

    /// The regression points of [`dima_corrected`] (resampled points, then the 100% point).
    fn points(p: &CountProfile, threshold: u64) -> Vec<(f64, f64)> {
        let n = p.support;
        let mut out: Vec<(f64, f64)> = rarefaction_grid(n, threshold)
            .into_iter()
            .map(|r| {
                let size = subsample_size(n, r);
                (1.0 / size as f64, expected_subsample_entropy(p, size))
            })
            .collect();
        out.push((1.0 / n as f64, plug_in(p).unwrap()));
        out
    }

    /// The proof that the expected intercept is never negative, step by step: the expected
    /// subsample entropy never falls as n grows, never exceeds the observed entropy, and the
    /// intercept is at least the mean of the points.
    fn intercept_bounds_hold(counts: &[u64], threshold: u64) {
        let p = profile(counts);
        let observed = plug_in(&p).unwrap();
        let tol = |x: f64| 1e-12 * x.abs().max(1e-300) + 1e-15;
        let mut pts = points(&p, threshold);
        pts.sort_by(|a, b| b.0.total_cmp(&a.0)); // by n, ascending
        for w in pts.windows(2) {
            assert!(
                w[1].1 >= w[0].1 - tol(w[1].1),
                "{counts:?}: E[H_n] fell: {w:?}"
            );
        }
        for &(_, y) in &pts {
            assert!(
                y <= observed + tol(observed),
                "{counts:?}: {y} > {observed}"
            );
        }
        let mean = pts.iter().map(|&(_, y)| y).sum::<f64>() / pts.len() as f64;
        if let Ok(h) = dima_corrected(&p, threshold) {
            assert!(
                h >= mean - tol(mean) && h >= 0.0,
                "{counts:?}: {h} < {mean}"
            );
        }
    }

    #[test]
    fn a_degenerate_regression_falls_back() {
        // N = 3, T = 2: the grid is 70..95% and every subsample size rounds up to 3 = N, so all
        // the points share one x and no line is defined.
        assert_eq!(
            dima_corrected(&profile(&[2, 1]), 2),
            Err(EntropyFallback::DegenerateRegression)
        );
    }

    #[test]
    fn reported_entropies_are_finite_and_not_negative() {
        let p = profile(&[3, 1]);
        let e = |entropy, observed| Estimate {
            entropy,
            observed,
            method: EntropyMethod::Dima,
            fallback: None,
        };
        for ok in [Some(0.0), Some(-0.0), Some(1.5), None] {
            assert!(reportable(e(ok, ok), &p, 1).is_ok());
        }
        for bad in [-1e-300, -0.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for (a, b) in [(Some(bad), Some(0.5)), (Some(0.5), Some(bad))] {
                let err = reportable(e(a, b), &p, 1).unwrap_err();
                assert!(matches!(err, DimaError::Internal { .. }), "{err}");
            }
        }
    }

    #[test]
    fn the_expected_intercept_is_never_negative_near_conservation() {
        // One to five sequences differing from the Index, where single random runs of the paper's
        // procedure most often give a negative intercept; T near N gives the shortest grids.
        for n in [101, 110, 125, 130, 171, 500, 1_000, 10_000, 1_000_000] {
            for minor in [vec![1], vec![1, 1], vec![2], vec![1, 1, 1, 1, 1], vec![5]] {
                let mut counts = vec![n - minor.iter().sum::<u64>()];
                counts.extend(&minor);
                for t in [1, 30, 100, n / 2, n - 1] {
                    intercept_bounds_hold(&counts, t);
                }
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn the_expected_intercept_is_never_negative(
            counts in proptest::collection::vec(1u64..400, 1..12),
            t_fraction in 0.0f64..1.0,
        ) {
            let n: u64 = counts.iter().sum();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let threshold = ((n as f64 * t_fraction) as u64).max(1);
            intercept_bounds_hold(&counts, threshold);
        }
    }

    #[test]
    fn normalised_is_undefined_for_one_kmer_and_capped_at_one() {
        assert_eq!(normalised(0.0, 0).unwrap(), None);
        assert_eq!(normalised(0.0, 1).unwrap(), None);
        // A uniform distribution over D k-mers has entropy log2 D: the ratio is 1, give or take
        // the rounding of two logarithms, and never reported above 1.
        for d in 2..=2000u64 {
            let counts = vec![1u64; d as usize];
            let h = plug_in(&profile(&counts)).unwrap();
            let x = normalised(h, d).unwrap().unwrap();
            assert!(
                (1.0 - 4.0 * f64::EPSILON..=1.0).contains(&x),
                "D = {d}: {x}"
            );
        }
        let x = normalised(1.4224, 10).unwrap().unwrap();
        assert!((x - 1.4224 / 10f64.log2()).abs() < 1e-15);
    }

    #[test]
    fn normalised_rejects_impossible_ratios_and_never_reports_negative_zero() {
        assert!(normalised(f64::NAN, 2).is_err());
        assert!(normalised(-1e-300, 2).is_err());
        assert!(normalised(1.0 + 1e-9, 2).is_err());
        assert!(normalised(f64::INFINITY, 2).is_err());
        let z = normalised(-0.0, 4).unwrap().unwrap();
        assert_eq!(z.to_bits(), 0.0f64.to_bits());
        assert_eq!(normalised(1.0 + 1e-13, 2).unwrap(), Some(1.0));
    }
}
