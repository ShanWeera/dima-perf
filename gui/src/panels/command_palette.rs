//! Cmd/Ctrl+K command palette: a unified finder and action launcher.
//!
//! One surface answers all three "where is it?" questions: jump to a position,
//! find which positions carry a given k-mer, and run an action without hunting
//! for its button. Keeping it keyboard-first suits the scanning workflow (the
//! entropy landscape is explored by position), while every action it exposes is
//! also reachable from a visible control, so the palette accelerates rather than
//! hides functionality.
//!
//! This module owns only palette state and rendering; it never mutates app
//! state. It reports the chosen [`PaletteAction`] and lets the app apply it.

use dima_lib::Results;

use crate::theme::DesignTokens;
use crate::util::truncate_display;
use crate::workers::io::ExportFormat;

/// Maximum matches listed, keeping both the list readable and the scan bounded
/// on datasets with thousands of variants per position.
const MAX_RESULTS: usize = 12;

/// Shortest query that triggers a sequence scan. Below this nearly everything
/// matches, which is slow and useless.
const MIN_SEQUENCE_QUERY: usize = 2;

/// Something the user picked in the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    /// Select (and scroll to) a position number.
    GoToPosition(usize),
    Export(ExportFormat),
    ResetFilters,
    ResetZoom,
    ToggleTheme,
    OpenFile,
    BackToSetup,
}

impl PaletteAction {
    fn label(&self) -> String {
        match self {
            Self::GoToPosition(p) => format!("Go to position {p}"),
            Self::Export(f) => format!("Export {}", f.label()),
            Self::ResetFilters => "Reset filters".to_string(),
            Self::ResetZoom => "Reset chart zoom".to_string(),
            Self::ToggleTheme => "Toggle light / dark theme".to_string(),
            Self::OpenFile => "Open file...".to_string(),
            Self::BackToSetup => "Back to setup".to_string(),
        }
    }
}

/// One row in the palette.
#[derive(Clone)]
struct Entry {
    action: PaletteAction,
    label: String,
    /// Secondary text (e.g. which motif a matching k-mer is).
    detail: String,
}

/// Palette state. Lives on the app; `show` is called every frame.
#[derive(Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    /// Index of the highlighted row, clamped to the result count when rendered.
    cursor: usize,
    /// Set for one frame after opening so the text field takes focus.
    just_opened: bool,

    // ── Result cache ──────────────────────────────────────────────────
    // Matching scans every variant of every position, which on a large dataset
    // is millions of comparisons. The palette re-renders every frame, so doing
    // that per frame would stall the UI for as long as it stayed open. Results
    // depend only on the query and the loaded dataset, so they are computed once
    // per change and reused.
    cached: Vec<Entry>,
    cached_query: Option<String>,
    /// Identifies the dataset the cache was built from, so loading new results
    /// invalidates it.
    cached_data_key: Option<(usize, String)>,
}

impl CommandPalette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the palette with an empty query.
    pub fn open(&mut self) {
        self.open = true;
        self.just_opened = true;
        self.query.clear();
        self.cursor = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.cursor = 0;
        // Release the cached matches; they can hold a row per position.
        self.cached = Vec::new();
        self.cached_query = None;
        self.cached_data_key = None;
    }

    /// Return the matches for the current query, recomputing only when the
    /// query or the loaded dataset changed.
    fn entries(&mut self, results: Option<&Results>) -> &[Entry] {
        // Cheap identity for the loaded dataset: a different analysis differs in
        // name or position count, and re-running replaces both anyway.
        let data_key = results.map(|r| (r.results.len(), r.query_name.clone()));

        let stale = self.cached_query.as_deref() != Some(self.query.as_str())
            || self.cached_data_key != data_key;

        if stale {
            self.cached = self.build_entries(results);
            self.cached_query = Some(self.query.clone());
            self.cached_data_key = data_key;
        }
        &self.cached
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Render the palette, returning an action when the user commits to one.
    ///
    /// `results` is `None` on the setup screen, where data-dependent entries
    /// (go-to-position, sequence search, exports) do not apply.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        tokens: &DesignTokens,
        results: Option<&Results>,
    ) -> Option<PaletteAction> {
        if !self.open {
            return None;
        }

        // Escape always dismisses, even while the text field has focus.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close();
            return None;
        }

        // Cached: recomputed only when the query or dataset changes.
        let entries = self.entries(results).to_vec();

        // Keep the highlight inside the current result set: the query changes
        // the list length between frames.
        if entries.is_empty() {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(entries.len() - 1);
        }

        let (up, down, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
            )
        });
        if down && !entries.is_empty() {
            self.cursor = (self.cursor + 1) % entries.len();
        }
        if up && !entries.is_empty() {
            self.cursor = self.cursor.checked_sub(1).unwrap_or(entries.len() - 1);
        }

        let mut chosen: Option<PaletteAction> = None;

        // Modal-style overlay: a dimmed backdrop makes it clear the palette has
        // focus, and clicking outside dismisses it.
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("command_palette_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let response = ui.allocate_rect(screen, egui::Sense::click());
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(96));
                if response.clicked() {
                    self.open = false;
                }
            });

        egui::Area::new(egui::Id::new("command_palette"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 96.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(tokens.surface_elevated)
                    .stroke(egui::Stroke::new(1.0, tokens.border))
                    .corner_radius(tokens.panel_rounding)
                    .inner_margin(tokens.space_12)
                    .show(ui, |ui| {
                        ui.set_width(520.0);

                        let field = ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .hint_text("Jump to a position, find a k-mer, or run a command")
                                .desired_width(f32::INFINITY),
                        );
                        if self.just_opened {
                            field.request_focus();
                            self.just_opened = false;
                        }

                        ui.add_space(tokens.space_8);

                        if entries.is_empty() {
                            ui.weak("No matches");
                            return;
                        }

                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for (i, entry) in entries.iter().enumerate() {
                                    let selected = i == self.cursor;
                                    let response = ui.selectable_label(
                                        selected,
                                        egui::RichText::new(&entry.label),
                                    );
                                    if !entry.detail.is_empty() {
                                        ui.weak(&entry.detail);
                                    }
                                    if response.clicked() {
                                        chosen = Some(entry.action.clone());
                                    }
                                    // Keep the keyboard highlight in view.
                                    if selected && (up || down) {
                                        response.scroll_to_me(None);
                                    }
                                }
                            });

                        ui.add_space(tokens.space_8);
                        ui.weak("\u{2191}\u{2193} navigate \u{00B7} Enter run \u{00B7} Esc close");
                    });
            });

        if enter && chosen.is_none() {
            chosen = entries.get(self.cursor).map(|e| e.action.clone());
        }

        if chosen.is_some() {
            self.close();
        }
        chosen
    }

    /// Build the ranked entry list for the current query.
    fn build_entries(&self, results: Option<&Results>) -> Vec<Entry> {
        let query = self.query.trim();
        let lower = query.to_lowercase();
        let mut entries: Vec<Entry> = Vec::new();

        // 1. A bare number is almost always "take me to this position".
        if let (Ok(position), Some(results)) = (query.parse::<usize>(), results) {
            if results.results.iter().any(|p| p.position == position) {
                entries.push(Entry {
                    action: PaletteAction::GoToPosition(position),
                    label: format!("Go to position {position}"),
                    detail: String::new(),
                });
            }
        }

        // 2. Sequence search: which positions carry this k-mer, and as what motif.
        if let Some(results) = results {
            if lower.len() >= MIN_SEQUENCE_QUERY && query.parse::<usize>().is_err() {
                let needle = lower.to_uppercase();
                for pos in &results.results {
                    if entries.len() >= MAX_RESULTS {
                        break;
                    }
                    let Some(variants) = pos.diversity_motifs.as_ref() else {
                        continue;
                    };
                    if let Some(v) = variants.iter().find(|v| v.sequence.contains(&needle)) {
                        entries.push(Entry {
                            action: PaletteAction::GoToPosition(pos.position),
                            label: format!(
                                "{} at position {}",
                                truncate_display(&v.sequence, 24),
                                pos.position
                            ),
                            detail: format!(
                                "{} \u{00B7} {:.1}%",
                                v.motif_long.as_deref().unwrap_or("variant"),
                                v.incidence
                            ),
                        });
                    }
                }
            }
        }

        // 3. Static commands, filtered by substring.
        let mut commands = vec![PaletteAction::OpenFile, PaletteAction::ToggleTheme];
        if results.is_some() {
            commands.extend([
                PaletteAction::ResetFilters,
                PaletteAction::ResetZoom,
                PaletteAction::BackToSetup,
            ]);
            commands.extend(ExportFormat::all().into_iter().map(PaletteAction::Export));
        }

        for action in commands {
            let label = action.label();
            if lower.is_empty() || label.to_lowercase().contains(&lower) {
                entries.push(Entry {
                    action,
                    label,
                    detail: String::new(),
                });
            }
        }

        entries.truncate(MAX_RESULTS * 2);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dima_lib::{HighestEntropy, Position, Variant};

    fn results() -> Results {
        let positions = (1..=5)
            .map(|i| Position {
                position: i * 10, // deliberately non-contiguous
                low_support: None,
                entropy: 0.5,
                support: 100,
                distinct_variants_count: 1,
                distinct_variants_incidence: 10.0,
                total_variants_incidence: 20.0,
                diversity_motifs: Some(vec![Variant {
                    sequence: if i == 3 {
                        "WQERTYUIP".to_string()
                    } else {
                        "ACDEFGHIK".to_string()
                    },
                    count: 50,
                    incidence: 50.0,
                    motif_short: Some("I".to_string()),
                    motif_long: Some("Index".to_string()),
                    metadata: None,
                }]),
            })
            .collect();

        Results {
            sequence_count: 100,
            support_threshold: 30,
            low_support_count: 0,
            query_name: "palette_test".to_string(),
            kmer_length: 9,
            highest_entropy: HighestEntropy {
                position: 10,
                entropy: 0.5,
            },
            average_entropy: 0.5,
            results: positions,
        }
    }

    fn palette_with(query: &str) -> CommandPalette {
        let mut p = CommandPalette::default();
        p.open();
        p.query = query.to_string();
        p
    }

    #[test]
    fn toggling_opens_and_closes() {
        let mut p = CommandPalette::default();
        assert!(!p.is_open());
        p.toggle();
        assert!(p.is_open());
        p.toggle();
        assert!(!p.is_open());
    }

    #[test]
    fn closing_clears_the_query() {
        let mut p = palette_with("abc");
        p.close();
        assert!(p.query.is_empty());
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn numeric_query_offers_a_jump_to_an_existing_position() {
        let r = results();
        let entries = palette_with("30").build_entries(Some(&r));
        assert_eq!(entries[0].action, PaletteAction::GoToPosition(30));
    }

    #[test]
    fn numeric_query_for_a_missing_position_offers_no_jump() {
        let r = results();
        // 33 is not one of the (10, 20, 30, 40, 50) positions.
        let entries = palette_with("33").build_entries(Some(&r));
        assert!(!entries
            .iter()
            .any(|e| matches!(e.action, PaletteAction::GoToPosition(_))));
    }

    #[test]
    fn sequence_query_finds_the_position_carrying_it() {
        let r = results();
        let entries = palette_with("wqer").build_entries(Some(&r));
        assert_eq!(
            entries.first().map(|e| e.action.clone()),
            Some(PaletteAction::GoToPosition(30)),
            "should locate the position whose variant contains the query"
        );
    }

    #[test]
    fn very_short_queries_do_not_trigger_a_sequence_scan() {
        let r = results();
        let entries = palette_with("a").build_entries(Some(&r));
        // Only command matches (e.g. none or a few), never a flood of positions.
        assert!(entries.len() <= MAX_RESULTS * 2);
        assert!(!entries.iter().any(|e| e.label.contains("at position")));
    }

    #[test]
    fn results_are_bounded() {
        let r = results();
        let entries = palette_with("ACDEF").build_entries(Some(&r));
        assert!(entries.len() <= MAX_RESULTS * 2);
    }

    #[test]
    fn data_actions_are_hidden_without_results() {
        let entries = palette_with("").build_entries(None);
        assert!(entries
            .iter()
            .all(|e| !matches!(e.action, PaletteAction::Export(_))));
        assert!(entries.iter().any(|e| e.action == PaletteAction::OpenFile));
    }

    #[test]
    fn results_are_cached_until_the_query_changes() {
        let r = results();
        let mut p = palette_with("theme");

        let first = p.entries(Some(&r)).len();
        assert_eq!(p.cached_query.as_deref(), Some("theme"));

        // Same query: served from cache, identical result.
        assert_eq!(p.entries(Some(&r)).len(), first);

        // Different query: cache must refresh rather than return stale rows.
        p.query = "zoom".to_string();
        let refreshed = p.entries(Some(&r));
        assert!(refreshed
            .iter()
            .any(|e| e.action == PaletteAction::ResetZoom));
        assert_eq!(p.cached_query.as_deref(), Some("zoom"));
    }

    #[test]
    fn cache_invalidates_when_a_different_dataset_loads() {
        let mut p = palette_with("30");
        let a = results();
        assert!(p
            .entries(Some(&a))
            .iter()
            .any(|e| e.action == PaletteAction::GoToPosition(30)));

        // A different dataset without position 30 must not keep offering it.
        let mut b = results();
        b.query_name = "other".to_string();
        b.results.retain(|pos| pos.position != 30);
        assert!(!p
            .entries(Some(&b))
            .iter()
            .any(|e| e.action == PaletteAction::GoToPosition(30)));
    }

    #[test]
    fn closing_releases_the_cache() {
        let r = results();
        let mut p = palette_with("ACDEF");
        let _ = p.entries(Some(&r));
        assert!(!p.cached.is_empty());
        p.close();
        assert!(p.cached.is_empty());
        assert!(p.cached_query.is_none());
    }

    #[test]
    fn commands_are_filtered_by_substring() {
        let r = results();
        let entries = palette_with("theme").build_entries(Some(&r));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, PaletteAction::ToggleTheme);
    }

    #[test]
    fn empty_query_lists_all_commands() {
        let r = results();
        let entries = palette_with("").build_entries(Some(&r));
        assert!(entries.iter().any(|e| e.action == PaletteAction::ResetZoom));
        assert!(entries
            .iter()
            .any(|e| e.action == PaletteAction::Export(ExportFormat::Svg)));
    }
}
