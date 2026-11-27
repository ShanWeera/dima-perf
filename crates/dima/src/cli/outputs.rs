//! Output planning and writing (D28, D61, D77, D99).
//!
//! Every destination ([`super::destinations`]) is planned *before* the analysis runs, so a
//! mistyped option never costs a long computation: its name is read once ([`settle`], with the
//! library's [`output::read_name`]) and must name a format and a compression DiMA writes, and
//! never a folder; `-o` writes exactly one stream, a TSV on `-o` needs `--table`, `-O` must
//! agree with the file extension, and `--tsv-dir` must be a folder (created if needed). The
//! files are then opened ([`Opened`]) before the analysis too, so a destination that cannot be
//! written is reported at once. [`Folders`] removes the folders a failed run created, and the
//! [`Ledger`] keeps what was committed, for the message after an interruption and the run
//! report.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dima_core::error::{IoAction, IoTarget};
use dima_core::model::{HcsResults, Results};
use dima_core::output::{self, OutputCompression, OutputFormat, Sink, Table, Target, Written};
use dima_core::run_report::RunReport;
use dima_core::{DimaError, Result};

use super::args::{FormatArg, HcsFormatArg, OutputArgs, TableArg};
use super::destinations::{Destination, Destinations, Kind};
use super::ui::{self, Verbosity};

/// One planned output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    /// Destination.
    pub target: Target,
    /// Compression (from the name).
    pub compression: OutputCompression,
    /// What goes there.
    pub content: Content,
}

/// What an output contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// The results as JSON.
    Json {
        /// Indented.
        pretty: bool,
    },
    /// One position per line.
    Jsonl,
    /// One TSV table.
    Tsv(Table),
    /// Conserved regions as JSON.
    HcsJson {
        /// Indented.
        pretty: bool,
    },
    /// The run report (D103).
    RunReport,
}

impl Content {
    /// Role name in the run report.
    #[must_use]
    pub const fn role(self) -> &'static str {
        match self {
            Self::Json { .. } => "results",
            Self::Jsonl => "results_jsonl",
            Self::Tsv(Table::Positions) => "positions_table",
            Self::Tsv(Table::Variants) => "variants_table",
            Self::Tsv(Table::Metadata) => "metadata_table",
            Self::Tsv(Table::Hcs) => "hcs_table",
            Self::HcsJson { .. } => "hcs",
            Self::RunReport => "run_report",
        }
    }

    /// `true` when conserved regions must be computed.
    #[must_use]
    pub const fn needs_hcs(self) -> bool {
        matches!(self, Self::HcsJson { .. } | Self::Tsv(Table::Hcs))
    }

    /// `true` when the output carries per-position values (the results as JSON or JSONL, or the
    /// positions table): the only outputs with similarity-weighted entropy. Every kind of output
    /// must decide, so a new one cannot be forgotten.
    #[must_use]
    pub const fn carries_position_values(self) -> bool {
        match self {
            Self::Json { .. } | Self::Jsonl | Self::Tsv(Table::Positions) => true,
            Self::Tsv(Table::Variants | Table::Metadata | Table::Hcs)
            | Self::HcsJson { .. }
            | Self::RunReport => false,
        }
    }
}

/// Everything to write, in order (the run report is planned on its own:
/// [`plan_run_report`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Outputs, in writing order.
    pub outputs: Vec<Planned>,
    /// Folders to create (`--tsv-dir`) when the outputs are opened.
    pub directories: Vec<PathBuf>,
    /// Files of conserved sequences whose names give no format, so they are written as JSON
    /// ([`warn_assumed_format`]), with the compression their names ask for.
    pub assumed_json: Vec<(PathBuf, OutputCompression)>,
}

impl Plan {
    /// `true` when conserved regions must be computed.
    #[must_use]
    pub fn needs_hcs(&self) -> bool {
        self.outputs.iter().any(|o| o.content.needs_hcs())
    }

    /// `true` when an output carrying per-position values is planned (results JSON or JSONL,
    /// or the positions table, including the one `--tsv-dir` writes).
    #[must_use]
    pub fn writes_position_values(&self) -> bool {
        self.outputs
            .iter()
            .any(|o| o.content.carries_position_values())
    }

    /// `true` when a JSON document (results or conserved regions) is planned, the only outputs
    /// `--pretty` changes.
    #[must_use]
    pub fn writes_json(&self) -> bool {
        self.outputs
            .iter()
            .any(|o| matches!(o.content, Content::Json { .. } | Content::HcsJson { .. }))
    }

    /// Plans conserved sequences on `d`, in the format its name gives (or `explicit`).
    fn push_hcs(
        &mut self,
        d: &Destination,
        explicit: Option<OutputFormat>,
        pretty: bool,
        stdout_is_terminal: bool,
    ) -> Result<()> {
        let s = settle(Kind::Hcs, &d.target, explicit)?;
        let content = match s.format {
            OutputFormat::Tsv => Content::Tsv(Table::Hcs),
            OutputFormat::Json | OutputFormat::Jsonl => Content::HcsJson {
                pretty: pretty || (d.target == Target::Stdout && stdout_is_terminal),
            },
        };
        if let (true, Target::Path(p)) = (s.assumed, &d.target) {
            self.assumed_json.push((p.clone(), s.compression));
        }
        self.outputs.push(Planned {
            target: d.target.clone(),
            compression: s.compression,
            content,
        });
        Ok(())
    }
}

/// Warns that `--pretty` has no effect when no JSON output is planned; `true` when it warned.
pub fn warn_unused_pretty(pretty: bool, plan: &Plan, v: Verbosity) -> bool {
    let unused = pretty && !plan.writes_json();
    if unused {
        ui::warning(
            "flag_has_no_effect",
            "--pretty has no effect without JSON output",
            None,
            v,
        );
    }
    unused
}

/// Warns (`output_format_assumed`) for each file of conserved sequences whose name gives no
/// format, so the documented JSON is written to it; `format_option`: the command chooses the
/// format with `-O` (`dima hcs`). Returns how many there are.
pub fn warn_assumed_format(plan: &Plan, format_option: bool, v: Verbosity) -> u64 {
    for (path, compression) in &plan.assumed_json {
        let mut hint = format!(
            "name it {}",
            renamed(path, None, *compression, &[".json", ".tsv"], ".hcs")
        );
        if format_option {
            hint.push_str(", or choose the format with -O json or -O tsv");
        }
        ui::warning(
            "output_format_assumed",
            &format!(
                "'{}' does not name JSON or TSV, so the conserved sequences are written to it as \
                 JSON",
                path.display()
            ),
            Some(&hint),
            v,
        );
    }
    plan.assumed_json.len() as u64
}

fn usage(message: impl Into<String>, hint: impl Into<String>) -> DimaError {
    DimaError::usage(message).with_hint(hint)
}

pub const fn table_of(t: TableArg) -> Table {
    match t {
        TableArg::Positions => Table::Positions,
        TableArg::Variants => Table::Variants,
        TableArg::Metadata => Table::Metadata,
        TableArg::Hcs => Table::Hcs,
    }
}

const fn format_of(f: FormatArg) -> OutputFormat {
    match f {
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Jsonl => OutputFormat::Jsonl,
        FormatArg::Tsv => OutputFormat::Tsv,
    }
}

const fn hcs_format_of(f: HcsFormatArg) -> OutputFormat {
    match f {
        HcsFormatArg::Json => OutputFormat::Json,
        HcsFormatArg::Tsv => OutputFormat::Tsv,
    }
}

/// What a destination's name settles: the format written there and its compression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settled {
    /// The format.
    pub format: OutputFormat,
    /// The compression the name asks for.
    pub compression: OutputCompression,
    /// The name of a file of conserved sequences gives no format, so their documented default
    /// (JSON) was taken ([`warn_assumed_format`]); never for standard output and other
    /// streams, whose names cannot give one.
    pub assumed: bool,
}

/// Reads what the name of a destination of `kind` says, with the format asked for explicitly
/// (`-O`) (D10). Standard output takes the format asked for, else JSON.
///
/// # Errors
/// A usage error (exit 2) when the name asks for something DiMA cannot write (a compression
/// it does not write, two compressions, a name ending in a separator), names an existing
/// folder, contradicts `-O`, or gives no format where one is needed: the results need one (or
/// `-O`); conserved sequences are JSON or TSV; the run report is always JSON.
pub fn settle(kind: Kind, target: &Target, explicit: Option<OutputFormat>) -> Result<Settled> {
    let Target::Path(path) = target else {
        return Ok(Settled {
            format: explicit.unwrap_or(OutputFormat::Json),
            compression: OutputCompression::None,
            assumed: false,
        });
    };
    let named = output::read_name(path).map_err(|e| {
        let (message, hint) = e.describe(path);
        match e {
            output::NameError::Folder => usage(message, file_in_folder(kind, path)),
            _ => usage(message, hint),
        }
    })?;
    if std::fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(usage(
            format!("'{}' is a folder, not a file", path.display()),
            file_in_folder(kind, path),
        ));
    }
    let shown = path.display();
    let mismatch = |f: OutputFormat, e: OutputFormat, extensions: &str| {
        usage(
            format!(
                "-O {} does not match the extension of '{shown}' ({})",
                f.name(),
                e.name()
            ),
            format!("drop -O, or give the file a matching extension ({extensions})"),
        )
    };
    let settled = |format| Settled {
        format,
        compression: named.compression,
        assumed: false,
    };
    match kind {
        Kind::Results => match (explicit, named.format) {
            (Some(f), Some(e)) if f != e => Err(mismatch(f, e, ".json, .jsonl, .tsv")),
            (Some(f), _) | (None, Some(f)) => Ok(settled(f)),
            (None, None) => Err(usage(
                format!("cannot tell the output format from '{shown}'"),
                "use a .json, .jsonl or .tsv extension (optionally .gz or .zst), or -O",
            )),
        },
        Kind::Hcs => match (explicit, named.format) {
            (_, Some(OutputFormat::Jsonl)) => Err(usage(
                format!(
                    "'{shown}' names JSON Lines, but conserved sequences are written as JSON or \
                     TSV"
                ),
                format!(
                    "name it {}",
                    renamed(
                        path,
                        named.format,
                        named.compression,
                        &[".json", ".tsv"],
                        ".hcs"
                    )
                ),
            )),
            (Some(f), Some(e)) if f != e => Err(mismatch(f, e, ".json or .tsv")),
            (Some(f), _) | (None, Some(f)) => Ok(settled(f)),
            (None, None) => Ok(Settled {
                assumed: !is_stream(path),
                ..settled(OutputFormat::Json)
            }),
        },
        Kind::RunReport => match named.format {
            None | Some(OutputFormat::Json) => Ok(settled(OutputFormat::Json)),
            Some(other) => Err(usage(
                format!(
                    "the run report is JSON, but '{shown}' names {}",
                    match other {
                        OutputFormat::Tsv => "a TSV table",
                        OutputFormat::Jsonl | OutputFormat::Json => "JSON Lines",
                    }
                ),
                format!(
                    "name it {}",
                    renamed(
                        path,
                        named.format,
                        named.compression,
                        &[".json"],
                        ".run-report"
                    )
                ),
            )),
        },
        Kind::Table(_) => Ok(settled(OutputFormat::Tsv)),
        // `dima cite` prints on standard output only (settled above).
        Kind::Citation => Err(usage(
            "the citation is printed on standard output",
            "redirect standard output to a file",
        )),
    }
}

/// The hint for a destination of `kind` that names the folder `path`: a file in it, named as
/// that output usually is (`out/` → `out/hcs.json`).
fn file_in_folder(kind: Kind, path: &Path) -> String {
    let example = match kind {
        Kind::Hcs => "hcs.json",
        Kind::RunReport => "run-report.json",
        Kind::Results | Kind::Table(_) | Kind::Citation => "results.json",
    };
    // Components drop the `.` of `x/.`, so the example reads `x/hcs.json`.
    let folder: std::path::PathBuf = path.components().collect();
    format!(
        "give a file name, such as '{}'",
        folder.join(example).display()
    )
}

/// `true` when `path` is written in place rather than replaced (D77): a descriptor such as
/// `/dev/stdout` or `/dev/fd/3`, a device such as `/dev/null`, or a FIFO.
fn is_stream(path: &Path) -> bool {
    dima_core::output::written_in_place(path)
}

/// `path` as written, its compression kept, with each of `extensions` added to a name that
/// names no format (`regions.txt` → `regions.txt.json or regions.txt.tsv (optionally followed
/// by .gz or .zst)`; `mers.hcs` → `mers.hcs.json`), or, in place of the extension of another
/// format, the output's `own` name and each of `extensions` (`run.tsv.gz` →
/// `run.run-report.json.gz`). The suggestion thus never becomes the name the results of the
/// same base usually have (`run.json`); the collision check still compares it with every
/// other output.
fn renamed(
    path: &Path,
    format: Option<OutputFormat>,
    compression: OutputCompression,
    extensions: &[&str],
    own: &str,
) -> String {
    let shown = path.display().to_string();
    // ASCII lower-casing keeps every byte position, so lengths found in `lower` cut `shown`.
    let lower = shown.to_ascii_lowercase();
    let compressed = compression.extension();
    let mut end = shown.len();
    if !compressed.is_empty() && lower.ends_with(compressed) {
        end -= compressed.len();
    }
    if let Some(len) = format.and_then(|f| {
        f.extensions()
            .iter()
            .find(|e| lower[..end].ends_with(*e))
            .map(|e| e.len())
    }) {
        end -= len;
        // Not twice: `x.hcs.jsonl` and `hcs.jsonl` keep their name.
        let base = lower[..end]
            .rsplit(std::path::is_separator)
            .next()
            .unwrap_or_default();
        if !base.ends_with(own) && base != own.trim_start_matches('.') {
            return names(&format!("{}{own}", &shown[..end]), extensions, compressed);
        }
    }
    names(&shown[..end], extensions, compressed)
}

/// `stem` with each of `extensions` and the compression, for a hint.
fn names(stem: &str, extensions: &[&str], compressed: &str) -> String {
    let names: Vec<String> = extensions
        .iter()
        .map(|e| format!("{stem}{e}{compressed}"))
        .collect();
    let names = names.join(" or ");
    if compressed.is_empty() {
        format!("{names} (optionally followed by .gz or .zst)")
    } else {
        names
    }
}

/// Refuses a `--tsv-dir` that exists and is not a folder.
///
/// # Errors
/// A usage error naming it.
pub fn refuse_non_folder(dir: &Path) -> Result<()> {
    match std::fs::metadata(dir) {
        Ok(m) if !m.is_dir() => Err(usage(
            if m.is_file() {
                format!("'{}' is a file, not a folder", dir.display())
            } else {
                format!("'{}' is not a folder", dir.display())
            },
            "give --tsv-dir the name of a folder; it is created if needed",
        )),
        _ => Ok(()),
    }
}

/// Plans the outputs of `analyze` and `view` from their destinations (the run report is
/// planned on its own, [`plan_run_report`]).
///
/// # Errors
/// Usage errors for inconsistent options and names DiMA cannot honour.
pub fn plan(
    args: &OutputArgs,
    destinations: &Destinations,
    stdout_is_terminal: bool,
) -> Result<Plan> {
    let has_results = destinations.files.iter().any(|d| d.kind == Kind::Results);
    if !has_results && args.output_format.is_some() {
        return Err(usage(
            "-O/--output-format applies to -o/--output",
            "add -o PATH (or -o - for standard output)",
        ));
    }
    if !has_results && args.table.is_some() {
        return Err(usage(
            "--table applies to -o with TSV output",
            "add -o PATH.tsv",
        ));
    }
    for dir in &destinations.folders {
        refuse_non_folder(dir)?;
    }
    let mut plan = Plan {
        directories: destinations.folders.clone(),
        ..Plan::default()
    };
    for d in &destinations.files {
        match d.kind {
            Kind::Results => {
                let s = settle(d.kind, &d.target, args.output_format.map(format_of))?;
                let content = match s.format {
                    OutputFormat::Json => Content::Json {
                        pretty: args.pretty || (d.target == Target::Stdout && stdout_is_terminal),
                    },
                    OutputFormat::Jsonl => Content::Jsonl,
                    OutputFormat::Tsv => Content::Tsv(table_of(args.table.ok_or_else(|| {
                        usage(
                            "TSV output holds one table; which one?",
                            "add --table positions|variants|metadata|hcs, or write all four with \
                             --tsv-dir DIR",
                        )
                    })?)),
                };
                if args.table.is_some() && s.format != OutputFormat::Tsv {
                    return Err(usage(
                        "--table applies to TSV output",
                        "add -O tsv or use a .tsv file name",
                    ));
                }
                plan.outputs.push(Planned {
                    target: d.target.clone(),
                    compression: s.compression,
                    content,
                });
            }
            Kind::Table(t) => plan.outputs.push(Planned {
                target: d.target.clone(),
                compression: OutputCompression::None,
                content: Content::Tsv(t),
            }),
            Kind::Hcs => plan.push_hcs(d, None, args.pretty, stdout_is_terminal)?,
            Kind::RunReport | Kind::Citation => {}
        }
    }
    Ok(plan)
}

/// Plans the output of `dima hcs`, in the format of `-O` or of its name.
///
/// # Errors
/// Usage errors for inconsistent options and names DiMA cannot honour.
pub fn plan_hcs(
    destinations: &Destinations,
    format: Option<HcsFormatArg>,
    pretty: bool,
    stdout_is_terminal: bool,
) -> Result<Plan> {
    let mut plan = Plan::default();
    for d in &destinations.files {
        plan.push_hcs(d, format.map(hcs_format_of), pretty, stdout_is_terminal)?;
    }
    Ok(plan)
}

/// Plans the run report: always JSON, compressed as its name asks.
///
/// # Errors
/// A usage error for a name that asks for another format or something DiMA cannot write.
pub fn plan_run_report(d: &Destination) -> Result<Planned> {
    let s = settle(Kind::RunReport, &d.target, None)?;
    Ok(Planned {
        target: d.target.clone(),
        compression: s.compression,
        content: Content::RunReport,
    })
}

/// The folders a run creates for its outputs. Unless the run completes ([`Folders::keep`]),
/// they are removed again when this is dropped — deepest first, and only those still empty —
/// so a failed run leaves nothing behind but a folder that holds its run report or outputs
/// that were complete.
#[derive(Debug, Default)]
pub struct Folders {
    created: Vec<PathBuf>,
    keep: bool,
}

impl Folders {
    /// Creates `dir` and every missing folder above it, remembering each one it created.
    ///
    /// # Errors
    /// An I/O error naming `dir`.
    pub fn create(&mut self, dir: &Path) -> Result<()> {
        let mut missing = Vec::new();
        let mut level = Some(dir);
        while let Some(d) = level {
            if d.as_os_str().is_empty() || d.is_dir() {
                break;
            }
            missing.push(d);
            level = d.parent();
        }
        for d in missing.into_iter().rev() {
            match std::fs::create_dir(d) {
                Ok(()) => self.created.push(d.to_path_buf()),
                // Created meanwhile, or a name such as `x/..`: fine once it is a folder.
                Err(_) if d.is_dir() => {}
                Err(e) => {
                    return Err(DimaError::io(
                        IoAction::CreateDir,
                        IoTarget::Path(dir.to_path_buf()),
                        e,
                    ));
                }
            }
        }
        Ok(())
    }

    /// The run completed: every folder stays.
    pub fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        if !self.keep {
            for d in self.created.iter().rev() {
                // Fails, as it should, unless the folder is still empty.
                let _ = std::fs::remove_dir(d);
            }
        }
    }
}

/// The outputs a command committed, kept apart from the writer, so that the message after an
/// interruption and the run report can tell what is complete once the writer has gone.
#[derive(Debug, Default)]
pub struct Ledger {
    /// Each committed output with its index in the plan.
    committed: RefCell<Vec<(usize, Planned, Written)>>,
    /// The stream being written, as shown to the user, while it is written.
    streaming: RefCell<Option<String>>,
}

impl Ledger {
    fn record(&self, index: usize, planned: &Planned, written: Written) {
        self.committed
            .borrow_mut()
            .push((index, planned.clone(), written));
    }

    /// Every committed output with what it wrote, in the plan's order.
    #[must_use]
    pub fn committed(&self) -> Vec<(Planned, Written)> {
        let mut all = self.committed.borrow().clone();
        all.sort_by_key(|(i, ..)| *i);
        all.into_iter().map(|(_, p, w)| (p, w)).collect()
    }

    /// The message of a command that was interrupted: what is complete, and the stream that
    /// was being written, which may then hold a partial document.
    #[must_use]
    pub fn cancelled_message(&self) -> String {
        let complete: Vec<String> = self
            .committed()
            .iter()
            .map(|(p, _)| shown(&p.target))
            .collect();
        let streaming = self.streaming.borrow().clone();
        let mut message = "cancelled".to_owned();
        if complete.is_empty() && streaming.is_none() {
            message.push_str("; no results were written");
        }
        if !complete.is_empty() {
            message.push_str("; complete: ");
            message.push_str(&complete.join(", "));
        }
        if let Some(stream) = streaming {
            message.push_str(&format!("; {stream} may hold a partial document"));
        }
        message
    }
}

/// A destination as the user wrote it, or `standard output`.
fn shown(target: &Target) -> String {
    match target {
        Target::Stdout => "standard output".to_owned(),
        Target::Path(p) => p.display().to_string(),
    }
}

/// When an output is written: files first, then the streams written in place as they are
/// produced (devices, FIFOs, other descriptors), and standard output last, so a reader of
/// standard output that stops early (`| head`) never costs another output.
fn writing_rank(planned: &Planned) -> u8 {
    match &planned.target {
        Target::Stdout => 2,
        Target::Path(p) if super::destinations::names_stdout(p) => 2,
        Target::Path(p) if is_stream(p) => 1,
        Target::Path(_) => 0,
    }
}

/// The outputs of a plan with their files opened (temporary files created), before the
/// analysis runs: a destination that cannot be written — a missing or read-only folder, a name
/// longer than file systems allow (255 bytes), a symbolic-link loop — stops the run before a
/// long computation. Dropped without
/// [`Opened::write`] (an error or an interruption), it removes its temporary files.
pub struct Opened<'a> {
    plan: &'a Plan,
    sinks: Vec<Option<Sink>>,
}

impl<'a> Opened<'a> {
    /// Creates the folders (through `folders`, which outlives this) and opens every output of
    /// `plan`. Writes stop with [`DimaError::Cancelled`] soon after `cancel` is set.
    ///
    /// # Errors
    /// I/O errors with the destination.
    pub fn open(plan: &'a Plan, folders: &mut Folders, cancel: &Arc<AtomicBool>) -> Result<Self> {
        for dir in &plan.directories {
            folders.create(dir)?;
        }
        let sinks = plan
            .outputs
            .iter()
            .map(|p| {
                Sink::create_cancellable(&p.target, p.compression, Some(Arc::clone(cancel)))
                    .map(Some)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { plan, sinks })
    }

    /// Writes the outputs, recording each commit in `ledger`. Files are written first and
    /// then committed together, so an error or an interruption while writing leaves every
    /// destination as it was ("no results were written"); only the commits (flush, sync,
    /// rename) follow each other without a check. Standard output and other streams come last:
    /// a reader that stops early (`| head`) can never cost a file output.
    ///
    /// # Errors
    /// I/O errors (with the destination), internal errors for non-finite values,
    /// [`DimaError::Cancelled`], and whatever `check` returns.
    pub fn write(
        mut self,
        results: Option<&Results>,
        hcs: Option<&HcsResults>,
        check: &dyn Fn() -> Result<()>,
        ledger: &Ledger,
    ) -> Result<()> {
        let plan = self.plan;
        let (mut streams, files): (Vec<usize>, Vec<usize>) =
            (0..plan.outputs.len()).partition(|&i| writing_rank(&plan.outputs[i]) > 0);
        // A stable sort: the streams keep the plan's order, standard output after the others.
        streams.sort_by_key(|&i| writing_rank(&plan.outputs[i]));
        for &i in &files {
            check()?;
            fill(&plan.outputs[i], self.sink(i)?, results, hcs, None)?;
        }
        check()?;
        for &i in &files {
            let written = self.take(i)?.commit()?;
            ledger.record(i, &plan.outputs[i], written);
        }
        for &i in &streams {
            check()?;
            *ledger.streaming.borrow_mut() = Some(shown(&plan.outputs[i].target));
            fill(&plan.outputs[i], self.sink(i)?, results, hcs, None)?;
            let written = self.take(i)?.commit()?;
            *ledger.streaming.borrow_mut() = None;
            ledger.record(i, &plan.outputs[i], written);
        }
        Ok(())
    }

    fn sink(&mut self, i: usize) -> Result<&mut Sink> {
        self.sinks[i].as_mut().ok_or_else(Self::reused)
    }

    fn take(&mut self, i: usize) -> Result<Sink> {
        self.sinks[i].take().ok_or_else(Self::reused)
    }

    fn reused() -> DimaError {
        DimaError::internal("an output was written twice")
    }
}

/// Treats a reader of standard output that went away (`dima … | head`) as the end of that
/// output, not an error: it has what it wanted, and every file output was committed before
/// (ripgrep's convention). The command then still prints its messages.
///
/// # Errors
/// Any other error of `result`.
pub fn unless_reader_gone(result: Result<()>) -> Result<()> {
    match result {
        Err(e) if e.is_broken_pipe() => Ok(()),
        other => other,
    }
}

/// Writes the run report into its sink, opened before the run (D103), and commits it.
///
/// # Errors
/// I/O errors with the destination.
pub fn write_report(mut sink: Sink, planned: &Planned, report: &RunReport) -> Result<Written> {
    fill(planned, &mut sink, None, None, Some(report))?;
    sink.commit()
}

/// Writes one output into its (uncommitted) sink.
fn fill(
    p: &Planned,
    sink: &mut Sink,
    results: Option<&Results>,
    hcs: Option<&HcsResults>,
    report: Option<&RunReport>,
) -> Result<()> {
    let missing = || DimaError::internal("an output was planned without its data");
    match p.content {
        Content::Json { pretty } => output::write_json(results.ok_or_else(missing)?, pretty, sink)?,
        Content::Jsonl => output::write_jsonl(results.ok_or_else(missing)?, sink)?,
        Content::Tsv(Table::Hcs) => {
            let h = hcs.ok_or_else(missing)?;
            output::check_finite_hcs(h)?;
            // The HCS table needs only the regions; results may be absent (`dima hcs`).
            output::tsv::write_table_hcs(h, &mut *sink).map_err(|e| sink.error(e))?;
        }
        Content::Tsv(t) => output::write_tsv(results.ok_or_else(missing)?, hcs, t, sink)?,
        Content::HcsJson { pretty } => {
            output::write_hcs_json(hcs.ok_or_else(missing)?, pretty, sink)?
        }
        Content::RunReport => {
            let r = report.ok_or_else(missing)?;
            serde_json::to_writer_pretty(&mut *sink, r)
                .map_err(|e| sink.error(std::io::Error::from(e)))?;
            std::io::Write::write_all(&mut *sink, b"\n").map_err(|e| sink.error(e))?;
        }
    }
    Ok(())
}

/// Display name of a target for messages and reports (file name only, or `-`).
#[must_use]
pub fn display_name(t: &Target) -> String {
    match t {
        Target::Stdout => "-".to_owned(),
        Target::Path(p) => p.file_name().map_or_else(
            || p.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(
        output: Option<&str>,
        format: Option<FormatArg>,
        table: Option<TableArg>,
        tsv_dir: Option<&Path>,
    ) -> OutputArgs {
        OutputArgs {
            output: output.map(PathBuf::from),
            output_format: format,
            table,
            tsv_dir: tsv_dir.map(Path::to_path_buf),
            pretty: false,
        }
    }

    /// Plans `a` the way `dima analyze` does, with `hcs` as `--hcs-output`.
    fn planned(a: &OutputArgs, hcs: Option<&Path>, terminal: bool) -> Result<Plan> {
        plan(a, &Destinations::of(a, hcs, None, Some("x")), terminal)
    }

    /// Similarity-weighted entropy is computed only when a planned output holds it: the results
    /// (JSON on stdout or in a file, JSONL) or the positions table, `--tsv-dir`'s included.
    #[test]
    fn plans_that_write_position_values() {
        let dir = Path::new("tables");
        let writes = |a: &OutputArgs, hcs: Option<&Path>| {
            planned(a, hcs, false).unwrap().writes_position_values()
        };
        assert!(writes(&args(None, None, None, None), None));
        assert!(writes(&args(Some("r.json"), None, None, None), None));
        assert!(writes(&args(Some("r.jsonl.gz"), None, None, None), None));
        assert!(writes(&args(None, None, None, Some(dir)), None));
        assert!(writes(
            &args(Some("p.tsv"), None, Some(TableArg::Positions), None),
            None
        ));
        for table in [TableArg::Variants, TableArg::Metadata, TableArg::Hcs] {
            assert!(!writes(&args(Some("t.tsv"), None, Some(table), None), None));
        }
        assert!(!writes(
            &args(None, None, None, None),
            Some(Path::new("hcs.json"))
        ));
    }

    #[test]
    fn default_is_json_on_stdout_pretty_on_terminals() {
        let p = planned(&args(None, None, None, None), None, true).unwrap();
        assert_eq!(
            p.outputs,
            [Planned {
                target: Target::Stdout,
                compression: OutputCompression::None,
                content: Content::Json { pretty: true }
            }]
        );
        let p = planned(&args(None, None, None, None), None, false).unwrap();
        assert_eq!(p.outputs[0].content, Content::Json { pretty: false });
    }

    #[test]
    fn formats_extensions_and_tables() {
        let p = planned(&args(Some("out.jsonl.zst"), None, None, None), None, false).unwrap();
        assert_eq!(
            (p.outputs[0].content, p.outputs[0].compression),
            (Content::Jsonl, OutputCompression::Zstd)
        );
        let e = planned(
            &args(Some("out.json"), Some(FormatArg::Tsv), None, None),
            None,
            false,
        )
        .unwrap_err();
        assert!(e.to_string().contains("does not match the extension"));
        let e = planned(&args(Some("out.tsv"), None, None, None), None, false).unwrap_err();
        assert!(e.hint().unwrap().contains("--table"));
        let p = planned(
            &args(
                Some("-"),
                Some(FormatArg::Tsv),
                Some(TableArg::Variants),
                None,
            ),
            None,
            false,
        )
        .unwrap();
        assert_eq!(p.outputs[0].content, Content::Tsv(Table::Variants));
        let e = planned(&args(Some("out.dat"), None, None, None), None, false).unwrap_err();
        assert!(e.to_string().contains("cannot tell the output format"));
        assert!(
            planned(
                &args(Some("out.json"), None, Some(TableArg::Hcs), None),
                None,
                false
            )
            .is_err()
        );
        // -O and --table need -o when other outputs are named.
        let e = planned(
            &args(None, Some(FormatArg::Json), None, Some(Path::new("t"))),
            None,
            false,
        )
        .unwrap_err();
        assert!(e.to_string().contains("-O/--output-format applies to -o"));
        let e = planned(
            &args(None, None, Some(TableArg::Hcs), None),
            Some(Path::new("h.json")),
            false,
        )
        .unwrap_err();
        assert!(e.to_string().contains("--table applies to -o"));
    }

    #[test]
    fn tsv_dir_and_hcs_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("new/sub");
        let a = args(None, None, None, Some(&d));
        let p = plan(
            &a,
            &Destinations::of(&a, Some(Path::new("h.tsv")), None, Some("MERS spike")),
            false,
        )
        .unwrap();
        assert!(!d.exists(), "created only when writing starts");
        assert_eq!(p.directories, std::slice::from_ref(&d));
        assert_eq!(
            p.outputs.len(),
            5,
            "no stdout JSON when other outputs are requested"
        );
        assert_eq!(
            p.outputs[0].target,
            Target::Path(d.join("MERS_spike.positions.tsv"))
        );
        assert_eq!(p.outputs[4].content, Content::Tsv(Table::Hcs));
        assert!(p.needs_hcs());
        assert!(p.assumed_json.is_empty());
    }

    fn path(p: &str) -> Target {
        Target::Path(PathBuf::from(p))
    }

    fn settled(kind: Kind, name: &str, explicit: Option<OutputFormat>) -> Result<Settled> {
        settle(kind, &path(name), explicit)
    }

    fn refusal(r: Result<Settled>) -> (String, String) {
        let e = r.unwrap_err();
        assert_eq!(e.class().code(), 2, "{e}");
        (e.to_string(), e.hint().unwrap_or_default().to_owned())
    }

    /// Every class of name (B2) for every kind of destination: formats and compressions DiMA
    /// writes are accepted; names DiMA cannot honour are refused alike everywhere.
    #[test]
    fn names_are_read_once_for_every_output() {
        use OutputCompression as C;
        use OutputFormat as F;
        let all = [Kind::Results, Kind::Hcs, Kind::RunReport];
        for kind in all {
            let what = |name: &str| settled(kind, name, None);
            // Compression DiMA does not write, two compressions, a folder's name: refused.
            for name in [
                "x.json.bz2",
                "x.json.xz",
                "x.zip",
                "x.json.gz.zst",
                "x.tgz",
                "out/",
            ] {
                let (message, hint) = refusal(what(name));
                assert!(
                    message.contains(name.trim_end_matches('/')),
                    "{kind:?} {name}: {message}"
                );
                assert!(!hint.is_empty(), "{kind:?} {name}");
            }
            // Even with -O.
            assert!(
                settled(kind, "x.json.bz2", Some(F::Json)).is_err(),
                "{kind:?}"
            );
            // Compression by name.
            assert_eq!(what("x.json.GZ").unwrap().compression, C::Gzip, "{kind:?}");
            assert_eq!(what("x.json.zst").unwrap().compression, C::Zstd, "{kind:?}");
            // Standard output: the format asked for, else JSON, uncompressed.
            assert_eq!(
                settle(kind, &Target::Stdout, None).unwrap(),
                Settled {
                    format: F::Json,
                    compression: C::None,
                    assumed: false
                }
            );
        }
        // The results: the name or -O, which must agree; nothing else is assumed.
        assert_eq!(
            settled(Kind::Results, "x.ndjson", None).unwrap().format,
            F::Jsonl
        );
        assert_eq!(
            settled(Kind::Results, "x.tab", None).unwrap().format,
            F::Tsv
        );
        assert_eq!(
            settled(Kind::Results, "x.txt", Some(F::Json))
                .unwrap()
                .format,
            F::Json
        );
        let (message, _) = refusal(settled(Kind::Results, "x.txt", None));
        assert!(
            message.contains("cannot tell the output format from 'x.txt'"),
            "{message}"
        );
        let (message, _) = refusal(settled(Kind::Results, "x.json", Some(F::Tsv)));
        assert!(
            message.contains("-O tsv does not match the extension of 'x.json' (json)"),
            "{message}"
        );
        // Conserved sequences: JSON or TSV; a file named neither gets JSON, marked assumed.
        assert_eq!(settled(Kind::Hcs, "h.tsv.gz", None).unwrap().format, F::Tsv);
        let s = settled(Kind::Hcs, "regions.txt", None).unwrap();
        assert_eq!((s.format, s.assumed), (F::Json, true));
        let s = settled(Kind::Hcs, "regions.txt", Some(F::Tsv)).unwrap();
        assert_eq!((s.format, s.assumed), (F::Tsv, false), "-O chooses");
        let (message, hint) = refusal(settled(Kind::Hcs, "x.jsonl", None));
        assert_eq!(
            message,
            "'x.jsonl' names JSON Lines, but conserved sequences are written as JSON or TSV"
        );
        assert_eq!(
            hint,
            "name it x.hcs.json or x.hcs.tsv (optionally followed by .gz or .zst)"
        );
        let (message, _) = refusal(settled(Kind::Hcs, "x.json", Some(F::Tsv)));
        assert!(message.contains("-O tsv does not match"), "{message}");
        // The run report: always JSON; a table or JSON Lines name is refused.
        assert_eq!(
            settled(Kind::RunReport, "run.log", None).unwrap().format,
            F::Json
        );
        let (message, hint) = refusal(settled(Kind::RunReport, "run.tsv", None));
        assert_eq!(
            message,
            "the run report is JSON, but 'run.tsv' names a TSV table"
        );
        assert_eq!(
            hint,
            "name it run.run-report.json (optionally followed by .gz or .zst)"
        );
        let (message, hint) = refusal(settled(Kind::RunReport, "run.jsonl.gz", None));
        assert!(message.ends_with("names JSON Lines"), "{message}");
        assert_eq!(hint, "name it run.run-report.json.gz");
    }

    #[test]
    fn streams_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("adir");
        std::fs::create_dir(&folder).unwrap();
        for kind in [Kind::Results, Kind::Hcs, Kind::RunReport] {
            let (message, hint) = refusal(settle(
                kind,
                &Target::Path(folder.clone()),
                Some(OutputFormat::Json),
            ));
            assert!(message.ends_with("is a folder, not a file"), "{message}");
            assert!(hint.starts_with("give a file name, such as"), "{hint}");
        }
        #[cfg(unix)]
        {
            // A stream cannot be named: JSON is written to it without a warning.
            let s = settled(Kind::Hcs, "/dev/null", None).unwrap();
            assert_eq!((s.format, s.assumed), (OutputFormat::Json, false));
            let s = settled(Kind::Hcs, "/dev/stdout", None).unwrap();
            assert!(!s.assumed);
        }
        // A --tsv-dir that is a file.
        let file = dir.path().join("f.json");
        std::fs::write(&file, "{}").unwrap();
        let a = args(None, None, None, Some(&file));
        let e = plan(&a, &Destinations::of(&a, None, None, Some("x")), false).unwrap_err();
        assert!(e.to_string().ends_with("is a file, not a folder"), "{e}");
        assert_eq!(e.class().code(), 2);
    }

    #[test]
    fn renamed_names_keep_the_compression() {
        let r = |p: &str, f, c, e: &[&str]| renamed(Path::new(p), f, c, e, ".hcs");
        use OutputCompression as C;
        // A name of no format is extended: `mers.hcs` must not become `mers.json`, the
        // results' usual name.
        assert_eq!(
            r("regions.txt", None, C::None, &[".json", ".tsv"]),
            "regions.txt.json or regions.txt.tsv (optionally followed by .gz or .zst)"
        );
        assert_eq!(
            r("mers.hcs", None, C::None, &[".json"]),
            "mers.hcs.json (optionally followed by .gz or .zst)"
        );
        assert_eq!(
            r("out/regions.gz", None, C::Gzip, &[".json", ".tsv"]),
            "out/regions.json.gz or out/regions.tsv.gz"
        );
        // An extension of another format gives way to the output's own name: `x.jsonl` next
        // to `-o x.json` must not become `x.json`.
        assert_eq!(
            r(
                "x.JSONL",
                Some(OutputFormat::Jsonl),
                C::None,
                &[".json", ".tsv"]
            ),
            "x.hcs.json or x.hcs.tsv (optionally followed by .gz or .zst)"
        );
        assert_eq!(
            r("run.TSV.zst", Some(OutputFormat::Tsv), C::Zstd, &[".json"]),
            "run.hcs.json.zst"
        );
        // A name that already has the output's own name keeps it once.
        assert_eq!(
            r(
                "out/hcs.jsonl",
                Some(OutputFormat::Jsonl),
                C::Gzip,
                &[".json"]
            ),
            "out/hcs.json.gz"
        );
        assert_eq!(
            r(
                "x.HCS.jsonl",
                Some(OutputFormat::Jsonl),
                C::None,
                &[".json"]
            ),
            "x.HCS.json (optionally followed by .gz or .zst)"
        );
        assert_eq!(
            r(".hidden", None, C::None, &[".json"]),
            ".hidden.json (optionally followed by .gz or .zst)"
        );
    }

    #[test]
    fn assumed_formats_are_listed() {
        let a = args(None, None, None, None);
        let p = plan(
            &a,
            &Destinations::of(&a, Some(Path::new("regions.txt.gz")), None, None),
            false,
        )
        .unwrap();
        assert_eq!(
            p.assumed_json,
            [(PathBuf::from("regions.txt.gz"), OutputCompression::Gzip)]
        );
        assert_eq!(p.outputs[0].compression, OutputCompression::Gzip);
        let p = plan_hcs(
            &Destinations::of_hcs(Some(Path::new("x"))),
            Some(HcsFormatArg::Tsv),
            false,
            false,
        )
        .unwrap();
        assert!(p.assumed_json.is_empty());
        assert_eq!(p.outputs[0].content, Content::Tsv(Table::Hcs));
    }

    #[test]
    fn failed_runs_remove_the_folders_they_created() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("new").join("sub");
        {
            let mut f = Folders::default();
            f.create(&deep).unwrap();
            assert!(deep.is_dir());
        }
        assert!(
            !dir.path().join("new").exists(),
            "every level the run created"
        );
        // A folder that holds a file stays (here the run report), and so do the ones above it.
        let kept = dir.path().join("kept").join("sub");
        {
            let mut f = Folders::default();
            f.create(&kept).unwrap();
            std::fs::write(kept.join("run.json"), "{}").unwrap();
        }
        assert!(kept.join("run.json").is_file());
        // A folder that existed is never removed; a completed run keeps what it created.
        {
            let mut f = Folders::default();
            f.create(dir.path()).unwrap();
            f.create(&dir.path().join("done")).unwrap();
            f.keep();
        }
        assert!(dir.path().join("done").is_dir());
        // A name that cannot be a folder.
        let file = dir.path().join("file");
        std::fs::write(&file, "").unwrap();
        let mut f = Folders::default();
        let e = f.create(&file.join("x")).unwrap_err();
        assert_eq!(e.class().code(), 3, "{e}");
    }

    #[test]
    fn the_ledger_tells_what_is_complete() {
        let ledger = Ledger::default();
        assert_eq!(
            ledger.cancelled_message(),
            "cancelled; no results were written"
        );
        let planned = |t: Target| Planned {
            target: t,
            compression: OutputCompression::None,
            content: Content::Tsv(Table::Positions),
        };
        let w = Written {
            bytes: 1,
            sha256: String::new(),
        };
        ledger.record(1, &planned(path("t/x.variants.tsv")), w.clone());
        ledger.record(0, &planned(path("t/x.positions.tsv")), w);
        assert_eq!(
            ledger.cancelled_message(),
            "cancelled; complete: t/x.positions.tsv, t/x.variants.tsv"
        );
        *ledger.streaming.borrow_mut() = Some("standard output".into());
        assert_eq!(
            ledger.cancelled_message(),
            "cancelled; complete: t/x.positions.tsv, t/x.variants.tsv; standard output may hold a \
             partial document"
        );
        let only = Ledger::default();
        *only.streaming.borrow_mut() = Some("standard output".into());
        assert_eq!(
            only.cancelled_message(),
            "cancelled; standard output may hold a partial document"
        );
    }
}
