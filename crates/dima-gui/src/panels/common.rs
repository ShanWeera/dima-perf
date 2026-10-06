//! Small drawing helpers shared by the panels.

use std::sync::Arc;

use egui::{Color32, RichText, Ui};

use crate::theme::DesignTokens;
use crate::util::truncate_display;

/// A section heading.
pub fn heading(ui: &mut Ui, t: &DesignTokens, text: impl Into<String>) {
    ui.label(
        RichText::new(text)
            .size(t.font_size_heading)
            .family(egui::FontFamily::Name(crate::theme::fonts::SEMIBOLD.into())),
    );
}

/// A sub-heading, one level below [`heading`].
pub fn sub_heading(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).strong());
}

/// A label for an option, with a dot and a "default: X" tooltip when its value differs from
/// the default (D73).
pub fn option_label(
    ui: &mut Ui,
    t: &DesignTokens,
    text: &str,
    help: &str,
    default: Option<String>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let label = ui.label(text).on_hover_text(help);
        let _ = label;
        if let Some(d) = default {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 3.5, t.accent);
            response
                .on_hover_text(format!("Changed from the default (default: {d})"))
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Label,
                        true,
                        format!("{text} changed; default: {d}"),
                    )
                });
        }
    });
}

/// A small coloured swatch followed by a caption, as one item (so wrapping never separates
/// them). `kind` draws the swatch.
pub fn legend_entry(ui: &mut Ui, t: &DesignTokens, caption: &str, kind: Swatch) -> egui::Response {
    const W: f32 = 22.0;
    let galley = ui.painter().layout_no_wrap(
        caption.to_owned(),
        egui::FontId::proportional(t.font_size_caption),
        t.text_secondary,
    );
    let size = egui::vec2(W + 6.0 + galley.size().x, galley.size().y.max(12.0));
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    let mid = rect.center().y;
    let sw = egui::Rect::from_min_size(egui::pos2(rect.left(), mid - 6.0), egui::vec2(W, 12.0));
    match kind {
        Swatch::Line(color) => {
            p.line_segment(
                [egui::pos2(sw.left(), mid), egui::pos2(sw.right(), mid)],
                egui::Stroke::new(2.0, color),
            );
        }
        Swatch::Dashed(color) => {
            p.extend(egui::Shape::dashed_line(
                &[egui::pos2(sw.left(), mid), egui::pos2(sw.right(), mid)],
                egui::Stroke::new(2.0, color),
                5.0,
                3.0,
            ));
        }
        Swatch::Hatched(fill, hatch) => {
            p.rect_filled(sw, 0.0, fill);
            hatch_rect(p, sw, hatch, 5.0);
        }
        Swatch::Block(color) => {
            p.rect_filled(sw.shrink2(egui::vec2(5.0, 2.0)), 2.0, color);
        }
        Swatch::Tick(color) => {
            p.line_segment(
                [
                    egui::pos2(sw.center().x, sw.top()),
                    egui::pos2(sw.center().x, sw.bottom()),
                ],
                egui::Stroke::new(2.0, color),
            );
        }
    }
    p.galley(
        egui::pos2(sw.right() + 6.0, mid - galley.size().y / 2.0),
        galley,
        t.text_secondary,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, caption));
    response
}

/// Legend swatch shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swatch {
    /// A solid line.
    Line(Color32),
    /// A dashed line.
    Dashed(Color32),
    /// A hatched box: fill and hatch colours.
    Hatched(Color32, Color32),
    /// A filled block.
    Block(Color32),
    /// A vertical tick.
    Tick(Color32),
}

/// Diagonal ("/") hatch lines over `rect`, every `spacing` points, clipped to it.
pub fn hatch_rect(painter: &egui::Painter, rect: egui::Rect, color: Color32, spacing: f32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let p = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let h = rect.height();
    let mut x = rect.left() - h;
    let stroke = egui::Stroke::new(1.0, color);
    while x < rect.right() {
        p.line_segment(
            [egui::pos2(x, rect.bottom()), egui::pos2(x + h, rect.top())],
            stroke,
        );
        x += spacing;
    }
}

/// One line of `text` elided with "…" to `max_width` (laid out from a bounded prefix, so a
/// pathological value never costs a frame).
#[must_use]
pub fn elided(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        truncate_display(text, 160),
        egui::TextFormat::simple(font, color),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(0.0));
    ui.painter().layout_job(job)
}

/// A disclosure triangle (painted, so it never depends on font coverage).
pub fn disclosure(painter: &egui::Painter, rect: egui::Rect, open: bool, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.3;
    let points = if open {
        vec![
            egui::pos2(c.x - s, c.y - s * 0.6),
            egui::pos2(c.x + s, c.y - s * 0.6),
            egui::pos2(c.x, c.y + s * 0.8),
        ]
    } else {
        vec![
            egui::pos2(c.x - s * 0.6, c.y - s),
            egui::pos2(c.x - s * 0.6, c.y + s),
            egui::pos2(c.x + s * 0.8, c.y),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

/// A muted "key value" pair in a grid row.
pub fn fact(ui: &mut Ui, t: &DesignTokens, key: &str, value: impl Into<String>) {
    ui.label(RichText::new(key).color(t.text_secondary));
    ui.label(value.into());
    ui.end_row();
}

/// A frame for a card (setup sections, error cards).
pub fn card(t: &DesignTokens) -> egui::Frame {
    egui::Frame::new()
        .fill(t.surface_secondary)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(t.panel_rounding)
        .inner_margin(t.space_12)
}

/// A sortable column header: a frameless button (reachable with Tab and announced as a button
/// by screen readers) showing the column name and, when the table is sorted by it, the
/// direction arrow.
pub fn sort_header(ui: &mut Ui, name: &str, arrow: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(format!("{name}{arrow}")).strong()).frame(false))
        .on_hover_text(format!("Sort by {}", name.to_lowercase()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elision_fits_and_never_panics() {
        let ctx = egui::Context::default();
        crate::theme::fonts::install(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = egui::FontId::proportional(13.0);
            let g = elided(
                ui,
                "a value far too long for a narrow inspector column",
                font.clone(),
                Color32::BLACK,
                60.0,
            );
            assert!(g.elided);
            assert!(g.size().x <= 61.0);
            assert!(!elided(ui, "host", font.clone(), Color32::BLACK, 200.0).elided);
            for w in [0.0, -10.0, f32::NAN] {
                let _ = elided(
                    ui,
                    "x".repeat(500).as_str(),
                    font.clone(),
                    Color32::BLACK,
                    w,
                );
            }
        });
        out.textures_delta.clear();
    }
}
