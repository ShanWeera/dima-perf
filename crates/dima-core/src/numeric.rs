//! Small numerical building blocks shared by the estimators and the HCS confidence bounds.
//!
//! Everything here is deterministic: fixed evaluation order, logarithms from `libm` (the same
//! bits on every platform), no randomness.

/// Neumaier's compensated summation (an improved Kahan sum): the error does not grow with the
/// number of terms.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Neumaier {
    sum: f64,
    compensation: f64,
}

impl Neumaier {
    pub(crate) const fn new() -> Self {
        Self {
            sum: 0.0,
            compensation: 0.0,
        }
    }

    pub(crate) fn add(&mut self, x: f64) {
        let t = self.sum + x;
        if self.sum.abs() >= x.abs() {
            self.compensation += (self.sum - t) + x;
        } else {
            self.compensation += (x - t) + self.sum;
        }
        self.sum = t;
    }

    /// The compensated total (never `-0.0`).
    pub(crate) fn total(self) -> f64 {
        let s = self.sum + self.compensation;
        if s == 0.0 { 0.0 } else { s }
    }
}

/// Weights below this fraction of the running total end a binomial walk.
const WALK_CUTOFF: f64 = 1e-18;

/// Visits the Binomial(`n`, `p`) probability mass function outward from its mode.
///
/// `visit(x, w)` receives unnormalised weights with `w(mode) = 1`; successive weights follow
/// from the ratio `w(x+1)/w(x) = (n−x)/(x+1) · p/(1−p)`, so no factorials, powers or `lgamma`
/// are needed and nothing overflows. Because the pmf is log-concave, each direction stops once a
/// weight falls below 10⁻¹⁸ of the running total. Returns the total weight (the normaliser).
/// The cost is O(√(n·p·(1−p))) per call.
///
/// Requires `n ≥ 1` and `0 < p < 1`.
pub(crate) fn binomial_walk(n: u64, p: f64, mut visit: impl FnMut(u64, f64)) -> f64 {
    debug_assert!(n >= 1 && p > 0.0 && p < 1.0, "n={n} p={p}");
    let r = p / (1.0 - p);
    // floor((n+1)p), the mode; any start point would do, the mode makes the walk shortest.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mode = ((((n + 1) as f64) * p) as u64).min(n);
    let mut total = 1.0;
    visit(mode, 1.0);
    let (mut w, mut x) = (1.0_f64, mode);
    while x < n {
        w *= (n - x) as f64 / (x + 1) as f64 * r;
        x += 1;
        total += w;
        visit(x, w);
        if w < WALK_CUTOFF * total {
            break;
        }
    }
    let (mut w, mut x) = (1.0_f64, mode);
    while x > 0 {
        w *= x as f64 / ((n - x + 1) as f64 * r);
        x -= 1;
        total += w;
        visit(x, w);
        if w < WALK_CUTOFF * total {
            break;
        }
    }
    total
}

/// P(X ≥ x) for X ~ Binomial(n, p), `n ≥ 1`, `0 < p < 1` (absolute error below 10⁻¹⁷).
pub(crate) fn binomial_upper_tail(n: u64, p: f64, x: u64) -> f64 {
    let mut tail = 0.0;
    let total = binomial_walk(n, p, |k, w| {
        if k >= x {
            tail += w;
        }
    });
    (tail / total).min(1.0)
}

/// Exact two-sided Clopper–Pearson lower confidence bound for a binomial proportion with `x`
/// successes in `n` trials at level `1 − 2·alpha_half` (Clopper & Pearson 1934): the `p` with
/// P(X ≥ x | p) = `alpha_half`. Uses `alpha_half^(1/n)` when `x = n`, and 0 when `x = 0`.
pub(crate) fn clopper_pearson_lower(x: u64, n: u64, alpha_half: f64) -> f64 {
    debug_assert!(x <= n && n >= 1);
    if x == 0 {
        return 0.0;
    }
    if x == n {
        return libm::pow(alpha_half, 1.0 / n as f64);
    }
    // P(X ≥ x | p) increases with p; bisect on (0, x/n), where the root lies.
    let (mut lo, mut hi) = (0.0_f64, x as f64 / n as f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if binomial_upper_tail(n, mid, x) < alpha_half {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Intercept of the ordinary-least-squares line through `(xs, ys)` (two-pass, centred).
/// `None` when every `x` is equal (no line is defined).
pub(crate) fn ols_intercept(xs: &[f64], ys: &[f64]) -> Option<f64> {
    debug_assert_eq!(xs.len(), ys.len());
    let n = xs.len() as f64;
    let mean = |v: &[f64]| {
        let mut s = Neumaier::new();
        v.iter().for_each(|&x| s.add(x));
        s.total() / n
    };
    let (mx, my) = (mean(xs), mean(ys));
    let (mut sxx, mut sxy) = (Neumaier::new(), Neumaier::new());
    for (&x, &y) in xs.iter().zip(ys) {
        let dx = x - mx;
        sxx.add(dx * dx);
        sxy.add(dx * (y - my));
    }
    let sxx = sxx.total();
    if sxx == 0.0 {
        return None;
    }
    Some(my - sxy.total() / sxx * mx)
}

/// ψ(n) − ψ(c) = Σ_{k=c}^{n−1} 1/k for integers 1 ≤ c ≤ n, accurate to a few ulps for any
/// size without an O(n) loop.
///
/// Small arguments are summed exactly; large ones use ψ(x) = ln x − 1/(2x) − [`digamma_tail`]
/// (remainder below 10⁻²³ for x ≥ 64), with ln(n/c) taken as `ln1p((n−c)/c)` so that close
/// arguments do not cancel.
pub(crate) fn digamma_difference(n: u64, c: u64) -> f64 {
    const EXACT: u64 = DIGAMMA_ASYMPTOTIC_MIN;
    debug_assert!(1 <= c && c <= n);
    let exact = |from: u64, to: u64| {
        let mut s = Neumaier::new();
        for k in from..to {
            s.add(1.0 / k as f64);
        }
        s.total()
    };
    let asymptotic = |n: u64, c: u64| {
        let (nf, cf) = (n as f64, c as f64);
        libm::log1p((n - c) as f64 / cf) - 0.5 / nf + 0.5 / cf - digamma_tail(nf) + digamma_tail(cf)
    };
    if n <= EXACT {
        exact(c, n)
    } else if c < EXACT {
        asymptotic(n, EXACT) + exact(c, EXACT)
    } else {
        asymptotic(n, c)
    }
}

/// Smallest argument for which [`digamma_tail`] is accurate to the last bit.
const DIGAMMA_ASYMPTOTIC_MIN: u64 = 64;

/// ln x − 1/(2x) − ψ(x) ∼ Σ_{k≥1} B_{2k}/(2k·x^{2k}), the derivative of Stirling's series for
/// ln Γ, truncated after the x⁻¹⁰ term: 1/(12x²) − 1/(120x⁴) + 1/(252x⁶) − 1/(240x⁸) +
/// 1/(132x¹⁰). Alzer (1997, Theorem 8) proves that ln Γ minus its Stirling series truncated
/// after 2n terms, and the negative of the remainder after 2n + 1 terms, are completely monotonic;
/// their derivatives are therefore negative, which puts the left side strictly between
/// consecutive truncations of this series. The remainder is thus smaller than the first omitted
/// term, 691/(32760·x¹²), so below 10⁻²³ for x ≥ 64.
fn digamma_tail(x: f64) -> f64 {
    let x2 = 1.0 / (x * x);
    x2 * (1.0 / 12.0 - x2 * (1.0 / 120.0 - x2 * (1.0 / 252.0 - x2 * (1.0 / 240.0 - x2 / 132.0))))
}

/// Euler's constant γ = −ψ(1), rounded to the nearest `f64`.
const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

/// Truncation error allowed in [`lerch_phi1`], relative to the value: 2⁻⁵⁴ (half the unit
/// roundoff), so the truncation never moves the result by more than half an ulp.
const LERCH_TOLERANCE: f64 = 0.25 * f64::EPSILON;

/// Hard cap on the terms or steps of [`lerch_phi1`]. Measured maxima: 17 series terms;
/// 817 continued-fraction steps for n < 64 (every valid count profile), 373 for n ≥ 64.
const LERCH_MAX_STEPS: u32 = 2048;

/// Φ(1−a, 1, n) = Σ_{j≥0} (1−a)^j/(n+j), the Lerch transcendent, for n ≥ 2 and 0 < a < 1, in a
/// time independent of n and 1/a. Every term added is non-negative.
///
/// Φ = ₂F₁(1, n; n+1; 1−a)/n, because (1)_j(n)_j/((n+1)_j j!) = n/(n+j) in the hypergeometric
/// series (Hankin 2015, eq. 15.1.1). With x = n·a:
///
/// * **Series** (n ≥ 64 and b₀ ≥ 0, i.e. x ≲ e^{−γ} ≈ 0.56): the expansion of ₂F₁(a, b; a+b; z)
///   about z = 1 (Hankin 2015, eq. 15.3.10) with a = 1, b = n gives
///   Φ = Σ_{k≥0} t_k b_k with t_k = (n)_k a^k/k! and b_k = ψ(k+1) − ψ(n+k) − ln a, where
///   b₀ = −γ − ln x + (ln n − ψ(n)) and b_{k+1} = b_k + 1/(k+1) − 1/(n+k) ≥ b_k. The ratio
///   r_k = t_{k+1}/t_k = (x + k·a)/(k+1) ≤ x < 1 does not increase, so the remainder after term k
///   is at most t_k·r_k/(1−r_k)·(b_k + 1/((k+1)(1−r_k))).
/// * **Continued fraction** (otherwise): Pfaff's transformation (Hankin 2015, eq. 15.3.4) turns
///   Φ into ₂F₁(1, 1; n+1; −(1−a)/a)/x. Gauss's continued fraction for
///   F(α, β+1; γ+1; z)/F(α, β; γ; z) with α = 1, β = 0, γ = n (whose denominator is 1) converges
///   to that ratio off the cut [1, ∞) (Van Vleck 1901, p. 1 and §9(a)) and gives
///   Φ = 1/(x·(1 + α₁/(1 + α₂/(1 + …)))) with
///   α_{2i+1} = (i+1)(n+i)(1−a)/((n+2i)(x+(2i+1)a)) and α_{2i} = i(n+i−1)(1−a)/((n+2i−1)(x+2ia)),
///   all positive: a Stieltjes fraction at a positive argument. Its approximants are the partial
///   sums of an alternating series with terms ∏α/(B_{m−1}B_m) (Jones & Thron 1974, p. 796), and
///   the value lies between any two consecutive approximants (Henrici & Pflüger 1966, §2). A
///   forward pass tracks that gap; the approximant is then evaluated by backward recurrence,
///   whose rounding error stays bounded for Stieltjes fractions (Jones & Thron 1974,
///   Theorem 5.1(i) with eq. 3.11).
///
/// Both stop when the remaining error bound is below [`LERCH_TOLERANCE`] of a lower bound of
/// the value. Returns NaN if [`LERCH_MAX_STEPS`] is reached (never observed), which the
/// caller's postcondition reports as an internal error.
pub(crate) fn lerch_phi1(n: u64, a: f64) -> f64 {
    debug_assert!(n >= 2 && a > 0.0 && a < 1.0, "n={n} a={a}");
    let nf = n as f64;
    let x = nf * a;
    if n >= DIGAMMA_ASYMPTOTIC_MIN {
        // ln n − ψ(n) = 1/(2n) + digamma_tail(n), without cancellation for n ≥ 64.
        let b0 = -EULER_GAMMA - libm::log(x) + 0.5 / nf + digamma_tail(nf);
        if b0 >= 0.0 {
            return lerch_series(nf, a, x, b0);
        }
    }
    lerch_continued_fraction(nf, a, x)
}

/// The series of [`lerch_phi1`]; requires b₀ ≥ 0 and a ≤ x < 1.
fn lerch_series(nf: f64, a: f64, x: f64, b0: f64) -> f64 {
    debug_assert!(b0 >= 0.0 && a <= x && x < 1.0, "a={a} x={x} b0={b0}");
    let mut sum = Neumaier::new();
    let (mut t, mut b) = (1.0_f64, b0);
    for k in 0..LERCH_MAX_STEPS {
        let k = f64::from(k);
        sum.add(t * b);
        let r = (x + k * a) / (k + 1.0);
        let remainder = t * r / (1.0 - r) * (b + 1.0 / ((k + 1.0) * (1.0 - r)));
        if remainder <= LERCH_TOLERANCE * sum.total() {
            return sum.total();
        }
        b += 1.0 / (k + 1.0) - 1.0 / (nf + k);
        t *= r;
    }
    f64::NAN
}

/// The continued fraction of [`lerch_phi1`].
fn lerch_continued_fraction(nf: f64, a: f64, x: f64) -> f64 {
    let q = 1.0 - a;
    let alpha = |m: u32| {
        let i = f64::from(m / 2);
        if m % 2 == 1 {
            (i + 1.0) * q * (nf + i) / ((nf + 2.0 * i) * (x + (2.0 * i + 1.0) * a))
        } else {
            i * q * (nf + i - 1.0) / ((nf + 2.0 * i - 1.0) * (x + 2.0 * i * a))
        }
    };
    // r = B_{m−1}/B_m and gap = ∏α/(B_{m−1}B_m) for the denominators B of 1/(1 + α₁/(1 + …)),
    // B_m = B_{m−1} + α_m·B_{m−2} (Jones & Thron 1974, eq. 1.2); the second approximant
    // 1/(1 + α₁) is a lower bound of the value.
    let lower = 1.0 / (1.0 + alpha(1));
    let (mut r, mut gap, mut m) = (1.0_f64, 1.0_f64, 0);
    while gap > LERCH_TOLERANCE * lower {
        if m == LERCH_MAX_STEPS {
            return f64::NAN;
        }
        m += 1;
        let c = alpha(m);
        let r_next = 1.0 / (1.0 + c * r);
        gap *= c * r * r_next;
        r = r_next;
    }
    let mut v = 1.0_f64;
    for j in (1..=m).rev() {
        v = 1.0 + alpha(j) / v;
    }
    1.0 / (v * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerch_phi1_matches_high_precision_values() {
        // Φ(1−A, 1, n) at the exact rational A = num/den, rounded to the nearest f64: mpmath 1.2
        // `lerchphi` at 50 digits, agreeing to at least 28 digits with
        // `hyp2f1(1, 1; n+1; −(1−A)/A)/(nA)` (with direct summation for x ≈ 9·10⁴).
        const U: u64 = u64::MAX;
        for &(n, num, den, want) in &[
            (2u64, 2u128, 3u128, 0.649_185_972_973_479_5), // N = 2, all singletons
            (3, 1, 3, 0.707_816_474_254_870_2),
            (3, 1, 2, 0.545_177_444_479_562_5),
            (63, 1, 1923, 2.977_217_915_402_172_3), // most fraction steps (817)
            (64, 2, 3971, 2.991_765_677_396_372_3), // smallest n of the series
            (
                1_000_000_000_000,
                401,
                714_285_714_285_401,
                0.861_827_063_695_258,
            ), // x = 0.5614
            (
                1_000_000_000_000,
                1,
                1_780_626_780_626,
                0.861_643_221_938_500_4,
            ), // x = 0.5616
            (17_500_000, 89, 175_000_079, 0.101_897_509_632_219_87), // x = 8.9
            (1_000_000, 100_000, 1_099_999, 1.099_988_000_251_992_1e-5), // x ≈ 9·10⁴
            (
                9_007_199_254_740_993,
                1,
                13_510_798_882_111_489,
                0.775_994_758_065_300_8,
            ),
            (
                U,
                1,
                9_223_362_813_482_738_952_224_194,
                12.545_173_802_850_902,
            ), // x = 2·10⁻⁶
            (
                U,
                281,
                9_223_372_036_854_775_807_281,
                0.861_275_808_550_380_4,
            ),
            (U, 1, 27_670_116_110_564_327_422, 0.775_994_758_065_300_8),
        ] {
            let got = lerch_phi1(n, num as f64 / den as f64);
            assert!(
                (got - want).abs() <= 2.0 * f64::EPSILON * want,
                "n={n} A={num}/{den}: {got} vs {want}"
            );
        }
    }

    #[test]
    fn lerch_phi1_matches_direct_summation() {
        // Every count profile with n ≤ 24 against Σ (1−a)^j/(n+j) summed term by term, with
        // (1−a)^j = exp(j·ln(1−a)) so that rounding does not compound.
        for n in 2u64..=24 {
            for f1 in 1..=n {
                for f2 in 0..=(n - f1) / 2 {
                    let (num, den) = if f2 > 0 {
                        (2 * f2, (n - 1) * f1 + 2 * f2)
                    } else {
                        (2, (n - 1) * (f1.max(2) - 1) + 2)
                    };
                    let a = num as f64 / den as f64;
                    let ln_q = libm::log1p(-a);
                    let mut direct = Neumaier::new();
                    for j in 0u32.. {
                        let term = libm::exp(f64::from(j) * ln_q) / (n as f64 + f64::from(j));
                        direct.add(term);
                        if term < 1e-18 * direct.total() {
                            break;
                        }
                    }
                    let got = lerch_phi1(n, a);
                    let want = direct.total();
                    assert!(
                        (got - want).abs() <= 1e-14 * want,
                        "n={n} a={a}: {got} vs {want}"
                    );
                }
            }
        }
    }

    #[test]
    fn lerch_series_and_continued_fraction_agree() {
        // Both representations are exact; where b₀ ≥ 0 both apply.
        for n in [64u64, 1000, 1 << 40, u64::MAX] {
            let nf = n as f64;
            for x in [0.3, 0.5, 0.561] {
                let a = x / nf;
                let b0 = -EULER_GAMMA - libm::log(x) + 0.5 / nf + digamma_tail(nf);
                let s = lerch_series(nf, a, x, b0);
                let c = lerch_continued_fraction(nf, a, x);
                assert!(
                    (s - c).abs() <= 4.0 * f64::EPSILON * s,
                    "n={n} x={x}: {s} vs {c}"
                );
            }
        }
    }

    #[test]
    fn lerch_step_cap_yields_nan() {
        // x = 10⁻³ at n = 10⁶ (served by the series in practice) needs about 10⁵ fraction steps.
        assert!(lerch_continued_fraction(1e6, 1e-9, 1e-3).is_nan());
    }

    #[test]
    fn neumaier_beats_naive() {
        let mut s = Neumaier::new();
        for x in [1.0, 1e100, 1.0, -1e100] {
            s.add(x);
        }
        assert_eq!(s.total(), 2.0);
        let mut z = Neumaier::new();
        z.add(-0.0);
        assert!(z.total().is_sign_positive());
    }

    fn ln_choose(n: u64, k: u64) -> f64 {
        libm::lgamma(n as f64 + 1.0)
            - libm::lgamma(k as f64 + 1.0)
            - libm::lgamma((n - k) as f64 + 1.0)
    }

    #[test]
    fn binomial_walk_matches_direct_pmf() {
        for &(n, p) in &[
            (1u64, 0.5),
            (10, 0.3),
            (37, 0.01),
            (500, 0.97),
            (2637, 0.123),
            (100_000, 0.5),
        ] {
            let mut pmf = vec![0.0; n as usize + 1];
            let total = binomial_walk(n, p, |x, w| pmf[x as usize] = w);
            for (k, w) in pmf.iter().enumerate() {
                let direct = (ln_choose(n, k as u64)
                    + k as f64 * p.ln()
                    + (n - k as u64) as f64 * (1.0 - p).ln())
                .exp();
                let got = w / total;
                assert!(
                    (got - direct).abs() < 1e-12 + 1e-9 * direct,
                    "n={n} p={p} k={k}: {got} vs {direct}"
                );
            }
        }
    }

    #[test]
    fn upper_tail_values() {
        // Exact: P(Bin(3, 0.5) ≥ 2) = 0.5; P(Bin(10, 0.2) ≥ 5) = 0.0327934976.
        assert!((binomial_upper_tail(3, 0.5, 2) - 0.5).abs() < 1e-15);
        assert!((binomial_upper_tail(10, 0.2, 5) - 0.032_793_497_6).abs() < 1e-12);
        assert!((binomial_upper_tail(10, 0.2, 0) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn clopper_pearson_known_values() {
        // x = n: 0.025^(1/n). Values used in the plan's evidence: n = 2, 3, 10, 100.
        for (n, expect) in [
            (2u64, 0.158_113_883_008_418_97),
            (3, 0.292_401_773_821_286_6),
            (10, 0.691_502_892_181_239_2),
            (100, 0.963_783_307_354_823_5),
        ] {
            assert!(
                (clopper_pearson_lower(n, n, 0.025) - expect).abs() < 1e-15,
                "n={n}"
            );
        }
        // Standard table value: x = 5, n = 10, 95% two-sided lower bound 0.187086.
        assert!((clopper_pearson_lower(5, 10, 0.025) - 0.187_086_028_447_398_52).abs() < 1e-12);
        assert_eq!(clopper_pearson_lower(0, 10, 0.025), 0.0);
        // The defining equation holds at the solution.
        let p = clopper_pearson_lower(2112, 2637, 0.025);
        assert!((binomial_upper_tail(2637, p, 2112) - 0.025).abs() < 1e-12);
    }

    #[test]
    fn ols_intercept_cases() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        let ys = [3.0, 5.0, 7.0, 9.0];
        assert!((ols_intercept(&xs, &ys).unwrap() - 1.0).abs() < 1e-15);
        assert_eq!(ols_intercept(&[2.0, 2.0], &[1.0, 3.0]), None);
    }

    #[test]
    fn digamma_difference_matches_harmonic_sums() {
        for &(n, c) in &[
            (1u64, 1u64),
            (2, 1),
            (64, 1),
            (65, 1),
            (65, 64),
            (1000, 1),
            (1000, 63),
            (1000, 64),
            (1000, 999),
            (123_457, 99_999),
            (5_000_000, 3),
        ] {
            let mut s = 0.0_f64;
            let mut comp = 0.0_f64;
            for k in c..n {
                // Kahan summation from the smallest terms for a reference value.
                let y = 1.0 / (n - 1 - (k - c)) as f64 - comp;
                let t = s + y;
                comp = (t - s) - y;
                s = t;
            }
            let got = digamma_difference(n, c);
            assert!(
                (got - s).abs() <= 1e-14 * s.max(1.0),
                "n={n} c={c}: {got} vs {s}"
            );
        }
    }
}
