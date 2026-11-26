//! Axis ticks: "nice numbers" (multiples of 1, 2 and 5 times a power of ten) covering a range,
//! labelled with as many decimals as their step needs, and integer-only ticks for positions; no
//! axis ever repeats a label (G17).

/// The step of nice ticks: 1, 2 or 5 times a power of ten.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The step, as the ticks are computed from it.
    value: f64,
    /// Its power of ten: a step of 0.005 has exponent −3, and a step of 10 × 10^e counts as
    /// 1 × 10^(e + 1).
    exponent: i32,
}

impl Step {
    /// Decimals that tell multiples of the step apart: two for every step of 0.01 or more (as
    /// always), otherwise as many as the step itself has (three for 0.001, 0.002 and 0.005).
    #[must_use]
    pub fn decimals(self) -> usize {
        let needed = -i64::from(self.exponent);
        usize::try_from(needed.max(2)).unwrap_or(2)
    }
}

/// Evenly spaced nice tick values covering `[lo, hi]`, at most about `max_ticks` of them, and
/// their step. No ticks for non-finite input; `[lo]` for an empty range (no step).
#[must_use]
pub fn nice_ticks(lo: f64, hi: f64, max_ticks: usize) -> (Vec<f64>, Option<Step>) {
    if !lo.is_finite() || !hi.is_finite() {
        return (Vec::new(), None);
    }
    if lo >= hi {
        return (vec![lo], None);
    }
    let max_ticks = max_ticks.clamp(2, 50);
    let step = nice_step((hi - lo) / (max_ticks - 1) as f64);
    if !(step.value.is_finite() && step.value > 0.0) {
        return (vec![lo, hi], None);
    }
    let mut ticks = Vec::with_capacity(max_ticks + 2);
    let first = (lo / step.value).ceil();
    // Ticks are integer multiples of the step, computed by multiplication (no accumulated
    // rounding error), and the loop is bounded.
    for i in 0..=max_ticks + 1 {
        let v = (first + i as f64) * step.value;
        if v > hi + step.value * 1e-9 {
            break;
        }
        ticks.push(v);
    }
    (ticks, Some(step))
}

/// The labels of value ticks, shared by the chart and the exported figures: two decimals, or
/// as many as the step needs to tell the ticks apart (see [`Step::decimals`]), so no label
/// repeats. Ticks without a step (fewer than two) get two decimals.
#[must_use]
pub fn y_tick_labels(ticks: &[f64], step: Option<Step>) -> Vec<String> {
    let decimals = step.map_or(2, Step::decimals);
    ticks.iter().map(|v| format!("{v:.decimals$}")).collect()
}

/// The smallest of 1, 2, 5 or 10 times a power of ten that is at least `rough`.
fn nice_step(rough: f64) -> Step {
    // libm, not the platform's maths library: the ticks of exported figures are identical on
    // every system.
    let power = libm::log10(rough).floor();
    let magnitude = libm::pow(10.0, power);
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
    // An integer-valued float (saturating for the non-finite powers of a zero or infinite
    // `rough`, whose steps the callers discard).
    #[allow(clippy::cast_possible_truncation)]
    let exponent = power as i32;
    Step {
        value: nice * magnitude,
        exponent: if nice == 10.0 {
            exponent.saturating_add(1)
        } else {
            exponent
        },
    }
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
    let step = nice_step(((hi - lo) / (max_ticks - 1) as f64).max(1.0))
        .value
        .max(1.0);
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
    use crate::charts::transform::y_axis_max;
    use proptest::prelude::*;

    /// The value labels of an axis for data whose largest value is `max`.
    fn value_labels(max: f64) -> Vec<String> {
        let (ticks, step) = nice_ticks(0.0, y_axis_max(Some(max)), 6);
        y_tick_labels(&ticks, step)
    }

    #[test]
    fn small_axes_get_a_third_decimal_and_a_floor() {
        // Two decimals read 0.00, 0.01, 0.01, 0.01, 0.02 here.
        assert_eq!(
            value_labels(0.02),
            ["0.000", "0.005", "0.010", "0.015", "0.020"]
        );
        assert_eq!(value_labels(0.005), ["0.000", "0.002", "0.004"]);
        // Below the floor the axis is 0.005 bits tall.
        assert_eq!(
            value_labels(1e-6),
            ["0.000", "0.001", "0.002", "0.003", "0.004", "0.005"]
        );
        // Steps of 0.01 and more keep two decimals (the golden figures' step is 1).
        assert_eq!(value_labels(0.04), ["0.00", "0.01", "0.02", "0.03", "0.04"]);
        assert_eq!(value_labels(2.4), ["0.00", "1.00", "2.00"]);
        // All-zero data keeps its 1-bit axis.
        let (ticks, step) = nice_ticks(0.0, y_axis_max(Some(0.0)), 6);
        assert_eq!(
            y_tick_labels(&ticks, step),
            ["0.00", "0.20", "0.40", "0.60", "0.80", "1.00"]
        );
    }

    proptest! {
        /// From subnormal maxima up to 64 bits (the most a position can hold), the value labels
        /// never repeat, increase and fit in five characters, and they are the two-decimal
        /// labels wherever those were already distinct.
        #[test]
        fn value_labels_never_repeat(log2_max in -1074.0f64..=6.0) {
            let max = libm::exp2(log2_max).min(64.0);
            let (ticks, step) = nice_ticks(0.0, y_axis_max(Some(max)), 6);
            let labels = y_tick_labels(&ticks, step);
            prop_assert!(ticks.len() >= 2, "{:?}", ticks);
            let values: Vec<f64> = labels.iter().map(|l| l.parse().unwrap()).collect();
            prop_assert!(values.windows(2).all(|w| w[0] < w[1]), "{:?}", labels);
            prop_assert!(labels.iter().all(|l| l.len() <= 5), "{:?}", labels);
            let two: Vec<String> = ticks.iter().map(|t| format!("{t:.2}")).collect();
            if two.windows(2).all(|w| w[0] != w[1]) {
                prop_assert_eq!(&labels, &two);
            }
        }
    }

    #[test]
    fn nice_ticks_cover_the_range_evenly() {
        let (t, _) = nice_ticks(0.0, 10.0, 6);
        assert_eq!(t.first(), Some(&0.0));
        assert_eq!(t.last(), Some(&10.0));
        for w in t.windows(2) {
            assert!(((w[1] - w[0]) - (t[1] - t[0])).abs() < 1e-12);
        }
        assert!(!nice_ticks(0.0, 0.5, 5).0.is_empty());
        assert!(nice_ticks(0.0, 1.0, 100).0.len() <= 52);
        let (t, _) = nice_ticks(0.0, 1.5878, 6);
        assert!(t.iter().all(|v| (0.0..=1.5878).contains(v)));
    }

    #[test]
    fn nice_ticks_degenerate_input() {
        assert_eq!(nice_ticks(5.0, 5.0, 5), (vec![5.0], None));
        assert!(nice_ticks(f64::NAN, 10.0, 5).0.is_empty());
        assert!(nice_ticks(0.0, f64::INFINITY, 5).0.is_empty());
        let (t, step) = nice_ticks(-f64::MAX, f64::MAX, 5);
        assert_eq!(
            (t.len(), step),
            (2, None),
            "a range too wide for a finite step"
        );
        assert_eq!(
            y_tick_labels(&[1.5], None),
            ["1.50"],
            "two decimals without a step"
        );
    }

    /// The step's exponent is an integer from the same power of ten as its value, with
    /// 10 × 10^e normalised to 1 × 10^(e + 1).
    #[test]
    fn steps_know_their_decimals() {
        let decimals = |rough: f64| nice_step(rough).decimals();
        assert_eq!(nice_step(0.0042).value, 0.005);
        assert_eq!(decimals(0.0042), 3, "0.005");
        assert_eq!(decimals(0.0011), 3, "0.002");
        assert_eq!(decimals(0.0006), 3, "10 × 0.0001 is 0.001");
        assert_eq!(decimals(0.006), 2, "10 × 0.001 is 0.01");
        assert_eq!(decimals(0.03), 2, "0.05");
        assert_eq!(decimals(0.504), 2, "10 × 0.1 is 1");
        assert_eq!(decimals(13.44), 2, "20");
        assert_eq!(decimals(0.00004), 5, "0.00005, below the axis floor");
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
