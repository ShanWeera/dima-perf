//! The application: owns the state, the background jobs and the configuration store; each
//! frame it feeds time, worker results and user intents through the pure `update`, performs
//! the effects `update` asks for, and draws. Nothing slow happens on this thread.

pub mod msg;
pub mod update;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dima_core::DimaError;

use self::msg::{Effect, Msg};
use self::update::update;
use crate::persist::{ConfigStore, RECENT_FILE, SETTINGS_FILE};
use crate::state::recent::RecentFiles;
use crate::state::settings::{Loaded, Settings};
use crate::state::toasts::Severity;
use crate::state::{AppState, Purpose};
use crate::theme::DesignTokens;
use crate::views::{self, UiState};
use crate::workers::dialogs::{self, DialogKind, NativeDialog};
use crate::workers::error_context::RecordExcerpt;
use crate::workers::export::ExportKind;
use crate::workers::inspect::Opened;
use crate::workers::{Poll, Worker, analysis, error_context, export, hcs, inspect};

/// A running pre-check or results load: its generation, its file and its worker.
type OpenJob = (u64, PathBuf, Worker<Result<Opened, DimaError>>);

/// Running background jobs, each tagged with the generation `update` gave it.
#[derive(Default)]
struct Jobs {
    open: Option<OpenJob>,
    analysis: Option<(u64, analysis::Job)>,
    error_context: Option<(u64, Worker<Option<RecordExcerpt>>)>,
    hcs: Option<(u64, Worker<hcs::Outcome>)>,
    export: Option<(ExportKind, Option<u64>, Worker<export::Outcome>)>,
    native_dialog: Option<(Purpose, NativeDialog)>,
    built_in_dialog: Option<(Purpose, Box<egui_file_dialog::FileDialog>)>,
}

/// The app.
pub struct DimaApp {
    /// Application state (public for the interface tests).
    pub state: AppState,
    ui: UiState,
    jobs: Jobs,
    store: ConfigStore,
    tokens: DesignTokens,
    ctx: Option<egui::Context>,
    /// Closing was confirmed: the next close request is not questioned.
    quitting: bool,
}

impl DimaApp {
    /// The app with its settings and recent files from `store`. `gpu` tells whether the chart
    /// line can be drawn by wgpu.
    #[must_use]
    pub fn new(store: ConfigStore, gpu: bool) -> Self {
        let (settings, loaded) = store.read(SETTINGS_FILE).map_or_else(
            || (Settings::default(), Loaded::Ok),
            |text| Settings::from_json(&text),
        );
        // Corrected settings are saved at once, so their notice is shown once.
        if matches!(loaded, Loaded::Adjusted(_)) {
            store.write(SETTINGS_FILE, settings.to_json());
        }
        let recent = store
            .read(RECENT_FILE)
            .map_or_else(RecentFiles::default, |text| RecentFiles::from_json(&text));
        let mut state = AppState::new(settings, recent);
        state.native_dialogs = dialogs::native_available();
        match loaded {
            Loaded::Ok => {}
            Loaded::Adjusted(n) => state.toasts.push(Severity::Info, n, None, Instant::now()),
            Loaded::Reset(n) => state
                .toasts
                .push(Severity::Warning, n, None, Instant::now()),
        }
        Self {
            state,
            ui: UiState {
                gpu,
                ..UiState::default()
            },
            jobs: Jobs::default(),
            store,
            tokens: DesignTokens::light(),
            ctx: None,
            quitting: false,
        }
    }

    /// The app for eframe: fonts, styles, the GPU line, the user's configuration.
    #[must_use]
    pub fn from_creation_context(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::fonts::install(&cc.egui_ctx);
        crate::theme::style::install(&cc.egui_ctx);
        let gpu = cc.wgpu_render_state.as_ref().is_some_and(|rs| {
            crate::charts::gpu_line::init(rs);
            true
        });
        Self::new(ConfigStore::system(), gpu)
    }

    /// One frame: time and worker results, then the interface, then its intents.
    pub fn frame(&mut self, ui: &mut egui::Ui, frame: Option<&eframe::Frame>) {
        let ctx = ui.ctx().clone();
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }
        self.tokens = DesignTokens::for_theme(ctx.theme());
        let now = Instant::now();
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        self.dispatch(Msg::Tick { now, pointer_down }, frame);
        if !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            self.dispatch(Msg::CloseRequested, frame);
        }
        for msg in self.poll_jobs() {
            self.dispatch(msg, frame);
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.dispatch(Msg::Dropped(dropped), frame);
        }
        self.ui.progress = self
            .jobs
            .analysis
            .as_ref()
            .map(|(_, job)| crate::panels::run::progress_of(job));
        let msgs = views::render(ui, &mut self.state, &mut self.ui, &self.tokens);
        let changed = !msgs.is_empty();
        for msg in msgs {
            self.dispatch(msg, frame);
        }
        self.show_built_in_dialog(&ctx, frame);
        if changed {
            ctx.request_repaint();
        }
        if self.state.busy() || self.jobs.native_dialog.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some(at) = self
            .state
            .toasts
            .items()
            .iter()
            .filter_map(crate::state::toasts::Toast::expires_at)
            .min()
        {
            ctx.request_repaint_after(at.saturating_duration_since(now));
        }
    }

    /// Applies a message and performs the resulting effects.
    pub fn dispatch(&mut self, msg: Msg, frame: Option<&eframe::Frame>) {
        for effect in update(&mut self.state, msg) {
            self.perform(effect, frame);
        }
    }

    fn perform(&mut self, effect: Effect, frame: Option<&eframe::Frame>) {
        let ctx = self.ctx.clone();
        match effect {
            Effect::Open { generation, path } => {
                let p = path.clone();
                let w = Worker::spawn(
                    "open",
                    crate::workers::Priority::Normal,
                    ctx,
                    move |cancel| inspect::open(&p, &cancel),
                );
                self.jobs.open = Some((generation, path, w));
            }
            Effect::CancelOpen => {
                if let Some((_, _, w)) = self.jobs.open.take() {
                    w.cancel();
                }
            }
            Effect::Analyze {
                generation,
                request,
            } => {
                self.jobs.analysis = Some((generation, analysis::spawn(*request, ctx)));
            }
            Effect::CancelAnalysis => {
                if let Some((_, job)) = &self.jobs.analysis {
                    job.worker.cancel();
                }
            }
            Effect::ReadErrorContext {
                generation,
                path,
                record,
                column,
            } => {
                if let Some((_, w)) = self.jobs.error_context.take() {
                    w.cancel();
                }
                self.jobs.error_context =
                    Some((generation, error_context::spawn(path, record, column, ctx)));
            }
            Effect::ComputeHcs {
                generation,
                results,
                params,
            } => {
                self.jobs.hcs = Some((generation, hcs::spawn(results, params, ctx)));
            }
            Effect::Dialog {
                purpose,
                kind,
                title,
                built_in,
            } => self.open_dialog(purpose, &kind, &title, built_in, frame),
            Effect::Export { session, job } => {
                let kind = job.kind;
                self.jobs.export = Some((kind, session, export::spawn(*job, ctx)));
            }
            Effect::CancelClose => {
                if let Some(ctx) = &ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
            Effect::Quit => {
                self.quitting = true;
                if let Some(ctx) = &ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Effect::SaveSettings(text) => self.store.write(SETTINGS_FILE, text),
            Effect::SaveRecent(text) => self.store.write(RECENT_FILE, text),
        }
    }

    fn open_dialog(
        &mut self,
        purpose: Purpose,
        kind: &DialogKind,
        title: &str,
        built_in: bool,
        frame: Option<&eframe::Frame>,
    ) {
        let start = self.start_directory();
        if !built_in {
            // A dialog that is open (or abandoned) for another purpose is replaced.
            self.jobs.built_in_dialog = None;
            let d = dialogs::spawn_native(kind, title, start.as_deref(), frame, self.ctx.clone());
            self.jobs.native_dialog = Some((purpose, d));
            return;
        }
        self.jobs.native_dialog = None;
        let mut d = egui_file_dialog::FileDialog::new()
            .title(title)
            .id(egui::Id::new("dima_file_dialog"))
            .allow_file_overwrite(true);
        if let Some(dir) = start {
            d = d.initial_directory(dir);
        }
        match kind {
            DialogKind::Open => d.pick_file(),
            DialogKind::Save { default_name, .. } => {
                d = d.default_file_name(default_name);
                d.save_file();
            }
            DialogKind::Folder => d.pick_directory(),
        }
        self.jobs.built_in_dialog = Some((purpose, Box::new(d)));
    }

    /// Where dialogs start: the folder of the current file, else the user's documents.
    fn start_directory(&self) -> Option<PathBuf> {
        self.state
            .setup
            .file
            .as_ref()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .filter(|p| p.is_dir())
            .or_else(dirs::document_dir)
    }

    fn show_built_in_dialog(&mut self, ctx: &egui::Context, frame: Option<&eframe::Frame>) {
        let Some((purpose, dialog)) = self.jobs.built_in_dialog.as_mut() else {
            return;
        };
        let purpose = *purpose;
        if self.state.dialog.pending != Some(purpose) {
            // The state no longer waits for this dialog (it was answered another way).
            self.jobs.built_in_dialog = None;
            return;
        }
        dialog.update(ctx);
        let closed = match dialog.state() {
            egui_file_dialog::DialogState::Open => None,
            egui_file_dialog::DialogState::Picked(p) => Some(Some(p.clone())),
            _ => Some(None),
        };
        if let Some(path) = closed {
            self.jobs.built_in_dialog = None;
            self.dispatch(
                Msg::DialogClosed {
                    purpose,
                    path,
                    failed_fast: false,
                },
                frame,
            );
        }
    }

    /// Collects finished jobs as messages.
    fn poll_jobs(&mut self) -> Vec<Msg> {
        let mut out = Vec::new();
        let j = &mut self.jobs;
        if let Some((generation, path, w)) = j.open.as_mut() {
            match w.poll() {
                Poll::Pending => {}
                Poll::Ready(outcome) => {
                    out.push(Msg::Opened {
                        generation: *generation,
                        path: path.clone(),
                        outcome,
                    });
                    j.open = None;
                }
                Poll::Panicked(message) => {
                    out.push(Msg::Panicked {
                        what: "opening the file",
                        message,
                    });
                    j.open = None;
                }
                Poll::Refused(reason) => {
                    out.push(Msg::Refused {
                        what: "opening the file",
                        reason,
                    });
                    j.open = None;
                }
            }
        }
        if let Some((generation, job)) = j.analysis.as_mut() {
            match job.worker.poll() {
                Poll::Pending => {}
                Poll::Ready(outcome) => {
                    out.push(Msg::AnalysisDone {
                        generation: *generation,
                        outcome,
                    });
                    j.analysis = None;
                }
                Poll::Panicked(message) => {
                    out.push(Msg::Panicked {
                        what: "analysing",
                        message,
                    });
                    j.analysis = None;
                }
                Poll::Refused(reason) => {
                    out.push(Msg::Refused {
                        what: "analysing",
                        reason,
                    });
                    j.analysis = None;
                }
            }
        }
        if let Some((generation, w)) = j.error_context.as_mut() {
            match w.poll() {
                Poll::Pending => {}
                Poll::Ready(excerpt) => {
                    out.push(Msg::ErrorContext {
                        generation: *generation,
                        excerpt,
                    });
                    j.error_context = None;
                }
                Poll::Panicked(_) | Poll::Refused(_) => {
                    out.push(Msg::ErrorContext {
                        generation: *generation,
                        excerpt: None,
                    });
                    j.error_context = None;
                }
            }
        }
        if let Some((generation, w)) = j.hcs.as_mut() {
            match w.poll() {
                Poll::Pending => {}
                Poll::Ready(outcome) => {
                    out.push(Msg::HcsDone {
                        generation: *generation,
                        outcome,
                    });
                    j.hcs = None;
                }
                Poll::Panicked(message) => {
                    out.push(Msg::Panicked {
                        what: "computing conserved regions",
                        message,
                    });
                    j.hcs = None;
                }
                Poll::Refused(reason) => {
                    out.push(Msg::Refused {
                        what: "computing conserved regions",
                        reason,
                    });
                    j.hcs = None;
                }
            }
        }
        if let Some((kind, session, w)) = j.export.as_mut() {
            match w.poll() {
                Poll::Pending => {}
                Poll::Ready(outcome) => {
                    out.push(Msg::ExportDone {
                        kind: *kind,
                        session: *session,
                        outcome,
                    });
                    j.export = None;
                }
                Poll::Panicked(message) => {
                    out.push(Msg::Panicked {
                        what: "exporting",
                        message,
                    });
                    j.export = None;
                }
                Poll::Refused(reason) => {
                    out.push(Msg::Refused {
                        what: "exporting",
                        reason,
                    });
                    j.export = None;
                }
            }
        }
        if let Some((purpose, d)) = j.native_dialog.as_mut() {
            match d.worker.poll() {
                Poll::Pending => {}
                Poll::Ready(path) => {
                    let failed_fast = path.is_none() && d.opened.elapsed() < dialogs::FAILED_FAST;
                    out.push(Msg::DialogClosed {
                        purpose: *purpose,
                        path,
                        failed_fast,
                    });
                    j.native_dialog = None;
                }
                Poll::Panicked(_) | Poll::Refused(_) => {
                    out.push(Msg::DialogClosed {
                        purpose: *purpose,
                        path: None,
                        failed_fast: true,
                    });
                    j.native_dialog = None;
                }
            }
        }
        out
    }

    /// Saves pending configuration writes (at exit).
    pub fn flush(&self) {
        self.store.flush();
    }
}

impl eframe::App for DimaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.frame(ui, Some(frame));
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // Nothing to do: settings and recent files are written in the background as they
        // change, and flushed at exit. Waiting here would stall the interface every time eframe
        // saves its window state.
    }

    fn on_exit(&mut self) {
        // Ask running jobs to stop; their threads end with the process, so the temporary file
        // of an unfinished export is removed here (the destination is left as it was).
        if let Some((_, job)) = &self.jobs.analysis {
            job.worker.cancel();
        }
        if let Some((_, _, w)) = &self.jobs.export {
            w.cancel();
        }
        dima_core::output::remove_temporaries();
        self.flush();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.tokens.surface_primary.to_normalized_gamma_f32()
    }
}

/// Starts the app.
///
/// # Errors
/// When the window or the renderer cannot be created.
pub fn run() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([980.0, 620.0])
        .with_drag_and_drop(true)
        .with_title("DiMA")
        .with_app_id("dima-gui");
    if let Some(icon) = crate::platform::icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        persist_window: true,
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "dima-gui",
        options,
        Box::new(|cc| Ok(Box::new(DimaApp::from_creation_context(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved figure size below the minimum is raised, said once and written back once; an
    /// invalid value still resets everything and leaves the file alone.
    #[test]
    fn adjusted_settings_are_saved_once() {
        let store = ConfigStore::in_memory();
        store.write(
            SETTINGS_FILE,
            r#"{"version": 1, "settings": {"support_threshold": 30, "figure_size": [1800, 150]}}"#
                .to_owned(),
        );
        let app = DimaApp::new(store, false);
        assert_eq!(app.state.settings.figure_size, (1800, 320));
        assert_eq!(app.state.settings.support_threshold, 30);
        let notes: Vec<_> = app.state.toasts.items().iter().collect();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].severity, Severity::Info);
        assert!(notes[0].text.ends_with("raised to 1800 \u{d7} 320 px"));
        let saved = app.store.read(SETTINGS_FILE).unwrap();
        assert_eq!(saved, app.state.settings.to_json(), "written back");
        // The next start reads the corrected file: no notice, nothing written.
        let again = DimaApp::new(app.store, false);
        assert!(again.state.toasts.items().is_empty());
        assert_eq!(again.store.read(SETTINGS_FILE).unwrap(), saved);
        assert_eq!(again.state.settings.figure_size, (1800, 320));

        let store = ConfigStore::in_memory();
        let bad =
            r#"{"version": 1, "settings": {"support_threshold": 0, "figure_size": [1800, 150]}}"#;
        store.write(SETTINGS_FILE, bad.to_owned());
        let app = DimaApp::new(store, false);
        assert_eq!(app.state.settings, Settings::default(), "reset");
        assert_eq!(app.state.toasts.items()[0].severity, Severity::Warning);
        assert_eq!(
            app.store.read(SETTINGS_FILE).unwrap(),
            bad,
            "not written back"
        );
    }
}
