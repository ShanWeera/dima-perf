//! The position table: every shown position, sortable by any column (null-aware, G10), with
//! the selection highlighted and kept in view (G46) and the chart's hovered position mirrored.
//! Only visible rows are laid out.

use egui::{RichText, Ui};

use crate::app::msg::Msg;
use crate::charts::series::Class;
use crate::state::session::{SelectSource, Session};
use crate::state::sort::position_columns as col;
use crate::theme::DesignTokens;
use crate::util::{opt_coverage, opt_f64, opt_percent};

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
    let row_height = 22.0;
    let coverage = s.results.coverage_summary.min_coverage.is_some();
    let scroll_to = if s.selection.scroll_to_row {
        s.selection.position.and_then(|p| s.row_of_position(p))
    } else {
        None
    };
    s.selection.scroll_to_row = false;
    let height = ui.available_height().max(row_height * 4.0);
    let mut builder = egui_extras::TableBuilder::new(ui)
        .id_salt("positions")
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false])
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(egui_extras::Column::auto().at_least(70.0))
        .column(egui_extras::Column::auto().at_least(80.0))
        .column(egui_extras::Column::auto().at_least(64.0))
        .column(egui_extras::Column::auto().at_least(60.0))
        .column(egui_extras::Column::auto().at_least(80.0));
    if coverage {
        builder = builder.column(egui_extras::Column::auto().at_least(70.0));
    }
    builder = builder
        .column(egui_extras::Column::remainder().at_least(120.0).clip(true))
        .max_scroll_height(height)
        .sense(egui::Sense::click());
    if let Some(row) = scroll_to {
        builder = builder.scroll_to_row(row, Some(egui::Align::Center));
    }
    let sort = s.sort;
    let mut clicked_col = None;
    let mut clicked_row = None;
    let selected = s.selection.position;
    let hovered = s.hovered;
    let r = &s.results;
    let rows = &s.rows;
    let mut names: Vec<&str> = col::NAMES[..col::TOP_VARIANT].to_vec();
    if coverage {
        names.push("Coverage");
    }
    names.push(col::NAMES[col::TOP_VARIANT]);
    builder
        .header(row_height, |mut header| {
            for (i, name) in names.iter().enumerate() {
                header.col(|ui| {
                    // The coverage column is not sortable; it maps to no sort column.
                    let column = column_of(i, coverage);
                    let arrow = match (sort, column) {
                        (Some(s), Some(c)) if s.column == c => s.direction.arrow(),
                        _ => "",
                    };
                    if let Some(c) = column {
                        if super::common::sort_header(ui, name, arrow).clicked() {
                            clicked_col = Some(c);
                        }
                    } else {
                        ui.label(RichText::new(*name).strong());
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
                row.col(|ui| {
                    let text = RichText::new(p.position.to_string());
                    ui.label(if is_hovered && !is_selected {
                        text.italics()
                    } else {
                        text
                    });
                });
                row.col(|ui| {
                    ui.label(opt_f64(p.entropy, 4));
                });
                row.col(|ui| {
                    ui.label(p.support.to_string());
                });
                row.col(|ui| match Class::of(p) {
                    Class::Eligible => {
                        ui.label("\u{2714}")
                            .on_hover_text(format!("eligible: support > {}", r.support_threshold));
                    }
                    c => {
                        ui.colored_label(t.warning_color, c.short())
                            .on_hover_text(c.describe(
                                r.support_threshold,
                                r.run.parameters.estimator.value,
                                p.entropy_method,
                            ));
                    }
                });
                row.col(|ui| {
                    ui.label(opt_percent(p.total_variants_incidence, 1));
                });
                if coverage {
                    row.col(|ui| {
                        let text = opt_coverage(p.sample_coverage, 3);
                        if p.below_min_coverage == Some(true) {
                            // The marker's shape is the cue: the chart's purple fails text
                            // contrast on light and selected rows (WCAG 2.2 SC 1.4.3).
                            ui.label(format!("{text} \u{25BC}"))
                                .on_hover_text("Below the minimum coverage of this run");
                        } else {
                            ui.label(text);
                        }
                    });
                }
                row.col(|ui| {
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
                });
                if row.response().clicked() {
                    clicked_row = Some(p.position);
                }
            });
        });
    if let Some(c) = clicked_col {
        out.push(Msg::SortPositions(c));
    }
    if let Some(p) = clicked_row {
        out.push(Msg::Select(p, SelectSource::Table));
    }
}

/// The sort column of a displayed column (`None` for the unsortable coverage column).
fn column_of(display: usize, coverage: bool) -> Option<usize> {
    match (coverage, display) {
        (false, i) => Some(i),
        (true, i) if i < col::TOP_VARIANT => Some(i),
        (true, i) if i == col::TOP_VARIANT => None,
        (true, _) => Some(col::TOP_VARIANT),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn coverage_column_is_not_sortable() {
        use super::col;
        assert_eq!(super::column_of(1, true), Some(col::ENTROPY));
        assert_eq!(super::column_of(col::TOP_VARIANT, true), None);
        assert_eq!(
            super::column_of(col::TOP_VARIANT + 1, true),
            Some(col::TOP_VARIANT)
        );
        assert_eq!(
            super::column_of(col::TOP_VARIANT, false),
            Some(col::TOP_VARIANT)
        );
    }
}
