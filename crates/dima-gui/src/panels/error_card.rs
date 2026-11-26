//! An error card (D85): what failed, where (record, line, column, byte), the offending header
//! and character, the hint, "no results were written" for runs, and "Copy details".

use egui::{RichText, Ui};

use crate::app::msg::Msg;
use crate::state::ErrorCard;
use crate::theme::DesignTokens;

/// Draws a card. `run` adds the run-specific lines and actions.
pub fn show(ui: &mut Ui, card: &ErrorCard, t: &DesignTokens, out: &mut Vec<Msg>, run: bool) {
    egui::Frame::new()
        .fill(t.surface_primary)
        .stroke(egui::Stroke::new(1.5, t.error_color))
        .corner_radius(t.panel_rounding)
        .inner_margin(t.space_12)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(format!("Error: {}", card.title))
                    .strong()
                    .color(t.error_color),
            );
            ui.add_space(t.space_4);
            ui.add(egui::Label::new(card.error.to_string()).wrap());
            if let Some(l) = &card.location {
                ui.add_space(t.space_4);
                egui::Grid::new("error_location")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (k, v) in [
                            ("Record", l.record),
                            ("Line", l.line),
                            ("Column", l.column),
                            ("Byte", l.byte),
                        ] {
                            if let Some(v) = v {
                                ui.label(RichText::new(k).color(t.text_secondary));
                                ui.monospace(v.to_string());
                                ui.end_row();
                            }
                        }
                        if let Some(e) = &card.excerpt {
                            ui.label(RichText::new("Header").color(t.text_secondary));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(">{}", e.header)).monospace(),
                                )
                                .wrap(),
                            );
                            ui.end_row();
                            if let Some(c) = &e.around_column {
                                ui.label(RichText::new("Sequence").color(t.text_secondary));
                                ui.monospace(c);
                                ui.end_row();
                            }
                        } else if card.excerpt_pending.is_some() {
                            ui.label(RichText::new("Header").color(t.text_secondary));
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak("reading the record\u{2026}");
                            });
                            ui.end_row();
                        }
                    });
            }
            if let Some(h) = card.error.hint() {
                ui.add_space(t.space_4);
                ui.add(
                    egui::Label::new(RichText::new(format!("Hint: {h}")).color(t.text_secondary))
                        .wrap(),
                );
            }
            if run {
                ui.add_space(t.space_4);
                ui.label(RichText::new("No results were written.").strong());
            }
            ui.add_space(t.space_8);
            ui.horizontal(|ui| {
                if ui.button("Copy details").clicked() {
                    ui.ctx().copy_text(card.details());
                    out.push(Msg::Notify(
                        crate::state::toasts::Severity::Info,
                        "Error details copied".to_owned(),
                    ));
                }
                if run
                    && card.failed_run.is_some()
                    && ui
                        .button("Save run report\u{2026}")
                        .on_hover_text(super::help_text::export(
                            crate::workers::export::ExportKind::RunReport,
                            crate::charts::metric::ChartMetric::Entropy,
                        ))
                        .clicked()
                {
                    out.push(Msg::SaveFailedRunReport);
                }
                if ui.button("Dismiss").clicked() {
                    out.push(Msg::DismissFailure);
                }
            });
        });
}
