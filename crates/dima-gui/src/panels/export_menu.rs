//! The export menu: results JSON (plain, .gz or .zst by name), JSON Lines, the tidy TSV folder,
//! conserved regions (JSON, TSV), the chart (PNG with M4, SVG at full resolution; named by the
//! metric it shows), the run report and the methods text with its references. One export at a
//! time, whichever control starts it (G6); every file is written atomically by the library (G30).

use egui::Ui;

use crate::app::msg::Msg;
use crate::state::AppState;
use crate::state::session::Origin;
use crate::workers::export::ExportKind;

/// Draws the menu button.
pub fn show(ui: &mut Ui, state: &AppState, out: &mut Vec<Msg>) {
    let busy = state.export_busy;
    let dialog = state.dialog.pending.is_some();
    let analysed = state
        .session
        .as_ref()
        .is_some_and(|s| matches!(s.origin, Origin::Analysed(_)));
    let metric = state.session.as_ref().map(|s| s.metric).unwrap_or_default();
    let title = if busy.is_some() {
        "Export (busy)"
    } else {
        "Export"
    };
    ui.menu_button(title, |ui| {
        for (i, kind) in ExportKind::ALL.into_iter().enumerate() {
            if matches!(i, 3 | 5 | 7) {
                ui.separator();
            }
            let enabled = busy.is_none() && !dialog && (kind != ExportKind::RunReport || analysed);
            let response = ui
                .add_enabled(enabled, egui::Button::new(kind.label(metric)))
                .on_hover_text(super::help_text::export(kind, metric));
            let response = if kind == ExportKind::RunReport && !analysed {
                response.on_disabled_hover_text("Available for analyses run in this app")
            } else if dialog {
                response.on_disabled_hover_text("A file dialog is open")
            } else {
                response.on_disabled_hover_text("An export is running")
            };
            if response.clicked() {
                out.push(Msg::Export(kind));
                ui.close();
            }
        }
    })
    .response
    .on_hover_text("Save the results, the chart, the run report or the methods text");
}
