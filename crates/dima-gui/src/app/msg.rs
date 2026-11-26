//! Events (`Msg`) and the side effects (`Effect`) the pure `update` asks the app to perform.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use dima_core::DimaError;
use dima_core::hcs::HcsParams;
use dima_core::model::{HcsResults, Results};

use crate::charts::metric::ChartMetric;
use crate::state::session::{Nav, SelectSource};
use crate::state::toasts::Severity;
use crate::state::{Purpose, View};
use crate::workers::analysis::{Outcome as AnalysisOutcome, Request};
use crate::workers::dialogs::DialogKind;
use crate::workers::error_context::RecordExcerpt;
use crate::workers::export::{ExportKind, Job as ExportJob, Outcome as ExportOutcome};
use crate::workers::inspect::Opened;

/// Something that happened: a user action, a worker result or the passage of time.
#[derive(Debug)]
pub enum Msg {
    // Files.
    /// "Open…": show a file dialog.
    OpenDialog,
    /// A file was chosen (dialog, path field, recent list, drop, palette).
    OpenPath(PathBuf),
    /// Files were dropped on the window.
    Dropped(Vec<PathBuf>),
    /// The pending question was answered.
    Confirmed(bool),
    /// A file was opened (pre-checked or loaded).
    Opened {
        /// Job generation.
        generation: u64,
        /// The file.
        path: PathBuf,
        /// What it turned out to be, or why it could not be opened.
        outcome: Result<Opened, DimaError>,
    },
    /// Stop opening the file being opened.
    CancelOpen,
    /// Remove a recent file.
    RecentRemoved(PathBuf),
    /// Clear the recent files.
    RecentCleared,

    // Setup.
    /// A remembered option changed (saved).
    OptionsChanged,
    /// "Reset to defaults".
    ResetOptions,
    /// A format was pasted into the header editor.
    HeaderPasted(String),
    /// "Analyze".
    Analyze,
    /// "Cancel" while analysing.
    CancelAnalysis,
    /// The analysis ended.
    AnalysisDone {
        /// Job generation.
        generation: u64,
        /// How it ended.
        outcome: AnalysisOutcome,
    },
    /// The offending record of a failed run was read back.
    ErrorContext {
        /// Job generation.
        generation: u64,
        /// The excerpt, if the record could be read.
        excerpt: Option<RecordExcerpt>,
    },
    /// Close the failure card.
    DismissFailure,
    /// "Save run report…" on the failure card.
    SaveFailedRunReport,
    /// Switch screens.
    ShowView(View),

    // Workspace.
    /// A filter control changed (applied when the pointer is released).
    FiltersEdited,
    /// Plot another metric (the switch above the chart).
    ChartMetric(ChartMetric),
    /// "Reset" filters.
    ResetFilters,
    /// Select a position.
    Select(u64, SelectSource),
    /// Select a variant of the selected position.
    SelectVariant(usize),
    /// Arrow keys, Home, End.
    Navigate(Nav),
    /// A position-table header was clicked.
    SortPositions(usize),
    /// A variant-table header was clicked.
    SortVariants(usize),
    /// An HCS parameter was edited.
    HcsEdited,
    /// Conserved regions were computed.
    HcsDone {
        /// Job generation.
        generation: u64,
        /// The regions, or why not.
        outcome: Result<Arc<HcsResults>, DimaError>,
    },
    /// The worker thread count changed.
    ThreadsChanged(Option<usize>),

    // Exports and dialogs.
    /// An export was chosen (asks for the destination).
    Export(ExportKind),
    /// An export finished.
    ExportDone {
        /// What.
        kind: ExportKind,
        /// The metric the chart showed when it was chosen (names a figure).
        metric: ChartMetric,
        /// The session exported from (`None`: the failed run's report).
        session: Option<u64>,
        /// The files written, or why not.
        outcome: ExportOutcome,
    },
    /// A file dialog closed.
    DialogClosed {
        /// What it was for.
        purpose: Purpose,
        /// The chosen path.
        path: Option<PathBuf>,
        /// The native dialog returned at once without a choice (it is not working here).
        failed_fast: bool,
    },
    /// "Use built-in browser" while the native dialog is open.
    UseBuiltInBrowser,

    // General.
    /// The user asked to close the window.
    CloseRequested,
    /// Once per frame: the time and whether a pointer button is held (debounces).
    Tick {
        /// Now.
        now: Instant,
        /// A pointer button is down.
        pointer_down: bool,
    },
    /// Show a notification.
    Notify(Severity, String),
    /// Dismiss a notification.
    DismissToast(u64),
    /// A worker panicked (D62).
    Panicked {
        /// What the worker was doing.
        what: &'static str,
        /// The panic message.
        message: String,
    },
    /// A worker could not start: the system refused a new thread.
    Refused {
        /// What the worker was to do.
        what: &'static str,
        /// The operating system's reason.
        reason: String,
    },
}

/// Something `update` asks the app to do.
#[derive(Debug)]
pub enum Effect {
    /// Open (sniff, pre-check or load) a file.
    Open {
        /// Job generation.
        generation: u64,
        /// The file.
        path: PathBuf,
    },
    /// Stop the open job.
    CancelOpen,
    /// Run an analysis.
    Analyze {
        /// Job generation.
        generation: u64,
        /// What to run (boxed: it carries the pre-check's probe facts).
        request: Box<Request>,
    },
    /// Ask the analysis to stop.
    CancelAnalysis,
    /// Read back the offending record of a failed run.
    ReadErrorContext {
        /// Job generation.
        generation: u64,
        /// The input.
        path: PathBuf,
        /// Record number.
        record: u64,
        /// Column, if known.
        column: Option<u64>,
    },
    /// Compute conserved regions.
    ComputeHcs {
        /// Job generation.
        generation: u64,
        /// The results.
        results: Arc<Results>,
        /// The parameters.
        params: HcsParams,
    },
    /// Show a file dialog.
    Dialog {
        /// What for.
        purpose: Purpose,
        /// What kind.
        kind: DialogKind,
        /// Title.
        title: String,
        /// Use the built-in browser (else the native dialog, which falls back).
        built_in: bool,
    },
    /// Write an export.
    Export {
        /// The session exported from (`None`: the failed run's report).
        session: Option<u64>,
        /// What to write.
        job: Box<ExportJob>,
    },
    /// Keep the window open (a question about closing is shown).
    CancelClose,
    /// Close the window.
    Quit,
    /// Save the settings.
    SaveSettings(String),
    /// Save the recent files.
    SaveRecent(String),
}
