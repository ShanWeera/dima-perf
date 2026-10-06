//! Facts about the selected position: its class, entropy (with the method or fallback that
//! produced it), observed entropy, support, distinct and total variants, sample coverage.
//! Undefined values show "—" (D42).

use egui::{RichText, Ui};

use dima_core::config::Estimator;
use dima_core::model::PositionResult;

use super::common::{fact, heading};
use crate::charts::series::Class;
use crate::theme::DesignTokens;
use crate::util::{opt_f64, opt_percent};

/// Draws the facts.
pub fn show(
    ui: &mut Ui,
    p: Option<&PositionResult>,
    threshold: u64,
    estimator: Estimator,
    t: &DesignTokens,
) {
    let Some(p) = p else {
        heading(ui, t, "Position");
        ui.colored_label(t.text_muted, "Select a position in the chart or the table");
        return;
    };
    heading(ui, t, format!("Position {}", p.position));
    let class = Class::of(p);
    let color = if class == Class::Eligible {
        t.success_color
    } else {
        t.warning_color
    };
    ui.label(RichText::new(class.describe(threshold, estimator, p.entropy_method)).color(color));
    ui.add_space(t.space_4);
    egui::Grid::new("position_facts")
        .num_columns(2)
        .spacing([t.space_12, t.space_4])
        .show(ui, |ui| {
            let method = match (p.entropy_method.as_str(), p.entropy_fallback) {
                (m, Some(f)) => format!("{m}; fallback: {}", f.as_str().replace('_', " ")),
                (m, None) => m.to_owned(),
            };
            fact(ui, t, "Entropy", format!("{} bits", opt_f64(p.entropy, 4)));
            fact(ui, t, "Method", method);
            fact(
                ui,
                t,
                "Observed entropy",
                format!("{} bits", opt_f64(p.observed_entropy, 4)),
            );
            fact(ui, t, "Support", p.support.to_string());
            fact(
                ui,
                t,
                "Distinct variants",
                format!(
                    "{} ({})",
                    p.distinct_variants_count,
                    opt_percent(p.distinct_variants_incidence, 2)
                ),
            );
            fact(
                ui,
                t,
                "Total variants",
                opt_percent(p.total_variants_incidence, 2),
            );
            let coverage = match p.below_min_coverage {
                Some(true) => format!("{} (below the minimum)", opt_f64(p.sample_coverage, 4)),
                _ => opt_f64(p.sample_coverage, 4),
            };
            fact(ui, t, "Sample coverage", coverage);
        });
}
