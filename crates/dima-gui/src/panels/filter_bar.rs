//! The filter bar: position range, the value range of the metric the chart plots (entropy, or
//! similarity-weighted entropy), motif classes and the low-support switch, with the shown/total
//! count and "Reset". Changes are applied when the pointer is released (a drag does not
//! refilter on every tick).
//!
//! The two value fields show four decimals (a drag value otherwise picks its decimals from its
//! speed and the screen's pixel size), a minimum that reads as 0 is 0 and a maximum that reads
//! as the metric's largest value is that value exactly: a drag value stores the text it showed
//! when it loses the focus, so tabbing through a field would otherwise narrow the range by the
//! rounding.
//!
//! The bar does not rearrange when its texts change: the value fields sit in slots as wide as
//! the widest value of the metrics the results hold, the count and Reset start a row of their
//! own whenever the longest count would not fit at the end of the last one, and when the metric
//! can be switched the value label keeps the width of the longer name.

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
        let before = *v;
        for (bound, prefix, end) in [(&mut v.0, "min ", 0.0), (&mut v.1, "max ", max)] {
            let width = ChartMetric::ALL
                .into_iter()
                .filter(|m| m.available(&s.results))
                .map(|m| value_field_width(ui, prefix, b.max_for(m).max(0.0)))
                .fold(0.0, f32::max);
            let field = egui::DragValue::new(&mut *bound)
                .prefix(prefix)
                .speed(speed)
                .range(0.0..=max)
                .fixed_decimals(VALUE_DECIMALS);
            // Not while the field is being typed in, which would replace the text being typed.
            if !fixed_width_slot(ui, width, |ui| ui.add(field)).has_focus() {
                *bound = snapped(*bound, end);
            }
        }
        // Only a change of the bounds themselves: leaving a field changes nothing by itself.
        changed |= *v != before;
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
        // over its controls, so they start a row of their own instead. The room is that of the
        // longest count (the interface's digits are all as wide), so a count that changes with
        // a filter or a metric switch never moves them to another row.
        let longest = format!("{total} of {total} positions");
        if ui.available_size_before_wrap().x < right_group_width(ui, &longest) {
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

/// Decimals of the value fields.
const VALUE_DECIMALS: usize = 4;

/// Half the value of the fields' last decimal: a bound nearer than this to its end of the range
/// (0 for the minimum, the largest value for the maximum) reads as that end.
const HALF_STEP: f64 = 0.5e-4;

/// `bound`, or `end` when the bound reads as it (see [`HALF_STEP`]).
fn snapped(bound: f64, end: f64) -> f64 {
    if (bound - end).abs() < HALF_STEP {
        end
    } else {
        bound
    }
}

/// The width of a value field showing `prefix` and `max` with [`VALUE_DECIMALS`] decimals: its
/// text in the drag-value style, the button's padding on both sides, and one digit to spare
/// for measuring the text whole rather than in its two parts.
fn value_field_width(ui: &Ui, prefix: &str, max: f64) -> f32 {
    let font = ui.style().drag_value_text_style.resolve(ui.style());
    let spacing = ui.spacing();
    let text = format!("{prefix}{max:.VALUE_DECIMALS$}");
    let width = text_width(ui, &text, font.clone())
        + 2.0 * spacing.button_padding.x
        + text_width(ui, "0", font);
    width.max(spacing.interact_size.x)
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
    use super::{HALF_STEP, VALUE_DECIMALS, snapped};

    /// A bound that reads as its end of the range is that end, so a field that is left without
    /// a change keeps the full range exactly; one that reads otherwise is kept.
    #[test]
    fn a_bound_that_reads_as_an_end_is_that_end() {
        // The largest entropy of the MERS spike sample, and maxima that round up and down.
        for max in [1.587_825_175_645_670_3, 0.454_776, 0.454_74, 2.0, 0.000_03] {
            let shown: f64 = format!("{max:.VALUE_DECIMALS$}").parse().unwrap();
            assert_eq!(snapped(shown, max), max, "{max}");
            assert_eq!(snapped(max, max), max);
        }
        assert_eq!(snapped(1.5877, 1.587_825_175_645_670_3), 1.5877);
        assert_eq!(snapped(0.0, 0.0), 0.0);
        assert_eq!(snapped(0.4 * HALF_STEP, 0.0), 0.0);
        assert_eq!(
            snapped(0.0001, 0.0),
            0.0001,
            "the smallest step shown is kept"
        );
        assert_eq!(
            format!("{:.VALUE_DECIMALS$}", 2.0 * HALF_STEP),
            format!("0.{}1", "0".repeat(VALUE_DECIMALS - 1)),
            "half the last decimal shown"
        );
    }

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
