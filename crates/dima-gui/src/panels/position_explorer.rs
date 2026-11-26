//! The position table: every shown position, sortable by every column but coverage
//! (null-aware, G10), with the selection highlighted and kept in view (G46) and the chart's
//! hovered position mirrored. Rows are chosen with a click or the arrow keys; the cells are no
//! keyboard stops. Only visible rows are laid out. When the run computed it, similarity-weighted
//! entropy has
//! its column right after entropy. The table scrolls sideways when its columns do not fit; a
//! header that gains the keyboard focus scrolls into view, sideways and in the column around
//! the table. Each set of columns is its own table for egui, so widths dragged for one set
//! are kept for it and never applied to the columns of another.

use egui::{RichText, Ui};

use dima_core::model::{PositionResult, Results};

use crate::app::msg::Msg;
use crate::charts::series::Class;
use crate::state::session::{SelectSource, Session};
use crate::state::sort::position_columns as col;
use crate::theme::DesignTokens;
use crate::util::{opt_coverage, opt_f64, opt_percent};

/// A column of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Column {
    Position,
    Entropy,
    SimilarityWeighted,
    Support,
    Class,
    TotalVariants,
    Coverage,
    TopVariant,
}

impl Column {
    /// The columns shown, in order: similarity-weighted entropy right after entropy when the
    /// run computed it, coverage when the run set a minimum coverage, the top variant last.
    fn shown(similarity_weighted: bool, coverage: bool) -> Vec<Self> {
        let mut c = vec![Self::Position, Self::Entropy];
        if similarity_weighted {
            c.push(Self::SimilarityWeighted);
        }
        c.extend([Self::Support, Self::Class, Self::TotalVariants]);
        if coverage {
            c.push(Self::Coverage);
        }
        c.push(Self::TopVariant);
        c
    }

    /// The header.
    const fn header(self) -> &'static str {
        match self {
            Self::Position => "Position",
            Self::Entropy => "Entropy",
            Self::SimilarityWeighted => "Similarity-weighted",
            Self::Support => "Support",
            Self::Class => "Class",
            Self::TotalVariants => "Variants %",
            Self::Coverage => "Coverage",
            Self::TopVariant => "Top variant",
        }
    }

    /// What sorting by the column orders by ("Sort by …") and its sort id; `None` for the
    /// coverage column, which is not sortable.
    const fn sort(self) -> Option<(&'static str, usize)> {
        match self {
            Self::Position => Some(("position", col::POSITION)),
            Self::Entropy => Some(("entropy", col::ENTROPY)),
            Self::SimilarityWeighted => {
                Some(("similarity-weighted entropy", col::SIMILARITY_WEIGHTED))
            }
            Self::Support => Some(("support", col::SUPPORT)),
            Self::Class => Some(("class", col::CLASS)),
            Self::TotalVariants => Some(("variants %", col::TOTAL_VARIANTS)),
            Self::Coverage => None,
            Self::TopVariant => Some(("top variant", col::TOP_VARIANT)),
        }
    }

    /// The narrowest the column gets.
    const fn min_width(self) -> f32 {
        match self {
            Self::Position | Self::Coverage => 70.0,
            Self::Entropy | Self::SimilarityWeighted | Self::TotalVariants => 80.0,
            Self::Support => 64.0,
            Self::Class => 60.0,
            Self::TopVariant => 120.0,
        }
    }
}

/// Draws the table.
pub fn show(ui: &mut Ui, s: &mut Session, t: &DesignTokens, out: &mut Vec<Msg>) {
    if s.rows.is_empty() {
        ui.colored_label(
            t.text_muted,
            if s.results.results.is_empty() {
                "These results contain no positions"
            } else {
                "No positions match the current filters \u{2014} Reset shows all"
            },
        );
        return;
    }
    let columns = Column::shown(
        s.results.run.parameters.similarity_weighted_entropy.value,
        s.results.coverage_summary.min_coverage.is_some(),
    );
    let scroll_to = if s.selection.scroll_to_row {
        s.selection.position.and_then(|p| s.row_of_position(p))
    } else {
        None
    };
    s.selection.scroll_to_row = false;
    let mut clicked_col = None;
    let mut clicked_row = None;
    let mut focused_header = None;
    egui::ScrollArea::horizontal()
        .id_salt("positions_columns")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let row_height = 22.0;
            let height = ui.available_height().max(row_height * 4.0);
            let mut builder = egui_extras::TableBuilder::new(ui)
                .id_salt(table_salt(&columns))
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
            for (i, c) in columns.iter().enumerate() {
                builder = builder.column(if i + 1 == columns.len() {
                    egui_extras::Column::remainder()
                        .at_least(c.min_width())
                        .clip(true)
                } else {
                    egui_extras::Column::auto().at_least(c.min_width())
                });
            }
            // Rows take clicks, but no cell is a keyboard stop (Tab would otherwise stop at
            // every cell, with no visible focus): the arrow keys move along the rows.
            builder = builder.max_scroll_height(height).sense(egui::Sense::CLICK);
            if let Some(row) = scroll_to {
                builder = builder.scroll_to_row(row, Some(egui::Align::Center));
            }
            let sort = s.sort;
            let selected = s.selection.position;
            let hovered = s.hovered;
            let r = &s.results;
            let rows = &s.rows;
            builder
                .header(row_height, |mut header| {
                    for c in &columns {
                        header.col(|ui| match c.sort() {
                            Some((sort_name, id)) => {
                                let arrow = match sort {
                                    Some(s) if s.column == id => s.direction.arrow(),
                                    _ => "",
                                };
                                let h =
                                    super::common::sort_header(ui, c.header(), sort_name, arrow);
                                if h.gained_focus() {
                                    focused_header = Some(h.rect);
                                }
                                if h.clicked() {
                                    clicked_col = Some(id);
                                }
                            }
                            None => {
                                ui.label(RichText::new(c.header()).strong());
                            }
                        });
                    }
                })
                .body(|body| {
                    body.rows(row_height, rows.len(), |mut row| {
                        let p = &r.results[rows[row.index()]];
                        let is_selected = selected == Some(p.position);
                        let is_hovered = hovered == Some(p.position);
                        row.set_selected(is_selected || is_hovered);
                        for &c in &columns {
                            row.col(|ui| cell(ui, c, p, r, t, is_hovered && !is_selected));
                        }
                        if row.response().clicked() {
                            clicked_row = Some(p.position);
                        }
                    });
                });
        });
    // The header scrolled itself into view sideways; the scrolling column around the table
    // needs to be told separately (each scroll area takes what is asked inside it).
    if let Some(rect) = focused_header {
        ui.scroll_to_rect(rect, None);
    }
    if let Some(c) = clicked_col {
        out.push(Msg::SortPositions(c));
    }
    if let Some(p) = clicked_row {
        out.push(Msg::Select(p, SelectSource::Table));
    }
}

/// The id salt of the table showing `columns`: egui keeps dragged column widths per table and
/// discards them only when the number of columns changes, so two sets of as many columns (with
/// similarity-weighted entropy, or with coverage) would otherwise share their widths.
fn table_salt(columns: &[Column]) -> (&'static str, &[Column]) {
    ("positions", columns)
}

/// One cell; `hovered` italicises the position of a row the chart hovers.
fn cell(ui: &mut Ui, c: Column, p: &PositionResult, r: &Results, t: &DesignTokens, hovered: bool) {
    match c {
        Column::Position => {
            let text = RichText::new(p.position.to_string());
            ui.label(if hovered { text.italics() } else { text });
        }
        Column::Entropy => {
            ui.label(opt_f64(p.entropy, 4));
        }
        Column::SimilarityWeighted => {
            ui.label(opt_f64(p.similarity_weighted_entropy, 4));
        }
        Column::Support => {
            ui.label(p.support.to_string());
        }
        Column::Class => match Class::of(p) {
            Class::Eligible => {
                ui.label("\u{2714}")
                    .on_hover_text(format!("eligible: support > {}", r.support_threshold));
            }
            class => {
                ui.colored_label(t.warning_color, class.short())
                    .on_hover_text(class.describe(
                        r.support_threshold,
                        r.run.parameters.estimator.value,
                        p.entropy_method,
                    ));
            }
        },
        Column::TotalVariants => {
            ui.label(opt_percent(p.total_variants_incidence, 1));
        }
        Column::Coverage => {
            let text = opt_coverage(p.sample_coverage, 3);
            if p.below_min_coverage == Some(true) {
                // The marker's shape is the cue: the chart's purple fails text contrast on
                // light and selected rows (WCAG 2.2 SC 1.4.3).
                ui.label(format!("{text} \u{25BC}"))
                    .on_hover_text("Below the minimum coverage of this run");
            } else {
                ui.label(text);
            }
        }
        Column::TopVariant => {
            if let Some(v) = p.diversity_motifs.first() {
                ui.label(
                    RichText::new(format!(
                        "{}  {}",
                        v.sequence,
                        dima_core::human::percent_bounded(v.incidence, 1)
                    ))
                    .monospace()
                    .size(t.font_size_mono),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Column, col};

    /// Similarity-weighted entropy right after entropy, coverage (not sortable) before the top
    /// variant, which is always last; the sort ids and names of the older columns are as they
    /// were.
    #[test]
    fn columns_follow_the_run() {
        use Column::*;
        assert_eq!(
            Column::shown(false, false),
            [Position, Entropy, Support, Class, TotalVariants, TopVariant]
        );
        assert_eq!(
            Column::shown(true, true),
            [
                Position,
                Entropy,
                SimilarityWeighted,
                Support,
                Class,
                TotalVariants,
                Coverage,
                TopVariant
            ]
        );
        assert_eq!(Coverage.sort(), None);
        assert_eq!(
            SimilarityWeighted.sort(),
            Some(("similarity-weighted entropy", col::SIMILARITY_WEIGHTED))
        );
        for c in Column::shown(true, true) {
            if let Some((name, _)) = c.sort().filter(|_| c != SimilarityWeighted) {
                assert_eq!(name, c.header().to_lowercase(), "unchanged tooltip text");
            }
        }
        assert_eq!(TopVariant.sort(), Some(("top variant", col::TOP_VARIANT)));
    }

    /// Two sets of seven columns are two tables, each with its own widths; a set is always the
    /// same table.
    #[test]
    fn each_set_of_columns_is_its_own_table() {
        let id = |similarity, coverage| {
            egui::Id::new(super::table_salt(&Column::shown(similarity, coverage)))
        };
        assert_eq!(
            Column::shown(true, false).len(),
            Column::shown(false, true).len()
        );
        assert_ne!(id(true, false), id(false, true));
        assert_ne!(id(false, false), id(true, true));
        assert_eq!(id(true, true), id(true, true));
    }
}
