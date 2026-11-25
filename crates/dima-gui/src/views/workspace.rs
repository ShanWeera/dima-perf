//! The results screen: header chips and filters across the top, the inspector on the right,
//! and the entropy chart, conserved-region strip and position table in the centre.

use super::UiState;
use crate::app::msg::Msg;
use crate::panels::{
    entropy_chart, filter_bar, hcs_strip, metadata, motif_composition, position_explorer,
    position_summary, summary, variant_table,
};
use crate::state::AppState;
use crate::theme::DesignTokens;

/// Width of the inspector.
const INSPECTOR_WIDTH: f32 = 360.0;

/// Draws the screen.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    u: &mut UiState,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    let Some(session) = state.session.as_mut() else {
        return;
    };
    egui::Panel::top("results_header").show(ui, |ui| {
        ui.add_space(t.space_4);
        summary::show(ui, session, t);
        ui.add_space(t.space_4);
    });
    egui::Panel::top("results_filters").show(ui, |ui| {
        ui.add_space(t.space_4);
        filter_bar::show(ui, session, t, out);
        ui.add_space(t.space_4);
    });
    egui::Panel::right("inspector")
        .resizable(false)
        .exact_size(INSPECTOR_WIDTH)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let threshold = session.results.support_threshold;
                    let estimator = session.results.run.parameters.estimator.value;
                    let similarity = session
                        .results
                        .run
                        .parameters
                        .similarity_weighted_entropy
                        .value;
                    position_summary::show(
                        ui,
                        session.selected(),
                        threshold,
                        estimator,
                        similarity,
                        t,
                    );
                    if let Some(p) = session.selected() {
                        motif_composition::show(ui, &p.diversity_motifs, t);
                        ui.add_space(t.space_8);
                        ui.separator();
                        variant_table::show(ui, session, t, out);
                        if !session.fields.is_empty() {
                            ui.add_space(t.space_8);
                            ui.separator();
                            metadata::show(ui, session, t);
                        }
                    }
                });
        });
    egui::CentralPanel::default().show(ui, |ui| {
        // In a small window the chart, the conserved regions and the table do not all fit: the
        // column then scrolls, and the table keeps at least four rows (the chart keeps the
        // wheel for its zoom).
        egui::ScrollArea::vertical()
            .id_salt("results_column")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let Some(session) = state.session.as_mut() else {
                    return;
                };
                let shared = entropy_chart::show(ui, session, &mut u.chart, u.gpu, t, out);
                ui.add_space(t.space_8);
                hcs_strip::show(ui, state, shared, t, out);
                ui.add_space(t.space_8);
                if let Some(session) = state.session.as_mut() {
                    position_explorer::show(ui, session, t, out);
                }
            });
    });
}
