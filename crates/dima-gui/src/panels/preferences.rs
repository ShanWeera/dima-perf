//! The preferences window: settings of the app rather than of an analysis, the file dialog
//! (D100) and the size of exported figures. Changes are remembered at once (D73). Every
//! control's tooltip is also its screen-reader description, and the two size fields are named
//! apart (width, height).

use crate::app::msg::Msg;
use crate::panels::common::{named, with_help};
use crate::state::settings::{DialogMode, FIGURE_HEIGHT, FIGURE_WIDTH, Settings};

/// The screen-reader names of the figure size fields.
pub const FIGURE_WIDTH_NAME: &str = "Width of exported figures";
/// See [`FIGURE_WIDTH_NAME`].
pub const FIGURE_HEIGHT_NAME: &str = "Height of exported figures";

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
                        let native = ui.radio_value(
                            &mut settings.dialogs,
                            DialogMode::Native,
                            "The system's file dialog",
                        );
                        changed |= with_help(
                            native,
                            "Falls back to DiMA's own browser when the system dialog is not \
                             available",
                        )
                        .changed();
                        let built_in = ui.radio_value(
                            &mut settings.dialogs,
                            DialogMode::BuiltIn,
                            "DiMA's built-in browser",
                        );
                        changed |= with_help(
                            built_in,
                            "For remote desktops and systems whose file dialog does not respond",
                        )
                        .changed();
                    });
                    ui.end_row();
                    ui.label("Exported figures");
                    ui.horizontal(|ui| {
                        let (w, h) = &mut settings.figure_size;
                        let width =
                            ui.add(egui::DragValue::new(w).range(FIGURE_WIDTH).suffix(" px"));
                        changed |= with_help(
                            named(width, FIGURE_WIDTH_NAME),
                            &format!(
                                "Width of the PNG and SVG chart exports: at least {} px, so \
                                 the title, legend and axes fit",
                                FIGURE_WIDTH.start()
                            ),
                        )
                        .changed();
                        ui.label("\u{D7}");
                        let height =
                            ui.add(egui::DragValue::new(h).range(FIGURE_HEIGHT).suffix(" px"));
                        changed |= with_help(
                            named(height, FIGURE_HEIGHT_NAME),
                            &format!(
                                "Height of the PNG and SVG chart exports: at least {} px, so \
                                 the title, legend and axes fit",
                                FIGURE_HEIGHT.start()
                            ),
                        )
                        .changed();
                    });
                    ui.end_row();
                });
            if changed {
                out.push(Msg::OptionsChanged);
            }
        });
}
