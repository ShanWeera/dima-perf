//! DimaApp: top-level application state and eframe::App implementation.
//!
//! Uses the egui 0.34 `logic()` + `ui()` split for SRP:
//! - `logic()`: state mutation (polling workers, checking channels, progress)
//! - `ui()`: rendering (panels, charts, buttons)

use dima_lib::{compute_hcs_regions, HcsRegion, Results, Variant};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use crate::charts::axis::nice_ticks;
use crate::charts::gpu_line::{
    init_gpu_resources, EntropyLineCallback, EntropyVertex, ViewTransform,
};
use crate::charts::lttb::{lttb_downsample_by_range, Point};
use crate::error::{ErrorMessage, ErrorState};
use crate::state::{FilterState, MotifType, RecentFileStore, SortDirection, TableSort};
use crate::theme::{apply_theme, init_both_theme_styles, DesignTokens, Theme};
use crate::views::View;

/// Maximum number of points to render in the entropy chart before
/// LTTB downsampling kicks in. Keeps the per-frame line segment
/// count manageable for CPU rendering (~800 draw calls).
const ENTROPY_CHART_MAX_POINTS: usize = 800;

/// Cached per-variant metadata: ((position_number, variant_index), sorted fields with sorted values).
/// Keyed by both position AND variant index so switching variants at the same position
/// correctly invalidates the cache.
type CachedMetadata = ((usize, usize), Vec<(String, Vec<(String, usize)>)>);

/// Keyboard navigation actions for filtered position traversal.
enum NavAction {
    Next,
    Previous,
    First,
    Last,
}

/// Outcome of a background analysis run.
enum AnalysisOutcome {
    Success {
        results: Box<Results>,
        validation_stats: Option<dima_lib::ValidationStats>,
        perf_report: dima_lib::PerfReport,
    },
    Error(String),
    Cancelled,
}

/// Handle to a running background analysis.
struct AnalysisHandle {
    result_rx: mpsc::Receiver<AnalysisOutcome>,
    cancel_token: Arc<AtomicBool>,
    progress_counter: Arc<AtomicUsize>,
    total_positions: usize,
}

/// Handle to a running background FASTA validation.
/// Separated from AnalysisHandle because validation is a distinct phase
/// with different cancellation and result semantics. Selecting a new file
/// cancels any in-progress validation via the `cancel_token`.
struct ValidationHandle {
    result_rx: mpsc::Receiver<Result<dima_lib::FastaValidationResult, std::io::Error>>,
    cancel_token: Arc<AtomicBool>,
}

/// User's alphabet selection in the Setup UI.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AlphabetChoice {
    Protein,
    Nucleotide,
    #[default]
    Auto,
}

/// UI-level analysis configuration.
/// Maps to the analyze() function's parameters + AnalysisConfig struct.
#[derive(Debug, Clone)]
pub struct AnalysisConfigUi {
    pub alphabet: AlphabetChoice,
    pub kmer_length: usize,
    /// IMPORTANT: This is a COUNT (1-10000), NOT a percentage
    pub support_threshold: usize,
    pub query_name: String,
    /// Pre-split header field names (e.g., ["id", "country", "host"]).
    /// Stored as structured data rather than a raw format string to avoid
    /// needing to know the original delimiter at split time.
    pub header_format: Option<Vec<String>>,
    pub header_fillna: Option<String>,
    pub metadata_fields: Vec<String>,
    pub validation_mode: dima_lib::ValidationMode,
    pub allow_lowercase: bool,
    /// Defaults to true so ValidationStats is available for the summary display
    pub report_invalid: bool,
}

impl Default for AnalysisConfigUi {
    fn default() -> Self {
        Self {
            alphabet: AlphabetChoice::Auto,
            kmer_length: 9,
            support_threshold: 30,
            query_name: String::new(),
            header_format: None,
            header_fillna: None,
            metadata_fields: Vec::new(),
            validation_mode: dima_lib::ValidationMode::default(),
            allow_lowercase: false,
            report_invalid: true,
        }
    }
}

/// Post-analysis settings (not parameters to analyze()).
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub hcs_threshold: f64,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            hcs_threshold: 95.0,
        }
    }
}

/// Top-level application state.
pub struct DimaApp {
    // Navigation
    pub current_view: View,

    // Theme (togglable via show_theme_toggle in workspace top bar and setup view)
    pub theme: Theme,
    pub tokens: DesignTokens,

    // Error handling
    pub error_state: ErrorState,

    // Setup phase state
    pub selected_file: Option<PathBuf>,
    pub validation_result: Option<dima_lib::FastaValidationResult>,
    pub analysis_config: AnalysisConfigUi,
    pub workspace_config: WorkspaceConfig,

    // Validation state (background FASTA validation)
    validation_handle: Option<ValidationHandle>,

    // Analysis state
    analysis_handle: Option<AnalysisHandle>,

    // Results state (direct Rust struct -- no serialization)
    pub results: Option<Arc<Results>>,
    pub validation_stats: Option<dima_lib::ValidationStats>,
    pub perf_report: Option<dima_lib::PerfReport>,

    // Derived data (recomputed when results or workspace_config change)
    pub hcs_regions: Vec<HcsRegion>,
    pub available_metadata_fields: Vec<String>,

    // Filter state
    pub filter_state: FilterState,
    pub filtered_positions: Vec<usize>,

    // O(1) lookup: maps position number → index in results.results Vec.
    // Rebuilt whenever results change (analysis success or .dima import).
    // Replaces three per-frame O(n) iter().find() calls in detail panels.
    pub position_index_map: std::collections::HashMap<usize, usize>,

    // Selection state (cross-panel sync) -- indexes into UNFILTERED results
    pub selected_position: Option<usize>,
    /// Position under the mouse cursor on the entropy chart (used for future
    /// hover-highlight synchronization across panels)
    #[allow(dead_code)]
    pub hovered_position: Option<usize>,

    // ── Sort state (one per sortable table) ──
    pub position_explorer_sort: Option<TableSort>,
    pub variant_table_sort: Option<TableSort>,

    // ── Cached per-variant metadata ──
    /// Pre-extracted metadata for the currently selected variant at the selected position.
    /// Cached to avoid per-frame re-computation and HashMap non-deterministic ordering.
    /// The key `(position_number, variant_index)` invalidates when either changes.
    cached_metadata: Option<CachedMetadata>,

    /// Tracks which metadata fields have their "Others" bucket expanded to show all values.
    /// Cleared whenever cached_metadata is invalidated (position/variant change).
    expanded_metadata_fields: std::collections::HashSet<String>,

    /// Index into the current position's `diversity_motifs` Vec for the selected variant.
    /// `None` means no variant is selected (no metadata shown).
    /// Auto-set to the default variant (Index or highest-incidence) when a position is selected.
    pub selected_variant_index: Option<usize>,

    // Entropy chart viewport (zoom/pan state)
    /// Visible x-range as (start_position, end_position), both 1-based.
    /// None means "show all" (no zoom applied).
    pub entropy_viewport: Option<(f64, f64)>,

    // GPU chart state
    pub data_version: u64,
    pub charts_need_data_upload: bool,
    /// Whether the wgpu GPU pipeline was successfully initialized.
    /// Falls back to CPU rendering if GPU init failed (e.g., headless CI).
    gpu_available: bool,

    // Recent files (persisted to platform config dir)
    pub recent_files: RecentFileStore,

    // egui context clone for request_repaint from workers
    egui_ctx: Option<egui::Context>,
}

impl DimaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::default();
        let tokens = match theme {
            Theme::Dark => DesignTokens::dark(),
            Theme::Light => DesignTokens::light(),
        };
        // Pre-populate BOTH theme slots in egui's Options with our custom
        // styles, so toggling always uses our design tokens (not egui defaults).
        init_both_theme_styles(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, &tokens, theme);

        // Disable egui's default Ctrl/Cmd+Scroll → UI zoom conversion so that
        // Ctrl+Scroll reaches our entropy chart zoom handler as a normal scroll
        // event. Pinch-to-zoom and keyboard zoom (Ctrl+Plus/Minus/0) are unaffected
        // — they use separate code paths in egui. Same pattern used by Matplotlib,
        // Figma, and VS Code for custom scroll-zoom.
        cc.egui_ctx.options_mut(|o| {
            o.input_options.zoom_modifier = egui::Modifiers::NONE;
        });

        // Initialize GPU resources for the entropy chart if wgpu is available.
        // Falls back to CPU rendering in headless/CI environments.
        let gpu_available = if let Some(ref render_state) = cc.wgpu_render_state {
            init_gpu_resources(render_state);
            true
        } else {
            tracing::warn!("wgpu render state not available — using CPU chart rendering");
            false
        };

        Self {
            current_view: View::Setup,
            theme,
            tokens,
            error_state: ErrorState::default(),
            selected_file: None,
            validation_result: None,
            analysis_config: AnalysisConfigUi::default(),
            workspace_config: WorkspaceConfig::default(),
            validation_handle: None,
            analysis_handle: None,
            results: None,
            validation_stats: None,
            perf_report: None,
            hcs_regions: Vec::new(),
            available_metadata_fields: Vec::new(),
            filter_state: FilterState {
                position_range: (1, 1),
                entropy_range: (0.0, 0.0),
                motif_types: vec![
                    MotifType::Index,
                    MotifType::Major,
                    MotifType::Minor,
                    MotifType::Unique,
                ],
                include_low_support: true,
            },
            filtered_positions: Vec::new(),
            position_index_map: std::collections::HashMap::new(),
            selected_position: None,
            hovered_position: None,
            position_explorer_sort: None,
            variant_table_sort: None,
            cached_metadata: None,
            expanded_metadata_fields: std::collections::HashSet::new(),
            selected_variant_index: None,
            entropy_viewport: None,
            data_version: 0,
            charts_need_data_upload: false,
            gpu_available,
            recent_files: RecentFileStore::load(),
            egui_ctx: None,
        }
    }

    /// Render a section header with consistent visual hierarchy.
    /// 15pt + strong creates a clear but compact distinction from 13pt body text,
    /// following IBM Carbon Design System's "productive-heading-02" pattern
    /// for data-dense task UIs.
    fn section_header(ui: &mut egui::Ui, text: impl Into<String>) {
        ui.label(egui::RichText::new(text).size(15.0).strong());
    }

    /// Find the default variant to auto-select at the given position.
    ///
    /// Strategy (matches DiMA web server behavior per PMC11596295):
    /// 1. Prefer the Index variant (motif_short == "I") with highest incidence
    /// 2. If no Index variant exists (highly diverse positions), fall back to
    ///    the variant with the highest incidence regardless of motif type
    ///
    /// Returns None only if no variants exist at all.
    fn find_default_variant(&self, results: &Results, pos_num: usize) -> Option<usize> {
        let idx = self.position_index_map.get(&pos_num)?;
        let pos = &results.results[*idx];
        let variants = pos.diversity_motifs.as_ref()?;
        if variants.is_empty() {
            return None;
        }

        // Prefer Index variant (motif_short == "I")
        let index_variant = variants
            .iter()
            .enumerate()
            .filter(|(_, v)| v.motif_short.as_deref() == Some("I"))
            .max_by(|(_, a), (_, b)| {
                a.incidence
                    .partial_cmp(&b.incidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i);

        if index_variant.is_some() {
            return index_variant;
        }

        // Fallback: highest-incidence variant regardless of motif
        variants
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.incidence
                    .partial_cmp(&b.incidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Extract metadata from a single selected variant.
    /// Returns a deterministically sorted Vec of (field_name, sorted_values).
    /// Values within each field are sorted by count descending, then by name
    /// ascending (tiebreaker) to prevent flickering from HashMap ordering.
    fn extract_variant_metadata(variant: &Variant) -> Vec<(String, Vec<(String, usize)>)> {
        let meta = match variant.metadata {
            Some(ref m) => m,
            None => return Vec::new(),
        };

        let mut result: Vec<(String, Vec<(String, usize)>)> = meta
            .iter()
            .map(|(field, values)| {
                let mut sorted_values: Vec<(String, usize)> =
                    values.iter().map(|(k, v)| (k.clone(), *v)).collect();
                sorted_values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                (field.clone(), sorted_values)
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Start a background analysis on a separate thread.
    fn start_analysis(&mut self, file_path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        let cancel_token = Arc::new(AtomicBool::new(false));
        let progress_counter = Arc::new(AtomicUsize::new(0));

        // Compute total_positions from validation result for progress bar.
        // Guard against edge case: alignment_length < kmer_length => 0 positions.
        let total_positions = self
            .validation_result
            .as_ref()
            .and_then(|vr| {
                vr.alignment_length
                    .filter(|&len| len >= self.analysis_config.kmer_length)
                    .map(|len| len - self.analysis_config.kmer_length + 1)
            })
            .unwrap_or(0);

        let ctx_clone = self.egui_ctx.clone();
        let config = self.analysis_config.clone();
        let cancel = cancel_token.clone();
        let progress = progress_counter.clone();

        std::thread::spawn(move || {
            let analysis_config = dima_lib::AnalysisConfig::new()
                .with_validation_mode(config.validation_mode)
                .with_allow_lowercase(config.allow_lowercase)
                .with_report_invalid(config.report_invalid)
                .with_cancel_token(cancel.clone())
                .with_progress_counter(progress);

            let header_format_vec: Option<Vec<String>> = config.header_format.clone();

            let metadata_fields_vec: Option<Vec<String>> = if config.metadata_fields.is_empty() {
                None
            } else {
                Some(config.metadata_fields.clone())
            };

            let alphabet: Option<String> = match config.alphabet {
                AlphabetChoice::Protein => Some("protein".to_string()),
                AlphabetChoice::Nucleotide => Some("nucleotide".to_string()),
                AlphabetChoice::Auto => None,
            };

            let query_name = if config.query_name.is_empty() {
                file_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                config.query_name.clone()
            };

            let input_source = dima_lib::InputSource::File(file_path);

            let outcome = match dima_lib::analyze(
                input_source,
                config.kmer_length,
                config.support_threshold,
                query_name,
                header_format_vec,
                alphabet,
                config.header_fillna.clone(),
                metadata_fields_vec,
                Some(analysis_config),
            ) {
                Ok((results, validation_stats, perf_report)) => AnalysisOutcome::Success {
                    results: Box::new(results),
                    validation_stats,
                    perf_report,
                },
                Err(dima_lib::AnalysisError::Cancelled) => AnalysisOutcome::Cancelled,
                Err(e) => AnalysisOutcome::Error(e.to_string()),
            };

            let _ = tx.send(outcome);
            if let Some(ctx) = ctx_clone {
                ctx.request_repaint();
            }
        });

        self.analysis_handle = Some(AnalysisHandle {
            result_rx: rx,
            cancel_token,
            progress_counter,
            total_positions,
        });
    }

    /// Cancel a running analysis.
    fn cancel_analysis(&mut self) {
        if let Some(ref handle) = self.analysis_handle {
            handle.cancel_token.store(true, Ordering::Relaxed);
        }
    }

    /// Scan all positions to discover every metadata field name.
    fn scan_all_metadata_fields(results: &Results) -> Vec<String> {
        let mut fields = std::collections::HashSet::new();
        for pos in &results.results {
            if let Some(ref variants) = pos.diversity_motifs {
                for v in variants {
                    if let Some(ref meta) = v.metadata {
                        for key in meta.keys() {
                            fields.insert(key.clone());
                        }
                    }
                }
            }
        }
        let mut sorted: Vec<String> = fields.into_iter().collect();
        sorted.sort();
        sorted
    }

    /// Load analysis results from a .dima binary file (skips analysis).
    pub fn load_dima_binary(&mut self, path: &std::path::Path) {
        // Cancel any running analysis first to prevent it from overwriting
        // imported results when it completes. Dropping the handle drops the
        // Receiver; the worker's send() silently fails and exits naturally.
        if self.analysis_handle.is_some() {
            self.cancel_analysis();
            self.analysis_handle = None;
        }

        // Guard against non-UTF-8 paths (Results::from_binary takes String)
        let path_str = match path.to_str() {
            Some(s) => s.to_string(),
            None => {
                self.error_state.push(ErrorMessage::error(
                    "File path contains non-UTF-8 characters and cannot be loaded.".to_string(),
                ));
                return;
            }
        };

        match Results::from_binary(path_str) {
            Ok(results) => {
                let results = Arc::new(results);
                // Only add to recent files after successful load, with actual
                // sequence count from the Results. This prevents corrupted .dima
                // files from appearing in recent files and triggering repeat errors.
                self.recent_files
                    .add(path.to_path_buf(), Some(results.sequence_count), None);
                self.hcs_regions =
                    compute_hcs_regions(&results, Some(self.workspace_config.hcs_threshold));
                self.available_metadata_fields = Self::scan_all_metadata_fields(&results);
                self.filter_state = FilterState::default_for(&results);
                self.filtered_positions = self.filter_state.apply(&results);
                self.position_index_map = results
                    .results
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.position, i))
                    .collect();
                self.results = Some(results);
                self.validation_stats = None;
                self.perf_report = None;
                self.selected_position = None;
                self.selected_variant_index = None;
                self.position_explorer_sort = None;
                self.variant_table_sort = None;
                self.cached_metadata = None;
                self.expanded_metadata_fields.clear();
                self.entropy_viewport = None;
                self.data_version += 1;
                self.charts_need_data_upload = true;
                self.current_view = View::Workspace;
                self.error_state.push(ErrorMessage::success(format!(
                    "Loaded analysis from {}",
                    path.display()
                )));
            }
            Err(e) => {
                self.error_state.push(ErrorMessage::error(format!(
                    "Failed to load .dima file: {}",
                    e
                )));
            }
        }
    }
}

// ─── eframe::App trait ──────────────────────────────────────────────────────

impl eframe::App for DimaApp {
    /// Logic phase: state mutation, worker polling, auto-dismiss.
    /// Called once before each `ui()` call, and additionally when
    /// `request_repaint()` fires while the UI is hidden.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Store ctx clone on first frame for worker threads
        if self.egui_ctx.is_none() {
            self.egui_ctx = Some(ctx.clone());
        }

        // Sync DesignTokens with the active egui theme so that
        // `global_theme_preference_switch` changes (which modify ctx.theme()
        // directly) are reflected in our token-based rendering.
        let active_theme = match ctx.theme() {
            egui::Theme::Dark => Theme::Dark,
            egui::Theme::Light => Theme::Light,
        };
        if active_theme != self.theme {
            self.theme = active_theme;
            self.tokens = match self.theme {
                Theme::Dark => DesignTokens::dark(),
                Theme::Light => DesignTokens::light(),
            };
            apply_theme(ctx, &self.tokens, self.theme);
        }

        // Poll background analysis (non-blocking)
        if let Some(ref handle) = self.analysis_handle {
            // Request periodic repaints while analysis is active so progress
            // bar updates even without user interaction (egui is event-driven)
            ctx.request_repaint_after(std::time::Duration::from_millis(100));

            // Use match (not if-let) to handle ALL try_recv outcomes.
            // Disconnected = worker thread exited without sending a result.
            match handle.result_rx.try_recv() {
                Ok(outcome) => {
                    match outcome {
                        AnalysisOutcome::Success {
                            results,
                            validation_stats,
                            perf_report,
                        } => {
                            let results = Arc::new(*results);
                            self.hcs_regions = compute_hcs_regions(
                                &results,
                                Some(self.workspace_config.hcs_threshold),
                            );
                            self.available_metadata_fields =
                                Self::scan_all_metadata_fields(&results);
                            self.filter_state = FilterState::default_for(&results);
                            self.filtered_positions = self.filter_state.apply(&results);
                            self.position_index_map = results
                                .results
                                .iter()
                                .enumerate()
                                .map(|(i, p)| (p.position, i))
                                .collect();
                            self.results = Some(results);
                            self.validation_stats = validation_stats;
                            self.perf_report = Some(perf_report);
                            self.selected_position = None;
                            self.selected_variant_index = None;
                            self.position_explorer_sort = None;
                            self.variant_table_sort = None;
                            self.cached_metadata = None;
                            self.expanded_metadata_fields.clear();
                            self.entropy_viewport = None;
                            self.data_version += 1;
                            self.charts_need_data_upload = true;
                            self.current_view = View::Workspace;
                        }
                        AnalysisOutcome::Error(msg) => {
                            self.error_state.push(ErrorMessage::error(msg));
                        }
                        AnalysisOutcome::Cancelled => {
                            // Stay on Setup view, progress bar disappears
                        }
                    }
                    self.analysis_handle = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still running -- progress bar continues
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Worker crashed without sending (panic in debug, or bug).
                    // In release (panic=abort), app would have already crashed.
                    self.error_state.push(ErrorMessage::error(
                        "Analysis worker crashed unexpectedly. Check logs for details.".to_string(),
                    ));
                    self.analysis_handle = None;
                }
            }
        }

        // Poll background FASTA validation (non-blocking).
        // Validation runs on a background thread to keep the UI responsive
        // during decompression and scanning of large/compressed files.
        if let Some(ref handle) = self.validation_handle {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));

            match handle.result_rx.try_recv() {
                Ok(Ok(result)) => {
                    // Auto-populate config from validation result
                    if let Some(ref path) = self.selected_file {
                        let alphabet_name =
                            result.detected_alphabet.as_ref().map(|a| format!("{}", a));
                        self.recent_files.add(
                            path.clone(),
                            Some(result.sequence_count),
                            alphabet_name,
                        );
                    }
                    if let Some(ref alphabet) = result.detected_alphabet {
                        match alphabet {
                            dima_lib::AlphabetType::Protein => {
                                self.analysis_config.alphabet = AlphabetChoice::Protein;
                                self.analysis_config.kmer_length = 9;
                            }
                            dima_lib::AlphabetType::Nucleotide => {
                                self.analysis_config.alphabet = AlphabetChoice::Nucleotide;
                                self.analysis_config.kmer_length = 27;
                            }
                        }
                    }
                    if let Some(ref fmt) = result.header_format {
                        let fields: Vec<String> = fmt
                            .format_string
                            .split(fmt.delimiter)
                            .map(|s| s.to_string())
                            .collect();
                        self.analysis_config.header_format = Some(fields);
                    }
                    self.validation_result = Some(result);
                    self.validation_handle = None;
                }
                Ok(Err(e)) => {
                    self.error_state
                        .push(ErrorMessage::error(format!("Validation failed: {}", e)));
                    self.validation_result = None;
                    self.selected_file = None;
                    self.validation_handle = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still running -- spinner continues
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.error_state.push(ErrorMessage::error(
                        "Validation worker crashed unexpectedly.".to_string(),
                    ));
                    self.validation_handle = None;
                }
            }
        }

        // Auto-dismiss expired success/info messages
        self.error_state.tick_auto_dismiss();
    }

    /// UI phase: all rendering. Receives &mut Ui (not &Context).
    ///
    /// eframe 0.34's `ui()` provides a raw `Ui` with NO margin or background
    /// color (see eframe::App docs). We MUST wrap content in a `CentralPanel`
    /// so that `panel_fill` from our theme tokens paints the background.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Error banners at the top
            self.show_error_banners(ui);

            // Main content
            match self.current_view {
                View::Setup => self.show_setup(ui),
                View::Workspace => self.show_workspace(ui),
            }
        });
    }

    /// Controls the wgpu surface clear color (painted behind all egui panels).
    /// Returns our theme's primary surface color so the background matches
    /// even in regions not covered by CentralPanel (e.g. during resize).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.tokens.surface_primary.to_normalized_gamma_f32()
    }
}

// ─── UI Rendering Methods ───────────────────────────────────────────────────

impl DimaApp {
    /// Renders egui's built-in light/dark/system theme toggle.
    /// Uses hand-drawn sun/moon icons that do not depend on font glyph support,
    /// avoiding the broken-square issue with U+263E on some platforms.
    /// Token sync is handled in `logic()` by comparing `ctx.theme()` each frame.
    fn show_theme_toggle(&mut self, ui: &mut egui::Ui) {
        egui::widgets::global_theme_preference_switch(ui);
    }

    fn show_error_banners(&mut self, ui: &mut egui::Ui) {
        if !self.error_state.has_messages() {
            return;
        }

        let mut to_dismiss = Vec::new();
        for (i, msg) in self.error_state.messages.iter().enumerate() {
            if msg.dismissed || msg.is_expired() {
                continue;
            }
            let color = match msg.severity {
                crate::error::ErrorSeverity::Error => self.tokens.error_color,
                crate::error::ErrorSeverity::Warning => self.tokens.warning_color,
                crate::error::ErrorSeverity::Success => self.tokens.success_color,
                crate::error::ErrorSeverity::Info => self.tokens.info_color,
            };
            ui.horizontal(|ui| {
                ui.colored_label(color, &msg.text);
                if !msg.should_auto_dismiss() && ui.small_button("\u{2715}").clicked() {
                    to_dismiss.push(i);
                }
            });
        }
        for i in to_dismiss {
            self.error_state.dismiss(i);
        }

        ui.separator();
    }

    /// Setup view layout wrapper: centers content in a scrollable container.
    /// Content rendering is delegated to `show_setup_content` (SRP).
    fn show_setup(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("setup_scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                let available_width = ui.available_width();
                let form_width = available_width.min(520.0);
                let margin = (available_width - form_width) / 2.0;

                ui.add_space(24.0);

                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    ui.vertical(|ui| {
                        ui.set_min_width(form_width);
                        ui.set_max_width(form_width);
                        self.show_setup_content(ui);
                    });
                });
            });
    }

    /// Setup view form content: title, file selection, configuration,
    /// analyze button, and recent files.
    fn show_setup_content(&mut self, ui: &mut egui::Ui) {
        // Theme toggle at top-right of the setup form
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            self.show_theme_toggle(ui);
        });

        // ── Prominent title ──
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("DiMA")
                    .size(self.tokens.font_size_title * 1.8)
                    .strong(),
            );
            ui.label(
                egui::RichText::new("Diversity Motif Analyser")
                    .size(self.tokens.font_size_body)
                    .color(self.tokens.text_secondary),
            );
        });
        ui.add_space(16.0);

        // ── File selection ──
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Input File");
            ui.horizontal(|ui| {
                if ui.button("Browse Alignment...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "FASTA files",
                            &[
                                "fasta", "fa", "fna", "ffn", "faa", "frn",
                                // Compressed formats supported by dima_lib (via needletail)
                                "gz", "bz2", "xz", "zst",
                            ],
                        )
                        .add_filter("DiMA binary", &["dima"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                    {
                        self.handle_file_selected(path);
                    }
                }
                if let Some(ref path) = self.selected_file {
                    ui.label(path.display().to_string());
                }
            });

            // Visual drop zone hint when no file is selected yet
            if self.selected_file.is_none() && self.validation_result.is_none() {
                let (drop_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 60.0),
                    egui::Sense::hover(),
                );
                // egui 0.34 requires StrokeKind as 4th parameter.
                // Inside = stroke inset from rect edge, keeping the rect visually crisp.
                let drop_painter = ui.painter_at(drop_rect);
                drop_painter.rect_stroke(
                    drop_rect,
                    4.0,
                    egui::Stroke::new(1.5_f32, self.tokens.border),
                    egui::StrokeKind::Inside,
                );
                drop_painter.text(
                    drop_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop a FASTA (.fa, .gz, .bz2, .xz, .zst) or .dima file here",
                    egui::FontId::proportional(self.tokens.font_size_body),
                    self.tokens.text_muted,
                );
            }

            // Drag-and-drop support (skip if same file already selected)
            let dropped_files = ui.ctx().input(|i| i.raw.dropped_files.clone());
            if let Some(file) = dropped_files.first() {
                if let Some(ref path) = file.path {
                    let already_selected = self.selected_file.as_deref() == Some(path.as_path());
                    if !already_selected {
                        self.handle_file_selected(path.clone());
                    }
                }
            }

            // Validation progress spinner (shown while background thread is running)
            if self.validation_handle.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Validating file...");
                });
            }

            // Validation result display
            if let Some(ref vr) = self.validation_result {
                if vr.is_valid() {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            self.tokens.success_color,
                            format!("{} sequences detected", vr.sequence_count),
                        );
                        if let Some(ref alphabet) = vr.detected_alphabet {
                            ui.label(format!("({})", alphabet));
                        }
                    });
                } else {
                    for err in &vr.errors {
                        ui.colored_label(self.tokens.error_color, err.to_string());
                    }
                }
            }
        });

        ui.add_space(8.0);

        // ── Configuration ──
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Configuration");

            ui.horizontal(|ui| {
                ui.label("Alphabet:");
                egui::ComboBox::from_id_salt("alphabet")
                    .selected_text(match self.analysis_config.alphabet {
                        AlphabetChoice::Auto => "Auto-detect",
                        AlphabetChoice::Protein => "Protein",
                        AlphabetChoice::Nucleotide => "Nucleotide",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.analysis_config.alphabet,
                            AlphabetChoice::Auto,
                            "Auto-detect",
                        );
                        ui.selectable_value(
                            &mut self.analysis_config.alphabet,
                            AlphabetChoice::Protein,
                            "Protein",
                        );
                        ui.selectable_value(
                            &mut self.analysis_config.alphabet,
                            AlphabetChoice::Nucleotide,
                            "Nucleotide",
                        );
                    });
            });

            ui.horizontal(|ui| {
                ui.label("K-mer length:");
                // Dynamically cap k-mer max based on alphabet to prevent silent
                // integer overflow in encode_kmer_validated (checked_mul returns
                // None, dropping k-mers from analysis — wrong results, not a crash)
                let max_k = match self.analysis_config.alphabet {
                    AlphabetChoice::Protein => dima_lib::max_kmer_length(true),
                    AlphabetChoice::Nucleotide => dima_lib::max_kmer_length(false),
                    AlphabetChoice::Auto => dima_lib::max_kmer_length(true),
                };
                ui.add(
                    egui::DragValue::new(&mut self.analysis_config.kmer_length).range(1..=max_k),
                );
                self.analysis_config.kmer_length = self.analysis_config.kmer_length.min(max_k);
            });

            ui.horizontal(|ui| {
                ui.label("Support threshold:");
                ui.add(
                    egui::DragValue::new(&mut self.analysis_config.support_threshold)
                        .range(1..=10000),
                );
            });

            // Advanced settings (progressive disclosure)
            egui::CollapsingHeader::new("Advanced settings")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Query name:");
                        ui.text_edit_singleline(&mut self.analysis_config.query_name);
                    });
                    ui.checkbox(
                        &mut self.analysis_config.allow_lowercase,
                        "Allow lowercase characters",
                    );
                    ui.horizontal(|ui| {
                        ui.label("HCS threshold (%):");
                        ui.add(
                            egui::DragValue::new(&mut self.workspace_config.hcs_threshold)
                                .range(0.0..=100.0)
                                .speed(0.5),
                        );
                    });

                    // Header format configuration (contextual -- only shown when
                    // auto-detection found a delimited header format)
                    if self.analysis_config.header_format.is_some() {
                        ui.separator();
                        ui.strong("Header Format");

                        // Show detected delimiter and sample header preview
                        if let Some(ref vr) = self.validation_result {
                            if let Some(ref fmt) = vr.header_format {
                                ui.label(format!(
                                    "Delimiter: '{}'  |  {} fields detected",
                                    if fmt.delimiter == '\t' {
                                        "tab".to_string()
                                    } else {
                                        fmt.delimiter.to_string()
                                    },
                                    fmt.field_count
                                ));
                                if let Some(first_header) = vr.sample_headers.first() {
                                    ui.label(
                                        egui::RichText::new(format!("Sample: {}", first_header))
                                            .small()
                                            .color(self.tokens.text_muted),
                                    );
                                }
                            }
                        }

                        // Editable field names
                        if let Some(ref mut fields) = self.analysis_config.header_format {
                            ui.label("Field names:");
                            let mut changed = false;
                            for (i, field) in fields.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("  {}.", i + 1));
                                    if ui.text_edit_singleline(field).changed() {
                                        changed = true;
                                    }
                                });
                            }
                            if changed {
                                // When a field name is renamed, remove any metadata_fields
                                // entries that no longer match any field name. This prevents
                                // orphaned selections that would be silently ignored.
                                self.analysis_config
                                    .metadata_fields
                                    .retain(|f| fields.contains(f));
                            }
                        }

                        // Metadata field selection (checkboxes).
                        // Empty metadata_fields = "all fields" per CLI semantics.
                        // The guard at the bottom prevents the Vec from ever becoming
                        // empty after explicit user toggling, avoiding the paradox where
                        // unchecking all would silently re-check all on the next frame.
                        if let Some(ref fields) = self.analysis_config.header_format.clone() {
                            if fields.len() > 1 {
                                ui.add_space(4.0);
                                ui.label("Include in metadata aggregation (select at least 1):");
                                for field in fields {
                                    let mut included =
                                        self.analysis_config.metadata_fields.is_empty()
                                            || self.analysis_config.metadata_fields.contains(field);
                                    if ui.checkbox(&mut included, field).changed() {
                                        if included {
                                            if !self.analysis_config.metadata_fields.contains(field)
                                            {
                                                if self.analysis_config.metadata_fields.is_empty() {
                                                    // First explicit check from "all" state:
                                                    // populate with all fields, then the toggle
                                                    // logic below handles it correctly.
                                                    self.analysis_config.metadata_fields = fields
                                                        .iter()
                                                        .filter(|f| *f != field)
                                                        .cloned()
                                                        .collect();
                                                    self.analysis_config
                                                        .metadata_fields
                                                        .push(field.clone());
                                                } else {
                                                    self.analysis_config
                                                        .metadata_fields
                                                        .push(field.clone());
                                                }
                                            }
                                        } else {
                                            if self.analysis_config.metadata_fields.is_empty() {
                                                // First uncheck from "all" state:
                                                // populate with all-except-this
                                                self.analysis_config.metadata_fields = fields
                                                    .iter()
                                                    .filter(|f| *f != field)
                                                    .cloned()
                                                    .collect();
                                            } else {
                                                self.analysis_config
                                                    .metadata_fields
                                                    .retain(|f| f != field);
                                            }
                                            // Guard: prevent metadata_fields from becoming empty.
                                            // Empty = "all fields" per CLI semantics, which would
                                            // re-check all boxes on the next frame.
                                            if self.analysis_config.metadata_fields.is_empty() {
                                                self.analysis_config
                                                    .metadata_fields
                                                    .push(field.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Fill-NA value
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Fill empty fields with:");
                            let fillna = self
                                .analysis_config
                                .header_fillna
                                .get_or_insert_with(|| "Unknown".to_string());
                            ui.text_edit_singleline(fillna);
                        });
                    }
                });
        });

        ui.add_space(16.0);

        // ── Analyze button ──
        let can_analyze = self.selected_file.is_some()
            && self
                .validation_result
                .as_ref()
                .is_some_and(|vr| vr.is_valid())
            && self.analysis_handle.is_none()
            && self.validation_handle.is_none();

        if self.analysis_handle.is_some() {
            // Show 3-phase progress: Reading -> Computing -> Finalizing
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                if let Some(ref handle) = self.analysis_handle {
                    let completed = handle.progress_counter.load(Ordering::Relaxed);
                    if handle.total_positions > 0 {
                        let fraction = completed as f32 / handle.total_positions as f32;
                        if completed == 0 {
                            ui.spinner();
                            ui.label("Reading sequences...");
                        } else if fraction >= 1.0 {
                            ui.spinner();
                            ui.label("Finalizing...");
                        } else {
                            let bar = egui::ProgressBar::new(fraction.min(1.0)).text(format!(
                                "Computing entropy: {}/{}",
                                completed, handle.total_positions
                            ));
                            ui.add(bar);
                        }
                    } else {
                        ui.spinner();
                        ui.label("Reading sequences...");
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.cancel_analysis();
                }
            });
        } else {
            let button_size = [ui.available_width(), 40.0];
            ui.add_enabled_ui(can_analyze, |ui| {
                if ui
                    .add_sized(button_size, egui::Button::new("\u{25B6} Analyze"))
                    .clicked()
                {
                    if let Some(ref path) = self.selected_file {
                        self.start_analysis(path.clone());
                    }
                }
            });
        }

        ui.add_space(16.0);

        // ── Recent files ──
        self.show_recent_files(ui);

        // Symmetric bottom padding to balance the 24px top padding in show_setup
        ui.add_space(24.0);
    }

    /// Show the recent files panel with clickable entries and relative timestamps.
    fn show_recent_files(&mut self, ui: &mut egui::Ui) {
        if self.recent_files.files.is_empty() {
            return;
        }

        // Current time for computing relative timestamps
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Recent Files");
            let mut selected_path: Option<PathBuf> = None;

            for recent in &self.recent_files.files {
                let file_name = recent
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();

                let relative_time = format_relative_time(now_secs, recent.last_opened_unix_secs);
                let label_text = if let Some(count) = recent.sequence_count {
                    format!("{}  ({} seq)   {}", file_name, count, relative_time)
                } else {
                    format!("{}   {}", file_name, relative_time)
                };

                let label = egui::RichText::new(&label_text).color(self.tokens.accent);
                if ui
                    .add(egui::Label::new(label).sense(egui::Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(recent.path.display().to_string())
                    .clicked()
                {
                    if recent.path.exists() {
                        selected_path = Some(recent.path.clone());
                    } else {
                        self.error_state.push(ErrorMessage::warning(format!(
                            "File no longer exists: {}",
                            recent.path.display()
                        )));
                    }
                }
            }

            // Process clicks outside the borrow of recent_files
            if let Some(path) = selected_path {
                self.handle_file_selected(path);
            }
        });
    }

    fn handle_file_selected(&mut self, path: PathBuf) {
        // IMPORTANT: Cancel any in-progress validation FIRST, before routing.
        // If this were placed after the .dima check, selecting a .dima file while
        // validation was running would skip cancellation. The stale worker would
        // later complete and overwrite config via logic() polling.
        if let Some(ref handle) = self.validation_handle {
            handle.cancel_token.store(true, Ordering::Relaxed);
        }
        self.validation_handle = None;
        self.validation_result = None;

        // Route by extension: .dima -> binary import, else -> FASTA validation
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if extension == "dima" {
            self.load_dima_binary(&path);
            return;
        }

        self.selected_file = Some(path.clone());

        // Always update query name from file stem (prevents stale name
        // from a previously selected file persisting)
        self.analysis_config.query_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Clear ALL header-related config from any previously selected file.
        // header_fillna and metadata_fields are derived from header detection;
        // keeping stale values would silently apply File A's settings to File B.
        self.analysis_config.header_format = None;
        self.analysis_config.header_fillna = None;
        self.analysis_config.metadata_fields.clear();

        // Spawn background validation thread.
        // Compressed files (gzip, bzip2, xz, zstd) are transparently decompressed
        // by validate_fasta's open_maybe_compressed() layer. For large compressed
        // files (multi-GB on disk), decompression + scanning can take seconds to
        // minutes, so the background thread keeps the UI responsive.
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let ctx_clone = self.egui_ctx.clone();

        std::thread::spawn(move || {
            let result = dima_lib::validate_fasta(&path, Some(&cancel_clone));
            let _ = tx.send(result);
            // Wake the GUI to process the result immediately on completion
            if let Some(ctx) = ctx_clone {
                ctx.request_repaint();
            }
        });

        self.validation_handle = Some(ValidationHandle {
            result_rx: rx,
            cancel_token: cancel,
        });
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        // Clone results Arc early for shared ownership without holding &self
        let results = match self.results.clone() {
            Some(r) => r,
            None => return,
        };

        // ── Keyboard navigation on filtered positions ──
        // Arrow keys navigate between filtered positions, Home/End jump to extremes.
        // This runs OUTSIDE the ScrollArea so it reads global keyboard events
        // and updates selected_position BEFORE any UI rendering.
        self.handle_keyboard_navigation(ui, &results);

        // Wrap all workspace content in a vertical ScrollArea so the entire
        // dashboard scrolls when content exceeds window height. Without this,
        // sections like Position Explorer get clipped at the bottom of the window.
        egui::ScrollArea::vertical()
            .id_salt("workspace_scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                // Top bar with navigation + export
                ui.horizontal(|ui| {
                    ui.heading(format!("DiMA \u{2014} {}", results.query_name));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("\u{25C0} Setup").clicked() {
                            self.current_view = View::Setup;
                        }
                        self.show_export_buttons(ui, &results);
                        self.show_theme_toggle(ui);
                    });
                });

                ui.separator();

                // Summary bar
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} sequences", results.sequence_count));
                    ui.separator();
                    ui.label(format!("{} positions", results.results.len()));
                    ui.separator();
                    ui.label(format!("Avg H = {:.4}", results.average_entropy));
                    ui.separator();
                    ui.label(format!("k = {}", results.kmer_length));
                    ui.separator();
                    ui.label(format!("s \u{2265} {}", results.support_threshold));
                    ui.separator();
                    ui.label(format!(
                        "Showing {}/{}",
                        self.filtered_positions.len(),
                        results.results.len()
                    ));
                });

                ui.add_space(8.0);

                // Filters (left) + Position Explorer (right) side-by-side,
                // above the entropy chart per user layout preference.
                // Arc::clone is a cheap ref-count bump.
                let results_for_layout = results.clone();
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(250.0, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            self.show_filter_controls(ui, &results_for_layout);
                            ui.add_space(8.0);
                            self.show_filter_summary(ui, &results_for_layout);
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            self.show_position_explorer(ui, &results_for_layout);
                        },
                    );
                });

                ui.add_space(8.0);

                // Entropy chart with LTTB downsampling, click-to-select, zoom/pan
                self.show_entropy_chart(ui, &results);

                ui.add_space(8.0);

                // HCS map with live threshold slider
                self.show_hcs_section(ui, &results);

                ui.add_space(8.0);

                // Detail panels: position details (left) + variant chart (right).
                // Each panel renders at its natural content height — no height
                // synchronization. The previous set_min_height-based sync had a
                // feedback loop (response.rect includes inflated height, so the max
                // could only grow, never shrink — egui#1813, egui#3155). Removing it
                // lets panels shrink when content is collapsed, eliminating persistent
                // empty space. horizontal_top aligns both at the top edge, which is
                // the important alignment axis for details-on-demand layout.
                let results_for_panels = results.clone();
                let panel_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;

                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_width, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.group(|ui| {
                                self.show_position_details(ui, &results_for_panels);
                            });
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_width, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.group(|ui| {
                                self.show_variant_panel(ui, &results_for_panels);
                            });
                        },
                    );
                });
            });
    }

    /// Handle keyboard arrow/Home/End navigation on filtered positions.
    /// Navigates between FILTERED positions but selects from UNFILTERED data.
    fn handle_keyboard_navigation(&mut self, ui: &egui::Ui, results: &Results) {
        if self.filtered_positions.is_empty() {
            return;
        }

        let nav_action = ui.ctx().input(|i| {
            if i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowDown) {
                Some(NavAction::Next)
            } else if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowUp) {
                Some(NavAction::Previous)
            } else if i.key_pressed(egui::Key::Home) {
                Some(NavAction::First)
            } else if i.key_pressed(egui::Key::End) {
                Some(NavAction::Last)
            } else {
                None
            }
        });

        let Some(action) = nav_action else {
            return;
        };

        // Find current position's index in the filtered list
        let current_filtered_idx = self.selected_position.and_then(|sel_pos| {
            self.filtered_positions
                .iter()
                .position(|&idx| results.results[idx].position == sel_pos)
        });

        let new_filtered_idx = match action {
            NavAction::Next => match current_filtered_idx {
                Some(i) if i + 1 < self.filtered_positions.len() => Some(i + 1),
                None => Some(0),
                _ => current_filtered_idx,
            },
            NavAction::Previous => match current_filtered_idx {
                Some(i) if i > 0 => Some(i - 1),
                None => Some(self.filtered_positions.len() - 1),
                _ => current_filtered_idx,
            },
            NavAction::First => Some(0),
            NavAction::Last => Some(self.filtered_positions.len() - 1),
        };

        if let Some(fi) = new_filtered_idx {
            let unfiltered_idx = self.filtered_positions[fi];
            let new_pos = results.results[unfiltered_idx].position;
            if self.selected_position != Some(new_pos) {
                self.selected_position = Some(new_pos);
                self.variant_table_sort = None;
                self.cached_metadata = None;
                self.expanded_metadata_fields.clear();
                self.selected_variant_index = self.find_default_variant(results, new_pos);
            }
        }
    }

    /// Entropy chart with LTTB downsampling, click-to-select, zoom/pan, and proper axes.
    ///
    /// Axes follow bioinformatics convention (IGV, WebLogo, Bio3D): tick marks only,
    /// no grid lines, per Tufte's data-ink ratio principle and Cleveland's graphical
    /// perception guidelines (Cleveland 1985, "The Elements of Graphing Data").
    fn show_entropy_chart(&mut self, ui: &mut egui::Ui, results: &Results) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                Self::section_header(ui, "Entropy Chart");
                if self.entropy_viewport.is_some() && ui.small_button("Reset Zoom").clicked() {
                    self.entropy_viewport = None;
                }
            });

            let available = ui.available_size();
            let chart_height = (ui.ctx().content_rect().height() * 0.25).clamp(150.0, 400.0);

            // Layout margins for axis labels and titles
            let y_axis_margin = 60.0_f32; // space for Y-axis ticks + rotated title
            let x_axis_margin = 32.0_f32; // space for X-axis ticks + title
            let top_margin = 8.0_f32;

            let (full_rect, response) = ui.allocate_exact_size(
                egui::vec2(available.x, chart_height + x_axis_margin + top_margin),
                egui::Sense::click_and_drag(),
            );

            // Data drawing area: inset from full_rect by axis margins
            let rect = egui::Rect::from_min_max(
                egui::pos2(
                    full_rect.left() + y_axis_margin,
                    full_rect.top() + top_margin,
                ),
                egui::pos2(full_rect.right(), full_rect.max.y - x_axis_margin),
            );

            if self.filtered_positions.is_empty() {
                return;
            }

            let full_data: Vec<(f32, f32)> = self
                .filtered_positions
                .iter()
                .map(|&idx| {
                    let pos = &results.results[idx];
                    (pos.position as f32, pos.entropy as f32)
                })
                .collect();

            let Some(max_e) = full_data.iter().map(|(_, e)| *e).reduce(f32::max) else {
                return;
            };
            if max_e <= 0.0 {
                return;
            }

            let data_min_x = full_data.first().map(|(x, _)| *x).unwrap_or(0.0);
            let data_max_x = full_data.last().map(|(x, _)| *x).unwrap_or(1.0);

            let (view_min_x, view_max_x) = if let Some((vmin, vmax)) = self.entropy_viewport {
                (vmin as f32, vmax as f32)
            } else {
                (data_min_x, data_max_x)
            };
            let view_range = (view_max_x - view_min_x).max(1.0);

            let margin_frac = view_range * 0.05;
            let visible_data: Vec<(f32, f32)> = full_data
                .iter()
                .filter(|(x, _)| *x >= view_min_x - margin_frac && *x <= view_max_x + margin_frac)
                .copied()
                .collect();

            let render_data: Vec<(f32, f32)> = if visible_data.len() > ENTROPY_CHART_MAX_POINTS {
                let points: Vec<Point> = visible_data
                    .iter()
                    .map(|&(x, y)| Point {
                        x: x as f64,
                        y: y as f64,
                    })
                    .collect();
                lttb_downsample_by_range(&points, ENTROPY_CHART_MAX_POINTS)
                    .into_iter()
                    .map(|p| (p.x as f32, p.y as f32))
                    .collect()
            } else {
                visible_data
            };

            let painter = ui.painter_at(full_rect);
            let tick_len = 4.0_f32;
            let axis_stroke = egui::Stroke::new(1.0_f32, self.tokens.chart_axis);
            let tick_font = egui::FontId::proportional(self.tokens.font_size_caption);

            // ── Y-axis (Entropy) ──
            painter.line_segment(
                [
                    egui::pos2(rect.left(), rect.top()),
                    egui::pos2(rect.left(), rect.bottom()),
                ],
                axis_stroke,
            );

            let y_ticks = nice_ticks(0.0, max_e as f64, 6);
            for &tick_val in &y_ticks {
                let ny = tick_val as f32 / max_e;
                if !(0.0..=1.0).contains(&ny) {
                    continue;
                }
                let y = rect.bottom() - ny * rect.height();
                // Tick mark
                painter.line_segment(
                    [
                        egui::pos2(rect.left() - tick_len, y),
                        egui::pos2(rect.left(), y),
                    ],
                    axis_stroke,
                );
                // Label
                painter.text(
                    egui::pos2(rect.left() - tick_len - 2.0, y),
                    egui::Align2::RIGHT_CENTER,
                    format!("{:.2}", tick_val),
                    tick_font.clone(),
                    self.tokens.chart_axis,
                );
            }

            // Y-axis rotated title using TextShape
            let y_title_pos = egui::pos2(full_rect.left() + 12.0, rect.center().y);
            let y_title_galley = painter.layout_no_wrap(
                "Entropy".to_string(),
                tick_font.clone(),
                self.tokens.chart_axis,
            );
            let y_title_shape =
                egui::epaint::TextShape::new(y_title_pos, y_title_galley, self.tokens.chart_axis)
                    .with_angle(-std::f32::consts::FRAC_PI_2)
                    .with_override_text_color(self.tokens.chart_axis);
            painter.add(y_title_shape);

            // ── X-axis (Position) ──
            painter.line_segment(
                [
                    egui::pos2(rect.left(), rect.bottom()),
                    egui::pos2(rect.right(), rect.bottom()),
                ],
                axis_stroke,
            );

            let x_ticks = nice_ticks(view_min_x as f64, view_max_x as f64, 8);
            for &tick_val in &x_ticks {
                let nx = (tick_val as f32 - view_min_x) / view_range;
                if !(0.0..=1.0).contains(&nx) {
                    continue;
                }
                let x = rect.left() + nx * rect.width();
                painter.line_segment(
                    [
                        egui::pos2(x, rect.bottom()),
                        egui::pos2(x, rect.bottom() + tick_len),
                    ],
                    axis_stroke,
                );
                painter.text(
                    egui::pos2(x, rect.bottom() + tick_len + 2.0),
                    egui::Align2::CENTER_TOP,
                    format!("{}", tick_val as usize),
                    tick_font.clone(),
                    self.tokens.chart_axis,
                );
            }

            // X-axis centered title
            painter.text(
                egui::pos2(rect.center().x, full_rect.bottom() - 4.0),
                egui::Align2::CENTER_BOTTOM,
                "Position",
                tick_font.clone(),
                self.tokens.chart_axis,
            );

            // Average entropy horizontal line (GLOBAL, unfiltered)
            let avg_ny = results.average_entropy as f32 / max_e;
            if (0.0..=1.0).contains(&avg_ny) {
                let avg_y = rect.bottom() - avg_ny * rect.height();
                painter.line_segment(
                    [
                        egui::pos2(rect.left(), avg_y),
                        egui::pos2(rect.right(), avg_y),
                    ],
                    egui::Stroke::new(1.0_f32, self.tokens.chart_avg_line),
                );
                painter.text(
                    egui::pos2(rect.right() - 4.0, avg_y - 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("avg {:.3}", results.average_entropy),
                    tick_font.clone(),
                    self.tokens.chart_avg_line,
                );
            }

            // Draw entropy line
            if self.gpu_available {
                let vertices: Vec<EntropyVertex> = render_data
                    .iter()
                    .map(|&(x, y)| EntropyVertex { x, y })
                    .collect();

                let new_vertices = if self.charts_need_data_upload {
                    Some(vertices)
                } else {
                    None
                };

                let callback = EntropyLineCallback {
                    view_transform: ViewTransform {
                        x_min: view_min_x,
                        x_range: view_range,
                        y_max: max_e,
                        _padding: 0.0,
                    },
                    new_data_version: self.data_version,
                    new_vertices,
                };

                ui.painter()
                    .add(egui_wgpu::Callback::new_paint_callback(rect, callback));
                self.charts_need_data_upload = false;
            } else {
                let screen_points: Vec<egui::Pos2> = render_data
                    .iter()
                    .map(|(x, e)| {
                        let nx = (*x - view_min_x) / view_range;
                        let ny = *e / max_e;
                        egui::pos2(
                            rect.left() + nx * rect.width(),
                            rect.bottom() - ny * rect.height(),
                        )
                    })
                    .collect();
                for window in screen_points.windows(2) {
                    painter.line_segment(
                        [window[0], window[1]],
                        egui::Stroke::new(1.5_f32, self.tokens.accent),
                    );
                }
            }

            // Highlight selected position — uses full-opacity chart_selection_line
            if let Some(sel_pos) = self.selected_position {
                let sel_nx = (sel_pos as f32 - view_min_x) / view_range;
                if (0.0..=1.0).contains(&sel_nx) {
                    let screen_x = rect.left() + sel_nx * rect.width();
                    painter.line_segment(
                        [
                            egui::pos2(screen_x, rect.top()),
                            egui::pos2(screen_x, rect.bottom()),
                        ],
                        egui::Stroke::new(2.0_f32, self.tokens.chart_selection_line),
                    );
                }
            }

            // Click-to-select
            if response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let click_nx = (pointer_pos.x - rect.left()) / rect.width();
                    let click_x = view_min_x + click_nx * view_range;
                    let nearest = full_data
                        .iter()
                        .min_by(|(ax, _), (bx, _)| {
                            let da = (ax - click_x).abs();
                            let db = (bx - click_x).abs();
                            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(x, _)| *x as usize);
                    if nearest != self.selected_position {
                        self.selected_position = nearest;
                        self.variant_table_sort = None;
                        self.cached_metadata = None;
                        self.expanded_metadata_fields.clear();
                        self.selected_variant_index =
                            nearest.and_then(|pos| self.find_default_variant(results, pos));
                    }
                }
            }

            // Ctrl+Scroll to zoom (Cmd+Scroll on macOS)
            let (scroll_delta, ctrl_held) = ui
                .ctx()
                .input(|i| (i.smooth_scroll_delta.y, i.modifiers.command));
            if response.hovered() && ctrl_held && scroll_delta.abs() > 0.1 {
                ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);

                let zoom_factor = if scroll_delta > 0.0 { 0.85 } else { 1.18 };
                let mouse_nx = ui
                    .ctx()
                    .pointer_latest_pos()
                    .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
                    .unwrap_or(0.5);
                let mouse_x = view_min_x + mouse_nx * view_range;
                let new_range = view_range * zoom_factor;
                let clamped_range = new_range.min(data_max_x - data_min_x).max(5.0);
                let new_min = mouse_x - mouse_nx * clamped_range;
                let new_max = new_min + clamped_range;
                let (new_min, new_max) = if new_min < data_min_x {
                    (data_min_x, data_min_x + clamped_range)
                } else if new_max > data_max_x {
                    (data_max_x - clamped_range, data_max_x)
                } else {
                    (new_min, new_max)
                };
                if (new_max - new_min) >= (data_max_x - data_min_x) * 0.99 {
                    self.entropy_viewport = None;
                } else {
                    self.entropy_viewport = Some((new_min as f64, new_max as f64));
                }
            }

            // Zoom hint
            if response.hovered() && !ctrl_held && self.entropy_viewport.is_none() {
                let hint = if cfg!(target_os = "macos") {
                    "\u{2318}+Scroll to zoom"
                } else {
                    "Ctrl+Scroll to zoom"
                };
                painter.text(
                    egui::pos2(rect.right() - 8.0, rect.bottom() - 4.0),
                    egui::Align2::RIGHT_BOTTOM,
                    hint,
                    tick_font,
                    self.tokens.text_muted,
                );
            }

            // Drag to pan
            if response.dragged() && self.entropy_viewport.is_some() {
                let drag_delta_x = response.drag_delta().x;
                let pan_amount = -(drag_delta_x / rect.width()) * view_range;
                let new_min = (view_min_x + pan_amount).max(data_min_x);
                let new_max = (new_min + view_range).min(data_max_x);
                let new_min = new_max - view_range;
                self.entropy_viewport = Some((new_min as f64, new_max as f64));
            }

            // Tooltip on hover
            response.on_hover_ui_at_pointer(|ui| {
                if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                    let hover_nx = (pointer_pos.x - rect.left()) / rect.width();
                    let hover_x = view_min_x + hover_nx * view_range;
                    if let Some(&(px, py)) = full_data.iter().min_by(|(ax, _), (bx, _)| {
                        let da = (ax - hover_x).abs();
                        let db = (bx - hover_x).abs();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                        ui.label(format!("Position: {}", px as usize));
                        ui.label(format!("Entropy: {:.4}", py));
                    }
                }
            });
        });
    }

    /// HCS map with live threshold slider and off-by-one coordinate fix.
    fn show_hcs_section(&mut self, ui: &mut egui::Ui, results: &Results) {
        // Live HCS threshold slider (Fix 16)
        let mut threshold_changed = false;
        ui.horizontal(|ui| {
            ui.label("HCS threshold (%):");
            threshold_changed = ui
                .add(
                    egui::DragValue::new(&mut self.workspace_config.hcs_threshold)
                        .range(0.0..=100.0)
                        .speed(0.5),
                )
                .changed();
        });

        if threshold_changed {
            self.hcs_regions =
                compute_hcs_regions(results, Some(self.workspace_config.hcs_threshold));
        }

        ui.group(|ui| {
            Self::section_header(
                ui,
                format!("HCS Map ({} conserved regions)", self.hcs_regions.len()),
            );

            if self.hcs_regions.is_empty() {
                // Stable-size empty state: allocate the same 30px bar height
                // so the section doesn't collapse/reappear as threshold changes.
                // Includes threshold value and recovery hint (Northbase pattern).
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 30.0),
                    egui::Sense::hover(),
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "No conserved regions at {:.0}% threshold \u{2014} try lowering the threshold",
                        self.workspace_config.hcs_threshold
                    ),
                    egui::FontId::proportional(12.0),
                    self.tokens.text_muted,
                );
                return;
            }

            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());

            // Coordinate mapping: positions are 1-based, convert to 0-based
            // for proportional rendering on the bar. Use (len + kmer_length - 1)
            // to account for the full alignment length, not just position count.
            let total_span = results.results.len().max(1) as f32;
            let painter = ui.painter_at(rect);

            for region in &self.hcs_regions {
                // Convert 1-based positions to 0-based for proportional mapping
                let left_frac = (region.start_position - 1) as f32 / total_span;
                let right_frac = region.end_position as f32 / total_span;
                let left = rect.left() + left_frac * rect.width();
                let right = rect.left() + right_frac * rect.width();

                let region_rect = egui::Rect::from_min_max(
                    egui::pos2(left, rect.top()),
                    egui::pos2(right, rect.bottom()),
                );
                painter.rect_filled(region_rect, 2.0_f32, self.tokens.hcs_conserved);

                // Per-region hover tooltip (details-on-demand per Shneiderman 1996).
                // ui.interact registers at the context layer, not the layout layer,
                // so it works on sub-regions of already-allocated space.
                let region_response = ui.interact(
                    region_rect,
                    egui::Id::new(("hcs_region", region.start_position)),
                    egui::Sense::hover(),
                );

                if region_response.hovered() {
                    // Compute per-region stats from position data
                    let positions_in_region: Vec<_> = results
                        .results
                        .iter()
                        .filter(|p| region.positions.contains(&p.position))
                        .collect();

                    let avg_entropy = if positions_in_region.is_empty() {
                        0.0
                    } else {
                        positions_in_region.iter().map(|p| p.entropy).sum::<f64>()
                            / positions_in_region.len() as f64
                    };

                    let avg_conservation = if positions_in_region.is_empty() {
                        0.0
                    } else {
                        let total_idx_incidence: f64 = positions_in_region
                            .iter()
                            .filter_map(|p| {
                                p.diversity_motifs.as_ref().and_then(|motifs| {
                                    motifs
                                        .iter()
                                        .find(|v| v.motif_short.as_deref() == Some("I"))
                                        .map(|v| v.incidence)
                                })
                            })
                            .sum();
                        total_idx_incidence / positions_in_region.len() as f64
                    };

                    // Truncate sequences for compact tooltip display
                    let seq_display = if region.sequence.len() > 30 {
                        format!("{}...", &region.sequence[..30])
                    } else {
                        region.sequence.clone()
                    };

                    // Capture warning_color before closure to satisfy Rust 2021
                    // disjoint field capture (Color32 is Copy)
                    let warning_color = self.tokens.warning_color;
                    let low_support_count = region.low_support_positions.len();

                    region_response.on_hover_ui(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Positions {}\u{2013}{} ({} positions)",
                                region.start_position,
                                region.end_position,
                                region.positions.len()
                            ))
                            .strong(),
                        );
                        ui.label(format!(
                            "Sequence: {} ({} residues)",
                            seq_display,
                            region.sequence.len()
                        ));
                        ui.label(format!("Avg entropy: {:.4}", avg_entropy));
                        ui.label(format!("Avg conservation: {:.1}%", avg_conservation));
                        if low_support_count > 0 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "\u{26A0} {} low-support positions",
                                    low_support_count
                                ))
                                .color(warning_color),
                            );
                        }
                    });
                }
            }
        });
    }

    /// Export buttons: JSON, .dima binary, and chart PNG.
    fn show_export_buttons(&mut self, ui: &mut egui::Ui, results: &Results) {
        // Chart PNG export using CPU rasterization
        if ui.button("Export PNG").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .set_file_name(format!("{}_entropy.png", results.query_name))
                .save_file()
            {
                match self.export_entropy_chart_png(&path, results) {
                    Ok(()) => {
                        self.error_state.push(ErrorMessage::success(format!(
                            "Chart exported to {}",
                            path.display()
                        )));
                    }
                    Err(e) => {
                        self.error_state.push(ErrorMessage::error(format!(
                            "Failed to export chart: {}",
                            e
                        )));
                    }
                }
            }
        }

        if ui.button("\u{1F4BE} Export .dima").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DiMA binary", &["dima"])
                .set_file_name(format!("{}.dima", results.query_name))
                .save_file()
            {
                // Guard against non-UTF-8 paths (BinaryFormat::write_to_file takes &str).
                // Do NOT use to_string_lossy() — it silently replaces non-UTF-8 bytes
                // with U+FFFD, corrupting the path and writing to a wrong location.
                match path.to_str() {
                    Some(path_str) => {
                        match dima_lib::BinaryFormat::write_to_file(
                            results,
                            path_str,
                            Some(dima_lib::BinaryFormatConfig::default()),
                        ) {
                            Ok(()) => {
                                self.error_state.push(ErrorMessage::success(format!(
                                    "Saved to {}",
                                    path.display()
                                )));
                            }
                            Err(e) => {
                                self.error_state.push(ErrorMessage::error(format!(
                                    "Failed to save .dima: {}",
                                    e
                                )));
                            }
                        }
                    }
                    None => {
                        self.error_state.push(ErrorMessage::error(
                            "File path contains non-UTF-8 characters and cannot be used."
                                .to_string(),
                        ));
                    }
                }
            }
        }
        if ui.button("\u{1F4C4} Export JSON").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_file_name(format!("{}.json", results.query_name))
                .save_file()
            {
                // Use the shared output function (same as CLI) for consistent
                // JSON format AND atomic writes (temp file + fsync + rename).
                match dima_lib::write_results_to_output(
                    results,
                    Some(&path),
                    dima_lib::OutputType::Json,
                    false,
                ) {
                    Ok(()) => {
                        self.error_state.push(ErrorMessage::success(format!(
                            "Saved to {}",
                            path.display()
                        )));
                    }
                    Err(e) => {
                        self.error_state
                            .push(ErrorMessage::error(format!("Failed to save JSON: {}", e)));
                    }
                }
            }
        }
    }

    /// Render the entropy chart to a PNG file with axes.
    /// Uses CPU rasterization (image crate) to draw the same chart that appears
    /// on screen, including axis lines, tick marks, and labels. This avoids
    /// requiring a GPU context for export and works headlessly in CI/tests.
    fn export_entropy_chart_png(
        &self,
        path: &std::path::Path,
        results: &Results,
    ) -> Result<(), String> {
        let width: u32 = 1920;
        let height: u32 = 600;

        let mut img = image::RgbaImage::new(width, height);

        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([255, 255, 255, 255]);
        }

        if self.filtered_positions.is_empty() {
            return img
                .save(path)
                .map_err(|e| format!("Failed to save PNG: {}", e));
        }

        let data: Vec<(f64, f64)> = self
            .filtered_positions
            .iter()
            .map(|&idx| {
                let pos = &results.results[idx];
                (pos.position as f64, pos.entropy)
            })
            .collect();

        let min_x = data.first().map(|(x, _)| *x).unwrap_or(0.0);
        let max_x = data.last().map(|(x, _)| *x).unwrap_or(1.0);
        let x_range = (max_x - min_x).max(1.0);
        let max_e = data
            .iter()
            .map(|(_, e)| *e)
            .filter(|e| e.is_finite())
            .fold(0.0_f64, f64::max);
        if max_e <= 0.0 {
            return img
                .save(path)
                .map_err(|e| format!("Failed to save PNG: {}", e));
        }

        // Chart area with margins for axis labels
        let left_margin = 80_u32;
        let right_margin = 20_u32;
        let top_margin = 20_u32;
        let bottom_margin = 50_u32;
        let chart_left = left_margin;
        let chart_right = width - right_margin;
        let chart_top = top_margin;
        let chart_bottom = height - bottom_margin;
        let chart_w = chart_right - chart_left;
        let chart_h = chart_bottom - chart_top;

        let axis_color = image::Rgba([107, 114, 128, 255]);
        let tick_len = 5_u32;

        // Draw Y-axis line
        for y in chart_top..=chart_bottom {
            img.put_pixel(chart_left, y, axis_color);
        }

        // Draw X-axis line
        for x in chart_left..=chart_right {
            img.put_pixel(x, chart_bottom, axis_color);
        }

        // Y-axis tick marks
        let y_ticks = nice_ticks(0.0, max_e, 6);
        for &tick_val in &y_ticks {
            let ny = tick_val / max_e;
            if !(0.0..=1.0).contains(&ny) {
                continue;
            }
            let y = chart_bottom - (ny * chart_h as f64) as u32;
            if y >= chart_top && y <= chart_bottom {
                for x in (chart_left - tick_len)..chart_left {
                    img.put_pixel(x, y, axis_color);
                }
            }
        }

        // X-axis tick marks
        let x_ticks = nice_ticks(min_x, max_x, 10);
        for &tick_val in &x_ticks {
            let nx = (tick_val - min_x) / x_range;
            if !(0.0..=1.0).contains(&nx) {
                continue;
            }
            let x = chart_left + (nx * chart_w as f64) as u32;
            if x >= chart_left && x <= chart_right {
                for y in chart_bottom..(chart_bottom + tick_len) {
                    if y < height {
                        img.put_pixel(x, y, axis_color);
                    }
                }
            }
        }

        // Draw average entropy line (red, dashed)
        let avg_ny = results.average_entropy / max_e;
        if (0.0..=1.0).contains(&avg_ny) {
            let avg_y = chart_bottom - (avg_ny * chart_h as f64) as u32;
            if avg_y >= chart_top && avg_y <= chart_bottom {
                for x in chart_left..chart_right {
                    if x % 8 < 5 {
                        img.put_pixel(x, avg_y, image::Rgba([220, 38, 38, 255]));
                    }
                }
            }
        }

        // Draw data lines (blue)
        let line_color = image::Rgba([37, 99, 235, 255]);
        for window in data.windows(2) {
            let (x0, y0) = window[0];
            let (x1, y1) = window[1];

            let sx0 = chart_left as f64 + (x0 - min_x) / x_range * chart_w as f64;
            let sy0 = chart_top as f64 + (1.0 - y0 / max_e) * chart_h as f64;
            let sx1 = chart_left as f64 + (x1 - min_x) / x_range * chart_w as f64;
            let sy1 = chart_top as f64 + (1.0 - y1 / max_e) * chart_h as f64;

            let steps = ((sx1 - sx0).abs().max((sy1 - sy0).abs()) as usize).max(1);
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let px = (sx0 + t * (sx1 - sx0)) as u32;
                let py = (sy0 + t * (sy1 - sy0)) as u32;
                if px < width && py < height {
                    img.put_pixel(px, py, line_color);
                    if py + 1 < height {
                        img.put_pixel(px, py + 1, line_color);
                    }
                }
            }
        }

        img.save(path)
            .map_err(|e| format!("Failed to save PNG: {}", e))
    }

    fn show_position_details(&mut self, ui: &mut egui::Ui, results: &Results) {
        Self::section_header(ui, "Position Details");
        if let Some(pos_num) = self.selected_position {
            if let Some(&idx) = self.position_index_map.get(&pos_num) {
                let pos = &results.results[idx];
                ui.label(format!("Position: {}", pos.position));
                ui.label(format!("Entropy: {:.4}", pos.entropy));
                ui.label(format!("Support: {}", pos.support));
                if let Some(ref ls) = pos.low_support {
                    ui.colored_label(self.tokens.warning_color, format!("Low support: {}", ls));
                }
                ui.separator();

                if let Some(ref variants) = pos.diversity_motifs {
                    ui.label(format!("Variants ({}):", variants.len()));

                    // Build a sorted index for virtual scrolling. Sorting the index
                    // instead of cloning the Vec avoids allocating ~7K Variant structs.
                    let mut sorted_indices: Vec<usize> = (0..variants.len()).collect();
                    if let Some(ref sort) = self.variant_table_sort {
                        sorted_indices.sort_by(|&a, &b| {
                            let cmp = match sort.column {
                                0 => variants[a].count.cmp(&variants[b].count),
                                1 => variants[a]
                                    .incidence
                                    .partial_cmp(&variants[b].incidence)
                                    .unwrap_or(std::cmp::Ordering::Equal),
                                2 => variants[a].motif_short.cmp(&variants[b].motif_short),
                                3 => variants[a].sequence.cmp(&variants[b].sequence),
                                _ => std::cmp::Ordering::Equal,
                            };
                            match sort.direction {
                                SortDirection::Ascending => cmp,
                                SortDirection::Descending => cmp.reverse(),
                            }
                        });
                    }

                    let row_height = 22.0;
                    let current_sort = self.variant_table_sort;

                    // Cell accumulators for header and body clicks inside table closures
                    let clicked_col = std::cell::Cell::new(None::<usize>);
                    let clicked_variant = std::cell::Cell::new(None::<usize>);

                    // Compute scroll height before TableBuilder borrows ui mutably.
                    let variant_scroll_height =
                        (ui.ctx().input(|i| i.viewport_rect().height()) * 0.35).clamp(150.0, 600.0);

                    let table = egui_extras::TableBuilder::new(ui)
                        .id_salt("variant_details")
                        .striped(true)
                        .resizable(true)
                        .auto_shrink([false, false])
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(egui_extras::Column::auto().at_least(60.0)) // Count
                        .column(egui_extras::Column::auto().at_least(70.0)) // Incidence
                        .column(egui_extras::Column::auto().at_least(50.0)) // Motif
                        .column(egui_extras::Column::remainder().at_least(100.0).clip(true)) // Sequence (last = fills remaining width)
                        .max_scroll_height(variant_scroll_height)
                        .sense(egui::Sense::click());

                    let selected_vi = self.selected_variant_index;

                    table
                        .header(row_height, |mut header| {
                            for (col_idx, col_name) in ["Count", "Incidence", "Motif", "Sequence"]
                                .iter()
                                .enumerate()
                            {
                                header.col(|ui| {
                                    let label = if current_sort.is_some_and(|s| s.column == col_idx)
                                    {
                                        format!(
                                            "{}{}",
                                            col_name,
                                            current_sort.unwrap().direction.indicator()
                                        )
                                    } else {
                                        col_name.to_string()
                                    };
                                    if ui
                                        .add(
                                            egui::Label::new(egui::RichText::new(label).strong())
                                                .sense(egui::Sense::click()),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        clicked_col.set(Some(col_idx));
                                    }
                                });
                            }
                        })
                        .body(|body| {
                            body.rows(row_height, sorted_indices.len(), |mut row| {
                                let vi = sorted_indices[row.index()];
                                let v = &variants[vi];

                                row.set_selected(selected_vi == Some(vi));

                                row.col(|ui| {
                                    ui.label(format!("{}", v.count));
                                });
                                row.col(|ui| {
                                    ui.label(format!("{:.1}%", v.incidence));
                                });
                                row.col(|ui| {
                                    if let Some(ref ms) = v.motif_short {
                                        ui.label(ms);
                                    }
                                });
                                row.col(|ui| {
                                    ui.monospace(&v.sequence);
                                });

                                if row.response().clicked() {
                                    clicked_variant.set(Some(vi));
                                }
                            });
                        });

                    // Apply sort change after table rendering completes
                    if let Some(col) = clicked_col.get() {
                        self.variant_table_sort = Some(
                            if self.variant_table_sort.is_some_and(|s| s.column == col) {
                                TableSort::new(
                                    col,
                                    self.variant_table_sort.unwrap().direction.toggle(),
                                )
                            } else {
                                TableSort::new(col, SortDirection::Ascending)
                            },
                        );
                    }

                    // Apply variant row click -- update selection and invalidate metadata cache
                    if let Some(vi) = clicked_variant.get() {
                        self.selected_variant_index = Some(vi);
                        self.cached_metadata = None;
                        self.expanded_metadata_fields.clear();
                    }
                }
            }
        } else {
            ui.colored_label(self.tokens.text_muted, "Click a position to view details");
        }
    }

    fn show_variant_panel(&mut self, ui: &mut egui::Ui, results: &Results) {
        Self::section_header(ui, "Variant Distribution");
        if let Some(pos_num) = self.selected_position {
            if let Some(&idx) = self.position_index_map.get(&pos_num) {
                let pos = &results.results[idx];
                if let Some(ref variants) = pos.diversity_motifs {
                    let mut index_inc = 0.0_f64;
                    let mut major_inc = 0.0_f64;
                    let mut minor_inc = 0.0_f64;
                    let mut unique_inc = 0.0_f64;

                    for v in variants {
                        match v.motif_short.as_deref() {
                            Some("I") => index_inc += v.incidence,
                            Some("Ma") => major_inc += v.incidence,
                            Some("Mi") => minor_inc += v.incidence,
                            Some("U") => unique_inc += v.incidence,
                            _ => {}
                        }
                    }

                    let bars: [(&str, f64, egui::Color32); 6] = [
                        ("Index", index_inc, self.tokens.motif_index),
                        ("Major", major_inc, self.tokens.motif_major),
                        ("Minor", minor_inc, self.tokens.motif_minor),
                        ("Unique", unique_inc, self.tokens.motif_unique),
                        (
                            "Total Variants",
                            pos.total_variants_incidence,
                            self.tokens.text_muted,
                        ),
                        (
                            "Distinct Variants",
                            pos.distinct_variants_incidence,
                            self.tokens.info_color,
                        ),
                    ];

                    for (label, value, color) in bars {
                        ui.horizontal(|ui| {
                            // Reserve space for the widest label ("Distinct Variants: 100.0%"
                            // ≈ 140px in proportional font); use the rest for the bar.
                            let label_reserve = 170.0_f32;
                            let bar_max_width = (ui.available_width()
                                - label_reserve
                                - ui.spacing().item_spacing.x)
                                .max(40.0); // floor at 40px so bar never vanishes
                            let bar_width = (value / 100.0) as f32 * bar_max_width;
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(bar_max_width, 16.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(bar_width.max(1.0), 16.0),
                                ),
                                2.0_f32,
                                color,
                            );
                            ui.label(format!("{}: {:.1}%", label, value));
                        });
                    }
                }
            }
        } else {
            ui.colored_label(self.tokens.text_muted, "Select a position");
        }

        // Metadata panel (SRP: delegated to extracted method)
        if !self.available_metadata_fields.is_empty() {
            ui.add_space(8.0);
            self.show_metadata_section(ui, results);
        }
    }

    /// Per-variant metadata section (SRP: extracted from `show_variant_panel`).
    ///
    /// Displays metadata for the SELECTED variant only, matching the published
    /// DiMA methodology (PMC11596295): "an individual distinct sequence is
    /// populated with the corresponding metadata header tags."
    ///
    /// Uses compact horizontal bars per field inside CollapsingHeaders for
    /// visual encoding of proportions (NNGroup bar chart research).
    /// Cached by (position, variant_index) to avoid per-frame recomputation.
    fn show_metadata_section(&mut self, ui: &mut egui::Ui, results: &Results) {
        let pos_num = match self.selected_position {
            Some(p) => p,
            None => return,
        };
        let vi = match self.selected_variant_index {
            Some(v) => v,
            None => {
                ui.colored_label(self.tokens.text_muted, "Select a variant to view metadata");
                return;
            }
        };

        let pos_idx = match self.position_index_map.get(&pos_num) {
            Some(&i) => i,
            None => return,
        };
        let pos = &results.results[pos_idx];
        let variants = match pos.diversity_motifs {
            Some(ref v) => v,
            None => return,
        };
        let variant = match variants.get(vi) {
            Some(v) => v,
            None => return,
        };

        // Header showing which variant's metadata is displayed
        let motif_label = variant.motif_short.as_deref().unwrap_or("?");
        let seq_display = if variant.sequence.len() > 20 {
            format!("{}...", &variant.sequence[..20])
        } else {
            variant.sequence.clone()
        };
        Self::section_header(
            ui,
            format!(
                "Metadata ({} - {}, {:.1}%)",
                seq_display, motif_label, variant.incidence
            ),
        );

        // Check cache validity (keyed by position AND variant index)
        let cache_key = (pos_num, vi);
        let needs_rebuild = self
            .cached_metadata
            .as_ref()
            .is_none_or(|((cp, cv), _)| *cp != pos_num || *cv != vi);

        if needs_rebuild {
            let extracted = Self::extract_variant_metadata(variant);
            self.cached_metadata = Some((cache_key, extracted));
        }

        // Render from cache using compact horizontal bars
        if let Some((_, ref fields)) = self.cached_metadata {
            let field_count = fields.len();
            for (field, values) in fields {
                let total: usize = values.iter().map(|(_, c)| *c).sum();
                if total == 0 {
                    continue;
                }

                // CollapsingHeader per field (default open when few fields)
                // Determine whether this field's "Others" bucket is expanded
                let is_expanded = self.expanded_metadata_fields.contains(field);
                let top_n = 5;
                let hidden_count = values.len().saturating_sub(top_n);
                // When only 1-2 items would be hidden, just show all (no toggle needed)
                let effective_show_all = is_expanded || hidden_count <= 2;
                let display_count = if effective_show_all {
                    values.len()
                } else {
                    top_n
                };
                let palette_len = self.tokens.categorical_palette.len();

                egui::CollapsingHeader::new(egui::RichText::new(format!("{}:", field)).strong())
                    .default_open(field_count <= 3)
                    .id_salt(format!("meta_{}", field))
                    .show(ui, |ui| {
                        // ScrollArea constrains expanded metadata to 200px,
                        // preventing viewport blowout when "Show all" is clicked
                        // on fields with many values (e.g. 50+ countries).
                        // auto_shrink (default true) keeps it compact when
                        // content fits. Nested ScrollArea is safe on egui 0.34.3
                        // (PR #7904 fixes scroll event consumption).
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .id_salt(format!("meta_scroll_{}", field))
                            .show(ui, |ui| {
                                for (i, (value, count)) in
                                    values.iter().take(display_count).enumerate()
                                {
                                    let pct = *count as f64 / total as f64 * 100.0;
                                    let fraction = (pct / 100.0) as f32;

                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [80.0, 14.0],
                                            egui::Label::new(egui::RichText::new(value).size(11.0))
                                                .truncate(),
                                        );

                                        let bar_width = (ui.available_width() - 50.0).max(20.0);
                                        let (bar_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(bar_width, 14.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            2.0,
                                            self.tokens.surface_secondary,
                                        );
                                        let fill_width = (fraction * bar_width).max(1.0);
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_size(
                                                bar_rect.min,
                                                egui::vec2(fill_width, 14.0),
                                            ),
                                            2.0,
                                            self.tokens.categorical_palette[i % palette_len],
                                        );
                                        ui.label(format!("{:.1}%", pct));
                                    });
                                }

                                // "Others" summary when > 2 items are hidden
                                if hidden_count > 2 && !effective_show_all {
                                    let others_total: usize =
                                        values.iter().skip(top_n).map(|(_, c)| *c).sum();
                                    let pct = others_total as f64 / total as f64 * 100.0;
                                    let fraction = (pct / 100.0) as f32;

                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [80.0, 14.0],
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "Others ({})",
                                                    hidden_count
                                                ))
                                                .size(11.0)
                                                .color(self.tokens.text_muted),
                                            )
                                            .truncate(),
                                        );
                                        let bar_width = (ui.available_width() - 50.0).max(20.0);
                                        let (bar_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(bar_width, 14.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            2.0,
                                            self.tokens.surface_secondary,
                                        );
                                        let fill_width = (fraction * bar_width).max(1.0);
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_size(
                                                bar_rect.min,
                                                egui::vec2(fill_width, 14.0),
                                            ),
                                            2.0,
                                            self.tokens.text_muted,
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{:.1}%", pct))
                                                .color(self.tokens.text_muted),
                                        );
                                    });
                                }
                            });

                        // Toggle link stays OUTSIDE ScrollArea so it's always
                        // visible as a fixed footer, never buried in a scroll
                        if hidden_count > 2 {
                            let label = if effective_show_all {
                                format!("Show top {}", top_n)
                            } else {
                                format!("Show all {} values", values.len())
                            };
                            if ui
                                .add(
                                    egui::Label::new(
                                        egui::RichText::new(label)
                                            .size(11.0)
                                            .color(self.tokens.accent),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                if effective_show_all {
                                    self.expanded_metadata_fields.remove(field);
                                } else {
                                    self.expanded_metadata_fields.insert(field.clone());
                                }
                            }
                        }
                    });
            }
        }
    }

    /// Filter controls panel: persistent sidebar for position/entropy range,
    /// motif type, and low-support filters. Separated from Position Explorer
    /// per SRP -- filters are a control surface, the explorer is a data view.
    fn show_filter_controls(&mut self, ui: &mut egui::Ui, results: &Results) {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            Self::section_header(ui, "Filters");

            let mut filter_changed = false;

            // DragValue range bounds derived from the full (unfiltered) result set.
            // O(n) scans but n = number of positions (typically <10k),
            // costing <100us per frame -- negligible for interactive UI.
            let last_position = results.results.last().map(|p| p.position).unwrap_or(1);
            let max_entropy = results
                .results
                .iter()
                .map(|p| p.entropy)
                .filter(|e| e.is_finite())
                .fold(0.0_f64, f64::max);

            ui.horizontal(|ui| {
                ui.label("Position:");
                let r = &mut self.filter_state.position_range;
                filter_changed |= ui
                    .add(
                        egui::DragValue::new(&mut r.0)
                            .prefix("from ")
                            .range(1..=last_position),
                    )
                    .changed();
                filter_changed |= ui
                    .add(
                        egui::DragValue::new(&mut r.1)
                            .prefix("to ")
                            .range(1..=last_position),
                    )
                    .changed();
            });

            ui.horizontal(|ui| {
                ui.label("Entropy:");
                let e = &mut self.filter_state.entropy_range;
                filter_changed |= ui
                    .add(
                        egui::DragValue::new(&mut e.0)
                            .prefix("min ")
                            .speed(0.01)
                            .range(0.0..=max_entropy),
                    )
                    .changed();
                filter_changed |= ui
                    .add(
                        egui::DragValue::new(&mut e.1)
                            .prefix("max ")
                            .speed(0.01)
                            .range(0.0..=max_entropy),
                    )
                    .changed();
            });

            // horizontal_wrapped so checkboxes wrap within the 250px panel
            ui.horizontal_wrapped(|ui| {
                ui.label("Motifs:");
                for motif in &[
                    MotifType::Index,
                    MotifType::Major,
                    MotifType::Minor,
                    MotifType::Unique,
                ] {
                    let mut checked = self.filter_state.motif_types.contains(motif);
                    if ui.checkbox(&mut checked, motif.display_name()).changed() {
                        if checked {
                            if !self.filter_state.motif_types.contains(motif) {
                                self.filter_state.motif_types.push(motif.clone());
                            }
                        } else {
                            self.filter_state.motif_types.retain(|m| m != motif);
                        }
                        filter_changed = true;
                    }
                }
            });

            filter_changed |= ui
                .checkbox(
                    &mut self.filter_state.include_low_support,
                    "Include low support",
                )
                .changed();

            if filter_changed {
                self.filtered_positions = self.filter_state.apply(results);
                self.position_explorer_sort = None;
                self.data_version += 1;
                self.charts_need_data_upload = true;
            }
        });
    }

    /// Filtered Data Summary: read-only card showing live statistics about
    /// the currently filtered position set. Placed below the Filters panel
    /// to provide immediate filter feedback per Grafana/Helios/DataCamp
    /// dashboard design consensus (NNGroup, OmNI PMC12789801). No state
    /// mutation — only reads `filtered_positions` and `results`.
    fn show_filter_summary(&self, ui: &mut egui::Ui, results: &Results) {
        let filtered_count = self.filtered_positions.len();
        let total_count = results.results.len();
        let proportion = if total_count > 0 {
            filtered_count as f32 / total_count as f32
        } else {
            0.0
        };

        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            // Count label OUTSIDE the bar per Carbon/Material/Primer best practice.
            // Text inside a ProgressBar creates WCAG SC 1.4.3 contrast failures:
            // text_primary on opaque fill only achieves ~3.5:1 (light) / ~2.2:1 (dark).
            ui.label(format!("Showing {} / {}", filtered_count, total_count));

            // Slim 6px progress bar with opaque fill for WCAG SC 1.4.11 compliance.
            // Default selection_bg (31% opacity) only achieves ~1.5:1 contrast vs track.
            // tokens.progress_bar_fill is opaque: 4.47:1 (light) / 4.94:1 (dark).
            ui.add(
                egui::ProgressBar::new(proportion)
                    .fill(self.tokens.progress_bar_fill)
                    .desired_height(6.0)
                    .corner_radius(ui.visuals().noninteractive().corner_radius),
            );

            // Single-pass fold: compute filtered avg entropy AND low-support count
            // together to halve iteration count vs two separate passes.
            // NS/LS check matches filter.rs line 123 (ELS excluded per PMC11596295).
            let (entropy_sum, finite_count, low_support_count) = self
                .filtered_positions
                .iter()
                .fold((0.0_f64, 0usize, 0usize), |(sum, n, ls), &idx| {
                    let pos = &results.results[idx];
                    let (new_sum, new_n) = if pos.entropy.is_finite() {
                        (sum + pos.entropy, n + 1)
                    } else {
                        (sum, n)
                    };
                    let new_ls = if pos
                        .low_support
                        .as_deref()
                        .is_some_and(|tag| tag == "NS" || tag == "LS")
                    {
                        ls + 1
                    } else {
                        ls
                    };
                    (new_sum, new_n, new_ls)
                });

            let filtered_avg = if finite_count > 0 {
                Some(entropy_sum / finite_count as f64)
            } else {
                None
            };

            ui.horizontal(|ui| {
                ui.label("Avg H:");
                match filtered_avg {
                    Some(avg) => {
                        ui.strong(format!("{:.4}", avg));
                        ui.weak(format!("(overall {:.4})", results.average_entropy));
                    }
                    None => {
                        ui.weak("--");
                    }
                }
            });

            // Low-support count shown only when > 0 to avoid noise
            if low_support_count > 0 {
                ui.weak(format!(
                    "Low support: {} position{}",
                    low_support_count,
                    if low_support_count == 1 { "" } else { "s" }
                ));
            }
        });
    }

    /// Position Explorer: virtual-scroll sortable table of filtered positions.
    /// Uses egui_extras::TableBuilder for on-demand row rendering (only visible
    /// rows are laid out and painted). Clicking a row selects that position;
    /// clicking a column header sorts by that column.
    fn show_position_explorer(&mut self, ui: &mut egui::Ui, results: &Results) {
        ui.group(|ui| {
            Self::section_header(
                ui,
                format!(
                    "Position Explorer ({} positions)",
                    self.filtered_positions.len()
                ),
            );

            if self.filtered_positions.is_empty() {
                ui.colored_label(self.tokens.text_muted, "No positions match current filters");
                return;
            }

            let row_height = 22.0;
            let table_height = 350.0;

            // Build a sorted index for the filtered positions
            let filtered_indices = &self.filtered_positions;
            let mut sorted_view: Vec<usize> = (0..filtered_indices.len()).collect();

            if let Some(ref sort) = self.position_explorer_sort {
                sorted_view.sort_by(|&a, &b| {
                    let pos_a = &results.results[filtered_indices[a]];
                    let pos_b = &results.results[filtered_indices[b]];
                    let cmp = match sort.column {
                        0 => pos_a.position.cmp(&pos_b.position),
                        1 => pos_a
                            .entropy
                            .partial_cmp(&pos_b.entropy)
                            .unwrap_or(std::cmp::Ordering::Equal),
                        2 => pos_a.support.cmp(&pos_b.support),
                        3 => pos_a.low_support.cmp(&pos_b.low_support),
                        4 => {
                            let top_a = pos_a
                                .diversity_motifs
                                .as_ref()
                                .and_then(|v| v.first())
                                .map(|v| v.sequence.as_str())
                                .unwrap_or("");
                            let top_b = pos_b
                                .diversity_motifs
                                .as_ref()
                                .and_then(|v| v.first())
                                .map(|v| v.sequence.as_str())
                                .unwrap_or("");
                            top_a.cmp(top_b)
                        }
                        _ => std::cmp::Ordering::Equal,
                    };
                    match sort.direction {
                        SortDirection::Ascending => cmp,
                        SortDirection::Descending => cmp.reverse(),
                    }
                });
            }

            let selected = self.selected_position;
            let warning_color = self.tokens.warning_color;
            let current_sort = self.position_explorer_sort;
            let mut new_selection: Option<usize> = None;
            let clicked_col = std::cell::Cell::new(None::<usize>);

            let col_names = ["Position", "Entropy", "Support", "Status", "Top Variant"];

            let table = egui_extras::TableBuilder::new(ui)
                .id_salt("position_explorer")
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::auto().at_least(60.0))
                .column(egui_extras::Column::auto().at_least(80.0))
                .column(egui_extras::Column::auto().at_least(60.0))
                .column(egui_extras::Column::auto().at_least(50.0))
                .column(egui_extras::Column::remainder())
                .max_scroll_height(table_height)
                .sense(egui::Sense::click());

            table
                .header(row_height, |mut header| {
                    for (col_idx, col_name) in col_names.iter().enumerate() {
                        header.col(|ui| {
                            let label = if current_sort.is_some_and(|s| s.column == col_idx) {
                                format!(
                                    "{}{}",
                                    col_name,
                                    current_sort.unwrap().direction.indicator()
                                )
                            } else {
                                col_name.to_string()
                            };
                            if ui
                                .add(
                                    egui::Label::new(egui::RichText::new(label).strong())
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                clicked_col.set(Some(col_idx));
                            }
                        });
                    }
                })
                .body(|body| {
                    body.rows(row_height, sorted_view.len(), |mut row| {
                        let filtered_idx = sorted_view[row.index()];
                        let idx = filtered_indices[filtered_idx];
                        let pos = &results.results[idx];

                        let is_selected = selected == Some(pos.position);
                        if is_selected {
                            row.set_selected(true);
                        }

                        row.col(|ui| {
                            ui.label(format!("{}", pos.position));
                        });
                        row.col(|ui| {
                            ui.label(format!("{:.4}", pos.entropy));
                        });
                        row.col(|ui| {
                            ui.label(format!("{}", pos.support));
                        });
                        row.col(|ui| {
                            if let Some(ref ls) = pos.low_support {
                                ui.colored_label(warning_color, ls.as_str());
                            } else {
                                ui.label("\u{2714}");
                            }
                        });
                        row.col(|ui| {
                            if let Some(ref variants) = pos.diversity_motifs {
                                if let Some(top) = variants.first() {
                                    ui.monospace(format!(
                                        "{}  {:.1}%",
                                        top.sequence, top.incidence
                                    ));
                                }
                            }
                        });

                        if row.response().clicked() {
                            new_selection = Some(pos.position);
                        }
                    });
                });

            // Apply header sort click
            if let Some(col) = clicked_col.get() {
                self.position_explorer_sort = Some(
                    if self.position_explorer_sort.is_some_and(|s| s.column == col) {
                        TableSort::new(col, self.position_explorer_sort.unwrap().direction.toggle())
                    } else {
                        TableSort::new(col, SortDirection::Ascending)
                    },
                );
            }

            // Apply row click selection and invalidate variant sort / metadata cache
            if let Some(pos) = new_selection {
                if self.selected_position != Some(pos) {
                    self.selected_position = Some(pos);
                    self.variant_table_sort = None;
                    self.cached_metadata = None;
                    self.expanded_metadata_fields.clear();
                    self.selected_variant_index = self.find_default_variant(results, pos);
                }
            }
        });
    }
}

/// Human-readable relative time from a Unix timestamp (e.g., "2 min ago", "yesterday").
/// Avoids adding chrono as a dependency — uses simple second-based thresholds.
fn format_relative_time(now_secs: u64, then_secs: u64) -> String {
    if then_secs == 0 || now_secs < then_secs {
        return String::new();
    }
    let elapsed = now_secs - then_secs;
    match elapsed {
        0..=59 => "just now".to_string(),
        60..=3599 => {
            let mins = elapsed / 60;
            if mins == 1 {
                "1 min ago".to_string()
            } else {
                format!("{} min ago", mins)
            }
        }
        3600..=86399 => {
            let hours = elapsed / 3600;
            if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        }
        86400..=172799 => "yesterday".to_string(),
        172800..=2591999 => {
            let days = elapsed / 86400;
            format!("{} days ago", days)
        }
        _ => {
            let weeks = elapsed / 604800;
            if weeks <= 4 {
                if weeks == 1 {
                    "1 week ago".to_string()
                } else {
                    format!("{} weeks ago", weeks)
                }
            } else {
                let months = elapsed / 2592000;
                if months == 1 {
                    "1 month ago".to_string()
                } else {
                    format!("{} months ago", months)
                }
            }
        }
    }
}
