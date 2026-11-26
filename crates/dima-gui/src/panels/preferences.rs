//! The preferences window: settings of the app rather than of an analysis, the file dialog
//! (D100) and the size of exported figures. Changes are remembered at once (D73).

use crate::app::msg::Msg;
use crate::state::settings::{DialogMode, FIGURE_HEIGHT, FIGURE_WIDTH, Settings};

/// Draws the window when open.
pub fn window(ctx: &egui::Context, open: &mut bool, settings: &mut Settings, out: &mut Vec<Msg>) {
    egui::Window::new("Preferences")
        .open(open)
        .resizable(false)
        .show(ctx, |ui| {
            let mut changed = false;
            egui::Grid::new("preferences")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("File dialogs");
                    ui.vertical(|ui| {
                        changed |= ui
                            .radio_value(
                                &mut settings.dialogs,
                                DialogMode::Native,
                                "The system's file dialog",
                            )
                            .on_hover_text(
                                "Falls back to DiMA's own browser when the system dialog is not \
                                 available",
                            )
                            .changed();
                        changed |= ui
                            .radio_value(
                                &mut settings.dialogs,
                                DialogMode::BuiltIn,
                                "DiMA's built-in browser",
                            )
                            .on_hover_text(
                                "For remote desktops and systems whose file dialog does not \
                                 respond",
                            )
                            .changed();
                    });
                    ui.end_row();
                    let figures = ui.label("Exported figures").id;
                    ui.horizontal(|ui| {
                        let (w, h) = &mut settings.figure_size;
                        changed |= ui
                            .add(egui::DragValue::new(w).range(FIGURE_WIDTH).suffix(" px"))
                            .labelled_by(figures)
                            .on_hover_text(format!(
                                "Width of the PNG and SVG chart exports: at least {} px, so \
                                 the title, legend and axes fit",
                                FIGURE_WIDTH.start()
                            ))
                            .changed();
                        ui.label("\u{D7}");
                        changed |= ui
                            .add(egui::DragValue::new(h).range(FIGURE_HEIGHT).suffix(" px"))
                            .labelled_by(figures)
                            .on_hover_text(format!(
                                "Height of the PNG and SVG chart exports: at least {} px, so \
                                 the title, legend and axes fit",
                                FIGURE_HEIGHT.start()
                            ))
                            .changed();
                    });
                    ui.end_row();
                });
            if changed {
                out.push(Msg::OptionsChanged);
            }
        });
}
