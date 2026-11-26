//! Human-readable numbers for messages: durations, sizes, counts and values (entropies,
//! coverages, percentages), shared by the command line and the desktop app so both say the same
//! thing (D104).
//!
//! Numbers are plain digits with `.` as the decimal mark and no thousands separators, as the
//! JSON outputs write them: the SI allows the comma as a decimal marker and never inserts commas
//! or dots between digit groups ("Neither dots nor commas are ever inserted in the spaces
//! between groups"), grouping being "a matter of choice" not always followed in "scripts to be
//! read by a computer" (BIPM SI Brochure, 9th ed., §5.4.4), and speech synthesisers read "2,637"
//! as a decimal in most languages. Unit symbols follow the number after a space (§5.4.3): `s`;
//! `min` and `h`, accepted for use with the SI (Table 8); and bytes, `B`, with the SI's decimal
//! prefixes `kB`, `MB`, `GB`. Every value is rounded *before* its unit is chosen, so the boundaries read
//! naturally (59.96 s is `1 min 00 s`, not `60.0 s`; 999 999 bytes is `1.0 MB`, not
//! `1000 kB`).

use std::time::Duration;

/// Separators of human-readable messages, in one place.
pub mod separators {
    /// Between groups of a message (`…in 1.00 s; entropy mean …`).
    pub const GROUP: &str = "; ";
    /// Between items of a list (`opening 0.01 s, counting k-mers 0.20 s`).
    pub const ITEM: &str = ", ";
}

/// `"1 sequence"` / `"3 sequences"`.
#[must_use]
pub fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `0.21 s`, `12.3 s`, `4 min 05 s`, `1 h 02 min` (hours keep counting past a day).
#[must_use]
pub fn duration(d: Duration) -> String {
    let s = d.as_secs_f64();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let round = |x: f64| x.round() as u64;
    let centis = round(s * 100.0);
    if centis < 1_000 {
        return format!("{}.{:02} s", centis / 100, centis % 100);
    }
    let decis = round(s * 10.0);
    if decis < 600 {
        return format!("{}.{} s", decis / 10, decis % 10);
    }
    let seconds = round(s);
    if seconds < 3_600 {
        return format!("{} min {:02} s", seconds / 60, seconds % 60);
    }
    let minutes = round(s / 60.0);
    format!("{} h {:02} min", minutes / 60, minutes % 60)
}

/// `17 B`, `512 kB`, `42.0 MB`, `1.2 GB`: decimal multiples with SI prefix symbols (lower-case
/// k for kilo; BIPM SI Brochure, 9th ed., Table 7), one decimal below 100.
#[must_use]
pub fn bytes(n: u64) -> String {
    const UNITS: [(&str, u64); 4] = [
        ("kB", 1_000),
        ("MB", 1_000_000),
        ("GB", 1_000_000_000),
        ("TB", 1_000_000_000_000),
    ];
    if n < 1_000 {
        return format!("{n} B");
    }
    for (i, (unit, size)) in UNITS.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let v = n as f64 / *size as f64;
        // One decimal below 100 (after rounding), none above.
        let text = if (v * 10.0).round() < 1_000.0 {
            format!("{v:.1}")
        } else {
            format!("{v:.0}")
        };
        let shown: f64 = text.parse().unwrap_or(v);
        if shown < 1_000.0 || i == UNITS.len() - 1 {
            return format!("{text} {unit}");
        }
    }
    unreachable!("the last unit always returns")
}

/// `x` with `decimals` decimal places, as values are shown to people (summaries, tables,
/// tooltips): an exact zero (or −0) shows as zeros; a value that is not zero but too small to
/// show as such reads `< 0.0001` (the smallest step at that precision), never a misleading
/// zero; everything else is rounded normally. Exports and sorting keep full precision.
#[must_use]
pub fn fixed(x: f64, decimals: usize) -> String {
    if x == 0.0 {
        return format!("{:.decimals$}", 0.0);
    }
    let shown = format!("{x:.decimals$}");
    if shown
        .trim_start_matches('-')
        .bytes()
        .all(|b| b == b'0' || b == b'.')
    {
        let step = format!("{:.decimals$}", 10f64.powi(-(decimals as i32)));
        return if x > 0.0 {
            format!("< {step}")
        } else {
            format!("> -{step}")
        };
    }
    shown
}

/// [`fixed`] for a value bounded above by `bound` (a coverage of 1, an incidence of 100):
/// a value below the bound that would round up to it reads `> 0.9999`, so it is never shown
/// as reaching the bound.
#[must_use]
pub fn fixed_bounded(x: f64, decimals: usize, bound: f64) -> String {
    let shown = fixed(x, decimals);
    if x < bound && shown == format!("{bound:.decimals$}") {
        let step = 10f64.powi(-(decimals as i32));
        return format!("> {:.decimals$}", bound - step);
    }
    shown
}

/// A percentage (0–100) with `decimals` decimal places and `%`: `12.3%`, `< 0.1%` (see
/// [`fixed`]).
#[must_use]
pub fn percent(x: f64, decimals: usize) -> String {
    format!("{}%", fixed(x, decimals))
}

/// [`percent`] bounded by 100%: `> 99.9%` rather than `100.0%` for a value below 100.
#[must_use]
pub fn percent_bounded(x: f64, decimals: usize) -> String {
    format!("{}%", fixed_bounded(x, decimals, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_round_before_choosing_the_unit() {
        let ms = Duration::from_millis;
        assert_eq!(duration(Duration::ZERO), "0.00 s");
        assert_eq!(duration(ms(210)), "0.21 s");
        assert_eq!(duration(ms(9_994)), "9.99 s");
        assert_eq!(duration(ms(9_996)), "10.0 s");
        assert_eq!(duration(ms(12_340)), "12.3 s");
        assert_eq!(duration(ms(59_940)), "59.9 s");
        assert_eq!(duration(ms(59_960)), "1 min 00 s");
        assert_eq!(duration(Duration::from_secs(245)), "4 min 05 s");
        assert_eq!(duration(ms(3_599_600)), "1 h 00 min");
        assert_eq!(duration(Duration::from_secs(3_720)), "1 h 02 min");
        assert_eq!(duration(Duration::from_secs(93_900)), "26 h 05 min");
    }

    #[test]
    fn sizes_round_before_choosing_the_unit() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1_000), "1.0 kB");
        assert_eq!(bytes(99_949), "99.9 kB");
        assert_eq!(bytes(99_960), "100 kB");
        assert_eq!(bytes(512_000), "512 kB");
        assert_eq!(bytes(999_499), "999 kB");
        assert_eq!(bytes(999_999), "1.0 MB");
        assert_eq!(bytes(42_000_000), "42.0 MB");
        assert_eq!(bytes(1_234_000_000), "1.2 GB");
        assert_eq!(bytes(5_000_000_000_000_000), "5000 TB");
    }

    #[test]
    fn plurals() {
        assert_eq!(plural(1, "warning", "warnings"), "1 warning");
        assert_eq!(plural(0, "warning", "warnings"), "0 warnings");
        assert_eq!(plural(2637, "sequence", "sequences"), "2637 sequences");
    }

    #[test]
    fn fixed_never_shows_a_non_zero_value_as_zero() {
        assert_eq!(fixed(0.0, 4), "0.0000");
        assert_eq!(fixed(-0.0, 4), "0.0000");
        for tiny in [3.288e-5, 4.9e-5, 5e-324] {
            assert_eq!(fixed(tiny, 4), "< 0.0001", "{tiny}");
        }
        assert_eq!(fixed(5e-5, 4), "0.0001");
        assert_eq!(fixed(1.805e-4, 4), "0.0002");
        assert_eq!(fixed(0.0004, 3), "< 0.001");
        assert_eq!(fixed(-1e-9, 4), "> -0.0001");
        assert_eq!(fixed(1.4224, 4), "1.4224");
        assert_eq!(fixed(0.0362601743099057, 4), "0.0363");
        assert_eq!(percent(0.04, 1), "< 0.1%");
        assert_eq!(percent(12.345, 1), "12.3%");
        assert_eq!(percent(0.0, 1), "0.0%");
    }

    #[test]
    fn bounded_values_never_show_as_reaching_their_bound() {
        assert_eq!(fixed_bounded(0.99996, 4, 1.0), "> 0.9999");
        assert_eq!(fixed_bounded(1.0, 4, 1.0), "1.0000");
        assert_eq!(fixed_bounded(0.9884, 4, 1.0), "0.9884");
        assert_eq!(percent_bounded(99.97, 1), "> 99.9%");
        assert_eq!(percent_bounded(100.0, 1), "100.0%");
        assert_eq!(percent_bounded(0.01, 1), "< 0.1%");
    }

    proptest::proptest! {
        /// Normal rounding everywhere except at the two ends of the shown range.
        #[test]
        fn fixed_is_normal_rounding_inside_the_range(x in 0.0f64..1e6, decimals in 0usize..7) {
            let normal = format!("{x:.decimals$}");
            let shown = fixed(x, decimals);
            let zero = normal.bytes().all(|b| b == b'0' || b == b'.');
            if x == 0.0 || !zero {
                proptest::prop_assert_eq!(shown, normal);
            } else {
                proptest::prop_assert!(shown.starts_with("< "));
            }
        }
    }
}
