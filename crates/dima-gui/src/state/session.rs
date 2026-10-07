//! One loaded result set and everything the workspace derives from it.
//!
//! Derived data (shown positions, chart series, sorted table rows) is computed when its inputs
//! change, never per frame (P5). The table order survives filter changes (G47), keyboard
//! navigation follows it (G11), and every selection change scrolls the table to the selected
//! row unless the table itself made it (G46). All per-dataset view state lives here, so a new
//! dataset starts clean (G26).

use std::path::PathBuf;
use std::sync::Arc;

use dima_core::config::AnalysisParams;
use dima_core::model::{Motif, PositionResult, Results, Variant};
use dima_core::run_report::FileRecord;

use super::filter::{Bounds, FilterState};
use super::metadata::MetadataView;
use super::sort::{TableSort, compare_positions, compare_variants};
use crate::charts::series::Series;
use crate::charts::transform::position_extent;
use crate::charts::viewport;
use crate::workers::analysis::RunFacts;

/// An analysis run in this session (for the run details and the run report, D103).
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysedRun {
    /// The alignment.
    pub path: PathBuf,
    /// The parameters passed to the library.
    pub params: AnalysisParams,
    /// How it ran.
    pub facts: RunFacts,
}

/// Where the results came from.
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    /// Analysed in this session.
    Analysed(Box<AnalysedRun>),
    /// Opened from a results file.
    Opened(PathBuf),
}

/// What made a selection (decides whether the table scrolls to it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectSource {
    /// A table row was clicked (already visible).
    Table,
    /// The chart was clicked.
    Chart,
    /// Arrow keys, Home or End.
    Keyboard,
    /// The command palette.
    Palette,
    /// Chosen by the app (first shown position).
    Auto,
}

/// Keyboard navigation over the table order (G11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    /// Next row.
    Next,
    /// Previous row.
    Previous,
    /// First row.
    First,
    /// Last row.
    Last,
}

/// The selected position and variant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Selected position number.
    pub position: Option<u64>,
    /// Index into that position's variants.
    pub variant: Option<usize>,
    /// The table should scroll the selected row into view on its next draw (G46).
    pub scroll_to_row: bool,
}

/// Variant-table tools of the inspector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VariantView {
    /// Case-insensitive substring of the k-mer or motif.
    pub search: String,
    /// Hide variants with a lower incidence (per cent); 0 shows all.
    pub min_incidence: f64,
    /// Column sort.
    pub sort: Option<TableSort>,
}

/// A loaded result set and its view state.
#[derive(Clone, Debug)]
pub struct Session {
    /// Unique per load (cache key of everything derived, G23).
    pub id: u64,
    /// The results.
    pub results: Arc<Results>,
    /// Where they came from.
    pub origin: Origin,
    /// Data files exported from these results (outputs of the run report).
    pub exports: Vec<FileRecord>,
    /// Stratified metadata fields, in header-format order (G54).
    pub fields: Vec<String>,
    /// Filter bounds.
    pub bounds: Bounds,
    /// Filters.
    pub filter: FilterState,
    /// A filter control changed; applied when the pointer is released (debounce).
    pub filter_dirty: bool,
    /// Shown positions (indices into `results.results`), in position order.
    pub shown: Arc<Vec<usize>>,
    /// The chart model of the shown positions.
    pub series: Arc<Series>,
    /// Incremented whenever `shown` changes.
    pub version: u64,
    /// Position-table sort.
    pub sort: Option<TableSort>,
    /// Table rows (indices into `results.results`) in table order.
    pub rows: Vec<usize>,
    /// Row of each result index (`u32::MAX` when hidden).
    row_of: Vec<u32>,
    /// Selection.
    pub selection: Selection,
    /// Position under the chart cursor (mirrored in the table).
    pub hovered: Option<u64>,
    /// Chart zoom; `None` shows every shown position.
    pub viewport: Option<(f64, f64)>,
    /// Variant-table tools.
    pub variants: VariantView,
    /// Metadata section state.
    pub metadata: MetadataView,
    /// The variant-table rows last computed, with what they were computed for.
    variant_rows: Option<(VariantRowsKey, Arc<[usize]>)>,
}

/// What the variant-table rows depend on: the position and the variant tools.
#[derive(Clone, Debug, PartialEq)]
struct VariantRowsKey {
    position: Option<u64>,
    search: String,
    min_incidence: u64,
    sort: Option<TableSort>,
}

impl Session {
    /// A session showing every position, with the first one selected.
    #[must_use]
    pub fn new(id: u64, results: Arc<Results>, origin: Origin) -> Self {
        let bounds = Bounds::of(&results);
        let fields = results
            .run
            .metadata
            .as_ref()
            .map(|m| m.stratified.clone())
            .unwrap_or_default();
        let mut s = Self {
            id,
            results,
            origin,
            exports: Vec::new(),
            fields,
            bounds,
            filter: FilterState::all(bounds),
            filter_dirty: false,
            shown: Arc::new(Vec::new()),
            series: Arc::new(Series::default()),
            version: 0,
            sort: None,
            rows: Vec::new(),
            row_of: Vec::new(),
            selection: Selection::default(),
            hovered: None,
            viewport: None,
            variants: VariantView::default(),
            metadata: MetadataView::default(),
            variant_rows: None,
        };
        s.refresh_shown();
        s.select_first(SelectSource::Auto);
        s
    }

    /// The index of a position in `results.results`.
    #[must_use]
    pub fn index_of(&self, position: u64) -> Option<usize> {
        self.results
            .results
            .binary_search_by_key(&position, |p| p.position)
            .ok()
    }

    /// A position's results.
    #[must_use]
    pub fn position(&self, position: u64) -> Option<&PositionResult> {
        self.index_of(position).map(|i| &self.results.results[i])
    }

    /// The selected position's results.
    #[must_use]
    pub fn selected(&self) -> Option<&PositionResult> {
        self.selection.position.and_then(|p| self.position(p))
    }

    /// The selected variant.
    #[must_use]
    pub fn selected_variant(&self) -> Option<(usize, &Variant)> {
        let i = self.selection.variant?;
        self.selected()?.diversity_motifs.get(i).map(|v| (i, v))
    }

    /// `true` when the position is shown under the current filters.
    #[must_use]
    pub fn is_shown(&self, position: u64) -> bool {
        self.index_of(position)
            .and_then(|i| self.row_of.get(i))
            .is_some_and(|r| *r != u32::MAX)
    }

    /// The table row of a position, when shown.
    #[must_use]
    pub fn row_of_position(&self, position: u64) -> Option<usize> {
        let i = self.index_of(position)?;
        let r = *self.row_of.get(i)?;
        (r != u32::MAX).then_some(r as usize)
    }

    /// The horizontal extent of the shown positions (chart and palette bounds, G19).
    #[must_use]
    pub fn extent(&self) -> Option<(f64, f64)> {
        self.series
            .extent
            .map(|(first, last)| position_extent(first, last))
    }

    /// The variant selected by default at a position: the first Index variant in the library's
    /// order (G53), else the most common variant.
    #[must_use]
    pub fn default_variant(p: &PositionResult) -> Option<usize> {
        if p.diversity_motifs.is_empty() {
            return None;
        }
        Some(
            p.diversity_motifs
                .iter()
                .position(|v| v.motif == Motif::Index)
                .unwrap_or(0),
        )
    }

    /// Selects a position. Returns `false` when it does not exist.
    pub fn select(&mut self, position: u64, source: SelectSource) -> bool {
        let Some(p) = self.position(position) else {
            return false;
        };
        let variant = Self::default_variant(p);
        if self.selection.position != Some(position) {
            self.selection.variant = variant;
        }
        self.selection.position = Some(position);
        self.selection.scroll_to_row = source != SelectSource::Table;
        if matches!(source, SelectSource::Keyboard | SelectSource::Palette)
            && let Some(full) = self.extent()
        {
            #[allow(clippy::cast_precision_loss)]
            let x = position as f64;
            if !viewport::shows(self.viewport, full, x) {
                self.viewport = viewport::center_on(self.viewport, full, x);
            }
        }
        true
    }

    /// Selects the first row of the table, or nothing when no position is shown.
    pub fn select_first(&mut self, source: SelectSource) {
        match self.rows.first().map(|&i| self.results.results[i].position) {
            Some(p) => {
                self.select(p, source);
            }
            None => self.selection = Selection::default(),
        }
    }

    /// Moves the selection along the table order (G11).
    pub fn navigate(&mut self, nav: Nav) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() - 1;
        let current = self
            .selection
            .position
            .and_then(|p| self.row_of_position(p));
        let row = match (nav, current) {
            (Nav::First, _) => 0,
            (Nav::Last, _) => last,
            (Nav::Next, Some(r)) => (r + 1).min(last),
            (Nav::Next, None) => 0,
            (Nav::Previous, Some(r)) => r.saturating_sub(1),
            (Nav::Previous, None) => last,
        };
        let position = self.results.results[self.rows[row]].position;
        self.select(position, SelectSource::Keyboard);
    }

    /// Applies the filters: recomputes the shown positions, the chart and the table (keeping
    /// its sort, G47), resets the zoom to the new extent (G5) and keeps the selection when it
    /// is still shown (otherwise the first shown position is selected).
    pub fn apply_filters(&mut self) {
        self.filter_dirty = false;
        self.refresh_shown();
        self.viewport = None;
        let still = self.selection.position.is_some_and(|p| self.is_shown(p));
        if still {
            self.selection.scroll_to_row = true;
        } else {
            self.select_first(SelectSource::Auto);
        }
    }

    /// Resets the filters to show everything and applies them.
    pub fn reset_filters(&mut self) {
        self.filter = FilterState::all(self.bounds);
        self.apply_filters();
    }

    /// Sorts the table by a column (header click).
    pub fn sort_by(&mut self, column: usize) {
        self.sort = Some(TableSort::clicked(self.sort, column));
        self.refresh_rows();
        self.selection.scroll_to_row = true;
    }

    /// Sorts the variant table by a column (header click).
    pub fn sort_variants_by(&mut self, column: usize) {
        self.variants.sort = Some(TableSort::clicked(self.variants.sort, column));
    }

    fn refresh_shown(&mut self) {
        let shown = self.filter.apply(&self.results);
        self.series = Arc::new(Series::build(&self.results, &shown));
        self.shown = Arc::new(shown);
        self.version = self.version.wrapping_add(1);
        self.refresh_rows();
    }

    fn refresh_rows(&mut self) {
        let mut rows: Vec<usize> = self.shown.as_ref().clone();
        if let Some(sort) = self.sort {
            let r = &self.results.results;
            rows.sort_by(|&a, &b| compare_positions(&r[a], &r[b], sort));
        }
        let mut row_of = vec![u32::MAX; self.results.results.len()];
        for (row, &i) in rows.iter().enumerate() {
            row_of[i] = u32::try_from(row).unwrap_or(u32::MAX);
        }
        self.rows = rows;
        self.row_of = row_of;
    }

    /// The variants of the selected position that pass the variant tools, in table order.
    /// The minimum incidence is null-aware: it hides only finite values below it (G33).
    /// Computed when the position or a tool changes, not per frame: a diverse position can
    /// have as many variants as sequences.
    pub fn visible_variants(&mut self) -> Arc<[usize]> {
        let key = VariantRowsKey {
            position: self.selection.position,
            search: self.variants.search.clone(),
            min_incidence: self.variants.min_incidence.to_bits(),
            sort: self.variants.sort,
        };
        if let Some((k, rows)) = &self.variant_rows
            && *k == key
        {
            return Arc::clone(rows);
        }
        let rows: Arc<[usize]> = self.compute_visible_variants().into();
        self.variant_rows = Some((key, Arc::clone(&rows)));
        rows
    }

    fn compute_visible_variants(&self) -> Vec<usize> {
        let Some(p) = self.selected() else {
            return Vec::new();
        };
        // Variants are upper-case letters (checked when results are read), so the search
        // compares them with the upper-cased needle without converting each one.
        let needle = self.variants.search.trim();
        let (upper, lower) = (needle.to_uppercase(), needle.to_lowercase());
        let min = self.variants.min_incidence;
        let mut out: Vec<usize> = p
            .diversity_motifs
            .iter()
            .enumerate()
            .filter(|(_, v)| !(min > 0.0 && v.incidence.is_finite() && v.incidence < min))
            .filter(|(_, v)| {
                needle.is_empty()
                    || v.sequence.contains(&upper)
                    || v.motif.long().to_lowercase().contains(&lower)
                    || v.motif.short().to_lowercase() == lower
            })
            .map(|(i, _)| i)
            .collect();
        if let Some(sort) = self.variants.sort {
            let vs = &p.diversity_motifs;
            out.sort_by(|&a, &b| compare_variants((a, &vs[a]), (b, &vs[b]), sort));
        }
        out
    }

    /// Records an exported data file for the run report (latest per name and role).
    pub fn record_export(&mut self, file: FileRecord) {
        self.exports
            .retain(|f| !(f.name == file.name && f.role == file.role));
        self.exports.push(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::series::tests::{position, results_of};
    use crate::state::sort::{Direction, position_columns};

    fn session(n: u64) -> Session {
        let positions = (1..=n)
            .map(|p| {
                #[allow(clippy::cast_precision_loss)]
                let e = (p % 7) as f64 / 3.0;
                position(
                    p,
                    if p % 10 == 0 { None } else { Some(e) },
                    if p % 10 == 0 { 0 } else { 200 },
                    100,
                )
            })
            .collect();
        Session::new(
            1,
            Arc::new(results_of(positions)),
            Origin::Opened(PathBuf::from("x.json")),
        )
    }

    #[test]
    fn starts_with_everything_shown_and_the_first_position_selected() {
        let s = session(30);
        assert_eq!(s.shown.len(), 30);
        assert_eq!(s.selection.position, Some(1));
        assert_eq!(s.selection.variant, Some(0));
        assert_eq!(s.series.ns_spans.len(), 3);
        assert_eq!(s.extent(), Some((0.5, 30.5)));
    }

    #[test]
    fn filters_keep_the_sort_reset_the_zoom_and_fix_the_selection() {
        let mut s = session(30);
        s.sort_by(position_columns::ENTROPY);
        s.sort_by(position_columns::ENTROPY);
        assert_eq!(s.sort.unwrap().direction, Direction::Descending);
        s.viewport = Some((5.0, 15.0));
        s.select(25, SelectSource::Chart);
        s.filter.position_range = (1, 20);
        s.filter_dirty = true;
        s.apply_filters();
        assert!(!s.filter_dirty);
        assert_eq!(
            s.sort.unwrap().direction,
            Direction::Descending,
            "sort kept (G47)"
        );
        assert_eq!(s.viewport, None, "zoom reset (G5)");
        assert_eq!(s.shown.len(), 20);
        let first_row = s.results.results[s.rows[0]].position;
        assert_eq!(
            s.selection.position,
            Some(first_row),
            "hidden selection replaced"
        );
        // NS positions sort last when sorting by entropy, in both directions.
        let last_row = s.results.results[*s.rows.last().unwrap()].position;
        assert_eq!(last_row % 10, 0);
    }

    #[test]
    fn keyboard_follows_the_table_order() {
        let mut s = session(12);
        s.sort_by(position_columns::POSITION);
        s.sort_by(position_columns::POSITION); // descending
        s.navigate(Nav::First);
        assert_eq!(s.selection.position, Some(12));
        s.navigate(Nav::Next);
        assert_eq!(
            s.selection.position,
            Some(11),
            "next row, not next position"
        );
        s.navigate(Nav::Last);
        assert_eq!(s.selection.position, Some(1));
        s.navigate(Nav::Next);
        assert_eq!(s.selection.position, Some(1), "stays on the last row");
        assert!(
            s.selection.scroll_to_row,
            "keyboard selection scrolls (G46)"
        );
        s.select(5, SelectSource::Table);
        assert!(!s.selection.scroll_to_row);
    }

    #[test]
    fn palette_selection_recentres_a_zoomed_chart() {
        let mut s = session(100);
        s.viewport = Some((0.5, 10.5));
        assert!(s.select(80, SelectSource::Palette));
        let (lo, hi) = s.viewport.unwrap();
        assert!(lo <= 80.0 && 80.0 <= hi);
        assert!(
            !s.select(1000, SelectSource::Palette),
            "unknown positions are refused"
        );
    }

    #[test]
    fn variant_tools_are_null_aware() {
        let mut s = session(3);
        s.variants.min_incidence = 50.0;
        assert_eq!(&*s.visible_variants(), [0]);
        s.variants.min_incidence = 100.5;
        assert!(s.visible_variants().is_empty());
        s.variants.min_incidence = 0.0;
        s.variants.search = "index".into();
        assert_eq!(&*s.visible_variants(), [0]);
        s.variants.search = "zzz".into();
        assert!(s.visible_variants().is_empty());
        // The k-mer search ignores case.
        let kmer = s.selected().unwrap().diversity_motifs[0]
            .sequence
            .to_lowercase();
        s.variants.search = kmer;
        assert_eq!(&*s.visible_variants(), [0]);
    }

    #[test]
    fn exports_are_recorded_once_per_file() {
        let mut s = session(3);
        let f = |bytes| FileRecord {
            role: "results",
            name: "x.json".into(),
            size_bytes: bytes,
            sha256: "0".repeat(64),
        };
        s.record_export(f(1));
        s.record_export(f(2));
        assert_eq!(s.exports.len(), 1);
        assert_eq!(s.exports[0].size_bytes, 2);
    }
}
