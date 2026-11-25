//! The run details window (D103): how the analysis ran — the equivalent command, worker
//! threads and their source, platform, phase times, time waiting for input, records and
//! positions, whether similarity-weighted entropy was computed, the input's identity and the
//! exported files — with "Save run report…". Results opened from a file show their recorded
//! provenance instead.

use egui::{RichText, Ui};

use dima_core::human;
use dima_core::run_report::{Platform, rfc3339};

use super::common::fact;
use crate::app::msg::Msg;
use crate::state::session::{Origin, Session};
use crate::theme::DesignTokens;
use crate::workers::analysis::RunFacts;
use crate::workers::export::ExportKind;

/// Draws the window when open.
pub fn window(
    ctx: &egui::Context,
    open: &mut bool,
    s: &Session,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    egui::Window::new("Run details")
        .open(open)
        .default_width(520.0)
        .vscroll(true)
        .show(ctx, |ui| contents(ui, s, t, out));
}

fn contents(ui: &mut Ui, s: &Session, t: &DesignTokens, out: &mut Vec<Msg>) {
    let r = &s.results;
    match &s.origin {
        Origin::Analysed(run) => {
            let command = crate::util::shell_words(&crate::report::command(
                &run.path,
                &run.params,
                run.facts.threads,
            ));
            ui.label(RichText::new("Equivalent command").strong());
            ui.horizontal_wrapped(|ui| {
                ui.monospace(&command);
            });
            if ui.small_button("Copy command").clicked() {
                ui.ctx().copy_text(command);
            }
            ui.add_space(t.space_8);
            let f = &run.facts;
            let platform = Platform::current();
            egui::Grid::new("run_facts")
                .num_columns(2)
                .spacing([t.space_12, t.space_4])
                .show(ui, |ui| {
                    fact(ui, t, "Started", rfc3339(f.start_time));
                    fact(ui, t, "Finished", rfc3339(f.end_time));
                    fact(ui, t, "Wall time", human::duration(f.wall));
                    fact(ui, t, "Worker threads", threads(f));
                    fact(
                        ui,
                        t,
                        "Platform",
                        format!(
                            "{} {}{}{} logical CPUs",
                            platform.os,
                            platform.arch,
                            human::separators::ITEM,
                            platform.logical_cpus
                        ),
                    );
                    for (phase, d) in &f.phases {
                        fact(ui, t, &format!("  {}", phase.label()), human::duration(*d));
                    }
                    if let Some(w) = f.waited_for_input {
                        fact(ui, t, "Waiting for input", human::duration(w));
                    }
                    fact(ui, t, "Records", f.records.to_string());
                    fact(
                        ui,
                        t,
                        "Positions",
                        f.positions
                            .map_or_else(|| "\u{2014}".to_owned(), |p| p.to_string()),
                    );
                    fact(
                        ui,
                        t,
                        SIMILARITY_WEIGHTED,
                        similarity_weighted(r).to_owned(),
                    );
                    fact(
                        ui,
                        t,
                        "Input",
                        format!(
                            "{}{}{}",
                            r.run.input.name,
                            human::separators::ITEM,
                            human::bytes(r.run.input.size_bytes)
                        ),
                    );
                    fact(ui, t, "SHA-256", r.run.input.checksum.value.clone());
                });
            ui.add_space(t.space_8);
            ui.label(RichText::new("Exported files").strong());
            if s.exports.is_empty() {
                ui.weak("none yet");
            }
            for e in &s.exports {
                ui.label(format!(
                    "{} ({}{}{})",
                    e.name,
                    e.role,
                    human::separators::ITEM,
                    human::bytes(e.size_bytes)
                ))
                .on_hover_text(format!("SHA-256 {}", e.sha256));
            }
            ui.add_space(t.space_8);
            if ui
                .button("Save run report\u{2026}")
                .on_hover_text(super::help_text::export(ExportKind::RunReport))
                .clicked()
            {
                out.push(Msg::Export(ExportKind::RunReport));
            }
            ui.label(
                RichText::new(
                    "CPU time and peak memory are not recorded: this process also draws the \
                     interface. Use the command line for those.",
                )
                .small()
                .color(t.text_muted),
            );
        }
        Origin::Opened(path) => {
            ui.label(format!(
                "These results were opened from {}. Run details are recorded for analyses run \
                 in this app; the results' own provenance:",
                dima_core::run_report::file_name(path)
            ));
            egui::Grid::new("provenance").num_columns(2).show(ui, |ui| {
                fact(
                    ui,
                    t,
                    "Written by",
                    format!("{} {}", r.run.tool.name, r.run.tool.version),
                );
                fact(ui, t, "Input", r.run.input.name.clone());
                fact(ui, t, "SHA-256", r.run.input.checksum.value.clone());
                fact(
                    ui,
                    t,
                    "Estimator",
                    r.run.parameters.estimator.value.as_str().to_owned(),
                );
                fact(
                    ui,
                    t,
                    SIMILARITY_WEIGHTED,
                    similarity_weighted(r).to_owned(),
                );
            });
        }
    }
}

/// The fact naming whether similarity-weighted entropy was computed.
const SIMILARITY_WEIGHTED: &str = "Similarity-weighted entropy";

/// `computed` or `not computed`: whether the run asked for similarity-weighted entropy (a file
/// written by dima 1.0.0 reads as not computed).
fn similarity_weighted(r: &dima_core::model::Results) -> &'static str {
    if r.run.parameters.similarity_weighted_entropy.value {
        "computed"
    } else {
        "not computed"
    }
}

/// `8 (settings)`, or `4 of 8 (settings; the system refused the rest)` (D110).
fn threads(f: &RunFacts) -> String {
    match f.threads_started {
        Some(started) if started < f.threads => format!(
            "{started} of {} (settings; the system refused the rest)",
            f.threads
        ),
        _ => format!("{} (settings)", f.threads),
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::fixtures;

    /// Both branches of the window say whether the run computed similarity-weighted entropy.
    #[test]
    fn similarity_weighted_entropy_is_named_computed_or_not() {
        assert_eq!(
            super::similarity_weighted(&fixtures::mers()),
            "not computed"
        );
        assert_eq!(
            super::similarity_weighted(&fixtures::mers_swe()),
            "computed"
        );
    }
}
