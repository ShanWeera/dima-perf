//! Application state: plain data, no egui. The pure `update` function (`app::update`) is the
//! only place that changes it in response to events; panels read it and edit form values.

pub mod filter;
pub mod hcs;
pub mod header_editor;
pub mod metadata;
pub mod options;
pub mod recent;
pub mod session;
pub mod settings;
pub mod sort;
pub mod toasts;

use std::path::PathBuf;

use dima_core::DimaError;
use dima_core::config::AnalysisParams;
use dima_core::error::Location;
use dima_core::validation::Inspection;

use self::hcs::HcsState;
use self::header_editor::HeaderEditor;
use self::options::AnalysisOptions;
use self::recent::RecentFiles;
use self::session::Session;
use self::settings::Settings;
use self::toasts::Toasts;
use crate::charts::metric::ChartMetric;
use crate::workers::analysis::RunFacts;
use crate::workers::error_context::RecordExcerpt;
use crate::workers::export::ExportKind;

/// The two screens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// Choose a file and the options, run.
    #[default]
    Setup,
    /// Explore the results.
    Workspace,
}

/// The analysis run of the setup screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunState {
    /// Nothing running.
    #[default]
    Idle,
    /// Running (job generation).
    Running(u64),
    /// Cancellation requested; waiting for the job to stop before another can start (G27).
    Cancelling(u64),
}

/// A run that failed, kept for its run report.
#[derive(Clone, Debug, PartialEq)]
pub struct FailedRun {
    /// The input.
    pub path: PathBuf,
    /// The parameters.
    pub params: AnalysisParams,
    /// How far it got.
    pub facts: RunFacts,
}

/// An error shown as a card (D85): what failed, where, the offending record, the hint, and
/// the assurance that nothing was written.
#[derive(Debug)]
pub struct ErrorCard {
    /// Heading.
    pub title: String,
    /// The error.
    pub error: DimaError,
    /// Where (record, line, column, byte), when the error has a location.
    pub location: Option<Location>,
    /// The offending record, read back after the failure.
    pub excerpt: Option<RecordExcerpt>,
    /// Generation of the excerpt being read.
    pub excerpt_pending: Option<u64>,
    /// The failed run, for "Save run report…".
    pub failed_run: Option<FailedRun>,
}

impl ErrorCard {
    /// A card for an error.
    #[must_use]
    pub fn new(title: impl Into<String>, error: DimaError) -> Self {
        let location = location_of(&error);
        Self {
            title: title.into(),
            error,
            location,
            excerpt: None,
            excerpt_pending: None,
            failed_run: None,
        }
    }

    /// The text copied by "Copy details": everything on the card plus version and platform.
    #[must_use]
    pub fn details(&self) -> String {
        let mut s = format!("{}\n\n{}", self.title, self.error.report());
        if let Some(l) = &self.location {
            for (k, v) in [
                ("record", l.record),
                ("line", l.line),
                ("column", l.column),
                ("byte", l.byte),
            ] {
                if let Some(v) = v {
                    s.push_str(&format!("\n{k}: {v}"));
                }
            }
        }
        if let Some(e) = &self.excerpt {
            s.push_str(&format!("\nheader: >{}", e.header));
            if let Some(c) = &e.around_column {
                s.push_str(&format!("\nsequence: {c}"));
            }
        }
        if self.failed_run.is_some() {
            s.push_str("\n\nNo results were written.");
        }
        s.push_str(&format!(
            "\n\ndima-gui {} ({}; {} {})",
            env!("CARGO_PKG_VERSION"),
            env!("DIMA_GUI_TARGET"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        s
    }
}

/// The location carried by an input error.
#[must_use]
pub fn location_of(e: &DimaError) -> Option<Location> {
    match e {
        DimaError::InputFormat { location, .. } => Some(location.as_ref().clone()),
        DimaError::InputData {
            location: Some(location),
            ..
        } => Some(location.as_ref().clone()),
        _ => None,
    }
}

/// The setup screen.
#[derive(Debug, Default)]
pub struct SetupState {
    /// The selected alignment.
    pub file: Option<PathBuf>,
    /// The file being opened (job generation and path).
    pub opening: Option<(u64, PathBuf)>,
    /// The pre-check of `file`.
    pub inspection: Option<Box<Inspection>>,
    /// Why the last file could not be opened.
    pub open_error: Option<ErrorCard>,
    /// The options.
    pub options: AnalysisOptions,
    /// The header-format editor.
    pub header: HeaderEditor,
    /// A note from the last paste into the header editor.
    pub header_note: Option<String>,
    /// The run.
    pub run: RunState,
    /// Why the last run failed.
    pub failure: Option<ErrorCard>,
    /// The path field (D100: always available).
    pub path_text: String,
    /// The open job re-inspects the selected file after it changed (G65): keep the options.
    pub reinspecting: bool,
    /// The request of the running analysis.
    pub in_flight: Option<crate::workers::analysis::Request>,
}

/// What a file dialog is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Open an alignment or a results file.
    OpenInput,
    /// Export the results of a session.
    Export {
        /// What to export.
        kind: ExportKind,
        /// The session whose results were shown when the export was chosen.
        session: u64,
        /// The metric the chart showed then (a figure is refused if it changed).
        metric: ChartMetric,
    },
    /// The run report of the failed run on the setup screen.
    FailedRunReport,
}

/// The file dialog in use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DialogState {
    /// What the open dialog is for.
    pub pending: Option<Purpose>,
    /// `true` while the native dialog is showing (the built-in one otherwise).
    pub native: bool,
}

/// A question waiting for an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    /// A file was chosen while an analysis runs (G35).
    ReplaceRun(PathBuf),
    /// A position hidden by the filters was asked for (G20).
    ClearFiltersFor(u64),
    /// The window is being closed while an export is written.
    QuitDuringExport,
    /// The extension was added to the name chosen in a save dialog, and that file exists.
    Overwrite {
        /// What the file is for.
        purpose: Purpose,
        /// The file that would be replaced.
        path: PathBuf,
    },
}

/// Secondary windows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Windows {
    /// Diagnostics and input validation (D60).
    pub diagnostics: bool,
    /// Run details (D103).
    pub run_details: bool,
    /// Cite DiMA (D89).
    pub cite: bool,
    /// About and licences.
    pub about: bool,
    /// Keyboard shortcuts.
    pub shortcuts: bool,
    /// Preferences.
    pub preferences: bool,
}

/// Everything the app knows.
pub struct AppState {
    /// The screen shown.
    pub view: View,
    /// Remembered settings.
    pub settings: Settings,
    /// The setup screen.
    pub setup: SetupState,
    /// The loaded results.
    pub session: Option<Session>,
    /// Conserved regions.
    pub hcs: HcsState,
    /// Recent files.
    pub recent: RecentFiles,
    /// Notifications.
    pub toasts: Toasts,
    /// File dialog.
    pub dialog: DialogState,
    /// The export running, if any (one at a time, whatever starts it: G6).
    pub export_busy: Option<ExportKind>,
    /// Secondary windows.
    pub windows: Windows,
    /// A pending question.
    pub confirm: Option<Confirm>,
    /// Whether the native file dialog can work here (D100).
    pub native_dialogs: bool,
    /// The time of the current frame (set by `Msg::Tick`).
    pub now: std::time::Instant,
    next_generation: u64,
}

impl AppState {
    /// The initial state with remembered settings.
    #[must_use]
    pub fn new(settings: Settings, recent: RecentFiles) -> Self {
        let setup = SetupState {
            options: AnalysisOptions::from_settings(&settings),
            ..SetupState::default()
        };
        Self {
            view: View::Setup,
            hcs: HcsState::from_settings(&settings),
            settings,
            setup,
            session: None,
            recent,
            toasts: Toasts::default(),
            dialog: DialogState::default(),
            export_busy: None,
            windows: Windows::default(),
            confirm: None,
            native_dialogs: true,
            now: std::time::Instant::now(),
            next_generation: 0,
        }
    }

    /// A new job generation (results of older generations are ignored).
    pub fn next_generation(&mut self) -> u64 {
        self.next_generation += 1;
        self.next_generation
    }

    /// `true` while an analysis runs or is being cancelled.
    #[must_use]
    pub fn running(&self) -> bool {
        self.setup.run != RunState::Idle
    }

    /// `true` while any job the user waits for is in flight (progress repaint).
    #[must_use]
    pub fn busy(&self) -> bool {
        self.running()
            || self.setup.opening.is_some()
            || self.export_busy.is_some()
            || self.hcs.running.is_some()
            || self.hcs.pending_since.is_some()
            || self.session.as_ref().is_some_and(|s| s.filter_dirty)
    }

    /// Why Analyze cannot start, shown under it and as its tooltip (and, should it be asked
    /// anyway, as "Not started: …"); `None` when it can: no file yet, an alphabet that Auto
    /// could not detect (with the library's reason, without the command line's hint), or the
    /// first problem of the options.
    #[must_use]
    pub fn not_ready(&self) -> Option<String> {
        if self.setup.file.is_none() {
            return Some("Open an alignment first".to_owned());
        }
        let inspection = self.setup.inspection.as_deref();
        if self.setup.options.alphabet.alphabet().is_none()
            && let Some(i) = inspection.filter(|i| i.alphabet.is_none())
        {
            let why = options::undetected_alphabet(i)
                .unwrap_or_else(|| "it could not be detected".to_owned());
            return Some(format!("Choose the alphabet: {why}"));
        }
        self.setup
            .options
            .problems(&self.setup.header, inspection)
            .into_iter()
            .next()
    }

    /// The settings with the current form values copied in.
    #[must_use]
    pub fn settings_snapshot(&self) -> Settings {
        let mut s = self.settings.clone();
        self.setup.options.store(&mut s);
        self.hcs.store(&mut s);
        s
    }
}
