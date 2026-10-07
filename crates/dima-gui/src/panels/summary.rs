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
            format!("{:.4} at position {}{ties}", e.entropy, e.position)
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
                r.average_entropy
                    .map_or_else(|| "\u{2014}".to_owned(), |m| format!("{m:.4} bits"))
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
            egui::Frame::new()
                .fill(t.surface_secondary)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .size(t.font_size_caption)
                            .color(t.text_secondary),
                    );
                })
                .response
                .on_hover_text(tip);
        }
    });
}
