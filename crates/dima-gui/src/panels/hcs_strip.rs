//! Historically conserved sequences (D86): the regions as a strip under the chart, on the
//! chart's own x mapping so they line up at every zoom and filter (G18); flagged regions
//! (positions with support ≤ T in their span) are hatched and marked with an asterisk. The
//! tooltip shows the region's minimum incidence, minimum support, 95% Clopper–Pearson lower
//! bound and low-support positions (G59), found by binary search (no per-hover scan).

use egui::{RichText, Stroke, Ui};

use dima_core::human::percent_bounded;
use dima_core::model::{HcsRegion, HcsResults};

use super::analysis_options::hcs_controls;
use super::common::{hatch_rect, heading};
use super::entropy_chart::{LEFT_MARGIN, RIGHT_MARGIN, Shared};
use crate::app::msg::Msg;
use crate::state::AppState;
use crate::state::hcs::region_at;
use crate::theme::DesignTokens;
use crate::util::truncate_display;

const STRIP_HEIGHT: f32 = 22.0;

/// The tooltip of a region (pure, for tests). Its minimum incidence, minimum support and lower
/// bound are the lowest among its members, position by position.
#[must_use]
pub fn region_tooltip(r: &HcsRegion, t: u64) -> String {
    let mut s = format!(
        "Region {}: columns {}\u{2013}{} ({} columns)\n{}\nminimum Index incidence {}\nminimum support {}\nlowest per-position 95% lower bound {} (Clopper\u{2013}Pearson)",
        r.id,
        r.start_column,
        r.end_column,
        r.length,
        truncate_display(&r.sequence, 60),
        percent_bounded(r.min_incidence, 2),
        r.min_support,
        percent_bounded(r.conservation_lower_bound_95 * 100.0, 1)
    );
    if r.flagged {
        let shown: Vec<String> = r
            .low_support_positions
            .iter()
            .take(12)
            .map(u64::to_string)
            .collect();
        let more = r.low_support_positions.len().saturating_sub(12);
        s.push_str(&format!(
            "\n* flagged: {} with support \u{2264} {t}: {}{}",
            dima_core::human::plural(
                r.low_support_positions.len() as u64,
                "position",
                "positions"
            ),
            shown.join(dima_core::human::separators::ITEM),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        ));
    }
    if r.split_from_previous {
        s.push_str("\nsplit from the previous region (their overlapping k-mers disagree)");
    }
    s
}

/// What the strip says to screen readers (it is painted): "Conserved regions: 1 region, 0
/// flagged", counted in words that fit one.
#[must_use]
pub fn strip_label(r: &HcsResults) -> String {
    format!(
        "Conserved regions: {}{}{} flagged",
        dima_core::human::plural(r.region_count, "region", "regions"),
        dima_core::human::separators::ITEM,
        r.flagged_region_count
    )
}

/// Draws the controls and the strip; `shared` is the chart's mapping (none when the chart has
/// nothing to show).
pub fn show(
    ui: &mut Ui,
    state: &mut AppState,
    shared: Option<Shared>,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    let Some(session_id) = state.session.as_ref().map(|s| s.id) else {
        return;
    };
    let threshold = state
        .session
        .as_ref()
        .map_or(0, |s| s.results.support_threshold);
    let result = state.hcs.current(session_id).cloned();
    ui.horizontal_wrapped(|ui| {
        heading(ui, t, "Conserved regions");
        match &result {
            Some(r) => {
                let text = format!(
                    "{}{}{} flagged",
                    dima_core::human::plural(r.region_count, "region", "regions"),
                    dima_core::human::separators::ITEM,
                    r.flagged_region_count
                );
                ui.label(RichText::new(text).color(t.text_secondary))
                    .on_hover_text(format!(
                        "Flagged regions include positions with support \u{2264} {threshold}"
                    ));
            }
            None if state.hcs.running.is_some() || state.hcs.pending_since.is_some() => {
                ui.spinner();
                ui.weak("computing\u{2026}");
            }
            None => {}
        }
        if let Some(f) = &state.hcs.failure {
            ui.colored_label(t.error_color, f);
        }
    });
    hcs_controls(ui, state, t, out);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), STRIP_HEIGHT),
        egui::Sense::click(),
    );
    // A Tab stop, for its description; outlined while it has the focus.
    super::common::focus_outline(ui, &response);
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + LEFT_MARGIN, rect.top()),
        egui::pos2(rect.right() - RIGHT_MARGIN, rect.bottom()),
    );
    let painter = ui.painter_at(strip);
    painter.rect_stroke(
        strip,
        2.0,
        Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    let (Some(r), Some(shared)) = (result, shared) else {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                true,
                "Conserved regions: none shown",
            )
        });
        return;
    };
    draw_regions(&painter, &r, shared, strip, t);
    let label = strip_label(&r);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &label));
    if r.regions.is_empty() {
        painter.text(
            strip.center(),
            egui::Align2::CENTER_CENTER,
            format!(
                "No conserved regions at {}% \u{2014} try a lower threshold",
                r.parameters.threshold_percent
            ),
            egui::FontId::proportional(t.font_size_caption),
            t.text_muted,
        );
    }
    if let Some(p) = response.hover_pos()
        && let Some(region) = region_at(&r.regions, shared.x.to_data(p.x))
    {
        response
            .clone()
            .on_hover_text_at_pointer(region_tooltip(region, threshold));
        if response.clicked() {
            out.push(Msg::Select(
                region.members[0].position,
                crate::state::session::SelectSource::Chart,
            ));
        }
    }
}

fn draw_regions(
    p: &egui::Painter,
    r: &HcsResults,
    shared: Shared,
    strip: egui::Rect,
    t: &DesignTokens,
) {
    let x = shared.x;
    let font = egui::FontId::proportional(t.font_size_body);
    for region in &r.regions {
        #[allow(clippy::cast_precision_loss)]
        let (a, b) = (
            x.to_screen(region.start_column as f64 - 0.5),
            x.to_screen(region.end_column as f64 + 0.5),
        );
        if b < strip.left() || a > strip.right() {
            continue;
        }
        let rect = egui::Rect::from_min_max(
            egui::pos2(a.max(strip.left()), strip.top() + 3.0),
            egui::pos2(b.min(strip.right()).max(a + 1.0), strip.bottom() - 3.0),
        );
        p.rect_filled(rect, 1.0, t.hcs_region);
        if region.flagged {
            hatch_rect(p, rect, t.hcs_flag_hatch, 4.0);
            if rect.width() >= 8.0 {
                p.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "*",
                    font.clone(),
                    t.hcs_flag_hatch,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flagged_tooltips_list_low_support_positions() {
        let results = crate::tests::fixtures::mers_nt();
        let hcs = dima_core::hcs::compute(&results, &dima_core::hcs::HcsParams::default()).unwrap();
        assert_eq!(
            hcs.region_count, 10,
            "MERS spike nt at 100% (plan, Verification)"
        );
        assert_eq!(hcs.flagged_region_count, 1);
        let flagged = hcs.regions.iter().find(|r| r.flagged).unwrap();
        let text = region_tooltip(flagged, results.support_threshold);
        assert!(text.contains("columns 3518\u{2013}3572"), "{text}");
        assert!(text.contains("minimum support 2"), "{text}");
        assert!(
            text.contains("lowest per-position 95% lower bound 15.8% (Clopper\u{2013}Pearson)"),
            "{text}"
        );
        assert!(text.contains("* flagged: 29 positions"), "{text}");
        assert!(text.contains("and 17 more"), "{text}");
        // An incidence or a bound below 100% never reads as 100%.
        let near = HcsRegion {
            min_incidence: 99.996,
            conservation_lower_bound_95: 0.99996,
            ..flagged.clone()
        };
        let text = region_tooltip(&near, results.support_threshold);
        assert!(text.contains("minimum Index incidence > 99.99%"), "{text}");
        assert!(
            text.contains("lowest per-position 95% lower bound > 99.9%"),
            "{text}"
        );
    }

    /// The strip's name for screen readers counts its regions in words that fit one.
    #[test]
    fn the_strip_label_counts_in_words() {
        let results = crate::tests::fixtures::mers();
        let mut hcs =
            dima_core::hcs::compute(&results, &dima_core::hcs::HcsParams::default()).unwrap();
        assert_eq!(
            strip_label(&hcs),
            "Conserved regions: 44 regions, 0 flagged"
        );
        hcs.region_count = 1;
        assert_eq!(strip_label(&hcs), "Conserved regions: 1 region, 0 flagged");
    }
}
