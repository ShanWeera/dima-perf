//! Small drawing helpers shared by the panels.

use std::sync::Arc;

use egui::{Color32, RichText, Ui};

use crate::theme::DesignTokens;
use crate::util::truncate_display;

/// The glyph of the buttons that close or remove something: × (U+00D7), which the bundled IBM
/// Plex Sans draws (✕, U+2715, is in none of the app's fonts and would show as an empty box).
pub const CLOSE: &str = "\u{00D7}";

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
/// the default (D73). The dot's place is kept when there is no dot, so the column of labels
/// keeps its width and no control moves when an option is changed (a second click at the same
/// place reaches the same control).
pub fn option_label(
    ui: &mut Ui,
    t: &DesignTokens,
    text: &str,
    help: &str,
    default: Option<String>,
) {
    const DOT: egui::Vec2 = egui::vec2(8.0, 8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(text).on_hover_text(help);
        let Some(d) = default else {
            ui.allocate_space(DOT);
            return;
        };
        let (rect, response) = ui.allocate_exact_size(DOT, egui::Sense::hover());
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
    });
}

/// Width of a legend swatch.
const SWATCH_WIDTH: f32 = 22.0;
/// From a swatch to its caption.
const SWATCH_GAP: f32 = 6.0;

/// A small coloured swatch followed by a caption, as one item (so wrapping never separates
/// them). `kind` draws the swatch.
pub fn legend_entry(ui: &mut Ui, t: &DesignTokens, caption: &str, kind: Swatch) -> egui::Response {
    legend_entry_at_least(ui, t, caption, kind, 0.0)
}

/// The width [`legend_entry`] takes for `caption`.
#[must_use]
pub fn legend_entry_width(ui: &Ui, t: &DesignTokens, caption: &str) -> f32 {
    SWATCH_WIDTH
        + SWATCH_GAP
        + text_width(ui, caption, egui::FontId::proportional(t.font_size_caption))
}

/// [`legend_entry`] in a slot at least `width` wide (the caption left-aligned in it), so what
/// follows keeps its place when the caption changes.
pub fn legend_entry_at_least(
    ui: &mut Ui,
    t: &DesignTokens,
    caption: &str,
    kind: Swatch,
    width: f32,
) -> egui::Response {
    const W: f32 = SWATCH_WIDTH;
    let galley = ui.painter().layout_no_wrap(
        caption.to_owned(),
        egui::FontId::proportional(t.font_size_caption),
        t.text_secondary,
    );
    let size = egui::vec2(
        (W + SWATCH_GAP + galley.size().x).max(width),
        galley.size().y.max(12.0),
    );
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
        egui::pos2(sw.right() + SWATCH_GAP, mid - galley.size().y / 2.0),
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

/// The width of `text` laid out on one line in `font`.
#[must_use]
pub fn text_width(ui: &Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER)
        .size()
        .x
}

/// Lays out `add_contents` left-aligned in a slot `width` wide and one row high (taller if the
/// contents are), as one item of the row, so whatever follows keeps its place when the
/// contents change.
pub fn fixed_width_slot<R>(ui: &mut Ui, width: f32, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            add_contents(ui)
        },
    )
    .inner
}

/// Gives a control its hover text and, from the same text, its AccessKit description, so a
/// screen reader says what the tooltip shows.
pub fn with_help(response: egui::Response, text: &str) -> egui::Response {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_description(text));
    response.on_hover_text(text)
}

/// Names a control whose own text does not say what it is (a field that shows only its value):
/// `name` is its AccessKit name, which a screen reader says first, and its hover text. Call it
/// after the control is added, whose own AccessKit node it completes.
pub fn with_name(response: egui::Response, name: &str) -> egui::Response {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response.on_hover_text(name)
}

/// Draws a keyboard-focus outline around a control that draws none of its own (a frameless
/// button, a clickable label) while it has the focus, so the focus is always visible.
pub fn focus_outline(ui: &Ui, response: &egui::Response) {
    if response.has_focus() {
        let visuals = ui.visuals();
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            visuals.widgets.active.corner_radius,
            visuals.widgets.active.bg_stroke,
            egui::StrokeKind::Outside,
        );
    }
}

/// One row as wide as the space left: `trailing` (controls such as buttons) at its top right,
/// laid out right to left, and `leading` (what the row says) left-aligned in the width left of
/// them less `gap`, wrapping onto further lines as it needs. `leading` is created first, so a
/// screen reader reads it before the controls at its end, as the row reads; `trailing` is laid
/// out first, so `leading` knows its width. Returns what both return.
pub fn row_with_trailing<L, T>(
    ui: &mut Ui,
    gap: f32,
    leading: impl FnOnce(&mut Ui) -> L,
    trailing: impl FnOnce(&mut Ui) -> T,
) -> (L, T) {
    let full = ui.available_rect_before_wrap();
    let first_line = egui::Rect::from_min_size(
        full.min,
        egui::vec2(full.width(), ui.spacing().interact_size.y),
    );
    let mut lead = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("leading")
            .max_rect(first_line)
            .layout(egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true)),
    );
    let mut trail = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("trailing")
            .max_rect(full)
            .layout(egui::Layout::right_to_left(egui::Align::Min)),
    );
    let t = trailing(&mut trail);
    let trail_rect = trail.min_rect();
    lead.set_max_width((trail_rect.left() - gap - full.left()).max(0.0));
    let l = leading(&mut lead);
    ui.advance_cursor_after_rect(lead.min_rect().union(trail_rect));
    (l, t)
}

/// A muted "key value" pair in a grid row; returns the key's response (for its tooltip).
pub fn fact(ui: &mut Ui, t: &DesignTokens, key: &str, value: impl Into<String>) -> egui::Response {
    let key = ui.label(RichText::new(key).color(t.text_secondary));
    ui.label(value.into());
    ui.end_row();
    key
}

/// [`fact`] with the key in a cell `key_width` wide, so grids drawn one below another line up
/// their values; a key with a line break takes two lines. The cell is exactly as tall as its
/// text, so the grid centres it in the row like any other label.
pub fn fact_at(
    ui: &mut Ui,
    t: &DesignTokens,
    key: &str,
    value: impl Into<String>,
    key_width: f32,
) -> egui::Response {
    let galley = ui.painter().layout(
        key.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        t.text_secondary,
        f32::INFINITY,
    );
    let size = egui::vec2(key_width.max(galley.size().x), galley.size().y);
    let (rect, key_response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().galley(rect.min, galley, t.text_secondary);
    key_response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, key));
    ui.label(value.into());
    ui.end_row();
    key_response
}

/// The width of the key column that fits every line of `keys` in the body font.
#[must_use]
pub fn key_column_width(ui: &Ui, keys: &[&str]) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    keys.iter()
        .flat_map(|k| k.lines())
        .map(|line| text_width(ui, line, font.clone()))
        .fold(0.0, f32::max)
}

/// A normalised entropy as the position facts and the chart's tooltip show it: the value, marked
/// "(low support)" at support ≤ T (LS, ELS), where it is less reliable; "— (one variant)" where
/// it is undefined because the position has a single distinct k-mer; "—" without support.
#[must_use]
pub fn normalised_value(value: Option<f64>, p: &dima_core::model::PositionResult) -> String {
    use crate::charts::series::Class;
    match value {
        Some(v) => {
            let shown = dima_core::human::fixed(v, 4);
            if matches!(Class::of(p), Class::Low | Class::EqualToThreshold) {
                format!("{shown} (low support)")
            } else {
                shown
            }
        }
        None if p.diversity_motifs.len() == 1 => "\u{2014} (one variant)".to_owned(),
        None => "\u{2014}".to_owned(),
    }
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
/// direction arrow. "Sort by `sort_name`" is its tooltip and its AccessKit description; a
/// header that has the keyboard focus is outlined (its text alone does not change) and scrolls
/// into view when it gains it.
pub fn sort_header(ui: &mut Ui, name: &str, sort_name: &str, arrow: &str) -> egui::Response {
    let response = ui
        .add(egui::Button::new(RichText::new(format!("{name}{arrow}")).strong()).frame(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let response = with_help(response, &format!("Sort by {sort_name}"));
    focus_outline(ui, &response);
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    response
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
