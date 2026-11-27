//! The variants of the selected position: searchable (k-mer or motif), filterable by minimum
//! incidence (null-aware, G33), sortable, and the selector of the variant whose metadata is
//! shown below (a click on its row, or Tab to its sequence and Space). The default variant is
//! the first Index variant in the library's order (G53).

use egui::{RichText, Ui};

use dima_core::model::Variant;

use super::common::Described;
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
                    .described("Clear the variant filters")
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
        const NAME: &str = "Minimum incidence";
        ui.label(NAME);
        let field = ui
            .add(
                egui::DragValue::new(&mut s.variants.min_incidence)
                    .suffix("%")
                    .speed(0.1)
                    .range(0.0..=100.0),
            )
            .described("Hide variants below this incidence (0 shows all)");
        // The field's text is only its value: screen readers get the label beside it as its
        // name.
        field
            .ctx
            .accesskit_node_builder(field.id, |node| node.set_label(NAME));
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
    // A click anywhere in a row selects it: the cells' text is not selectable, which would take
    // the click (copying goes through the row's menu instead); restored after the table.
    let selectable = ui.style().interaction.selectable_labels;
    ui.style_mut().interaction.selectable_labels = false;
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
        // Rows take clicks, but no cell is a keyboard stop: each row's one stop is its
        // sequence (below), not its four cells, which would show no focus.
        .sense(egui::Sense::CLICK)
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
                let [count, incidence, motif] = cells(v);
                row.col(|ui| {
                    ui.label(count);
                });
                row.col(|ui| {
                    ui.label(incidence);
                });
                row.col(|ui| {
                    ui.label(motif);
                });
                row.col(|ui| {
                    let text = RichText::new(&v.sequence)
                        .monospace()
                        .size(t.font_size_mono);
                    if !shown_whole(ui) {
                        ui.label(text);
                        return;
                    }
                    // The row's keyboard stop: Tab reaches the sequence (outlined while it has
                    // the focus), and Space or Enter selects the variant, as a click does. It
                    // has an id of its own, as a cell's changes when the table scrolls (which
                    // would drop the focus); the colour is a plain label's, which a clickable
                    // one would change.
                    let id = ui.id().with("sequence");
                    let sequence = ui
                        .scope_builder(egui::UiBuilder::new().id(id), |ui| {
                            ui.add(
                                egui::Label::new(text.color(t.text_primary))
                                    .sense(egui::Sense::click()),
                            )
                        })
                        .inner;
                    super::common::focus_outline(ui, &sequence);
                    if sequence.clicked() {
                        clicked_row = Some(i);
                    }
                    // The sequence takes its own clicks, the right one included.
                    sequence.context_menu(|ui| copy_menu(ui, v));
                });
                let response = row.response();
                if response.clicked() {
                    clicked_row = Some(i);
                }
                response.context_menu(|ui| copy_menu(ui, v));
            });
        });
    ui.style_mut().interaction.selectable_labels = selectable;
    if let Some(c) = clicked_col {
        out.push(Msg::SortVariants(c));
    }
    if let Some(i) = clicked_row {
        out.push(Msg::SelectVariant(i));
    }
}

/// The count, incidence and motif cells of a variant's row, as the table shows them.
fn cells(v: &Variant) -> [String; 3] {
    [
        v.count.to_string(),
        dima_core::human::percent_bounded(v.incidence, 2),
        v.motif.long().to_owned(),
    ]
}

/// A variant's row menu: "Copy k-mer", and "Copy row" (its cells as the table shows them,
/// separated by tabs, so that it pastes into a spreadsheet as one row).
fn copy_menu(ui: &mut Ui, v: &Variant) {
    if ui.button("Copy k-mer").clicked() {
        ui.ctx().copy_text(v.sequence.clone());
        ui.close();
    }
    if ui.button("Copy row").clicked() {
        let [count, incidence, motif] = cells(v);
        ui.ctx()
            .copy_text([count, incidence, motif, v.sequence.clone()].join("\t"));
        ui.close();
    }
}

/// Whether the table shows this cell's row whole. Only such rows are keyboard stops: the table
/// also lays out rows it shows in part or not at all (at its edges), whose focus would be hidden.
fn shown_whole(ui: &Ui) -> bool {
    let (cell, clip) = (ui.max_rect(), ui.clip_rect());
    clip.top() <= cell.top() && cell.bottom() <= clip.bottom()
}
