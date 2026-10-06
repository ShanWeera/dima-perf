//! The setup screen: input and pre-check, options, header fields, run, recent files.

use super::UiState;
use crate::app::msg::Msg;
use crate::panels::{analysis_options, header_editor, input, recent_files, run};
use crate::state::AppState;
use crate::theme::DesignTokens;

/// Width of the setup column.
const COLUMN: f32 = 760.0;

/// Draws the screen.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    u: &mut UiState,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("setup_scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                let width = ui.available_width().min(COLUMN);
                let margin = ((ui.available_width() - width) / 2.0).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add_space(t.space_16);
                        input::show(ui, state, t, out);
                        ui.add_space(t.space_12);
                        analysis_options::show(ui, state, t, out);
                        ui.add_space(t.space_12);
                        header_editor::show(ui, state, &mut u.paste, t, out);
                        ui.add_space(t.space_12);
                        run::show(ui, state, u.progress.as_ref(), t, out);
                        ui.add_space(t.space_12);
                        recent_files::show(ui, &state.recent, t, out);
                        ui.add_space(t.space_24);
                    });
                });
            });
    });
}
