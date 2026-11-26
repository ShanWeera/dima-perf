//! Output planning and writing (D28, D61, D77, D99).
//!
//! All outputs are decided and checked *before* the analysis runs, so a mistyped option never
//! costs a long computation: `-o` writes exactly one stream, a TSV on `-o` needs `--table`, `-O`
//! must agree with the file extension, `--tsv-dir` is created if missing, and two outputs (or an
//! output and the input) can never be the same file. Their files are then opened ([`Opened`])
//! before the analysis too, so a destination that cannot be written is reported at once.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dima_core::model::{HcsResults, Results};
use dima_core::output::{self, OutputCompression, OutputFormat, Sink, Table, Target, Written};
use dima_core::{DimaError, Result};

use super::args::{FormatArg, HcsFormatArg, OutputArgs, TableArg};
use super::ui::{self, Verbosity};

/// One planned output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    /// Destination.
    pub target: Target,
    /// Compression (from the extension).
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

/// Everything to write, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Outputs except the run report, in writing order.
    pub outputs: Vec<Planned>,
    /// The run report, written last.
    pub run_report: Option<Planned>,
    /// Directories to create (`--tsv-dir`) when the outputs are opened; removed again, when
    /// still empty, if the run fails, so a failed run leaves nothing behind.
    pub directories: Vec<PathBuf>,
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

/// Plans the results outputs of `analyze`/`view`.
///
/// # Errors
/// Usage errors for inconsistent options; I/O errors creating `--tsv-dir`.
pub fn plan(
    args: &OutputArgs,
    hcs_output: Option<&Path>,
    run_report: Option<&Path>,
    tsv_base: &str,
    stdout_is_terminal: bool,
) -> Result<Plan> {
    let mut outputs = Vec::new();
    let pretty_default =
        |target: &Target| args.pretty || (*target == Target::Stdout && stdout_is_terminal);
    let explicit = args.output.is_some() || args.tsv_dir.is_some() || hcs_output.is_some();
    if args.output.is_none() && args.output_format.is_some() && explicit {
        return Err(usage(
            "-O/--output-format applies to -o/--output",
            "add -o PATH (or -o - for standard output)",
        ));
    }
    if args.output.is_some() || !explicit {
        let path = args.output.clone().unwrap_or_else(|| PathBuf::from("-"));
        let target = Target::from_arg(&path);
        let (from_ext, compression) = if target == Target::Stdout {
            (None, OutputCompression::None)
        } else {
            output::detect(&path)
        };
        let format = match (args.output_format.map(format_of), from_ext) {
            (Some(f), Some(e)) if f != e => {
                return Err(usage(
                    format!(
                        "-O {} does not match the extension of '{}' ({})",
                        f.name(),
                        path.display(),
                        e.name()
                    ),
                    "drop -O, or give the file a matching extension (.json, .jsonl, .tsv)",
                ));
            }
            (Some(f), _) | (None, Some(f)) => f,
            (None, None) => {
                if target != Target::Stdout {
                    return Err(usage(
                        format!("cannot tell the output format from '{}'", path.display()),
                        "use a .json, .jsonl or .tsv extension (optionally .gz or .zst), or -O",
                    ));
                }
                OutputFormat::Json
            }
        };
        let content = match format {
            OutputFormat::Json => Content::Json {
                pretty: pretty_default(&target),
            },
            OutputFormat::Jsonl => Content::Jsonl,
            OutputFormat::Tsv => {
                let table = args.table.ok_or_else(|| {
                    usage(
                        "TSV output holds one table; which one?",
                        "add --table positions|variants|metadata|hcs, or write all four with --tsv-dir DIR",
                    )
                })?;
                Content::Tsv(table_of(table))
            }
        };
        if args.table.is_some() && format != OutputFormat::Tsv {
            return Err(usage(
                "--table applies to TSV output",
                "add -O tsv or use a .tsv file name",
            ));
        }
        outputs.push(Planned {
            target,
            compression,
            content,
        });
    } else if args.table.is_some() {
        return Err(usage(
            "--table applies to -o with TSV output",
            "add -o PATH.tsv",
        ));
    }
    let mut directories = Vec::new();
    if let Some(dir) = &args.tsv_dir {
        directories.push(dir.clone());
        let base = dima_core::output::file_stem(tsv_base);
        for t in Table::ALL {
            outputs.push(Planned {
                target: Target::Path(dir.join(format!("{base}.{}.tsv", t.name()))),
                compression: OutputCompression::None,
                content: Content::Tsv(t),
            });
        }
    }
    if let Some(path) = hcs_output {
        outputs.push(hcs_planned(path, None, args.pretty, stdout_is_terminal)?);
    }
    let run_report = run_report.map(|p| Planned {
        target: Target::from_arg(p),
        compression: output::detect(p).1,
        content: Content::RunReport,
    });
    Ok(Plan {
        outputs,
        run_report,
        directories,
    })
}

/// Plans an HCS output (`--hcs-output`, or `dima hcs -o`).
///
/// # Errors
/// Usage errors for inconsistent format options.
pub fn hcs_planned(
    path: &Path,
    format: Option<HcsFormatArg>,
    pretty: bool,
    stdout_is_terminal: bool,
) -> Result<Planned> {
    let target = Target::from_arg(path);
    let (from_ext, compression) = if target == Target::Stdout {
        (None, OutputCompression::None)
    } else {
        output::detect(path)
    };
    let from_ext = match from_ext {
        Some(OutputFormat::Jsonl) => {
            return Err(usage(
                "conserved regions are written as JSON or TSV",
                "use a .json or .tsv file name",
            ));
        }
        Some(OutputFormat::Json) => Some(HcsFormatArg::Json),
        Some(OutputFormat::Tsv) => Some(HcsFormatArg::Tsv),
        None => None,
    };
    let fmt = match (format, from_ext) {
        (Some(f), Some(e)) if f != e => {
            return Err(usage(
                format!("-O does not match the extension of '{}'", path.display()),
                "drop -O, or give the file a matching extension",
            ));
        }
        (Some(f), _) | (None, Some(f)) => f,
        (None, None) => HcsFormatArg::Json,
    };
    let content = match fmt {
        HcsFormatArg::Json => Content::HcsJson {
            pretty: pretty || (target == Target::Stdout && stdout_is_terminal),
        },
        HcsFormatArg::Tsv => Content::Tsv(Table::Hcs),
    };
    Ok(Planned {
        target,
        compression,
        content,
    })
}

/// The file a path names, as far as can be told before writing: the file an output would be
/// written to (symbolic links followed as the writer follows them, dangling ones included),
/// resolved as far as it exists.
struct Identity {
    exact: String,
    /// Equal for two paths that may name the same file: `exact` without letter case. File
    /// systems that ignore case (the default on macOS and Windows) store `R.json` and `r.json`
    /// as one file, so they are kept apart on every platform; a false alarm costs a rename, a
    /// missed one would lose an output.
    key: String,
}

impl Identity {
    fn of(path: &Path) -> Self {
        let exact = resolved(&output::destination_of(path))
            .to_string_lossy()
            .into_owned();
        let key = exact.to_lowercase();
        Self { exact, key }
    }

    /// The message and hint for a clash of `self` (given as `path`) with `other`: certain when
    /// both resolve alike, possible when only letter case differs.
    fn clash(&self, other: &Self, certain: String, possible: String, fix: &str) -> DimaError {
        if self.exact == other.exact {
            usage(certain, fix)
        } else {
            usage(
                possible,
                format!(
                    "names that differ only in letter case are one file on macOS and Windows; \
                     {fix}"
                ),
            )
        }
    }
}

/// `path` made absolute, with its deepest existing ancestor canonicalised and the rest
/// appended (`.` and `..` resolved by name): two spellings of a file in a directory that does
/// not exist yet (`newdir/x`, `./newdir/x`) resolve alike.
fn resolved(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let parts: Vec<Component<'_>> = absolute.components().collect();
    for split in (0..=parts.len()).rev() {
        let head: PathBuf = parts[..split].iter().collect();
        if head.as_os_str().is_empty() {
            continue;
        }
        if let Ok(mut out) = std::fs::canonicalize(&head) {
            for c in &parts[split..] {
                match c {
                    Component::ParentDir => {
                        out.pop();
                    }
                    Component::CurDir => {}
                    other => out.push(other.as_os_str()),
                }
            }
            return out;
        }
    }
    absolute
}

/// Rejects two outputs on the same file or stream, and outputs on the input file.
///
/// # Errors
/// A usage error naming the clash.
pub fn check_collisions(plan: &Plan, inputs: &[&Path]) -> Result<()> {
    let mut seen: Vec<(Identity, &str, &Path)> = Vec::new();
    let mut stdout_used = false;
    let all = plan.outputs.iter().chain(plan.run_report.iter());
    for o in all {
        match &o.target {
            Target::Stdout => {
                if stdout_used {
                    return Err(usage(
                        "two outputs would go to standard output",
                        "send at most one output to '-'",
                    ));
                }
                stdout_used = true;
            }
            Target::Path(p) => {
                let id = Identity::of(p);
                let role = o.content.role();
                if let Some((q, other, q_path)) = seen.iter().find(|(q, ..)| q.key == id.key) {
                    return Err(id.clash(
                        q,
                        format!(
                            "'{}' would be written twice ({other} and {role})",
                            p.display()
                        ),
                        format!(
                            "the {role} '{}' may be the same file as the {other} '{}'",
                            p.display(),
                            q_path.display()
                        ),
                        "give every output its own file",
                    ));
                }
                for input in inputs.iter().filter(|i| i.as_os_str() != "-") {
                    let input_id = Identity::of(input);
                    if input_id.key == id.key {
                        return Err(id.clash(
                            &input_id,
                            format!("output '{}' is the input file", p.display()),
                            format!(
                                "output '{}' may be the same file as the input '{}'",
                                p.display(),
                                input.display()
                            ),
                            "choose another output path; the input is never overwritten",
                        ));
                    }
                }
                seen.push((id, role, p));
            }
        }
    }
    Ok(())
}

/// Rejects a run report that would replace the input, another output or a `--tsv-dir`
/// directory (checked before the run, since the report is also written when the run fails
/// before the full collision check).
///
/// # Errors
/// A usage error naming the clash.
pub fn check_run_report(report: &Path, others: &[&Path]) -> Result<()> {
    if report.as_os_str() == "-" {
        return Ok(());
    }
    let id = Identity::of(report);
    for o in others {
        let other = Identity::of(o);
        if other.key == id.key {
            return Err(id.clash(
                &other,
                format!(
                    "--run-report '{}' is also given as '{}'",
                    report.display(),
                    o.display()
                ),
                format!(
                    "--run-report '{}' may be the same file as '{}'",
                    report.display(),
                    o.display()
                ),
                "write the run report to its own file",
            ));
        }
    }
    Ok(())
}

/// The outputs of a plan with their files opened (temporary files created), before the
/// analysis runs: a destination that cannot be written — a missing or read-only directory, a
/// directory given as a file, a name too long for the file system — stops the run before a long
/// computation. Dropped without [`Opened::write`] (an error or an interruption), it removes its
/// temporary files and the `--tsv-dir` directories it created, when still empty.
pub struct Opened<'a> {
    plan: &'a Plan,
    sinks: Vec<Option<Sink>>,
    created: Vec<PathBuf>,
    committed: bool,
}

impl<'a> Opened<'a> {
    /// Creates the directories and opens every output of `plan` (not the run report). Writes
    /// stop with [`DimaError::Cancelled`] soon after `cancel` is set.
    ///
    /// # Errors
    /// I/O errors with the destination.
    pub fn open(plan: &'a Plan, cancel: &Arc<AtomicBool>) -> Result<Self> {
        let mut opened = Self {
            plan,
            sinks: Vec::with_capacity(plan.outputs.len()),
            created: Vec::new(),
            committed: false,
        };
        for dir in &plan.directories {
            if !dir.exists() {
                std::fs::create_dir_all(dir).map_err(|e| {
                    DimaError::io(
                        dima_core::error::IoAction::CreateDir,
                        dima_core::error::IoTarget::Path(dir.clone()),
                        e,
                    )
                })?;
                opened.created.push(dir.clone());
            }
        }
        for p in &plan.outputs {
            let sink =
                Sink::create_cancellable(&p.target, p.compression, Some(Arc::clone(cancel)))?;
            opened.sinks.push(Some(sink));
        }
        Ok(opened)
    }

    /// Writes the outputs. Files are written first and then committed together, so an error or
    /// an interruption while writing leaves every destination as it was ("no results were
    /// written"); only the commits (flush, sync, rename) follow each other without a check.
    /// Standard output comes last, streamed: a reader that stops early (`| head`) can never cost
    /// a file output. Returns what was written, in the plan's order.
    ///
    /// # Errors
    /// I/O errors (with the destination), internal errors for non-finite values,
    /// [`DimaError::Cancelled`], and whatever `check` returns.
    pub fn write(
        mut self,
        results: Option<&Results>,
        hcs: Option<&HcsResults>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<Written>> {
        let plan = self.plan;
        let (streams, files): (Vec<usize>, Vec<usize>) =
            (0..plan.outputs.len()).partition(|&i| plan.outputs[i].target == Target::Stdout);
        let mut written: Vec<Option<Written>> = vec![None; plan.outputs.len()];
        for &i in &files {
            check()?;
            fill(&plan.outputs[i], self.sink(i)?, results, hcs, None)?;
        }
        check()?;
        for &i in &files {
            let sink = self.sinks[i].take().ok_or_else(Self::reused)?;
            written[i] = Some(sink.commit()?);
            self.committed = true;
        }
        for &i in &streams {
            check()?;
            fill(&plan.outputs[i], self.sink(i)?, results, hcs, None)?;
            let sink = self.sinks[i].take().ok_or_else(Self::reused)?;
            written[i] = Some(sink.commit()?);
        }
        written
            .into_iter()
            .map(|w| w.ok_or_else(Self::reused))
            .collect()
    }

    fn sink(&mut self, i: usize) -> Result<&mut Sink> {
        self.sinks[i].as_mut().ok_or_else(Self::reused)
    }

    fn reused() -> DimaError {
        DimaError::internal("an output was written twice")
    }
}

impl Drop for Opened<'_> {
    fn drop(&mut self) {
        // Uncommitted sinks remove their temporary files.
        self.sinks.clear();
        if !self.committed {
            for dir in self.created.iter().rev() {
                // Fails, as it should, unless the directory is still empty.
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
}

/// Writes one output and commits it.
///
/// # Errors
/// I/O errors (with the destination) and internal errors for non-finite values.
pub fn write(
    p: &Planned,
    results: Option<&Results>,
    hcs: Option<&HcsResults>,
    report: Option<&dima_core::run_report::RunReport>,
) -> Result<Written> {
    let mut sink = Sink::create(&p.target, p.compression)?;
    fill(p, &mut sink, results, hcs, report)?;
    sink.commit()
}

/// Writes one output into its (uncommitted) sink.
fn fill(
    p: &Planned,
    sink: &mut Sink,
    results: Option<&Results>,
    hcs: Option<&HcsResults>,
    report: Option<&dima_core::run_report::RunReport>,
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

    /// Similarity-weighted entropy is computed only when a planned output holds it: the results
    /// (JSON on stdout or in a file, JSONL) or the positions table, `--tsv-dir`'s included.
    #[test]
    fn plans_that_write_position_values() {
        let dir = Path::new("tables");
        let writes = |a: &OutputArgs, hcs: Option<&Path>, report: Option<&Path>| {
            plan(a, hcs, report, "x", false)
                .unwrap()
                .writes_position_values()
        };
        assert!(writes(&args(None, None, None, None), None, None));
        assert!(writes(&args(Some("r.json"), None, None, None), None, None));
        assert!(writes(
            &args(Some("r.jsonl.gz"), None, None, None),
            None,
            None
        ));
        assert!(writes(&args(None, None, None, Some(dir)), None, None));
        assert!(writes(
            &args(Some("p.tsv"), None, Some(TableArg::Positions), None),
            None,
            None
        ));
        for table in [TableArg::Variants, TableArg::Metadata, TableArg::Hcs] {
            assert!(!writes(
                &args(Some("t.tsv"), None, Some(table), None),
                None,
                None
            ));
        }
        assert!(!writes(
            &args(None, None, None, None),
            Some(Path::new("hcs.json")),
            None
        ));
        assert!(!writes(
            &args(Some("v.tsv"), None, Some(TableArg::Variants), None),
            None,
            Some(Path::new("report.json"))
        ));
    }

    #[test]
    fn default_is_json_on_stdout_pretty_on_terminals() {
        let p = plan(&args(None, None, None, None), None, None, "x", true).unwrap();
        assert_eq!(
            p.outputs,
            [Planned {
                target: Target::Stdout,
                compression: OutputCompression::None,
                content: Content::Json { pretty: true }
            }]
        );
        let p = plan(&args(None, None, None, None), None, None, "x", false).unwrap();
        assert_eq!(p.outputs[0].content, Content::Json { pretty: false });
    }

    #[test]
    fn formats_extensions_and_tables() {
        let p = plan(
            &args(Some("out.jsonl.zst"), None, None, None),
            None,
            None,
            "x",
            false,
        )
        .unwrap();
        assert_eq!(
            (p.outputs[0].content, p.outputs[0].compression),
            (Content::Jsonl, OutputCompression::Zstd)
        );
        let e = plan(
            &args(Some("out.json"), Some(FormatArg::Tsv), None, None),
            None,
            None,
            "x",
            false,
        )
        .unwrap_err();
        assert!(e.to_string().contains("does not match the extension"));
        let e = plan(
            &args(Some("out.tsv"), None, None, None),
            None,
            None,
            "x",
            false,
        )
        .unwrap_err();
        assert!(e.hint().unwrap().contains("--table"));
        let p = plan(
            &args(
                Some("-"),
                Some(FormatArg::Tsv),
                Some(TableArg::Variants),
                None,
            ),
            None,
            None,
            "x",
            false,
        )
        .unwrap();
        assert_eq!(p.outputs[0].content, Content::Tsv(Table::Variants));
        let e = plan(
            &args(Some("out.dat"), None, None, None),
            None,
            None,
            "x",
            false,
        )
        .unwrap_err();
        assert!(e.to_string().contains("cannot tell the output format"));
        assert!(
            plan(
                &args(Some("out.json"), None, Some(TableArg::Hcs), None),
                None,
                None,
                "x",
                false
            )
            .is_err()
        );
    }

    #[test]
    fn tsv_dir_and_hcs_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("new/sub");
        let p = plan(
            &args(None, None, None, Some(&d)),
            Some(Path::new("h.tsv")),
            None,
            "MERS spike",
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
    }

    #[test]
    fn collisions_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.json");
        let p = plan(
            &args(Some(a.to_str().unwrap()), None, None, None),
            Some(&a),
            None,
            "x",
            false,
        )
        .unwrap();
        assert!(
            check_collisions(&p, &[])
                .unwrap_err()
                .to_string()
                .contains("written twice")
        );
        let input = dir.path().join("in.json");
        std::fs::write(&input, "x").unwrap();
        let p = plan(
            &args(Some(input.to_str().unwrap()), None, None, None),
            None,
            None,
            "x",
            false,
        )
        .unwrap();
        assert!(
            check_collisions(&p, &[&input])
                .unwrap_err()
                .to_string()
                .contains("is the input file")
        );
        let p = plan(
            &args(Some("-"), None, None, None),
            Some(Path::new("-")),
            None,
            "x",
            false,
        )
        .unwrap();
        assert!(check_collisions(&p, &[]).is_err());
        // Different spellings of one file are the same file.
        let p = plan(
            &args(Some(a.to_str().unwrap()), None, None, None),
            None,
            Some(&dir.path().join("./a.json")),
            "x",
            false,
        )
        .unwrap();
        assert!(check_collisions(&p, &[]).is_err());
        // So are names that differ only in case (one file on macOS and Windows).
        let p = plan(
            &args(Some(a.to_str().unwrap()), None, None, None),
            None,
            Some(&dir.path().join("A.JSON")),
            "x",
            false,
        )
        .unwrap();
        let message = check_collisions(&p, &[]).unwrap_err().to_string();
        assert!(message.contains("may be the same file"), "{message}");
        // An existing file reached with other letter case is certainly the same file where the
        // file system ignores case (macOS, Windows), and possibly the same file elsewhere.
        std::fs::write(dir.path().join("case-probe"), "").unwrap();
        let ignores_case = dir.path().join("CASE-PROBE").exists();
        let message = check_run_report(&dir.path().join("In.Json"), &[&input])
            .unwrap_err()
            .to_string();
        let expected = if ignores_case {
            "is also given as"
        } else {
            "may be the same file"
        };
        assert!(message.contains(expected), "{message}");
        assert!(
            check_run_report(&input, &[&input])
                .unwrap_err()
                .to_string()
                .contains("is also given as")
        );
    }
}
