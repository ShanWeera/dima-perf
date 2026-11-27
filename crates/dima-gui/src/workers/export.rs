//! Exports: every file is written by the library's atomic [`Sink`] (a temporary file in the
//! destination directory, synced and renamed into place), so a failed or interrupted export
//! never leaves a partial file and never truncates an existing one (G30, E2); a failed export
//! into a folder it created removes the folder again.
//!
//! The names the data exports accept are those `dima analyze` and `dima hcs` accept for the
//! same output, read by the library (`output::read_name`): the format's extensions, plain or
//! compressed with gzip or Zstandard, compressed as the name says, and a name the command line
//! refuses is refused. Any other name gets the export's usual extension (D100).

use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use dima_core::hcs::{self, HcsParams};
use dima_core::methods::{self, CitationFormat, Cited};
use dima_core::model::Results;
use dima_core::output::{self, NameError, OutputCompression, OutputFormat, Sink, Table, Target};
use dima_core::run_report::RunReport;
use dima_core::{DimaError, Result};

use super::{Priority, Worker};
use crate::charts::metric::ChartMetric;
use crate::charts::scene::ChartScene;

/// What can be exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportKind {
    /// The complete results (`.json`, `.json.gz`, `.json.zst`).
    ResultsJson,
    /// One position per line.
    Jsonl,
    /// The four tidy tables into a folder.
    TsvFolder,
    /// Conserved regions as JSON.
    HcsJson,
    /// Conserved regions as a TSV table.
    HcsTsv,
    /// The entropy figure, raster.
    Png,
    /// The entropy figure, vector.
    Svg,
    /// The run report (D103).
    RunReport,
    /// Methods paragraph and numbered references (D89).
    Methods,
    /// The run's references as BibTeX.
    Bibtex,
}

impl ExportKind {
    /// Every kind, in menu order.
    pub const ALL: [Self; 10] = [
        Self::ResultsJson,
        Self::Jsonl,
        Self::TsvFolder,
        Self::HcsJson,
        Self::HcsTsv,
        Self::Png,
        Self::Svg,
        Self::RunReport,
        Self::Methods,
        Self::Bibtex,
    ];

    /// Menu label; a figure is named by the metric the chart shows.
    #[must_use]
    pub const fn label(self, metric: ChartMetric) -> &'static str {
        match (self, metric) {
            (Self::ResultsJson, _) => "Results (JSON)\u{2026}",
            (Self::Jsonl, _) => "Positions (JSON Lines)\u{2026}",
            (Self::TsvFolder, _) => "Tables (TSV folder)\u{2026}",
            (Self::HcsJson, _) => "Conserved regions (JSON)\u{2026}",
            (Self::HcsTsv, _) => "Conserved regions (TSV)\u{2026}",
            (Self::Png, ChartMetric::Entropy) => "Entropy chart (PNG)\u{2026}",
            (Self::Svg, ChartMetric::Entropy) => "Entropy chart (SVG)\u{2026}",
            (Self::Png, ChartMetric::SimilarityWeighted) => {
                "Similarity-weighted entropy chart (PNG)\u{2026}"
            }
            (Self::Svg, ChartMetric::SimilarityWeighted) => {
                "Similarity-weighted entropy chart (SVG)\u{2026}"
            }
            (Self::RunReport, _) => "Run report\u{2026}",
            (Self::Methods, _) => "Methods & references\u{2026}",
            (Self::Bibtex, _) => "References (BibTeX)\u{2026}",
        }
    }

    /// `true` for the chart's figures, which depend on the metric shown.
    #[must_use]
    pub const fn is_figure(self) -> bool {
        matches!(self, Self::Png | Self::Svg)
    }

    /// The default extension (without dot); `None` for a folder.
    #[must_use]
    pub const fn extension(self) -> Option<&'static str> {
        match self {
            Self::ResultsJson | Self::HcsJson | Self::RunReport => Some("json"),
            Self::Jsonl => Some("jsonl"),
            Self::TsvFolder => None,
            Self::HcsTsv => Some("tsv"),
            Self::Png => Some("png"),
            Self::Svg => Some("svg"),
            Self::Methods => Some("txt"),
            Self::Bibtex => Some("bib"),
        }
    }

    /// The data format of the exports that share an output of `dima analyze` or `dima hcs`
    /// (results, positions, conserved regions, run report), whose names are read as the command
    /// line reads them; `None` for the folder, the figures and the texts.
    #[must_use]
    pub const fn format(self) -> Option<OutputFormat> {
        match self {
            Self::ResultsJson | Self::HcsJson | Self::RunReport => Some(OutputFormat::Json),
            Self::Jsonl => Some(OutputFormat::Jsonl),
            Self::HcsTsv => Some(OutputFormat::Tsv),
            Self::TsvFolder | Self::Png | Self::Svg | Self::Methods | Self::Bibtex => None,
        }
    }

    /// The endings a name keeps (without the leading dot): for the data exports every
    /// extension of their format, plain or compressed with gzip or Zstandard (the library's
    /// list, as `dima analyze` accepts them: `json.gz`, `ndjson.zst`, `tab`); for the figures
    /// and texts their one extension; none for the folder.
    #[must_use]
    pub fn accepted(self) -> Vec<String> {
        const COMPRESSIONS: [OutputCompression; 3] = [
            OutputCompression::None,
            OutputCompression::Gzip,
            OutputCompression::Zstd,
        ];
        match self.format() {
            Some(format) => format
                .extensions()
                .iter()
                .flat_map(|e| {
                    COMPRESSIONS.map(|c| format!("{}{}", e.trim_start_matches('.'), c.extension()))
                })
                .collect(),
            None => self.extension().map(str::to_owned).into_iter().collect(),
        }
    }

    /// The default file name for a query name; a figure's names the metric the chart shows.
    #[must_use]
    pub fn default_name(self, query_name: &str, metric: ChartMetric) -> String {
        let stem = dima_core::output::file_stem(query_name);
        match self {
            Self::ResultsJson => format!("{stem}.json"),
            Self::Jsonl => format!("{stem}.jsonl"),
            Self::TsvFolder => format!("{stem}_tables"),
            Self::HcsJson => format!("{stem}.hcs.json"),
            Self::HcsTsv => format!("{stem}.hcs.tsv"),
            Self::Png => format!("{stem}.{}.png", metric.file_part()),
            Self::Svg => format!("{stem}.{}.svg", metric.file_part()),
            Self::RunReport => format!("{stem}.run-report.json"),
            Self::Methods => format!("{stem}.methods.txt"),
            Self::Bibtex => format!("{stem}.references.bib"),
        }
    }

    /// The file an export chosen as `chosen` is written to (D100: "the extension is appended
    /// on save if missing"): a name that ends in one of the export's endings
    /// ([`ExportKind::accepted`], any letter case) is kept; any other name gets the default
    /// extension appended (`results` → `results.json`, `results.gz` → `results.gz.json`). The
    /// folder is kept as chosen.
    ///
    /// # Errors
    /// For the data exports, the [`NameError`] of a name the command line refuses too
    /// (`output::read_name`: a compression DiMA does not write, such as `.bz2`; two
    /// compressions; a name ending in a separator), instead of a name that says something else
    /// than the file holds.
    pub fn destination(self, chosen: PathBuf) -> std::result::Result<PathBuf, NameError> {
        let Some(ext) = self.extension() else {
            return Ok(chosen);
        };
        let kept = match self.format() {
            Some(format) => output::read_name(&chosen)?.format == Some(format),
            None => chosen
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .is_some_and(|n| n.ends_with(&format!(".{ext}"))),
        };
        if kept {
            Ok(chosen)
        } else {
            let mut s = chosen.into_os_string();
            s.push(format!(".{ext}"));
            Ok(PathBuf::from(s))
        }
    }

    /// The compression of the export written to `path`: the one its name asks for, for the
    /// data exports; none for the figures, the texts and the tables.
    ///
    /// # Errors
    /// The name's [`NameError`], as a usage error with its hint, for a name
    /// [`ExportKind::destination`] would not have given.
    fn compression(self, path: &Path) -> Result<OutputCompression> {
        if self.format().is_none() {
            return Ok(OutputCompression::None);
        }
        output::read_name(path).map(|n| n.compression).map_err(|e| {
            let (message, hint) = e.describe(path);
            DimaError::usage(message).with_hint(hint)
        })
    }
}

/// The four tables a folder export writes, in [`Table::ALL`] order: `<name>.positions.tsv`
/// and the others, named after the results' query name as `dima analyze --tsv-dir` names them
/// (`output::file_stem`).
#[must_use]
pub fn table_paths(folder: &Path, query_name: &str) -> Vec<PathBuf> {
    let base = output::file_stem(query_name);
    Table::ALL
        .into_iter()
        .map(|t| folder.join(format!("{base}.{}.tsv", t.name())))
        .collect()
}

/// What writing to a destination would do to a file the app has read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replaces {
    /// It is that file (the library's writer follows symbolic links, so a link to the input is
    /// the input; a name that differs only in letter case or Unicode form is when the file
    /// system says so).
    Certainly,
    /// It may be: the names differ only in letter case or Unicode form, and the file system
    /// cannot say whether it ignores that ([`output::one_file`]); why, for the message.
    Possibly(&'static str),
}

/// Whether writing to `dest` would replace `input`, a file the app has read, or `None`. A hard
/// link to the input is another name of it, which replacing leaves the input's own name
/// intact. A file looked at, never changed.
#[must_use]
pub fn replaces(dest: &Path, input: &Path) -> Option<Replaces> {
    let input = std::fs::canonicalize(input).ok()?;
    let dest = resolved(&output::destination_of(dest));
    if dest == input {
        return Some(Replaces::Certainly);
    }
    if output::folded(&dest) != output::folded(&input) {
        return None;
    }
    match output::one_file(&dest, &input) {
        Some(true) => Some(Replaces::Certainly),
        Some(false) => None,
        // The resolved names, so the reason names what differs, not how they were spelled.
        None => Some(Replaces::Possibly(output::why_one_file(&dest, &input))),
    }
}

/// `path` made absolute, with its deepest existing ancestor resolved by the file system and
/// the rest appended (`.` and `..` resolved by name), so that two spellings of one file
/// resolve alike, also when it does not exist yet.
fn resolved(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let parts: Vec<Component<'_>> = absolute.components().collect();
    for split in (1..=parts.len()).rev() {
        let head: PathBuf = parts[..split].iter().collect();
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

/// Creates `dir` and every missing folder above it, and returns the folders it created,
/// deepest first, for [`remove_folders`] should the export fail.
fn create_folders(dir: &Path) -> Result<Vec<PathBuf>> {
    let missing: Vec<PathBuf> = dir
        .ancestors()
        .take_while(|d| !d.as_os_str().is_empty() && std::fs::symlink_metadata(d).is_err())
        .map(Path::to_path_buf)
        .collect();
    std::fs::create_dir_all(dir).map_err(|e| {
        remove_folders(&missing);
        DimaError::io(
            dima_core::error::IoAction::CreateDir,
            dima_core::error::IoTarget::Path(dir.to_path_buf()),
            e,
        )
    })?;
    Ok(missing)
}

/// Removes the folders a failed export created, deepest first. A folder that holds anything (a
/// table already in place, or a file put there meanwhile) is kept.
fn remove_folders(created: &[PathBuf]) {
    for d in created {
        let _ = std::fs::remove_dir(d);
    }
}

/// The data an export needs, captured when it is requested.
#[derive(Clone, Debug)]
pub enum Payload {
    /// Results-based exports (JSON, JSONL, tables, conserved regions, methods, BibTeX).
    Results {
        /// The results.
        results: Arc<Results>,
        /// Conserved-region parameters as currently shown.
        hcs: HcsParams,
    },
    /// A figure.
    Figure(Box<ChartScene>),
    /// A run report.
    Report(Box<RunReport>),
}

/// One export to perform.
#[derive(Clone, Debug)]
pub struct Job {
    /// What to write.
    pub kind: ExportKind,
    /// The metric the chart showed when the export was chosen (names a figure in messages).
    pub metric: ChartMetric,
    /// Destination (a folder for [`ExportKind::TsvFolder`]).
    pub path: PathBuf,
    /// The data.
    pub payload: Payload,
}

/// A file written by an export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrittenFile {
    /// Where.
    pub path: PathBuf,
    /// The run-report role (`results`, `positions_table`, …); `None` for figures and texts.
    pub role: Option<&'static str>,
    /// Bytes written.
    pub bytes: u64,
    /// SHA-256 of the bytes.
    pub sha256: String,
}

/// The outcome of an export.
pub type Outcome = Result<Vec<WrittenFile>>;

/// Starts an export on a background thread.
#[must_use]
pub fn spawn(job: Job, ctx: Option<egui::Context>) -> Worker<Outcome> {
    Worker::spawn("export", Priority::Background, ctx, move |_| run(&job))
}

/// A written but not yet committed file: dropping it removes its temporary file.
struct Filled {
    path: PathBuf,
    role: Option<&'static str>,
    sink: Sink,
}

impl Filled {
    fn new(
        path: &Path,
        role: Option<&'static str>,
        compression: OutputCompression,
        body: impl FnOnce(&mut Sink) -> Result<()>,
    ) -> Result<Self> {
        let mut sink = Sink::create(&Target::Path(path.to_path_buf()), compression)?;
        body(&mut sink)?;
        Ok(Self {
            path: path.to_path_buf(),
            role,
            sink,
        })
    }

    fn commit(self) -> Result<WrittenFile> {
        let w = self.sink.commit()?;
        Ok(WrittenFile {
            path: self.path,
            role: self.role,
            bytes: w.bytes,
            sha256: w.sha256,
        })
    }
}

fn write_file(
    path: &Path,
    role: Option<&'static str>,
    compression: OutputCompression,
    body: impl FnOnce(&mut Sink) -> Result<()>,
) -> Result<WrittenFile> {
    Filled::new(path, role, compression, body)?.commit()
}

fn text(sink: &mut Sink, s: &str) -> Result<()> {
    sink.write_all(s.as_bytes()).map_err(|e| sink.error(e))
}

/// Performs an export synchronously (the job body).
///
/// # Errors
/// Write errors with the destination; conserved-region errors; a figure without data; a name
/// that asks for what DiMA does not write.
pub fn run(job: &Job) -> Outcome {
    let compression = job.kind.compression(&job.path)?;
    match (&job.payload, job.kind) {
        (Payload::Results { results, .. }, ExportKind::ResultsJson) => Ok(vec![write_file(
            &job.path,
            Some("results"),
            compression,
            |s| output::write_json(results, false, s),
        )?]),
        (Payload::Results { results, .. }, ExportKind::Jsonl) => Ok(vec![write_file(
            &job.path,
            Some("results_jsonl"),
            compression,
            |s| output::write_jsonl(results, s),
        )?]),
        (
            Payload::Results {
                results,
                hcs: params,
            },
            ExportKind::TsvFolder,
        ) => {
            // Before the folder exists: a computation that fails creates nothing.
            let regions = hcs::compute(results, params)?;
            let created = create_folders(&job.path)?;
            // Every table is written before any replaces its destination, as with
            // `dima analyze --tsv-dir`: an error leaves the folder's tables as they were, and
            // removes the folders this export created.
            let written = Table::ALL
                .into_iter()
                .zip(table_paths(&job.path, &results.query_name))
                .map(|(t, path)| {
                    let role = match t {
                        Table::Positions => "positions_table",
                        Table::Variants => "variants_table",
                        Table::Metadata => "metadata_table",
                        Table::Hcs => "hcs_table",
                    };
                    Filled::new(&path, Some(role), OutputCompression::None, |s| {
                        output::write_tsv(results, Some(&regions), t, s)
                    })
                })
                .collect::<Result<Vec<_>>>()
                .and_then(|filled| filled.into_iter().map(Filled::commit).collect());
            if written.is_err() {
                remove_folders(&created);
            }
            written
        }
        (
            Payload::Results {
                results,
                hcs: params,
            },
            ExportKind::HcsJson,
        ) => {
            let regions = hcs::compute(results, params)?;
            Ok(vec![write_file(
                &job.path,
                Some("hcs"),
                compression,
                |s| output::write_hcs_json(&regions, false, s),
            )?])
        }
        (
            Payload::Results {
                results,
                hcs: params,
            },
            ExportKind::HcsTsv,
        ) => {
            let regions = hcs::compute(results, params)?;
            Ok(vec![write_file(
                &job.path,
                Some("hcs_table"),
                compression,
                |s| output::tsv::write_table_hcs(&regions, s).map_err(|e| s.error(e)),
            )?])
        }
        (
            Payload::Results {
                results,
                hcs: params,
            },
            ExportKind::Methods,
        ) => {
            // The conserved regions at the threshold shown are part of the analysis described.
            let regions = hcs::compute(results, params)?;
            let body = methods::methods_text(Cited {
                results: Some(results),
                hcs: Some(&regions),
            })
            .ok_or_else(|| DimaError::internal("a methods text without results"))?;
            Ok(vec![write_file(
                &job.path,
                None,
                OutputCompression::None,
                |s| text(s, &body),
            )?])
        }
        (
            Payload::Results {
                results,
                hcs: params,
            },
            ExportKind::Bibtex,
        ) => {
            let regions = hcs::compute(results, params)?;
            let body = format!(
                "{}\n",
                methods::references(
                    Cited {
                        results: Some(results),
                        hcs: Some(&regions),
                    },
                    CitationFormat::Bibtex
                )
            );
            Ok(vec![write_file(
                &job.path,
                None,
                OutputCompression::None,
                |s| text(s, &body),
            )?])
        }
        (Payload::Figure(scene), ExportKind::Png) => {
            let bytes = scene.to_png().map_err(DimaError::usage)?;
            Ok(vec![write_file(
                &job.path,
                None,
                OutputCompression::None,
                |s| s.write_all(&bytes).map_err(|e| s.error(e)),
            )?])
        }
        (Payload::Figure(scene), ExportKind::Svg) => {
            let svg = scene
                .to_svg()
                .map_err(|e| DimaError::usage(e.to_string()))?;
            Ok(vec![write_file(
                &job.path,
                None,
                OutputCompression::None,
                |s| text(s, &svg),
            )?])
        }
        (Payload::Report(report), ExportKind::RunReport) => {
            let mut json = serde_json::to_string_pretty(report.as_ref())
                .map_err(|e| DimaError::internal(format!("run report serialisation: {e}")))?;
            json.push('\n');
            // Compressed as its name asks (run.json.gz), as `dima analyze --run-report` does.
            Ok(vec![write_file(&job.path, None, compression, |s| {
                text(s, &json)
            })?])
        }
        _ => Err(DimaError::internal(
            "an export was requested without its data",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(kind: ExportKind, path: PathBuf) -> Job {
        Job {
            kind,
            metric: ChartMetric::Entropy,
            path,
            payload: Payload::Results {
                results: crate::tests::fixtures::mers(),
                hcs: HcsParams::default(),
            },
        }
    }

    #[test]
    fn data_exports_match_the_library_writers() {
        let dir = tempfile::tempdir().unwrap();
        let r = crate::tests::fixtures::mers();
        let json = dir.path().join("x.json.gz");
        let w = run(&job(ExportKind::ResultsJson, json.clone())).unwrap();
        assert_eq!(w[0].role, Some("results"));
        assert_eq!(
            *crate::workers::inspect::load_results(
                &json,
                &std::sync::atomic::AtomicBool::new(false)
            )
            .unwrap(),
            *r
        );
        let w = run(&job(ExportKind::TsvFolder, dir.path().join("tables"))).unwrap();
        let names: Vec<String> = w
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "mers_spike_aa.positions.tsv",
                "mers_spike_aa.variants.tsv",
                "mers_spike_aa.metadata.tsv",
                "mers_spike_aa.hcs.tsv"
            ],
            "the CLI's --tsv-dir names"
        );
        for f in &w {
            assert_eq!(std::fs::metadata(&f.path).unwrap().len(), f.bytes);
        }
        let positions = std::fs::read_to_string(&w[0].path).unwrap();
        assert!(positions.starts_with("position\tlow_support\tsupport"));
        assert_eq!(positions.lines().count(), 1 + 1345);
        let hcs = dir.path().join("h.json");
        run(&job(ExportKind::HcsJson, hcs.clone())).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&hcs).unwrap()).unwrap();
        assert_eq!(
            v["region_count"], 44,
            "MERS spike aa at 100% (plan, Verification)"
        );
        let methods = dir.path().join("m.txt");
        run(&job(ExportKind::Methods, methods.clone())).unwrap();
        let m = std::fs::read_to_string(&methods).unwrap();
        assert!(m.contains("Tharanga") && m.contains("References"));
        let bib = dir.path().join("r.bib");
        run(&job(ExportKind::Bibtex, bib.clone())).unwrap();
        assert!(
            std::fs::read_to_string(&bib)
                .unwrap()
                .contains("@article{tharanga2025")
        );
        let jsonl = dir.path().join("p.jsonl");
        run(&job(ExportKind::Jsonl, jsonl.clone())).unwrap();
        assert_eq!(
            std::fs::read_to_string(&jsonl).unwrap().lines().count(),
            1345
        );
    }

    /// The data exports accept the names the command line accepts for the same output, every
    /// extension of the format, plain or compressed, in any letter case, and refuse those it
    /// refuses; other names get the usual extension (D100).
    #[test]
    fn export_names_are_the_command_lines() {
        use ExportKind as K;
        let dest = |k: K, name: &str| k.destination(PathBuf::from(name));
        let kept = |k: K, name: &str| assert_eq!(dest(k, name), Ok(PathBuf::from(name)), "{k:?}");
        for name in ["x.json", "x.json.gz", "x.JSON.zst"] {
            kept(K::ResultsJson, name);
            kept(K::HcsJson, name);
            kept(K::RunReport, name);
        }
        for name in [
            "p.jsonl",
            "p.jsonl.gz",
            "p.ndjson",
            "p.ndjson.gz",
            "p.NDJSON.zst",
        ] {
            kept(K::Jsonl, name);
        }
        for name in ["r.tsv", "r.tsv.zst", "r.tab", "r.tab.gz"] {
            kept(K::HcsTsv, name);
        }
        // Other names get the usual extension, the format's: never a name that misstates
        // the bytes.
        assert_eq!(dest(K::ResultsJson, "x.gz"), Ok(PathBuf::from("x.gz.json")));
        assert_eq!(
            dest(K::ResultsJson, "x.tsv"),
            Ok(PathBuf::from("x.tsv.json"))
        );
        assert_eq!(
            dest(K::RunReport, "run.tsv"),
            Ok(PathBuf::from("run.tsv.json"))
        );
        assert_eq!(dest(K::Jsonl, "p.json"), Ok(PathBuf::from("p.json.jsonl")));
        assert_eq!(dest(K::HcsTsv, "r.txt"), Ok(PathBuf::from("r.txt.tsv")));
        // The names the command line refuses.
        for (k, name) in [
            (K::ResultsJson, "x.json.bz2"),
            (K::HcsTsv, "r.tsv.xz"),
            (K::RunReport, "run.json.zip"),
            (K::Jsonl, "p.jsonl.zstd"),
        ] {
            assert!(
                matches!(dest(k, name), Err(NameError::UnsupportedCompression { .. })),
                "{name}"
            );
        }
        assert!(matches!(
            dest(K::ResultsJson, "x.json.gz.zst"),
            Err(NameError::TwoCompressions { .. })
        ));
        assert_eq!(dest(K::ResultsJson, "out/"), Err(NameError::Folder));
        // The figures and texts keep their own extension only; nothing is refused there.
        assert_eq!(
            dest(K::Svg, "chart.bz2"),
            Ok(PathBuf::from("chart.bz2.svg"))
        );
        assert_eq!(dest(K::Methods, "m.TXT"), Ok(PathBuf::from("m.TXT")));
        // Every accepted ending is one the library reads as the export's format, and the
        // dialog's filter lists them.
        for k in ExportKind::ALL {
            for a in k.accepted() {
                assert_eq!(
                    dest(k, &format!("x.{a}")),
                    Ok(PathBuf::from(format!("x.{a}")))
                );
                if let Some(f) = k.format() {
                    let named = output::read_name(Path::new(&format!("x.{a}"))).unwrap();
                    assert_eq!(named.format, Some(f), "{a}");
                }
            }
        }
        assert_eq!(
            K::Jsonl.accepted(),
            [
                "jsonl",
                "jsonl.gz",
                "jsonl.zst",
                "ndjson",
                "ndjson.gz",
                "ndjson.zst"
            ]
        );
        assert_eq!(K::RunReport.accepted(), ["json", "json.gz", "json.zst"]);
        assert!(K::TsvFolder.accepted().is_empty());
        assert_eq!(K::Png.accepted(), ["png"]);
    }

    /// A run report named .json.gz or .json.zst is written compressed, as the command line
    /// writes one; a results file too.
    #[test]
    fn compressed_names_are_written_compressed() {
        use std::io::Read as _;
        let dir = tempfile::tempdir().unwrap();
        let facts = crate::workers::analysis::RunFacts {
            start_time: std::time::SystemTime::UNIX_EPOCH,
            end_time: std::time::SystemTime::UNIX_EPOCH,
            wall: std::time::Duration::ZERO,
            threads: 1,
            threads_started: Some(1),
            phases: Vec::new(),
            records: 191,
            positions: Some(1345),
            waited_for_input: None,
        };
        let report = crate::report::build(
            &crate::tests::fixtures::mers_path(),
            &dima_core::config::AnalysisParams::default(),
            &facts,
            dima_core::run_report::Status::Completed,
            None,
            Some(&crate::tests::fixtures::mers()),
            &[],
        );
        for (name, magic) in [
            ("run.json.gz", &[0x1F, 0x8B][..]),
            ("run.json.zst", &[0x28, 0xB5, 0x2F, 0xFD][..]),
            ("run.json", b"{\n"),
        ] {
            let path = dir.path().join(name);
            let job = Job {
                kind: ExportKind::RunReport,
                metric: ChartMetric::Entropy,
                path: path.clone(),
                payload: Payload::Report(Box::new(report.clone())),
            };
            run(&job).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(bytes.starts_with(magic), "{name}");
            let mut text = String::new();
            match magic[0] {
                0x1F => {
                    flate2::read::GzDecoder::new(&bytes[..])
                        .read_to_string(&mut text)
                        .unwrap();
                }
                0x28 => text = String::from_utf8(zstd::decode_all(&bytes[..]).unwrap()).unwrap(),
                _ => text = String::from_utf8(bytes).unwrap(),
            }
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["status"], "completed", "{name}");
        }
        // A name the command line refuses is never written, even when asked directly.
        let job = Job {
            kind: ExportKind::RunReport,
            metric: ChartMetric::Entropy,
            path: dir.path().join("run.json.bz2"),
            payload: Payload::Report(Box::new(report)),
        };
        let e = run(&job).unwrap_err();
        assert!(e.to_string().contains("bzip2 compression"), "{e}");
        assert!(!dir.path().join("run.json.bz2").exists());
    }

    /// A folder export that fails removes the folders it created (a table name too long for
    /// the file system), and one whose computation fails creates none; a folder that existed is
    /// kept.
    #[test]
    fn a_failed_folder_export_removes_the_folders_it_created() {
        let dir = tempfile::tempdir().unwrap();
        let mut long = (*crate::tests::fixtures::mers()).clone();
        long.query_name = "q".repeat(300);
        let folder = dir.path().join("new").join("tables");
        let job = |results: Results, path: PathBuf| Job {
            kind: ExportKind::TsvFolder,
            metric: ChartMetric::Entropy,
            path,
            payload: Payload::Results {
                results: Arc::new(results),
                hcs: HcsParams::default(),
            },
        };
        assert!(run(&job(long.clone(), folder.clone())).is_err());
        assert!(!dir.path().join("new").exists(), "both levels removed");
        // Into a folder that existed: it stays.
        std::fs::create_dir(dir.path().join("kept")).unwrap();
        assert!(run(&job(long, dir.path().join("kept"))).is_err());
        assert!(dir.path().join("kept").is_dir());
        // Results whose positions do not start at 1: the conserved regions fail first.
        let mut gap = (*crate::tests::fixtures::mers()).clone();
        gap.results.remove(0);
        assert!(run(&job(gap, folder)).is_err());
        assert!(!dir.path().join("new").exists(), "nothing created");
    }

    /// The files an export must not replace are recognised however their path is spelt, and
    /// through a symbolic link; another file, or a hard link (a second name the replacement
    /// leaves alone), is not refused.
    #[test]
    fn an_export_never_replaces_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.fasta");
        std::fs::write(&input, ">a\nACGT\n").unwrap();
        assert_eq!(replaces(&input, &input), Some(Replaces::Certainly));
        let spelt = dir.path().join("sub").join("..").join(".").join("in.fasta");
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        assert_eq!(replaces(&spelt, &input), Some(Replaces::Certainly));
        assert_eq!(replaces(&dir.path().join("other.json"), &input), None);
        // Letter case: the file system tells. Where `IN.fasta` is not found, it is another file
        // (Linux); where it is, it is the input (macOS by inode; on Windows, where the
        // canonical path has the letter case on disk or no inode is known, certainly or
        // possibly).
        let upper = dir.path().join("IN.fasta");
        let found = replaces(&upper, &input);
        if std::fs::metadata(&upper).is_err() {
            assert_eq!(found, None);
        } else if cfg!(unix) {
            assert_eq!(found, Some(Replaces::Certainly));
        } else {
            assert!(found.is_some(), "{found:?}");
        }
        // A file that is gone protects nothing.
        assert_eq!(replaces(&input, &dir.path().join("gone.fasta")), None);
        #[cfg(unix)]
        {
            let link = dir.path().join("link.json");
            std::os::unix::fs::symlink(&input, &link).unwrap();
            assert_eq!(
                replaces(&link, &input),
                Some(Replaces::Certainly),
                "the writer follows links"
            );
            let hard = dir.path().join("hard.fasta");
            std::fs::hard_link(&input, &hard).unwrap();
            assert_eq!(replaces(&hard, &input), None);
        }
        // The tables a folder export writes.
        assert_eq!(
            table_paths(Path::new("out"), "MERS spike")
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                Path::new("out").join("MERS_spike.positions.tsv"),
                Path::new("out").join("MERS_spike.variants.tsv"),
                Path::new("out").join("MERS_spike.metadata.tsv"),
                Path::new("out").join("MERS_spike.hcs.tsv"),
            ]
            .map(|p| p.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn failed_exports_leave_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("missing-dir").join("x.json");
        let e = run(&job(ExportKind::ResultsJson, target)).unwrap_err();
        assert_eq!(e.class().code(), 3);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let mismatched = Job {
            kind: ExportKind::Png,
            metric: ChartMetric::Entropy,
            path: dir.path().join("x.png"),
            payload: Payload::Results {
                results: crate::tests::fixtures::mers(),
                hcs: HcsParams::default(),
            },
        };
        assert!(run(&mismatched).is_err());
    }

    #[test]
    fn a_failed_table_folder_export_changes_no_table() {
        let dir = tempfile::tempdir().unwrap();
        let results = crate::tests::fixtures::mers();
        let base = dima_core::output::file_stem(&results.query_name);
        let old = dir.path().join(format!("{base}.positions.tsv"));
        std::fs::write(&old, "old").unwrap();
        // The third table cannot be written: its name is taken by a folder.
        std::fs::create_dir(dir.path().join(format!("{base}.metadata.tsv"))).unwrap();
        let e = run(&job(ExportKind::TsvFolder, dir.path().to_path_buf())).unwrap_err();
        assert_eq!(e.class().code(), 3, "{e}");
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "old");
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "no temporary or new files: {names:?}");
    }

    #[test]
    fn names_and_extensions() {
        let k = ExportKind::ResultsJson;
        assert_eq!(
            k.default_name("MERS spike", ChartMetric::SimilarityWeighted),
            "MERS_spike.json",
            "data files do not depend on the chart"
        );
        for (kind, metric, name, label) in [
            (
                ExportKind::Png,
                ChartMetric::Entropy,
                "MERS_spike.entropy.png",
                "Entropy chart (PNG)\u{2026}",
            ),
            (
                ExportKind::Svg,
                ChartMetric::Entropy,
                "MERS_spike.entropy.svg",
                "Entropy chart (SVG)\u{2026}",
            ),
            (
                ExportKind::Png,
                ChartMetric::SimilarityWeighted,
                "MERS_spike.similarity-weighted-entropy.png",
                "Similarity-weighted entropy chart (PNG)\u{2026}",
            ),
            (
                ExportKind::Svg,
                ChartMetric::SimilarityWeighted,
                "MERS_spike.similarity-weighted-entropy.svg",
                "Similarity-weighted entropy chart (SVG)\u{2026}",
            ),
        ] {
            assert_eq!(kind.default_name("MERS spike", metric), name);
            assert_eq!(kind.label(metric), label);
            assert!(kind.is_figure());
        }
        assert_eq!(
            ExportKind::RunReport.label(ChartMetric::SimilarityWeighted),
            "Run report\u{2026}"
        );
        let dest = |k: ExportKind, name: &str| k.destination(PathBuf::from(name));
        assert_eq!(dest(k, "a"), Ok(PathBuf::from("a.json")));
        assert_eq!(dest(k, "a.JSON.ZST"), Ok(PathBuf::from("a.JSON.ZST")));
        assert_eq!(dest(ExportKind::TsvFolder, "d"), Ok(PathBuf::from("d")));
        assert_eq!(
            dest(ExportKind::Png, "f.svg"),
            Ok(PathBuf::from("f.svg.png"))
        );
        assert_eq!(dest(ExportKind::Png, "f.PNG"), Ok(PathBuf::from("f.PNG")));
        for k in ExportKind::ALL {
            for m in ChartMetric::ALL {
                assert!(!k.label(m).is_empty());
            }
            assert_eq!(
                k.is_figure(),
                matches!(k, ExportKind::Png | ExportKind::Svg)
            );
        }
    }
}
