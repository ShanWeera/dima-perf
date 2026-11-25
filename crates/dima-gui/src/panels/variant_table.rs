//! The variants of the selected position: searchable (k-mer or motif), filterable by minimum
//! incidence (null-aware, G33), sortable, and the selector of the variant whose metadata is
//! shown below. The default variant is the first Index variant in the library's order (G53).

use egui::{RichText, Ui};

use crate::app::msg::Msg;
use crate::state::session::Session;
use crate::state::sort::variant_columns as col;
use crate::theme::DesignTokens;

/// Draws the tools and the table.
pub fn show(ui: &mut Ui, s: &mut Session, t: &DesignTokens, out: &mut Vec<Msg>) {
    let Some(total) = s.selected().map(|p| p.diversity_motifs.len()) else {
        return;
    };
    ui.horizontal(|ui| {
        super::common::sub_heading(ui, "Variants");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let filtering = !s.variants.search.is_empty() || s.variants.min_incidence > 0.0;
            if filtering
                && ui
                    .small_button("Clear")
                    .on_hover_text("Clear the variant filters")
                    .clicked()
            {
                s.variants.search.clear();
                s.variants.min_incidence = 0.0;
            }
        });
    });
    if total == 0 {
        ui.colored_label(
            t.text_muted,
            "No variants: no sequence has a valid k-mer here",
        );
        return;
    }
    ui.add(
        egui::TextEdit::singleline(&mut s.variants.search)
            .hint_text("Search k-mer or motif")
            .desired_width(f32::INFINITY),
    );
    ui.horizontal(|ui| {
        ui.label("Minimum incidence");
        ui.add(
            egui::DragValue::new(&mut s.variants.min_incidence)
                .suffix("%")
                .speed(0.1)
                .range(0.0..=100.0),
        )
        .on_hover_text("Hide variants below this incidence (0 shows all)");
    });
    let visible = s.visible_variants();
    let visible = &*visible;
    if visible.len() < total {
        ui.weak(format!("{} of {total} shown", visible.len()));
    } else {
        ui.weak(dima_core::human::plural(
            total as u64,
            "variant",
            "variants",
        ));
    }
    let Some(p) = s.selected() else {
        return;
    };
    let variants = &p.diversity_motifs;
    let sort = s.variants.sort;
    let selected = s.selection.variant;
    let height = (ui.ctx().content_rect().height() * 0.3).clamp(140.0, 380.0);
    let mut clicked_col = None;
    let mut clicked_row = None;
    egui_extras::TableBuilder::new(ui)
        .id_salt("variants")
        .striped(true)
        .resizable(true)
        .auto_shrink([false, true])
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(egui_extras::Column::auto().at_least(54.0))
        .column(egui_extras::Column::auto().at_least(70.0))
        .column(egui_extras::Column::auto().at_least(56.0))
        .column(egui_extras::Column::remainder().at_least(90.0).clip(true))
        .max_scroll_height(height)
        .sense(egui::Sense::click())
        .header(22.0, |mut header| {
            for (i, name) in col::NAMES.iter().enumerate() {
                header.col(|ui| {
                    let arrow = sort
                        .filter(|s| s.column == i)
                        .map_or("", |s| s.direction.arrow());
                    if super::common::sort_header(ui, name, &name.to_lowercase(), arrow).clicked() {
                        clicked_col = Some(i);
                    }
                });
            }
        })
        .body(|body| {
            body.rows(22.0, visible.len(), |mut row| {
                let i = visible[row.index()];
                let v = &variants[i];
                row.set_selected(selected == Some(i));
                row.col(|ui| {
                    ui.label(v.count.to_string());
                });
                row.col(|ui| {
                    ui.label(dima_core::human::percent_bounded(v.incidence, 2));
                });
                row.col(|ui| {
                    ui.label(v.motif.long());
                });
                row.col(|ui| {
                    ui.label(
                        RichText::new(&v.sequence)
                            .monospace()
                            .size(t.font_size_mono),
                    );
                });
                if row.response().clicked() {
                    clicked_row = Some(i);
                }
            });
        });
    if let Some(c) = clicked_col {
        out.push(Msg::SortVariants(c));
    }
    if let Some(i) = clicked_row {
        out.push(Msg::SelectVariant(i));
    }
}
