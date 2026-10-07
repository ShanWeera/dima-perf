//! Cancellation and progress shared between an analysis and its front end.
//!
//! The library never prints or draws: it updates lock-free counters that the CLI (progress
//! bars) and the GUI (progress panel) poll, and it checks a cancellation flag at every record,
//! batch and finalised position.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::error::{DimaError, Result};

/// Phases of an analysis, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Phase {
    /// Opening and probing the input.
    Opening = 0,
    /// Reading records and counting k-mers.
    Counting = 1,
    /// Computing per-position statistics.
    Finalising = 2,
    /// Writing outputs.
    Writing = 3,
    /// Finished.
    Done = 4,
}

impl Phase {
    /// Every phase, in order.
    pub const ALL: [Self; 5] = [
        Self::Opening,
        Self::Counting,
        Self::Finalising,
        Self::Writing,
        Self::Done,
    ];

    const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Opening,
            1 => Self::Counting,
            2 => Self::Finalising,
            3 => Self::Writing,
            _ => Self::Done,
        }
    }

    /// Stable machine name (run reports).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Counting => "counting",
            Self::Finalising => "finalising",
            Self::Writing => "writing",
            Self::Done => "done",
        }
    }

    /// Human-readable name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Counting => "counting k-mers",
            Self::Finalising => "computing statistics",
            Self::Writing => "writing",
            Self::Done => "done",
        }
    }
}

/// Progress counters, updated by the library and read by front ends at any time.
#[derive(Debug)]
pub struct Progress {
    origin: Instant,
    phase: AtomicU8,
    /// When each phase started: nanoseconds since `origin` plus one; 0 = not reached.
    phase_started: [AtomicU64; Phase::ALL.len()],
    bytes_read: AtomicU64,
    bytes_total: AtomicU64,
    records: AtomicU64,
    positions_done: AtomicU64,
    positions_total: AtomicU64,
    /// Worker threads started (D110); 0 until the pool is built.
    threads_started: AtomicUsize,
}

/// A consistent-enough copy of the progress counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressSnapshot {
    /// Current phase.
    pub phase: Phase,
    /// Stored (possibly compressed) input bytes read so far.
    pub bytes_read: u64,
    /// Stored input size, when known (regular files).
    pub bytes_total: Option<u64>,
    /// Records read so far.
    pub records: u64,
    /// Positions finalised so far.
    pub positions_done: u64,
    /// Positions to finalise (known once the alignment length is).
    pub positions_total: u64,
}

impl Default for Progress {
    fn default() -> Self {
        let p = Self {
            origin: Instant::now(),
            phase: AtomicU8::new(Phase::Opening as u8),
            phase_started: Default::default(),
            bytes_read: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
            records: AtomicU64::new(0),
            positions_done: AtomicU64::new(0),
            positions_total: AtomicU64::new(0),
            threads_started: AtomicUsize::new(0),
        };
        p.phase_started[Phase::Opening as usize].store(1, Ordering::Relaxed);
        p
    }
}

impl Progress {
    /// A fresh counter set; the opening phase starts now.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Time since [`Progress::new`] (the start of the run).
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.origin.elapsed()
    }

    /// When `phase` started, measured from [`Progress::new`]; `None` if it has not.
    #[must_use]
    pub fn phase_started(&self, phase: Phase) -> Option<Duration> {
        match self.phase_started[phase as usize].load(Ordering::Relaxed) {
            0 => None,
            n => Some(Duration::from_nanos(n - 1)),
        }
    }

    /// Enters `phase` (front ends enter [`Phase::Writing`] and [`Phase::Done`] themselves) and
    /// records when it started.
    pub fn set_phase(&self, phase: Phase) {
        let nanos = u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX - 1);
        let _ = self.phase_started[phase as usize].compare_exchange(
            0,
            nanos + 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        self.phase.store(phase as u8, Ordering::Relaxed);
    }

    /// Reads all counters.
    #[must_use]
    pub fn snapshot(&self) -> ProgressSnapshot {
        let total = self.bytes_total.load(Ordering::Relaxed);
        ProgressSnapshot {
            phase: Phase::from_u8(self.phase.load(Ordering::Relaxed)),
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            bytes_total: (total > 0).then_some(total),
            records: self.records.load(Ordering::Relaxed),
            positions_done: self.positions_done.load(Ordering::Relaxed),
            positions_total: self.positions_total.load(Ordering::Relaxed),
        }
    }

    /// Worker threads the run started (D110): `None` until its pool is built, so a run that
    /// fails or is cancelled afterwards still reports them.
    #[must_use]
    pub fn threads_started(&self) -> Option<usize> {
        match self.threads_started.load(Ordering::Relaxed) {
            0 => None,
            n => Some(n),
        }
    }

    pub(crate) fn set_threads_started(&self, n: usize) {
        self.threads_started.store(n, Ordering::Relaxed);
    }

    pub(crate) fn add_bytes_read(&self, n: u64) {
        self.bytes_read.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn set_bytes_total(&self, n: u64) {
        self.bytes_total.store(n, Ordering::Relaxed);
    }

    pub(crate) fn add_records(&self, n: u64) {
        self.records.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn set_positions_total(&self, n: u64) {
        self.positions_total.store(n, Ordering::Relaxed);
    }

    pub(crate) fn add_positions_done(&self, n: u64) {
        self.positions_done.fetch_add(n, Ordering::Relaxed);
    }
}

/// A function a front end has the analysis run at a given moment. It must return promptly.
#[derive(Clone)]
pub struct Hook(Arc<dyn Fn() + Send + Sync>);

impl Hook {
    /// Wraps `f`.
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// Runs it.
    pub fn call(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hook")
    }
}

/// Cancellation, progress and threading for one analysis.
#[derive(Clone, Debug, Default)]
pub struct RunControl {
    /// Set to request cancellation; the analysis returns [`DimaError::Cancelled`] promptly.
    pub cancel: Arc<AtomicBool>,
    /// Progress counters.
    pub progress: Arc<Progress>,
    /// Worker threads; `None` uses the available parallelism.
    pub threads: Option<NonZeroUsize>,
    /// Batch limits of the reader (defaults per D40; smaller values only change memory use and
    /// are used by tests to exercise batch boundaries — results never depend on them).
    pub batch_limits: crate::input::BatchLimits,
    /// What a pre-check saw in the input's first batch (D108). When set, the run stops with
    /// [`DimaError::InputChanged`] before counting if its own first batch differs, so a file
    /// replaced after it was checked is never analysed with settings chosen for another one.
    /// It never changes results.
    pub expected_probe: Option<ProbeFacts>,
    /// Runs first in every thread the analysis starts (reader, checksum, workers). The desktop
    /// app lowers their priority there: on Windows threads do not inherit it (D101).
    pub thread_start: Option<Hook>,
    /// Runs once every thread the analysis needs is running, before counting. The command line
    /// starts its progress display there, so an optional thread never takes the place of one
    /// the run needs when the system limits threads (D110).
    pub threads_ready: Option<Hook>,
}

/// What the first batch of an input shows, computed by one function for the pre-check and the
/// run (`validation::probe_facts`), so the two can be compared exactly (D108).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProbeFacts {
    /// Records in the first batch.
    pub records: u64,
    /// Letters of their sequences (what alphabet detection reads).
    pub letters: crate::alphabet::LetterCounts,
    /// Alignment columns of the first record.
    pub alignment_length: usize,
    /// `|`-separated fields of the first header.
    pub header_fields: usize,
}

impl RunControl {
    /// A control block with fresh flags and the default thread count.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the number of worker threads.
    #[must_use]
    pub fn with_threads(mut self, threads: NonZeroUsize) -> Self {
        self.threads = Some(threads);
        self
    }

    /// `true` once cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// `Err(Cancelled)` once cancellation was requested.
    ///
    /// # Errors
    /// [`DimaError::Cancelled`].
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(DimaError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// The worker-thread count: the explicit value, else the available parallelism (≥ 1).
    #[must_use]
    pub fn thread_count(&self) -> NonZeroUsize {
        self.threads
            .unwrap_or_else(|| std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_round_trip() {
        let p = Progress::new();
        assert_eq!(p.snapshot().phase, Phase::Opening);
        assert_eq!(p.snapshot().bytes_total, None);
        p.set_phase(Phase::Finalising);
        p.add_bytes_read(10);
        p.add_bytes_read(5);
        p.set_bytes_total(100);
        p.add_records(3);
        p.set_positions_total(9);
        p.add_positions_done(4);
        let s = p.snapshot();
        assert_eq!(s.phase, Phase::Finalising);
        assert_eq!((s.bytes_read, s.bytes_total, s.records), (15, Some(100), 3));
        assert_eq!((s.positions_done, s.positions_total), (4, 9));
        assert_eq!(Phase::Counting.label(), "counting k-mers");
        assert_eq!(p.phase_started(Phase::Opening), Some(Duration::ZERO));
        assert!(p.phase_started(Phase::Finalising).is_some());
        assert_eq!(
            p.phase_started(Phase::Counting),
            None,
            "skipped phases have no start"
        );
        // A phase keeps its first start time.
        let first = p.phase_started(Phase::Finalising);
        p.set_phase(Phase::Finalising);
        assert_eq!(p.phase_started(Phase::Finalising), first);
    }

    #[test]
    fn cancellation() {
        let c = RunControl::new();
        assert!(c.check().is_ok());
        c.cancel.store(true, Ordering::Relaxed);
        assert!(matches!(c.check(), Err(DimaError::Cancelled)));
        assert!(c.thread_count().get() >= 1);
        let c = RunControl::new().with_threads(NonZeroUsize::new(3).unwrap());
        assert_eq!(c.thread_count().get(), 3);
    }
}
