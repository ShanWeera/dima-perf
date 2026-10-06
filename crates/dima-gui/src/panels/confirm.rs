//! The confirmation dialog for questions raised by `update`: replacing a running analysis
//! (G35), showing a position hidden by the filters (G20), closing during an export and
//! replacing a file whose name got its extension after the save dialog.

use crate::app::msg::Msg;
use crate::state::Confirm;

/// Draws the question when one is pending.
pub fn show(ctx: &egui::Context, confirm: Option<&Confirm>, out: &mut Vec<Msg>) {
    let Some(c) = confirm else {
        return;
    };
    let (title, text, yes) = match c {
        Confirm::ReplaceRun(path) => (
            "An analysis is running",
            format!(
                "Cancel it and open {}? Nothing of the running analysis is kept.",
                dima_core::run_report::file_name(path)
            ),
            "Cancel analysis and open",
        ),
        Confirm::ClearFiltersFor(p) => (
            "Position hidden by the filters",
            format!(
                "Position {p} is hidden by the current filters. Show every position and select it?"
            ),
            "Reset filters and select",
        ),
        Confirm::Overwrite { path, .. } => (
            "Replace the file?",
            format!(
                "{} already exists in {}. Replace it?",
                dima_core::run_report::file_name(path),
                path.parent()
                    .map_or_else(String::new, |p| p.display().to_string())
            ),
            "Replace",
        ),
        Confirm::QuitDuringExport => (
            "An export is being saved",
            "Quit now? The file being saved will not be written. Keep DiMA open to let the \
             export finish."
                .to_owned(),
            "Quit without saving it",
        ),
    };
    let modal = egui::Modal::new(egui::Id::new("dima_confirm")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading(title);
        ui.add_space(6.0);
        ui.label(text);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(yes).clicked() {
                out.push(Msg::Confirmed(true));
            }
            if ui.button("Keep as is").clicked() {
                out.push(Msg::Confirmed(false));
            }
        });
    });
    if modal.should_close() {
        out.push(Msg::Confirmed(false));
    }
}
