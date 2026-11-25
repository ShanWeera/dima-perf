//! `dima analyze`.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use dima_core::config::{Alphabet, AnalysisParams, Estimator, Percent, ValidationMode};
use dima_core::control::{Hook, Phase, Progress, RunControl};
use dima_core::hcs::{self, HcsParams};
use dima_core::header::{self, HeaderSpec};
use dima_core::human;
use dima_core::input::{self, Source};
use dima_core::model::{HcsResults, Results};
use dima_core::{DimaError, Result};

use super::Context;
use super::args::{AlphabetArg, AnalyzeArgs, EstimatorArg, ValidationArg};
use super::outputs::{self, Content, Plan};
use super::progress::{self, Display};
use super::resources;
use super::threads::{self, Threads};
use super::ui::{self, Verbosity};
use dima_core::run_report::{self as report, FileRecord, PhaseTime, Platform, RunReport, Status};

const fn alphabet(a: AlphabetArg) -> Option<Alphabet> {
    match a {
        AlphabetArg::Auto => None,
        AlphabetArg::Protein => Some(Alphabet::Protein),
        AlphabetArg::Dna => Some(Alphabet::Dna),
        AlphabetArg::Rna => Some(Alphabet::Rna),
    }
}

const fn estimator(e: EstimatorArg) -> Estimator {
    match e {
        EstimatorArg::Dima => Estimator::Dima,
        EstimatorArg::ChaoWangJost => Estimator::ChaoWangJost,
        EstimatorArg::MillerMadow => Estimator::MillerMadow,
        EstimatorArg::Raw => Estimator::Raw,
    }
}

const fn validation(v: ValidationArg) -> ValidationMode {
    match v {
        ValidationArg::Strict => ValidationMode::Strict,
        ValidationArg::Permissive => ValidationMode::Permissive,
        ValidationArg::Report => ValidationMode::Report,
    }
}

/// The analysis parameters given on the command line.
///
/// # Errors
/// A usage error when k does not fit the platform's word size.
pub fn params(a: &AnalyzeArgs) -> Result<AnalysisParams> {
    let kmer_length = a
        .kmer_length
        .map(|k| {
            usize::try_from(k)
                .map_err(|_| DimaError::usage(format!("k-mer length {k} is too large")))
        })
        .transpose()?;
    Ok(AnalysisParams {
        alphabet: alphabet(a.alphabet),
        kmer_length,
        support_threshold: a.support_threshold,
        estimator: a.estimator.map(estimator),
        similarity_weighted_entropy: false,
        validation: a.validation.map(validation),
        min_coverage: a.min_coverage,
        query_name: a.query_name.clone(),
        header_format: a.header_format.as_deref().map(header::split_format),
        metadata_fields: a.metadata_fields.as_deref().map(header::split_format),
        header_fill: a.header_fill.clone(),
    })
}

/// The command rebuilt from the parsed options: long option names, file names without
/// directories (D103).
fn command(a: &AnalyzeArgs) -> Vec<String> {
    let name = |p: &PathBuf| report::file_name(p);
    let mut c: Vec<String> = vec!["dima".into(), "analyze".into(), "--input".into()];
    c.push(a.input.as_ref().map_or_else(|| "-".to_owned(), name));
    c.extend(params(a).map(|p| p.to_args()).unwrap_or_default());
    let mut opt = |flag: &str, value: Option<String>| {
        if let Some(v) = value {
            c.push(format!("--{flag}"));
            c.push(v);
        }
    };
    opt("output", a.output.output.as_ref().map(name));
    opt(
        "output-format",
        a.output
            .output_format
            .map(|f| format!("{f:?}").to_lowercase()),
    );
    opt(
        "table",
        a.output.table.map(|t| format!("{t:?}").to_lowercase()),
    );
    opt("tsv-dir", a.output.tsv_dir.as_ref().map(name));
    opt("hcs-output", a.hcs_output.as_ref().map(name));
    opt("hcs-min-incidence", a.hcs.hcs_min_incidence.clone());
    opt("run-report", a.run_report.as_ref().map(name));
    opt("threads", a.threads.map(|t| t.to_string()));
    if a.output.pretty {
        c.push("--pretty".into());
    }
    if a.hcs.hcs_eligible_only {
        c.push("--hcs-eligible-only".into());
    }
    c
}

/// Everything known about a run once it ends, for the summary and the run report.
#[derive(Default)]
struct Facts {
    /// Another output goes to standard output, so the run report cannot.
    stdout_taken: bool,
    /// The analysis was started (its phases are then recorded).
    analysis_started: bool,
    threads: Option<Threads>,
    input: Option<FileRecord>,
    execution: Option<dima_core::analysis::Execution>,
    outputs: Vec<FileRecord>,
    writing: Option<std::time::Duration>,
}

/// Runs `dima analyze`.
///
/// # Errors
/// Any error of the analysis or of writing the outputs.
pub fn run(a: &AnalyzeArgs, ctx: &Context) -> Result<()> {
    let start = Instant::now();
    let start_time = SystemTime::now();
    let progress = Arc::new(Progress::new());
    let mut facts = Facts::default();
    // A run report that would overwrite the input or another output is refused before anything
    // runs, because the report is written even when the run fails early (D103).
    if let Some(path) = &a.run_report {
        outputs::check_run_report(path, &known_paths(a, false))?;
    }
    let outcome = execute(a, ctx, &progress, &mut facts);
    // A report refused because another output takes standard output is not written there.
    let report_path = a
        .run_report
        .as_ref()
        .filter(|p| !(facts.stdout_taken && p.as_os_str() == "-"));
    if let Some(path) = report_path {
        // As `finish` ends the process: a reader of standard output that went away (`| head`)
        // after every file output was committed is a success.
        let (status, exit_code, error) = match &outcome {
            Ok(()) => (Status::Completed, 0, None),
            Err(e) if e.is_broken_pipe() => (Status::Completed, 0, None),
            Err(DimaError::Cancelled) => (Status::Cancelled, ctx.signals.exit_code(), None),
            Err(e) => (Status::Failed, e.class().code(), Some(e)),
        };
        let r = build_report(
            a, &facts, &progress, status, exit_code, error, start, start_time,
        );
        let planned = outputs::Planned {
            target: dima_core::output::Target::from_arg(path),
            compression: dima_core::output::detect(path).1,
            content: Content::RunReport,
        };
        if let Err(e) = outputs::write(&planned, None, None, Some(&r)) {
            if status == Status::Completed {
                return Err(e);
            }
            ctx.later_errors.borrow_mut().push(e);
        }
    }
    outcome
}

#[allow(clippy::too_many_arguments)]
fn build_report(
    a: &AnalyzeArgs,
    f: &Facts,
    progress: &Progress,
    status: Status,
    exit_code: u8,
    error: Option<&DimaError>,
    start: Instant,
    start_time: SystemTime,
) -> RunReport {
    let usage = resources::usage();
    let phases = match &f.execution {
        Some(e) => {
            let mut phases: Vec<PhaseTime> = e
                .phases
                .iter()
                .map(|(p, d)| PhaseTime::new(*p, *d))
                .collect();
            let writing = f.writing.or_else(|| {
                // Writing failed or was interrupted: until now.
                progress
                    .phase_started(Phase::Writing)
                    .map(|t| progress.elapsed().saturating_sub(t))
            });
            if let Some(w) = writing {
                phases.push(PhaseTime::new(Phase::Writing, w));
            }
            phases
        }
        None if f.analysis_started => reached_phases(progress),
        None => Vec::new(),
    };
    RunReport {
        schema_version: report::REPORT_SCHEMA_VERSION,
        tool: report::Tool {
            name: "dima",
            version: env!("CARGO_PKG_VERSION"),
            target: super::TARGET.to_owned(),
        },
        command: command(a),
        status,
        exit_code,
        error: error
            .filter(|e| !matches!(e, DimaError::Cancelled))
            .map(|e| report::error_text(e, &known_paths(a, true))),
        start_time: report::rfc3339(start_time),
        end_time: report::rfc3339(SystemTime::now()),
        threads: f.threads.as_ref().map(|t| report::Threads {
            count: t.count.get(),
            started: f
                .execution
                .as_ref()
                .map(|e| e.threads)
                .or_else(|| progress.threads_started()),
            source: match &t.source {
                threads::Source::Flag => "--threads",
                threads::Source::RayonEnv => "RAYON_NUM_THREADS",
                // The variable name only: its value may name this host (D103).
                threads::Source::Scheduler { variable, .. } => variable,
                threads::Source::Os => "available_parallelism",
            },
        }),
        platform: Platform::current(),
        resources: report::Resources {
            wall_seconds: start.elapsed().as_secs_f64(),
            cpu_user_seconds: Some(usage.user.as_secs_f64()),
            cpu_system_seconds: Some(usage.system.as_secs_f64()),
            peak_rss_bytes: usage.peak_rss_bytes,
            waited_for_input_seconds: f
                .execution
                .as_ref()
                .map(|e| e.waited_for_input.as_secs_f64()),
        },
        phases,
        work: report::Work {
            records: f
                .execution
                .as_ref()
                .map_or_else(|| progress.snapshot().records, |e| e.records),
            positions: f.execution.as_ref().map(|e| e.positions),
        },
        inputs: f.input.iter().cloned().collect(),
        outputs: f.outputs.clone(),
    }
}

/// The phases a run that ended early had reached, from the start times the library records;
/// the last one lasted until now.
fn reached_phases(progress: &Progress) -> Vec<PhaseTime> {
    let now = progress.elapsed();
    let started: Vec<(Phase, Duration)> = Phase::ALL
        .into_iter()
        .filter(|&p| p != Phase::Done)
        .filter_map(|p| progress.phase_started(p).map(|t| (p, t)))
        .collect();
    started
        .iter()
        .enumerate()
        .map(|(i, &(p, t))| {
            let end = started.get(i + 1).map_or(now, |&(_, next)| next);
            PhaseTime::new(p, end.saturating_sub(t))
        })
        .collect()
}

/// Every path given on the command line; with `run_report`, the run report's own path too.
fn known_paths(a: &AnalyzeArgs, run_report: bool) -> Vec<&Path> {
    [
        a.input.as_deref(),
        a.output.output.as_deref(),
        a.output.tsv_dir.as_deref(),
        a.hcs_output.as_deref(),
    ]
    .into_iter()
    .chain(run_report.then_some(a.run_report.as_deref()))
    .flatten()
    .filter(|p| p.as_os_str() != "-")
    .collect()
}

fn source_of(a: &AnalyzeArgs) -> Result<Source> {
    match &a.input {
        Some(p) if p.as_os_str() != "-" => Ok(Source::Path(p.clone())),
        Some(_) => Ok(Source::Stdin),
        None if std::io::stdin().is_terminal() => Err(DimaError::usage("no input alignment")
            .with_hint("give one with -i FILE, or pipe it in: dima analyze < alignment.fasta")),
        None => Ok(Source::Stdin),
    }
}

fn execute(
    a: &AnalyzeArgs,
    ctx: &Context,
    progress: &Arc<Progress>,
    facts: &mut Facts,
) -> Result<()> {
    let v = ctx.verbosity;
    let start = Instant::now();
    // Everything that can be checked before reading the input is checked first.
    let source = source_of(a)?;
    let params = params(a)?;
    params.validate()?;
    if let Some(format) = &params.header_format {
        HeaderSpec::new(
            format,
            params.metadata_fields.as_deref(),
            &params.header_fill().value,
        )?;
    }
    let hcs_threshold = match &a.hcs.hcs_min_incidence {
        Some(t) => Percent::hcs_threshold(t)?,
        None => dima_core::config::DEFAULT_HCS_THRESHOLD,
    };
    let tsv_base = a
        .query_name
        .clone()
        .or_else(|| input::query_name_from_source(&source))
        .unwrap_or_else(|| "dima".to_owned());
    let plan = outputs::plan(
        &a.output,
        a.hcs_output.as_deref(),
        a.run_report.as_deref(),
        &tsv_base,
        std::io::stdout().is_terminal(),
    )?;
    facts.stdout_taken = plan
        .outputs
        .iter()
        .any(|o| o.target == dima_core::output::Target::Stdout);
    let input_paths: Vec<&Path> = a.input.iter().map(PathBuf::as_path).collect();
    outputs::check_collisions(&plan, &input_paths)?;
    // Warnings raised here count in the summary line too (D104).
    let mut cli_warnings = warn_unused_flags(a, &plan, v);

    let available = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let host = threads::hostname();
    let t = threads::resolve(
        a.threads,
        &|k| std::env::var(k).ok(),
        available,
        host.as_deref(),
    )
    .map_err(|m| {
        DimaError::usage(m)
            .with_hint("set RAYON_NUM_THREADS to a whole number such as 4, or use --threads")
    })?;
    if t.count.get() > available {
        cli_warnings += 1;
        ui::warning(
            "threads_exceed_cpus",
            &format!(
                "{} threads requested but only {available} CPUs are available",
                t.count
            ),
            Some("more threads than CPUs slows the run down"),
            v,
        );
    }
    ui::verbose(&format!("threads: {t}"), v);
    if let Some(omp) = std::env::var("OMP_NUM_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
    {
        if omp < t.count.get() {
            ui::verbose(
                &format!("note: OMP_NUM_THREADS={omp} is ignored; use --threads to limit dima"),
                v,
            );
        }
    }
    let mode = progress::mode(a.progress(), v, ctx.redraw_terminal);
    let display = Display::new(Arc::clone(progress), mode);
    let control = RunControl {
        cancel: Arc::clone(&ctx.signals.cancel),
        progress: Arc::clone(progress),
        threads: Some(t.count),
        threads_ready: Some(Hook::new(display.launcher())),
        ..RunControl::new()
    };
    facts.threads = Some(t);
    // Every destination is opened before the analysis, so one that cannot be written is
    // reported now, not after a long computation.
    let opened = outputs::Opened::open(&plan, &ctx.signals.cancel)?;
    facts.analysis_started = true;
    let analysis = dima_core::analyze(&source, &params, &control)?;
    if let Some(t) = &facts.threads {
        cli_warnings += warn_refused_threads(&analysis.execution, t, v);
    }
    let results = analysis.results;
    facts.execution = Some(analysis.execution);
    facts.input = Some(FileRecord {
        role: "alignment",
        name: results.run.input.name.clone(),
        size_bytes: results.run.input.size_bytes,
        sha256: results.run.input.checksum.value.clone(),
    });
    display.writing();
    let writing_start = Instant::now();
    let hcs: Option<HcsResults> = if plan.needs_hcs() {
        Some(hcs::compute(
            &results,
            &HcsParams {
                threshold: hcs_threshold,
                eligible_only: a.hcs.hcs_eligible_only,
            },
        )?)
    } else {
        None
    };
    let written = opened.write(Some(&results), hcs.as_ref(), &|| control.check())?;
    for (p, w) in plan.outputs.iter().zip(written) {
        facts.outputs.push(FileRecord {
            role: p.content.role(),
            name: outputs::display_name(&p.target),
            size_bytes: w.bytes,
            sha256: w.sha256,
        });
        if let dima_core::output::Target::Path(path) = &p.target {
            ui::verbose(
                &format!("wrote {} ({})", path.display(), human::bytes(w.bytes)),
                v,
            );
        }
    }
    facts.writing = Some(writing_start.elapsed());
    display.finish();
    let mut warnings = cli_warnings;
    for d in results
        .run
        .diagnostics
        .iter()
        .chain(hcs.iter().flat_map(|h| &h.diagnostics))
    {
        if d.severity == dima_core::diagnostics::Severity::Warning {
            warnings += 1;
        }
        ui::diagnostic(d, v);
    }
    if plan.outputs.iter().any(|p| p.content == Content::Jsonl) {
        ui::note(
            "JSONL holds one position per line; provenance and summaries are in the JSON output",
            v,
        );
    }
    if params.validation().value == ValidationMode::Report {
        print_character_table(&results, v);
    }
    // The -v details come first, so the summary is always the last line of a log.
    if v.verbose() {
        performance(facts, v);
    }
    summary(&results, hcs.as_ref(), warnings, start.elapsed(), v);
    if ctx.stderr_is_terminal && v.normal() {
        let target = plan
            .outputs
            .iter()
            .find(|p| matches!(p.content, Content::Json { .. }))
            .and_then(|p| match &p.target {
                dima_core::output::Target::Path(path) => Some(path.display().to_string()),
                dima_core::output::Target::Stdout => None,
            });
        let how = target.map_or_else(
            || "`dima cite` prints the references".to_owned(),
            |t| format!("`dima cite --from {t}` lists this run's references"),
        );
        ui::line(
            &format!("Cite: Tharanga et al. 2025, doi:10.1093/bib/bbae607 ({how})"),
            v,
        );
    }
    Ok(())
}

/// Warns when the system refused some of the worker threads, so the run used fewer (D110);
/// returns how many warnings were printed.
fn warn_refused_threads(e: &dima_core::analysis::Execution, t: &Threads, v: Verbosity) -> u64 {
    if e.threads >= e.threads_requested {
        return 0;
    }
    ui::warning(
        "threads_refused",
        &threads::refused_message(e.threads, e.threads_requested, &t.source),
        Some(&format!(
            "results are the same with any number of threads; {}",
            dima_core::error::THREAD_LIMIT_HINT
        )),
        v,
    );
    1
}

/// Warns about options that have no effect with the chosen outputs; returns how many.
fn warn_unused_flags(a: &AnalyzeArgs, plan: &Plan, v: Verbosity) -> u64 {
    let mut count = 0;
    if !plan.needs_hcs() && (a.hcs.hcs_min_incidence.is_some() || a.hcs.hcs_eligible_only) {
        count += 1;
        ui::warning(
            "flag_has_no_effect",
            "--hcs-min-incidence/--hcs-eligible-only have no effect without an HCS output",
            Some("add --hcs-output PATH, --tsv-dir DIR or --table hcs"),
            v,
        );
    }
    if outputs::warn_unused_pretty(a.output.pretty, plan, v) {
        count += 1;
    }
    count
}

fn print_character_table(r: &Results, v: Verbosity) {
    ui::line("characters in sequence lines:", v);
    ui::line("  char   byte  class         count", v);
    for c in &r.run.validation.characters {
        ui::line(
            &format!(
                "  {:<5}  0x{:02X}  {:<12}  {}",
                // Characters without a printable form are identified by their byte alone.
                c.character.as_deref().unwrap_or(""),
                c.byte,
                c.class.as_str(),
                c.count
            ),
            v,
        );
    }
}

/// The end-of-run line (D104): the run's results as the paper reports them (eligible
/// positions, entropy mean, minimum and maximum; Tharanga et al. 2025, Results) and the wall
/// time; conserved regions when computed; the warning count only when there were warnings.
fn summary(
    r: &Results,
    hcs: Option<&HcsResults>,
    warnings: u64,
    elapsed: std::time::Duration,
    v: Verbosity,
) {
    use dima_core::human::separators::GROUP;
    let mut line = format!(
        "analysed {}, {} ({} eligible) in {}",
        human::plural(r.sequence_count, "sequence", "sequences"),
        human::plural(r.position_count, "position", "positions"),
        r.eligible_position_count,
        human::duration(elapsed),
    );
    line.push_str(GROUP);
    match (r.average_entropy, r.lowest_entropy, r.highest_entropy) {
        (Some(mean), Some(lo), Some(hi)) => line.push_str(&format!(
            "entropy mean {mean:.4}, min {:.4}, max {:.4} bits",
            lo.entropy, hi.entropy
        )),
        _ => line.push_str("no entropy summary (no eligible positions)"),
    }
    if let Some(h) = hcs {
        line.push_str(GROUP);
        line.push_str(&human::plural(
            h.region_count,
            "conserved region",
            "conserved regions",
        ));
    }
    if warnings > 0 {
        line.push_str(GROUP);
        line.push_str(&human::plural(warnings, "warning", "warnings"));
        line.push_str(" (see above)");
    }
    ui::line(&line, v);
}

fn performance(f: &Facts, v: Verbosity) {
    let u = resources::usage();
    if let Some(e) = &f.execution {
        if e.threads < e.threads_requested {
            ui::verbose(
                &format!(
                    "worker threads: {} of {} started",
                    e.threads, e.threads_requested
                ),
                v,
            );
        }
        let mut parts: Vec<String> = e
            .phases
            .iter()
            .map(|(p, d)| format!("{} {}", p.label(), human::duration(*d)))
            .collect();
        if let Some(w) = f.writing {
            parts.push(format!("writing {}", human::duration(w)));
        }
        ui::verbose(
            &format!("phases: {}", parts.join(human::separators::ITEM)),
            v,
        );
        let counting = e
            .phases
            .iter()
            .find(|(p, _)| *p == dima_core::control::Phase::Counting)
            .map(|(_, d)| *d);
        if let Some(c) = counting {
            if c.as_secs_f64() > 0.5 && e.waited_for_input.as_secs_f64() > 0.5 * c.as_secs_f64() {
                ui::verbose(
                    &format!(
                        "reading the input limited the run (waited {} of {}); zstd decompresses fastest",
                        human::duration(e.waited_for_input),
                        human::duration(c)
                    ),
                    v,
                );
            }
        }
    }
    let peak = u
        .peak_rss_bytes
        .map_or_else(|| "unknown".to_owned(), human::bytes);
    ui::verbose(
        &format!(
            "CPU {} user + {} system{}peak memory {peak}",
            human::duration(u.user),
            human::duration(u.system),
            human::separators::ITEM
        ),
        v,
    );
}
