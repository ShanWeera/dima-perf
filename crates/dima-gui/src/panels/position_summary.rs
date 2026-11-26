//! Facts about the selected position: its class, entropy (with the method or fallback that
//! produced it), observed entropy, support, distinct and total variants, sample coverage and,
//! when the run computed them, similarity-weighted entropy and the two normalised entropies.
//! Undefined values show "—" (D42); a normalised entropy reads "— (one variant)" where a
//! single distinct k-mer leaves it undefined, and is marked "(low support)" at support ≤ T.

use egui::{RichText, Ui};

use dima_core::config::Estimator;
use dima_core::model::PositionResult;

use super::common::{fact, fact_at, heading, key_column_width, normalised_value, with_help};
use super::help_text;
use crate::charts::series::Class;
use crate::theme::DesignTokens;
use crate::util::{opt_coverage, opt_f64, opt_percent};

/// The keys of the facts every run has.
const FACTS: [&str; 7] = [
    "Entropy",
    "Method",
    "Observed entropy",
    "Support",
    "Distinct variants",
    "Total variants",
    "Sample coverage",
];
/// The keys of the facts of runs that computed similarity-weighted entropy.
const SIMILARITY_FACTS: [&str; 3] = [
    "Similarity-weighted entropy",
    "Normalised observed entropy",
    "Normalised\nsimilarity-weighted entropy",
];

/// Draws the facts; `similarity_weighted` says whether the run computed similarity-weighted
/// entropy.
pub fn show(
    ui: &mut Ui,
    p: Option<&PositionResult>,
    threshold: u64,
    estimator: Estimator,
    similarity_weighted: bool,
    t: &DesignTokens,
) {
    let Some(p) = p else {
        heading(ui, t, "Position");
        ui.colored_label(t.text_muted, "Select a position in the chart or the table");
        return;
    };
    heading(ui, t, format!("Position {}", p.position));
    // The facts end before the inspector's scroll bar, which floats over the panel's right
    // edge while the pointer is over it, so a long value (a method with its fallback) wraps
    // before the bar instead of under it.
    let bar = &ui.spacing().scroll;
    let room = bar.bar_inner_margin + bar.bar_width + bar.bar_outer_margin;
    let width = (ui.available_width() - room).max(0.0);
    ui.scope(|ui| {
        ui.set_max_width(width);
        facts(ui, p, threshold, estimator, similarity_weighted, t);
    });
}

/// The class line and the facts' grids of `p`.
fn facts(
    ui: &mut Ui,
    p: &PositionResult,
    threshold: u64,
    estimator: Estimator,
    similarity_weighted: bool,
    t: &DesignTokens,
) {
    let class = Class::of(p);
    let color = if class == Class::Eligible {
        t.success_color
    } else {
        t.warning_color
    };
    ui.label(RichText::new(class.describe(threshold, estimator, p.entropy_method)).color(color));
    ui.add_space(t.space_4);
    // With the similarity facts, both grids give their keys the width of the widest, so their
    // values line up; without them the grid is as it always was.
    let key_width = if similarity_weighted {
        key_column_width(
            ui,
            &[FACTS.as_slice(), SIMILARITY_FACTS.as_slice()].concat(),
        )
    } else {
        0.0
    };
    let row = |ui: &mut Ui, key: &str, value: String| {
        if similarity_weighted {
            fact_at(ui, t, key, value, key_width)
        } else {
            fact(ui, t, key, value)
        }
    };
    // A value wider than its column wraps instead of running out of the panel.
    let max_width = ui.available_width();
    egui::Grid::new("position_facts")
        .num_columns(2)
        .spacing([t.space_12, t.space_4])
        .max_col_width(max_width)
        .show(ui, |ui| {
            let method = match (p.entropy_method.as_str(), p.entropy_fallback) {
                (m, Some(f)) => format!("{m}; fallback: {}", f.as_str().replace('_', " ")),
                (m, None) => m.to_owned(),
            };
            let values = [
                format!("{} bits", opt_f64(p.entropy, 4)),
                method,
                format!("{} bits", opt_f64(p.observed_entropy, 4)),
                p.support.to_string(),
                format!(
                    "{} ({})",
                    p.distinct_variants_count,
                    opt_percent(p.distinct_variants_incidence, 2)
                ),
                opt_percent(p.total_variants_incidence, 2),
                match p.below_min_coverage {
                    Some(true) => {
                        format!("{} (below the minimum)", opt_coverage(p.sample_coverage, 4))
                    }
                    _ => opt_coverage(p.sample_coverage, 4),
                },
            ];
            for (key, value) in FACTS.into_iter().zip(values) {
                row(ui, key, value);
            }
        });
    if !similarity_weighted {
        return;
    }
    egui::Grid::new("position_facts_similarity")
        .num_columns(2)
        .spacing([t.space_12, t.space_4])
        .max_col_width(max_width)
        .show(ui, |ui| {
            let [similarity, observed, normalised] = SIMILARITY_FACTS;
            let help = [
                help_text::similarity_weighted_fact(),
                help_text::normalised_observed_fact(),
                help_text::normalised_similarity_weighted_fact(),
            ];
            let values = similarity_values(p);
            for ((key, value), help) in [similarity, observed, normalised]
                .into_iter()
                .zip(values)
                .zip(help)
            {
                with_help(row(ui, key, value), &help);
            }
        });
}

/// The values of the similarity facts: similarity-weighted entropy in bits (0.0000 with one
/// distinct k-mer, a plain "—" without support), then the two normalised entropies.
#[must_use]
pub fn similarity_values(p: &PositionResult) -> [String; 3] {
    [
        p.similarity_weighted_entropy.map_or_else(
            || "\u{2014}".to_owned(),
            |v| format!("{} bits", dima_core::human::fixed(v, 4)),
        ),
        normalised_value(p.normalised_observed_entropy, p),
        normalised_value(p.normalised_similarity_weighted_entropy, p),
    ]
}

#[cfg(test)]
mod tests {
    use super::similarity_values;
    use crate::charts::series::Class;
    use crate::tests::fixtures;

    /// MERS spike position 25, the methodology page's worked example.
    #[test]
    fn the_worked_example() {
        let r = fixtures::mers_swe();
        let p = r.results.iter().find(|p| p.position == 25).unwrap();
        assert_eq!(
            similarity_values(p),
            ["0.4275 bits", "0.4282", "0.1287"].map(str::to_owned)
        );
    }

    /// One distinct k-mer: similarity-weighted entropy 0, the normalised values undefined
    /// because of it; no supported MERS protein position has low support, so none is marked.
    #[test]
    fn one_variant_and_no_low_support_mark_on_the_protein() {
        let r = fixtures::mers_swe();
        let single = r
            .results
            .iter()
            .find(|p| p.diversity_motifs.len() == 1)
            .unwrap();
        assert_eq!(
            similarity_values(single),
            [
                "0.0000 bits".to_owned(),
                "\u{2014} (one variant)".to_owned(),
                "\u{2014} (one variant)".to_owned()
            ]
        );
        assert!(
            r.results
                .iter()
                .flat_map(similarity_values)
                .all(|v| !v.contains("low support"))
        );
    }

    /// Low support marks the normalised values only: where every sequence carries a different
    /// k-mer the normalised observed entropy is 1.0000 by arithmetic, and is marked; a position
    /// with one k-mer reads "— (one variant)" whatever its support; no support reads "—".
    #[test]
    fn low_support_marks_the_normalised_values_only() {
        let low = |p: &&dima_core::model::PositionResult| {
            matches!(Class::of(p), Class::Low | Class::EqualToThreshold)
        };
        let nt = fixtures::mers_nt_swe();
        let nt_low: Vec<_> = nt.results.iter().filter(low).collect();
        assert_eq!(nt_low.len(), 29);
        for p in nt_low {
            assert_eq!(p.diversity_motifs.len(), 1);
            assert_eq!(
                similarity_values(p),
                [
                    "0.0000 bits",
                    "\u{2014} (one variant)",
                    "\u{2014} (one variant)"
                ]
                .map(str::to_owned)
            );
        }
        let hiv = fixtures::hiv_pro_swe();
        let marked: Vec<_> = hiv
            .results
            .iter()
            .filter(low)
            .filter(|p| p.diversity_motifs.len() > 1)
            .collect();
        assert_eq!(marked.len(), 41);
        for p in &marked {
            let [h, n, ns] = similarity_values(p);
            assert!(!h.contains("low support"), "{h}");
            assert!(
                n.ends_with(" (low support)") && ns.ends_with(" (low support)"),
                "{n} {ns}"
            );
        }
        let all_distinct = marked
            .iter()
            .find(|p| p.diversity_motifs.len() as u64 == p.support)
            .unwrap();
        assert_eq!(similarity_values(all_distinct)[1], "1.0000 (low support)");
        let ns = hiv.results.iter().find(|p| p.support == 0).unwrap();
        assert_eq!(similarity_values(ns), ["\u{2014}"; 3].map(str::to_owned));
    }
}
