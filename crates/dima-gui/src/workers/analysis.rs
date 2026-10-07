//! The analysis job: `dima_core::analyze` on a worker thread, with cancellation and progress.
//!
//! Before reading, the input's size and modification time are compared with those seen by the
//! pre-check (G65): a file that changed in between is re-inspected instead of analysed, so the
//! settings the user chose always describe the data that run.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use dima_core::DimaError;
use dima_core::analysis::Analysis;
use dima_core::config::AnalysisParams;
use dima_core::control::{Phase, ProbeFacts, Progress, ProgressSnapshot, RunControl};
use dima_core::input::Source;

use super::{Priority, Worker};

/// Size and modification time of a file, as seen by the pre-check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    /// Size in bytes.
    pub size: Option<u64>,
    /// Modification time.
    pub modified: Option<SystemTime>,
}

impl FileStamp {
    /// The current stamp of `path`. Like the pre-check, only a regular file has a size and a
    /// modification time; a FIFO or device has neither, so its stamp always matches (comparing
    /// them would report a change on every run).
    #[must_use]
    pub fn of(path: &std::path::Path) -> Option<Self> {
        let m = std::fs::metadata(path).ok()?;
        let file = m.is_file();
        Some(Self {
            size: file.then_some(m.len()),
            modified: if file { m.modified().ok() } else { None },
        })
    }
}

/// What to run.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// The alignment.
    pub path: PathBuf,
    /// Parameters (library types; `None` = default).
    pub params: AnalysisParams,
    /// Worker threads (settings, D101).
    pub threads: NonZeroUsize,
    /// The stamp seen by the pre-check, if any (G65).
    pub expected: Option<FileStamp>,
    /// What the pre-check saw in the first batch; the run stops before counting if its own
    /// first batch differs (D108).
    pub expected_probe: Option<ProbeFacts>,
}

/// Execution facts kept for the run details and the run report (D103).
#[derive(Clone, Debug, PartialEq)]
pub struct RunFacts {
    /// Start, wall clock.
    pub start_time: SystemTime,
    /// End, wall clock.
    pub end_time: SystemTime,
    /// Wall time.
    pub wall: Duration,
    /// Worker threads requested (the settings).
    pub threads: usize,
    /// Worker threads started: `threads`, or fewer when the system refused some (D110); `None`
    /// when the run stopped before they were started.
    pub threads_started: Option<usize>,
    /// Time per phase, in order.
    pub phases: Vec<(Phase, Duration)>,
    /// Records read.
    pub records: u64,
    /// Positions analysed, when the run got that far.
    pub positions: Option<u64>,
    /// Time the counting threads waited for input, when counting ran.
    pub waited_for_input: Option<Duration>,
}

/// How a run ended.
#[derive(Debug)]
pub enum Outcome {
    /// Results.
    Completed(Box<Analysis>, RunFacts),
    /// An error stopped the run; nothing was written.
    Failed(DimaError, RunFacts),
    /// The user cancelled.
    Cancelled,
    /// The file changed since the pre-check; nothing was run.
    Changed,
}

/// A running analysis.
pub struct Job {
    /// The worker.
    pub worker: Worker<Outcome>,
    /// Progress counters (polled once per frame).
    pub progress: Arc<Progress>,
    /// When it started.
    pub started: Instant,
}

impl Job {
    /// The current progress.
    #[must_use]
    pub fn snapshot(&self) -> ProgressSnapshot {
        self.progress.snapshot()
    }
}

/// Starts `request` on a background thread.
#[must_use]
pub fn spawn(request: Request, ctx: Option<egui::Context>) -> Job {
    let progress = Arc::new(Progress::new());
    let shared = Arc::clone(&progress);
    let worker = Worker::spawn("analysis", Priority::Background, ctx, move |cancel| {
        run(&request, &shared, cancel)
    });
    Job {
        worker,
        progress,
        started: Instant::now(),
    }
}

/// Runs a request synchronously (the job body; also used by tests).
pub fn run(
    request: &Request,
    progress: &Arc<Progress>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> Outcome {
    if let Some(expected) = request.expected
        && FileStamp::of(&request.path).is_some_and(|now| now != expected)
    {
        return Outcome::Changed;
    }
    let start_time = SystemTime::now();
    let started = Instant::now();
    let control = RunControl {
        cancel,
        progress: Arc::clone(progress),
        threads: Some(request.threads),
        expected_probe: request.expected_probe.clone(),
        thread_start: Some(dima_core::control::Hook::new(super::lower_priority)),
        ..RunControl::new()
    };
    let result = dima_core::analyze(
        &Source::Path(request.path.clone()),
        &request.params,
        &control,
    );
    let end_time = SystemTime::now();
    let wall = started.elapsed();
    match result {
        Ok(analysis) => {
            let e = &analysis.execution;
            let facts = RunFacts {
                start_time,
                end_time,
                wall,
                threads: e.threads_requested,
                threads_started: Some(e.threads),
                phases: e.phases.clone(),
                records: e.records,
                positions: Some(e.positions),
                waited_for_input: e
                    .phases
                    .iter()
                    .any(|(p, _)| *p == Phase::Counting)
                    .then_some(e.waited_for_input),
            };
            Outcome::Completed(Box::new(analysis), facts)
        }
        Err(e) => {
            let facts = RunFacts {
                start_time,
                end_time,
                wall,
                threads: request.threads.get(),
                threads_started: progress.threads_started(),
                phases: phases_from_progress(progress, wall),
                records: progress.snapshot().records,
                positions: None,
                waited_for_input: None,
            };
            match e {
                DimaError::Cancelled => Outcome::Cancelled,
                // Changed in a way the size and time did not show (D108): check it again.
                DimaError::InputChanged { .. } => Outcome::Changed,
                other => Outcome::Failed(other, facts),
            }
        }
    }
}

/// Phase times of a run that stopped early, from the phase start times: each phase lasts until
/// the next one started, the last until `total`.
#[must_use]
pub fn phases_from_progress(progress: &Progress, total: Duration) -> Vec<(Phase, Duration)> {
    let starts: Vec<(Phase, Duration)> = [Phase::Opening, Phase::Counting, Phase::Finalising]
        .into_iter()
        .filter_map(|p| progress.phase_started(p).map(|t| (p, t)))
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(i, &(phase, start))| {
            let end = starts.get(i + 1).map_or(total, |&(_, next)| next);
            (phase, end.saturating_sub(start))
        })
        .collect()
}

/// A one-line description of the progress: the phase, its step number and its counts, and the
/// completed fraction when it is known.
#[must_use]
pub fn describe(s: &ProgressSnapshot, elapsed: Duration) -> (String, Option<f32>) {
    use dima_core::human::{bytes, duration, separators::GROUP};
    let elapsed = duration(elapsed);
    let fraction = |done: u64, total: u64| {
        (total > 0).then(|| {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let f = (done as f64 / total as f64).clamp(0.0, 1.0) as f32;
            f
        })
    };
    match s.phase {
        Phase::Opening => (
            format!("Step 1 of 3: opening the input{GROUP}{elapsed}"),
            None,
        ),
        Phase::Counting => {
            let size = s
                .bytes_total
                .map_or_else(String::new, |t| format!(" of {}", bytes(t)));
            (
                format!(
                    "Step 2 of 3: counting k-mers{GROUP}{} sequences read{GROUP}{}{size}{GROUP}{elapsed}",
                    s.records,
                    bytes(s.bytes_read)
                ),
                s.bytes_total.and_then(|t| fraction(s.bytes_read, t)),
            )
        }
        Phase::Finalising => (
            format!(
                "Step 3 of 3: computing statistics{GROUP}{} of {} positions{GROUP}{elapsed}",
                s.positions_done, s.positions_total
            ),
            fraction(s.positions_done, s.positions_total),
        ),
        Phase::Writing | Phase::Done => (format!("Finishing{GROUP}{elapsed}"), Some(1.0)),
    }
}

/// The default number of worker threads in the app: all CPUs but one, so the interface keeps a
/// core (D101), and at least one.
#[must_use]
pub fn default_threads() -> NonZeroUsize {
    NonZeroUsize::new(cpus().saturating_sub(1)).unwrap_or(NonZeroUsize::MIN)
}

/// The logical CPUs available to the app (at least 1), read once: on Linux the query reads
/// cgroup files, too slow for every frame of the options panel.
#[must_use]
pub fn cpus() -> usize {
    static CPUS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CPUS.get_or_init(|| std::thread::available_parallelism().map_or(1, NonZeroUsize::get))
}

/// The worker threads to use: the remembered choice within 1..=`cpus` (settings can come
/// from another machine, roaming profiles on Windows), else the default.
#[must_use]
pub fn worker_threads(setting: Option<usize>, cpus: usize) -> NonZeroUsize {
    setting
        .map(|n| n.clamp(1, cpus.max(1)))
        .and_then(NonZeroUsize::new)
        .unwrap_or_else(default_threads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn remembered_thread_counts_fit_this_machine() {
        assert_eq!(worker_threads(Some(64), 8).get(), 8);
        assert_eq!(worker_threads(Some(0), 8).get(), 1);
        assert_eq!(worker_threads(Some(3), 8).get(), 3);
        assert_eq!(worker_threads(None, 8), default_threads());
    }

    fn request(path: PathBuf) -> Request {
        Request {
            path,
            params: AnalysisParams::default(),
            threads: NonZeroUsize::new(2).unwrap(),
            expected: None,
            expected_probe: None,
        }
    }

    #[test]
    fn analyses_the_sample_like_the_cli() {
        let progress = Arc::new(Progress::new());
        let out = run(
            &request(crate::tests::fixtures::mers_path()),
            &progress,
            Arc::new(AtomicBool::new(false)),
        );
        let Outcome::Completed(a, facts) = out else {
            panic!("completed");
        };
        assert_eq!(a.results.sequence_count, 191);
        assert_eq!(a.results.position_count, 1345);
        assert_eq!(facts.threads, 2);
        assert_eq!(facts.threads_started, Some(2));
        assert_eq!(facts.positions, Some(1345));
        assert_eq!(
            facts.phases.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            [Phase::Opening, Phase::Counting, Phase::Finalising]
        );
        assert!(facts.waited_for_input.is_some());
        assert_eq!(
            a.results,
            *crate::tests::fixtures::mers(),
            "same results as the fixture"
        );
    }

    #[test]
    fn a_changed_file_is_not_analysed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.fa");
        std::fs::write(&p, ">a\nACGT\n").unwrap();
        let mut r = request(p.clone());
        r.expected = Some(FileStamp {
            size: Some(1),
            modified: None,
        });
        let progress = Arc::new(Progress::new());
        assert!(matches!(
            run(&r, &progress, Arc::new(AtomicBool::new(false))),
            Outcome::Changed
        ));
        r.expected = FileStamp::of(&p);
        assert!(!matches!(
            run(&r, &progress, Arc::new(AtomicBool::new(false))),
            Outcome::Changed
        ));
        // A non-regular file has no stamp, as in the pre-check: never "changed".
        #[cfg(unix)]
        assert_eq!(
            FileStamp::of(std::path::Path::new("/dev/null")),
            Some(FileStamp {
                size: None,
                modified: None
            })
        );
    }

    /// D108: a rewrite that keeps the size and restores the modification time (`cp -p`,
    /// `rsync -t`, a restore from backup) passes the stamp check but not the probe check.
    #[test]
    fn a_rewrite_with_the_same_stamp_is_not_analysed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.fa");
        std::fs::write(&p, ">a\nMKVLAAGIIG\n>b\nMKVLAAGIVG\n").unwrap();
        let checked = dima_core::validation::inspect(&Source::Path(p.clone())).unwrap();
        let mtime = std::fs::metadata(&p).unwrap().modified().unwrap();
        std::fs::write(&p, ">a\nACGTACGTAC\n>b\nACGTACGTAA\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        let mut r = request(p.clone());
        r.expected = Some(FileStamp {
            size: checked.size_bytes,
            modified: checked.modified,
        });
        assert_eq!(FileStamp::of(&p), r.expected, "the stamp cannot tell");
        r.expected_probe = Some(checked.probe);
        let progress = Arc::new(Progress::new());
        assert!(matches!(
            run(&r, &progress, Arc::new(AtomicBool::new(false))),
            Outcome::Changed
        ));
    }

    #[test]
    fn failures_and_cancellation_keep_their_facts() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.fa");
        std::fs::write(&p, ">a\nACDEFGHIK\n>b\nACDEF1HIK\n").unwrap();
        let mut r = request(p);
        r.params.alphabet = Some(dima_core::config::Alphabet::Protein);
        let progress = Arc::new(Progress::new());
        match run(&r, &progress, Arc::new(AtomicBool::new(false))) {
            Outcome::Failed(e, facts) => {
                assert_eq!(e.class().code(), 4);
                assert!(!facts.phases.is_empty());
                assert_eq!(facts.positions, None);
            }
            other => panic!("expected a failure, got {other:?}"),
        }
        let progress = Arc::new(Progress::new());
        let r = request(crate::tests::fixtures::mers_path());
        assert!(matches!(
            run(&r, &progress, Arc::new(AtomicBool::new(true))),
            Outcome::Cancelled
        ));
    }

    #[test]
    fn progress_descriptions() {
        let mut s = ProgressSnapshot {
            phase: Phase::Counting,
            bytes_read: 50,
            bytes_total: Some(200),
            records: 1234,
            positions_done: 0,
            positions_total: 0,
        };
        let (text, f) = describe(&s, Duration::from_secs(3));
        assert!(text.contains("1234 sequences read"), "{text}");
        assert_eq!(f, Some(0.25));
        s.phase = Phase::Finalising;
        s.positions_total = 10;
        s.positions_done = 5;
        assert_eq!(describe(&s, Duration::ZERO).1, Some(0.5));
        s.phase = Phase::Opening;
        assert_eq!(describe(&s, Duration::ZERO).1, None);
        assert!(default_threads().get() >= 1);
    }

    #[test]
    fn phases_of_an_early_stop() {
        let p = Progress::new();
        p.set_phase(Phase::Counting);
        let phases = phases_from_progress(&p, Duration::from_secs(1));
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].0, Phase::Opening);
        assert_eq!(phases[1].0, Phase::Counting);
    }
}
