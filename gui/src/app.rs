//! DimaApp: top-level application state and eframe::App implementation.
//!
//! Uses eframe's `logic()` + `ui()` split for SRP:
//! - `logic()`: state mutation (polling workers, applying results, progress)
//! - `ui()`: rendering (panels, charts, buttons)
//!
//! Long-running work lives in [`crate::workers`]; configuration state lives in
//! [`crate::state`]. This module orchestrates them.

use dima_lib::{compute_hcs_regions, HcsRegion, Position, Results, Variant};
use std::path::PathBuf;
use std::sync::Arc;

use crate::charts::axis::nice_ticks;
use crate::charts::gpu_line::{
    init_gpu_resources, EntropyLineCallback, EntropyVertex, ViewTransform,
};
use crate::charts::lttb::{lttb_downsample_by_range, Point};
use crate::charts::scene::{ChartScene, SceneStyle};
use crate::error::{ErrorMessage, ErrorState};
use crate::panels::{CommandPalette, PaletteAction};
use crate::state::{
    AlphabetChoice, AnalysisConfigUi, FilterState, MotifType, RecentFileStore, SortDirection,
    TableSort, WorkspaceConfig,
};
use crate::theme::{init_both_theme_styles, DesignTokens, Theme};
use crate::util::truncate_display;
use crate::views::View;
use crate::workers::analysis::{AnalysisJob, AnalysisOutcome, AnalysisPhase};
use crate::workers::io::{ExportFormat, ExportOutcome, ExportRequest, ImportOutcome};
use crate::workers::validation::ValidationOutcome;
use crate::workers::{analysis, io as io_worker, validation, Poll, Worker};

/// Maximum number of points to render in the entropy chart before
/// LTTB downsampling kicks in. Keeps the per-frame line segment
/// count manageable for CPU rendering (~800 draw calls).
const ENTROPY_CHART_MAX_POINTS: usize = 800;

/// Narrowest zoom span, in positions.
///
/// Prevents a stray micro-drag or scroll burst from zooming to a degenerate
/// range where the axis would collapse.
const MIN_ENTROPY_ZOOM_SPAN: f32 = 5.0;

/// Width of the right-hand inspector dock.
///
/// Fixed rather than resizable: it always holds the same content at the same
/// density, and a stable width keeps the chart and table geometry predictable
/// between sessions. Wide enough for a k-mer sequence plus its counts without
/// wrapping, narrow enough to leave the centre the majority of the window.
const INSPECTOR_WIDTH: f32 = 340.0;

/// Motif classes in the order the composition bar draws and lists them, each
/// paired with the `motif_short` code `Position::new` assigns to it.
///
/// Pairing the code with its label here makes this the single source of truth
/// for both: [`DimaApp::motif_class_index`] maps a code to an index into this
/// table, and a test holds the two in step, so neither can drift into
/// mislabelling a segment.
const MOTIF_CLASSES: [(&str, &str); 4] = [
    ("I", "Index"),
    ("Ma", "Major"),
    ("Mi", "Minor"),
    ("U", "Unique"),
];

/// Metadata values listed per field before the user asks for the rest.
///
/// Five rows cover the dominant categories of a typical field (host, country,
/// year) and give every field the same collapsed height, which is what makes a
/// list of fields scannable. It is also the top-N default of the field
/// breakdowns this list is modelled on (Kibana field statistics, Datadog
/// facets).
const METADATA_TOP_VALUES: usize = 5;

/// Hard cap on metadata values rendered for a single field, even after the
/// user asks to see all of them.
///
/// A field can hold as many distinct values as the alignment has sequences —
/// tens of thousands for a large dataset, and unbounded for a `.dima` file,
/// whose container is validated but whose contents are not. Laying out every
/// row would cost a frame and produce a list nobody can read, so comparable
/// field sidebars cap theirs too (Splunk shows at most ~100 values per field).
/// The remainder is reported as a count, so nothing disappears silently.
const METADATA_MAX_VALUES: usize = 200;

/// Left inset for the values belonging to a metadata field.
///
/// Sits just inside where the field name starts, so the values read as nested
/// under their field without needing rules or boxes to say so.
const METADATA_INDENT: f32 = 24.0;

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

    // ── Background jobs ──────────────────────────────────────────────────
    // Each is `Some` only while that job is in flight. Dropping a handle
    // abandons the job safely (see `workers::Worker`).
    /// Background FASTA pre-validation.
    validation_job: Option<Worker<ValidationOutcome>>,
    /// Background diversity analysis, with its two-phase progress counters.
    analysis_job: Option<AnalysisJob>,
    /// Background `.dima` import.
    import_job: Option<Worker<ImportOutcome>>,
    /// Path being imported, kept so the recent-files entry and the success
    /// message can name it once the worker reports back.
    import_path: Option<PathBuf>,
    /// Background export (JSON/TSV/.dima/PNG/SVG).
    export_job: Option<Worker<ExportOutcome>>,

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
    /// Set when a filter control changed; applied by `logic()` once the
    /// interaction ends (see `apply_pending_filters`).
    filter_dirty: bool,

    // O(1) lookup: maps position number → index in results.results Vec.
    // Rebuilt whenever results change (analysis success or .dima import).
    // Replaces three per-frame O(n) iter().find() calls in detail panels.
    pub position_index_map: std::collections::HashMap<usize, usize>,

    // Selection state (cross-panel sync) -- indexes into UNFILTERED results
    pub selected_position: Option<usize>,
    /// Position under the mouse cursor on the entropy chart.
    ///
    /// Published by the chart and consumed by the position table, which
    /// highlights the matching row so the two views stay visually linked.
    /// `None` whenever the pointer is not over the chart.
    pub hovered_position: Option<usize>,

    // ── Sort state (one per sortable table) ──
    pub position_explorer_sort: Option<TableSort>,
    pub variant_table_sort: Option<TableSort>,

    // ── Cached per-variant metadata ──
    /// Pre-extracted metadata for the currently selected variant at the selected position.
    /// Cached to avoid per-frame re-computation and HashMap non-deterministic ordering.
    /// The key `(position_number, variant_index)` invalidates when either changes.
    cached_metadata: Option<CachedMetadata>,

    /// Metadata fields whose value list is expanded, or `None` before any field
    /// has been opened or closed — in which case the first field is opened so
    /// the section arrives showing data instead of a stack of shut headers.
    ///
    /// Deliberately sticky across position and variant changes: comparing one
    /// field between variants is the main reason to switch variants at all, and
    /// collapsing it on every click would defeat that. `Some(empty)` therefore
    /// means "the user closed everything" and is never silently re-opened.
    metadata_open_fields: Option<std::collections::HashSet<String>>,

    /// Metadata fields listing every value rather than only the top
    /// [`METADATA_TOP_VALUES`]. Sticky for the same reason as
    /// [`Self::metadata_open_fields`]; both are reset only when a new dataset
    /// arrives, which is the only point at which the field names can change.
    expanded_metadata_fields: std::collections::HashSet<String>,

    /// Index into the current position's `diversity_motifs` Vec for the selected variant.
    /// `None` means no variant is selected (no metadata shown).
    /// Auto-set to the default variant (Index or highest-incidence) when a position is selected.
    pub selected_variant_index: Option<usize>,

    // ── Variant table tools (Inspector) ──
    /// Case-insensitive substring filter over variant sequence / motif.
    variant_search: String,
    /// Hide variants whose incidence is below this percentage.
    ///
    /// A position can hold thousands of singleton variants; this is the quickest
    /// way to reduce the table to the biologically interesting ones.
    variant_min_incidence: f64,

    // Entropy chart viewport (zoom/pan state)
    /// Visible x-range as (start_position, end_position), both 1-based.
    /// None means "show all" (no zoom applied).
    pub entropy_viewport: Option<(f64, f64)>,

    // GPU chart state
    pub data_version: u64,
    pub charts_need_data_upload: bool,
    /// Screen-x where a Shift+drag region-zoom began, while one is in progress.
    ///
    /// Held in app state because egui reports only the release position on the
    /// frame a drag ends, so the origin cannot be recovered from the response.
    chart_drag_origin_x: Option<f32>,

    /// Viewport whose downsampled points are currently on the GPU.
    ///
    /// Compared against `entropy_viewport` each frame so pan/zoom triggers a
    /// re-upload at the new visible resolution.
    last_uploaded_viewport: Option<(f64, f64)>,
    /// Whether the wgpu GPU pipeline was successfully initialized.
    /// Falls back to CPU rendering if GPU init failed (e.g., headless CI).
    gpu_available: bool,

    /// Cmd/Ctrl+K finder and action launcher.
    command_palette: CommandPalette,

    // Recent files (persisted to platform config dir)
    pub recent_files: RecentFileStore,

    // egui context clone for request_repaint from workers
    egui_ctx: Option<egui::Context>,
}

impl DimaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Install bundled typography before any text is laid out, so the first
        // frame already measures with the final fonts (avoids a reflow flash).
        crate::theme::fonts::install(&cc.egui_ctx);

        let theme = Theme::default();
        let tokens = match theme {
            Theme::Dark => DesignTokens::dark(),
            Theme::Light => DesignTokens::light(),
        };
        // Pre-populate BOTH theme slots in egui's Options with our custom
        // styles, so toggling always uses our design tokens (not egui defaults).
        init_both_theme_styles(&cc.egui_ctx);

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

        Self::with_theme(theme, tokens, gpu_available)
    }

    /// Build the initial state.
    ///
    /// Split from [`Self::new`] so tests can construct an app without an eframe
    /// `CreationContext` (which cannot be built outside a real event loop) and
    /// still exercise the real rendering code.
    fn with_theme(theme: Theme, tokens: DesignTokens, gpu_available: bool) -> Self {
        Self {
            current_view: View::Setup,
            theme,
            tokens,
            error_state: ErrorState::default(),
            selected_file: None,
            validation_result: None,
            analysis_config: AnalysisConfigUi::default(),
            workspace_config: WorkspaceConfig::default(),
            validation_job: None,
            analysis_job: None,
            import_job: None,
            import_path: None,
            export_job: None,
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
            filter_dirty: false,
            position_index_map: std::collections::HashMap::new(),
            selected_position: None,
            hovered_position: None,
            position_explorer_sort: None,
            variant_table_sort: None,
            cached_metadata: None,
            metadata_open_fields: None,
            expanded_metadata_fields: std::collections::HashSet::new(),
            selected_variant_index: None,
            variant_search: String::new(),
            variant_min_incidence: 0.0,
            entropy_viewport: None,
            data_version: 0,
            charts_need_data_upload: false,
            chart_drag_origin_x: None,
            last_uploaded_viewport: None,
            gpu_available,
            command_palette: CommandPalette::default(),
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

    /// Render a sub-section label: one level below [`Self::section_header`].
    ///
    /// Body size at `strong` weight, so a group inside a section reads as
    /// subordinate to it. Holding the inspector to these two levels is what
    /// makes the per-position/per-variant split legible at a glance; a third
    /// heading weight would flatten the distinction again.
    fn sub_label(ui: &mut egui::Ui, text: impl Into<String>) {
        ui.label(egui::RichText::new(text).strong());
    }

    /// Font for text painted directly into inspector rows.
    ///
    /// Resolved from the active style rather than hard-coded so painted text
    /// measures identically to neighbouring [`egui::Ui::label`] text, and keeps
    /// doing so if the text styles are ever re-tuned.
    fn row_font(ui: &egui::Ui) -> egui::FontId {
        egui::TextStyle::Body.resolve(ui.style())
    }

    /// Lay out one line of `text`, elided with `…` at `max_width`.
    ///
    /// Elision is measured in glyphs, which a character budget cannot do: in
    /// proportional text any fixed count either shortens narrow names that
    /// would have fitted or lets wide ones overrun. Clipping instead would cut
    /// the final glyph in half and read as a rendering fault rather than as
    /// "there is more here".
    ///
    /// `text` is bounded first because laying out a pathological value — a
    /// `.dima` file's strings are never length-checked — would otherwise cost a
    /// frame. The bound is far wider than any inspector column, so the ellipsis
    /// is always what the user actually sees.
    fn elided_galley(
        ui: &egui::Ui,
        text: &str,
        font: egui::FontId,
        color: egui::Color32,
        max_width: f32,
    ) -> std::sync::Arc<egui::Galley> {
        const LAYOUT_BUDGET: usize = 128;

        let mut job = egui::text::LayoutJob::single_section(
            truncate_display(text, LAYOUT_BUDGET),
            egui::TextFormat::simple(font, color),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(0.0));
        ui.painter().layout_job(job)
    }

    /// Total incidence per motif class at a position, in [`MOTIF_CLASSES`]
    /// order.
    ///
    /// Variants with no recognised `motif_short` are excluded rather than
    /// bucketed: they carry no classification, so charging their incidence to
    /// any class would misstate the composition. `Position::new` always
    /// classifies what it builds, but a `.dima` import reconstructs positions
    /// straight from the file, which is validated for container integrity and
    /// not for the values inside it. Non-finite incidences are skipped for the
    /// same reason — a single NaN would otherwise poison the total and collapse
    /// every segment to zero width.
    fn motif_incidences(variants: &[Variant]) -> [f64; 4] {
        let mut totals = [0.0_f64; 4];
        for v in variants {
            let Some(class) = Self::motif_class_index(v.motif_short.as_deref()) else {
                continue;
            };
            if v.incidence.is_finite() && v.incidence > 0.0 {
                totals[class] += v.incidence;
            }
        }
        totals
    }

    /// Each motif class's percentage share of the classified variants at a
    /// position, in [`MOTIF_CLASSES`] order, or `None` when there is nothing
    /// meaningful to draw.
    ///
    /// Separated from the painting so the "is this drawable at all" decision is
    /// testable without a renderer, and so the bar and its legend read the same
    /// numbers rather than each dividing again.
    ///
    /// `None` covers a position with no classified variant, and one whose
    /// incidences sum past the floating-point range — only reachable from an
    /// import carrying absurd values, where every share would come out as `NaN`
    /// and the legend would read "NaN%".
    fn motif_shares(variants: &[Variant]) -> Option<[f64; 4]> {
        let totals = Self::motif_incidences(variants);
        let sum: f64 = totals.iter().sum();
        if !sum.is_finite() || sum <= 0.0 {
            return None;
        }
        Some(totals.map(|total| total / sum * 100.0))
    }

    /// Index into [`MOTIF_CLASSES`] for a `motif_short` code, or `None` when the
    /// code is absent or unrecognised.
    ///
    /// Matched rather than searched through the table: this runs once per
    /// variant per frame and a single position can hold thousands of them. The
    /// arms are the table's codes in its own order, which
    /// `motif_class_index_agrees_with_the_class_table` keeps true.
    fn motif_class_index(code: Option<&str>) -> Option<usize> {
        match code? {
            "I" => Some(0),
            "Ma" => Some(1),
            "Mi" => Some(2),
            "U" => Some(3),
            _ => None,
        }
    }

    /// The selected position's data, if a position is selected and present.
    ///
    /// Selection is held as a position *number*, so the lookup can only fail if
    /// selection and results fall out of step. Resolving it in one place lets
    /// every inspector section degrade to "nothing to show" together, rather
    /// than each rendering half a panel around a stale selection.
    ///
    /// The result borrows `results`, not `self`, so callers stay free to mutate
    /// their own view state while holding it.
    fn selected_position_data<'r>(&self, results: &'r Results) -> Option<&'r Position> {
        let position = self.selected_position?;
        let &idx = self.position_index_map.get(&position)?;
        results.results.get(idx)
    }

    /// How many values of a metadata field to render.
    ///
    /// Bounded in both modes: collapsed shows the top [`METADATA_TOP_VALUES`],
    /// and expanded still stops at [`METADATA_MAX_VALUES`] so one pathological
    /// field cannot stall a frame.
    fn visible_metadata_values(len: usize, show_all: bool) -> usize {
        if show_all {
            len.min(METADATA_MAX_VALUES)
        } else {
            len.min(METADATA_TOP_VALUES)
        }
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

    /// Start a background analysis of `file_path` with the current configuration.
    fn start_analysis(&mut self, file_path: PathBuf) {
        // The position count comes from the validation scan so progress has a
        // denominator. `expected_positions` yields 0 when it cannot be known,
        // which the progress UI renders as indeterminate rather than as a
        // misleading percentage.
        let total_positions = analysis::expected_positions(
            self.validation_result
                .as_ref()
                .and_then(|vr| vr.alignment_length),
            self.analysis_config.kmer_length,
        );

        self.analysis_job = Some(analysis::spawn(
            self.egui_ctx.clone(),
            file_path,
            self.analysis_config.clone(),
            total_positions,
        ));
    }

    /// Cancel a running analysis and stop tracking it.
    ///
    /// Taking the handle drops the receiver, so even if the worker wins the race
    /// and completes, its result is discarded rather than overwriting fresh state.
    fn cancel_analysis(&mut self) {
        if let Some(job) = self.analysis_job.take() {
            job.cancel();
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

    /// Begin importing analysis results from a `.dima` binary file.
    ///
    /// Runs off-thread: decoding a large binary took long enough that doing it
    /// inline visibly froze the window.
    pub fn load_dima_binary(&mut self, path: &std::path::Path) {
        // A running analysis would overwrite the imported results on completion.
        self.cancel_analysis();

        self.import_path = Some(path.to_path_buf());
        self.import_job = Some(io_worker::spawn_import(
            self.egui_ctx.clone(),
            path.to_path_buf(),
        ));
    }

    /// Adopt a freshly computed or imported result set.
    ///
    /// The single place where results become current. Every derived structure is
    /// rebuilt and every piece of per-dataset view state is cleared together.
    /// Analysis completion and `.dima` import previously duplicated this
    /// sequence, which is exactly how stale zoom viewport and selection state
    /// leaked from one dataset into the next.
    fn apply_results(&mut self, results: Results) {
        let results = Arc::new(results);

        // Derived data — computed once here, never per frame.
        self.hcs_regions = compute_hcs_regions(&results, Some(self.workspace_config.hcs_threshold));
        self.available_metadata_fields = Self::scan_all_metadata_fields(&results);
        self.filter_state = FilterState::default_for(&results);
        self.filtered_positions = self.filter_state.apply(&results);
        self.position_index_map = results
            .results
            .iter()
            .enumerate()
            .map(|(i, p)| (p.position, i))
            .collect();

        // Per-dataset view state — cleared so nothing from the previous dataset
        // survives (a stale viewport would otherwise render an empty chart).
        self.selected_position = None;
        self.hovered_position = None;
        self.selected_variant_index = None;
        self.position_explorer_sort = None;
        self.variant_table_sort = None;
        self.cached_metadata = None;
        // The metadata field names belong to the previous dataset, so the
        // expand/open state keyed by them is meaningless here. This is the only
        // place either set is reset: within one dataset both are sticky, so
        // that switching variant keeps the field you are reading open.
        self.metadata_open_fields = None;
        self.expanded_metadata_fields.clear();
        self.entropy_viewport = None;

        self.results = Some(results);
        self.data_version += 1;
        self.charts_need_data_upload = true;
        // Force a fresh GPU upload: the previous dataset's points are still
        // resident, and its viewport must not be mistaken for the current one.
        self.last_uploaded_viewport = None;
        self.current_view = View::Workspace;

        // Details-on-demand still needs something to show on arrival, so select
        // the first position instead of presenting empty detail panels.
        self.select_first_filtered_position();
    }

    /// Select the first position that passes the current filters, if any.
    fn select_first_filtered_position(&mut self) {
        let Some(results) = self.results.clone() else {
            return;
        };
        let Some(&idx) = self.filtered_positions.first() else {
            return;
        };
        let position = results.results[idx].position;
        self.set_selected_position(Some(position), &results);
    }

    /// Change the selected position and invalidate everything derived from it.
    ///
    /// Centralised because several call sites (chart click, table row, keyboard
    /// navigation, auto-select) must invalidate the same derived state. When that
    /// sequence was duplicated, the variant selection and metadata cache could
    /// fall out of sync with the selected position.
    fn set_selected_position(&mut self, position: Option<usize>, results: &Results) {
        if self.selected_position == position {
            return;
        }
        self.selected_position = position;
        self.variant_table_sort = None;
        // Only the per-variant cache is invalidated. Which metadata fields are
        // open, and which list every value, is view state the user set and the
        // field names do not change between positions, so it survives.
        self.cached_metadata = None;
        self.selected_variant_index =
            position.and_then(|pos| self.find_default_variant(results, pos));
    }

    /// Build an export description of the chart as currently filtered.
    ///
    /// Built on the UI thread because it reads filter state; the result is
    /// self-contained so the export worker needs no access to app state.
    fn build_chart_scene(&self, results: &Results) -> Option<ChartScene> {
        if self.filtered_positions.is_empty() {
            return None;
        }
        let points: Vec<(f64, f64)> = self
            .filtered_positions
            .iter()
            .map(|&idx| {
                let p = &results.results[idx];
                (p.position as f64, p.entropy)
            })
            .collect();

        Some(ChartScene {
            title: format!("{} \u{2014} Shannon entropy", results.query_name),
            points,
            average_entropy: results.average_entropy,
            width: 1920,
            height: 640,
            style: SceneStyle::publication(),
        })
    }

    /// Start a background export of `format` to `path`.
    fn start_export(&mut self, format: ExportFormat, path: PathBuf, results: &Arc<Results>) {
        let scene = if format.is_figure() {
            match self.build_chart_scene(results) {
                Some(scene) => Some(scene),
                None => {
                    self.error_state.push(ErrorMessage::warning(
                        "No positions match the current filters, so there is no chart to export."
                            .to_string(),
                    ));
                    return;
                }
            }
        } else {
            None
        };

        self.export_job = Some(io_worker::spawn_export(
            self.egui_ctx.clone(),
            ExportRequest {
                format,
                path,
                results: Arc::clone(results),
                scene,
            },
        ));
    }

    /// Draw the whole application for one frame.
    ///
    /// Kept separate from the `eframe::App::ui` hook (which only forwards here)
    /// so the full render path can run in a headless test.
    pub(crate) fn render(&mut self, ui: &mut egui::Ui) {
        // Dropped files are handled once per frame for the whole window rather
        // than inside the setup form, so a drop works in the workspace too —
        // previously only the setup screen accepted one, which made drag-and-drop
        // silently do nothing once results were open.
        self.handle_dropped_files(ui.ctx());

        // The workspace builds its own panel stack directly on the root Ui so
        // its header, filter bar, inspector and centre can each claim their own
        // region and flex to fill the window. The setup screen is a single
        // centred surface, so it gets a plain central panel.
        match self.current_view {
            View::Setup => {
                egui::CentralPanel::default().show(ui, |ui| {
                    self.show_setup(ui);
                });
            }
            View::Workspace => self.show_workspace(ui),
        }

        // The palette draws above everything and is handled last so it can
        // overlay whichever view is active.
        self.show_command_palette(ui.ctx());

        // Notifications float above the layout so appearing or dismissing one
        // never reflows the content underneath.
        self.show_toasts(ui.ctx());
    }

    /// Handle the palette shortcut, render it, and apply the chosen action.
    fn show_command_palette(&mut self, ctx: &egui::Context) {
        // Cmd+K on macOS, Ctrl+K elsewhere — the near-universal binding for this
        // pattern. Consumed so the shortcut cannot also reach a focused widget.
        let toggle = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::K,
            ))
        });
        if toggle {
            self.command_palette.toggle();
        }

        let results = self.results.clone();
        let Some(action) = self
            .command_palette
            .show(ctx, &self.tokens, results.as_deref())
        else {
            return;
        };

        match action {
            PaletteAction::GoToPosition(position) => {
                if let Some(results) = results {
                    self.set_selected_position(Some(position), &results);
                    // Bring it into view if the chart is zoomed elsewhere.
                    self.center_viewport_on(position);
                }
            }
            PaletteAction::Export(format) => {
                if let Some(results) = results {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(format.label(), &[format.extension()])
                        .set_file_name(format!(
                            "{}.{}",
                            sanitize_file_stem(&results.query_name),
                            format.extension()
                        ))
                        .save_file()
                    {
                        self.start_export(format, path, &results);
                    }
                }
            }
            PaletteAction::ResetFilters => {
                if let Some(results) = results {
                    self.filter_state = FilterState::default_for(&results);
                    self.filter_dirty = true;
                }
            }
            PaletteAction::ResetZoom => self.entropy_viewport = None,
            PaletteAction::ToggleTheme => {
                let next = match self.theme {
                    Theme::Light => egui::Theme::Dark,
                    Theme::Dark => egui::Theme::Light,
                };
                // Drive egui's preference; `logic()` syncs our tokens from it.
                ctx.set_theme(next);
            }
            PaletteAction::OpenFile => {
                if let Some(path) = Self::pick_input_file() {
                    self.handle_file_selected(path);
                }
            }
            PaletteAction::BackToSetup => self.current_view = View::Setup,
        }
    }

    /// Re-centre the chart viewport on `position`, preserving the zoom span.
    ///
    /// Only meaningful while zoomed: at full range the position is already shown.
    fn center_viewport_on(&mut self, position: usize) {
        let Some((lo, hi)) = self.entropy_viewport else {
            return;
        };
        let Some(results) = self.results.as_ref() else {
            return;
        };
        let (Some(first), Some(last)) = (results.results.first(), results.results.last()) else {
            return;
        };

        let span = hi - lo;
        let data_min = first.position as f64;
        let data_max = last.position as f64;
        let mut new_lo = position as f64 - span / 2.0;
        new_lo = new_lo.clamp(data_min, (data_max - span).max(data_min));
        self.entropy_viewport = Some((new_lo, new_lo + span));
    }

    /// Open the platform file picker for an input file.
    ///
    /// Shared by the Browse button and the palette so both offer the same
    /// filters.
    fn pick_input_file() -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(
                "FASTA files",
                &[
                    "fasta", "fa", "fna", "ffn", "faa", "frn", "gz", "bz2", "xz", "zst",
                ],
            )
            .add_filter("DiMA binary", &["dima"])
            .add_filter("All files", &["*"])
            .pick_file()
    }

    /// Accept a file dropped anywhere on the window.
    ///
    /// egui 0.36 models a dropped file as a trait object whose `path()` is always
    /// present on native targets, so there is no `Option` to unwrap.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        let Some(file) = dropped.first() else {
            return;
        };

        if dropped.len() > 1 {
            // Be explicit rather than silently ignoring the rest.
            self.error_state.push(ErrorMessage::warning(format!(
                "Only one file can be opened at a time \u{2014} using {}",
                file.path().display()
            )));
        }

        let path = file.path();
        // Re-dropping the file already loaded would needlessly re-validate it.
        if self.selected_file.as_deref() != Some(path) {
            self.handle_file_selected(path.to_path_buf());
        }
    }

    /// Centred message used when the chart has nothing meaningful to draw.
    ///
    /// Drawn into the space already allocated for the chart so the surrounding
    /// layout does not jump between the populated and empty states.
    fn chart_placeholder(ui: &egui::Ui, rect: egui::Rect, tokens: &DesignTokens, message: &str) {
        ui.painter_at(rect).text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(tokens.font_size_body),
            tokens.text_muted,
        );
    }

    /// Zoom the entropy chart about a normalised cursor position.
    ///
    /// Anchoring on the cursor (rather than the centre) keeps the point under
    /// the pointer fixed, which is what makes scroll-zoom feel direct. Snaps back
    /// to "show everything" once the span approaches the full range, so the user
    /// cannot get stuck almost-but-not-quite zoomed out.
    #[allow(clippy::too_many_arguments)]
    fn zoom_entropy_chart(
        &mut self,
        zoom_factor: f32,
        cursor_nx: f32,
        view_min_x: f32,
        view_range: f32,
        data_min_x: f32,
        data_max_x: f32,
    ) {
        let full_span = data_max_x - data_min_x;
        if full_span <= 0.0 {
            return;
        }

        let cursor_x = view_min_x + cursor_nx * view_range;
        let new_span = (view_range * zoom_factor)
            .min(full_span)
            .max(MIN_ENTROPY_ZOOM_SPAN.min(full_span));

        let mut new_min = cursor_x - cursor_nx * new_span;
        new_min = new_min.clamp(data_min_x, (data_max_x - new_span).max(data_min_x));
        let new_max = new_min + new_span;

        if new_span >= full_span * 0.99 {
            self.entropy_viewport = None;
        } else {
            self.entropy_viewport = Some((new_min as f64, new_max as f64));
        }
    }

    /// Zoom about the centre of the current view (used by the toolbar buttons).
    ///
    /// Needs the data range, which only the chart knows, so it recovers it from
    /// the filtered positions rather than duplicating that state.
    fn zoom_entropy_chart_centered(&mut self, zoom_factor: f32) {
        let Some(results) = self.results.clone() else {
            return;
        };
        let Some(&first) = self.filtered_positions.first() else {
            return;
        };
        let Some(&last) = self.filtered_positions.last() else {
            return;
        };

        let data_min_x = results.results[first].position as f32;
        let data_max_x = results.results[last].position as f32;
        let (view_min_x, view_max_x) = self
            .entropy_viewport
            .map(|(a, b)| (a as f32, b as f32))
            .unwrap_or((data_min_x, data_max_x));
        let view_range = (view_max_x - view_min_x).max(1.0);

        self.zoom_entropy_chart(
            zoom_factor,
            0.5,
            view_min_x,
            view_range,
            data_min_x,
            data_max_x,
        );
    }

    // ── Background job polling ───────────────────────────────────────────
    //
    // Each poller follows the same shape: take the poll result (which owns its
    // value, so no borrow of `self` is held), then mutate state. Every arm —
    // including `Disconnected` — clears the handle, so a job can never wedge the
    // UI in a permanently "busy" state.

    /// Whether any background job is currently running.
    pub fn is_busy(&self) -> bool {
        self.validation_job.is_some()
            || self.analysis_job.is_some()
            || self.import_job.is_some()
            || self.export_job.is_some()
    }

    fn poll_validation(&mut self) {
        let outcome = match self.validation_job.as_ref() {
            Some(job) => job.poll(),
            None => return,
        };

        match outcome {
            Poll::Pending => {}
            Poll::Ready(Ok(result)) => {
                self.validation_job = None;
                self.apply_validation_result(result);
            }
            Poll::Ready(Err(e)) => {
                self.validation_job = None;
                // Clear the selection as well: keeping `selected_file` set while
                // an older file's validation result lingered would leave Analyze
                // enabled for a file we could not even read.
                self.validation_result = None;
                self.selected_file = None;
                self.error_state
                    .push(ErrorMessage::error(format!("Validation failed: {e}")));
            }
            Poll::Disconnected => {
                self.validation_job = None;
                self.error_state.push(ErrorMessage::error(
                    "Validation stopped unexpectedly. Please try the file again.".to_string(),
                ));
            }
        }
    }

    /// Adopt a successful validation result, auto-populating configuration.
    fn apply_validation_result(&mut self, result: dima_lib::FastaValidationResult) {
        // Record the file only once we know it is readable, and with its real
        // sequence count and detected alphabet.
        if let Some(ref path) = self.selected_file {
            let alphabet_name = result.detected_alphabet.as_ref().map(|a| format!("{a}"));
            self.recent_files
                .add(path.clone(), Some(result.sequence_count), alphabet_name);
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
            self.analysis_config.header_format = Some(
                fmt.format_string
                    .split(fmt.delimiter)
                    .map(|s| s.to_string())
                    .collect(),
            );
        }

        self.validation_result = Some(result);
    }

    fn poll_analysis(&mut self) {
        let outcome = match self.analysis_job.as_ref() {
            Some(job) => job.worker.poll(),
            None => return,
        };

        match outcome {
            Poll::Pending => {}
            Poll::Ready(result) => {
                self.analysis_job = None;
                match result {
                    AnalysisOutcome::Success {
                        results,
                        validation_stats,
                        perf_report,
                    } => {
                        self.validation_stats = validation_stats;
                        self.perf_report = Some(perf_report);
                        self.apply_results(*results);
                    }
                    AnalysisOutcome::Error(msg) => {
                        self.error_state.push(ErrorMessage::error(msg));
                    }
                    // Cancelled: stay on Setup with the configuration intact.
                    AnalysisOutcome::Cancelled => {}
                }
            }
            Poll::Disconnected => {
                self.analysis_job = None;
                self.error_state.push(ErrorMessage::error(
                    "Analysis stopped unexpectedly. Check the logs for details.".to_string(),
                ));
            }
        }
    }

    fn poll_import(&mut self) {
        let outcome = match self.import_job.as_ref() {
            Some(job) => job.poll(),
            None => return,
        };

        match outcome {
            Poll::Pending => {}
            Poll::Ready(Ok(results)) => {
                self.import_job = None;
                let path = self.import_path.take();

                // Record the file only after a successful decode, so a corrupt
                // `.dima` does not linger in recent files and fail again later.
                if let Some(ref p) = path {
                    self.recent_files
                        .add(p.clone(), Some(results.sequence_count), None);
                }

                // An imported result set carries no validation or perf telemetry.
                self.validation_stats = None;
                self.perf_report = None;
                self.apply_results(*results);

                if let Some(p) = path {
                    self.error_state.push(ErrorMessage::success(format!(
                        "Loaded analysis from {}",
                        p.display()
                    )));
                }
            }
            Poll::Ready(Err(msg)) => {
                self.import_job = None;
                self.import_path = None;
                self.error_state.push(ErrorMessage::error(msg));
            }
            Poll::Disconnected => {
                self.import_job = None;
                self.import_path = None;
                self.error_state.push(ErrorMessage::error(
                    "Import stopped unexpectedly. The file may be corrupt.".to_string(),
                ));
            }
        }
    }

    /// Apply pending filter changes once the user stops interacting.
    ///
    /// Deferring until the pointer is released debounces drags: re-filtering per
    /// tick also invalidated the chart's GPU buffer every frame. A click releases
    /// the pointer immediately, so simple toggles still feel instant.
    fn apply_pending_filters(&mut self, ctx: &egui::Context) {
        if !self.filter_dirty || ctx.egui_is_using_pointer() {
            return;
        }
        let Some(results) = self.results.clone() else {
            self.filter_dirty = false;
            return;
        };

        self.filter_dirty = false;
        self.filtered_positions = self.filter_state.apply(&results);
        self.position_explorer_sort = None;
        self.data_version += 1;
        self.charts_need_data_upload = true;

        // Keep the selection meaningful: if the selected position was filtered
        // out, the detail panels would sit empty. Fall back to the first visible
        // position instead (and clear when nothing matches at all).
        let still_visible = self.selected_position.is_some_and(|pos| {
            self.position_index_map
                .get(&pos)
                .is_some_and(|idx| self.filtered_positions.contains(idx))
        });
        if !still_visible {
            self.selected_position = None;
            self.select_first_filtered_position();
        }
    }

    fn poll_export(&mut self) {
        let outcome = match self.export_job.as_ref() {
            Some(job) => job.poll(),
            None => return,
        };

        match outcome {
            Poll::Pending => {}
            Poll::Ready(Ok(path)) => {
                self.export_job = None;
                self.error_state.push(ErrorMessage::success(format!(
                    "Saved to {}",
                    path.display()
                )));
            }
            Poll::Ready(Err(msg)) => {
                self.export_job = None;
                self.error_state.push(ErrorMessage::error(msg));
            }
            Poll::Disconnected => {
                self.export_job = None;
                self.error_state.push(ErrorMessage::error(
                    "Export stopped unexpectedly. Nothing was written.".to_string(),
                ));
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
        }

        // ── Poll background jobs ─────────────────────────────────────────
        // While any job is in flight, request periodic repaints: egui is
        // event-driven, so without this the progress indicator would stall
        // until the next incidental input event.
        if self.is_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.poll_validation();
        self.poll_analysis();
        self.poll_import();
        self.poll_export();

        // Debounced filter application (see the method docs).
        self.apply_pending_filters(ctx);

        // Auto-dismiss expired success/info messages
        self.error_state.tick_auto_dismiss();
    }

    /// UI phase: all rendering. Receives `&mut Ui` (not `&Context`).
    ///
    /// eframe's `ui()` provides a raw `Ui` with no margin or background colour,
    /// so each view wraps its content in panels that paint `panel_fill` from our
    /// theme tokens.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe's `Frame` is unused, so all rendering lives in `render`, which
        // can therefore be driven headlessly by tests.
        self.render(ui);
    }

    /// Persist state on eframe's auto-save timer and at shutdown.
    ///
    /// Window geometry and egui memory (including the theme preference) are
    /// handled by eframe's `persistence` feature. This hook additionally flushes
    /// the recent-files list *synchronously*, so an entry added moments before
    /// exit cannot be lost to an in-flight background write.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.recent_files.save();
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

    /// Notification toasts, anchored to the bottom-right corner.
    ///
    /// Previously these were stacked inline at the top of the window, so every
    /// message pushed the whole UI down and an unbounded run of errors could
    /// displace the content entirely. Floating them in an `Area` keeps the
    /// layout perfectly stable, and the stack is capped so a burst of failures
    /// cannot cover the app.
    fn show_toasts(&mut self, ctx: &egui::Context) {
        /// Most toasts shown at once; older ones are summarised.
        const MAX_VISIBLE: usize = 4;

        // Cheap early-out on the common path, before allocating anything.
        if !self.error_state.has_messages() {
            return;
        }

        let visible: Vec<(usize, crate::error::ErrorMessage)> = self
            .error_state
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.dismissed && !m.is_expired())
            .map(|(i, m)| (i, m.clone()))
            .collect();

        if visible.is_empty() {
            return;
        }

        let hidden = visible.len().saturating_sub(MAX_VISIBLE);
        // Show the most recent messages: during a burst the newest is what the
        // user is waiting on.
        let shown: Vec<(usize, crate::error::ErrorMessage)> =
            visible.into_iter().rev().take(MAX_VISIBLE).collect();

        let mut dismiss: Option<usize> = None;

        egui::Area::new(egui::Id::new("dima_toasts"))
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                egui::vec2(-self.tokens.space_16, -self.tokens.space_16),
            )
            .interactable(true)
            .show(ctx, |ui| {
                // Bottom-up: the first widget added sits nearest the corner, so
                // the newest toast is closest to the cursor's resting place and
                // the overflow summary ends up at the top of the stack.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    for (index, msg) in shown {
                        let accent = match msg.severity {
                            crate::error::ErrorSeverity::Error => self.tokens.error_color,
                            crate::error::ErrorSeverity::Warning => self.tokens.warning_color,
                            crate::error::ErrorSeverity::Success => self.tokens.success_color,
                            crate::error::ErrorSeverity::Info => self.tokens.info_color,
                        };

                        egui::Frame::new()
                            .fill(self.tokens.surface_elevated)
                            .stroke(egui::Stroke::new(1.0, self.tokens.border))
                            .corner_radius(self.tokens.panel_rounding)
                            .inner_margin(self.tokens.space_8)
                            .show(ui, |ui| {
                                ui.set_max_width(360.0);
                                ui.horizontal(|ui| {
                                    // Severity is carried by a colour bar plus the
                                    // message text, not colour alone.
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(
                                            3.0,
                                            ui.text_style_height(&egui::TextStyle::Body),
                                        ),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(rect, 1.0, accent);
                                    ui.add(egui::Label::new(&msg.text).wrap());

                                    // Auto-dismissing toasts fade on their own;
                                    // persistent ones need an explicit control.
                                    if !msg.should_auto_dismiss()
                                        && ui
                                            .small_button("\u{2715}")
                                            .on_hover_text("Dismiss")
                                            .clicked()
                                    {
                                        dismiss = Some(index);
                                    }
                                });
                            });
                        ui.add_space(self.tokens.space_4);
                    }

                    // Added last so it renders at the top of the stack, above the
                    // toasts it is summarising.
                    if hidden > 0 {
                        ui.weak(format!("+{hidden} more"));
                    }
                });
            });

        if let Some(index) = dismiss {
            self.error_state.dismiss(index);
        }
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

                ui.add_space(self.tokens.space_24);

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
        ui.add_space(self.tokens.space_16);

        // ── File selection ──
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Input File");
            ui.horizontal(|ui| {
                if ui
                    .button("Browse...")
                    .on_hover_text(
                        "Open an aligned FASTA (optionally gz/bz2/xz/zst compressed) \
                         or a previously saved .dima results file",
                    )
                    .clicked()
                {
                    // Shared with the command palette so both offer identical filters.
                    if let Some(path) = Self::pick_input_file() {
                        self.handle_file_selected(path);
                    }
                }
                if let Some(ref path) = self.selected_file {
                    ui.add(egui::Label::new(path.display().to_string()).truncate())
                        .on_hover_text(path.display().to_string());
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

            // Validation progress spinner (shown while background thread is running)
            if self.validation_job.is_some() {
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

        ui.add_space(self.tokens.space_8);

        // ── Configuration ──
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Configuration");

            ui.horizontal(|ui| {
                ui.label("Alphabet:")
                    .on_hover_text("Residue alphabet. Auto-detect infers it from the sequences.");
                egui::ComboBox::from_id_salt("alphabet")
                    .selected_text(self.analysis_config.alphabet.display_name())
                    .show_ui(ui, |ui| {
                        for choice in [
                            AlphabetChoice::Auto,
                            AlphabetChoice::Protein,
                            AlphabetChoice::Nucleotide,
                        ] {
                            let label = choice.display_name();
                            ui.selectable_value(&mut self.analysis_config.alphabet, choice, label);
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("K-mer length:").on_hover_text(
                    "Sliding-window size (k). The default of 9 for protein matches \
                     HLA class I binding peptides.",
                );
                // The maximum is alphabet-dependent: exceeding it makes k-mer
                // encoding overflow and silently drop k-mers, which corrupts
                // results rather than failing loudly.
                let max_k = self.analysis_config.alphabet.max_kmer_length();
                ui.add(
                    egui::DragValue::new(&mut self.analysis_config.kmer_length).range(1..=max_k),
                );
                // Re-clamp after an alphabet change lowered the ceiling.
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
                    )
                    .on_hover_text(
                        "Treat lowercase residues as valid (they are upper-cased). \
                         When off, they count as invalid characters.",
                    );

                    // Character-validation policy. Previously this was fixed at
                    // its default with no way to change it, even though the
                    // library and CLI both expose it.
                    ui.horizontal(|ui| {
                        ui.label("Invalid characters:")
                            .on_hover_text("How to handle residues outside the selected alphabet");
                        egui::ComboBox::from_id_salt("validation_mode")
                            .selected_text(validation_mode_label(
                                self.analysis_config.validation_mode,
                            ))
                            .show_ui(ui, |ui| {
                                for mode in [
                                    dima_lib::ValidationMode::Strict,
                                    dima_lib::ValidationMode::Permissive,
                                    dima_lib::ValidationMode::ReportOnly,
                                ] {
                                    ui.selectable_value(
                                        &mut self.analysis_config.validation_mode,
                                        mode,
                                        validation_mode_label(mode),
                                    )
                                    .on_hover_text(validation_mode_help(mode));
                                }
                            });
                    });

                    ui.checkbox(
                        &mut self.analysis_config.report_invalid,
                        "Report invalid characters",
                    )
                    .on_hover_text(
                        "Collect statistics about invalid characters, shown in the \
                         analysis summary. Turning this off is marginally faster.",
                    );

                    ui.horizontal(|ui| {
                        ui.label("HCS threshold (%):").on_hover_text(
                            "Minimum index incidence for a position to count as highly \
                             conserved. Adjustable later without re-running the analysis.",
                        );
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
                                ui.add_space(self.tokens.space_4);
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
                        ui.add_space(self.tokens.space_4);
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

        ui.add_space(self.tokens.space_16);

        // ── Analyze button / inline stepped progress ──
        let can_analyze = self.selected_file.is_some()
            && self
                .validation_result
                .as_ref()
                .is_some_and(|vr| vr.is_valid())
            && self.analysis_job.is_none()
            && self.validation_job.is_none();

        if self.analysis_job.is_some() {
            self.show_analysis_progress(ui);
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

        ui.add_space(self.tokens.space_16);

        // ── Recent files ──
        self.show_recent_files(ui);

        // Symmetric bottom padding to balance the 24px top padding in show_setup
        ui.add_space(self.tokens.space_24);
    }

    /// Inline, determinate, two-phase analysis progress with a cancel control.
    ///
    /// Replaces the Analyze button in place rather than switching screens, so the
    /// user keeps sight of the configuration that is running. Both of the
    /// library's passes are reported with real counts: showing a completed bar
    /// while position building was still running (the previous "Finalizing..."
    /// spinner) misrepresented how much work remained.
    fn show_analysis_progress(&mut self, ui: &mut egui::Ui) {
        let Some(job) = self.analysis_job.as_ref() else {
            return;
        };
        let phase = job.progress.phase();
        let fraction = job.progress.fraction();
        let elapsed = analysis::format_elapsed(job.elapsed());

        ui.group(|ui| {
            ui.set_min_width(ui.available_width());

            let (caption, detail) = match phase {
                AnalysisPhase::Reading => (
                    "Reading and encoding sequences".to_string(),
                    // No percentage exists yet, so elapsed time is the honest
                    // signal that work is progressing.
                    format!("elapsed {elapsed}"),
                ),
                AnalysisPhase::ComputingEntropy { done, total } => (
                    "Step 1 of 2 \u{2014} computing entropy".to_string(),
                    format!("{done} / {total} positions \u{00B7} {elapsed}"),
                ),
                AnalysisPhase::BuildingPositions { done, total } => (
                    "Step 2 of 2 \u{2014} building positions".to_string(),
                    format!("{done} / {total} positions \u{00B7} {elapsed}"),
                ),
                AnalysisPhase::Indeterminate => {
                    ("Analyzing".to_string(), format!("elapsed {elapsed}"))
                }
            };

            ui.horizontal(|ui| {
                ui.strong(&caption);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(&detail);
                });
            });

            match fraction {
                // Determinate: real completion across both counted passes.
                Some(f) => {
                    ui.add(
                        egui::ProgressBar::new(f)
                            .desired_height(6.0)
                            .fill(self.tokens.progress_bar_fill)
                            .corner_radius(ui.visuals().noninteractive().corner_radius),
                    );
                }
                // Not measurable yet. A spinner is used deliberately rather than
                // an "animated" ProgressBar: egui only animates the *filled*
                // portion (zero-width at 0%), and documents that the animation is
                // skipped entirely when a corner radius is set. Either way the bar
                // would sit perfectly still, which is indistinguishable from a
                // hung application — the exact failure this replaces.
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Working \u{2014} this stage reports no percentage");
                    });
                }
            }

            ui.add_space(self.tokens.space_4);
            if ui.button("Cancel").clicked() {
                self.cancel_analysis();
            }
        });
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

            ui.horizontal(|ui| {
                ui.strong("Recent Files");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("Clear all")
                        .on_hover_text("Remove every entry from this list")
                        .clicked()
                    {
                        self.recent_files.clear();
                    }
                });
            });

            // Actions are collected and applied after the loop so the borrow of
            // `recent_files` ends before it is mutated.
            let mut selected_path: Option<PathBuf> = None;
            let mut remove_path: Option<PathBuf> = None;
            let mut missing_path: Option<PathBuf> = None;

            for recent in &self.recent_files.files {
                let file_name = recent
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();

                let relative_time = format_relative_time(now_secs, recent.last_opened_unix_secs);
                let detail = match recent.sequence_count {
                    Some(count) => format!("{count} seq · {relative_time}"),
                    None => relative_time,
                };

                ui.horizontal(|ui| {
                    // Remove control first (right-aligned) so the file name can
                    // take the remaining width without pushing it off-panel.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("\u{2715}")
                            .on_hover_text("Remove from recent files")
                            .clicked()
                        {
                            remove_path = Some(recent.path.clone());
                        }
                        ui.weak(&detail);

                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let label =
                                egui::RichText::new(file_name.as_ref()).color(self.tokens.accent);
                            if ui
                                .add(
                                    egui::Label::new(label)
                                        .sense(egui::Sense::click())
                                        .truncate(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(recent.path.display().to_string())
                                .clicked()
                            {
                                // Check existence only on click: stat-ing every
                                // entry each frame would touch the filesystem
                                // (possibly a network mount) during rendering.
                                if recent.path.exists() {
                                    selected_path = Some(recent.path.clone());
                                } else {
                                    missing_path = Some(recent.path.clone());
                                }
                            }
                        });
                    });
                });
            }

            if let Some(path) = remove_path {
                self.recent_files.remove(&path);
            }
            if let Some(path) = missing_path {
                // Drop it from the list as well: an entry that cannot be opened
                // is noise, and leaving it invites repeated failed clicks.
                self.recent_files.remove(&path);
                self.error_state.push(ErrorMessage::warning(format!(
                    "File no longer exists and was removed from recent files: {}",
                    path.display()
                )));
            }
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
        if let Some(job) = self.validation_job.take() {
            job.cancel();
        }
        self.validation_result = None;

        // Cancel any in-flight ANALYSIS too: selecting a new file supersedes it.
        // Previously only the `.dima` branch did this, so choosing a new FASTA
        // mid-analysis left the old run alive; on completion it overwrote the
        // freshly selected file's state and force-switched to the Workspace view,
        // silently discarding the user's new selection.
        self.cancel_analysis();

        // A pending `.dima` import is superseded for exactly the same reason: on
        // completion it would call `apply_results` and jump to the Workspace,
        // discarding whatever the user has just chosen. Dropping the handle
        // discards its result. (The `.dima` branch below then starts a fresh one.)
        self.import_job = None;
        self.import_path = None;

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

        // Auto-fill the query name from the file stem without clobbering a name
        // the user typed. An auto-filled name always equals the *previous* file's
        // stem, which is what distinguishes it from a custom one.
        let new_stem = file_stem_of(&path);
        let previous_stem = self
            .selected_file
            .as_deref()
            .map(file_stem_of)
            .unwrap_or_default();
        if self.analysis_config.query_name.is_empty()
            || self.analysis_config.query_name == previous_stem
        {
            self.analysis_config.query_name = new_stem;
        }

        self.selected_file = Some(path.clone());

        // Clear every header-derived setting from the previous file; keeping them
        // would silently apply file A's metadata schema to file B.
        self.analysis_config.clear_file_derived();

        // Validation streams the entire file (transparently decompressing
        // gz/bz2/xz/zst). On large inputs that takes seconds to minutes, so it
        // runs off-thread to keep the window responsive.
        self.validation_job = Some(validation::spawn(self.egui_ctx.clone(), path));
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        // Cheap Arc clone: panels take `&Results` while `self` stays mutable.
        let results = match self.results.clone() {
            Some(r) => r,
            None => return,
        };

        // Keyboard navigation runs before any panel so a selection change is
        // reflected in the same frame it is made.
        self.handle_keyboard_navigation(ui, &results);

        // ── Panel stack ──────────────────────────────────────────────────
        // Panels are declared outermost-first and the central panel last, which
        // is what lets every region flex to fill the window with no dead space:
        // the header and filter bar take only the height they need, the
        // inspector takes a fixed width, and the centre absorbs everything else.
        egui::Panel::top("workspace_header").show(ui, |ui| {
            self.show_workspace_header(ui, &results);
        });

        egui::Panel::top("workspace_filter_bar").show(ui, |ui| {
            self.show_filter_bar(ui, &results);
        });

        // Fixed-width details dock. Always present and always populated (a
        // position is auto-selected on load), so it never reads as wasted space.
        egui::Panel::right("workspace_inspector")
            .resizable(false)
            .exact_size(INSPECTOR_WIDTH)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("inspector_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.show_inspector(ui, &results);
                    });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            self.show_workspace_center(ui, &results);
        });
    }

    /// Workspace header: identity, dataset summary, and global actions.
    fn show_workspace_header(&mut self, ui: &mut egui::Ui, results: &Arc<Results>) {
        ui.add_space(self.tokens.space_4);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&results.query_name)
                    .size(self.tokens.font_size_title)
                    .family(egui::FontFamily::Name(crate::theme::fonts::SEMIBOLD.into())),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.show_theme_toggle(ui);
                self.show_export_buttons(ui, results);
                if ui
                    .button("\u{25C0} Setup")
                    .on_hover_text("Back to file selection and parameters")
                    .clicked()
                {
                    self.current_view = View::Setup;
                }
            });
        });

        // Dataset facts as compact chips. `horizontal_wrapped` keeps them
        // readable when the window is narrow instead of clipping them.
        ui.horizontal_wrapped(|ui| {
            let sep = |ui: &mut egui::Ui| {
                ui.add_space(self.tokens.space_8);
                ui.weak("\u{00B7}");
                ui.add_space(self.tokens.space_8);
            };
            ui.weak(format!("{} sequences", results.sequence_count));
            sep(ui);
            ui.weak(format!("{} positions", results.results.len()));
            sep(ui);
            ui.weak(format!("k = {}", results.kmer_length));
            sep(ui);
            ui.weak(format!("support \u{2265} {}", results.support_threshold));
            sep(ui);
            ui.weak(format!("mean H = {:.4}", results.average_entropy));
        });
        ui.add_space(self.tokens.space_4);
    }

    /// Central region: the entropy overview, conservation strip, and the
    /// position table, stacked so the table absorbs all remaining height.
    fn show_workspace_center(&mut self, ui: &mut egui::Ui, results: &Arc<Results>) {
        self.show_entropy_chart(ui, results);
        ui.add_space(self.tokens.space_8);
        self.show_hcs_section(ui, results);
        ui.add_space(self.tokens.space_8);
        // Rendered last and given the rest of the height: the table is the
        // densest element, so any leftover space is most useful here.
        self.show_position_explorer(ui, results);
    }

    /// Right-hand inspector: everything about the selected position, then the
    /// variant selected within it.
    ///
    /// Ordered so the scope of each section is unambiguous: the summary and the
    /// variants table both describe the *position*, and everything from the
    /// Metadata header down describes the *selected variant*. The motif
    /// composition used to sit below the table, under a "Variant Distribution"
    /// heading, even though it is computed from the whole position — so it read
    /// as per-variant data that never changed when the variant did.
    ///
    /// The sections below the summary are skipped entirely when no position is
    /// selected, rather than each printing its own placeholder.
    fn show_inspector(&mut self, ui: &mut egui::Ui, results: &Arc<Results>) {
        self.show_position_summary(ui, results);
        if self.selected_position_data(results).is_none() {
            return;
        }

        ui.add_space(self.tokens.space_12);
        ui.separator();
        ui.add_space(self.tokens.space_8);
        self.show_variant_table(ui, results);

        // Only when the dataset carries metadata at all: with none, the header
        // and its placeholder would be permanent dead space.
        if !self.available_metadata_fields.is_empty() {
            ui.add_space(self.tokens.space_12);
            ui.separator();
            ui.add_space(self.tokens.space_8);
            self.show_metadata_section(ui, results);
        }
    }

    /// Handle keyboard arrow/Home/End navigation on filtered positions.
    /// Navigates between FILTERED positions but selects from UNFILTERED data.
    fn handle_keyboard_navigation(&mut self, ui: &egui::Ui, results: &Results) {
        // Never steal keys from a focused text field or a DragValue in text-edit
        // mode. Without this guard, Arrow/Home/End are consumed for position
        // navigation while the user is editing a filter or threshold value,
        // making those controls impossible to edit by keyboard.
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }

        // The command palette owns the arrow keys while it is open (they move
        // its result highlight); navigating positions at the same time would
        // move the selection behind the overlay.
        if self.command_palette.is_open() {
            return;
        }

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
            self.set_selected_position(Some(new_pos), results);
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
                Self::section_header(ui, "Entropy");

                // Visible zoom controls. The gestures are the primary path, but
                // an explicit control makes zooming discoverable rather than
                // something the user has to guess at (genome browsers ship both).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let zoomed = self.entropy_viewport.is_some();
                    if ui
                        .add_enabled(zoomed, egui::Button::new("Fit").small())
                        .on_hover_text("Show all positions (or double-click the chart)")
                        .clicked()
                    {
                        self.entropy_viewport = None;
                    }
                    if ui
                        .small_button("+")
                        .on_hover_text("Zoom in (or scroll over the chart)")
                        .clicked()
                    {
                        self.zoom_entropy_chart_centered(0.8);
                    }
                    if ui
                        .add_enabled(zoomed, egui::Button::new("\u{2212}").small())
                        .on_hover_text("Zoom out")
                        .clicked()
                    {
                        self.zoom_entropy_chart_centered(1.25);
                    }
                    ui.weak("drag to pan \u{00B7} shift-drag to zoom to region");
                });
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

            // Any early return below leaves the chart un-hovered; clear now so a
            // stale highlight cannot linger in the position table.
            self.hovered_position = None;

            if self.filtered_positions.is_empty() {
                // Say why the chart is blank instead of leaving an empty frame.
                Self::chart_placeholder(
                    ui,
                    full_rect,
                    &self.tokens,
                    "No positions match the current filters",
                );
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

            // A flat, all-zero landscape has no vertical range to plot. That is a
            // meaningful scientific result (every position fully conserved), so
            // it is reported rather than shown as an empty frame.
            let max_e = full_data.iter().map(|(_, e)| *e).reduce(f32::max);
            let Some(max_e) = max_e.filter(|m| *m > 0.0) else {
                Self::chart_placeholder(
                    ui,
                    full_rect,
                    &self.tokens,
                    "Entropy is zero at every shown position \u{2014} fully conserved",
                );
                return;
            };

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

            // Soft fill under the curve, drawn before the line so the stroke
            // stays crisp on top. The filled area is what makes the chart read as
            // a conservation "landscape" at a glance: troughs are conserved
            // regions (candidate targets), peaks are diverse ones. Rendered on
            // the CPU in both paths because it is a cheap, low-vertex shape.
            if render_data.len() >= 2 {
                let fill_painter = ui.painter_at(rect);
                let fill_color = self.tokens.accent.gamma_multiply(0.18);

                // Built as ONE mesh rather than a quad per segment. Separate
                // translucent shapes are each antialiased, so neighbouring quads
                // blend twice along every shared edge and the fill acquires
                // hundreds of faint vertical seams. A single triangle strip
                // shares those vertices, giving an exactly uniform tint in one
                // draw call.
                let mut mesh = egui::Mesh::default();
                for &(x, e) in &render_data {
                    let sx = rect.left() + ((x - view_min_x) / view_range) * rect.width();
                    let sy = rect.bottom() - (e / max_e) * rect.height();
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(sx, sy),
                        uv: egui::epaint::WHITE_UV,
                        color: fill_color,
                    });
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(sx, rect.bottom()),
                        uv: egui::epaint::WHITE_UV,
                        color: fill_color,
                    });
                }
                for i in 0..render_data.len().saturating_sub(1) {
                    let top = (i * 2) as u32;
                    let bottom = top + 1;
                    let next_top = top + 2;
                    let next_bottom = top + 3;
                    mesh.indices.extend_from_slice(&[
                        top,
                        bottom,
                        next_top,
                        bottom,
                        next_bottom,
                        next_top,
                    ]);
                }
                fill_painter.add(egui::Shape::mesh(mesh));
            }

            // Draw entropy line
            if self.gpu_available {
                let vertices: Vec<EntropyVertex> = render_data
                    .iter()
                    .map(|&(x, y)| EntropyVertex { x, y })
                    .collect();

                // Re-upload whenever the data changed OR the viewport moved.
                // `render_data` is re-downsampled for the *visible* range each
                // frame, so zooming in yields finer detail — but only if it is
                // actually uploaded. Uploading solely on data changes left the
                // GPU magnifying the original whole-range downsample, so zooming
                // revealed no extra detail and the GPU path silently disagreed
                // with the CPU fallback (which re-maps every frame).
                let viewport_changed = self.last_uploaded_viewport != self.entropy_viewport;
                let new_vertices = if self.charts_need_data_upload || viewport_changed {
                    self.last_uploaded_viewport = self.entropy_viewport;
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
                        // Theme-driven so the line is correct in both themes and
                        // matches the CPU fallback below.
                        color: self.tokens.accent.to_normalized_gamma_f32(),
                    },
                    new_data_version: self.data_version,
                    new_vertices,
                };

                ui.painter()
                    .add(egui_wgpu::Callback::new_paint_callback(rect, callback));
                self.charts_need_data_upload = false;
            } else {
                // Clip to the data area: `painter` covers `full_rect` (which
                // includes the axis gutters), and `visible_data` deliberately
                // carries a 5% margin beyond the viewport so the line reaches the
                // edges. Without this clip those margin segments paint over the
                // axis labels.
                let data_painter = ui.painter_at(rect);
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
                    data_painter.line_segment(
                        [window[0], window[1]],
                        egui::Stroke::new(1.5_f32, self.tokens.accent),
                    );
                }
            }

            // Low-support markers along the baseline.
            //
            // NS/LS positions still carry an entropy value, but it is unreliable
            // (computed from too few sequences), so the chart must say so rather
            // than presenting those points as equivalent. Marks sit on the axis to
            // avoid competing with the curve, and are drawn only when the visible
            // range is small enough for them to be distinguishable.
            {
                let marker_painter = ui.painter_at(rect);
                let visible_positions = view_range.max(1.0);
                if visible_positions <= 2000.0 {
                    for &idx in &self.filtered_positions {
                        let pos = &results.results[idx];
                        let Some(tag) = pos.low_support.as_deref() else {
                            continue;
                        };
                        // ELS is scientifically valid; only NS/LS are unreliable.
                        if tag != "NS" && tag != "LS" {
                            continue;
                        }
                        let nx = (pos.position as f32 - view_min_x) / view_range;
                        if !(0.0..=1.0).contains(&nx) {
                            continue;
                        }
                        let x = rect.left() + nx * rect.width();
                        marker_painter.line_segment(
                            [
                                egui::pos2(x, rect.bottom()),
                                egui::pos2(x, rect.bottom() - 4.0),
                            ],
                            egui::Stroke::new(1.0, self.tokens.warning_color),
                        );
                    }
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

            // Click-to-select. The `rect.width() > 0.0` guard avoids a division by
            // zero (NaN/inf into the nearest-point search) on a degenerate rect,
            // which a collapsed or zero-width layout can produce. Clamping maps a
            // click in the axis gutter to the nearest in-range position instead of
            // extrapolating outside the data range.
            if response.clicked() && rect.width() > 0.0 {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let click_nx = ((pointer_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                    let click_x = view_min_x + click_nx * view_range;
                    let nearest = full_data
                        .iter()
                        .min_by(|(ax, _), (bx, _)| {
                            let da = (ax - click_x).abs();
                            let db = (bx - click_x).abs();
                            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(x, _)| *x as usize);
                    self.set_selected_position(nearest, results);
                }
            }

            // ── Navigation gestures ──────────────────────────────────
            // Genome browsers (IGV, JBrowse, UCSC) and charting grammars (Vega)
            // converge on the same model for a 1-D position track, so that is
            // what this implements:
            //   drag        pan along positions
            //   scroll      zoom about the cursor
            //   shift+drag  zoom to a dragged region
            //   click       select a position
            //   dbl-click   reset to the full range
            // Scroll needs no modifier because the chart no longer lives inside a
            // scrolling container, so there is nothing for it to conflict with.
            let scroll_delta = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            let shift_held = ui.ctx().input(|i| i.modifiers.shift);

            if response.hovered() && scroll_delta.abs() > 0.1 && rect.width() > 0.0 {
                let zoom_factor = if scroll_delta > 0.0 { 0.85 } else { 1.18 };
                let mouse_nx = ui
                    .ctx()
                    .pointer_latest_pos()
                    .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
                    .unwrap_or(0.5);
                self.zoom_entropy_chart(
                    zoom_factor,
                    mouse_nx,
                    view_min_x,
                    view_range,
                    data_min_x,
                    data_max_x,
                );
            }

            // Shift+drag selects a region to zoom into.
            //
            // The drag ORIGIN must be remembered explicitly: on the frame the
            // drag ends, egui reports only the release position, so deriving the
            // origin from the response would collapse the band to a single point
            // and the zoom would never fire.
            if shift_held && response.drag_started() {
                self.chart_drag_origin_x = response.interact_pointer_pos().map(|p| p.x);
            }

            if let Some(origin_x) = self.chart_drag_origin_x {
                let current_x = ui
                    .ctx()
                    .pointer_latest_pos()
                    .map(|p| p.x)
                    .unwrap_or(origin_x);

                if response.dragged() {
                    // Live preview of the region being selected.
                    let band = egui::Rect::from_x_y_ranges(
                        origin_x.min(current_x)..=origin_x.max(current_x),
                        rect.y_range(),
                    );
                    painter.rect_filled(band, 0.0, self.tokens.selection_highlight);
                } else {
                    // Drag finished (or was interrupted): commit and clear.
                    self.chart_drag_origin_x = None;
                    if rect.width() > 0.0 {
                        let to_data = |x: f32| {
                            view_min_x
                                + ((x - rect.left()) / rect.width()).clamp(0.0, 1.0) * view_range
                        };
                        let (a, b) = (to_data(origin_x), to_data(current_x));
                        let (lo, hi) = (a.min(b), a.max(b));
                        // Ignore an accidental micro-drag, which would otherwise
                        // zoom absurdly far into a near-zero span.
                        if hi - lo >= MIN_ENTROPY_ZOOM_SPAN {
                            self.entropy_viewport = Some((lo as f64, hi as f64));
                        }
                    }
                }
            } else if response.dragged() {
                // Plain drag pans. Has no effect at full zoom-out, where there is
                // nothing to pan.
                let pan_amount = -(response.drag_delta().x / rect.width().max(1.0)) * view_range;
                if let Some((vmin, vmax)) = self.entropy_viewport {
                    let span = (vmax - vmin) as f32;
                    let mut new_min = vmin as f32 + pan_amount;
                    // Clamp within the data range while preserving the span.
                    new_min = new_min.clamp(data_min_x, (data_max_x - span).max(data_min_x));
                    self.entropy_viewport = Some((new_min as f64, (new_min + span) as f64));
                }
            }

            // Double-click resets to the full range.
            if response.double_clicked() {
                self.entropy_viewport = None;
            }

            // Resolve the hovered point once, before the tooltip closure, so the
            // same value can both fill the tooltip and drive the cross-panel
            // highlight (the closure borrows `ui`, so `self` cannot be mutated
            // inside it).
            let hovered = if response.hovered() && rect.width() > 0.0 {
                ui.ctx().pointer_latest_pos().and_then(|pointer_pos| {
                    let hover_nx = ((pointer_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                    let hover_x = view_min_x + hover_nx * view_range;
                    full_data
                        .iter()
                        .min_by(|(ax, _), (bx, _)| {
                            let da = (ax - hover_x).abs();
                            let db = (bx - hover_x).abs();
                            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .copied()
                })
            } else {
                None
            };

            // Publish the hovered position so the position table can highlight
            // the matching row. Cleared when the pointer leaves, so a stale
            // highlight never lingers.
            self.hovered_position = hovered.map(|(px, _)| px as usize);

            // Marker on the curve at the hovered point, so the tooltip is clearly
            // attached to a specific position rather than floating near the line.
            if let Some((px, py)) = hovered {
                let nx = (px - view_min_x) / view_range;
                if (0.0..=1.0).contains(&nx) {
                    let marker_painter = ui.painter_at(rect);
                    let x = rect.left() + nx * rect.width();
                    let y = rect.bottom() - (py / max_e) * rect.height();
                    marker_painter.circle_filled(
                        egui::pos2(x, y),
                        3.0,
                        self.tokens.chart_selection_line,
                    );
                }
            }

            if let Some((px, py)) = hovered {
                response.on_hover_ui_at_pointer(|ui| {
                    ui.label(format!("Position: {}", px as usize));
                    ui.label(format!("Entropy: {:.4}", py));
                });
            }
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

            // Coordinate mapping is driven by the actual position RANGE, not by
            // the number of positions. Position numbers are 1-based and need not
            // be contiguous (`compute_hcs_regions` sorts defensively because
            // deserialized results may be sparse or offset). Dividing by the
            // count assumed positions ran 1..=len, which placed every region
            // wrongly — and could push `end/len` past 1.0 — whenever they did not.
            let first_position = results.results.first().map(|p| p.position).unwrap_or(1);
            let last_position = results.results.last().map(|p| p.position).unwrap_or(1);
            // Span is inclusive of the last position, and never zero.
            let total_span = ((last_position.saturating_sub(first_position)) + 1).max(1) as f32;
            let painter = ui.painter_at(rect);

            for region in &self.hcs_regions {
                // Offset by the first position so the bar starts at the data's
                // true left edge, then normalise over the inclusive span.
                let start_offset = region.start_position.saturating_sub(first_position);
                let end_offset = region.end_position.saturating_sub(first_position);
                let left_frac = (start_offset as f32 / total_span).clamp(0.0, 1.0);
                let right_frac = (((end_offset + 1) as f32) / total_span).clamp(0.0, 1.0);
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

                    // Truncate sequences for compact tooltip display.
                    // Character-based (never byte-based): HCS sequences are stitched
                    // from imported data whose encoding is not guaranteed ASCII.
                    let seq_display = truncate_display(&region.sequence, 30);

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

    /// Export menu covering every supported output format.
    ///
    /// All exports run on a background worker: the binary writer and the chart
    /// rasteriser both take long enough on real datasets that doing them inline
    /// visibly froze the window. A single menu (rather than one button per
    /// format) keeps the header uncluttered as formats are added.
    fn show_export_buttons(&mut self, ui: &mut egui::Ui, results: &Arc<Results>) {
        let busy = self.export_job.is_some();

        ui.add_enabled_ui(!busy, |ui| {
            ui.menu_button("Export \u{25BE}", |ui| {
                for format in ExportFormat::all() {
                    let response = ui
                        .button(format.label())
                        .on_hover_text(format.description());
                    if response.clicked() {
                        ui.close();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(format.label(), &[format.extension()])
                            .set_file_name(format!(
                                "{}.{}",
                                sanitize_file_stem(&results.query_name),
                                format.extension()
                            ))
                            .save_file()
                        {
                            self.start_export(format, path, results);
                        }
                    }
                }
            })
            .response
            .on_hover_text(if busy {
                "An export is already running..."
            } else {
                "Save results or the entropy figure to a file"
            });
        });

        if busy {
            ui.spinner();
        }
    }

    /// Per-position facts: identity, entropy, support, and motif composition.
    ///
    /// Everything rendered here describes the *position*, which is why the
    /// motif composition sits at the top of the inspector rather than below the
    /// variants table, where it used to read as a property of the selected
    /// variant while never changing with it.
    fn show_position_summary(&self, ui: &mut egui::Ui, results: &Results) {
        let Some(pos) = self.selected_position_data(results) else {
            Self::section_header(ui, "Position Details");
            ui.add_space(self.tokens.space_4);
            ui.colored_label(self.tokens.text_muted, "Click a position to view details");
            return;
        };

        // The number carries the heading, so it is not repeated as a stat row.
        Self::section_header(ui, format!("Position {}", pos.position));

        // Two short facts share one wrapped line: as separate rows they spent
        // three lines of a narrow dock and read as an unrelated list.
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Entropy: {:.4}", pos.entropy))
                .on_hover_text("Shannon entropy of this k-mer position, in bits");
            ui.add_space(self.tokens.space_8);
            ui.weak("\u{00B7}");
            ui.add_space(self.tokens.space_8);
            ui.label(format!("Support: {}", pos.support))
                .on_hover_text("Sequences carrying a complete, unambiguous k-mer here");
        });
        if let Some(ref ls) = pos.low_support {
            ui.colored_label(self.tokens.warning_color, format!("Low support: {}", ls));
        }

        self.show_motif_composition(ui, pos);
    }

    /// Stacked composition bar for the motif classes at this position.
    ///
    /// One 100%-stacked bar rather than a bar per class: every class is a share
    /// of the same whole, and stacking makes those shares directly comparable
    /// while taking a quarter of the height four rows needed. The exact figures
    /// live in the legend, because judging a segment's length is far less
    /// precise than reading a number (Cleveland & McGill, 1984).
    ///
    /// `Total variants` is deliberately absent: it is the non-index share of
    /// support, so it always equals `100% - Index` and the bar already shows it.
    fn show_motif_composition(&self, ui: &mut egui::Ui, pos: &Position) {
        const BAR_HEIGHT: f32 = 18.0;

        let Some(variants) = pos.diversity_motifs.as_ref() else {
            return;
        };
        // Nothing drawable: a zero-width bar would imply an empty position, and
        // the table below already reports what is actually there.
        let Some(shares) = Self::motif_shares(variants) else {
            return;
        };

        ui.add_space(self.tokens.space_12);
        Self::sub_label(ui, "Motif composition");
        ui.add_space(self.tokens.space_4);

        let colors = [
            self.tokens.motif_index,
            self.tokens.motif_major,
            self.tokens.motif_minor,
            self.tokens.motif_unique,
        ];
        let bar_size = egui::vec2(ui.available_width(), BAR_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(bar_size, egui::Sense::hover());

        // Each segment starts where the last ended, so rounding can never open a
        // seam between neighbours. Empty classes are skipped, so a class at 0%
        // cannot claim one of the rounded end corners.
        let mut segments: Vec<(egui::Rect, egui::Color32, &'static str, f64)> =
            Vec::with_capacity(MOTIF_CLASSES.len());
        let mut cumulative = 0.0_f64;
        let mut left = rect.left();
        for (i, &share) in shares.iter().enumerate() {
            cumulative += share;
            let right = rect.left() + (cumulative / 100.0) as f32 * rect.width();
            if right > left {
                segments.push((
                    egui::Rect::from_min_max(
                        egui::pos2(left, rect.top()),
                        egui::pos2(right, rect.bottom()),
                    ),
                    colors[i],
                    MOTIF_CLASSES[i].1,
                    share,
                ));
            }
            left = right;
        }

        // Only the outer corners are rounded, so the classes read as one bar
        // instead of four adjacent pills. `checked_sub` also covers a dock
        // collapsed to zero width, where there is no segment to round.
        if let Some(last) = segments.len().checked_sub(1) {
            let radius = (BAR_HEIGHT * 0.25) as u8;
            for (i, &(segment, color, _, _)) in segments.iter().enumerate() {
                ui.painter().rect_filled(
                    segment,
                    egui::CornerRadius {
                        nw: if i == 0 { radius } else { 0 },
                        sw: if i == 0 { radius } else { 0 },
                        ne: if i == last { radius } else { 0 },
                        se: if i == last { radius } else { 0 },
                    },
                    color,
                );
            }
        }

        // The tooltip is resolved from the pointer instead of allocating four
        // widgets: separate allocations would insert item spacing between the
        // segments and break the bar into pieces.
        let hovered = response
            .hover_pos()
            .and_then(|p| segments.iter().find(|(segment, ..)| segment.contains(p)))
            .map(|(_, _, class, pct)| {
                format!("{class}: {pct:.1}% of classified variants at this position")
            });
        if let Some(text) = hovered {
            response.on_hover_text(text);
        }

        ui.add_space(self.tokens.space_4);
        // Every class is listed, empty ones included: a legend whose entries
        // come and go as the selection moves makes the row width jump, and
        // "Minor 0.0%" is itself a fact worth stating.
        ui.horizontal_wrapped(|ui| {
            for (i, &share) in shares.iter().enumerate() {
                Self::legend_entry(
                    ui,
                    colors[i],
                    format!("{} {share:.1}%", MOTIF_CLASSES[i].1),
                    self.tokens.text_secondary,
                    self.tokens.font_size_caption,
                );
            }
        });

        ui.add_space(self.tokens.space_4);
        ui.label(
            egui::RichText::new(format!(
                "Distinct variants: {} ({:.1}%)",
                pos.distinct_variants_count, pos.distinct_variants_incidence
            ))
            .color(self.tokens.text_muted),
        )
        .on_hover_text(
            "Distinct non-index k-mer types at this position, and their share of all \
             non-index reads \u{2014} the type richness of the minority population \
             (Tharanga et al., PMC11596295)",
        );
    }

    /// Paint one legend entry — colour swatch plus caption — as a single item.
    ///
    /// A single allocation because `horizontal_wrapped` can only break between
    /// allocations: as two items, a swatch could wrap away from the label it
    /// explains. The swatch is painted rather than typed because a glyph such
    /// as `▪` depends on the bundled font's symbol coverage, and a missing
    /// glyph renders as a tofu box.
    fn legend_entry(
        ui: &mut egui::Ui,
        color: egui::Color32,
        text: String,
        text_color: egui::Color32,
        font_size: f32,
    ) {
        const SWATCH: f32 = 9.0;

        let gap = ui.spacing().item_spacing.x * 0.5;
        let galley =
            ui.painter()
                .layout_no_wrap(text, egui::FontId::proportional(font_size), text_color);
        let text_size = galley.size();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(SWATCH + gap + text_size.x, text_size.y.max(SWATCH)),
            egui::Sense::hover(),
        );
        let swatch = egui::Rect::from_center_size(
            egui::pos2(rect.left() + SWATCH * 0.5, rect.center().y),
            egui::Vec2::splat(SWATCH),
        );
        ui.painter().rect_filled(swatch, 2.0_f32, color);
        ui.painter().galley(
            egui::pos2(swatch.right() + gap, rect.center().y - text_size.y * 0.5),
            galley,
            text_color,
        );
    }

    /// Searchable, sortable table of the distinct k-mers at this position, and
    /// the selector for the variant described in the metadata section below.
    fn show_variant_table(&mut self, ui: &mut egui::Ui, results: &Results) {
        let Some(pos) = self.selected_position_data(results) else {
            return;
        };
        let Some(variants) = pos.diversity_motifs.as_ref() else {
            Self::sub_label(ui, "Variants");
            ui.add_space(self.tokens.space_4);
            ui.colored_label(
                self.tokens.text_muted,
                "No variants were recorded at this position",
            );
            return;
        };

        // ── Variant tools ────────────────────────────────────────────────
        // A single position can carry thousands of distinct k-mers (mostly
        // singletons), so the raw table is hard to work with. Search and a
        // minimum-incidence floor make it possible to find a specific sequence
        // or hide singleton noise.
        ui.horizontal(|ui| {
            Self::sub_label(ui, "Variants");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let filtering = !self.variant_search.is_empty() || self.variant_min_incidence > 0.0;
                if filtering
                    && ui
                        .small_button("\u{2715}")
                        .on_hover_text("Clear variant filters")
                        .clicked()
                {
                    self.variant_search.clear();
                    self.variant_min_incidence = 0.0;
                }
            });
        });

        ui.add(
            egui::TextEdit::singleline(&mut self.variant_search)
                .hint_text("Search sequence or motif")
                .desired_width(f32::INFINITY),
        )
        .on_hover_text("Case-insensitive substring match on the k-mer or motif");

        ui.horizontal(|ui| {
            ui.label("Min incidence");
            ui.add(
                egui::DragValue::new(&mut self.variant_min_incidence)
                    .suffix(" %")
                    .speed(0.1)
                    .range(0.0..=100.0),
            )
            .on_hover_text("Hide variants below this incidence (0 shows all)");
        });

        // Filter first, then sort, so the ordering applies to what is actually
        // shown.
        let needle = self.variant_search.trim().to_lowercase();
        let min_incidence = self.variant_min_incidence;
        let mut sorted_indices: Vec<usize> = (0..variants.len())
            .filter(|&i| {
                let v = &variants[i];
                if v.incidence < min_incidence {
                    return false;
                }
                if needle.is_empty() {
                    return true;
                }
                v.sequence.to_lowercase().contains(&needle)
                    || v.motif_short
                        .as_deref()
                        .is_some_and(|m| m.to_lowercase().contains(&needle))
                    || v.motif_long
                        .as_deref()
                        .is_some_and(|m| m.to_lowercase().contains(&needle))
            })
            .collect();

        let hidden = variants.len() - sorted_indices.len();
        if hidden > 0 {
            ui.weak(format!(
                "{} of {} shown ({} hidden)",
                sorted_indices.len(),
                variants.len(),
                hidden
            ));
        } else {
            ui.weak(format!("{} distinct", variants.len()));
        }

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
        // Sequences use the bundled monospace face at the token size so
        // residues align column-wise across rows.
        let mono_size = self.tokens.font_size_mono;
        let current_sort = self.variant_table_sort;

        // Cell accumulators for header and body clicks inside table closures
        let clicked_col = std::cell::Cell::new(None::<usize>);
        let clicked_variant = std::cell::Cell::new(None::<usize>);

        // Compute scroll height before TableBuilder borrows ui mutably. A
        // share of the viewport keeps the table proportionate on any display,
        // and the cap leaves room for the metadata section beneath it.
        let variant_scroll_height =
            (ui.ctx().input(|i| i.viewport_rect().height()) * 0.32).clamp(160.0, 420.0);

        let table = egui_extras::TableBuilder::new(ui)
            .id_salt("variant_details")
            .striped(true)
            .resizable(true)
            // Fill the dock's width, but only take the height the rows need:
            // reserving the full scroll height left a block of empty striping
            // below positions that carry a handful of variants.
            .auto_shrink([false, true])
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
                        ui.label(egui::RichText::new(&v.sequence).monospace().size(mono_size));
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
                    TableSort::new(col, self.variant_table_sort.unwrap().direction.toggle())
                } else {
                    TableSort::new(col, SortDirection::Ascending)
                },
            );
        }

        // Apply variant row click. Only the per-variant metadata cache is
        // invalidated: which fields are open, and which list every value, is
        // view state the user set, and keeping it is what makes comparing one
        // field across variants possible.
        if let Some(vi) = clicked_variant.get() {
            self.selected_variant_index = Some(vi);
            self.cached_metadata = None;
        }
    }

    /// Metadata carried by the selected variant, as an expandable field list.
    ///
    /// Metadata belongs to an individual distinct sequence in the DiMA
    /// methodology (Tharanga et al., PMC11596295), so this section describes
    /// the variant selected in the table above, not the position.
    ///
    /// Fields are listed collapsed with a top-N breakdown on demand: the
    /// pattern field-oriented explorers converge on (Splunk's field sidebar,
    /// Datadog facets, Kibana field statistics) because it keeps a dozen fields
    /// scannable in a narrow dock. Each value gets a name line with a
    /// full-width proportional bar beneath it, which is what removes the nested
    /// scroll areas the previous version needed: a field now grows the single
    /// inspector scroll instead of trapping the wheel in a 200px box.
    fn show_metadata_section(&mut self, ui: &mut egui::Ui, results: &Results) {
        let Some(pos) = self.selected_position_data(results) else {
            return;
        };
        let variants = pos.diversity_motifs.as_ref();
        let selected = self
            .selected_variant_index
            .and_then(|vi| variants?.get(vi).map(|variant| (vi, variant)));

        // Every way of having no variant to describe ends in the same place: a
        // header and one line saying so. Returning before the header would
        // leave the separator `show_inspector` already drew hanging over
        // nothing, which is exactly the dead space this redesign is removing.
        let Some((vi, variant)) = selected else {
            Self::section_header(ui, "Metadata");
            ui.add_space(self.tokens.space_4);
            ui.colored_label(
                self.tokens.text_muted,
                if variants.is_none_or(|v| v.is_empty()) {
                    "No variants at this position"
                } else {
                    "Select a variant above to see its metadata"
                },
            );
            return;
        };

        let motif = variant.motif_short.as_deref().unwrap_or("?");
        // Truncated by characters, not bytes: sequences reach the UI from
        // `.dima` import, which validates the container but not the encoding
        // of the strings inside it. The budget keeps the heading on one line at
        // the dock's fixed width even for a long k-mer.
        let sequence = truncate_display(&variant.sequence, 14);
        Self::section_header(
            ui,
            format!(
                "Metadata ({} \u{2014} {}, {:.1}%)",
                sequence, motif, variant.incidence
            ),
        );
        ui.add_space(self.tokens.space_4);

        // Keyed by position *and* variant, because switching either changes the
        // whole breakdown. Caching also fixes the order: the source is a
        // `HashMap`, so re-extracting per frame would make the rows flicker.
        let cache_key = (pos.position, vi);
        if self
            .cached_metadata
            .as_ref()
            .is_none_or(|(key, _)| *key != cache_key)
        {
            self.cached_metadata = Some((cache_key, Self::extract_variant_metadata(variant)));
        }

        // First metadata render for this dataset: open one field so the section
        // arrives showing data rather than a stack of shut headers. Left
        // uninitialised while there is nothing to open, so the default still
        // applies to the first variant that actually carries metadata.
        if self.metadata_open_fields.is_none() {
            if let Some(first) = self
                .cached_metadata
                .as_ref()
                .and_then(|(_, fields)| fields.first())
                .map(|(name, _)| name.clone())
            {
                self.metadata_open_fields = Some(std::iter::once(first).collect());
            }
        }

        let Some((_, fields)) = self.cached_metadata.as_ref() else {
            return;
        };

        // Clicks are collected and applied after the loop: the field list is
        // borrowed from `self.cached_metadata` while it is drawn, and only one
        // row can be clicked per frame in any case.
        let mut toggle_open: Option<String> = None;
        let mut toggle_show_all: Option<String> = None;
        let mut rendered_any_field = false;

        for (field, values) in fields {
            // Saturating, not `sum()`: these counts are read straight out of a
            // `.dima` file, whose container is checksummed but whose numbers are
            // not range-checked, and a plain sum of near-`usize::MAX` counts
            // panics under debug overflow checks. Saturating instead caps every
            // share at 100%, which is the honest reading of nonsense input.
            //
            // A field whose values all count zero has nothing to apportion, and
            // a bar chart of zeroes says nothing.
            let total = values
                .iter()
                .fold(0_usize, |acc, (_, count)| acc.saturating_add(*count));
            if total == 0 {
                continue;
            }
            rendered_any_field = true;

            let open = self
                .metadata_open_fields
                .as_ref()
                .is_some_and(|open| open.contains(field));
            if self.metadata_field_header(ui, field, values.len(), open) {
                toggle_open = Some(field.clone());
            }
            if !open {
                continue;
            }

            let show_all = self.expanded_metadata_fields.contains(field);
            let shown = Self::visible_metadata_values(values.len(), show_all);
            for (value, count) in values.iter().take(shown) {
                self.metadata_value_row(ui, field, value, *count, total);
            }

            // The cap is stated rather than applied silently, so a truncated
            // list can never be mistaken for a complete one.
            if show_all && shown < values.len() {
                ui.horizontal(|ui| {
                    ui.add_space(METADATA_INDENT);
                    ui.label(
                        egui::RichText::new(format!("+{} more not shown", values.len() - shown))
                            .size(self.tokens.font_size_caption)
                            .color(self.tokens.text_muted),
                    );
                });
            }
            if values.len() > METADATA_TOP_VALUES
                && self.metadata_show_all_toggle(ui, values.len(), show_all)
            {
                toggle_show_all = Some(field.clone());
            }
            ui.add_space(self.tokens.space_4);
        }

        // Covers both a variant with no metadata map and one whose fields hold
        // nothing countable: either way the section says so, rather than
        // trailing off into blank space beneath its own header.
        if !rendered_any_field {
            ui.colored_label(self.tokens.text_muted, "This variant carries no metadata");
        }

        // Both sets are guaranteed initialised here: a toggle can only be
        // clicked on a row that was drawn, which required a non-empty field
        // list. If that ever stops holding, the click is a no-op rather than a
        // panic.
        if let Some(field) = toggle_open {
            if let Some(open) = self.metadata_open_fields.as_mut() {
                if !open.remove(&field) {
                    open.insert(field);
                }
            }
        }
        if let Some(field) = toggle_show_all {
            if !self.expanded_metadata_fields.remove(&field) {
                self.expanded_metadata_fields.insert(field);
            }
        }
    }

    /// One metadata field header: disclosure triangle, name, and value count.
    ///
    /// Returns `true` when the row was clicked. The whole row is the target
    /// rather than the triangle alone — a full-width 22px band is an easy hit,
    /// a 9px glyph is not.
    fn metadata_field_header(
        &self,
        ui: &mut egui::Ui,
        field: &str,
        value_count: usize,
        open: bool,
    ) -> bool {
        const ROW_HEIGHT: f32 = 22.0;
        const TRIANGLE_BOX: f32 = 14.0;

        let row_size = egui::vec2(ui.available_width(), ROW_HEIGHT);
        let (_, rect) = ui.allocate_space(row_size);
        // Identified by field name rather than by position in the list. egui's
        // automatic ids are positional, so opening one field would renumber
        // every row after it — moving keyboard focus to a different field and
        // churning the accessibility tree. Field names are map keys, so they
        // are unique within a variant.
        let response = ui.interact(
            rect,
            ui.make_persistent_id(("metadata_field", field)),
            egui::Sense::click(),
        );
        let font = Self::row_font(ui);
        let painter = ui.painter();

        if response.hovered() {
            painter.rect_filled(rect, 4.0_f32, self.tokens.hover_highlight);
        }
        // `Sense::click` makes the row reachable with Tab and activated with
        // Space or Enter, so keyboard focus has to be visible as well as real.
        if response.has_focus() {
            painter.rect_stroke(
                rect,
                4.0_f32,
                egui::Stroke::new(1.0_f32, self.tokens.accent),
                egui::StrokeKind::Inside,
            );
        }

        let triangle = egui::Rect::from_center_size(
            egui::pos2(rect.left() + TRIANGLE_BOX * 0.5, rect.center().y),
            egui::Vec2::splat(TRIANGLE_BOX),
        );
        Self::paint_disclosure(painter, triangle, open, self.tokens.text_secondary);

        let count = painter.text(
            egui::pos2(rect.right(), rect.center().y),
            egui::Align2::RIGHT_CENTER,
            format!(
                "{} value{}",
                value_count,
                if value_count == 1 { "" } else { "s" }
            ),
            egui::FontId::proportional(self.tokens.font_size_caption),
            self.tokens.text_muted,
        );

        // Elided to whatever the count on the right leaves free, so a long
        // field name can never collide with it.
        let name_left = triangle.right() + 2.0;
        let name = Self::elided_galley(
            ui,
            field,
            font,
            self.tokens.text_primary,
            count.left() - 6.0 - name_left,
        );
        let elided = name.elided;
        painter.galley(
            egui::pos2(name_left, rect.center().y - name.size().y * 0.5),
            name,
            self.tokens.text_primary,
        );

        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        // Only when the name did not fit: a tooltip on every field header would
        // be noise.
        let response = if elided {
            response.on_hover_text(field)
        } else {
            response
        };

        // The row is painted, not built from widgets, so it would otherwise
        // reach assistive technology as an unlabelled box. eframe ships with
        // AccessKit enabled, and the state belongs in the name because egui has
        // no disclosure role to carry it.
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!(
                    "{field}, {value_count} values, {}",
                    if open { "expanded" } else { "collapsed" }
                ),
            )
        });
        response.clicked()
    }

    /// One metadata value: its name and share on one line, with a proportional
    /// bar spanning the full width beneath.
    ///
    /// Stacked rather than laid out as name/bar/percentage columns because the
    /// dock is 340px wide: three columns leave roughly 100px for names like
    /// "Saudi Arabia" and a bar too short to compare against its neighbours.
    /// Stacking gives the name the full width and every bar the same length
    /// baseline, which is how narrow-sidebar field breakdowns are drawn.
    fn metadata_value_row(
        &self,
        ui: &mut egui::Ui,
        field: &str,
        value: &str,
        count: usize,
        total: usize,
    ) {
        const LINE_HEIGHT: f32 = 17.0;
        const BAR_HEIGHT: f32 = 5.0;
        const BAR_GAP: f32 = 2.0;

        // `total` is the sum of every count in the field, so the share is in
        // range by construction; clamping costs nothing and keeps a corrupt
        // import from painting outside the track.
        let share = (count as f64 / total as f64).clamp(0.0, 1.0);

        // One allocation for the whole entry keeps the rows evenly spaced and
        // gives the tooltip a single hit area covering label and bar. Keyed by
        // field and value for the same reason as the header above.
        let entry_size = egui::vec2(ui.available_width(), LINE_HEIGHT + BAR_GAP + BAR_HEIGHT);
        let (_, rect) = ui.allocate_space(entry_size);
        let response = ui.interact(
            rect,
            ui.make_persistent_id(("metadata_value", field, value)),
            egui::Sense::hover(),
        );
        let font = Self::row_font(ui);
        let painter = ui.painter();

        let label_center_y = rect.top() + LINE_HEIGHT * 0.5;
        let percentage = painter.text(
            egui::pos2(rect.right(), label_center_y),
            egui::Align2::RIGHT_CENTER,
            format!("{:.1}% ({})", share * 100.0, count),
            egui::FontId::proportional(self.tokens.font_size_caption),
            self.tokens.text_secondary,
        );

        // Elided to the space the percentage leaves, so a long value can never
        // overlap it. The untruncated value stays available on hover.
        let name_left = rect.left() + METADATA_INDENT;
        let name = Self::elided_galley(
            ui,
            value,
            font,
            self.tokens.text_primary,
            percentage.left() - 6.0 - name_left,
        );
        painter.galley(
            egui::pos2(name_left, label_center_y - name.size().y * 0.5),
            name,
            self.tokens.text_primary,
        );

        let track = egui::Rect::from_min_max(
            egui::pos2(name_left, rect.bottom() - BAR_HEIGHT),
            egui::pos2(rect.right(), rect.bottom()),
        );
        if track.width() > 0.0 {
            painter.rect_filled(track, 2.0_f32, self.tokens.border);
            // A single accent for every value: the length already encodes the
            // magnitude, so per-value colours would add a dimension that means
            // nothing. Floored so a sub-percent share stays a visible mark
            // rather than vanishing into the track.
            let fill = (share as f32 * track.width()).max(2.0).min(track.width());
            painter.rect_filled(
                egui::Rect::from_min_size(track.min, egui::vec2(fill, BAR_HEIGHT)),
                2.0_f32,
                self.tokens.accent,
            );
        }

        // Painted rather than built from widgets, so its name has to be
        // supplied explicitly for assistive technology (see the field header).
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Label,
                true,
                format!("{value}, {count} of {total}"),
            )
        });
        response.on_hover_text(format!("{value} \u{2014} {count} of {total}"));
    }

    /// The "show every value" / "show the top few" toggle for one field.
    ///
    /// Returns `true` when clicked.
    fn metadata_show_all_toggle(
        &self,
        ui: &mut egui::Ui,
        value_count: usize,
        show_all: bool,
    ) -> bool {
        let text = if show_all {
            format!("Show top {METADATA_TOP_VALUES}")
        } else {
            format!("Show all {value_count} values")
        };

        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.add_space(METADATA_INDENT);
            clicked = ui
                .add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .size(self.tokens.font_size_caption)
                            .color(self.tokens.accent),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();
        });
        clicked
    }

    /// Paint a disclosure triangle inside `rect`: pointing right when closed,
    /// down when open.
    ///
    /// Painted rather than typed because a glyph such as `▸` depends on the
    /// bundled font's symbol coverage, and a missing glyph would render as a
    /// tofu box in the one place the user needs a clear affordance.
    fn paint_disclosure(
        painter: &egui::Painter,
        rect: egui::Rect,
        open: bool,
        color: egui::Color32,
    ) {
        let c = rect.center();
        let s = rect.width().min(rect.height()) * 0.3;
        let points = if open {
            vec![
                egui::pos2(c.x - s, c.y - s * 0.6),
                egui::pos2(c.x + s, c.y - s * 0.6),
                egui::pos2(c.x, c.y + s * 0.8),
            ]
        } else {
            vec![
                egui::pos2(c.x - s * 0.6, c.y - s),
                egui::pos2(c.x - s * 0.6, c.y + s),
                egui::pos2(c.x + s * 0.8, c.y),
            ]
        };
        painter.add(egui::Shape::convex_polygon(
            points,
            color,
            egui::Stroke::NONE,
        ));
    }

    /// Horizontal filter bar shown directly beneath the header.
    ///
    /// These filters scope every view below them, and the established guidance
    /// for whole-view filters is that they must stay permanently visible —
    /// hidden filters make users misread the data. A horizontal bar is the
    /// conventional home for a handful of such filters over a table: it keeps
    /// them and their active state in one place while leaving the full window
    /// width for the chart and table (a tall sidebar for five controls would
    /// itself be mostly empty).
    fn show_filter_bar(&mut self, ui: &mut egui::Ui, results: &Results) {
        // Bounds come from the unfiltered data so the user can always widen a
        // range back out again. O(n) over positions, only while the bar is drawn.
        let last_position = results.results.last().map(|p| p.position).unwrap_or(1);
        let max_entropy = results
            .results
            .iter()
            .map(|p| p.entropy)
            .filter(|e| e.is_finite())
            .fold(0.0_f64, f64::max);

        let mut changed = false;

        ui.add_space(self.tokens.space_4);
        ui.horizontal_wrapped(|ui| {
            ui.label("Positions")
                .on_hover_text("Restrict the view to a range of alignment positions");
            let r = &mut self.filter_state.position_range;
            changed |= ui
                .add(
                    egui::DragValue::new(&mut r.0)
                        .prefix("from ")
                        .range(1..=last_position),
                )
                .changed();
            changed |= ui
                .add(
                    egui::DragValue::new(&mut r.1)
                        .prefix("to ")
                        .range(1..=last_position),
                )
                .changed();

            ui.add_space(self.tokens.space_12);
            ui.label("Entropy")
                .on_hover_text("Restrict the view to an entropy (diversity) range, in bits");
            let e = &mut self.filter_state.entropy_range;
            changed |= ui
                .add(
                    egui::DragValue::new(&mut e.0)
                        .prefix("min ")
                        .speed(0.01)
                        .range(0.0..=max_entropy),
                )
                .changed();
            changed |= ui
                .add(
                    egui::DragValue::new(&mut e.1)
                        .prefix("max ")
                        .speed(0.01)
                        .range(0.0..=max_entropy),
                )
                .changed();

            ui.add_space(self.tokens.space_12);
            ui.label("Motifs").on_hover_text(
                "Keep positions containing at least one variant of these motif types",
            );
            for motif in [
                MotifType::Index,
                MotifType::Major,
                MotifType::Minor,
                MotifType::Unique,
            ] {
                let mut checked = self.filter_state.motif_types.contains(&motif);
                if ui.checkbox(&mut checked, motif.display_name()).changed() {
                    if checked {
                        if !self.filter_state.motif_types.contains(&motif) {
                            self.filter_state.motif_types.push(motif);
                        }
                    } else {
                        self.filter_state.motif_types.retain(|m| m != &motif);
                    }
                    changed = true;
                }
            }

            ui.add_space(self.tokens.space_12);
            changed |= ui
                .checkbox(&mut self.filter_state.include_low_support, "Low support")
                .on_hover_text(
                    "Include positions tagged NS (no support) or LS (low support). \
                     ELS positions are always included \u{2014} they are scientifically valid.",
                )
                .changed();

            // Live result count and reset, right-aligned so the bar reads
            // "controls on the left, outcome on the right".
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Reset")
                    .on_hover_text("Restore the default view: all positions, all motifs")
                    .clicked()
                {
                    self.filter_state = FilterState::default_for(results);
                    changed = true;
                }

                ui.add_space(self.tokens.space_8);

                let shown = self.filtered_positions.len();
                let total = results.results.len();
                let text = format!("{shown} / {total} positions");
                if shown == total {
                    ui.weak(text);
                } else {
                    // Emphasise when a filter is actually narrowing the view, so
                    // a filtered dataset is never mistaken for the whole one.
                    ui.label(egui::RichText::new(text).color(self.tokens.accent).strong());
                }
            });
        });
        ui.add_space(self.tokens.space_4);

        // Applied by `logic()` once the interaction ends: see
        // `apply_pending_filters` for why this is deferred rather than immediate.
        if changed {
            self.filter_dirty = true;
        }
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
                // Distinguish "filtered everything out" (actionable: widen the
                // filters) from "the analysis produced nothing" (not actionable).
                let message = if results.results.is_empty() {
                    "This analysis contains no positions"
                } else {
                    "No positions match the current filters \u{2014} try Reset"
                };
                ui.colored_label(self.tokens.text_muted, message);
                return;
            }

            let row_height = 22.0;
            // Absorb whatever height the chart and HCS strip left over, instead
            // of a fixed size. A constant height both wasted space on tall
            // windows and, now that the workspace no longer sits in an outer
            // ScrollArea, pushed the table off the bottom of short ones with no
            // way to reach it. The floor keeps a few rows visible when the window
            // is extremely short; the table scrolls internally beyond that.
            let table_height = ui.available_height().max(row_height * 4.0);

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
            // Captured before the table closures borrow `ui`.
            let mono_size = self.tokens.font_size_mono;
            // Mirrors the entropy chart's cursor (see `show_entropy_chart`).
            let hovered = self.hovered_position;
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

                        // Selection and hover are distinct states: selection is
                        // sticky and drives the inspector, hover is transient and
                        // mirrors the chart cursor so the two views stay linked.
                        let is_selected = selected == Some(pos.position);
                        let is_hovered = hovered == Some(pos.position);
                        if is_selected || is_hovered {
                            row.set_selected(true);
                        }

                        row.col(|ui| {
                            let text = format!("{}", pos.position);
                            // Mark the chart-hovered row so it is distinguishable
                            // from the selected one at a glance.
                            if is_hovered && !is_selected {
                                ui.label(egui::RichText::new(text).italics());
                            } else {
                                ui.label(text);
                            }
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
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}  {:.1}%",
                                            top.sequence, top.incidence
                                        ))
                                        .monospace()
                                        .size(mono_size),
                                    );
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

            // Apply row click selection (shared helper invalidates derived state)
            if let Some(pos) = new_selection {
                self.set_selected_position(Some(pos), results);
            }
        });
    }
}

/// Short label for a character-validation mode.
fn validation_mode_label(mode: dima_lib::ValidationMode) -> &'static str {
    match mode {
        dima_lib::ValidationMode::Strict => "Strict",
        dima_lib::ValidationMode::Permissive => "Permissive",
        dima_lib::ValidationMode::ReportOnly => "Report only",
    }
}

/// Explanation shown on hover for a character-validation mode.
fn validation_mode_help(mode: dima_lib::ValidationMode) -> &'static str {
    match mode {
        dima_lib::ValidationMode::Strict => {
            "Accept only characters in the selected alphabet. Recommended for \
             scientific accuracy."
        }
        dima_lib::ValidationMode::Permissive => {
            "Also accept ambiguous characters. They still produce NA k-mers but \
             do not raise warnings."
        }
        dima_lib::ValidationMode::ReportOnly => {
            "Accept everything and report invalid characters. Useful for \
             assessing data quality."
        }
    }
}

/// Build a safe default file name from a query name.
///
/// Query names come from user input or from a FASTA header, so they may contain
/// path separators or characters the platform rejects in file names. Mapping
/// those to underscores keeps the save dialog's suggested name usable instead of
/// proposing a name the OS would refuse.
fn sanitize_file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Leading dots would create a hidden file; trailing dots are invalid on
    // Windows. Trim both, and fall back when nothing usable remains.
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == '_');
    if trimmed.is_empty() {
        "dima_results".to_string()
    } else {
        trimmed.to_string()
    }
}

/// File stem as a lossy `String`, or empty when the path has none.
fn file_stem_of(path: &std::path::Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use dima_lib::{HighestEntropy, Position, Variant};

    /// Build an app with the real initial state, without an eframe context.
    fn test_app() -> DimaApp {
        DimaApp::with_theme(Theme::Light, DesignTokens::light(), false)
    }

    /// A small but structurally complete result set.
    fn test_results(positions: usize) -> Results {
        let results = (1..=positions)
            .map(|i| Position {
                position: i,
                // Exercise the low-support rendering path too.
                low_support: (i % 7 == 0).then(|| "LS".to_string()),
                entropy: (i as f64 % 5.0) * 0.3,
                support: 100,
                distinct_variants_count: 2,
                distinct_variants_incidence: 25.0,
                total_variants_incidence: 40.0,
                diversity_motifs: Some(vec![
                    Variant {
                        sequence: "ACDEFGHIK".to_string(),
                        count: 60,
                        incidence: 60.0,
                        motif_short: Some("I".to_string()),
                        motif_long: Some("Index".to_string()),
                        metadata: None,
                    },
                    Variant {
                        sequence: "ACDEFGHIL".to_string(),
                        count: 40,
                        incidence: 40.0,
                        motif_short: Some("Ma".to_string()),
                        motif_long: Some("Major".to_string()),
                        metadata: None,
                    },
                ]),
            })
            .collect();

        Results {
            sequence_count: 100,
            support_threshold: 30,
            low_support_count: 0,
            query_name: "render_test".to_string(),
            kmer_length: 9,
            highest_entropy: HighestEntropy {
                position: 1,
                entropy: 1.2,
            },
            average_entropy: 0.6,
            results,
        }
    }

    /// Render one frame of the whole app, returning any panic-free completion.
    ///
    /// This drives the real panel stack (header, filter bar, inspector, centre),
    /// so a malformed layout, a bad id, or an out-of-bounds index surfaces here
    /// rather than only at runtime.
    fn render_one_frame(app: &mut DimaApp) {
        let ctx = egui::Context::default();
        crate::theme::fonts::install(&ctx);
        init_both_theme_styles(&ctx);
        // `run_ui` hands us the root `Ui` directly, which is exactly what the
        // real `eframe::App::ui` hook receives — so this drives the identical
        // code path, including the panel stack.
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.render(ui);
        });

        // There is no renderer here to upload font/image deltas to. egui asserts
        // if an unapplied delta is dropped, and clearing is the documented way to
        // discard one intentionally.
        output.textures_delta.clear();
    }

    /// A variant with just the fields the motif helpers read.
    fn variant(sequence: &str, incidence: f64, motif_short: Option<&str>) -> Variant {
        Variant {
            sequence: sequence.to_string(),
            count: 1,
            incidence,
            motif_short: motif_short.map(str::to_string),
            motif_long: None,
            metadata: None,
        }
    }

    /// Results whose index variant carries every awkward shape of metadata the
    /// field list has to survive: more values than the top-N, exactly one
    /// value, all-zero counts, more values than the hard cap, and a long
    /// non-ASCII value. The major variant carries none at all.
    ///
    /// Field types are never named here: the `metadata` field's own type drives
    /// inference, so the test does not depend on which map implementation
    /// `dima_lib` uses.
    fn test_results_with_metadata() -> Results {
        let countries: Vec<(String, usize)> =
            (0..12).map(|i| (format!("country-{i}"), 12 - i)).collect();
        let regions: Vec<(String, usize)> = (0..METADATA_MAX_VALUES + 50)
            .map(|i| (format!("region-{i}"), 1))
            .collect();

        let mut results = test_results(3);
        for pos in &mut results.results {
            let Some(variants) = pos.diversity_motifs.as_mut() else {
                continue;
            };
            variants[0].metadata = Some(
                [
                    ("country".to_string(), countries.iter().cloned().collect()),
                    (
                        "host".to_string(),
                        [("Homo sapiens".to_string(), 60)].into_iter().collect(),
                    ),
                    // Every count zero: there is nothing to apportion, so the
                    // field is skipped rather than drawn as empty bars.
                    (
                        "note".to_string(),
                        [("unrecorded".to_string(), 0)].into_iter().collect(),
                    ),
                    ("region".to_string(), regions.iter().cloned().collect()),
                    (
                        "strain".to_string(),
                        [(
                            "\u{03B2}-variant \u{4E2D}\u{6587} lineage, deliberately long label"
                                .to_string(),
                            7,
                        )]
                        .into_iter()
                        .collect(),
                    ),
                ]
                .into_iter()
                .collect(),
            );
        }
        results
    }

    #[test]
    fn renders_setup_view_without_panicking() {
        let mut app = test_app();
        assert_eq!(app.current_view, View::Setup);
        render_one_frame(&mut app);
    }

    #[test]
    fn elided_galley_fits_its_width_and_reports_elision() {
        let ctx = egui::Context::default();
        crate::theme::fonts::install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = DimaApp::row_font(ui);
            let ink = egui::Color32::BLACK;
            let long = "a metadata value far too long for a narrow inspector column";

            let galley = DimaApp::elided_galley(ui, long, font.clone(), ink, 60.0);
            assert!(galley.elided, "text wider than the column must be elided");
            assert!(
                galley.size().x <= 61.0,
                "elided text must fit the column, got {}",
                galley.size().x
            );

            // Short enough to fit: left alone, so no tooltip is offered for it.
            let galley = DimaApp::elided_galley(ui, "host", font.clone(), ink, 200.0);
            assert!(!galley.elided);

            // Degenerate widths are reachable when the percentage column eats
            // the row; they must lay out rather than panic.
            for width in [0.0, -10.0, f32::NAN] {
                let _ = DimaApp::elided_galley(ui, long, font.clone(), ink, width);
            }
        });
        // No renderer here to apply the font atlas delta to; egui asserts if an
        // unapplied delta is dropped.
        output.textures_delta.clear();
    }

    #[test]
    fn motif_class_index_agrees_with_the_class_table() {
        // `motif_class_index` matches the codes literally for speed, so nothing
        // but this test stops it drifting from the table that supplies the
        // labels and the segment order — and a drift would silently paint one
        // class's incidence under another class's name.
        for (i, (code, _label)) in MOTIF_CLASSES.iter().enumerate() {
            assert_eq!(
                DimaApp::motif_class_index(Some(code)),
                Some(i),
                "code {code} must map to its own row in MOTIF_CLASSES"
            );
        }
        assert_eq!(DimaApp::motif_class_index(None), None);
        assert_eq!(DimaApp::motif_class_index(Some("not-a-motif")), None);
        // Codes are case-sensitive as `Position::new` writes them.
        assert_eq!(DimaApp::motif_class_index(Some("i")), None);
    }

    #[test]
    fn motif_incidences_total_each_class_and_ignore_the_unclassified() {
        let variants = [
            variant("AAA", 10.0, Some("I")),
            variant("AAC", 5.0, Some("Ma")),
            variant("AAG", 2.5, Some("Mi")),
            variant("AAT", 1.0, Some("U")),
            variant("ACA", 1.0, Some("U")),
            // A `.dima` import is validated for container integrity, not for
            // the values inside it, so an absent or unknown motif is reachable.
            // Charging its incidence to a class would misstate the composition.
            variant("ACC", 99.0, None),
            variant("ACG", 99.0, Some("not-a-motif")),
        ];
        assert_eq!(DimaApp::motif_incidences(&variants), [10.0, 5.0, 2.5, 2.0]);
    }

    #[test]
    fn motif_incidences_skip_non_finite_and_negative_incidences() {
        // A single NaN would otherwise poison the total, and every segment
        // would be painted at NaN width.
        let variants = [
            variant("AAA", 20.0, Some("I")),
            variant("AAC", f64::NAN, Some("I")),
            variant("AAG", f64::INFINITY, Some("Ma")),
            variant("AAT", -5.0, Some("Mi")),
        ];
        let totals = DimaApp::motif_incidences(&variants);
        assert_eq!(totals, [20.0, 0.0, 0.0, 0.0]);
        assert!(totals.iter().sum::<f64>().is_finite());
    }

    #[test]
    fn motif_shares_are_percentages_or_nothing_to_draw() {
        // Normal position: shares are the classes' percentages and total 100.
        let shares = DimaApp::motif_shares(&[
            variant("AAA", 30.0, Some("I")),
            variant("AAC", 10.0, Some("Ma")),
        ])
        .expect("a classified position is drawable");
        assert!((shares[0] - 75.0).abs() < 1e-9);
        assert!((shares[1] - 25.0).abs() < 1e-9);
        assert_eq!(shares[2], 0.0);
        assert_eq!(shares[3], 0.0);
        assert!((shares.iter().sum::<f64>() - 100.0).abs() < 1e-9);

        // Normalised against what is actually classified, so an unclassified
        // variant cannot shrink the bar to a partial fill.
        let shares =
            DimaApp::motif_shares(&[variant("AAA", 10.0, Some("I")), variant("AAC", 90.0, None)])
                .expect("one classified variant is enough");
        assert!((shares[0] - 100.0).abs() < 1e-9);

        // Nothing to draw: no variants, none classified, and incidences that
        // sum past the floating-point range.
        assert_eq!(DimaApp::motif_shares(&[]), None);
        assert_eq!(DimaApp::motif_shares(&[variant("AAA", 50.0, None)]), None);
        assert_eq!(
            DimaApp::motif_shares(&[variant("AAA", 0.0, Some("I"))]),
            None
        );
        assert_eq!(
            DimaApp::motif_shares(&[
                variant("AAA", f64::MAX, Some("I")),
                variant("AAC", f64::MAX, Some("I")),
            ]),
            None,
            "an infinite total would make every share NaN"
        );
    }

    #[test]
    fn motif_composition_belongs_to_the_position_not_the_selected_variant() {
        // The defect this section replaced: the composition was drawn under a
        // per-variant heading, so it read as changing with the selected
        // variant when it never could.
        let mut app = test_app();
        app.apply_results(test_results(5));
        let results = app.results.clone().unwrap();

        let motifs = |app: &DimaApp| {
            let pos = app
                .selected_position_data(&results)
                .expect("a position is auto-selected");
            DimaApp::motif_incidences(pos.diversity_motifs.as_ref().expect("has variants"))
        };

        let before = motifs(&app);
        app.selected_variant_index = Some(1);
        assert_eq!(before, motifs(&app));
    }

    #[test]
    fn visible_metadata_values_is_bounded_in_both_modes() {
        // Collapsed: the top few, and never more than exist.
        assert_eq!(
            DimaApp::visible_metadata_values(50, false),
            METADATA_TOP_VALUES
        );
        assert_eq!(DimaApp::visible_metadata_values(3, false), 3);
        assert_eq!(DimaApp::visible_metadata_values(0, false), 0);

        // Expanded: everything, up to the cap that stops one pathological
        // field from stalling a frame.
        assert_eq!(DimaApp::visible_metadata_values(50, true), 50);
        assert_eq!(
            DimaApp::visible_metadata_values(METADATA_MAX_VALUES, true),
            METADATA_MAX_VALUES
        );
        assert_eq!(
            DimaApp::visible_metadata_values(METADATA_MAX_VALUES + 1_000, true),
            METADATA_MAX_VALUES
        );
    }

    #[test]
    fn metadata_view_state_survives_selection_but_not_a_new_dataset() {
        let mut app = test_app();
        app.apply_results(test_results(10));
        let results = app.results.clone().unwrap();

        // The user opens a field and asks to see all of its values.
        app.metadata_open_fields = Some(std::iter::once("host".to_string()).collect());
        app.expanded_metadata_fields.insert("host".to_string());

        // Moving position, and with it the auto-selected variant, must keep
        // both: reading one field across variants is the reason to switch
        // variant at all. Only the per-variant cache is invalidated.
        app.set_selected_position(Some(4), &results);
        assert!(
            app.cached_metadata.is_none(),
            "the per-variant metadata cache must still invalidate"
        );
        assert_eq!(
            app.metadata_open_fields,
            Some(std::iter::once("host".to_string()).collect()),
            "an open field must stay open across a position change"
        );
        assert!(app.expanded_metadata_fields.contains("host"));

        // A new dataset can have entirely different field names, so state keyed
        // by them is meaningless and both reset.
        app.apply_results(test_results(3));
        assert_eq!(app.metadata_open_fields, None);
        assert!(app.expanded_metadata_fields.is_empty());
    }

    #[test]
    fn renders_inspector_metadata_for_every_shape_of_field() {
        let mut app = test_app();
        app.apply_results(test_results_with_metadata());
        assert_eq!(
            app.available_metadata_fields,
            ["country", "host", "note", "region", "strain"],
            "every field in the dataset must be discovered"
        );

        // Arrival: exactly one field opens itself, so the section shows data
        // rather than a stack of shut headers.
        render_one_frame(&mut app);
        assert_eq!(
            app.metadata_open_fields,
            Some(std::iter::once("country".to_string()).collect()),
            "the first field must open itself on the first render"
        );

        // Every field open and fully expanded: exercises the top-N overflow,
        // the hard cap and its "+N more" note, the single-value field, the
        // all-zero field that must be skipped, and the long non-ASCII value.
        let all: std::collections::HashSet<String> =
            app.available_metadata_fields.iter().cloned().collect();
        app.metadata_open_fields = Some(all.clone());
        app.expanded_metadata_fields = all;
        render_one_frame(&mut app);

        // Everything closed by the user: stays closed rather than re-opening.
        app.metadata_open_fields = Some(std::collections::HashSet::new());
        render_one_frame(&mut app);
        assert_eq!(
            app.metadata_open_fields,
            Some(std::collections::HashSet::new()),
            "`Some(empty)` is a user decision and must never be auto-filled"
        );

        // The major variant carries no metadata, and no variant at all is also
        // reachable (a position whose variants were filtered out of view).
        app.selected_variant_index = Some(1);
        render_one_frame(&mut app);
        app.selected_variant_index = None;
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_inspector_metadata_header_when_a_position_has_no_variants() {
        // A dataset can carry metadata while an individual position holds no
        // variants at all (no support). The metadata section still has to draw
        // its header and say so, or the separator above it hangs over nothing.
        let mut app = test_app();
        let mut results = test_results_with_metadata();
        results.results[0].diversity_motifs = None;
        app.apply_results(results);

        assert_eq!(app.selected_position, Some(1));
        assert_eq!(app.selected_variant_index, None);
        assert!(!app.available_metadata_fields.is_empty());
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_inspector_for_hostile_metadata_and_motif_numbers() {
        // `.dima` import checksums the container but never range-checks the
        // numbers inside it, so these are all reachable from a corrupt or
        // crafted file. None of them may panic: the release profile aborts on
        // panic, so a bad file would take the window down.
        let mut app = test_app();
        let mut results = test_results_with_metadata();

        let pos = &mut results.results[0];
        let variants = pos.diversity_motifs.as_mut().expect("has variants");
        // Counts that overflow a plain `usize` sum, which panics under debug
        // overflow checks.
        variants[0].metadata = Some(
            [(
                "host".to_string(),
                [
                    ("saturating".to_string(), usize::MAX),
                    ("overflow".to_string(), usize::MAX),
                ]
                .into_iter()
                .collect(),
            )]
            .into_iter()
            .collect(),
        );
        // Two variants of the same class, each at the top of the f64 range, so
        // their incidences sum past infinity and exercise the composition's
        // non-finite guard. Asserted here so the test cannot quietly stop
        // covering that path.
        variants[0].incidence = f64::MAX;
        variants[1].incidence = f64::MAX;
        variants[1].motif_short = variants[0].motif_short.clone();
        assert!(
            !DimaApp::motif_incidences(variants)
                .iter()
                .sum::<f64>()
                .is_finite(),
            "this input is meant to drive the non-finite branch"
        );

        app.apply_results(results);
        app.metadata_open_fields = Some(std::iter::once("host".to_string()).collect());
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_inspector_with_an_out_of_range_variant_selection() {
        // Selection is an index into the position's variants; it must degrade
        // to "nothing to show" rather than panicking if it ever goes stale.
        let mut app = test_app();
        app.apply_results(test_results_with_metadata());
        app.selected_variant_index = Some(9_999);
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_workspace_panel_stack_without_panicking() {
        let mut app = test_app();
        app.apply_results(test_results(40));
        assert_eq!(app.current_view, View::Workspace);
        render_one_frame(&mut app);
    }

    #[test]
    fn applying_results_auto_selects_the_first_position() {
        let mut app = test_app();
        app.apply_results(test_results(10));
        assert_eq!(
            app.selected_position,
            Some(1),
            "the inspector must never open empty"
        );
        // A default variant must be chosen so the variant panel has content.
        assert!(app.selected_variant_index.is_some());
    }

    #[test]
    fn applying_results_clears_previous_dataset_view_state() {
        let mut app = test_app();
        app.apply_results(test_results(50));

        // Simulate an explored state on the first dataset.
        app.entropy_viewport = Some((10.0, 20.0));
        app.position_explorer_sort = Some(TableSort::new(1, SortDirection::Descending));
        app.expanded_metadata_fields.insert("host".to_string());
        app.set_selected_position(Some(42), &app.results.clone().unwrap());

        // Loading a smaller dataset must not carry any of it over: a stale
        // viewport would render an empty chart, and a stale selection would
        // point at a position that no longer exists.
        app.apply_results(test_results(5));
        assert_eq!(app.entropy_viewport, None);
        assert_eq!(app.position_explorer_sort, None);
        assert!(app.expanded_metadata_fields.is_empty());
        assert_eq!(app.selected_position, Some(1));
        assert_eq!(app.last_uploaded_viewport, None);
    }

    #[test]
    fn renders_workspace_with_no_positions() {
        // Degenerate but reachable: an analysis can yield zero positions.
        let mut app = test_app();
        let mut empty = test_results(1);
        empty.results.clear();
        app.apply_results(empty);
        assert_eq!(app.selected_position, None);
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_workspace_when_filters_exclude_everything() {
        let mut app = test_app();
        app.apply_results(test_results(20));
        // An impossible range: the chart and table must both degrade gracefully.
        app.filter_state.position_range = (999, 1000);
        app.filtered_positions = app.filter_state.apply(&app.results.clone().unwrap());
        assert!(app.filtered_positions.is_empty());
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_workspace_with_all_zero_entropy() {
        // Scientifically meaningful (every position fully conserved) and a
        // degenerate chart range: must show the explanatory placeholder rather
        // than dividing by a zero y-range.
        let mut app = test_app();
        let mut results = test_results(15);
        for p in &mut results.results {
            p.entropy = 0.0;
        }
        app.apply_results(results);
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_workspace_with_non_finite_entropy() {
        // Corrupt or unusual input can carry NaN/Inf; rendering must not panic.
        let mut app = test_app();
        let mut results = test_results(12);
        results.results[3].entropy = f64::NAN;
        results.results[4].entropy = f64::INFINITY;
        app.apply_results(results);
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_workspace_with_variants_missing() {
        // `diversity_motifs` is optional; the inspector must tolerate None.
        let mut app = test_app();
        let mut results = test_results(6);
        for p in &mut results.results {
            p.diversity_motifs = None;
        }
        app.apply_results(results);
        assert_eq!(app.selected_variant_index, None);
        render_one_frame(&mut app);
    }

    #[test]
    fn renders_with_toasts_of_every_severity() {
        let mut app = test_app();
        app.apply_results(test_results(5));
        app.error_state.push(ErrorMessage::error("err".into()));
        app.error_state.push(ErrorMessage::warning("warn".into()));
        app.error_state.push(ErrorMessage::success("ok".into()));
        app.error_state.push(ErrorMessage::info("info".into()));
        // More than the visible cap, to exercise the "+N more" path.
        for i in 0..5 {
            app.error_state
                .push(ErrorMessage::error(format!("extra {i}")));
        }
        render_one_frame(&mut app);
    }

    #[test]
    fn selection_helper_is_idempotent_and_invalidates_caches() {
        let mut app = test_app();
        app.apply_results(test_results(10));
        let results = app.results.clone().unwrap();

        app.set_selected_position(Some(3), &results);
        app.cached_metadata = Some(((3, 0), Vec::new()));
        app.variant_table_sort = Some(TableSort::new(0, SortDirection::Ascending));

        // Re-selecting the same position must not clear caches (no needless work).
        app.set_selected_position(Some(3), &results);
        assert!(app.cached_metadata.is_some());
        assert!(app.variant_table_sort.is_some());

        // Changing position must invalidate them.
        app.set_selected_position(Some(4), &results);
        assert!(app.cached_metadata.is_none());
        assert!(app.variant_table_sort.is_none());
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_fixed() {
        let mut app = test_app();
        // View 1..=100; cursor at 25% => data x = 25.75.
        let (view_min, view_range) = (1.0_f32, 99.0_f32);
        let cursor_nx = 0.25_f32;
        let cursor_x = view_min + cursor_nx * view_range;

        app.zoom_entropy_chart(0.5, cursor_nx, view_min, view_range, 1.0, 100.0);
        let (lo, hi) = app.entropy_viewport.expect("should be zoomed");

        // The same fraction into the new view must still be the same data x.
        let new_cursor_x = lo as f32 + cursor_nx * (hi - lo) as f32;
        assert!(
            (new_cursor_x - cursor_x).abs() < 0.01,
            "cursor anchor drifted: {new_cursor_x} vs {cursor_x}"
        );
    }

    #[test]
    fn zooming_out_snaps_back_to_showing_everything() {
        let mut app = test_app();
        app.entropy_viewport = Some((40.0, 60.0));
        // Feed the updated viewport back in each round, exactly as the chart does
        // per frame. It must end at "show all" rather than at a span that is
        // almost-but-not-quite the full range.
        let mut view = (40.0_f32, 20.0_f32); // (min, span)
        for _ in 0..20 {
            app.zoom_entropy_chart(1.25, 0.5, view.0, view.1, 1.0, 100.0);
            match app.entropy_viewport {
                None => return,
                Some((lo, hi)) => view = (lo as f32, (hi - lo) as f32),
            }
        }
        panic!("zooming out never returned to the full range");
    }

    #[test]
    fn zoom_never_collapses_below_the_minimum_span() {
        let mut app = test_app();
        let mut view = (1.0_f32, 99.0_f32);
        for _ in 0..50 {
            app.zoom_entropy_chart(0.5, 0.5, view.0, view.1, 1.0, 100.0);
            if let Some((lo, hi)) = app.entropy_viewport {
                let span = (hi - lo) as f32;
                assert!(
                    span >= MIN_ENTROPY_ZOOM_SPAN - 0.001,
                    "span {span} collapsed below the floor"
                );
                view = (lo as f32, span);
            }
        }
    }

    #[test]
    fn zoom_stays_within_the_data_range() {
        let mut app = test_app();
        // Cursor pinned to the far right edge must not scroll past the data.
        app.zoom_entropy_chart(0.5, 1.0, 1.0, 99.0, 1.0, 100.0);
        let (lo, hi) = app.entropy_viewport.expect("zoomed");
        assert!(lo >= 1.0 - 0.001, "left edge escaped: {lo}");
        assert!(hi <= 100.0 + 0.001, "right edge escaped: {hi}");
    }

    #[test]
    fn zoom_is_a_no_op_for_a_degenerate_data_range() {
        let mut app = test_app();
        // A single-position dataset has no span to zoom into.
        app.zoom_entropy_chart(0.5, 0.5, 5.0, 1.0, 5.0, 5.0);
        assert_eq!(app.entropy_viewport, None);
    }

    #[test]
    fn sanitize_file_stem_produces_usable_names() {
        assert_eq!(sanitize_file_stem("spike_aa"), "spike_aa");
        assert_eq!(sanitize_file_stem("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_file_stem("  "), "dima_results");
        assert_eq!(sanitize_file_stem(""), "dima_results");
        assert_eq!(sanitize_file_stem("..."), "dima_results");
        // Leading dots would create a hidden file.
        assert!(!sanitize_file_stem(".hidden").starts_with('.'));
    }

    #[test]
    fn file_stem_of_handles_missing_stem() {
        assert_eq!(file_stem_of(std::path::Path::new("/tmp/a.fasta")), "a");
        assert_eq!(file_stem_of(std::path::Path::new("/")), "");
    }
}
