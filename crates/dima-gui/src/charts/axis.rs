//! Axis ticks: "nice numbers" (multiples of 1, 2 and 5 times a power of ten) covering a range,
//! and integer-only ticks for positions, which never repeat a label (G17).

/// Evenly spaced nice tick values covering `[lo, hi]`, at most about `max_ticks` of them.
/// Empty for non-finite input; `[lo]` for an empty range.
#[must_use]
pub fn nice_ticks(lo: f64, hi: f64, max_ticks: usize) -> Vec<f64> {
    if !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }
    if lo >= hi {
        return vec![lo];
    }
    let max_ticks = max_ticks.clamp(2, 50);
    let step = nice_step((hi - lo) / (max_ticks - 1) as f64);
    if !(step.is_finite() && step > 0.0) {
        return vec![lo, hi];
    }
    let mut ticks = Vec::with_capacity(max_ticks + 2);
    let first = (lo / step).ceil();
    // Ticks are integer multiples of the step, computed by multiplication (no accumulated
    // rounding error), and the loop is bounded.
    for i in 0..=max_ticks + 1 {
        let v = (first + i as f64) * step;
        if v > hi + step * 1e-9 {
            break;
        }
        ticks.push(v);
    }
    ticks
}

/// The smallest of 1, 2, 5 or 10 times a power of ten that is at least `rough`.
fn nice_step(rough: f64) -> f64 {
    // libm, not the platform's maths library: the ticks of exported figures are identical on
    // every system.
    let magnitude = libm::pow(10.0, libm::log10(rough).floor());
    let normalized = rough / magnitude;
    let nice = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

/// Integer ticks inside `[lo, hi]` (positions): the step is a nice number of at least 1, so a
/// zoomed-in axis never shows the same label twice.
#[must_use]
pub fn integer_ticks(lo: f64, hi: f64, max_ticks: usize) -> Vec<u64> {
    if !lo.is_finite() || !hi.is_finite() || hi < 1.0 {
        return Vec::new();
    }
    let lo = lo.max(1.0).ceil();
    let hi = hi.floor();
    if lo > hi {
        return Vec::new();
    }
    let max_ticks = max_ticks.clamp(2, 50);
    let step = nice_step(((hi - lo) / (max_ticks - 1) as f64).max(1.0)).max(1.0);
    // Positions are below 2^53, so these conversions are exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (step, lo, hi) = (step.round() as u64, lo as u64, hi as u64);
    let first = lo.div_ceil(step) * step;
    (0..=max_ticks as u64 + 1)
        .filter_map(|i| first.checked_add(i.checked_mul(step)?))
        .take_while(|&v| v <= hi)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks_cover_the_range_evenly() {
        let t = nice_ticks(0.0, 10.0, 6);
        assert_eq!(t.first(), Some(&0.0));
        assert_eq!(t.last(), Some(&10.0));
        for w in t.windows(2) {
            assert!(((w[1] - w[0]) - (t[1] - t[0])).abs() < 1e-12);
        }
        assert!(!nice_ticks(0.0, 0.5, 5).is_empty());
        assert!(nice_ticks(0.0, 1.0, 100).len() <= 52);
        let t = nice_ticks(0.0, 1.5878, 6);
        assert!(t.iter().all(|v| (0.0..=1.5878).contains(v)));
    }

    #[test]
    fn nice_ticks_degenerate_input() {
        assert_eq!(nice_ticks(5.0, 5.0, 5), vec![5.0]);
        assert!(nice_ticks(f64::NAN, 10.0, 5).is_empty());
        assert!(nice_ticks(0.0, f64::INFINITY, 5).is_empty());
    }

    #[test]
    fn integer_ticks_never_repeat_when_zoomed_in() {
        // A 5-position view used to print "12 12 13 13 14" with fractional ticks.
        let t = integer_ticks(11.6, 16.4, 8);
        assert_eq!(t, vec![12, 13, 14, 15, 16]);
        let mut d = t.clone();
        d.dedup();
        assert_eq!(d, t);
        let t = integer_ticks(0.5, 1345.5, 8);
        assert!(t.len() >= 3 && t.len() <= 10, "{t:?}");
        assert!(t.windows(2).all(|w| w[1] > w[0]));
        assert_eq!(integer_ticks(0.5, 1.5, 8), vec![1]);
        assert!(integer_ticks(f64::NAN, 3.0, 8).is_empty());
        assert!(integer_ticks(2.2, 2.8, 8).is_empty());
        assert!(integer_ticks(-5.0, 0.5, 8).is_empty());
    }
}
