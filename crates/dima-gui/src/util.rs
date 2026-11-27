//! Small display helpers shared by the panels. Pure functions, unit-tested without egui.
//! Durations, sizes, counts and values in messages come from `dima_core::human`, shared with
//! the CLI (D104): a value that is not zero never reads as zero (`< 0.0001`), and a coverage or
//! incidence below its maximum never reads as the maximum (`> 0.9999`, `> 99.9%`). Numbers in
//! interface text are plain digits without thousands separators.

use std::path::{Path, PathBuf};

use dima_core::human;

/// `s` cut to at most `max_chars` characters (never bytes) with `...` appended when cut.
///
/// Counting characters keeps the cut on a UTF-8 boundary (byte slicing panics on multi-byte
/// characters, which header values and file names may contain).
#[must_use]
pub fn truncate_display(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((boundary, _)) => {
            let mut out = String::with_capacity(boundary + 3);
            out.push_str(&s[..boundary]);
            out.push_str("...");
            out
        }
        None => s.to_owned(),
    }
}

/// How long ago `then` was, relative to `now` (both Unix seconds): `just now`, `5 min ago`,
/// `yesterday`, `3 weeks ago`. Empty when unknown or in the future.
#[must_use]
pub fn format_relative_time(now_secs: u64, then_secs: u64) -> String {
    if then_secs == 0 || now_secs < then_secs {
        return String::new();
    }
    let elapsed = now_secs - then_secs;
    let unit =
        |n: u64, one: &str, many: &str| format!("{n} {} ago", if n == 1 { one } else { many });
    match elapsed {
        0..=59 => "just now".to_owned(),
        60..=3599 => unit(elapsed / 60, "min", "min"),
        3600..=86_399 => unit(elapsed / 3600, "hour", "hours"),
        86_400..=172_799 => "yesterday".to_owned(),
        172_800..=2_419_199 if elapsed < 604_800 => unit(elapsed / 86_400, "day", "days"),
        172_800..=2_419_199 => unit(elapsed / 604_800, "week", "weeks"),
        _ => unit(elapsed / 2_592_000, "month", "months"),
    }
}

/// A value for a single-quoted POSIX shell word: `it's` → `'it'\''s'`.
#[must_use]
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The path in text pasted into the path field, in the forms file managers copy: surrounding
/// whitespace and one pair of quotes removed (Windows "Copy as path"), a `file://` URI decoded
/// (Linux and macOS file managers), and a leading `~` expanded to `home`. `None` when empty.
#[must_use]
pub fn pasted_path(text: &str, home: Option<&Path>) -> Option<PathBuf> {
    let mut s = text.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            s = s[1..s.len() - 1].trim();
            break;
        }
    }
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix("file://") {
        // `file:///p`, `file://localhost/p`; other hosts (network shares) are left to the OS.
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if let Some(decoded) = percent_decode(rest) {
            // `file:///C:/x` on Windows.
            let windows_drive = cfg!(windows)
                && decoded.len() > 2
                && decoded.starts_with('/')
                && decoded.as_bytes()[2] == b':';
            return Some(PathBuf::from(if windows_drive {
                &decoded[1..]
            } else {
                &decoded
            }));
        }
    }
    if let Some(home) = home {
        if s == "~" {
            return Some(home.to_path_buf());
        }
        if let Some(rest) = s
            .strip_prefix("~/")
            .or_else(|| cfg!(windows).then(|| s.strip_prefix("~\\")).flatten())
        {
            return Some(home.join(rest));
        }
    }
    Some(PathBuf::from(s))
}

/// `%XX` escapes decoded; `None` when an escape is malformed or the result is not UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A command line for a POSIX shell: words made only of characters that no shell treats
/// specially stay as they are, every other word is single-quoted (`|` in a header format,
/// spaces in a file name, an empty value).
#[must_use]
pub fn shell_words(args: &[String]) -> String {
    let plain = |a: &str| {
        !a.is_empty()
            && a.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&b))
    };
    args.iter()
        .map(|a| if plain(a) { a.clone() } else { shell_quote(a) })
        .collect::<Vec<_>>()
        .join(" ")
}

/// An optional value shown with a fixed number of decimals (`1.4224`, `< 0.0001`; see
/// [`dima_core::human::fixed`]), or `—` when undefined (D42).
#[must_use]
pub fn opt_f64(v: Option<f64>, decimals: usize) -> String {
    v.map_or_else(|| "\u{2014}".to_owned(), |x| human::fixed(x, decimals))
}

/// An optional sample coverage (at most 1): as [`opt_f64`], and `> 0.9999` rather than
/// `1.0000` below 1.
#[must_use]
pub fn opt_coverage(v: Option<f64>, decimals: usize) -> String {
    v.map_or_else(
        || "\u{2014}".to_owned(),
        |x| human::fixed_bounded(x, decimals, 1.0),
    )
}

/// A position's sample coverage with `decimals` places, as the facts, the table and the chart
/// show it: as [`opt_coverage`], and, at a position below the run's minimum coverage `min`,
/// bounded at that minimum (`> 0.9499` rather than `0.9500` for 0.94996 below 0.95), so a
/// flagged value never reads as reaching the minimum it is below.
#[must_use]
pub fn position_coverage(
    p: &dima_core::model::PositionResult,
    min: Option<f64>,
    decimals: usize,
) -> String {
    match (p.sample_coverage, min) {
        (Some(x), Some(m)) if p.below_min_coverage == Some(true) => {
            human::fixed_bounded(x, decimals, m)
        }
        _ => opt_coverage(p.sample_coverage, decimals),
    }
}

/// An optional incidence (a percentage of the supporting sequences, at most 100%): `21.95%`,
/// `< 0.01%`, `> 99.99%` rather than `100.00%` below 100, or `—`.
#[must_use]
pub fn opt_percent(v: Option<f64>, decimals: usize) -> String {
    v.map_or_else(
        || "\u{2014}".to_owned(),
        |x| human::percent_bounded(x, decimals),
    )
}

/// The panic payload of a caught panic as text.
#[must_use]
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

/// Details to attach to an internal-error report: what failed, the version and the platform
/// (no paths, user or host names).
#[must_use]
pub fn internal_error_details(what: &str, message: &str) -> String {
    format!(
        "{what}\ninternal error: {message}\n\ndima-gui {} ({}; {} {})\nPlease report this at {}/issues",
        env!("CARGO_PKG_VERSION"),
        env!("DIMA_GUI_TARGET"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        env!("CARGO_PKG_REPOSITORY"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_is_character_based_and_never_panics() {
        assert_eq!(truncate_display("ACDEFG", 20), "ACDEFG");
        assert_eq!(truncate_display("ACDEF", 5), "ACDEF");
        assert_eq!(truncate_display("ABCDEFGHIJ", 4), "ABCD...");
        assert_eq!(truncate_display("αβγδεζ", 3), "αβγ...");
        assert_eq!(truncate_display("αβγδε", 8), "αβγδε");
        assert_eq!(truncate_display("🧬🧬🧬🧬", 2), "🧬🧬...");
        assert_eq!(truncate_display("ACDE", 0), "...");
        assert_eq!(truncate_display("", 0), "");
        let s = "Aα🧬G\u{0301}Z";
        for n in 0..=s.chars().count() + 3 {
            let _ = truncate_display(s, n);
        }
    }

    #[test]
    fn relative_times() {
        let now = 10_000_000;
        assert_eq!(format_relative_time(now, 0), "");
        assert_eq!(format_relative_time(now, now + 5), "");
        assert_eq!(format_relative_time(now, now - 30), "just now");
        assert_eq!(format_relative_time(now, now - 60), "1 min ago");
        assert_eq!(format_relative_time(now, now - 7200), "2 hours ago");
        assert_eq!(format_relative_time(now, now - 90_000), "yesterday");
        assert_eq!(format_relative_time(now, now - 3 * 86_400), "3 days ago");
        assert_eq!(format_relative_time(now, now - 14 * 86_400), "2 weeks ago");
        assert_eq!(format_relative_time(now, now - 61 * 86_400), "2 months ago");
    }

    #[test]
    fn pasted_paths_in_the_forms_file_managers_copy() {
        let home = Path::new("/home/ana");
        let p = |s: &str| pasted_path(s, Some(home));
        assert_eq!(p("  "), None);
        assert_eq!(p("\"\""), None);
        assert_eq!(
            p(" /data/spike.fa\n"),
            Some(PathBuf::from("/data/spike.fa"))
        );
        assert_eq!(
            p("\"/data/my spike.fa\""),
            Some(PathBuf::from("/data/my spike.fa"))
        );
        assert_eq!(p("'/data/x.fa'"), Some(PathBuf::from("/data/x.fa")));
        assert_eq!(
            p("file:///data/my%20spike%C3%A9.fa"),
            Some(PathBuf::from("/data/my spike\u{e9}.fa"))
        );
        assert_eq!(
            p("file://localhost/data/x.fa"),
            Some(PathBuf::from("/data/x.fa"))
        );
        // A malformed escape is kept as typed (the open then reports the path).
        assert_eq!(p("file:///a%zz"), Some(PathBuf::from("file:///a%zz")));
        assert_eq!(p("~"), Some(PathBuf::from("/home/ana")));
        assert_eq!(p("~/x.fa"), Some(PathBuf::from("/home/ana/x.fa")));
        assert_eq!(p("~bob/x.fa"), Some(PathBuf::from("~bob/x.fa")));
        assert_eq!(pasted_path("~/x.fa", None), Some(PathBuf::from("~/x.fa")));
    }

    #[test]
    fn quoting_and_optionals() {
        assert_eq!(shell_quote("A|B"), "'A|B'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        let words =
            |a: &[&str]| shell_words(&a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        assert_eq!(
            words(&[
                "dima",
                "analyze",
                "--input",
                "my spike.fa",
                "--header-format",
                "A|B"
            ]),
            "dima analyze --input 'my spike.fa' --header-format 'A|B'"
        );
        assert_eq!(
            words(&["--query-name", "", "--header-fillna", "n/a", "-s", "100"]),
            "--query-name '' --header-fillna n/a -s 100"
        );
        assert_eq!(
            words(&["$HOME", "a*b", "x;y", "Zürich"]),
            "'$HOME' 'a*b' 'x;y' 'Zürich'"
        );
        assert_eq!(opt_f64(None, 3), "\u{2014}");
        assert_eq!(opt_f64(Some(1.23456), 3), "1.235");
        assert_eq!(opt_f64(Some(0.0), 4), "0.0000");
        assert_eq!(
            opt_f64(Some(3.288e-5), 4),
            "< 0.0001",
            "never a misleading zero"
        );
        assert_eq!(
            opt_f64(Some(0.99996), 4),
            "1.0000",
            "entropies have no bound"
        );
        assert_eq!(opt_coverage(None, 3), "\u{2014}");
        assert_eq!(opt_coverage(Some(0.9884), 4), "0.9884");
        assert_eq!(opt_coverage(Some(0.99996), 4), "> 0.9999");
        assert_eq!(opt_coverage(Some(1.0), 3), "1.000");
        assert_eq!(opt_percent(Some(21.951), 2), "21.95%");
        assert_eq!(opt_percent(Some(0.004), 2), "< 0.01%");
        assert_eq!(opt_percent(Some(99.996), 2), "> 99.99%");
        assert_eq!(opt_percent(Some(100.0), 1), "100.0%");
        assert_eq!(opt_percent(None, 1), "\u{2014}");
        let payload: Box<dyn std::any::Any + Send> = Box::new("boom");
        assert_eq!(panic_message(payload.as_ref()), "boom");
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from("bang"));
        assert_eq!(panic_message(payload.as_ref()), "bang");
        assert!(internal_error_details("analysis", "x").contains("dima-gui"));
    }

    /// A coverage below the run's minimum never reads as the minimum (0.94996 below 0.95 is
    /// "> 0.9499", not "0.9500"); others read as before, and below 1 never as 1.
    #[test]
    fn coverage_below_the_minimum_never_reads_as_reaching_it() {
        let mut p = crate::charts::series::tests::position(1, Some(0.1), 200, 100);
        p.sample_coverage = Some(0.949_96);
        p.below_min_coverage = Some(true);
        assert_eq!(position_coverage(&p, Some(0.95), 4), "> 0.9499");
        assert_eq!(position_coverage(&p, Some(0.95), 3), "> 0.949");
        assert_eq!(
            position_coverage(&p, None, 4),
            "0.9500",
            "no minimum: as before"
        );
        p.sample_coverage = Some(0.9646);
        assert_eq!(position_coverage(&p, Some(0.98), 4), "0.9646");
        p.below_min_coverage = Some(false);
        p.sample_coverage = Some(0.999_96);
        assert_eq!(position_coverage(&p, Some(0.95), 4), "> 0.9999");
        p.sample_coverage = None;
        p.below_min_coverage = None;
        assert_eq!(position_coverage(&p, Some(0.95), 4), "\u{2014}");
    }
}
