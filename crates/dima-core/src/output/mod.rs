//! Writing and reading results (D10, D22, D28, D31, D50, D61; findings E2, E11, F4, P4).
//!
//! * **JSON** (canonical, complete): compact by default or indented with `pretty`; always ends
//!   with a newline. Every number is finite (checked before writing: JSON has no NaN).
//! * **JSONL**: one position per line (Nextclade's NDJSON convention); the provenance and
//!   summaries live only in the JSON.
//! * **TSV**: the tidy tables of [`tsv::Table`].
//! * `.gz` and `.zst` outputs are compressed; the format comes from the extension (compound and
//!   case-insensitive: `x.JSONL.ZST`) unless given explicitly.
//! * [`read_results`] reads plain, gzip or Zstandard results JSON (detected by content) and
//!   rejects other schema major versions with a clear message. Errors reading the file are I/O
//!   errors (exit 3); corrupt or truncated compressed data is invalid input (exit 4).

mod sink;
pub mod tsv;

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub use sink::{OutputCompression, Sink, Target, Written, destination_of, remove_temporaries};
pub use tsv::{NA, Table, write_table};

use crate::error::{DimaError, IoAction, IoTarget, Location, Result};
use crate::input::StoredReadError;
use crate::model::{HCS_SCHEMA_VERSION, HcsResults, PositionResult, Results, SCHEMA_VERSION};

/// A file-name-safe version of a query name, for output file names (`--tsv-dir`, the desktop
/// app's exports): characters other than letters, digits, `.`, `-` and `_` become `_`; leading
/// and trailing dots and underscores are removed (no hidden files); `dima` if nothing is left.
#[must_use]
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches(|c| c == '.' || c == '_');
    if s.is_empty() {
        "dima".to_owned()
    } else {
        s.to_owned()
    }
}

/// Serialisation format of an output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// The complete results as one JSON document.
    Json,
    /// One JSON object per position per line.
    Jsonl,
    /// A tab-separated table.
    Tsv,
}

impl OutputFormat {
    /// Name on the command line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Jsonl => "jsonl",
            Self::Tsv => "tsv",
        }
    }
}

/// Format and compression implied by a file name's extensions (case-insensitive): an optional
/// `.gz`/`.zst` after `.json`, `.jsonl`/`.ndjson` or `.tsv`/`.tab`. `None` when the name has no
/// recognised format extension.
#[must_use]
pub fn detect(path: &Path) -> (Option<OutputFormat>, OutputCompression) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let (stem, compression) = if let Some(s) = name.strip_suffix(".gz") {
        (s, OutputCompression::Gzip)
    } else if let Some(s) = name.strip_suffix(".zst") {
        (s, OutputCompression::Zstd)
    } else {
        (name.as_str(), OutputCompression::None)
    };
    let format = if stem.ends_with(".jsonl") || stem.ends_with(".ndjson") {
        Some(OutputFormat::Jsonl)
    } else if stem.ends_with(".json") {
        Some(OutputFormat::Json)
    } else if stem.ends_with(".tsv") || stem.ends_with(".tab") {
        Some(OutputFormat::Tsv)
    } else {
        None
    };
    (format, compression)
}

/// Checks that every floating-point value of the results is finite.
///
/// # Errors
/// [`DimaError::Internal`] naming the first non-finite value.
pub fn check_finite(r: &Results) -> Result<()> {
    let bad = |what: String| Err(DimaError::internal(format!("non-finite {what} in results")));
    let ok = |v: Option<f64>| v.is_none_or(f64::is_finite);
    let summaries = [
        r.average_entropy,
        r.lowest_entropy.map(|e| e.entropy),
        r.highest_entropy.map(|e| e.entropy),
        r.all_positions_summary.average_entropy,
        r.all_positions_summary.lowest_entropy.map(|e| e.entropy),
        r.all_positions_summary.highest_entropy.map(|e| e.entropy),
        r.coverage_summary.min,
        r.coverage_summary.mean,
        r.coverage_summary.min_coverage,
    ];
    let similarity = r.similarity_weighted_entropy_summary.as_ref();
    let similarity_summaries = [
        similarity.and_then(|s| s.average_entropy),
        similarity.and_then(|s| s.lowest_entropy.map(|e| e.entropy)),
        similarity.and_then(|s| s.highest_entropy.map(|e| e.entropy)),
        similarity.and_then(|s| s.average_observed_entropy),
    ];
    let summaries = summaries.into_iter().chain(similarity_summaries);
    if !summaries.into_iter().all(ok) {
        return bad("summary value".into());
    }
    for p in &r.results {
        let values = [
            p.entropy,
            p.observed_entropy,
            p.distinct_variants_incidence,
            p.total_variants_incidence,
            p.sample_coverage,
            p.similarity_weighted_entropy,
            p.normalised_observed_entropy,
            p.normalised_similarity_weighted_entropy,
        ];
        if !values.into_iter().all(ok)
            || !p.diversity_motifs.iter().all(|v| v.incidence.is_finite())
        {
            return bad(format!("value at position {}", p.position));
        }
    }
    Ok(())
}

/// Writes the results as JSON (compact or indented), followed by a newline.
///
/// # Errors
/// [`DimaError::Internal`] for non-finite values; write errors of `out`.
pub fn write_json(results: &Results, pretty: bool, out: &mut Sink) -> Result<()> {
    check_finite(results)?;
    if results.schema_version == SCHEMA_VERSION {
        write_json_value(results, pretty, out)
    } else {
        // Read from a file of another minor version: written as this version holds it (keys it
        // does not know were not read; keys an older version did not have are null, and options
        // it did not have are off), so with this version's schema number.
        let mut current = results.clone();
        current.schema_version = SCHEMA_VERSION.to_owned();
        write_json_value(&current, pretty, out)
    }
}

/// Writes HCS results as JSON, followed by a newline.
///
/// # Errors
/// Write errors of `out`.
pub fn write_hcs_json(hcs: &HcsResults, pretty: bool, out: &mut Sink) -> Result<()> {
    check_finite_hcs(hcs)?;
    if hcs.schema_version == HCS_SCHEMA_VERSION {
        write_json_value(hcs, pretty, out)
    } else {
        let mut current = hcs.clone();
        current.schema_version = HCS_SCHEMA_VERSION.to_owned();
        write_json_value(&current, pretty, out)
    }
}

fn write_json_value(value: &impl serde::Serialize, pretty: bool, out: &mut Sink) -> Result<()> {
    let r = if pretty {
        serde_json::to_writer_pretty(&mut *out, value)
    } else {
        serde_json::to_writer(&mut *out, value)
    };
    r.map_err(|e| out.error(io::Error::from(e)))?;
    out.write_all(b"\n").map_err(|e| out.error(e))
}

/// Writes one JSON object per position, one per line.
///
/// # Errors
/// [`DimaError::Internal`] for non-finite values; write errors of `out`.
pub fn write_jsonl(results: &Results, out: &mut Sink) -> Result<()> {
    check_finite(results)?;
    for p in &results.results {
        serde_json::to_writer(&mut *out, p).map_err(|e| out.error(io::Error::from(e)))?;
        out.write_all(b"\n").map_err(|e| out.error(e))?;
    }
    Ok(())
}

/// Writes a TSV table.
///
/// # Errors
/// [`DimaError::Internal`] for non-finite values; write errors of `out`.
pub fn write_tsv(
    results: &Results,
    hcs: Option<&HcsResults>,
    table: Table,
    out: &mut Sink,
) -> Result<()> {
    check_finite(results)?;
    match (table, hcs) {
        (Table::Hcs, None) => return Err(DimaError::internal("the HCS table needs HCS results")),
        (Table::Hcs, Some(h)) => check_finite_hcs(h)?,
        _ => {}
    }
    write_table(results, hcs, table, out).map_err(|e| out.error(e))
}

/// Reads a results JSON file (plain, gzip or Zstandard, detected by content); `-` reads standard
/// input.
///
/// # Errors
/// [`DimaError::Io`] if it cannot be read; [`DimaError::InputData`] if it is not a DiMA results
/// file of this schema major version, or its compressed data is corrupt or truncated.
pub fn read_results(path: &Path) -> Result<Results> {
    read_results_from(open_json(path, None)?)
}

/// [`read_results`] that stops with [`DimaError::Cancelled`] soon after `cancel` is set (a
/// front end no longer wants the file, which can be gigabytes).
///
/// # Errors
/// As [`read_results`], and [`DimaError::Cancelled`].
pub fn read_results_cancellable(path: &Path, cancel: &AtomicBool) -> Result<Results> {
    read_results_from(open_json(path, Some(cancel))?)
}

/// Reads a conserved-sequences JSON file (written by `dima hcs` or `--hcs-output`).
///
/// # Errors
/// As [`read_results`].
pub fn read_hcs(path: &Path) -> Result<HcsResults> {
    let h: HcsResults = open_json(path, None)?.parse(Kind::Hcs)?;
    check_finite_hcs(&h)?;
    Ok(h)
}

/// A DiMA JSON document of either kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Document {
    /// The results of an analysis.
    Results(Box<Results>),
    /// Conserved sequences.
    Hcs(Box<HcsResults>),
}

/// Reads a results or conserved-sequences JSON file, whichever it is (told apart by the keys
/// DiMA writes first).
///
/// # Errors
/// As [`read_results`].
pub fn read_document(path: &Path) -> Result<Document> {
    let file = open_json(path, None)?;
    if file.kind() == Kind::Hcs {
        let h: HcsResults = file.parse(Kind::Hcs)?;
        check_finite_hcs(&h)?;
        Ok(Document::Hcs(Box::new(h)))
    } else {
        read_results_from(file).map(|r| Document::Results(Box::new(r)))
    }
}

fn read_results_from(file: JsonFile<'_>) -> Result<Results> {
    let name = file.name.clone();
    let mut results: Results = file.parse(Kind::Results)?;
    // Positions listed out of order (for example by a tool that edited the file) are sorted
    // (G25); repeated or missing positions are then reported by the check.
    if !results.results.is_sorted_by_key(|p| p.position) {
        results.results.sort_by_key(|p| p.position);
    }
    check_consistent(&results, &name)?;
    Ok(results)
}

/// The two kinds of DiMA JSON file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Results,
    Hcs,
}

impl Kind {
    /// The schema's name in messages.
    const fn schema_name(self) -> &'static str {
        match self {
            Self::Results => "results",
            Self::Hcs => "conserved-sequences",
        }
    }

    const fn what(self) -> &'static str {
        match self {
            Self::Results => "results file",
            Self::Hcs => "conserved-sequences file",
        }
    }

    /// The schema version this version of DiMA writes for the kind.
    const fn current_version(self) -> &'static str {
        match self {
            Self::Results => SCHEMA_VERSION,
            Self::Hcs => HCS_SCHEMA_VERSION,
        }
    }

    /// What to do with a file a newer DiMA wrote: the key that names its version differs.
    const fn newer_hint(self) -> &'static str {
        match self {
            Self::Results => {
                "open it with the DiMA version that wrote it (its run.tool.version) or a newer one"
            }
            Self::Hcs => {
                "open it with the DiMA version that wrote it (its tool.version) or a newer one"
            }
        }
    }

    const fn hint(self) -> &'static str {
        match self {
            Self::Results => "give the JSON written by `dima analyze` (optionally .gz or .zst)",
            Self::Hcs => {
                "give the JSON written by `dima hcs` or `--hcs-output` (optionally .gz or .zst)"
            }
        }
    }
}

/// A document's schema version.
trait Versioned {
    fn schema_version(&self) -> &str;
}

impl Versioned for Results {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl Versioned for HcsResults {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

/// The stored bytes of a JSON file: marks read errors (I/O, exit 3) apart from decoding errors
/// (corrupt data, exit 4), and stops when cancelled.
struct Stored<'a> {
    raw: Box<dyn Read + 'a>,
    cancel: Option<&'a AtomicBool>,
}

/// Marks the error that stops a cancelled read.
#[derive(Debug)]
struct ReadCancelled;

impl std::fmt::Display for ReadCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for ReadCancelled {}

impl Read for Stored<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(io::Error::other(ReadCancelled));
        }
        loop {
            match self.raw.read(buf) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(io::Error::new(e.kind(), StoredReadError(e))),
                ok => return ok,
            }
        }
    }
}

/// The compression of a JSON file, for messages and the command that tests it.
#[derive(Clone, Copy)]
enum Packed {
    Plain,
    Gzip,
    Zstd,
}

/// The decompressed text of a JSON file: the bytes already read for recognition, then the rest.
type Rejoined<'a> = io::Chain<io::Cursor<Vec<u8>>, Box<dyn Read + 'a>>;

/// A JSON file opened for parsing: decompressed, with its first bytes read.
struct JsonFile<'a> {
    reader: BufReader<Rejoined<'a>>,
    /// The first bytes (lossily decoded), where DiMA writes its leading keys.
    prefix: String,
    name: String,
    target: IoTarget,
    packed: Packed,
}

fn open_json<'a>(path: &Path, cancel: Option<&'a AtomicBool>) -> Result<JsonFile<'a>> {
    let (target, raw): (IoTarget, Box<dyn Read>) = if path.as_os_str() == "-" {
        (IoTarget::Stdin, Box::new(io::stdin()))
    } else {
        let t = IoTarget::Path(path.to_path_buf());
        if path.is_dir() {
            return Err(
                DimaError::usage(format!("'{}' is a directory", path.display()))
                    .with_hint("give a JSON file written by DiMA"),
            );
        }
        let f = File::open(path).map_err(|e| DimaError::io(IoAction::Open, t.clone(), e))?;
        (t, Box::new(f))
    };
    let name = match &target {
        IoTarget::Path(p) => p.display().to_string(),
        _ => "standard input".to_owned(),
    };
    let mut packed = Packed::Plain;
    let read_err = |e: io::Error, packed: Packed| stream_error(e, &target, &name, packed);
    // Sniffing reads fixed prefixes however the input is split into reads (pipes, slow
    // decompression), then puts them back in front of the stream.
    let mut raw = Stored { raw, cancel };
    let head = read_prefix(&mut raw, 4).map_err(|e| read_err(e, packed))?;
    let stored = BufReader::with_capacity(1 << 20, io::Cursor::new(head.clone()).chain(raw));
    let mut decoded: Box<dyn Read + 'a> = match head.as_slice() {
        [0x1F, 0x8B, ..] => {
            packed = Packed::Gzip;
            Box::new(flate2::bufread::MultiGzDecoder::new(stored))
        }
        [0x28, 0xB5, 0x2F, 0xFD, ..] => {
            packed = Packed::Zstd;
            Box::new(
                zstd::stream::read::Decoder::with_buffer(stored)
                    .map_err(|e| read_err(e, packed))?,
            )
        }
        _ => Box::new(stored),
    };
    // DiMA writes the schema version first, then the keys that tell the kinds apart, so a file
    // of another major version or kind is refused before the whole file is parsed. A
    // byte-order mark (added by some editors) is skipped.
    let mut first = read_prefix(&mut decoded, 256).map_err(|e| read_err(e, packed))?;
    if first.starts_with(b"\xEF\xBB\xBF") {
        first.drain(..3);
    }
    let prefix = String::from_utf8_lossy(&first).into_owned();
    Ok(JsonFile {
        reader: BufReader::with_capacity(1 << 20, io::Cursor::new(first).chain(decoded)),
        prefix,
        name,
        target,
        packed,
    })
}

impl JsonFile<'_> {
    /// The kind the leading keys show: results open with `query_name`, conserved sequences
    /// with `tool` and `source` (whose object holds the results' `query_name`). Results when
    /// neither shows.
    fn kind(&self) -> Kind {
        let at = |key: &str| self.prefix.find(&format!("\"{key}\""));
        match (at("source"), at("query_name")) {
            (Some(s), Some(q)) if s < q => Kind::Hcs,
            (Some(_), None) => Kind::Hcs,
            _ => Kind::Results,
        }
    }

    fn parse<T: serde::de::DeserializeOwned + Versioned>(self, kind: Kind) -> Result<T> {
        let name = self.name.clone();
        let not_it = || {
            DimaError::input_data(
                Some(Location::new(name.clone())),
                format!("not a DiMA {}", kind.what()),
            )
            .with_hint(kind.hint())
        };
        if !self.prefix.trim_start().starts_with('{') {
            return Err(not_it());
        }
        let found = self.kind();
        if found != kind {
            return Err(DimaError::input_data(
                Some(Location::new(name.clone())),
                format!("this is a DiMA {}, not a {}", found.what(), kind.what()),
            )
            .with_hint(kind.hint()));
        }
        let early_version = early_schema_version(&self.prefix);
        let current = kind.current_version();
        if let Some(version) = early_version {
            check_schema_major(version, kind, &name)?;
        }
        // A newer 1.x may use values (an estimator, a method) this version does not know.
        let newer = early_version
            .filter(|v| is_newer(v, current))
            .map(str::to_owned);
        let (target, packed) = (self.target, self.packed);
        let value: T = serde_json::from_reader(self.reader).map_err(|e| {
            if e.is_io() {
                stream_error(io::Error::from(e), &target, &name, packed)
            } else if e.to_string().starts_with("missing field `schema_version`") {
                // A JSON object, but not one written by DiMA.
                not_it()
            } else if let Some(version) = &newer {
                DimaError::input_data(
                    Some(Location::new(name.clone())),
                    format!(
                        "written by a newer DiMA ({} schema {version}) with values this \
                         version (schema {current}) does not know: {e}",
                        kind.schema_name()
                    ),
                )
                .with_hint(kind.newer_hint())
            } else {
                DimaError::input_data(
                    Some(Location::new(name.clone())),
                    format!("not a valid DiMA {}: {e}", kind.what()),
                )
                .with_hint(kind.hint())
            }
        })?;
        check_schema_major(value.schema_version(), kind, &name)?;
        Ok(value)
    }
}

/// An error while reading a JSON file: a cancellation, an I/O error on the stored bytes,
/// or corrupt or truncated compressed data.
fn stream_error(e: io::Error, target: &IoTarget, name: &str, packed: Packed) -> DimaError {
    let kind = e.kind();
    let detail = e.to_string();
    if e.get_ref().is_some_and(|i| i.is::<ReadCancelled>()) {
        return DimaError::Cancelled;
    }
    if e.get_ref().is_some_and(|i| i.is::<StoredReadError>()) {
        let stored = e
            .into_inner()
            .and_then(|i| i.downcast::<StoredReadError>().ok())
            .map_or_else(|| io::Error::from(kind), |s| s.0);
        return DimaError::io(IoAction::Read, target.clone(), stored);
    }
    // Every error reading the stored bytes is marked, so the others come from the decoder.
    let (format, check) = match packed {
        Packed::Plain => return DimaError::io(IoAction::Read, target.clone(), e),
        Packed::Gzip => ("gzip", "gzip -t"),
        Packed::Zstd => ("Zstandard", "zstd -t"),
    };
    let what = if kind == io::ErrorKind::UnexpectedEof {
        "truncated"
    } else {
        "corrupt"
    };
    DimaError::input_data(
        Some(Location::new(name.to_owned())),
        format!("{what} {format} data: {detail}"),
    )
    .with_hint(format!(
        "the file may be truncated or damaged; test it with `{check}`, or write it again with \
         `dima analyze` or `dima view`"
    ))
}

/// The string value of the first `"schema_version"` member in the start of a file, if the key is
/// followed by a string (a number, `null` or a cut-off string gives `None`, and the full parse
/// then reports the file).
fn early_schema_version(prefix: &str) -> Option<&str> {
    let rest = prefix.split("\"schema_version\"").nth(1)?;
    let rest = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    rest.find('"').map(|end| &rest[..end])
}

/// Refuses a file of another schema major version.
fn check_schema_major(version: &str, kind: Kind, name: &str) -> Result<()> {
    let current = kind.current_version();
    let major = |v: &str| v.split('.').next().unwrap_or("").to_owned();
    if major(version) == major(current) {
        return Ok(());
    }
    // A version that is not a plain one is quoted, so spaces or odd characters show.
    let shown = if parse_schema_version(version).is_some() {
        version.to_owned()
    } else {
        format!("{version:?}")
    };
    Err(DimaError::input_data(
        Some(Location::new(name.to_owned())),
        format!(
            "written with {} schema {shown}, but this version of DiMA reads schema \
             {current} (major version {})",
            kind.schema_name(),
            major(current)
        ),
    )
    .with_hint(
        "re-run the analysis with this version, or use the DiMA version that wrote the file",
    ))
}

/// `true` when `version` is a later schema version than `current` (`1.2.0` after `1.0.0`);
/// unparsable versions are not.
fn is_newer(version: &str, current: &str) -> bool {
    matches!(
        (parse_schema_version(version), parse_schema_version(current)),
        (Some(a), Some(b)) if a > b
    )
}

/// A schema version `MAJOR.MINOR.PATCH` as numbers, compared in that order (`1.10.0` after
/// `1.9.0`); `None` unless it is exactly three dot-separated decimal numbers without leading
/// zeros (SemVer 2.0.0, §2), so `1.1`, `1.1.0-dev`, `+1.1.0` and `01.1.0` are `None`.
#[must_use]
pub fn parse_schema_version(version: &str) -> Option<[u64; 3]> {
    let mut parts = version.split('.');
    let mut out = [0u64; 3];
    for slot in &mut out {
        let part = parts.next()?;
        let canonical = part == "0" || (!part.starts_with('0') && !part.is_empty());
        if !canonical || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

/// Up to `n` bytes from the start of `r` (fewer only at the end of the input).
fn read_prefix(r: &mut impl Read, n: u64) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    r.by_ref().take(n).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Checks what downstream computations (conserved sequences, tables) rely on, so that an
/// edited or damaged results file is reported as invalid input instead of failing later:
/// k and T at least 1, positions 1..P (sorted) each listed once, every variant k upper-case
/// letters, counts at least 1 and summing to the position's support, values within their
/// ranges, and the similarity-weighted values present exactly where the run computed them (an
/// invariant of every 1.x results file: all `null` without the option; with it, the
/// similarity-weighted entropy wherever there is support, the normalised values wherever there
/// are at least two distinct k-mers, and the summary).
fn check_consistent(r: &Results, name: &str) -> Result<()> {
    let invalid = |message: String| {
        DimaError::input_data(Some(Location::new(name.to_owned())), message)
            .with_hint("the file was edited or damaged; run `dima analyze` again to recreate it")
    };
    if r.kmer_length == 0 || r.support_threshold == 0 {
        return Err(invalid(
            "the k-mer length and the support threshold must be at least 1".to_owned(),
        ));
    }
    if r.results.len() as u64 != r.position_count {
        return Err(invalid(format!(
            "position_count is {} but there are {} positions",
            r.position_count,
            r.results.len()
        )));
    }
    let k = usize::try_from(r.kmer_length).unwrap_or(usize::MAX);
    let computed = r.run.parameters.similarity_weighted_entropy.value;
    for (i, p) in r.results.iter().enumerate() {
        let expected = i as u64 + 1;
        if p.position != expected {
            return Err(invalid(if p.position == 0 {
                "positions start at 1, but position 0 is listed".to_owned()
            } else if p.position < expected {
                format!("position {} is listed more than once", p.position)
            } else {
                format!("position {expected} is missing")
            }));
        }
        let mut total = 0u64;
        for v in &p.diversity_motifs {
            if v.sequence.len() != k || !v.sequence.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err(invalid(format!(
                    "position {}: variant {:?} is not {k} upper-case letters",
                    p.position, v.sequence
                )));
            }
            if v.count == 0 {
                return Err(invalid(format!(
                    "position {}: variant {} has a count of 0",
                    p.position, v.sequence
                )));
            }
            total = total.saturating_add(v.count);
        }
        if total != p.support {
            return Err(invalid(format!(
                "position {}: the variant counts sum to {total}, but the support is {}",
                p.position, p.support
            )));
        }
        // The ranges of `schemas/results.schema.json`, so an edited value is refused here
        // instead of being written on.
        let values = [
            ("entropy", p.entropy, 0.0, f64::INFINITY),
            ("observed_entropy", p.observed_entropy, 0.0, f64::INFINITY),
            (
                "distinct_variants_incidence",
                p.distinct_variants_incidence,
                0.0,
                100.0,
            ),
            (
                "total_variants_incidence",
                p.total_variants_incidence,
                0.0,
                100.0,
            ),
            ("sample_coverage", p.sample_coverage, 0.0, 1.0),
            (
                "similarity_weighted_entropy",
                p.similarity_weighted_entropy,
                0.0,
                f64::INFINITY,
            ),
            (
                "normalised_observed_entropy",
                p.normalised_observed_entropy,
                0.0,
                1.0,
            ),
            (
                "normalised_similarity_weighted_entropy",
                p.normalised_similarity_weighted_entropy,
                0.0,
                1.0,
            ),
        ];
        for (field, value, low, high) in values {
            if let Some(v) = value {
                if !(low..=high).contains(&v) {
                    return Err(invalid(format!(
                        "position {}: {field} is {v}, outside {low} to {high}",
                        p.position
                    )));
                }
            }
        }
        if let Some(v) = p
            .diversity_motifs
            .iter()
            .find(|v| !(v.incidence > 0.0 && v.incidence <= 100.0))
        {
            return Err(invalid(format!(
                "position {}: variant {} has an incidence of {}, outside (0, 100]",
                p.position, v.sequence, v.incidence
            )));
        }
        if let Some(problem) = similarity_presence(p, computed) {
            return Err(invalid(format!("position {}: {problem}", p.position)));
        }
    }
    if let Some(problem) = summary_problem(r) {
        return Err(invalid(problem));
    }
    match (computed, r.similarity_weighted_entropy_summary.is_some()) {
        (true, false) => Err(invalid(
            "similarity_weighted_entropy_summary is missing, but the run computed \
             similarity-weighted entropy"
                .to_owned(),
        )),
        (false, true) => Err(invalid(
            "similarity_weighted_entropy_summary is given, but the run did not compute \
             similarity-weighted entropy"
                .to_owned(),
        )),
        _ => Ok(()),
    }
}

/// The first value of the summaries outside the range the schema gives it, if any: means and
/// extremes of entropies at least 0, coverages between 0 and 1, an extreme's position and tie
/// count at least 1.
fn summary_problem(r: &Results) -> Option<String> {
    let all = &r.all_positions_summary;
    let cov = &r.coverage_summary;
    let swe = r.similarity_weighted_entropy_summary.as_ref();
    let mut extremes = vec![
        ("lowest_entropy", r.lowest_entropy.as_ref()),
        ("highest_entropy", r.highest_entropy.as_ref()),
        (
            "all_positions_summary.lowest_entropy",
            all.lowest_entropy.as_ref(),
        ),
        (
            "all_positions_summary.highest_entropy",
            all.highest_entropy.as_ref(),
        ),
    ];
    let mut values = vec![
        ("average_entropy", r.average_entropy, 0.0, f64::INFINITY),
        (
            "all_positions_summary.average_entropy",
            all.average_entropy,
            0.0,
            f64::INFINITY,
        ),
        ("coverage_summary.min", cov.min, 0.0, 1.0),
        ("coverage_summary.mean", cov.mean, 0.0, 1.0),
        ("coverage_summary.min_coverage", cov.min_coverage, 0.0, 1.0),
    ];
    if let Some(s) = swe {
        extremes.push((
            "similarity_weighted_entropy_summary.lowest_entropy",
            s.lowest_entropy.as_ref(),
        ));
        extremes.push((
            "similarity_weighted_entropy_summary.highest_entropy",
            s.highest_entropy.as_ref(),
        ));
        values.push((
            "similarity_weighted_entropy_summary.average_entropy",
            s.average_entropy,
            0.0,
            f64::INFINITY,
        ));
        values.push((
            "similarity_weighted_entropy_summary.average_observed_entropy",
            s.average_observed_entropy,
            0.0,
            f64::INFINITY,
        ));
    }
    for (field, e) in extremes {
        if let Some(e) = e {
            values.push((field, Some(e.entropy), 0.0, f64::INFINITY));
            if e.position == 0 || e.tied_position_count == 0 {
                return Some(format!(
                    "{field} has a position or tie count of 0, not at least 1"
                ));
            }
        }
    }
    values.into_iter().find_map(|(field, value, low, high)| {
        value
            .filter(|v| !(low..=high).contains(v))
            .map(|v| format!("{field} is {v}, outside {low} to {high}"))
    })
}

/// What is wrong with the similarity-weighted values of a position, if anything: with
/// `computed` off all three are `null`; on, similarity-weighted entropy is given exactly where
/// there is support, and the normalised values exactly where there are two or more distinct
/// k-mers.
fn similarity_presence(p: &PositionResult, computed: bool) -> Option<String> {
    let normalised = (
        p.diversity_motifs.len() >= 2,
        "no support or one distinct k-mer",
    );
    let values = [
        (
            "similarity_weighted_entropy",
            p.similarity_weighted_entropy,
            (p.support >= 1, "no support"),
        ),
        (
            "normalised_observed_entropy",
            p.normalised_observed_entropy,
            normalised,
        ),
        (
            "normalised_similarity_weighted_entropy",
            p.normalised_similarity_weighted_entropy,
            normalised,
        ),
    ];
    values
        .into_iter()
        .find_map(
            |(field, value, (defined, why))| match (value.is_some(), computed && defined) {
                (true, false) if !computed => Some(format!(
                    "{field} is given, but the run did not compute similarity-weighted entropy"
                )),
                (true, false) => Some(format!(
                    "{field} is given, but it is undefined here ({why})"
                )),
                (false, true) => Some(format!("{field} is missing")),
                _ => None,
            },
        )
}

/// Rejects non-finite numbers in conserved-sequence results.
///
/// # Errors
/// [`DimaError::Internal`]: a non-finite value is a bug.
pub fn check_finite_hcs(h: &HcsResults) -> Result<()> {
    let finite = h.regions.iter().all(|r| {
        r.min_incidence.is_finite()
            && r.conservation_lower_bound_95.is_finite()
            && r.members
                .iter()
                .all(|m| m.incidence.is_finite() && m.conservation_lower_bound_95.is_finite())
    });
    if finite {
        Ok(())
    } else {
        Err(DimaError::internal(
            "non-finite value in conserved-sequence results",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn file_stems_are_safe_names() {
        assert_eq!(file_stem("MERS spike/aa"), "MERS_spike_aa");
        assert_eq!(file_stem("..."), "dima");
        assert_eq!(file_stem(""), "dima");
        assert_eq!(file_stem("HIV-1.pol"), "HIV-1.pol");
        assert_eq!(file_stem(".hidden_"), "hidden");
        assert_eq!(file_stem("Zürich"), "Zürich");
    }

    #[test]
    fn formats_from_extensions() {
        let d = |s: &str| detect(&PathBuf::from(s));
        assert_eq!(
            d("out.json"),
            (Some(OutputFormat::Json), OutputCompression::None)
        );
        assert_eq!(
            d("dir.v1/OUT.JSON.GZ"),
            (Some(OutputFormat::Json), OutputCompression::Gzip)
        );
        assert_eq!(
            d("x.jsonl.zst"),
            (Some(OutputFormat::Jsonl), OutputCompression::Zstd)
        );
        assert_eq!(
            d("x.ndjson"),
            (Some(OutputFormat::Jsonl), OutputCompression::None)
        );
        assert_eq!(
            d("x.tab"),
            (Some(OutputFormat::Tsv), OutputCompression::None)
        );
        assert_eq!(
            d("x.tsv.gz"),
            (Some(OutputFormat::Tsv), OutputCompression::Gzip)
        );
        assert_eq!(d("x.txt"), (None, OutputCompression::None));
        assert_eq!(d("x.gz"), (None, OutputCompression::Gzip));
        assert_eq!(d("-"), (None, OutputCompression::None));
    }
}
