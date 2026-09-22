//! Off-thread file I/O: `.dima` import and every result export.
//!
//! These were previously performed inline on the render thread, which froze the
//! window for the duration of the operation — noticeable for large binary
//! imports and for the rasterised chart export. Moving them here keeps the UI
//! responsive and lets the app show progress and report failures as toasts.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dima_lib::Results;

use super::Worker;
use crate::charts::scene::ChartScene;

/// Result of importing a `.dima` file.
///
/// Boxed to keep the channel message small; `Results` is a large struct.
pub type ImportOutcome = Result<Box<Results>, String>;

/// Convert a path to the `&str` the binary-format API requires.
///
/// Deliberately *not* `to_string_lossy`: that would silently replace invalid
/// sequences with U+FFFD and operate on a different path than the user chose.
fn path_as_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "File path contains non-UTF-8 characters and cannot be used.".to_string())
}

/// Spawn a background import of a `.dima` binary results file.
pub fn spawn_import(ctx: Option<egui::Context>, path: PathBuf) -> Worker<ImportOutcome> {
    Worker::spawn(ctx, move |_cancel| {
        // Reading a .dima is a single streaming decode with no natural
        // cancellation point, so the token is intentionally unused; abandoning
        // the worker simply drops the result.
        let path_str = path_as_str(&path)?;
        match Results::from_binary(path_str.to_string()) {
            Ok(results) => Ok(Box::new(results)),
            Err(e) => Err(format!("Failed to load .dima file: {e}")),
        }
    })
}

/// A file format the workspace can export to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Full results as JSON (same writer as the CLI).
    Json,
    /// 17-column, vDiveR-aligned TSV (same writer as the CLI).
    Tsv,
    /// Compact binary results, re-importable by this app and the CLI.
    Dima,
    /// Entropy figure as a raster image.
    Png,
    /// Entropy figure as vector art, for publication figures.
    Svg,
}

impl ExportFormat {
    /// Menu label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Tsv => "TSV",
            Self::Dima => "DiMA binary",
            Self::Png => "Chart PNG",
            Self::Svg => "Chart SVG",
        }
    }

    /// One-line explanation shown as a tooltip.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Json => "Complete results as JSON — identical to `dima analyze -O json`.",
            Self::Tsv => "17-column tab-separated table (vDiveR-aligned) for R/Python workflows.",
            Self::Dima => {
                "Compact binary results. Re-import instantly without re-running the analysis."
            }
            Self::Png => "Entropy chart as a raster image, for slides and quick sharing.",
            Self::Svg => "Entropy chart as scalable vector art, for publication figures.",
        }
    }

    /// File extension (without the dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Tsv => "tsv",
            Self::Dima => "dima",
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }

    /// Whether this format renders the chart rather than the result tables.
    pub fn is_figure(&self) -> bool {
        matches!(self, Self::Png | Self::Svg)
    }

    /// Every format, in menu order.
    pub fn all() -> [Self; 5] {
        [Self::Json, Self::Tsv, Self::Dima, Self::Png, Self::Svg]
    }
}

/// A fully-specified export job.
pub struct ExportRequest {
    pub format: ExportFormat,
    pub path: PathBuf,
    pub results: Arc<Results>,
    /// Pre-built figure description; required for [`ExportFormat::is_figure`].
    ///
    /// Built on the UI thread (it is just a point list) so the worker needs no
    /// access to filter or theme state.
    pub scene: Option<ChartScene>,
}

/// Result of an export: the path written, or a user-facing error message.
pub type ExportOutcome = Result<PathBuf, String>;

/// Spawn a background export.
pub fn spawn_export(ctx: Option<egui::Context>, request: ExportRequest) -> Worker<ExportOutcome> {
    Worker::spawn(ctx, move |_cancel| run_export(request))
}

/// Perform an export synchronously. Separated from spawning so it is testable.
pub fn run_export(request: ExportRequest) -> ExportOutcome {
    let ExportRequest {
        format,
        path,
        results,
        scene,
    } = request;

    match format {
        // JSON/TSV go through the shared CLI writer, which performs an atomic
        // write (temp file + rename) so a failure cannot leave a partial file.
        ExportFormat::Json | ExportFormat::Tsv => {
            let output_type = if format == ExportFormat::Json {
                dima_lib::OutputType::Json
            } else {
                dima_lib::OutputType::Tsv
            };
            dima_lib::write_results_to_output(&results, Some(&path), output_type, false)
                .map_err(|e| format!("Failed to write {}: {e}", format.label()))?;
        }
        ExportFormat::Dima => {
            let path_str = path_as_str(&path)?;
            dima_lib::BinaryFormat::write_to_file(
                &results,
                path_str,
                Some(dima_lib::BinaryFormatConfig::default()),
            )
            .map_err(|e| format!("Failed to write .dima: {e}"))?;
        }
        ExportFormat::Png => {
            let scene = scene.ok_or_else(|| "No chart data to export.".to_string())?;
            let bytes = scene.to_png_bytes()?;
            std::fs::write(&path, bytes).map_err(|e| format!("Failed to write PNG: {e}"))?;
        }
        ExportFormat::Svg => {
            let scene = scene.ok_or_else(|| "No chart data to export.".to_string())?;
            let svg = scene.to_svg().map_err(|e| e.to_string())?;
            std::fs::write(&path, svg).map_err(|e| format!("Failed to write SVG: {e}"))?;
        }
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::scene::SceneStyle;
    use dima_lib::{HighestEntropy, Position, Variant};

    fn test_results() -> Arc<Results> {
        let positions = (1..=5)
            .map(|i| Position {
                position: i,
                low_support: None,
                entropy: i as f64 * 0.25,
                support: 100,
                distinct_variants_count: 1,
                distinct_variants_incidence: 10.0,
                total_variants_incidence: 20.0,
                diversity_motifs: Some(vec![Variant {
                    sequence: "ACDEF".to_string(),
                    count: 100,
                    incidence: 100.0,
                    motif_short: Some("I".to_string()),
                    motif_long: Some("Index".to_string()),
                    metadata: None,
                }]),
            })
            .collect();

        Arc::new(Results {
            sequence_count: 100,
            support_threshold: 30,
            low_support_count: 0,
            query_name: "unit_test".to_string(),
            kmer_length: 5,
            highest_entropy: HighestEntropy {
                position: 5,
                entropy: 1.25,
            },
            average_entropy: 0.75,
            results: positions,
        })
    }

    fn test_scene() -> ChartScene {
        ChartScene {
            title: "unit_test".to_string(),
            points: (1..=5).map(|i| (i as f64, i as f64 * 0.25)).collect(),
            average_entropy: 0.75,
            width: 400,
            height: 300,
            style: SceneStyle::publication(),
        }
    }

    fn export_to_temp(format: ExportFormat) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dima_export_test_{}_{:?}",
            std::process::id(),
            format
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("out.{}", format.extension()));
        let outcome = run_export(ExportRequest {
            format,
            path: path.clone(),
            results: test_results(),
            scene: format.is_figure().then(test_scene),
        });
        assert!(outcome.is_ok(), "{format:?} export failed: {outcome:?}");
        path
    }

    #[test]
    fn exports_every_format_to_a_non_empty_file() {
        for format in ExportFormat::all() {
            let path = export_to_temp(format);
            let meta = std::fs::metadata(&path).expect("exported file should exist");
            assert!(meta.len() > 0, "{format:?} produced an empty file");
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn tsv_export_has_the_17_column_header() {
        let path = export_to_temp(ExportFormat::Tsv);
        let text = std::fs::read_to_string(&path).unwrap();
        let header = text.lines().next().unwrap();
        assert_eq!(header.split('\t').count(), 17);
        assert!(header.starts_with("position\tentropy\tsupport"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dima_export_round_trips_through_import() {
        let path = export_to_temp(ExportFormat::Dima);
        let reloaded = Results::from_binary(path.to_str().unwrap().to_string())
            .expect("exported .dima must be re-importable");
        assert_eq!(reloaded.query_name, "unit_test");
        assert_eq!(reloaded.results.len(), 5);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn figure_export_without_a_scene_is_rejected_cleanly() {
        // Must surface a message rather than panicking.
        for format in [ExportFormat::Png, ExportFormat::Svg] {
            let outcome = run_export(ExportRequest {
                format,
                path: std::env::temp_dir().join("never_written"),
                results: test_results(),
                scene: None,
            });
            assert!(outcome.is_err());
        }
    }

    #[test]
    fn format_metadata_is_consistent() {
        for format in ExportFormat::all() {
            assert!(!format.label().is_empty());
            assert!(!format.description().is_empty());
            assert!(!format.extension().is_empty());
        }
        assert!(ExportFormat::Png.is_figure());
        assert!(ExportFormat::Svg.is_figure());
        assert!(!ExportFormat::Json.is_figure());
    }
}
