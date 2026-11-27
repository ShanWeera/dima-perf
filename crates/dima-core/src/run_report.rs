//! The run report (D103): how a run went, written to a separate JSON file
//! (`schemas/run-report.schema.json`) so that the results stay byte-identical across runs.
//!
//! It records what an audit needs: version and build target, the command (rebuilt from the
//! parameters, file names only), status, UTC times, threads and where their number came from,
//! platform, phase times, CPU time, peak memory, and the size and SHA-256 of every input and
//! output. It never records anything that identifies a person or machine: no host or user
//! names, no directories, no environment values, no raw command line (data minimisation, GDPR
//! art. 5 and 25). Field names follow the Workflow Run RO-Crate terms (Leo et al. 2024) so a
//! Process Run Crate export can follow.
//!
//! Front ends fill it: the CLI (`--run-report`) and the desktop app ("Save run report…").

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::control::Phase;

/// Version of the run-report schema.
pub const REPORT_SCHEMA_VERSION: &str = "1.1.0";

/// The program that ran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Tool {
    /// `dima` (the CLI) or `dima-gui` (the desktop app).
    pub name: &'static str,
    /// Its version.
    pub version: &'static str,
    /// The Rust target triple it was built for.
    pub target: String,
}

/// Worker threads and where their number came from (D101).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Threads {
    /// Worker threads requested.
    pub count: usize,
    /// Worker threads started: `count`, or fewer when the system refused some (D110); `None`
    /// when the run stopped before they were started.
    pub started: Option<usize>,
    /// `--threads`, `RAYON_NUM_THREADS`, `available_parallelism`, `settings` (desktop app) or
    /// the name of the scheduler variable that limited the count. Never a value that could
    /// name a host.
    pub source: &'static str,
}

/// Operating system and processor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Platform {
    /// `linux`, `macos`, `windows`, …
    pub os: &'static str,
    /// `x86_64`, `aarch64`, …
    pub arch: &'static str,
    /// Logical CPUs available to the process.
    pub logical_cpus: usize,
}

impl Platform {
    /// This process's platform.
    #[must_use]
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            logical_cpus: std::thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get),
        }
    }
}

/// Time and memory used.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Resources {
    /// Wall-clock time of the run.
    pub wall_seconds: f64,
    /// CPU time in user mode; `None` where it is not measured (the desktop app's process time
    /// includes drawing the interface).
    pub cpu_user_seconds: Option<f64>,
    /// CPU time in kernel mode; `None` where it is not measured.
    pub cpu_system_seconds: Option<f64>,
    /// Peak resident memory; `None` where it is not measured.
    pub peak_rss_bytes: Option<u64>,
    /// Time the counting threads waited for input while counting k-mers (the first batch, read
    /// while opening, is not included); `None` when the analysis itself did not finish. A run
    /// that fails or is cancelled while its outputs are written keeps the value.
    pub waited_for_input_seconds: Option<f64>,
}

/// Wall time of one phase.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PhaseTime {
    /// [`Phase::as_str`].
    pub name: &'static str,
    /// Its wall-clock time.
    pub wall_seconds: f64,
}

impl PhaseTime {
    /// The time spent in `phase`.
    #[must_use]
    pub fn new(phase: Phase, time: Duration) -> Self {
        Self {
            name: phase.as_str(),
            wall_seconds: time.as_secs_f64(),
        }
    }
}

/// Amount of work done.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Work {
    /// Records read.
    pub records: u64,
    /// Positions analysed; `None` if the run stopped before.
    pub positions: Option<u64>,
}

/// An input or output file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileRecord {
    /// `alignment`, `results`, `results_jsonl`, `positions_table`, `variants_table`,
    /// `metadata_table`, `hcs_table` or `hcs`.
    pub role: &'static str,
    /// File name without directories; `-` for a standard stream.
    pub name: String,
    /// Stored size in bytes.
    pub size_bytes: u64,
    /// SHA-256 of the stored bytes (lower-case hex).
    pub sha256: String,
}

/// How the run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Every output was written.
    Completed,
    /// An error stopped the run.
    Failed,
    /// The user cancelled the run.
    Cancelled,
}

/// The run report. Serialised in field order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunReport {
    /// [`REPORT_SCHEMA_VERSION`].
    pub schema_version: &'static str,
    /// The program.
    pub tool: Tool,
    /// The equivalent `dima` command (file names only).
    pub command: Vec<String>,
    /// How the run ended.
    pub status: Status,
    /// The process exit code (CLI) or the code the CLI would have returned.
    pub exit_code: u8,
    /// The error report with directories removed ([`redact_paths`]); `None` unless failed.
    pub error: Option<String>,
    /// Start, RFC 3339 UTC ([`rfc3339`]).
    pub start_time: String,
    /// End, RFC 3339 UTC.
    pub end_time: String,
    /// `None` when the run stopped before the thread count was resolved.
    pub threads: Option<Threads>,
    /// Operating system and processor.
    pub platform: Platform,
    /// Time and memory.
    pub resources: Resources,
    /// Wall time per phase, in order.
    pub phases: Vec<PhaseTime>,
    /// Records and positions.
    pub work: Work,
    /// The input alignment (when it was read to the end).
    pub inputs: Vec<FileRecord>,
    /// Every output written, in order.
    pub outputs: Vec<FileRecord>,
}

/// `2026-10-05T08:12:01.123+00:00` (RFC 3339, UTC, milliseconds).
#[must_use]
pub fn rfc3339(t: SystemTime) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = d.as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, day) = civil_from_days(i64::try_from(days).unwrap_or(0));
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{:03}+00:00",
        rem / 3600,
        (rem / 60) % 60,
        rem % 60,
        d.subsec_millis()
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (y, m as u32, d as u32)
}

/// A path reduced to its file name (no directories), `-` for standard streams and descriptor
/// paths such as `/dev/fd/63` (process substitution).
#[must_use]
pub fn file_name(p: &std::path::Path) -> String {
    if p.as_os_str() == "-" || crate::input::is_descriptor_alias(p) {
        return "-".to_owned();
    }
    p.file_name()
        .map_or_else(|| "-".to_owned(), |n| n.to_string_lossy().into_owned())
}

/// The error text a run report records: the first line of the message (later lines can list
/// input data such as FASTA headers), every cause and the hint, with directories removed
/// ([`redact_paths`]; `known` are the paths given to the program).
#[must_use]
pub fn error_text(error: &crate::DimaError, known: &[&std::path::Path]) -> String {
    let message = error.to_string();
    let mut text = message.lines().next().unwrap_or_default().to_owned();
    let mut cause = std::error::Error::source(error);
    while let Some(c) = cause {
        text.push_str("\ncaused by: ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    if let Some(h) = error.hint() {
        text.push_str("\nhint: ");
        text.push_str(h);
    }
    redact_paths(&text, known)
}

/// `message` with every path reduced to its file name, so error texts in the run report hold
/// no directories (D103): first the paths given on the command line (`known`, longest first),
/// then any quoted path, then any bare absolute path (e.g. in an operating-system message).
#[must_use]
pub fn redact_paths(message: &str, known: &[&std::path::Path]) -> String {
    let mut known: Vec<String> = known
        .iter()
        .map(|p| p.display().to_string())
        .filter(|p| !p.is_empty() && p != "-")
        .collect();
    known.sort_by_key(|p| std::cmp::Reverse(p.len()));
    let mut out = message.to_owned();
    for p in &known {
        out = out.replace(p.as_str(), last_component(p));
    }
    redact_bare(&redact_quoted(&out))
}

const SEPARATORS: [char; 2] = ['/', '\\'];

/// The last non-empty component of a path (`/a/b/` → `b`).
fn last_component(p: &str) -> &str {
    p.rsplit(SEPARATORS).find(|c| !c.is_empty()).unwrap_or(p)
}

/// Reduces `'…'` and `"…"` segments that contain a directory separator.
fn redact_quoted(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['\'', '"']) {
        let quote = &rest[i..=i];
        result.push_str(&rest[..=i]);
        let after = &rest[i + 1..];
        let Some(end) = after.find(quote) else {
            rest = after;
            continue;
        };
        let content = &after[..end];
        result.push_str(if content.contains(SEPARATORS) {
            last_component(content)
        } else {
            content
        });
        result.push_str(quote);
        rest = &after[end + 1..];
    }
    result.push_str(rest);
    result
}

/// `true` for `/…`, `\\…`, `~/…` and `X:\…`/`X:/…`.
fn is_absolute(t: &str) -> bool {
    let b = t.as_bytes();
    t.starts_with(SEPARATORS)
        || t.starts_with("~/")
        || (b.len() > 2
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'/' || b[2] == b'\\'))
}

/// Reduces whitespace-separated absolute paths, keeping surrounding punctuation.
fn redact_bare(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for (i, token) in s.split(' ').enumerate() {
        if i > 0 {
            result.push(' ');
        }
        let core = token.trim_start_matches(['(', '\'', '"']);
        let core = core.trim_end_matches([',', ';', ':', ')', '.', '\'', '"']);
        if core.len() > 1 && is_absolute(core) {
            let start = token.len() - token.trim_start_matches(['(', '\'', '"']).len();
            result.push_str(&token[..start]);
            result.push_str(last_component(core));
            result.push_str(&token[start + core.len()..]);
        } else {
            result.push_str(token);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_text_keeps_no_input_data() {
        let e = crate::DimaError::input_data(
            Some(crate::error::Location::new("/data/run1/in.fasta")),
            "2 headers do not have the 3 fields\n  record 4 (line 7): 2 fields: patient-17|ward B",
        )
        .with_hint("fix the headers");
        let text = error_text(&e, &[std::path::Path::new("/data/run1/in.fasta")]);
        assert_eq!(
            text,
            "in.fasta: 2 headers do not have the 3 fields\nhint: fix the headers"
        );
    }

    #[test]
    fn dates() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00.000+00:00");
        let t = UNIX_EPOCH + Duration::from_millis(1_791_000_000_123);
        assert_eq!(rfc3339(t), "2026-10-03T04:00:00.123+00:00");
        // Leap day.
        let t = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(rfc3339(t), "2000-02-29T00:00:00.000+00:00");
        assert_eq!(file_name(std::path::Path::new("/a/b/c.fa")), "c.fa");
        assert_eq!(file_name(std::path::Path::new("-")), "-");
    }

    #[test]
    fn paths_are_redacted() {
        let known = [std::path::Path::new("data dir/in.fasta")];
        assert_eq!(
            redact_paths(
                "cannot open 'data dir/in.fasta'\ncaused by: No such file",
                &known
            ),
            "cannot open 'in.fasta'\ncaused by: No such file"
        );
        assert_eq!(
            redact_paths("failed at path \"/home/me/out/.tmpA1\" (x)", &[]),
            "failed at path \".tmpA1\" (x)"
        );
        assert_eq!(
            redact_paths("see /home/me/x.json, then C:\\Users\\me\\y.tsv.", &[]),
            "see x.json, then y.tsv."
        );
        // Not paths: ratios, header formats, apostrophes.
        let m = "entropy ~ 1/n; format 'A|B' can't match";
        assert_eq!(redact_paths(m, &[]), m);
    }
}
