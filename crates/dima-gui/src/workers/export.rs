//! Exports: every file is written by the library's atomic [`Sink`] (a temporary file in the
//! destination directory, synced and renamed into place), so a failed or interrupted export
//! never leaves a partial file and never truncates an existing one (G30, E2).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dima_core::hcs::{self, HcsParams};
use dima_core::methods::{self, CitationFormat, Cited};
use dima_core::model::Results;
use dima_core::output::{self, OutputCompression, Sink, Table, Target};
use dima_core::run_report::RunReport;
use dima_core::{DimaError, Result};

use super::{Priority, Worker};
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

    /// Menu label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ResultsJson => "Results (JSON)\u{2026}",
            Self::Jsonl => "Positions (JSON Lines)\u{2026}",
            Self::TsvFolder => "Tables (TSV folder)\u{2026}",
            Self::HcsJson => "Conserved regions (JSON)\u{2026}",
            Self::HcsTsv => "Conserved regions (TSV)\u{2026}",
            Self::Png => "Entropy chart (PNG)\u{2026}",
            Self::Svg => "Entropy chart (SVG)\u{2026}",
            Self::RunReport => "Run report\u{2026}",
            Self::Methods => "Methods & references\u{2026}",
            Self::Bibtex => "References (BibTeX)\u{2026}",
        }
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

    /// Extensions accepted in the save dialog (compressed variants included).
    #[must_use]
    pub fn accepted(self) -> &'static [&'static str] {
        match self {
            Self::ResultsJson => &["json", "json.gz", "json.zst"],
            Self::Jsonl => &["jsonl", "jsonl.gz", "jsonl.zst", "ndjson"],
            Self::HcsJson => &["json", "json.gz", "json.zst"],
            Self::HcsTsv => &["tsv", "tsv.gz", "tsv.zst"],
            Self::TsvFolder => &[],
            Self::Png => &["png"],
            Self::Svg => &["svg"],
            Self::RunReport => &["json"],
            Self::Methods => &["txt"],
            Self::Bibtex => &["bib"],
        }
    }

    /// The default file name for a query name.
    #[must_use]
    pub fn default_name(self, query_name: &str) -> String {
        let stem = dima_core::output::file_stem(query_name);
        match self {
            Self::ResultsJson => format!("{stem}.json"),
            Self::Jsonl => format!("{stem}.jsonl"),
            Self::TsvFolder => format!("{stem}_tables"),
            Self::HcsJson => format!("{stem}.hcs.json"),
            Self::HcsTsv => format!("{stem}.hcs.tsv"),
            Self::Png => format!("{stem}.entropy.png"),
            Self::Svg => format!("{stem}.entropy.svg"),
            Self::RunReport => format!("{stem}.run-report.json"),
            Self::Methods => format!("{stem}.methods.txt"),
            Self::Bibtex => format!("{stem}.references.bib"),
        }
    }

    /// Appends the default extension when the chosen name has none of the accepted ones
    /// (D100: "the extension is appended on save if missing").
    #[must_use]
    pub fn with_extension(self, path: PathBuf) -> PathBuf {
        let Some(ext) = self.extension() else {
            return path;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if self
            .accepted()
            .iter()
            .any(|a| name.ends_with(&format!(".{a}")))
        {
            path
        } else {
            let mut s = path.into_os_string();
            s.push(format!(".{ext}"));
            PathBuf::from(s)
        }
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
/// Write errors with the destination; conserved-region errors; a figure without data.
pub fn run(job: &Job) -> Outcome {
    let compression = output::detect(&job.path).1;
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
            std::fs::create_dir_all(&job.path).map_err(|e| {
                DimaError::io(
                    dima_core::error::IoAction::CreateDir,
                    dima_core::error::IoTarget::Path(job.path.clone()),
                    e,
                )
            })?;
            let regions = hcs::compute(results, params)?;
            let base = dima_core::output::file_stem(&results.query_name);
            // Every table is written before any replaces its destination, as with
            // `dima analyze --tsv-dir`: an error leaves the folder's tables as they were.
            let filled = Table::ALL
                .into_iter()
                .map(|t| {
                    let path = job.path.join(format!("{base}.{}.tsv", t.name()));
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
                .collect::<Result<Vec<_>>>()?;
            filled.into_iter().map(Filled::commit).collect()
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
            Ok(vec![write_file(
                &job.path,
                None,
                OutputCompression::None,
                |s| text(s, &json),
            )?])
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

    #[test]
    fn failed_exports_leave_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("missing-dir").join("x.json");
        let e = run(&job(ExportKind::ResultsJson, target)).unwrap_err();
        assert_eq!(e.class().code(), 3);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let mismatched = Job {
            kind: ExportKind::Png,
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
        assert_eq!(k.default_name("MERS spike"), "MERS_spike.json");
        assert_eq!(
            k.with_extension(PathBuf::from("a")),
            PathBuf::from("a.json")
        );
        assert_eq!(
            k.with_extension(PathBuf::from("a.JSON.ZST")),
            PathBuf::from("a.JSON.ZST")
        );
        assert_eq!(
            ExportKind::TsvFolder.with_extension(PathBuf::from("d")),
            PathBuf::from("d")
        );
        assert_eq!(
            ExportKind::Png.with_extension(PathBuf::from("f.svg")),
            PathBuf::from("f.svg.png")
        );
        for k in ExportKind::ALL {
            assert!(!k.label().is_empty());
        }
    }
}
