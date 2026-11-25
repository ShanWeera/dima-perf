//! Header metadata of the selected variant: its count per value of each stratified field, in
//! the header-format order (G54), values by count. A field lists its top five values, or all
//! (at most 200, the rest counted) on request; which fields are open stays as the user left
//! it while moving between positions and variants, and resets with a new dataset. Value orders
//! are cached per variant (`state::metadata`), so a frame draws without copying or sorting.

use std::sync::Arc;

use egui::{RichText, Ui};

use super::common::{disclosure, elided, heading};
use crate::state::metadata::{MAX_VALUES, TOP_VALUES};
use crate::state::session::Session;
use crate::theme::DesignTokens;
use crate::util::truncate_display;

const INDENT: f32 = 22.0;

/// Draws the section (only when the results carry metadata).
pub fn show(ui: &mut Ui, s: &mut Session, t: &DesignTokens) {
    if s.fields.is_empty() {
        return;
    }
    let (Some(position), Some((variant, _))) = (s.selection.position, s.selected_variant()) else {
        heading(ui, t, "Metadata");
        ui.colored_label(t.text_muted, "Select a variant to see its metadata");
        return;
    };
    // The results are shared, so the variant can be read while the view state changes.
    let results = Arc::clone(&s.results);
    let Some(v) = s
        .index_of(position)
        .and_then(|i| results.results[i].diversity_motifs.get(variant))
    else {
        return;
    };
    heading(
        ui,
        t,
        format!("Metadata of {}", truncate_display(&v.sequence, 16)),
    );
    let Some(md) = &v.metadata else {
        ui.colored_label(t.text_muted, "This variant carries no metadata");
        return;
    };
    if let Some(first) = md.0.first() {
        s.metadata.open_first(&first.field);
    }
    let mut toggle_open = None;
    let mut toggle_all = None;
    for (index, field) in md.0.iter().enumerate() {
        let open = s.metadata.is_open(&field.field);
        if field_header(ui, t, &field.field, field.values.len(), open) {
            toggle_open = Some(field.field.clone());
        }
        if !open {
            continue;
        }
        let all = s.metadata.all.contains(&field.field);
        let order = s.metadata.order((position, variant), index, field);
        if order.total == 0 {
            continue;
        }
        let shown = order
            .top
            .len()
            .min(if all { MAX_VALUES } else { TOP_VALUES });
        for &i in &order.top[..shown] {
            let (value, count) = &field.values[i];
            value_row(ui, t, value, *count, order.total);
        }
        if all && shown < order.len {
            ui.horizontal(|ui| {
                ui.add_space(INDENT);
                ui.weak(format!("{} more not listed", order.len - shown));
            });
        }
        if order.len > TOP_VALUES {
            let len = order.len;
            ui.horizontal(|ui| {
                ui.add_space(INDENT);
                let text = if all {
                    format!("Show top {TOP_VALUES}")
                } else {
                    format!("Show all {len} values")
                };
                if ui.link(text).clicked() {
                    toggle_all = Some(field.field.clone());
                }
            });
        }
    }
    if let Some(f) = toggle_open {
        s.metadata.toggle_open(f);
    }
    if let Some(f) = toggle_all {
        s.metadata.toggle_all(f);
    }
}

fn field_header(ui: &mut Ui, t: &DesignTokens, field: &str, values: usize, open: bool) -> bool {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("metadata_field", field)),
        egui::Sense::click(),
    );
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect, 4.0, t.hover_highlight);
    }
    if response.has_focus() {
        p.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, t.accent),
            egui::StrokeKind::Inside,
        );
    }
    let tri = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 7.0, rect.center().y),
        egui::Vec2::splat(14.0),
    );
    disclosure(p, tri, open, t.text_secondary);
    let count = p.text(
        egui::pos2(rect.right(), rect.center().y),
        egui::Align2::RIGHT_CENTER,
        dima_core::human::plural(values as u64, "value", "values"),
        egui::FontId::proportional(t.font_size_caption),
        t.text_muted,
    );
    let name = elided(
        ui,
        field,
        egui::TextStyle::Body.resolve(ui.style()),
        t.text_primary,
        count.left() - 8.0 - tri.right(),
    );
    ui.painter().galley(
        egui::pos2(tri.right() + 2.0, rect.center().y - name.size().y / 2.0),
        name,
        t.text_primary,
    );
    let label = format!(
        "{field}, {values} values, {}",
        if open { "expanded" } else { "collapsed" }
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// "84.5% (169)": a value's `share` of the sequences (0 to 1) as a percentage, which never
/// reads 100.0% below all of them nor 0.0% above none, and its `count`.
fn share_text(share: f64, count: u64) -> String {
    format!(
        "{} ({count})",
        dima_core::human::percent_bounded(share * 100.0, 1)
    )
}

fn value_row(ui: &mut Ui, t: &DesignTokens, value: &str, count: u64, total: u64) {
    #[allow(clippy::cast_precision_loss)]
    let share = (count as f64 / total as f64).clamp(0.0, 1.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::hover());
    let p = ui.painter();
    let pct = p.text(
        egui::pos2(rect.right(), rect.top() + 8.0),
        egui::Align2::RIGHT_CENTER,
        share_text(share, count),
        egui::FontId::proportional(t.font_size_caption),
        t.text_secondary,
    );
    let left = rect.left() + INDENT;
    let name = elided(
        ui,
        value,
        egui::TextStyle::Body.resolve(ui.style()),
        t.text_primary,
        pct.left() - 6.0 - left,
    );
    ui.painter().galley(
        egui::pos2(left, rect.top() + 8.0 - name.size().y / 2.0),
        name,
        t.text_primary,
    );
    let track =
        egui::Rect::from_min_max(egui::pos2(left, rect.bottom() - 5.0), rect.right_bottom());
    if track.width() > 0.0 {
        let p = ui.painter();
        p.rect_filled(track, 2.0, t.border);
        #[allow(clippy::cast_possible_truncation)]
        let w = (share as f32 * track.width()).max(2.0).min(track.width());
        p.rect_filled(
            egui::Rect::from_min_size(track.min, egui::vec2(w, track.height())),
            2.0,
            t.accent,
        );
    }
    let label = format!("{value}, {count} of {total}");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
    response.on_hover_text(RichText::new(format!("{value}: {count} of {total}")));
}

#[cfg(test)]
mod tests {
    /// A share just below all the sequences, or just above none, is never shown as all or none.
    #[test]
    fn shares_never_round_to_all_or_none() {
        let text = |count: u64, total: u64| {
            #[allow(clippy::cast_precision_loss)]
            super::share_text(count as f64 / total as f64, count)
        };
        assert_eq!(text(9_999, 10_000), "> 99.9% (9999)");
        assert_eq!(text(10_000, 10_000), "100.0% (10000)");
        assert_eq!(text(1, 100_000), "< 0.1% (1)");
        assert_eq!(text(169, 200), "84.5% (169)");
    }
}
