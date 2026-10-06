//! The top bar of both screens: screen switch (setup ↔ results, G7), export menu, run
//! details, diagnostics badge (D60), help (Cite DiMA, D89; shortcuts; about) and theme.

use egui::{RichText, Ui};

use dima_core::diagnostics::Severity as DiagSeverity;

use super::export_menu;
use crate::app::msg::Msg;
use crate::state::{AppState, View};
use crate::theme::DesignTokens;

/// Warnings and notes of the loaded results (run and conserved regions).
#[must_use]
pub fn diagnostic_counts(state: &AppState) -> (usize, usize) {
    let Some(s) = state.session.as_ref() else {
        return (0, 0);
    };
    let hcs = state.hcs.current(s.id);
    let all = s
        .results
        .run
        .diagnostics
        .iter()
        .chain(hcs.iter().flat_map(|h| &h.diagnostics));
    let (mut warnings, mut notes) = (0, 0);
    for d in all {
        match d.severity {
            DiagSeverity::Warning => warnings += 1,
            DiagSeverity::Info => notes += 1,
        }
    }
    (warnings, notes)
}

/// "Waiting for the system file dialog…" with the way out when it never appears (D100).
pub fn waiting_for_dialog(ui: &mut Ui, out: &mut Vec<Msg>) {
    ui.spinner();
    ui.label("Waiting for the system file dialog\u{2026}");
    if ui
        .button("Use built-in browser")
        .on_hover_text("Use DiMA's own file browser if the system dialog does not appear")
        .clicked()
    {
        out.push(Msg::UseBuiltInBrowser);
    }
}

/// Draws the bar.
pub fn show(ui: &mut Ui, state: &mut AppState, t: &DesignTokens, out: &mut Vec<Msg>) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.label(
            RichText::new("DiMA")
                .size(t.font_size_title)
                .family(egui::FontFamily::Name(crate::theme::fonts::SEMIBOLD.into())),
        );
        ui.add_space(t.space_12);
        if ui
            .selectable_label(state.view == View::Setup, "Setup")
            .clicked()
        {
            out.push(Msg::ShowView(View::Setup));
        }
        let has_results = state.session.is_some();
        let results = ui
            .add_enabled(
                has_results,
                egui::Button::selectable(state.view == View::Workspace, "Results"),
            )
            .on_disabled_hover_text("Run an analysis or open a results file first");
        if results.clicked() {
            out.push(Msg::ShowView(View::Workspace));
        }
        ui.add_space(t.space_8);
        let dialog = state.dialog.pending.is_some();
        if ui
            .add_enabled(!dialog, egui::Button::new("Open\u{2026}"))
            .on_disabled_hover_text("A file dialog is open")
            .clicked()
        {
            out.push(Msg::OpenDialog);
        }
        if has_results {
            export_menu::show(ui, state, out);
        }
        // The setup screen shows this next to its own Open button.
        if dialog && state.dialog.native && state.view != View::Setup {
            waiting_for_dialog(ui, out);
        }
        ui.menu_button("Help", |ui| {
            if ui.button("Cite DiMA\u{2026}").clicked() {
                state.windows.cite = true;
                ui.close();
            }
            if ui.button("Keyboard shortcuts").clicked() {
                state.windows.shortcuts = true;
                ui.close();
            }
            if ui.button("About DiMA\u{2026}").clicked() {
                state.windows.about = true;
                ui.close();
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::widgets::global_theme_preference_switch(ui);
            if ui
                .button("Preferences")
                .on_hover_text("File dialogs and the size of exported figures")
                .clicked()
            {
                state.windows.preferences = !state.windows.preferences;
            }
            ui.weak(if cfg!(target_os = "macos") {
                "\u{2318}K"
            } else {
                "Ctrl+K"
            })
            .on_hover_text("Command palette: go to a position, find a k-mer, run a command");
            if has_results {
                let (warnings, notes) = diagnostic_counts(state);
                let text = if warnings > 0 {
                    RichText::new(format!(
                        "\u{26A0} {}",
                        dima_core::human::plural(warnings as u64, "warning", "warnings")
                    ))
                    .color(t.warning_color)
                } else if notes > 0 {
                    RichText::new(dima_core::human::plural(notes as u64, "note", "notes"))
                } else {
                    RichText::new("No warnings")
                };
                if ui
                    .button(text)
                    .on_hover_text("Diagnostics and input validation")
                    .clicked()
                {
                    state.windows.diagnostics = !state.windows.diagnostics;
                }
                if ui.button("Run details").clicked() {
                    state.windows.run_details = !state.windows.run_details;
                }
            }
        });
    });
}
