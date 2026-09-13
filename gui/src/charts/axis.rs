//! Axis tick generation using the "Nice Numbers" algorithm.
//!
//! Based on Heckbert 1990 (Graphics Gems I, Academic Press, pp. 61–63):
//! "Nice Numbers for Graph Labels". Produces human-friendly tick values
//! (multiples of 1, 2, 5) that cover the data range with uniform spacing.
//!
//! Used by the entropy chart for both X (position) and Y (entropy) axes.

/// Generate "nice" evenly-spaced tick values covering `[lo, hi]`.
///
/// Returns at most `max_ticks` values. Handles edge cases:
/// - `lo >= hi` → returns `[lo]` (or empty if NaN/Inf)
/// - NaN / Infinity → returns empty Vec
/// - Negative ranges → works correctly (negative positions unlikely but safe)
///
/// `max_ticks` is capped at 50 to prevent unbounded allocation from
/// adversarial or buggy inputs.
pub fn nice_ticks(lo: f64, hi: f64, max_ticks: usize) -> Vec<f64> {
    // Guard against NaN / Infinity
    if !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }

    let max_ticks = max_ticks.clamp(2, 50);

    if lo >= hi {
        return vec![lo];
    }

    let range = hi - lo;
    // Rough step: divide range by desired number of intervals
    let rough_step = range / (max_ticks - 1) as f64;
    let magnitude = 10.0_f64.powf(rough_step.log10().floor());

    // Pick the "nicest" step that is >= rough_step
    let normalized = rough_step / magnitude;
    let nice_step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    } * magnitude;

    // Guard against zero or negative step (shouldn't happen with valid inputs,
    // but prevents infinite loop from floating-point edge cases)
    if nice_step <= 0.0 || !nice_step.is_finite() {
        return vec![lo, hi];
    }

    let first_tick = (lo / nice_step).floor() * nice_step;
    let mut ticks = Vec::with_capacity(max_ticks + 2);
    let mut v = first_tick;

    // Safety cap: at most max_ticks + 2 iterations to prevent infinite loop
    // from floating-point accumulation edge cases
    let iteration_cap = max_ticks + 2;
    for _ in 0..iteration_cap {
        if v > hi + nice_step * 0.001 {
            break;
        }
        ticks.push(v);
        v += nice_step;
    }

    ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nice_ticks_basic() {
        let ticks = nice_ticks(0.0, 10.0, 6);
        assert!(!ticks.is_empty());
        assert!(*ticks.first().unwrap() <= 0.0);
        assert!(*ticks.last().unwrap() >= 10.0);
        // All intervals should be equal
        for w in ticks.windows(2) {
            let diff = w[1] - w[0];
            assert!((diff - (ticks[1] - ticks[0])).abs() < 1e-10);
        }
    }

    #[test]
    fn test_nice_ticks_small_range() {
        let ticks = nice_ticks(0.0, 0.5, 5);
        assert!(!ticks.is_empty());
    }

    #[test]
    fn test_nice_ticks_degenerate() {
        assert_eq!(nice_ticks(5.0, 5.0, 5), vec![5.0]);
        assert!(nice_ticks(f64::NAN, 10.0, 5).is_empty());
        assert!(nice_ticks(0.0, f64::INFINITY, 5).is_empty());
    }

    #[test]
    fn test_nice_ticks_large_range() {
        let ticks = nice_ticks(1.0, 2204.0, 10);
        assert!(ticks.len() <= 12); // max_ticks + 2
        assert!(*ticks.first().unwrap() <= 1.0);
        // The last tick must be at or near the data max; nice_ticks rounds to
        // human-friendly numbers, so the last tick may be slightly below `hi`
        // (e.g., 2000 for max 2204 with step 500).
        assert!(
            *ticks.last().unwrap() >= 2000.0,
            "last tick {} should be near 2204",
            ticks.last().unwrap()
        );
    }

    #[test]
    fn test_nice_ticks_respects_cap() {
        let ticks = nice_ticks(0.0, 1.0, 100); // capped to 50
        assert!(ticks.len() <= 52);
    }
}
