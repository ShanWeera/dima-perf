//! Orchestration of an analysis: input → probe → normalisation → counting → statistics →
//! summaries and provenance. Each step lives in its own module; this one only wires them.
//!
//! Reading runs on its own thread, one batch ahead of normalisation and counting, which run in
//! parallel on a local rayon pool sized by [`RunControl::thread_count`], or smaller when the
//! system refuses some of its threads (D110). At most two batches are in memory at a time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::alphabet::{self, ByteClassTable};
use crate::config::{AnalysisParams, Origin, Param, ValidationMode};
use crate::control::{Hook, Phase, RunControl};
use crate::diagnostics::Tally;
use crate::engine::Engine;
use crate::error::{DimaError, Result};
use crate::header::{self, HeaderSpec, HeaderStats, ValueDictionary};
use crate::input::{self, RawBatch, Source};
use crate::model::{AlphabetMethod, Parameters, Results, RunInfo, SCHEMA_VERSION, ToolInfo};
use crate::normalise::{NormalisedBatch, Normaliser, SequenceStats};
use crate::pool;
use crate::position::Finaliser;
use crate::provenance;
use crate::summary;
use crate::validation::probe_facts;

/// How often a wait for input checks for cancellation.
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// Execution facts of a run (not part of the deterministic results).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Execution {
    /// Worker threads used: those requested, or fewer when the system refused some (D110).
    pub threads: usize,
    /// Worker threads requested ([`RunControl::thread_count`]).
    pub threads_requested: usize,
    /// Wall time per phase, in order.
    pub phases: Vec<(Phase, Duration)>,
    /// Time the counting loop waited for the reader while counting k-mers (high values mean
    /// reading or decompression is the bottleneck). The wait for the first batch, which is read
    /// while opening, is part of that phase and not included.
    pub waited_for_input: Duration,
    /// Records read.
    pub records: u64,
    /// Positions analysed.
    pub positions: u64,
}

/// The outcome of [`analyze`].
#[derive(Clone, Debug)]
pub struct Analysis {
    /// The deterministic results.
    pub results: Results,
    /// How the run went.
    pub execution: Execution,
}

/// Analyses `source` with `params`.
///
/// # Errors
/// Usage, I/O, input-format and input-data errors with their locations; [`DimaError::Cancelled`]
/// when `control.cancel` is set.
pub fn analyze(source: &Source, params: &AnalysisParams, control: &RunControl) -> Result<Analysis> {
    params.validate()?;
    let spec = match &params.header_format {
        Some(format) => Some(HeaderSpec::new(
            format,
            params.metadata_fields.as_deref(),
            &params.header_fill().value,
        )?),
        None => None,
    };
    let requested = control.thread_count().get();
    Run::new(source, params, spec.as_ref(), control, requested).execute()
}

struct Run<'a> {
    source: &'a Source,
    params: &'a AnalysisParams,
    spec: Option<&'a HeaderSpec>,
    control: &'a RunControl,
    /// Worker threads requested.
    requested: usize,
    /// Worker threads started (D110); `requested` until the pool is built.
    threads: usize,
    phases: Vec<(Phase, Duration)>,
    phase_start: Instant,
}

/// Everything accumulated while reading.
struct Counting {
    engine: Engine,
    table: ByteClassTable,
    resolution: alphabet::Resolution,
    width: usize,
    k: usize,
    probe_records: u64,
    records: u64,
    sequence_stats: SequenceStats,
    header_stats: HeaderStats,
    dictionary: ValueDictionary,
    identifier: Option<crate::diagnostics::Diagnostic>,
    waited: Duration,
}

impl<'a> Run<'a> {
    fn new(
        source: &'a Source,
        params: &'a AnalysisParams,
        spec: Option<&'a HeaderSpec>,
        control: &'a RunControl,
        requested: usize,
    ) -> Self {
        Self {
            source,
            params,
            spec,
            control,
            requested,
            threads: requested,
            phases: Vec::new(),
            phase_start: Instant::now(),
        }
    }

    fn enter(&mut self, phase: Phase) {
        let now = Instant::now();
        if let Some(last) = self.phases.last_mut() {
            last.1 = now - self.phase_start;
        }
        self.control.progress.set_phase(phase);
        self.phases.push((phase, Duration::ZERO));
        self.phase_start = now;
    }

    fn execute(mut self) -> Result<Analysis> {
        self.enter(Phase::Opening);
        let thread_start = self.control.thread_start.clone();
        let opened = self.open_input(thread_start.as_ref())?;
        let input::Opened {
            compression,
            batches,
            checksum,
            ..
        } = opened;
        let stop = Arc::new(AtomicBool::new(false));
        let (batch_tx, batch_rx) = mpsc::sync_channel::<Result<RawBatch>>(0);
        let (recycle_tx, recycle_rx) = mpsc::channel::<RawBatch>();
        let reader_stop = Arc::clone(&stop);
        let limits = self.control.batch_limits;
        let cancel = Arc::clone(&self.control.cancel);
        // Detached on purpose: a reader blocked on a slow pipe must never delay an error or a
        // cancellation (D96). It exits at the next record once `stop` is set.
        let reader_start = thread_start.clone();
        let reader = thread::Builder::new()
            .name("dima-reader".into())
            .spawn(move || {
                if let Some(h) = &reader_start {
                    h.call();
                }
                let mut batches = batches.with_limits(limits);
                loop {
                    if reader_stop.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let mut b = recycle_rx.try_recv().unwrap_or_default();
                    match batches.read_batch(&mut b, &reader_stop) {
                        Ok(true) => {
                            if batch_tx.send(Ok(b)).is_err() {
                                break;
                            }
                        }
                        Ok(false) => break,
                        Err(e) => {
                            let _ = batch_tx.send(Err(e));
                            break;
                        }
                    }
                }
            })
            .map_err(|e| DimaError::system("the reader thread", e))?;
        // The worker pool comes after the threads the run cannot do without (checksum, reader),
        // so a system short of threads runs fewer workers instead of refusing those (D110).
        let pool = match pool::build(self.requested, thread_start) {
            Ok(fitted) => {
                self.threads = fitted.started;
                self.control.progress.set_threads_started(fitted.started);
                fitted.pool
            }
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                return Err(e);
            }
        };
        if let Some(h) = &self.control.threads_ready {
            h.call();
        }
        pool.install(move || {
            let counting = match self.count(&batch_rx, &recycle_tx) {
                Ok(c) => c,
                Err(e) => {
                    stop.store(true, Ordering::Relaxed);
                    return Err(e);
                }
            };
            drop(batch_rx);
            reader
                .join()
                .map_err(|_| DimaError::internal("the reader thread panicked"))?;
            let (checksum, size_bytes) = checksum.finish()?;
            let input_info = crate::model::InputInfo {
                name: self.source.provenance_name(),
                size_bytes,
                checksum,
                compression,
            };
            self.finish(counting, input_info)
        })
    }

    /// Opens the input on a thread of its own (D96): opening reads the first bytes (to
    /// recognise the compression, a byte-order mark and leading blank lines), and a pipe or
    /// FIFO with no bytes yet would block a cancellation until the grace period ends the
    /// process without its closing message or run report. Waiting here notices it within
    /// [`CANCEL_POLL`]; a thread blocked on an idle pipe is left behind, as the reader is.
    fn open_input(&self, thread_start: Option<&Hook>) -> Result<input::Opened> {
        let (tx, rx) = mpsc::sync_channel(1);
        let source = self.source.clone();
        let progress = Arc::clone(&self.control.progress);
        let hook = thread_start.cloned();
        thread::Builder::new()
            .name("dima-open".into())
            .spawn(move || {
                if let Some(h) = &hook {
                    h.call();
                }
                let _ = tx.send(input::open_with(&source, &progress, hook.as_ref()));
            })
            .map_err(|e| DimaError::system("the thread that opens the input", e))?;
        loop {
            match rx.recv_timeout(CANCEL_POLL) {
                Ok(opened) => return opened,
                Err(mpsc::RecvTimeoutError::Timeout) => self.control.check()?,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(DimaError::internal(
                        "the thread that opens the input panicked",
                    ));
                }
            }
        }
    }

    /// The next batch, `None` at the end of the input. Waiting for a slow pipe still notices a
    /// cancellation within [`CANCEL_POLL`].
    fn next_batch(
        rx: &mpsc::Receiver<Result<RawBatch>>,
        waited: &mut Duration,
        control: &RunControl,
    ) -> Result<Option<RawBatch>> {
        let t = Instant::now();
        let r = loop {
            match rx.recv_timeout(CANCEL_POLL) {
                Ok(r) => break Some(r),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Err(e) = control.check() {
                        *waited += t.elapsed();
                        return Err(e);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };
        *waited += t.elapsed();
        match r {
            Some(Ok(b)) => Ok(Some(b)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    fn count(
        &mut self,
        rx: &mpsc::Receiver<Result<RawBatch>>,
        recycle: &mpsc::Sender<RawBatch>,
    ) -> Result<Counting> {
        let control = self.control;
        // The first batch arrives while opening; `waited` covers the counting phase only, so a
        // slow start (opening a file, a pipe that has not written yet) is not counted as input
        // limiting the counting.
        let mut first_wait = Duration::ZERO;
        let mut waited = Duration::ZERO;
        let first = Self::next_batch(rx, &mut first_wait, control)?.ok_or_else(|| {
            control
                .check()
                .err()
                .unwrap_or_else(|| DimaError::internal("the reader stopped before the first batch"))
        })?;
        control.check()?;
        // Probe (D40): alphabet and alignment width from the first batch, computed like the
        // pre-check's (D108), which a front end may ask to match.
        let probe = probe_facts(&first);
        if control.expected_probe.as_ref().is_some_and(|e| *e != probe) {
            return Err(DimaError::InputChanged {
                source_name: self.source.display_name(),
            });
        }
        let probe_records = probe.records;
        let source_name = self.source.display_name();
        // An empty first record leaves nothing to detect the alphabet from: report the record.
        if let (0, Some(r)) = (probe.alignment_length, first.records.first()) {
            return Err(crate::normalise::empty_sequence(
                &source_name,
                first.first_record,
                r.line,
            ));
        }
        let resolution = alphabet::resolve(self.params.alphabet, &probe.letters, probe_records)
            .map_err(|e| e.in_source(&source_name))?;
        let table = ByteClassTable::new(resolution.alphabet);
        let width = probe.alignment_length;
        let mode = self.params.validation().value;
        let strict = mode == ValidationMode::Strict;
        let normaliser = Normaliser {
            table: &table,
            mode,
            width,
            header: self.spec,
            source: &source_name,
            looks_like: resolution.looks_like(),
        };
        let mut norm = NormalisedBatch::default();
        normaliser.normalise(&first, &mut norm)?;
        let k = self
            .params
            .kmer_length_for(resolution.alphabet, width)?
            .value;
        let strata = self.spec.map_or(0, |s| s.stratified().len());
        let mut engine = Engine::new(width, k, strata, self.threads)?;
        tracing::debug!(
            alphabet = %resolution.alphabet,
            method = ?resolution.method,
            probe_records,
            first_batch_wait_ms = first_wait.as_millis(),
            alignment_length = width,
            k,
            positions = engine.positions(),
            shards = engine.shard_count(),
            threads = self.threads,
            strata,
            "probe done; counting"
        );
        control
            .progress
            .set_positions_total(engine.positions() as u64);
        self.enter(Phase::Counting);
        let identifier = self.spec.and_then(|spec| {
            let mut columns: Vec<Vec<String>> = vec![Vec::with_capacity(norm.records); strata];
            for values in &norm.header_values {
                for (c, v) in columns.iter_mut().zip(values) {
                    c.push(v.clone());
                }
            }
            header::identifier_diagnostic(spec, &columns, probe_records)
        });
        let mut records = 0u64;
        let mut sequence_stats = SequenceStats::default();
        let mut header_stats = HeaderStats::default();
        let mut dictionary = ValueDictionary::new(strata);
        let mut values: Vec<u32> = Vec::new();
        let mut current = Some(first);
        let mut normalised = true;
        while let Some(batch) = current.take() {
            control.check()?;
            if !normalised {
                normaliser.normalise(&batch, &mut norm)?;
            }
            normalised = false;
            values.clear();
            for record in norm.header_values.drain(..) {
                for (f, v) in record.into_iter().enumerate() {
                    values.push(dictionary.intern(f, v)?);
                }
            }
            header_stats.merge(std::mem::take(&mut norm.header_stats));
            // In strict mode a header mismatch ends the run once all headers are read (D18);
            // counting the rest would be wasted.
            if !(strict && header_stats.mismatch_count > 0) {
                engine.add_batch(&norm.rows, norm.records, &values, &control.cancel)?;
            }
            records = records
                .checked_add(norm.records as u64)
                .ok_or(DimaError::Overflow {
                    what: "the number of records",
                })?;
            control.progress.add_records(norm.records as u64);
            sequence_stats.merge(std::mem::take(&mut norm.sequence_stats));
            let _ = recycle.send(batch);
            current = Self::next_batch(rx, &mut waited, control)?;
        }
        control.check()?;
        tracing::debug!(
            records,
            waited_for_input_ms = waited.as_millis(),
            distinct_header_values = dictionary.len(),
            "input read"
        );
        if let Some(spec) = self.spec {
            if strict && header_stats.mismatch_count > 0 {
                return Err(header::mismatch_error(&source_name, spec, &header_stats));
            }
        }
        Ok(Counting {
            engine,
            table,
            resolution,
            width,
            k,
            probe_records,
            records,
            sequence_stats,
            header_stats,
            dictionary,
            identifier,
            waited,
        })
    }

    fn finish(mut self, c: Counting, input_info: crate::model::InputInfo) -> Result<Analysis> {
        self.enter(Phase::Finalising);
        let params = self.params;
        let threshold = params.support_threshold();
        let estimator = params.estimator();
        let validation = params.validation();
        let alphabet = c.resolution.alphabet;
        let strata_names = self
            .spec
            .map(HeaderSpec::stratified_names)
            .unwrap_or_default();
        let values: Vec<(usize, String)> = (0..c.dictionary.len())
            .map(|id| {
                #[allow(clippy::cast_possible_truncation)] // ids are u32.
                let (f, v, _) = c.dictionary.entry(id as u32);
                (f, v.to_owned())
            })
            .collect();
        let finaliser = Finaliser {
            threshold: threshold.value,
            estimator: estimator.value,
            min_coverage: params.min_coverage,
            similarity_weighted_entropy: params.similarity_weighted_entropy,
            fields: &strata_names,
            values: &values,
        };
        let progress = &self.control.progress;
        let cancel = &self.control.cancel;
        let counts = c.engine.finish();
        let positions: Vec<crate::model::PositionResult> = counts
            .into_par_iter()
            .enumerate()
            .map(|(i, pc)| {
                if cancel.load(Ordering::Relaxed) {
                    return Err(DimaError::Cancelled);
                }
                let r = finaliser.finalise(i as u64 + 1, &pc);
                progress.add_positions_done(1);
                r
            })
            .collect::<Result<_>>()?;
        self.control.check()?;

        let label_counts = summary::label_counts(&positions);
        let (average_entropy, lowest_entropy, highest_entropy) = summary::eligible(&positions);
        let mut below = Tally::default();
        for p in positions
            .iter()
            .filter(|p| p.below_min_coverage == Some(true))
        {
            let coverage = p.sample_coverage.unwrap_or_default();
            let min = params.min_coverage.unwrap_or(1.0);
            below.add(1, || {
                // Bounded by the minimum, so a coverage just below it never reads as reaching it.
                let shown = crate::human::fixed_bounded(coverage, 4, min);
                format!("position {} (coverage {shown})", p.position)
            });
        }
        let value_diagnostics = self
            .spec
            .map(|spec| header::value_diagnostics(spec, &c.dictionary))
            .unwrap_or_default();
        let diagnostics = provenance::diagnostics(provenance::DiagnosticInputs {
            resolution: &c.resolution,
            sequences: &c.sequence_stats,
            headers: self.spec.map(|s| (s, &c.header_stats)),
            value_diagnostics,
            identifier: c.identifier.clone(),
            sequence_count: c.records,
            threshold: threshold.value,
            estimator: estimator.value,
            eligible_positions: label_counts.eligible,
            positions: &positions,
            below_min_coverage: params.min_coverage.map(|m| (m, below)),
            fill_without_format: params.header_fill.is_some() && self.spec.is_none(),
        });

        let query_name = match &params.query_name {
            Some(q) => Param {
                value: q.trim().to_owned(),
                origin: Origin::User,
            },
            None => match input::query_name_from_source(self.source) {
                Some(stem) => Param {
                    value: stem,
                    origin: Origin::Derived,
                },
                None => Param {
                    value: format!("Unknown {}", alphabet.display_name()),
                    origin: Origin::Default,
                },
            },
        };
        let user_or_default = |given: bool| if given { Origin::User } else { Origin::Default };
        let parameters = Parameters {
            alphabet: Param {
                value: alphabet,
                origin: if c.resolution.method == AlphabetMethod::User {
                    Origin::User
                } else {
                    Origin::Derived
                },
            },
            kmer_length: {
                let k = params.kmer_length(alphabet);
                Param {
                    value: k.value as u64,
                    origin: k.origin,
                }
            },
            support_threshold: threshold.clone(),
            estimator: estimator.clone(),
            similarity_weighted_entropy: params.similarity_weighted_entropy(),
            validation: validation.clone(),
            min_coverage: Param {
                value: params.min_coverage,
                origin: user_or_default(params.min_coverage.is_some()),
            },
            query_name: query_name.clone(),
            header_format: Param {
                value: self.spec.map(|s| s.fields().to_vec()),
                origin: user_or_default(self.spec.is_some()),
            },
            metadata_fields: Param {
                value: self.spec.map(HeaderSpec::stratified_names),
                origin: user_or_default(params.metadata_fields.is_some()),
            },
            // The value used: trimmed and NFC-normalised like the field names.
            header_fill: {
                let fill = params.header_fill();
                Param {
                    value: self
                        .spec
                        .map_or_else(|| header::clean_name(&fill.value), |s| s.fill().to_owned()),
                    origin: fill.origin,
                }
            },
        };
        let run = RunInfo {
            tool: ToolInfo {
                name: "dima".to_owned(),
                version: crate::citations::SOFTWARE_VERSION.to_owned(),
            },
            parameters,
            alphabet: provenance::alphabet_info(
                &c.resolution,
                c.probe_records,
                c.sequence_stats.t_u_folded,
            ),
            alignment_length: c.width as u64,
            input: input_info,
            validation: provenance::validation_info(
                validation.value,
                c.records,
                &c.table,
                &c.sequence_stats,
                self.spec.map(|_| &c.header_stats),
            ),
            metadata: self
                .spec
                .map(|s| provenance::metadata_info(s, &c.header_stats, c.probe_records)),
            entropy: provenance::entropy_info(estimator.value),
            diagnostics,
            references: provenance::references(provenance::MethodsUsed {
                estimator: estimator.value,
                alphabet,
                alphabet_detected: c.resolution.method == AlphabetMethod::Detected,
                similarity_weighted_entropy: params.similarity_weighted_entropy,
            }),
        };
        let results = Results {
            schema_version: SCHEMA_VERSION.to_owned(),
            query_name: query_name.value,
            sequence_count: c.records,
            support_threshold: threshold.value,
            kmer_length: c.k as u64,
            low_support_count: label_counts.no_support
                + label_counts.low
                + label_counts.equal_to_threshold,
            average_entropy,
            lowest_entropy,
            highest_entropy,
            position_count: positions.len() as u64,
            eligible_position_count: label_counts.eligible,
            support_label_counts: label_counts,
            all_positions_summary: summary::all_positions(&positions),
            coverage_summary: summary::coverage(&positions, params.min_coverage),
            similarity_weighted_entropy_summary: params
                .similarity_weighted_entropy
                .then(|| summary::similarity_weighted(&positions)),
            run,
            results: positions,
        };
        self.enter(Phase::Done);
        let execution = Execution {
            threads: self.threads,
            threads_requested: self.requested,
            phases: self.phases[..self.phases.len() - 1].to_vec(),
            waited_for_input: c.waited,
            records: c.records,
            positions: results.position_count,
        };
        Ok(Analysis { results, execution })
    }
}
