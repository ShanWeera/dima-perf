//! The pure state machine: `update(&mut AppState, Msg) -> Vec<Effect>`.
//!
//! Every user action and worker result goes through here; nothing here draws, blocks, spawns
//! or touches the file system (the returned effects do that), so every transition is unit
//! tested without a window. Time enters only through `Msg::Tick`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dima_core::error::IoAction;
use dima_core::run_report::{FileRecord, Status};

use super::msg::{Effect, Msg};
use crate::charts::metric::ChartMetric;
use crate::charts::scene::{ChartScene, SceneStyle};
use crate::charts::transform::y_axis_max;
use crate::state::header_editor::HeaderEditor;
use crate::state::options::AnalysisOptions;
use crate::state::recent::{RecentFile, RecentKind, now_unix};
use crate::state::session::{AnalysedRun, Origin, SelectSource, Session};
use crate::state::settings::DialogMode;
use crate::state::toasts::Severity;
use crate::state::{AppState, Confirm, ErrorCard, FailedRun, Purpose, RunState, View};
use crate::workers::analysis::{FileStamp, Outcome, Request, cpus, worker_threads};
use crate::workers::dialogs::DialogKind;
use crate::workers::export::{ExportKind, Job, Payload};
use crate::workers::inspect::Opened;
use dima_core::human::{self, plural, separators::ITEM};
use dima_core::output::file_stem;

/// Applies one event.
pub fn update(state: &mut AppState, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::OpenDialog => open_dialog(state, Purpose::OpenInput),
        Msg::OpenPath(path) => open_path(state, path),
        Msg::Dropped(paths) => dropped(state, paths),
        Msg::Confirmed(yes) => confirmed(state, yes),
        Msg::CloseRequested => {
            if state.export_busy.is_some() {
                state.confirm = Some(Confirm::QuitDuringExport);
                vec![Effect::CancelClose]
            } else {
                Vec::new()
            }
        }
        Msg::Opened {
            generation,
            path,
            outcome,
        } => opened(state, generation, path, outcome),
        Msg::CancelOpen => {
            state.setup.opening = None;
            vec![Effect::CancelOpen]
        }
        Msg::RecentRemoved(path) => {
            if state.recent.remove(&path) {
                vec![Effect::SaveRecent(state.recent.to_json())]
            } else {
                Vec::new()
            }
        }
        Msg::RecentCleared => {
            if state.recent.clear() {
                vec![Effect::SaveRecent(state.recent.to_json())]
            } else {
                Vec::new()
            }
        }
        Msg::OptionsChanged => save_settings(state),
        Msg::ResetOptions => {
            state.setup.options = AnalysisOptions::default();
            state.settings.threads = None;
            state.hcs.text = dima_core::config::DEFAULT_HCS_THRESHOLD.to_string();
            state.hcs.params.eligible_only = false;
            state.hcs.edited(state.now);
            save_settings(state)
        }
        Msg::HeaderPasted(text) => {
            state.setup.header_note = state.setup.header.paste(&text);
            Vec::new()
        }
        Msg::Analyze => analyze(state),
        Msg::CancelAnalysis => match state.setup.run {
            RunState::Running(g) => {
                state.setup.run = RunState::Cancelling(g);
                vec![Effect::CancelAnalysis]
            }
            _ => Vec::new(),
        },
        Msg::AnalysisDone {
            generation,
            outcome,
        } => analysis_done(state, generation, outcome),
        Msg::ErrorContext {
            generation,
            excerpt,
        } => {
            if let Some(card) = state.setup.failure.as_mut()
                && card.excerpt_pending == Some(generation)
            {
                card.excerpt_pending = None;
                card.excerpt = excerpt;
            }
            Vec::new()
        }
        Msg::SaveFailedRunReport => {
            if state.export_busy.is_some() {
                return Vec::new();
            }
            open_dialog(state, Purpose::FailedRunReport)
        }
        Msg::DismissFailure => {
            state.setup.failure = None;
            state.setup.open_error = None;
            Vec::new()
        }
        Msg::ShowView(v) => {
            if v == View::Setup || state.session.is_some() {
                state.view = v;
            }
            Vec::new()
        }
        Msg::FiltersEdited => {
            if let Some(s) = state.session.as_mut() {
                s.filter_dirty = true;
            }
            Vec::new()
        }
        Msg::ChartMetric(metric) => chart_metric(state, metric),
        Msg::ResetFilters => {
            if let Some(s) = state.session.as_mut() {
                s.reset_filters();
            }
            Vec::new()
        }
        Msg::Select(position, source) => select(state, position, source),
        Msg::SelectVariant(i) => {
            if let Some(s) = state.session.as_mut()
                && s.selected().is_some_and(|p| i < p.diversity_motifs.len())
            {
                s.selection.variant = Some(i);
            }
            Vec::new()
        }
        Msg::Navigate(nav) => {
            if let Some(s) = state.session.as_mut() {
                s.navigate(nav);
            }
            Vec::new()
        }
        Msg::SortPositions(column) => {
            if let Some(s) = state.session.as_mut() {
                s.sort_by(column);
            }
            Vec::new()
        }
        Msg::SortVariants(column) => {
            if let Some(s) = state.session.as_mut() {
                s.sort_variants_by(column);
            }
            Vec::new()
        }
        Msg::HcsEdited => {
            state.hcs.edited(state.now);
            save_settings(state)
        }
        Msg::HcsDone {
            generation,
            outcome,
        } => {
            if let Some((g, session, params)) = state.hcs.running
                && g == generation
            {
                state.hcs.running = None;
                match outcome {
                    Ok(r) => {
                        state.hcs.failure = None;
                        state.hcs.result = Some((session, params, r));
                    }
                    Err(e) => state.hcs.failure = Some(e.report()),
                }
            }
            Vec::new()
        }
        Msg::ThreadsChanged(threads) => {
            state.settings.threads = threads;
            save_settings(state)
        }
        Msg::Export(kind) => request_export(state, kind),
        Msg::ExportDone {
            kind,
            metric,
            session,
            outcome,
        } => export_done(state, kind, metric, session, outcome),
        Msg::DialogClosed {
            purpose,
            path,
            failed_fast,
        } => dialog_closed(state, purpose, path, failed_fast),
        Msg::UseBuiltInBrowser => match state.dialog.pending {
            Some(purpose) if state.dialog.native => {
                state.dialog.native = false;
                vec![dialog_effect(state, purpose, true)]
            }
            _ => Vec::new(),
        },
        Msg::Tick { now, pointer_down } => tick(state, now, pointer_down),
        Msg::Notify(severity, text) => {
            state.toasts.push(severity, text, None, state.now);
            Vec::new()
        }
        Msg::DismissToast(id) => {
            state.toasts.dismiss(id);
            Vec::new()
        }
        Msg::Panicked { what, message } => {
            job_stopped(state, what);
            state.toasts.push(
                Severity::Error,
                format!("Internal error while {what}. Nothing was written. Please report it (Copy details)."),
                Some(crate::util::internal_error_details(what, &message)),
                state.now,
            );
            Vec::new()
        }
        Msg::Refused { what, reason } => {
            job_stopped(state, what);
            state.toasts.push(
                Severity::Error,
                format!(
                    "Could not start {what}: the system refused a new thread ({reason}). Nothing \
                     was written. Close other programs or lower the number of threads in the \
                     settings, then try again."
                ),
                None,
                state.now,
            );
            Vec::new()
        }
    }
}

/// Clears the busy state of a job that ended without a result.
fn job_stopped(state: &mut AppState, what: &str) {
    match what {
        "analysing" => state.setup.run = RunState::Idle,
        "opening the file" => state.setup.opening = None,
        "exporting" => state.export_busy = None,
        "computing conserved regions" => state.hcs.running = None,
        "showing the file dialog" => state.dialog.pending = None,
        _ => {}
    }
}

fn save_settings(state: &mut AppState) -> Vec<Effect> {
    state.settings = state.settings_snapshot();
    vec![Effect::SaveSettings(state.settings.to_json())]
}

fn name_of(path: &Path) -> String {
    dima_core::run_report::file_name(path)
}

fn dialog_effect(state: &AppState, purpose: Purpose, built_in: bool) -> Effect {
    let query = state
        .session
        .as_ref()
        .map_or_else(|| "dima".to_owned(), |s| s.results.query_name.clone());
    let (kind, title) = match purpose {
        Purpose::OpenInput => (
            DialogKind::Open,
            "Open an alignment or a DiMA results file".to_owned(),
        ),
        Purpose::Export {
            kind: ExportKind::TsvFolder,
            ..
        } => (
            DialogKind::Folder,
            "Choose a folder for the tables".to_owned(),
        ),
        Purpose::Export { kind, metric, .. } => (
            DialogKind::Save {
                default_name: kind.default_name(&query, metric),
                filter: kind.extension().map(|_| {
                    // Native filters match the last extension; compound names (.json.gz) pass.
                    let mut exts: Vec<String> = kind
                        .accepted()
                        .iter()
                        .map(|a| a.rsplit('.').next().unwrap_or(a).to_owned())
                        .collect();
                    exts.sort();
                    exts.dedup();
                    (
                        kind.label(metric).trim_end_matches('\u{2026}').to_owned(),
                        exts,
                    )
                }),
            },
            format!("Save {}", kind.label(metric).trim_end_matches('\u{2026}')),
        ),
        Purpose::FailedRunReport => (
            DialogKind::Save {
                default_name: format!(
                    "{}.run-report.json",
                    state
                        .setup
                        .file
                        .as_deref()
                        .map_or_else(|| "dima".to_owned(), |p| file_stem(&name_of(p)))
                ),
                filter: Some(("Run report (JSON)".to_owned(), vec!["json".to_owned()])),
            },
            "Save run report".to_owned(),
        ),
    };
    Effect::Dialog {
        purpose,
        kind,
        title,
        built_in,
    }
}

fn open_dialog(state: &mut AppState, purpose: Purpose) -> Vec<Effect> {
    if state.dialog.pending.is_some() {
        // A shortcut or the command palette: the buttons are disabled meanwhile.
        state.toasts.push(
            Severity::Info,
            "A file dialog is already open; choose a file there or cancel it first",
            None,
            state.now,
        );
        return Vec::new();
    }
    let built_in = state.settings.dialogs == DialogMode::BuiltIn || !state.native_dialogs;
    state.dialog.pending = Some(purpose);
    state.dialog.native = !built_in;
    vec![dialog_effect(state, purpose, built_in)]
}

fn open_path(state: &mut AppState, path: PathBuf) -> Vec<Effect> {
    if state.running() {
        state.confirm = Some(Confirm::ReplaceRun(path));
        return Vec::new();
    }
    start_open(state, path, false)
}

fn start_open(state: &mut AppState, path: PathBuf, reinspect: bool) -> Vec<Effect> {
    let generation = state.next_generation();
    state.setup.path_text = path.display().to_string();
    state.setup.opening = Some((generation, path.clone()));
    state.setup.reinspecting = reinspect;
    state.setup.open_error = None;
    vec![Effect::Open { generation, path }]
}

fn dropped(state: &mut AppState, paths: Vec<PathBuf>) -> Vec<Effect> {
    let Some(first) = paths.first().cloned() else {
        return Vec::new();
    };
    let note = if paths.len() > 1 {
        format!("One file at a time: opening {}", name_of(&first))
    } else {
        format!("Opening {}", name_of(&first))
    };
    // Feedback in either view (G32); an alignment then shows on the setup screen.
    state.toasts.push(Severity::Info, note, None, state.now);
    open_path(state, first)
}

fn confirmed(state: &mut AppState, yes: bool) -> Vec<Effect> {
    match state.confirm.take() {
        Some(Confirm::ReplaceRun(path)) if yes => {
            let mut effects = Vec::new();
            if let RunState::Running(g) = state.setup.run {
                state.setup.run = RunState::Cancelling(g);
                effects.push(Effect::CancelAnalysis);
            }
            effects.extend(start_open(state, path, false));
            effects
        }
        Some(Confirm::QuitDuringExport) if yes => vec![Effect::Quit],
        Some(Confirm::Overwrite { purpose, path }) if yes => write_chosen(state, purpose, path),
        Some(Confirm::ClearFiltersFor(position)) if yes => {
            if let Some(s) = state.session.as_mut() {
                s.reset_filters();
                s.select(position, SelectSource::Palette);
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn recent_entry(
    path: &Path,
    kind: RecentKind,
    sequences: Option<u64>,
    estimated: bool,
    alphabet: Option<String>,
) -> RecentFile {
    RecentFile {
        path: path.to_path_buf(),
        opened: now_unix(),
        kind,
        sequences,
        estimated,
        alphabet,
    }
}

fn opened(
    state: &mut AppState,
    generation: u64,
    path: PathBuf,
    outcome: Result<Opened, dima_core::DimaError>,
) -> Vec<Effect> {
    if state.setup.opening.as_ref().map(|(g, _)| *g) != Some(generation) {
        return Vec::new();
    }
    state.setup.opening = None;
    let reinspect = std::mem::take(&mut state.setup.reinspecting);
    match outcome {
        Ok(Opened::Alignment(inspection)) => {
            let entry = recent_entry(
                &path,
                RecentKind::Alignment,
                inspection.estimated_records,
                !inspection.complete,
                inspection.alphabet.map(|a| a.as_str().to_owned()),
            );
            let header = HeaderEditor::from_inspection(
                &inspection.header_fields,
                inspection.header_field_count_mismatches,
            );
            let same_layout = reinspect && header.rows.len() == state.setup.header.rows.len();
            if same_layout {
                // Re-inspected after a change (G65): keep the user's names and choices.
                for (old, new) in state.setup.header.rows.iter_mut().zip(header.rows) {
                    old.samples = new.samples;
                    old.identifier_like = new.identifier_like;
                }
                state.setup.header.mismatched_headers = header.mismatched_headers;
            } else {
                state.setup.header = header;
                state.setup.options.reset_for_new_file();
            }
            state.setup.header_note = None;
            state.setup.file = Some(path);
            state.setup.inspection = Some(inspection);
            state.setup.failure = None;
            state.setup.open_error = None;
            state.view = View::Setup;
            let mut effects = Vec::new();
            if state.recent.add(entry) {
                effects.push(Effect::SaveRecent(state.recent.to_json()));
            }
            effects
        }
        Ok(Opened::Results(results)) => {
            let entry = recent_entry(
                &path,
                RecentKind::Results,
                Some(results.sequence_count),
                false,
                Some(results.run.parameters.alphabet.value.as_str().to_owned()),
            );
            // The setup screen described another file: clear it (G31).
            state.setup.file = None;
            state.setup.inspection = None;
            state.setup.failure = None;
            state.setup.header = HeaderEditor::default();
            state.setup.options.reset_for_new_file();
            state.toasts.push(
                Severity::Success,
                format!("Opened {}", name_of(&path)),
                None,
                state.now,
            );
            let mut effects = start_session(state, Arc::new(*results), Origin::Opened(path));
            if state.recent.add(entry) {
                effects.push(Effect::SaveRecent(state.recent.to_json()));
            }
            effects
        }
        Err(error) => {
            let missing = matches!(
                &error,
                dima_core::DimaError::Io { action: IoAction::Open, source, .. }
                    if source.kind() == std::io::ErrorKind::NotFound
            );
            state.setup.file = None;
            state.setup.inspection = None;
            state.setup.open_error = Some(ErrorCard::new(
                format!("{} could not be opened", name_of(&path)),
                error,
            ));
            state.view = View::Setup;
            let mut effects = Vec::new();
            if missing && state.recent.remove(&path) {
                effects.push(Effect::SaveRecent(state.recent.to_json()));
            }
            effects
        }
    }
}

fn start_session(
    state: &mut AppState,
    results: Arc<dima_core::model::Results>,
    origin: Origin,
) -> Vec<Effect> {
    let id = state.next_generation();
    state.session = Some(Session::new(id, results, origin));
    state.view = View::Workspace;
    state.windows.diagnostics = false;
    state.windows.run_details = false;
    state.hcs.pending_since = None;
    start_hcs(state)
}

fn start_hcs(state: &mut AppState) -> Vec<Effect> {
    let Some(session) = state.session.as_ref() else {
        return Vec::new();
    };
    let (id, results) = (session.id, Arc::clone(&session.results));
    let generation = state.next_generation();
    let params = state.hcs.params;
    state.hcs.pending_since = None;
    state.hcs.running = Some((generation, id, params));
    vec![Effect::ComputeHcs {
        generation,
        results,
        params,
    }]
}

fn analyze(state: &mut AppState) -> Vec<Effect> {
    if state.running() {
        return Vec::new();
    }
    let Some(path) = state.setup.file.clone() else {
        return Vec::new();
    };
    let alignment_length = state.setup.inspection.as_ref().map(|i| i.alignment_length);
    let problems = state
        .setup
        .options
        .problems(&state.setup.header, alignment_length);
    if let Some(first) = problems.first() {
        state.toasts.push(
            Severity::Warning,
            format!("Not started: {first}"),
            None,
            state.now,
        );
        return Vec::new();
    }
    let request = Request {
        path,
        params: state.setup.options.to_params(&state.setup.header),
        threads: worker_threads(state.settings.threads, cpus()),
        expected: state.setup.inspection.as_ref().map(|i| FileStamp {
            size: i.size_bytes,
            modified: i.modified,
        }),
        expected_probe: state.setup.inspection.as_ref().map(|i| i.probe.clone()),
    };
    let generation = state.next_generation();
    state.setup.run = RunState::Running(generation);
    state.setup.failure = None;
    state.setup.in_flight = Some(request.clone());
    vec![Effect::Analyze {
        generation,
        request: Box::new(request),
    }]
}

fn analysis_done(state: &mut AppState, generation: u64, outcome: Outcome) -> Vec<Effect> {
    let matches = matches!(state.setup.run, RunState::Running(g) | RunState::Cancelling(g) if g == generation);
    if !matches {
        return Vec::new();
    }
    // A run that ends after the user cancelled it is cancelled, however it ended: its results,
    // error or changed file must not replace what the user went on to do (G35).
    let outcome = if matches!(state.setup.run, RunState::Cancelling(_)) {
        Outcome::Cancelled
    } else {
        outcome
    };
    state.setup.run = RunState::Idle;
    let Some(request) = state.setup.in_flight.take() else {
        return Vec::new();
    };
    match outcome {
        Outcome::Completed(analysis, facts) => {
            let results = Arc::new(analysis.results);
            state.toasts.push(
                Severity::Success,
                format!(
                    "Analysed {}{ITEM}{} in {}",
                    plural(results.sequence_count, "sequence", "sequences"),
                    plural(results.position_count, "position", "positions"),
                    human::duration(facts.wall)
                ),
                None,
                state.now,
            );
            if let Some(started) = facts.threads_started.filter(|&n| n < facts.threads) {
                state.toasts.push(
                    Severity::Warning,
                    format!(
                        "Used {started} of {} worker threads: the system refused to start more. \
                         Results are the same with any number of threads; to avoid this, close \
                         other programs or lower the number of threads in the settings.",
                        facts.threads
                    ),
                    None,
                    state.now,
                );
            }
            let mut effects = Vec::new();
            let entry = recent_entry(
                &request.path,
                RecentKind::Alignment,
                Some(results.sequence_count),
                false,
                Some(results.run.alphabet.resolved.as_str().to_owned()),
            );
            if state.recent.add(entry) {
                effects.push(Effect::SaveRecent(state.recent.to_json()));
            }
            let origin = Origin::Analysed(Box::new(AnalysedRun {
                path: request.path,
                params: request.params,
                facts,
            }));
            effects.extend(start_session(state, results, origin));
            effects
        }
        Outcome::Failed(error, facts) => {
            let mut card = ErrorCard::new(
                "The analysis stopped \u{2014} no results were written",
                error,
            );
            card.failed_run = Some(FailedRun {
                path: request.path.clone(),
                params: request.params,
                facts,
            });
            let mut effects = Vec::new();
            if let Some(record) = card.location.as_ref().and_then(|l| l.record) {
                let g = state.next_generation();
                card.excerpt_pending = Some(g);
                effects.push(Effect::ReadErrorContext {
                    generation: g,
                    path: request.path,
                    record,
                    column: card.location.as_ref().and_then(|l| l.column),
                });
            }
            state.setup.failure = Some(card);
            state.view = View::Setup;
            effects
        }
        Outcome::Cancelled => {
            state.toasts.push(
                Severity::Info,
                "Analysis cancelled \u{2014} no results were written",
                None,
                state.now,
            );
            Vec::new()
        }
        Outcome::Changed => {
            state.toasts.push(
                Severity::Warning,
                format!(
                    "{} changed after it was checked. It was checked again: review the settings and press Analyze.",
                    name_of(&request.path)
                ),
                None,
                state.now,
            );
            start_open(state, request.path, true)
        }
    }
}

fn select(state: &mut AppState, position: u64, source: SelectSource) -> Vec<Effect> {
    let Some(s) = state.session.as_mut() else {
        return Vec::new();
    };
    if s.position(position).is_none() {
        state.toasts.push(
            Severity::Warning,
            format!("There is no position {position} in these results"),
            None,
            state.now,
        );
        return Vec::new();
    }
    if !s.is_shown(position) {
        // Never select something the views do not show (G20): offer to clear the filters.
        state.confirm = Some(Confirm::ClearFiltersFor(position));
        return Vec::new();
    }
    s.select(position, source);
    Vec::new()
}

/// Switches the chart to `metric`: refused when the results do not hold it, nothing to do
/// when it is shown already. A value range the user had narrowed is reset to the new metric's
/// full range, and they are told.
fn chart_metric(state: &mut AppState, metric: ChartMetric) -> Vec<Effect> {
    let Some(s) = state.session.as_mut() else {
        return Vec::new();
    };
    if s.metric == metric || !metric.available(&s.results) {
        return Vec::new();
    }
    if s.set_metric(metric) {
        state.toasts.push(
            Severity::Info,
            format!(
                "Value range reset to the full range of {}",
                metric.in_sentence()
            ),
            None,
            state.now,
        );
    }
    Vec::new()
}

/// The figure of `metric` for the current view of a session: `None` when nothing is shown, or
/// when the chart shows another metric (its series is the only one there is).
#[must_use]
pub fn chart_scene(state: &AppState, metric: ChartMetric) -> Option<ChartScene> {
    let s = state.session.as_ref()?;
    if s.series.is_empty() || s.metric != metric {
        return None;
    }
    let full = s.extent()?;
    let view = s.viewport.unwrap_or(full);
    let r = &s.results;
    let (width, height) = state.settings.figure_size;
    Some(ChartScene {
        title: metric.figure_title(&r.query_name),
        subtitle: metric.figure_subtitle(r, s.shown.len()),
        y_title: metric.figure_axis_title().to_owned(),
        width,
        height,
        series: s.series.as_ref().clone(),
        view,
        y_max: y_axis_max(s.series.y_max),
        mean: metric.mean(r),
        legend: crate::charts::series::legend(
            r.support_threshold,
            r.run.parameters.estimator.value,
            metric,
        ),
        min_coverage: r.coverage_summary.min_coverage,
        style: SceneStyle::publication(),
    })
}

fn request_export(state: &mut AppState, kind: ExportKind) -> Vec<Effect> {
    if state.export_busy.is_some() {
        state.toasts.push(
            Severity::Info,
            "An export is already running; try again when it finishes",
            None,
            state.now,
        );
        return Vec::new();
    }
    let Some(s) = state.session.as_ref() else {
        return Vec::new();
    };
    if kind == ExportKind::RunReport && !matches!(s.origin, Origin::Analysed(_)) {
        state.toasts.push(
            Severity::Info,
            "A run report describes an analysis run in this app; these results were opened from a file",
            None,
            state.now,
        );
        return Vec::new();
    }
    if matches!(kind, ExportKind::Png | ExportKind::Svg) && s.series.is_empty() {
        state.toasts.push(
            Severity::Warning,
            "No positions are shown, so there is no chart to export (check the filters)",
            None,
            state.now,
        );
        return Vec::new();
    }
    let (session, metric) = (s.id, s.metric);
    open_dialog(
        state,
        Purpose::Export {
            kind,
            session,
            metric,
        },
    )
}

fn export_job(
    state: &AppState,
    kind: ExportKind,
    path: PathBuf,
    metric: ChartMetric,
) -> Option<Job> {
    let s = state.session.as_ref()?;
    let payload = match kind {
        ExportKind::Png | ExportKind::Svg => Payload::Figure(Box::new(chart_scene(state, metric)?)),
        ExportKind::RunReport => {
            let Origin::Analysed(run) = &s.origin else {
                return None;
            };
            Payload::Report(Box::new(crate::report::build(
                &run.path,
                &run.params,
                &run.facts,
                Status::Completed,
                None,
                Some(&s.results),
                &s.exports,
            )))
        }
        _ => Payload::Results {
            results: Arc::clone(&s.results),
            hcs: state.hcs.params,
        },
    };
    Some(Job {
        kind,
        metric,
        path,
        payload,
    })
}

fn dialog_closed(
    state: &mut AppState,
    purpose: Purpose,
    path: Option<PathBuf>,
    failed_fast: bool,
) -> Vec<Effect> {
    if state.dialog.pending != Some(purpose) {
        return Vec::new();
    }
    let was_native = state.dialog.native;
    state.dialog.pending = None;
    state.dialog.native = false;
    let Some(path) = path else {
        if failed_fast && was_native {
            state.toasts.push(
                Severity::Info,
                "The system file dialog is not available here; using the built-in browser",
                None,
                state.now,
            );
            state.dialog.pending = Some(purpose);
            return vec![dialog_effect(state, purpose, true)];
        }
        return Vec::new();
    };
    let kind = match purpose {
        Purpose::OpenInput => return open_path(state, path),
        Purpose::Export { kind, .. } => kind,
        Purpose::FailedRunReport => ExportKind::RunReport,
    };
    // The dialog asked about the name as typed; when the extension is added to it, ask about
    // the file that would actually be replaced (D100). A read-only look at the file system.
    let full = kind.with_extension(path.clone());
    if full != path && full.exists() {
        state.confirm = Some(Confirm::Overwrite {
            purpose,
            path: full,
        });
        return Vec::new();
    }
    write_chosen(state, purpose, full)
}

/// Writes an export to the destination chosen for it (with its extension).
fn write_chosen(state: &mut AppState, purpose: Purpose, path: PathBuf) -> Vec<Effect> {
    match purpose {
        Purpose::OpenInput => open_path(state, path),
        Purpose::Export {
            kind,
            session,
            metric,
        } => {
            // Other results may have been loaded while the dialog was open; never write them
            // under the name chosen for the earlier ones.
            let Some(shown) = state
                .session
                .as_ref()
                .filter(|s| s.id == session)
                .map(|s| s.metric)
            else {
                state.toasts.push(
                    Severity::Warning,
                    "Nothing was saved: other results were loaded while the dialog was open. \
                     Export again to save the results now shown",
                    None,
                    state.now,
                );
                return Vec::new();
            };
            // Nor a figure of another metric under the name chosen for this one (the switch
            // stays usable through a shortcut while a dialog is open).
            if kind.is_figure() && shown != metric {
                state.toasts.push(
                    Severity::Warning,
                    format!(
                        "Nothing was saved: the chart was switched to {} while the dialog was \
                         open. Export again to save the chart now shown.",
                        shown.in_sentence()
                    ),
                    None,
                    state.now,
                );
                return Vec::new();
            }
            match export_job(state, kind, path, metric) {
                Some(job) => {
                    state.export_busy = Some(kind);
                    vec![Effect::Export {
                        session: Some(session),
                        job: Box::new(job),
                    }]
                }
                None => Vec::new(),
            }
        }
        Purpose::FailedRunReport => {
            let Some(run) = state
                .setup
                .failure
                .as_ref()
                .and_then(|c| c.failed_run.as_ref().map(|r| (c, r)))
            else {
                return Vec::new();
            };
            let (card, run) = run;
            let report = crate::report::build(
                &run.path,
                &run.params,
                &run.facts,
                Status::Failed,
                Some(&card.error),
                None,
                &[],
            );
            state.export_busy = Some(ExportKind::RunReport);
            vec![Effect::Export {
                session: None,
                job: Box::new(Job {
                    kind: ExportKind::RunReport,
                    metric: ChartMetric::Entropy,
                    path,
                    payload: Payload::Report(Box::new(report)),
                }),
            }]
        }
    }
}

fn export_done(
    state: &mut AppState,
    kind: ExportKind,
    metric: ChartMetric,
    session: Option<u64>,
    outcome: crate::workers::export::Outcome,
) -> Vec<Effect> {
    state.export_busy = None;
    // Nothing is lost by closing any more.
    if state.confirm == Some(Confirm::QuitDuringExport) {
        state.confirm = None;
    }
    match outcome {
        Ok(files) => {
            // The files belong to the run report of the session they were exported from, which
            // may have been replaced meanwhile.
            if let Some(s) = state.session.as_mut().filter(|s| session == Some(s.id)) {
                for f in &files {
                    if let Some(role) = f.role {
                        s.record_export(FileRecord {
                            role,
                            name: name_of(&f.path),
                            size_bytes: f.bytes,
                            sha256: f.sha256.clone(),
                        });
                    }
                }
            }
            let text = match files.as_slice() {
                [one] => format!("Saved {}", one.path.display()),
                many => format!(
                    "Saved {} files to {}",
                    many.len(),
                    many.first()
                        .and_then(|f| f.path.parent())
                        .map_or_else(String::new, |p| p.display().to_string())
                ),
            };
            state.toasts.push(Severity::Success, text, None, state.now);
        }
        Err(e) => state.toasts.push(
            Severity::Error,
            format!(
                "{} failed: {}",
                kind.label(metric).trim_end_matches('\u{2026}'),
                e
            ),
            Some(e.report()),
            state.now,
        ),
    }
    Vec::new()
}

fn tick(state: &mut AppState, now: std::time::Instant, pointer_down: bool) -> Vec<Effect> {
    state.now = now;
    state.toasts.tick(now);
    if let Some(s) = state.session.as_mut()
        && s.filter_dirty
        && !pointer_down
    {
        s.apply_filters();
    }
    if state.hcs.due(now, pointer_down) {
        if state.session.is_some() {
            return start_hcs(state);
        }
        // Nothing to compute yet: loading results computes the regions.
        state.hcs.pending_since = None;
    }
    Vec::new()
}

#[cfg(test)]
mod tests;
