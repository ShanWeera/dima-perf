//! The filter bar: position range, the value range of the metric the chart plots (entropy, or
//! similarity-weighted entropy), motif classes and the low-support switch, with the shown/total
//! count and "Reset". Changes are applied when the pointer is released (a drag does not
//! refilter on every tick). When the metric can be switched, its label keeps the width of the
//! longer name, so the bar does not rearrange on a switch.

use egui::{RichText, Ui};

use super::common::{fixed_width_slot, text_width};
use super::help_text;
use crate::app::msg::Msg;
use crate::charts::metric::ChartMetric;
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
        let metric = s.metric;
        let label = |ui: &mut Ui| {
            ui.label(metric.name())
                .on_hover_text(help_text::value_range(metric));
        };
        if ChartMetric::SimilarityWeighted.available(&s.results) {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let width = ChartMetric::ALL
                .map(|m| text_width(ui, m.name(), font.clone()))
                .into_iter()
                .fold(0.0, f32::max);
            fixed_width_slot(ui, width, label);
        } else {
            label(ui);
        }
        let v = &mut s.filter.value_range;
        let max = b.max_for(metric).max(0.0);
        let speed = drag_speed(max);
        changed |= ui
            .add(egui::DragValue::new(&mut v.0).prefix("min ").speed(speed).range(0.0..=max).max_decimals(4))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut v.1).prefix("max ").speed(speed).range(0.0..=max).max_decimals(4))
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
        let (shown, total) = (s.shown.len(), s.results.results.len());
        let count = format!("{shown} of {total} positions");
        // The count and Reset end the last row; a row without room for them would put them
        // over its controls, so they start a row of their own instead.
        if ui.available_size_before_wrap().x < right_group_width(ui, &count) {
            ui.end_row();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(!s.filter.is_everything(b, metric), egui::Button::new("Reset"))
                .on_hover_text("Show every position")
                .clicked()
            {
                out.push(Msg::ResetFilters);
            }
            if shown == total {
                ui.weak(count);
            } else {
                ui.label(RichText::new(count).strong().color(t.accent));
            }
        });
    });
    if changed {
        out.push(Msg::FiltersEdited);
    }
}

/// Value change per pixel dragged on a range of `0..=max` bits: the full range in 200 pixels,
/// whatever the metric's scale (similarity-weighted entropy is often a small fraction of a bit).
fn drag_speed(max: f64) -> f64 {
    (max / 200.0).max(1e-4)
}

/// The width the count `text` and the Reset button take at the end of the bar.
fn right_group_width(ui: &Ui, text: &str) -> f32 {
    let width = |s: &str, style: egui::TextStyle| {
        ui.painter()
            .layout_no_wrap(
                s.to_owned(),
                style.resolve(ui.style()),
                egui::Color32::PLACEHOLDER,
            )
            .size()
            .x
    };
    let spacing = ui.spacing();
    width(text, egui::TextStyle::Body)
        + width("Reset", egui::TextStyle::Button)
        + 2.0 * spacing.button_padding.x
        + 2.0 * spacing.item_spacing.x
}

#[cfg(test)]
mod tests {
    #[test]
    fn dragging_crosses_the_range_in_200_pixels() {
        assert!((super::drag_speed(1.6) - 0.008).abs() < 1e-12);
        assert!((super::drag_speed(0.02) - 0.0001).abs() < 1e-12);
        assert_eq!(
            super::drag_speed(0.0),
            1e-4,
            "an all-zero range still moves"
        );
    }
}
