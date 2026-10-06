//! The filter bar: position range, entropy range, motif classes and the low-support switch,
//! with the shown/total count and "Reset". Changes are applied when the pointer is released
//! (a drag does not refilter on every tick).

use egui::{RichText, Ui};

use crate::app::msg::Msg;
use crate::state::filter::MOTIFS;
use crate::state::session::Session;
use crate::theme::DesignTokens;

/// Draws the bar.
pub fn show(ui: &mut Ui, s: &mut Session, t: &DesignTokens, out: &mut Vec<Msg>) {
    let b = s.bounds;
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Positions")
            .on_hover_text("Show a range of positions");
        let r = &mut s.filter.position_range;
        changed |= ui
            .add(egui::DragValue::new(&mut r.0).prefix("from ").range(b.positions.0..=b.positions.1))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut r.1).prefix("to ").range(b.positions.0..=b.positions.1))
            .changed();
        ui.add_space(t.space_12);
        ui.label("Entropy").on_hover_text(
            "Show positions whose entropy is in this range (bits). Positions without usable \
             support have no entropy and are not affected.",
        );
        let e = &mut s.filter.entropy_range;
        let max = b.max_entropy.max(0.0);
        changed |= ui
            .add(egui::DragValue::new(&mut e.0).prefix("min ").speed(0.01).range(0.0..=max).max_decimals(4))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut e.1).prefix("max ").speed(0.01).range(0.0..=max).max_decimals(4))
            .changed();
        ui.add_space(t.space_12);
        ui.label("Motifs")
            .on_hover_text("Show positions with at least one variant of a checked motif");
        for (i, m) in MOTIFS.iter().enumerate() {
            changed |= ui.checkbox(&mut s.filter.motifs[i], m.long()).changed();
        }
        ui.add_space(t.space_12);
        changed |= ui
            .checkbox(&mut s.filter.include_low_support, "Low support (NS, LS, ELS)")
            .on_hover_text("Show positions with support \u{2264} T: none (NS), below T (LS) and equal to T (ELS)")
            .changed();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(!s.filter.is_everything(b), egui::Button::new("Reset"))
                .on_hover_text("Show every position")
                .clicked()
            {
                out.push(Msg::ResetFilters);
            }
            let (shown, total) = (s.shown.len(), s.results.results.len());
            let text = format!("{shown} of {total} positions");
            if shown == total {
                ui.weak(text);
            } else {
                ui.label(RichText::new(text).strong().color(t.accent));
            }
        });
    });
    if changed {
        out.push(Msg::FiltersEdited);
    }
}
