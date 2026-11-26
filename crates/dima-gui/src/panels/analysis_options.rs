//! The analysis options (D72): the paper's options on the main panel — alphabet, k, T, query
//! name, conserved-region threshold — and a collapsed "Advanced" section with the estimator,
//! validation mode, fill value, minimum coverage, similarity-weighted entropy and worker
//! threads. Every option has a cited tooltip (D89); an option that differs from its default
//! carries a dot whose tooltip names the default, the collapsed section says how many of its
//! options do ("Advanced (2 changed)"), and "Reset to defaults" restores them all (D73). No
//! arbitrary caps (D72).

use egui::{RichText, Ui};

use dima_core::config::{
    DEFAULT_FILL, DEFAULT_SUPPORT_THRESHOLD, Estimator, Percent, ValidationMode,
};

use super::common::{card, heading, option_label, with_help};
use super::help_text as help;
use crate::app::msg::Msg;
use crate::state::AppState;
use crate::state::options::{AlphabetChoice, AnalysisOptions, estimator_label, validation_label};
use crate::theme::DesignTokens;
use crate::workers::analysis::default_threads;

/// Draws the options card.
pub fn show(ui: &mut Ui, state: &mut AppState, t: &DesignTokens, out: &mut Vec<Msg>) {
    let detected = state.setup.inspection.as_ref().and_then(|i| i.alphabet);
    let alignment_length = state.setup.inspection.as_ref().map(|i| i.alignment_length);
    let suggested_name = state
        .setup
        .inspection
        .as_ref()
        .and_then(|i| i.suggested_query_name.clone());
    let mut changed = false;
    card(t).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            heading(ui, t, "Analysis options");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let differs = state.setup.options.differs_from_defaults()
                    || state.settings.threads.is_some()
                    || state.hcs.params != dima_core::hcs::HcsParams::default();
                if ui
                    .add_enabled(differs, egui::Button::new("Reset to defaults"))
                    .on_hover_text("Restore every option to the DiMA defaults")
                    .clicked()
                {
                    out.push(Msg::ResetOptions);
                }
            });
        });
        ui.add_space(t.space_4);
        let o = &mut state.setup.options;
        egui::Grid::new("paper_options")
            .num_columns(2)
            .spacing([t.space_16, t.space_8])
            .show(ui, |ui| {
                // Alphabet.
                let label = option_label(
                    ui,
                    t,
                    "Alphabet",
                    &help::alphabet(),
                    (o.alphabet != AlphabetChoice::Auto).then(|| "Auto".to_owned()),
                );
                let auto_text = match detected {
                    Some(a) => format!("Auto (detected: {})", a.display_name()),
                    None => "Auto".to_owned(),
                };
                egui::ComboBox::from_id_salt("alphabet")
                    .selected_text(if o.alphabet == AlphabetChoice::Auto {
                        auto_text.clone()
                    } else {
                        o.alphabet.label().to_owned()
                    })
                    .show_ui(ui, |ui| {
                        for choice in AlphabetChoice::ALL {
                            let label = if choice == AlphabetChoice::Auto {
                                auto_text.clone()
                            } else {
                                choice.label().to_owned()
                            };
                            changed |= ui
                                .selectable_value(&mut o.alphabet, choice, label)
                                .changed();
                        }
                    })
                    .response
                    .labelled_by(label);
                ui.end_row();

                // k.
                let default_k = o.default_kmer(detected);
                let label = option_label(
                    ui,
                    t,
                    "k-mer length",
                    &help::kmer_length(),
                    o.kmer_length.filter(|k| Some(*k) != default_k).map(|_| {
                        default_k.map_or_else(|| "from the alphabet".to_owned(), |k| k.to_string())
                    }),
                );
                ui.horizontal(|ui| {
                    let mut k = o.kmer_length.or(default_k).unwrap_or(9);
                    let max = alignment_length.unwrap_or(usize::MAX / 2).max(1);
                    if ui
                        .add(egui::DragValue::new(&mut k).range(1..=max).speed(0.2))
                        .labelled_by(label)
                        .on_hover_text(help::kmer_length())
                        .changed()
                    {
                        // The default is passed as unset, like every other option, so the run
                        // records it as derived from the alphabet (as `dima analyze` without -k).
                        o.kmer_length = (Some(k) != default_k).then_some(k);
                        changed = true;
                    }
                    if o.kmer_length.is_some() {
                        if ui.small_button("Use default").clicked() {
                            o.kmer_length = None;
                            changed = true;
                        }
                    } else {
                        ui.weak(match default_k {
                            Some(_) => "default for the alphabet",
                            None => "set once the alphabet is known",
                        });
                    }
                });
                ui.end_row();

                // T.
                let label = option_label(
                    ui,
                    t,
                    "Support threshold T",
                    &help::support_threshold(),
                    (o.support_threshold != DEFAULT_SUPPORT_THRESHOLD)
                        .then(|| DEFAULT_SUPPORT_THRESHOLD.to_string()),
                );
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut o.support_threshold)
                                .range(1..=u64::MAX)
                                .speed(1.0),
                        )
                        .labelled_by(label)
                        .on_hover_text(help::support_threshold())
                        .changed();
                    ui.weak("positions with support > T are eligible");
                });
                ui.end_row();

                // Query name.
                let label = option_label(
                    ui,
                    t,
                    "Query name",
                    &help::query_name(),
                    (!o.query_name.trim().is_empty()).then(|| "the file name".to_owned()),
                );
                let hint = suggested_name
                    .clone()
                    .unwrap_or_else(|| "from the file name".to_owned());
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut o.query_name)
                            .hint_text(hint)
                            .desired_width(260.0),
                    )
                    .labelled_by(label)
                    .on_hover_text(help::query_name())
                    .changed();
                ui.end_row();
            });
        ui.add_space(t.space_8);
        hcs_controls(ui, state, t, out);
        ui.add_space(t.space_8);
        let header_on = &mut state.setup.header.enabled;
        if ui
            .checkbox(header_on, "Stratify variants by FASTA header fields")
            .on_hover_text(help::header_format())
            .changed()
        {
            changed = true;
        }
        ui.add_space(t.space_8);
        let defaults = AdvancedDefaults::of(&state.setup.options, state.settings.threads);
        // The salt keeps the open state when the title's count changes.
        egui::CollapsingHeader::new(advanced_title(defaults.changed()))
            .id_salt("advanced_options")
            .default_open(false)
            .show(ui, |ui| {
                changed |= advanced(ui, state, &defaults, t, out);
            });
    });
    if changed {
        out.push(Msg::OptionsChanged);
    }
}

/// The conserved-region threshold and switch (one state with the workspace strip, G40).
pub fn hcs_controls(ui: &mut Ui, state: &mut AppState, t: &DesignTokens, out: &mut Vec<Msg>) {
    let h = &mut state.hcs;
    ui.horizontal_wrapped(|ui| {
        let default = dima_core::config::DEFAULT_HCS_THRESHOLD;
        let label = option_label(
            ui,
            t,
            "Conserved regions (HCS) at",
            &help::hcs_threshold(),
            (h.params.threshold != default).then(|| format!("{default}%")),
        );
        let mut value = h.params.threshold.to_f64();
        let slider = ui
            .add(
                egui::Slider::new(&mut value, 50.5..=100.0)
                    .step_by(0.5)
                    .suffix("%")
                    .text("Index incidence"),
            )
            .labelled_by(label)
            .on_hover_text(help::hcs_threshold());
        if slider.changed()
            && let Ok(p) = Percent::hcs_threshold(&format!("{value}"))
        {
            h.text = p.to_string();
            out.push(Msg::HcsEdited);
        }
        let field = ui
            .add(egui::TextEdit::singleline(&mut h.text).desired_width(64.0))
            .labelled_by(label)
            .on_hover_text("Exact value, e.g. 95.25");
        if field.changed() {
            out.push(Msg::HcsEdited);
        }
        if ui
            .checkbox(&mut h.params.eligible_only, "Eligible positions only")
            .on_hover_text(help::hcs_eligible_only())
            .changed()
        {
            out.push(Msg::HcsEdited);
        }
    });
    if let Some(e) = &h.error {
        ui.colored_label(t.error_color, e);
    }
}

/// The default of every Advanced row whose value differs from it, `None` for a row at its
/// default: the one source of the rows' dots and of the section's "N changed".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AdvancedDefaults {
    estimator: Option<String>,
    validation: Option<String>,
    fill: Option<String>,
    min_coverage: Option<String>,
    similarity_weighted_entropy: Option<String>,
    threads: Option<String>,
}

impl AdvancedDefaults {
    /// For the form's options and the remembered worker threads (`None` = the default).
    fn of(o: &AnalysisOptions, threads: Option<usize>) -> Self {
        Self {
            estimator: (o.estimator != Estimator::default())
                .then(|| estimator_label(Estimator::default()).to_owned()),
            validation: (o.validation != ValidationMode::default())
                .then(|| validation_label(ValidationMode::default()).to_owned()),
            fill: (o.fill != DEFAULT_FILL).then(|| DEFAULT_FILL.to_owned()),
            min_coverage: o.min_coverage.map(|_| "off".to_owned()),
            similarity_weighted_entropy: o.similarity_weighted_entropy.then(|| "off".to_owned()),
            threads: threads.map(|_| format!("{} (all CPUs but one)", default_threads().get())),
        }
    }

    /// How many rows differ from their default.
    fn changed(&self) -> usize {
        [
            &self.estimator,
            &self.validation,
            &self.fill,
            &self.min_coverage,
            &self.similarity_weighted_entropy,
            &self.threads,
        ]
        .into_iter()
        .filter(|d| d.is_some())
        .count()
    }
}

/// "Advanced", or "Advanced (N changed)" when N of its options differ from their defaults, so
/// a changed option is seen while the section is closed.
fn advanced_title(changed: usize) -> String {
    if changed == 0 {
        "Advanced".to_owned()
    } else {
        format!("Advanced ({changed} changed)")
    }
}

fn advanced(
    ui: &mut Ui,
    state: &mut AppState,
    defaults: &AdvancedDefaults,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) -> bool {
    let mut changed = false;
    let metadata = state.setup.header.enabled;
    let o = &mut state.setup.options;
    egui::Grid::new("advanced_options")
        .num_columns(2)
        .spacing([t.space_16, t.space_8])
        .show(ui, |ui| {
            let label = option_label(
                ui,
                t,
                "Entropy estimator",
                &help::estimator(),
                defaults.estimator.clone(),
            );
            egui::ComboBox::from_id_salt("estimator")
                .selected_text(estimator_label(o.estimator))
                .show_ui(ui, |ui| {
                    for e in Estimator::ALL {
                        changed |= ui
                            .selectable_value(&mut o.estimator, e, estimator_label(e))
                            .changed();
                    }
                })
                .response
                .labelled_by(label);
            ui.end_row();

            let label = option_label(
                ui,
                t,
                "Validation",
                &help::validation(),
                defaults.validation.clone(),
            );
            egui::ComboBox::from_id_salt("validation")
                .selected_text(validation_label(o.validation))
                .show_ui(ui, |ui| {
                    for v in [
                        ValidationMode::Strict,
                        ValidationMode::Permissive,
                        ValidationMode::Report,
                    ] {
                        changed |= ui
                            .selectable_value(&mut o.validation, v, validation_label(v))
                            .changed();
                    }
                })
                .response
                .labelled_by(label);
            ui.end_row();

            let label = option_label(ui, t, "Fill value", &help::fill(), defaults.fill.clone());
            ui.horizontal(|ui| {
                changed |= ui
                    .add_enabled(
                        metadata,
                        egui::TextEdit::singleline(&mut o.fill).desired_width(140.0),
                    )
                    .labelled_by(label)
                    .on_hover_text(help::fill())
                    .on_disabled_hover_text(
                        "Used only when variants are stratified by header fields",
                    )
                    .changed();
                if o.fill.trim().is_empty() {
                    ui.colored_label(t.error_color, "must not be empty");
                }
            });
            ui.end_row();

            let label = option_label(
                ui,
                t,
                "Minimum coverage",
                &help::min_coverage(),
                defaults.min_coverage.clone(),
            );
            ui.horizontal(|ui| {
                let mut on = o.min_coverage.is_some();
                if ui
                    .checkbox(&mut on, "flag below")
                    .on_hover_text(help::min_coverage())
                    .changed()
                {
                    o.min_coverage = on.then_some(0.95);
                    changed = true;
                }
                if let Some(c) = o.min_coverage.as_mut() {
                    changed |= ui
                        .add(
                            egui::DragValue::new(c)
                                .range(0.0..=1.0)
                                .speed(0.005)
                                .max_decimals(3),
                        )
                        .labelled_by(label)
                        .changed();
                }
            });
            ui.end_row();

            let help = help::similarity_weighted_entropy();
            option_label(
                ui,
                t,
                "Similarity-weighted",
                &help,
                defaults.similarity_weighted_entropy.clone(),
            );
            let checkbox = ui.checkbox(
                &mut o.similarity_weighted_entropy,
                "Compute similarity-weighted entropy (Wu 2025)",
            );
            changed |= with_help(checkbox, &help).changed();
            ui.end_row();

            let cpus = crate::workers::analysis::cpus();
            let default = default_threads().get();
            let label = option_label(
                ui,
                t,
                "Worker threads",
                &help::threads(),
                defaults.threads.clone(),
            );
            ui.horizontal(|ui| {
                let mut n =
                    crate::workers::analysis::worker_threads(state.settings.threads, cpus).get();
                if ui
                    .add(egui::Slider::new(&mut n, 1..=cpus.max(1)).text(format!("of {cpus} CPUs")))
                    .labelled_by(label)
                    .on_hover_text(help::threads())
                    .changed()
                {
                    out.push(Msg::ThreadsChanged((n != default).then_some(n)));
                }
            });
            ui.end_row();
        });
    if !metadata {
        ui.label(
            RichText::new("The fill value applies when variants are stratified by header fields.")
                .small()
                .color(t.text_muted),
        );
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The section's title counts the rows whose dot is shown.
    #[test]
    fn the_advanced_title_counts_the_changed_options() {
        let mut o = AnalysisOptions::default();
        let d = AdvancedDefaults::of(&o, None);
        assert_eq!(d, AdvancedDefaults::default());
        assert_eq!(advanced_title(d.changed()), "Advanced");
        o.similarity_weighted_entropy = true;
        let d = AdvancedDefaults::of(&o, None);
        assert_eq!(d.similarity_weighted_entropy.as_deref(), Some("off"));
        assert_eq!(advanced_title(d.changed()), "Advanced (1 changed)");
        let d = AdvancedDefaults::of(&o, Some(2));
        assert!(d.threads.is_some(), "the threads come from the settings");
        assert_eq!(advanced_title(d.changed()), "Advanced (2 changed)");
        o.estimator = Estimator::Raw;
        o.validation = ValidationMode::Report;
        o.fill = "NA".into();
        o.min_coverage = Some(0.9);
        assert_eq!(AdvancedDefaults::of(&o, Some(2)).changed(), 6, "every row");
        // Options outside the section are not counted.
        let main = AnalysisOptions {
            support_threshold: 30,
            kmer_length: Some(5),
            ..AnalysisOptions::default()
        };
        assert_eq!(AdvancedDefaults::of(&main, None).changed(), 0);
    }
}
