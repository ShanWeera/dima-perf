//! Background diversity analysis.
//!
//! Runs `dima_lib::analyze` off the render thread with cooperative cancellation
//! and two progress counters, one per pipeline phase, so the UI can show honest
//! determinate progress instead of an opaque spinner.

use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use dima_lib::Results;

use super::Worker;
use crate::state::AnalysisConfigUi;

/// Result of a completed analysis run.
pub enum AnalysisOutcome {
    Success {
        // Boxed: `Results` is large and is moved across a channel.
        results: Box<Results>,
        validation_stats: Option<dima_lib::ValidationStats>,
        perf_report: dima_lib::PerfReport,
    },
    Error(String),
    Cancelled,
}

/// Live progress counters shared with the running job.
///
/// `dima_lib` runs two sequential parallel passes over the same position set:
/// entropy computation, then position building (variant decoding and metadata
/// aggregation). Each pass increments its own counter, so the UI can report
/// "phase 1 of 2" / "phase 2 of 2" with real numbers rather than showing a
/// completed bar while the second, often longer, pass is still running.
#[derive(Clone)]
pub struct AnalysisProgress {
    entropy: Arc<AtomicUsize>,
    build: Arc<AtomicUsize>,
    /// Expected positions per pass, derived from the validation scan.
    /// Zero when unknown, in which case progress is reported as indeterminate.
    total_positions: usize,
}

/// Which phase the analysis is currently in, with its completion counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisPhase {
    /// No position has been processed yet: reading and encoding sequences.
    Reading,
    /// Phase 1 of 2 — entropy computation.
    ComputingEntropy { done: usize, total: usize },
    /// Phase 2 of 2 — building positions (variants + metadata).
    BuildingPositions { done: usize, total: usize },
    /// Total position count is unknown, so no determinate bar can be shown.
    Indeterminate,
}

impl AnalysisProgress {
    fn new(total_positions: usize) -> Self {
        Self {
            entropy: Arc::new(AtomicUsize::new(0)),
            build: Arc::new(AtomicUsize::new(0)),
            total_positions,
        }
    }

    /// Classify the current phase from the two counters.
    ///
    /// Reads are `Relaxed`: the counters are monotonic and only drive a progress
    /// display, so eventual visibility is sufficient and no ordering is implied.
    pub fn phase(&self) -> AnalysisPhase {
        use std::sync::atomic::Ordering::Relaxed;

        let total = self.total_positions;
        if total == 0 {
            return AnalysisPhase::Indeterminate;
        }

        let build = self.build.load(Relaxed);
        if build > 0 {
            return AnalysisPhase::BuildingPositions {
                done: build.min(total),
                total,
            };
        }

        let entropy = self.entropy.load(Relaxed);
        if entropy == 0 {
            AnalysisPhase::Reading
        } else {
            AnalysisPhase::ComputingEntropy {
                done: entropy.min(total),
                total,
            }
        }
    }

    /// Overall completion in `0.0..=1.0` across both counted phases.
    ///
    /// `None` means "not measurable yet", which the UI must render as an
    /// animated indeterminate indicator rather than an empty bar:
    ///
    /// * before the entropy pass starts, the analysis is reading, parsing and
    ///   k-mer-encoding the alignment. That stage reports no progress and, on a
    ///   large input, is a substantial share of the total run — showing a
    ///   stationary 0% bar there makes a working analysis look hung.
    /// * `total_positions` can be unknown (no validation scan), in which case no
    ///   honest percentage exists at all.
    pub fn fraction(&self) -> Option<f32> {
        use std::sync::atomic::Ordering::Relaxed;

        let total = self.total_positions;
        if total == 0 {
            return None;
        }
        let entropy = self.entropy.load(Relaxed).min(total);
        let build = self.build.load(Relaxed).min(total);
        if entropy == 0 && build == 0 {
            return None;
        }
        Some(((entropy + build) as f32 / (2.0 * total as f32)).clamp(0.0, 1.0))
    }
}

/// A running analysis: the worker handle plus its progress counters.
pub struct AnalysisJob {
    pub worker: Worker<AnalysisOutcome>,
    pub progress: AnalysisProgress,
    /// When the job was spawned, used to show elapsed time.
    started: std::time::Instant,
}

impl AnalysisJob {
    pub fn cancel(&self) {
        self.worker.cancel();
    }

    /// Wall-clock time since the analysis started.
    ///
    /// Displayed during the unmeasurable reading phase so the user can see the
    /// run is alive even while no percentage can be reported.
    pub fn elapsed(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

/// Format a duration compactly for the progress readout (e.g. `1m 04s`).
pub fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Compute how many k-mer positions an alignment of `alignment_length` yields.
///
/// Returns 0 when the alignment is shorter than the window, which is also the
/// "unknown" sentinel that makes progress indeterminate.
pub fn expected_positions(alignment_length: Option<usize>, kmer_length: usize) -> usize {
    alignment_length
        .filter(|&len| len >= kmer_length && kmer_length > 0)
        .map(|len| len - kmer_length + 1)
        .unwrap_or(0)
}

/// Spawn an analysis of `file_path` using `config`.
///
/// `total_positions` should come from [`expected_positions`] using the
/// validation scan's alignment length.
pub fn spawn(
    ctx: Option<egui::Context>,
    file_path: PathBuf,
    config: AnalysisConfigUi,
    total_positions: usize,
) -> AnalysisJob {
    let progress = AnalysisProgress::new(total_positions);
    let entropy_counter = Arc::clone(&progress.entropy);
    let build_counter = Arc::clone(&progress.build);

    let worker = Worker::spawn(ctx, move |cancel| {
        let analysis_config = dima_lib::AnalysisConfig::new()
            .with_validation_mode(config.validation_mode)
            .with_allow_lowercase(config.allow_lowercase)
            .with_report_invalid(config.report_invalid)
            .with_cancel_token(cancel)
            .with_progress_counter(entropy_counter)
            .with_build_progress_counter(build_counter);

        // An empty query name falls back to the file stem, matching CLI behaviour.
        let query_name = if config.query_name.is_empty() {
            file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            config.query_name.clone()
        };

        match dima_lib::analyze(
            dima_lib::InputSource::File(file_path),
            config.kmer_length,
            config.support_threshold,
            query_name,
            config.header_format.clone(),
            config.alphabet.lib_name(),
            config.header_fillna.clone(),
            config.metadata_fields_arg(),
            Some(analysis_config),
        ) {
            Ok((results, validation_stats, perf_report)) => AnalysisOutcome::Success {
                results: Box::new(results),
                validation_stats,
                perf_report,
            },
            Err(dima_lib::AnalysisError::Cancelled) => AnalysisOutcome::Cancelled,
            Err(e) => AnalysisOutcome::Error(e.to_string()),
        }
    });

    AnalysisJob {
        worker,
        progress,
        started: std::time::Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering::Relaxed;

    #[test]
    fn expected_positions_uses_sliding_window_formula() {
        assert_eq!(expected_positions(Some(100), 9), 92);
        assert_eq!(expected_positions(Some(9), 9), 1);
    }

    #[test]
    fn expected_positions_is_zero_when_unknown_or_too_short() {
        assert_eq!(expected_positions(None, 9), 0);
        // Alignment shorter than the window yields no positions.
        assert_eq!(expected_positions(Some(8), 9), 0);
        // Degenerate k guards against underflow.
        assert_eq!(expected_positions(Some(100), 0), 0);
    }

    #[test]
    fn phase_progresses_reading_then_entropy_then_build() {
        let p = AnalysisProgress::new(10);
        assert_eq!(p.phase(), AnalysisPhase::Reading);

        p.entropy.store(4, Relaxed);
        assert_eq!(
            p.phase(),
            AnalysisPhase::ComputingEntropy { done: 4, total: 10 }
        );

        p.entropy.store(10, Relaxed);
        p.build.store(3, Relaxed);
        assert_eq!(
            p.phase(),
            AnalysisPhase::BuildingPositions { done: 3, total: 10 }
        );
    }

    #[test]
    fn phase_is_indeterminate_without_a_total() {
        let p = AnalysisProgress::new(0);
        assert_eq!(p.phase(), AnalysisPhase::Indeterminate);
        assert_eq!(p.fraction(), None);
    }

    #[test]
    fn fraction_is_indeterminate_until_the_first_pass_starts() {
        // Before any position is counted the run is reading/encoding, which
        // reports nothing. Returning Some(0.0) here would render a stationary
        // bar that is indistinguishable from a hang.
        let p = AnalysisProgress::new(10);
        assert_eq!(p.fraction(), None);

        p.entropy.store(1, Relaxed);
        assert!(p.fraction().is_some());
    }

    #[test]
    fn format_elapsed_is_compact_at_every_scale() {
        use std::time::Duration;
        assert_eq!(format_elapsed(Duration::from_secs(5)), "5s");
        assert_eq!(format_elapsed(Duration::from_secs(64)), "1m 04s");
        assert_eq!(
            format_elapsed(Duration::from_secs(3 * 3600 + 5 * 60)),
            "3h 05m"
        );
    }

    #[test]
    fn fraction_spans_both_phases_and_is_clamped() {
        let p = AnalysisProgress::new(10);

        p.entropy.store(10, Relaxed);
        assert_eq!(p.fraction(), Some(0.5));

        p.build.store(10, Relaxed);
        assert_eq!(p.fraction(), Some(1.0));

        // Counters can overshoot if the library processes more units than the
        // validation scan predicted; the display must never exceed 100%.
        p.build.store(999, Relaxed);
        assert_eq!(p.fraction(), Some(1.0));
    }
}
