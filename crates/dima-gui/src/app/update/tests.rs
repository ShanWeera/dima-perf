//! Unit tests of the pure state machine: every transition without a window or a thread.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use dima_core::control::Progress;
use dima_core::input::Source;

use super::*;
use crate::state::recent::RecentFiles;
use crate::state::settings::Settings;
use crate::state::toasts::Severity;
use crate::tests::fixtures;
use crate::workers::analysis::{self, Outcome, Request};
use crate::workers::inspect::Opened;

fn state() -> AppState {
    AppState::new(Settings::default(), RecentFiles::default())
}

fn opened_alignment(state: &mut AppState) -> Vec<Effect> {
    let path = fixtures::mers_path();
    let effects = update(state, Msg::OpenPath(path.clone()));
    let Some(Effect::Open { generation, .. }) = effects.first() else {
        panic!("an open effect");
    };
    let inspection = dima_core::validation::inspect(&Source::Path(path.clone())).unwrap();
    update(
        state,
        Msg::Opened {
            generation: *generation,
            path,
            outcome: Ok(Opened::Alignment(Box::new(inspection))),
        },
    )
}

fn with_session(state: &mut AppState) {
    with_results(state, &fixtures::mers());
}

/// Opens `results` as if read from a results file.
fn with_results(state: &mut AppState, results: &dima_core::model::Results) {
    let effects = update(state, Msg::OpenPath(PathBuf::from("results.json")));
    let Some(Effect::Open { generation, .. }) = effects.first() else {
        panic!("an open effect");
    };
    update(
        state,
        Msg::Opened {
            generation: *generation,
            path: PathBuf::from("results.json"),
            outcome: Ok(Opened::Results(Box::new(results.clone()))),
        },
    );
}

fn tick(state: &mut AppState, after: Duration, pointer_down: bool) -> Vec<Effect> {
    let now = state.now + after;
    update(state, Msg::Tick { now, pointer_down })
}

#[test]
fn opening_an_alignment_resets_the_per_file_options_only() {
    let mut s = state();
    s.setup.options.alphabet = crate::state::options::AlphabetChoice::Dna;
    s.setup.options.kmer_length = Some(5);
    s.setup.options.query_name = "old".into();
    s.setup.options.support_threshold = 30;
    let effects = opened_alignment(&mut s);
    assert!(matches!(effects.as_slice(), [Effect::SaveRecent(_)]));
    assert_eq!(s.setup.file, Some(fixtures::mers_path()));
    assert_eq!(
        s.setup.options.alphabet,
        crate::state::options::AlphabetChoice::Auto,
        "G12"
    );
    assert_eq!(s.setup.options.kmer_length, None, "G12");
    assert!(s.setup.options.query_name.is_empty(), "G13");
    assert_eq!(
        s.setup.options.support_threshold, 30,
        "remembered options stay"
    );
    assert_eq!(
        s.setup.header.names(),
        ["field_1", "field_2", "field_3", "field_4"],
        "G36"
    );
    assert!(!s.setup.header.enabled);
    assert_eq!(s.recent.files.len(), 1);
    assert_eq!(s.view, View::Setup);
}

#[test]
fn stale_open_results_are_ignored() {
    let mut s = state();
    update(&mut s, Msg::OpenPath(PathBuf::from("a.fa")));
    let effects = update(
        &mut s,
        Msg::Opened {
            generation: 999,
            path: PathBuf::from("a.fa"),
            outcome: Err(dima_core::DimaError::usage("x")),
        },
    );
    assert!(effects.is_empty());
    assert!(
        s.setup.opening.is_some(),
        "still waiting for the current job"
    );
    assert!(s.setup.open_error.is_none());
    update(&mut s, Msg::CancelOpen);
    assert!(s.setup.opening.is_none());
}

#[test]
fn opening_results_starts_a_session_and_clears_the_setup() {
    let mut s = state();
    opened_alignment(&mut s);
    s.setup.failure = Some(ErrorCard::new("x", dima_core::DimaError::usage("x")));
    let effects = update(&mut s, Msg::OpenPath(PathBuf::from("r.json")));
    let Some(Effect::Open { generation, .. }) = effects.first() else {
        panic!();
    };
    let effects = update(
        &mut s,
        Msg::Opened {
            generation: *generation,
            path: PathBuf::from("r.json"),
            outcome: Ok(Opened::Results(Box::new((*fixtures::mers()).clone()))),
        },
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::ComputeHcs { .. }))
    );
    assert_eq!(s.view, View::Workspace);
    assert!(
        s.setup.file.is_none() && s.setup.inspection.is_none(),
        "G31"
    );
    assert!(s.setup.failure.is_none());
    let session = s.session.as_ref().unwrap();
    assert_eq!(
        session.selection.position,
        Some(1),
        "first position selected"
    );
    assert!(matches!(session.origin, Origin::Opened(_)));
}

#[test]
fn a_missing_file_is_reported_and_forgotten() {
    let mut s = state();
    let path = PathBuf::from("/definitely/missing.fasta");
    s.recent.add(crate::state::recent::RecentFile {
        path: path.clone(),
        opened: 1,
        kind: crate::state::recent::RecentKind::Alignment,
        sequences: None,
        estimated: false,
        alphabet: None,
    });
    let effects = update(&mut s, Msg::OpenPath(path.clone()));
    let Some(Effect::Open { generation, .. }) = effects.first() else {
        panic!();
    };
    let error = crate::workers::inspect::open(&path, &AtomicBool::new(false)).unwrap_err();
    let effects = update(
        &mut s,
        Msg::Opened {
            generation: *generation,
            path,
            outcome: Err(error),
        },
    );
    assert!(matches!(effects.as_slice(), [Effect::SaveRecent(_)]));
    assert!(s.recent.files.is_empty());
    let card = s.setup.open_error.as_ref().unwrap();
    assert!(card.error.to_string().contains("missing.fasta"), "G34");
}

#[test]
fn analyze_builds_the_request_from_the_form() {
    let mut s = state();
    assert!(update(&mut s, Msg::Analyze).is_empty(), "no file");
    opened_alignment(&mut s);
    s.setup.options.fill = " ".into();
    s.setup.header.enabled = true;
    assert!(
        update(&mut s, Msg::Analyze).is_empty(),
        "invalid fill (G52)"
    );
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text == "Not started: the Fill value under Advanced must not be empty"),
        "in the form's words, once"
    );
    // Every Stratify box unticked: the boxes are named, not the command line's option.
    for row in &mut s.setup.header.rows {
        row.stratify = false;
    }
    s.setup.options.fill = "Unknown".into();
    assert!(update(&mut s, Msg::Analyze).is_empty());
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text == "Not started: tick at least one Stratify box"),
        "{:?}",
        s.toasts.items()
    );
    for row in &mut s.setup.header.rows {
        row.stratify = true;
    }
    s.setup.options.fill = "Unknown".into();
    s.setup.header.rows[0].name = "Accession".into();
    s.setup.header.rows[0].stratify = false;
    s.settings.threads = Some(3);
    let effects = update(&mut s, Msg::Analyze);
    let [
        Effect::Analyze {
            request,
            generation,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(request.threads.get(), 3);
    assert_eq!(request.params.alphabet, None, "Auto stays detection (G1)");
    assert_eq!(request.params.kmer_length, None, "no clamp (G1)");
    assert_eq!(
        request.params.header_format.as_deref(),
        Some(
            &[
                "Accession".to_owned(),
                "field_2".into(),
                "field_3".into(),
                "field_4".into()
            ][..]
        )
    );
    assert_eq!(
        request.params.metadata_fields.as_deref(),
        Some(&["field_2".to_owned(), "field_3".into(), "field_4".into()][..])
    );
    assert!(request.expected.is_some(), "G65 stamp");
    assert_eq!(s.setup.run, RunState::Running(*generation));
    assert!(update(&mut s, Msg::Analyze).is_empty(), "one run at a time");
}

#[test]
fn opening_a_file_while_running_asks_first() {
    let mut s = state();
    opened_alignment(&mut s);
    update(&mut s, Msg::Analyze);
    assert!(update(&mut s, Msg::OpenPath(PathBuf::from("b.fa"))).is_empty());
    assert_eq!(
        s.confirm,
        Some(Confirm::ReplaceRun(PathBuf::from("b.fa"))),
        "G35"
    );
    assert!(update(&mut s, Msg::Confirmed(false)).is_empty());
    assert!(s.running());
    update(&mut s, Msg::OpenPath(PathBuf::from("b.fa")));
    let effects = update(&mut s, Msg::Confirmed(true));
    assert!(matches!(
        effects.as_slice(),
        [Effect::CancelAnalysis, Effect::Open { .. }]
    ));
    assert!(
        matches!(s.setup.run, RunState::Cancelling(_)),
        "G27: no new run until it stops"
    );
    assert!(update(&mut s, Msg::Analyze).is_empty());
}

#[test]
fn a_run_that_ends_after_cancellation_is_cancelled() {
    let mut s = state();
    let (generation, request) = run_outcome(&mut s);
    let outcome = analysis::run(
        &request,
        &Arc::new(Progress::new()),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(matches!(outcome, Outcome::Completed(..)));
    update(&mut s, Msg::CancelAnalysis);
    assert!(matches!(s.setup.run, RunState::Cancelling(_)));
    // It completed before it saw the request: nothing of it is kept.
    let effects = update(
        &mut s,
        Msg::AnalysisDone {
            generation,
            outcome,
        },
    );
    assert!(effects.is_empty(), "{effects:?}");
    assert!(s.session.is_none());
    assert_eq!(s.setup.run, RunState::Idle);
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.starts_with("Analysis cancelled"))
    );
}

fn run_outcome(s: &mut AppState) -> (u64, Request) {
    opened_alignment(s);
    let effects = update(s, Msg::Analyze);
    let [
        Effect::Analyze {
            request,
            generation,
        },
    ] = effects.as_slice()
    else {
        panic!();
    };
    (*generation, (**request).clone())
}

#[test]
fn a_completed_run_opens_the_results() {
    let mut s = state();
    let (generation, request) = run_outcome(&mut s);
    let outcome = analysis::run(
        &request,
        &Arc::new(Progress::new()),
        Arc::new(AtomicBool::new(false)),
    );
    assert!(
        update(
            &mut s,
            Msg::AnalysisDone {
                generation: generation + 100,
                outcome: Outcome::Changed
            }
        )
        .is_empty()
    );
    let effects = update(
        &mut s,
        Msg::AnalysisDone {
            generation,
            outcome,
        },
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::ComputeHcs { .. }))
    );
    assert_eq!(s.view, View::Workspace);
    assert_eq!(s.setup.run, RunState::Idle);
    let session = s.session.as_ref().unwrap();
    assert!(matches!(&session.origin, Origin::Analysed(r) if r.facts.positions == Some(1345)));
    assert_eq!(
        *session.results,
        *fixtures::mers(),
        "the same results as the library"
    );
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.starts_with("Analysed 191 sequences"))
    );
}

#[test]
fn a_run_with_fewer_threads_than_requested_warns() {
    let mut s = state();
    let (generation, request) = run_outcome(&mut s);
    let Outcome::Completed(analysis, mut facts) = analysis::run(
        &request,
        &Arc::new(Progress::new()),
        Arc::new(AtomicBool::new(false)),
    ) else {
        panic!("completed");
    };
    // The system refused all but one of the requested threads (D110).
    facts.threads = 4;
    facts.threads_started = Some(1);
    update(
        &mut s,
        Msg::AnalysisDone {
            generation,
            outcome: Outcome::Completed(analysis, facts),
        },
    );
    let warning = s
        .toasts
        .items()
        .iter()
        .find(|t| t.severity == Severity::Warning)
        .expect("a warning");
    assert!(
        warning
            .text
            .starts_with("Used 1 of 4 worker threads: the system refused to start more."),
        "{}",
        warning.text
    );
    assert!(
        warning
            .text
            .ends_with("lower Worker threads under Advanced (Setup screen)."),
        "names the control: {}",
        warning.text
    );
    assert_eq!(s.view, View::Workspace, "the results open all the same");
}

#[test]
fn a_failed_run_shows_a_card_and_reads_the_record_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.fa");
    std::fs::write(&path, ">a|x\nACDEFGHIK\n>b|y\nACDEF1HIK\n").unwrap();
    let mut s = state();
    s.setup.file = Some(path.clone());
    s.setup.options.alphabet = crate::state::options::AlphabetChoice::Protein;
    let effects = update(&mut s, Msg::Analyze);
    let [
        Effect::Analyze {
            request,
            generation,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    let outcome = analysis::run(
        request,
        &Arc::new(Progress::new()),
        Arc::new(AtomicBool::new(false)),
    );
    let effects = update(
        &mut s,
        Msg::AnalysisDone {
            generation: *generation,
            outcome,
        },
    );
    let [
        Effect::ReadErrorContext {
            generation: g,
            record,
            column,
            ..
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!((*record, *column), (2, Some(6)));
    let card = s.setup.failure.as_ref().unwrap();
    assert!(card.failed_run.is_some());
    assert!(card.details().contains("No results were written"));
    let excerpt =
        crate::workers::error_context::excerpt(&path, 2, Some(6), &AtomicBool::new(false));
    update(
        &mut s,
        Msg::ErrorContext {
            generation: *g,
            excerpt,
        },
    );
    let card = s.setup.failure.as_ref().unwrap();
    assert_eq!(card.excerpt.as_ref().unwrap().header, "b|y");
    assert_eq!(
        card.excerpt.as_ref().unwrap().around_column.as_deref(),
        Some("ACDEF[1]HIK")
    );
    // The failed run's report goes through a save dialog.
    let effects = update(&mut s, Msg::SaveFailedRunReport);
    assert!(matches!(
        effects.as_slice(),
        [Effect::Dialog {
            purpose: Purpose::FailedRunReport,
            ..
        }]
    ));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::FailedRunReport,
            path: Some(dir.path().join("report")),
            failed_fast: false,
        },
    );
    let [Effect::Export { session: None, job }] = effects.as_slice() else {
        panic!();
    };
    assert_eq!(
        job.path,
        dir.path().join("report.json"),
        "extension appended (D100)"
    );
    let written = crate::workers::export::run(job).unwrap();
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&written[0].path).unwrap()).unwrap();
    assert_eq!(v["status"], "failed");
    assert_eq!(v["exit_code"], 4);
}

#[test]
fn a_changed_file_is_checked_again_keeping_the_choices() {
    let mut s = state();
    let (generation, request) = run_outcome(&mut s);
    s.setup.header.enabled = true;
    s.setup.header.rows[1].name = "Host".into();
    s.setup.options.support_threshold = 30;
    s.setup.options.alphabet = crate::state::options::AlphabetChoice::Protein;
    let effects = update(
        &mut s,
        Msg::AnalysisDone {
            generation,
            outcome: Outcome::Changed,
        },
    );
    let [
        Effect::Open {
            generation: g,
            path,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.contains("changed after it was checked")),
        "G65"
    );
    let inspection = dima_core::validation::inspect(&Source::Path(request.path.clone())).unwrap();
    update(
        &mut s,
        Msg::Opened {
            generation: *g,
            path: path.clone(),
            outcome: Ok(Opened::Alignment(Box::new(inspection))),
        },
    );
    assert_eq!(s.setup.header.rows[1].name, "Host");
    assert!(s.setup.header.enabled);
    assert_eq!(
        s.setup.options.alphabet,
        crate::state::options::AlphabetChoice::Protein
    );
}

#[test]
fn hidden_positions_ask_to_clear_the_filters() {
    let mut s = state();
    with_session(&mut s);
    {
        let session = s.session.as_mut().unwrap();
        session.filter.position_range = (1, 10);
        session.apply_filters();
    }
    assert!(
        update(
            &mut s,
            Msg::Select(500, crate::state::session::SelectSource::Palette)
        )
        .is_empty()
    );
    assert_eq!(s.confirm, Some(Confirm::ClearFiltersFor(500)), "G20");
    update(&mut s, Msg::Confirmed(true));
    let session = s.session.as_ref().unwrap();
    assert_eq!(session.selection.position, Some(500));
    assert_eq!(session.shown.len(), 1345);
    update(
        &mut s,
        Msg::Select(99_999, crate::state::session::SelectSource::Palette),
    );
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.contains("no position 99999"))
    );
}

/// A figure whose positions were all filtered out while its save dialog was open is not saved,
/// and a warning says why.
#[test]
fn a_figure_with_no_positions_left_is_not_saved_silently() {
    let mut s = state();
    with_session(&mut s);
    let effects = update(&mut s, Msg::Export(ExportKind::Png));
    let [Effect::Dialog { purpose, .. }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    let purpose = *purpose;
    s.session.as_mut().unwrap().filter.value_range = (2.0, 3.0);
    update(&mut s, Msg::FiltersEdited);
    tick(&mut s, Duration::from_millis(16), false);
    assert!(s.session.as_ref().unwrap().shown.is_empty());
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose,
            path: Some(PathBuf::from("/x/figure.png")),
            failed_fast: false,
        },
    );
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(s.export_busy, None);
    assert!(s.toasts.items().iter().any(|t| t.severity == Severity::Warning
        && t.text
            == "Nothing was saved: no positions are shown, so there is no chart to export (check \
                the filters)"));
}

#[test]
fn filters_apply_when_the_pointer_is_released() {
    let mut s = state();
    with_session(&mut s);
    s.session.as_mut().unwrap().filter.position_range = (1, 100);
    update(&mut s, Msg::FiltersEdited);
    tick(&mut s, Duration::from_millis(16), true);
    assert_eq!(
        s.session.as_ref().unwrap().shown.len(),
        1345,
        "not while dragging"
    );
    tick(&mut s, Duration::from_millis(32), false);
    assert_eq!(s.session.as_ref().unwrap().shown.len(), 100);
    update(&mut s, Msg::ResetFilters);
    assert_eq!(s.session.as_ref().unwrap().shown.len(), 1345);
}

#[test]
fn hcs_edits_are_debounced_and_results_matched() {
    let mut s = state();
    with_session(&mut s);
    // The initial computation.
    let (g0, _, _) = s.hcs.running.unwrap();
    s.hcs.text = "40".into();
    let effects = update(&mut s, Msg::HcsEdited);
    assert!(matches!(effects.as_slice(), [Effect::SaveSettings(_)]));
    assert!(s.hcs.error.is_some(), "(50, 100] (G41)");
    s.hcs.text = "95".into();
    update(&mut s, Msg::HcsEdited);
    assert!(
        tick(&mut s, Duration::from_millis(10), false).is_empty(),
        "debounced"
    );
    let effects = tick(&mut s, Duration::from_millis(400), false);
    let [
        Effect::ComputeHcs {
            generation,
            params,
            results,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(params.threshold.to_string(), "95");
    // The first job's late result is ignored.
    let stale =
        Arc::new(dima_core::hcs::compute(results, &dima_core::hcs::HcsParams::default()).unwrap());
    update(
        &mut s,
        Msg::HcsDone {
            generation: g0,
            outcome: Ok(stale),
        },
    );
    assert!(s.hcs.result.is_none());
    let fresh = Arc::new(dima_core::hcs::compute(results, params).unwrap());
    update(
        &mut s,
        Msg::HcsDone {
            generation: *generation,
            outcome: Ok(fresh),
        },
    );
    let id = s.session.as_ref().unwrap().id;
    assert!(s.hcs.current(id).is_some());
    assert_eq!(s.settings.hcs_threshold, "95", "remembered (D73)");
}

#[test]
fn an_hcs_edit_without_results_leaves_nothing_pending() {
    // Edited on the setup screen before any results exist: nothing waits, so the app stops
    // repainting once the debounce time has passed.
    let mut s = state();
    s.hcs.text = "95".into();
    update(&mut s, Msg::HcsEdited);
    assert!(s.busy());
    assert!(tick(&mut s, Duration::from_millis(400), false).is_empty());
    assert_eq!(s.hcs.pending_since, None);
    assert!(!s.busy());
    // Loading results computes the regions at the edited threshold.
    with_session(&mut s);
    let (_, _, params) = s.hcs.running.unwrap();
    assert_eq!(params.threshold.to_string(), "95");
}

#[test]
fn exports_are_guarded_and_go_through_a_dialog() {
    let mut s = state();
    with_session(&mut s);
    let id = s.session.as_ref().unwrap().id;
    // A run report needs a run made in the app.
    assert!(update(&mut s, Msg::Export(ExportKind::RunReport)).is_empty());
    // One export at a time, whoever starts it (G6).
    s.export_busy = Some(ExportKind::Png);
    assert!(update(&mut s, Msg::Export(ExportKind::Svg)).is_empty());
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.contains("already running"))
    );
    s.export_busy = None;
    let effects = update(&mut s, Msg::Export(ExportKind::Svg));
    assert!(matches!(
        effects.as_slice(),
        [Effect::Dialog {
            purpose: Purpose::Export {
                kind: ExportKind::Svg,
                session,
                ..
            },
            kind: DialogKind::Save { .. },
            built_in: false,
            ..
        }] if *session == id
    ));
    assert!(
        update(&mut s, Msg::Export(ExportKind::Png)).is_empty(),
        "one dialog at a time"
    );
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.starts_with("A file dialog is already open")),
        "and the user is told why"
    );
    // The native dialog failed at once: the built-in browser takes over.
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind: ExportKind::Svg,
                session: id,
                metric: ChartMetric::Entropy,
            },
            path: None,
            failed_fast: true,
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::Dialog { built_in: true, .. }]
    ));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind: ExportKind::Svg,
                session: id,
                metric: ChartMetric::Entropy,
            },
            path: Some(PathBuf::from("/x/figure")),
            failed_fast: false,
        },
    );
    let [
        Effect::Export {
            session: Some(exported_from),
            job,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(*exported_from, id);
    assert_eq!(job.path, PathBuf::from("/x/figure.svg"));
    assert_eq!(s.export_busy, Some(ExportKind::Svg));
    let files = vec![crate::workers::export::WrittenFile {
        path: PathBuf::from("/x/r.json"),
        role: Some("results"),
        bytes: 5,
        sha256: "0".repeat(64),
    }];
    update(
        &mut s,
        Msg::ExportDone {
            kind: ExportKind::ResultsJson,
            metric: ChartMetric::Entropy,
            session: Some(id),
            outcome: Ok(files.clone()),
        },
    );
    assert_eq!(s.export_busy, None);
    assert_eq!(s.session.as_ref().unwrap().exports.len(), 1);
    // An export of results replaced meanwhile is not recorded for the new ones.
    update(
        &mut s,
        Msg::ExportDone {
            kind: ExportKind::ResultsJson,
            metric: ChartMetric::Entropy,
            session: Some(id + 1),
            outcome: Ok(files),
        },
    );
    assert_eq!(s.session.as_ref().unwrap().exports.len(), 1);
    // Nothing to draw: no figure export.
    s.session.as_mut().unwrap().filter.position_range = (5, 1);
    s.session.as_mut().unwrap().apply_filters();
    assert!(update(&mut s, Msg::Export(ExportKind::Png)).is_empty());
}

#[test]
fn built_in_dialogs_follow_the_setting_and_the_environment() {
    let mut s = state();
    s.native_dialogs = false;
    let effects = update(&mut s, Msg::OpenDialog);
    assert!(matches!(
        effects.as_slice(),
        [Effect::Dialog {
            built_in: true,
            kind: DialogKind::Open,
            ..
        }]
    ));
    assert!(!s.dialog.native);
    let mut s = state();
    update(&mut s, Msg::OpenDialog);
    assert!(s.dialog.native);
    let effects = update(&mut s, Msg::UseBuiltInBrowser);
    assert!(matches!(
        effects.as_slice(),
        [Effect::Dialog { built_in: true, .. }]
    ));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::OpenInput,
            path: Some(PathBuf::from("a.fa")),
            failed_fast: false,
        },
    );
    assert!(matches!(effects.as_slice(), [Effect::Open { .. }]));
}

#[test]
fn panics_become_errors_with_details() {
    let mut s = state();
    s.export_busy = Some(ExportKind::Png);
    update(
        &mut s,
        Msg::Panicked {
            what: "exporting",
            message: "boom".into(),
        },
    );
    assert_eq!(s.export_busy, None);
    let t = &s.toasts.items()[0];
    assert_eq!(t.severity, Severity::Error);
    assert!(t.details.as_deref().unwrap().contains("boom"));
}

#[test]
fn refused_threads_are_not_reported_as_bugs() {
    let mut s = state();
    s.export_busy = Some(ExportKind::Png);
    update(
        &mut s,
        Msg::Refused {
            what: "exporting",
            reason: "Resource temporarily unavailable (os error 11)".into(),
        },
    );
    assert_eq!(s.export_busy, None);
    let t = &s.toasts.items()[0];
    assert_eq!(t.severity, Severity::Error);
    assert!(t.text.contains("refused a new thread"), "{}", t.text);
    assert!(!t.text.contains("report"), "{}", t.text);
    assert!(
        t.text
            .contains("lower Worker threads under Advanced (Setup screen)"),
        "names the control: {}",
        t.text
    );
}

#[test]
fn drops_options_views_and_threads() {
    let mut s = state();
    let effects = update(
        &mut s,
        Msg::Dropped(vec![PathBuf::from("a.fa"), PathBuf::from("b.fa")]),
    );
    assert!(matches!(effects.as_slice(), [Effect::Open { .. }]));
    assert!(
        s.toasts.items()[0].text.contains("One file at a time"),
        "G32"
    );
    update(&mut s, Msg::ShowView(View::Workspace));
    assert_eq!(s.view, View::Setup, "no results to show");
    s.setup.options.support_threshold = 7;
    s.setup.options.similarity_weighted_entropy = true;
    update(&mut s, Msg::OptionsChanged);
    assert!(s.settings.similarity_weighted_entropy, "remembered (D73)");
    let effects = update(&mut s, Msg::ResetOptions);
    assert!(matches!(effects.as_slice(), [Effect::SaveSettings(_)]));
    assert_eq!(s.setup.options.support_threshold, 100);
    assert!(!s.setup.options.similarity_weighted_entropy);
    assert!(!s.settings.similarity_weighted_entropy);
    update(&mut s, Msg::ThreadsChanged(Some(2)));
    assert_eq!(s.settings.threads, Some(2));
}

#[test]
fn an_export_never_writes_results_loaded_while_its_dialog_was_open() {
    let mut s = state();
    with_session(&mut s);
    let first = s.session.as_ref().unwrap().id;
    update(&mut s, Msg::Export(ExportKind::ResultsJson));
    // Other results are opened (a drop or a recent file) before the dialog closes.
    with_session(&mut s);
    assert_ne!(s.session.as_ref().unwrap().id, first);
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind: ExportKind::ResultsJson,
                session: first,
                metric: ChartMetric::Entropy,
            },
            path: Some(PathBuf::from("/x/first.json")),
            failed_fast: false,
        },
    );
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(s.export_busy, None);
    assert_eq!(s.dialog.pending, None);
    assert!(
        s.toasts
            .items()
            .iter()
            .any(|t| t.text.starts_with("Nothing was saved"))
    );
}

#[test]
fn closing_during_an_export_asks_first() {
    let mut s = state();
    assert!(
        update(&mut s, Msg::CloseRequested).is_empty(),
        "nothing to lose"
    );
    assert_eq!(s.confirm, None);
    s.export_busy = Some(ExportKind::ResultsJson);
    let effects = update(&mut s, Msg::CloseRequested);
    assert!(matches!(effects.as_slice(), [Effect::CancelClose]));
    assert_eq!(s.confirm, Some(Confirm::QuitDuringExport));
    // "Keep DiMA open".
    assert!(update(&mut s, Msg::Confirmed(false)).is_empty());
    assert_eq!(s.confirm, None);
    // Asked again, then confirmed.
    update(&mut s, Msg::CloseRequested);
    let effects = update(&mut s, Msg::Confirmed(true));
    assert!(matches!(effects.as_slice(), [Effect::Quit]));
    // The export finishing while the question is open makes it moot.
    update(&mut s, Msg::CloseRequested);
    update(
        &mut s,
        Msg::ExportDone {
            kind: ExportKind::ResultsJson,
            metric: ChartMetric::Entropy,
            session: None,
            outcome: Ok(Vec::new()),
        },
    );
    assert_eq!(s.confirm, None);
}

#[test]
fn an_appended_extension_never_replaces_a_file_unasked() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("results.json"), "old").unwrap();
    let mut s = state();
    with_session(&mut s);
    let session = s.session.as_ref().unwrap().id;
    let purpose = Purpose::Export {
        kind: ExportKind::ResultsJson,
        session,
        metric: ChartMetric::Entropy,
    };
    update(&mut s, Msg::Export(ExportKind::ResultsJson));
    // "results" was typed: the dialog checked that name, the app adds ".json", which exists.
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose,
            path: Some(dir.path().join("results")),
            failed_fast: false,
        },
    );
    assert!(effects.is_empty(), "{effects:?}");
    let asked = Confirm::Overwrite {
        purpose,
        path: dir.path().join("results.json"),
    };
    assert_eq!(s.confirm, Some(asked.clone()));
    assert!(update(&mut s, Msg::Confirmed(false)).is_empty());
    assert_eq!(s.export_busy, None);
    // Asked again and confirmed: the export is written there.
    s.confirm = Some(asked);
    let effects = update(&mut s, Msg::Confirmed(true));
    let [Effect::Export { job, .. }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(job.path, dir.path().join("results.json"));
    // A name the dialog itself checked (with the extension) is not asked about again.
    s.export_busy = None;
    update(&mut s, Msg::Export(ExportKind::ResultsJson));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose,
            path: Some(dir.path().join("results.json")),
            failed_fast: false,
        },
    );
    assert!(matches!(effects.as_slice(), [Effect::Export { .. }]));
}

/// The text of the newest notification.
fn last_toast(s: &AppState) -> String {
    s.toasts
        .items()
        .last()
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// Chooses `path` in the save dialog of an export of `kind`; returns the effects.
fn export_to(s: &mut AppState, kind: ExportKind, path: PathBuf) -> Vec<Effect> {
    s.export_busy = None;
    update(s, Msg::Export(kind));
    let session = s.session.as_ref().unwrap().id;
    update(
        s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind,
                session,
                metric: ChartMetric::Entropy,
            },
            path: Some(path),
            failed_fast: false,
        },
    )
}

/// The names the command line accepts are accepted, and written as they say; the ones it
/// refuses are refused with its reason, and nothing is written.
#[test]
fn export_names_are_those_of_the_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = state();
    with_session(&mut s);
    for (kind, name) in [
        (ExportKind::Jsonl, "p.ndjson.gz"),
        (ExportKind::HcsTsv, "r.tab"),
        (ExportKind::ResultsJson, "x.JSON.zst"),
    ] {
        let effects = export_to(&mut s, kind, dir.path().join(name));
        let [Effect::Export { job, .. }] = effects.as_slice() else {
            panic!("{name}: {effects:?}");
        };
        assert_eq!(job.path, dir.path().join(name), "kept as typed");
    }
    let bz2 = dir.path().join("x.json.bz2");
    assert!(export_to(&mut s, ExportKind::ResultsJson, bz2.clone()).is_empty());
    assert_eq!(s.export_busy, None);
    assert_eq!(
        last_toast(&s),
        format!(
            "Nothing was saved: '{}' asks for bzip2 compression, which DiMA does not write \
             (DiMA compresses with gzip or Zstandard: name it {} or {})",
            bz2.display(),
            dir.path().join("x.json.gz").display(),
            dir.path().join("x.json.zst").display()
        )
    );
    assert!(
        export_to(
            &mut s,
            ExportKind::HcsJson,
            dir.path().join("h.json.gz.zst")
        )
        .is_empty()
    );
    assert!(
        last_toast(&s).contains("asks for two compressions"),
        "{}",
        last_toast(&s)
    );
    assert!(!dir.path().join("x.json.bz2").exists());
    // The save dialogs' filters list the last extension of every accepted ending, the usual
    // one first: Windows and macOS add the first to a name typed without an extension.
    assert_eq!(
        filter_extensions(ExportKind::Jsonl),
        ["jsonl", "ndjson", "gz", "zst"]
    );
    assert_eq!(
        filter_extensions(ExportKind::HcsTsv),
        ["tsv", "tab", "gz", "zst"]
    );
    assert_eq!(
        filter_extensions(ExportKind::RunReport),
        ["json", "gz", "zst"]
    );
    assert_eq!(
        filter_extensions(ExportKind::ResultsJson),
        ["json", "gz", "zst"]
    );
    assert_eq!(filter_extensions(ExportKind::Png), ["png"]);
}

/// An export never replaces a file the app has loaded: the results file opened, the alignment
/// analysed, or the alignment on the Setup screen (a table of a folder export included).
#[test]
fn an_export_never_replaces_a_loaded_input() {
    let dir = tempfile::tempdir().unwrap();
    let opened = dir.path().join("results.json");
    std::fs::write(&opened, "{}").unwrap();
    let mut s = state();
    let effects = update(&mut s, Msg::OpenPath(opened.clone()));
    let Some(Effect::Open { generation, .. }) = effects.first() else {
        panic!();
    };
    update(
        &mut s,
        Msg::Opened {
            generation: *generation,
            path: opened.clone(),
            outcome: Ok(Opened::Results(Box::new((*fixtures::mers()).clone()))),
        },
    );
    // The same file, also as typed without its extension (which the app adds).
    for chosen in [opened.clone(), dir.path().join("results")] {
        assert!(export_to(&mut s, ExportKind::ResultsJson, chosen).is_empty());
        assert_eq!(
            last_toast(&s),
            "Nothing was saved: 'results.json' is the results file these results were opened \
             from; choose another name"
        );
        assert_eq!(s.confirm, None, "refused before asking to replace it");
    }
    assert_eq!(std::fs::read_to_string(&opened).unwrap(), "{}");
    // Another name is written.
    assert!(matches!(
        export_to(
            &mut s,
            ExportKind::ResultsJson,
            dir.path().join("copy.json")
        )
        .as_slice(),
        [Effect::Export { .. }]
    ));
    // The alignment on the Setup screen, as one of the four tables of a folder export.
    let alignment = dir.path().join("mers_spike_aa.positions.tsv");
    std::fs::write(&alignment, ">a\nACDE\n").unwrap();
    s.setup.file = Some(alignment.clone());
    assert!(export_to(&mut s, ExportKind::TsvFolder, dir.path().to_path_buf()).is_empty());
    assert_eq!(
        last_toast(&s),
        "Nothing was saved: 'mers_spike_aa.positions.tsv' is the alignment on the Setup screen; \
         choose another name"
    );
    assert_eq!(std::fs::read_to_string(&alignment).unwrap(), ">a\nACDE\n");
}

/// The alignment a run was computed from is protected too.
#[test]
fn an_export_never_replaces_the_analysed_alignment() {
    let dir = tempfile::tempdir().unwrap();
    let alignment = dir.path().join("spike.json");
    std::fs::copy(fixtures::mers_path(), &alignment).unwrap();
    let mut s = state();
    s.setup.file = Some(alignment.clone());
    let effects = update(&mut s, Msg::Analyze);
    let [
        Effect::Analyze {
            request,
            generation,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    let outcome = analysis::run(
        request,
        &Arc::new(Progress::new()),
        Arc::new(AtomicBool::new(false)),
    );
    let generation = *generation;
    update(
        &mut s,
        Msg::AnalysisDone {
            generation,
            outcome,
        },
    );
    assert!(s.session.is_some());
    // Only the analysed run protects it now.
    s.setup.file = None;
    assert!(export_to(&mut s, ExportKind::HcsJson, alignment.clone()).is_empty());
    assert_eq!(
        last_toast(&s),
        "Nothing was saved: 'spike.json' is the alignment these results were computed from; \
         choose another name"
    );
}

/// The chart's metric: entropy on every load; a metric the results lack is refused, the metric
/// shown is a no-op; a switch keeps the zoom and the selection, resets the value range to the
/// new metric's full range, and says so only when the user had narrowed it, in which case the
/// table also brings the selected row back into view.
#[test]
fn the_chart_metric_switch() {
    use crate::charts::metric::ChartMetric::{Entropy, SimilarityWeighted};
    let mut s = state();
    with_session(&mut s);
    assert!(update(&mut s, Msg::ChartMetric(SimilarityWeighted)).is_empty());
    assert_eq!(
        s.session.as_ref().unwrap().metric,
        Entropy,
        "not computed: refused"
    );
    with_results(&mut s, &fixtures::mers_swe());
    s.toasts = crate::state::toasts::Toasts::default();
    let session = s.session.as_mut().unwrap();
    assert_eq!(
        session.metric, Entropy,
        "every load starts with entropy (G26)"
    );
    session.filter.value_range = (0.0, 1.0);
    session.apply_filters();
    session.viewport = Some((100.5, 200.5));
    session.select(150, SelectSource::Chart);
    // The table has drawn since, and scrolled to the row.
    session.selection.scroll_to_row = false;
    let (shown, version) = (session.shown.len(), session.version);
    assert!(shown < 1345, "a narrowed range");
    // The metric shown: nothing changes.
    assert!(update(&mut s, Msg::ChartMetric(Entropy)).is_empty());
    let session = s.session.as_ref().unwrap();
    assert_eq!(
        (session.version, session.filter.value_range),
        (version, (0.0, 1.0))
    );
    assert!(s.toasts.is_empty());
    // A switch after narrowing: the new metric's full range, with a note.
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    let session = s.session.as_ref().unwrap();
    assert_eq!(session.metric, SimilarityWeighted);
    let max = session.bounds.max_similarity_weighted;
    assert!((max - 0.4548).abs() < 5e-5, "{max}");
    assert_eq!(session.filter.value_range, (0.0, max));
    assert_eq!(session.shown.len(), 1345);
    assert_eq!(session.viewport, Some((100.5, 200.5)), "the zoom is kept");
    assert_eq!(
        session.selection.position,
        Some(150),
        "the selection is kept"
    );
    assert!(
        session.selection.scroll_to_row,
        "the positions that came back can push its row out of view"
    );
    assert_ne!(session.version, version, "the chart's lines are recomputed");
    assert_eq!(session.series.y_max, Some(max), "its own axis");
    let notes: Vec<_> = s.toasts.items().iter().collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].severity, Severity::Info);
    assert_eq!(
        notes[0].text,
        "Value range reset to the full range of similarity-weighted entropy"
    );
    // Back without narrowing: no note, and the same rows, so the table does not move.
    s.toasts = crate::state::toasts::Toasts::default();
    s.session.as_mut().unwrap().selection.scroll_to_row = false;
    update(&mut s, Msg::ChartMetric(Entropy));
    let session = s.session.as_ref().unwrap();
    assert_eq!(session.metric, Entropy);
    assert_eq!(
        session.filter.value_range,
        (0.0, session.bounds.max_entropy)
    );
    assert_eq!(session.viewport, Some((100.5, 200.5)));
    assert!(!session.selection.scroll_to_row);
    assert!(s.toasts.is_empty());
    // Entropy narrowed, then switched to similarity-weighted entropy and back: only the
    // narrowed range is told.
    let session = s.session.as_mut().unwrap();
    session.filter.value_range = (0.5, session.bounds.max_entropy);
    session.apply_filters();
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    update(&mut s, Msg::ChartMetric(Entropy));
    assert_eq!(s.toasts.items().len(), 1, "only the narrowed range is told");
    // A new load starts with entropy again.
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    with_results(&mut s, &fixtures::mers_swe());
    assert_eq!(s.session.as_ref().unwrap().metric, Entropy);
}

/// The figure of similarity-weighted entropy: its own title, value axis, legend (never
/// "corrected") and mean, and no estimator in the subtitle; entropy's is unchanged.
#[test]
fn the_figure_follows_the_metric() {
    use crate::charts::metric::ChartMetric::SimilarityWeighted;
    let mut s = state();
    with_results(&mut s, &fixtures::mers_swe());
    let entropy = chart_scene(&s, ChartMetric::Entropy).unwrap();
    assert_eq!(
        entropy.title,
        "mers_spike_aa \u{2014} Shannon entropy per k-mer position"
    );
    assert_eq!(entropy.y_title, "Shannon entropy (bits)");
    assert_eq!(
        entropy.mean,
        s.session.as_ref().unwrap().results.average_entropy
    );
    assert!(entropy.subtitle.contains("estimator dima"));
    assert_eq!(entropy.legend[0], "support > 100: corrected where possible");
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    let scene = chart_scene(&s, SimilarityWeighted).unwrap();
    assert_eq!(
        scene.title,
        "mers_spike_aa \u{2014} Similarity-weighted entropy per k-mer position"
    );
    assert_eq!(scene.y_title, "Similarity-weighted entropy (bits)");
    assert!(
        scene.legend.iter().all(|l| !l.contains("corrected")),
        "{:?}",
        scene.legend
    );
    assert_eq!(scene.legend[0], "support > 100");
    let mean = scene.mean.unwrap();
    assert!((mean - 0.0363).abs() < 5e-5, "{mean}");
    assert!(!scene.subtitle.contains("estimator"), "{}", scene.subtitle);
    assert!(
        scene
            .subtitle
            .starts_with("191 sequences, k = 9, T = 100; 1345 of 1345")
    );
    let max = scene.series.y_max.unwrap();
    assert!(
        (max - 0.4548).abs() < 5e-5,
        "the similarity-weighted values: {max}"
    );
    let svg = scene.to_svg().unwrap();
    assert!(svg.contains(">Similarity-weighted entropy (bits)</text>"));
    assert!(svg.contains(&format!(
        "eligible mean {}",
        dima_core::human::fixed(mean, 4)
    )));
}

/// A figure is named and saved for the metric shown when it was chosen: the dialog proposes
/// `<stem>.similarity-weighted-entropy.png`, and if the chart was switched while the dialog was
/// open nothing is saved (a data export is unaffected). Otherwise the export job carries that
/// metric and its figure, and a failed export's message names the figure by it.
#[test]
fn figures_are_saved_only_for_the_metric_they_were_chosen_for() {
    use crate::charts::metric::ChartMetric::{Entropy, SimilarityWeighted};
    let mut s = state();
    with_results(&mut s, &fixtures::mers_swe());
    let session = s.session.as_ref().unwrap().id;
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    let effects = update(&mut s, Msg::Export(ExportKind::Png));
    let [
        Effect::Dialog {
            purpose,
            kind: DialogKind::Save { default_name, .. },
            title,
            ..
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(
        *purpose,
        Purpose::Export {
            kind: ExportKind::Png,
            session,
            metric: SimilarityWeighted
        }
    );
    assert_eq!(
        default_name,
        "mers_spike_aa.similarity-weighted-entropy.png"
    );
    assert_eq!(title, "Save Similarity-weighted entropy chart (PNG)");
    let purpose = *purpose;
    // Switched back (the palette, while the dialog is open): nothing is saved.
    update(&mut s, Msg::ChartMetric(Entropy));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose,
            path: Some(PathBuf::from("/x/figure.png")),
            failed_fast: false,
        },
    );
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(s.export_busy, None);
    assert_eq!(s.dialog.pending, None);
    let warning = s
        .toasts
        .items()
        .iter()
        .find(|t| t.severity == Severity::Warning)
        .expect("a warning");
    assert_eq!(
        warning.text,
        "Nothing was saved: the chart was switched to entropy while the dialog was open. \
         Export again to save the chart now shown."
    );
    // The other way round, with the SVG.
    s.toasts = crate::state::toasts::Toasts::default();
    update(&mut s, Msg::Export(ExportKind::Svg));
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind: ExportKind::Svg,
                session,
                metric: Entropy,
            },
            path: Some(PathBuf::from("/x/figure.svg")),
            failed_fast: false,
        },
    );
    assert!(s.toasts.items().iter().any(|t| t.text
        == "Nothing was saved: the chart was switched to similarity-weighted entropy while the \
            dialog was open. Export again to save the chart now shown."));
    // A data export does not depend on the chart.
    update(&mut s, Msg::Export(ExportKind::ResultsJson));
    update(&mut s, Msg::ChartMetric(Entropy));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind: ExportKind::ResultsJson,
                session,
                metric: SimilarityWeighted,
            },
            path: Some(PathBuf::from("/x/r.json")),
            failed_fast: false,
        },
    );
    assert!(
        matches!(effects.as_slice(), [Effect::Export { .. }]),
        "{effects:?}"
    );
    s.export_busy = None;
    // The metric unchanged: the figure of that metric is written.
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    update(&mut s, Msg::Export(ExportKind::Svg));
    let effects = update(
        &mut s,
        Msg::DialogClosed {
            purpose: Purpose::Export {
                kind: ExportKind::Svg,
                session,
                metric: SimilarityWeighted,
            },
            path: Some(PathBuf::from("/x/figure")),
            failed_fast: false,
        },
    );
    let [Effect::Export { job, .. }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(job.metric, SimilarityWeighted);
    assert_eq!(job.path, PathBuf::from("/x/figure.svg"));
    let crate::workers::export::Payload::Figure(scene) = &job.payload else {
        panic!("a figure");
    };
    assert_eq!(scene.y_title, "Similarity-weighted entropy (bits)");
    // A failed export names the figure by its metric.
    update(
        &mut s,
        Msg::ExportDone {
            kind: ExportKind::Svg,
            metric: SimilarityWeighted,
            session: Some(session),
            outcome: Err(dima_core::DimaError::usage("disk full")),
        },
    );
    let error = s
        .toasts
        .items()
        .iter()
        .find(|t| t.severity == Severity::Error)
        .expect("an error");
    assert!(
        error
            .text
            .starts_with("Similarity-weighted entropy chart (SVG) failed: "),
        "{}",
        error.text
    );
}

/// A metric switch in the frame of a filter edit not yet applied applies that edit with the
/// switch and keeps the zoom, as the switch promises; the edit was applied again on the next
/// tick, which resets the zoom. The rows the edit changes can push the selected one out of
/// view, so the table scrolls back to it, as after any filter edit.
#[test]
fn a_metric_switch_applies_a_pending_filter_edit_and_keeps_the_zoom() {
    use crate::charts::metric::ChartMetric::SimilarityWeighted;
    let mut s = state();
    with_results(&mut s, &fixtures::mers_swe());
    let session = s.session.as_mut().unwrap();
    session.viewport = Some((100.5, 600.5));
    session.select(150, SelectSource::Chart);
    session.selection.scroll_to_row = false;
    session.filter.position_range = (50, 900);
    update(&mut s, Msg::FiltersEdited);
    update(&mut s, Msg::ChartMetric(SimilarityWeighted));
    tick(&mut s, Duration::from_millis(16), false);
    let session = s.session.as_ref().unwrap();
    assert_eq!(session.metric, SimilarityWeighted);
    assert!(!session.filter_dirty, "applied with the switch");
    assert!(
        session
            .shown
            .iter()
            .all(|&i| (50..=900).contains(&session.results.results[i].position)),
        "the edit is applied"
    );
    assert_eq!(session.viewport, Some((100.5, 600.5)), "the zoom is kept");
    assert_eq!(session.selection.position, Some(150));
    assert!(session.selection.scroll_to_row);
}

/// Results written with a newer schema, or one this version does not recognise, open with the
/// warning `dima view` gives, as an export keeps only the fields this version knows; this
/// version's schema and older ones open without it.
#[test]
fn results_of_a_newer_schema_open_with_a_warning() {
    let current = dima_core::model::SCHEMA_VERSION;
    for (version, shown) in [
        ("1.2.0", Some("1.2.0")),
        ("1.10.0", Some("1.10.0")),
        ("1.1.0-dev", Some("\"1.1.0-dev\"")),
        (current, None),
        ("1.0.0", None),
    ] {
        let mut s = state();
        let mut r = (*fixtures::mers()).clone();
        r.schema_version = version.to_owned();
        with_results(&mut s, &r);
        let warnings: Vec<&str> = s
            .toasts
            .items()
            .iter()
            .filter(|t| t.severity == Severity::Warning)
            .map(|t| t.text.as_str())
            .collect();
        let expected = shown.map(|v| {
            format!(
                "The results were written with schema {v}; this version writes {current} and \
                 keeps only the fields it knows. Use the DiMA version that wrote the file to \
                 keep every field."
            )
        });
        assert_eq!(
            warnings,
            expected.as_deref().into_iter().collect::<Vec<_>>(),
            "{version}"
        );
        assert!(s.session.is_some(), "{version}: opened all the same");
    }
}
