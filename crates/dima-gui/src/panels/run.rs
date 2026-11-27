//! The run section of the setup screen: "Analyze" (enabled as soon as a file is checked, D85),
//! the inline progress with the library's phase counters and "Cancel", and the failure card.

use std::time::Instant;

use egui::{RichText, Ui};

use super::error_card;
use crate::app::msg::Msg;
use crate::state::{AppState, RunState, View};
use crate::theme::DesignTokens;
use crate::workers::analysis;

/// Progress of the running analysis, read from the job (polled once per frame).
#[derive(Clone, Debug, PartialEq)]
pub struct RunProgress {
    /// Description of the phase.
    pub text: String,
    /// Completed fraction, when known.
    pub fraction: Option<f32>,
}

/// Draws the run section.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    progress: Option<&RunProgress>,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    match state.setup.run {
        RunState::Idle => analyze_button(ui, state, t, out),
        RunState::Running(_) | RunState::Cancelling(_) => {
            let cancelling = matches!(state.setup.run, RunState::Cancelling(_));
            super::common::card(t).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let text = progress.map_or("Starting\u{2026}", |p| p.text.as_str());
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new(if cancelling {
                            "Cancelling\u{2026}"
                        } else {
                            text
                        })
                        .strong(),
                    );
                });
                if let Some(f) = progress.and_then(|p| p.fraction) {
                    ui.add(egui::ProgressBar::new(f).desired_height(6.0).fill(t.accent));
                }
                ui.add_space(t.space_4);
                if ui
                    .add_enabled(!cancelling, egui::Button::new("Cancel"))
                    .clicked()
                {
                    out.push(Msg::CancelAnalysis);
                }
            });
        }
    }
    if let Some(card) = &state.setup.failure {
        ui.add_space(t.space_8);
        error_card::show(ui, card, t, out, true);
    }
}

fn analyze_button(ui: &mut Ui, state: &AppState, t: &DesignTokens, out: &mut Vec<Msg>) {
    let reason = state.not_ready();
    let ready = state.setup.opening.is_none() && reason.is_none();
    let button = egui::Button::new(RichText::new("\u{25B6}  Analyze").size(t.font_size_body + 2.0))
        .min_size(egui::vec2(ui.available_width(), 40.0));
    let response = ui.add_enabled(ready, button);
    let response = match &reason {
        Some(r) => response.on_disabled_hover_text(r),
        None => response,
    };
    if response.clicked() {
        out.push(Msg::Analyze);
    }
    if let (Some(r), true) = (&reason, state.setup.file.is_some()) {
        ui.colored_label(t.error_color, r);
    }
    if state.session.is_some() {
        ui.add_space(t.space_4);
        if ui
            .button("Back to the results \u{25B6}")
            .on_hover_text("Return to the results already loaded")
            .clicked()
        {
            out.push(Msg::ShowView(View::Workspace));
        }
    }
}

/// The progress of a running job.
#[must_use]
pub fn progress_of(job: &analysis::Job) -> RunProgress {
    let (text, fraction) =
        analysis::describe(&job.snapshot(), Instant::now().duration_since(job.started));
    RunProgress { text, fraction }
}
