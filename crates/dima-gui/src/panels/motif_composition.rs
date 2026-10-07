//! The motif composition of the selected position: one 100%-stacked bar of the Index, Major,
//! Minor and Unique shares of its supporting sequences, with exact figures in the legend.

use egui::Ui;

use dima_core::model::{Motif, Variant};

use super::common::{Swatch, legend_entry, sub_heading};
use crate::state::filter::MOTIFS;
use crate::theme::DesignTokens;

/// Incidence per motif class, in [`MOTIFS`] order; non-finite incidences are skipped.
#[must_use]
pub fn shares(variants: &[Variant]) -> Option<[f64; 4]> {
    let mut totals = [0.0_f64; 4];
    for v in variants {
        let i = match v.motif {
            Motif::Index => 0,
            Motif::Major => 1,
            Motif::Minor => 2,
            Motif::Unique => 3,
        };
        if v.incidence.is_finite() && v.incidence > 0.0 {
            totals[i] += v.incidence;
        }
    }
    let sum: f64 = totals.iter().sum();
    (sum.is_finite() && sum > 0.0).then(|| totals.map(|x| x / sum * 100.0))
}

/// Draws the bar and its legend.
pub fn show(ui: &mut Ui, variants: &[Variant], t: &DesignTokens) {
    let Some(shares) = shares(variants) else {
        return;
    };
    ui.add_space(t.space_8);
    sub_heading(ui, "Motif composition");
    let colors = t.motif_colors();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 16.0), egui::Sense::hover());
    let mut left = rect.left();
    let mut cumulative = 0.0;
    let mut segments = Vec::new();
    for (i, share) in shares.iter().enumerate() {
        cumulative += share;
        #[allow(clippy::cast_possible_truncation)]
        let right = rect.left() + (cumulative / 100.0) as f32 * rect.width();
        if right > left {
            let seg = egui::Rect::from_min_max(
                egui::pos2(left, rect.top()),
                egui::pos2(right, rect.bottom()),
            );
            ui.painter().rect_filled(seg, 0.0, colors[i]);
            segments.push((seg, i));
        }
        left = right;
    }
    if let Some(p) = response.hover_pos()
        && let Some(&(_, i)) = segments.iter().find(|(r, _)| r.contains(p))
    {
        response.on_hover_text(format!(
            "{}: {:.1}% of the supporting sequences",
            MOTIFS[i].long(),
            shares[i]
        ));
    }
    ui.horizontal_wrapped(|ui| {
        for (i, m) in MOTIFS.iter().enumerate() {
            legend_entry(
                ui,
                t,
                &format!("{} {:.1}%", m.long(), shares[i]),
                Swatch::Block(colors[i]),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(incidence: f64, motif: Motif) -> Variant {
        Variant {
            sequence: "AAA".into(),
            count: 1,
            incidence,
            motif,
            metadata: None,
        }
    }

    #[test]
    fn shares_sum_to_100_and_survive_bad_numbers() {
        let s = shares(&[v(75.0, Motif::Index), v(25.0, Motif::Unique)]).unwrap();
        assert!((s[0] - 75.0).abs() < 1e-9 && (s[3] - 25.0).abs() < 1e-9);
        assert!((s.iter().sum::<f64>() - 100.0).abs() < 1e-9);
        assert_eq!(shares(&[]), None);
        assert_eq!(shares(&[v(f64::NAN, Motif::Index)]), None);
        assert_eq!(
            shares(&[v(f64::MAX, Motif::Index), v(f64::MAX, Motif::Major)]),
            None
        );
    }
}
