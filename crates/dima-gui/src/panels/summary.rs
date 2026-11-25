//! The results header: the query name and the run's facts as chips — sequences, positions, k,
//! T with "support > T eligible" (G42), the eligible mean, lowest and highest entropy with
//! their positions and tie counts (D43, D88), the NS/LS/ELS counts and the low-support total,
//! and the total-variants incidence at the selected position (G60).

use egui::{RichText, Ui};

use dima_core::model::{Extremum, Results};

use crate::state::session::Session;
use crate::theme::DesignTokens;
use crate::util::opt_percent;

fn extremum(e: Option<Extremum>) -> String {
    e.map_or_else(
        || "\u{2014}".to_owned(),
        |e| {
            let ties = if e.tied_position_count > 1 {
                format!(", {} positions tied", e.tied_position_count)
            } else {
                String::new()
            };
            format!(
                "{} at position {}{ties}",
                dima_core::human::fixed(e.entropy, 4),
                e.position
            )
        },
    )
}

/// The chip texts and their tooltips (pure, for tests).
#[must_use]
pub fn chips(r: &Results, session: &Session) -> Vec<(String, String)> {
    let c = &r.support_label_counts;
    let mut out = vec![
        (
            dima_core::human::plural(r.sequence_count, "sequence", "sequences"),
            "Sequences in the alignment".to_owned(),
        ),
        (
            dima_core::human::plural(r.position_count, "position", "positions"),
            format!(
                "k-mer positions (alignment length {} \u{2212} k + 1)",
                r.run.alignment_length
            ),
        ),
        (format!("k = {}", r.kmer_length), "k-mer length".to_owned()),
        (
            format!("T = {} (support > T eligible)", r.support_threshold),
            "Positions supported by more than T sequences are eligible: they enter the \
             summaries, and the DiMA estimator corrects their entropy"
                .to_owned(),
        ),
        (
            format!(
                "mean {}",
                r.average_entropy.map_or_else(
                    || "\u{2014}".to_owned(),
                    |m| format!("{} bits", dima_core::human::fixed(m, 4))
                )
            ),
            format!(
                "Mean entropy over the {} eligible positions",
                r.eligible_position_count
            ),
        ),
        (
            format!("lowest {}", extremum(r.lowest_entropy)),
            "Lowest entropy among eligible positions: the first position with it and how many \
             positions share it"
                .to_owned(),
        ),
        (
            format!("highest {}", extremum(r.highest_entropy)),
            "Highest entropy among eligible positions: the first position with it and how many \
             positions share it"
                .to_owned(),
        ),
        (
            format!(
                "low support {} (NS {}, LS {}, ELS {})",
                r.low_support_count, c.no_support, c.low, c.equal_to_threshold
            ),
            format!(
                "Positions with support \u{2264} T: none (NS), below T (LS), equal to T (ELS). \
                 They are excluded from the summaries; {} positions are eligible.",
                c.eligible
            ),
        ),
    ];
    if let Some(p) = session.selected() {
        out.push((
            format!(
                "total variants at {}: {}",
                p.position,
                opt_percent(p.total_variants_incidence, 2)
            ),
            "Share of the supporting sequences that carry a non-Index variant at the selected \
             position"
                .to_owned(),
        ));
    }
    out
}

/// Draws the header.
pub fn show(ui: &mut Ui, session: &Session, t: &DesignTokens) {
    let r = &session.results;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&r.query_name)
                .size(t.font_size_title)
                .family(egui::FontFamily::Name(crate::theme::fonts::SEMIBOLD.into())),
        );
        if let crate::state::session::Origin::Opened(path) = &session.origin {
            ui.weak(format!(
                "opened from {}",
                dima_core::run_report::file_name(path)
            ))
            .on_hover_text(path.display().to_string());
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (text, tip) in chips(r, session) {
            chip(ui, t, &text).on_hover_text(tip);
        }
    });
}

/// One chip: its text on a rounded frame, allocated as a single item, so a row that is full
/// moves the whole chip to the next row instead of wrapping its text into a narrow column.
fn chip(ui: &mut Ui, t: &DesignTokens, text: &str) -> egui::Response {
    const MARGIN: egui::Vec2 = egui::vec2(8.0, 2.0);
    const STROKE: f32 = 1.0;
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(t.font_size_caption),
        t.text_secondary,
    );
    let inset = MARGIN + egui::Vec2::splat(STROKE);
    let (rect, response) =
        ui.allocate_exact_size(galley.size() + 2.0 * inset, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.rect(
            rect,
            10.0,
            t.surface_secondary,
            egui::Stroke::new(STROKE, t.border),
            egui::StrokeKind::Inside,
        );
        p.galley(rect.min + inset, galley, t.text_secondary);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    response
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::charts::series::tests::{position, results_of};
    use crate::state::session::Origin;

    fn texts(r: Results) -> Vec<String> {
        let s = Session::new(1, Arc::new(r), Origin::Opened("x.json".into()));
        chips(&s.results, &s)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// A value that is not zero never reads 0.0000, an exact zero does, and an incidence below
    /// 100% never reads 100.00%.
    #[test]
    fn chips_never_round_a_value_to_zero_or_to_its_bound() {
        let mut p = position(1, Some(3.288e-5), 200, 100);
        p.total_variants_incidence = Some(99.996);
        let mut r = results_of(vec![p]);
        r.average_entropy = Some(4.9e-5);
        r.lowest_entropy = Some(Extremum {
            position: 7,
            entropy: 0.0,
            tied_position_count: 455,
        });
        r.highest_entropy = Some(Extremum {
            position: 26,
            entropy: 3.288e-5,
            tied_position_count: 1,
        });
        let chips = texts(r);
        for expected in [
            "mean < 0.0001 bits",
            "lowest 0.0000 at position 7, 455 positions tied",
            "highest < 0.0001 at position 26",
            "total variants at 1: > 99.99%",
        ] {
            assert!(
                chips.iter().any(|c| c == expected),
                "{expected:?} in {chips:?}"
            );
        }
        // Ordinary values read as before.
        let mut r = results_of(vec![position(1, Some(0.5), 200, 100)]);
        r.average_entropy = Some(0.126_084_5);
        let chips = texts(r);
        assert!(chips.iter().any(|c| c == "mean 0.1261 bits"), "{chips:?}");
        assert!(
            chips.iter().any(|c| c == "total variants at 1: 0.00%"),
            "{chips:?}"
        );
    }
}
