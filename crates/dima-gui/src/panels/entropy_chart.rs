//! The entropy chart (D87): one line per run of shown positions — solid Okabe–Ito blue where
//! support > T (corrected entropy with the paper's estimator), dashed vermillion where support
//! ≤ T — with hatched full-height bands labelled "no usable support" over positions without
//! support (never drawn as 0), the eligible-mean line, coverage marks, a legend and per-point
//! tooltips. Long ranges are M4-downsampled per pixel column, keeping class boundaries.
//!
//! When the run computed similarity-weighted entropy, a "Metric:" switch on its own row above
//! the heading plots it instead, with its own value axis, mean line and legend; the heading and
//! the legend entries that change with the metric keep the width of the longer text, so no
//! control moves on a switch.
//!
//! Gestures: scroll zooms about the cursor, drag pans, Shift-drag zooms to a region,
//! double-click resets, click selects; the hovered position is mirrored in the table.

use std::sync::Arc;

use dima_core::human;
use egui::{Color32, Pos2, Rect, Stroke, Ui};

use super::common::Described;
use super::common::{
    Swatch, fixed_width_slot, hatch_rect, heading, legend_entry, legend_entry_at_least,
    legend_entry_width, row_with_trailing, text_width, with_help,
};
use super::help_text;
use crate::app::msg::Msg;
use crate::charts::axis::{integer_ticks, nice_ticks, y_tick_labels};
use crate::charts::gpu_line::{LineCallback, LineUniforms, SegmentVertex, segment_vertices};
use crate::charts::metric::ChartMetric;
use crate::charts::series::{
    ChartKey, Class, NsSpan, Point, Series, Stroke as DataStroke, Style, chart_cache_key, legend,
    nearest_point, ns_span_at, strokes,
};
use crate::charts::transform::{XMap, YMap, y_axis_max};
use crate::charts::viewport;
use crate::state::session::{SelectSource, Session};
use crate::theme::DesignTokens;

/// Left margin for the y axis (shared with the conserved-region strip, G18).
pub const LEFT_MARGIN: f32 = 58.0;
/// Right margin.
pub const RIGHT_MARGIN: f32 = 10.0;
const BOTTOM_MARGIN: f32 = 34.0;
const TOP_MARGIN: f32 = 6.0;
const LINE_WIDTH: f32 = 1.6;
const NS_LABEL: &str = "no usable support";

/// Chart state that is not application state.
#[derive(Default)]
pub struct ChartUi {
    /// Screen x where a Shift-drag began.
    pub drag_origin: Option<f32>,
    /// The lines last drawn, with their key.
    lines: Option<Lines>,
}

/// The lines of one [`ChartKey`], computed when the key changes, not per frame (P5): the
/// strokes and markers in data units and, when the GPU draws the solid line, its vertices.
struct Lines {
    key: ChartKey,
    strokes: Vec<DataStroke>,
    markers: Vec<Point>,
    /// The solid strokes as GPU quads, x relative to the view's left edge.
    vertices: Option<Arc<Vec<SegmentVertex>>>,
}

impl ChartUi {
    /// The lines of `series` in `view`, reduced to `columns`: from the cache when `key` (which
    /// identifies all of them, see [`chart_cache_key`]) has not changed. The GPU vertices are
    /// built only when `gpu`.
    fn lines(
        &mut self,
        key: ChartKey,
        series: &Series,
        view: (f64, f64),
        columns: usize,
        gpu: bool,
    ) -> &Lines {
        let lines = match self.lines.take() {
            Some(l) if l.key == key => l,
            _ => {
                let (strokes, markers) = strokes(series, view.0, view.1, Some(columns));
                Lines {
                    key,
                    strokes,
                    markers,
                    vertices: None,
                }
            }
        };
        let lines = self.lines.insert(lines);
        if gpu && lines.vertices.is_none() {
            lines.vertices = Some(Arc::new(solid_vertices(&lines.strokes, view.0)));
        }
        lines
    }
}

/// The GPU quads of the solid strokes, with x relative to the view's left edge `lo` (so `f32`
/// keeps sub-pixel precision at any position, G58).
fn solid_vertices(strokes: &[DataStroke], lo: f64) -> Vec<SegmentVertex> {
    #[allow(clippy::cast_possible_truncation)]
    let lines: Vec<Vec<[f32; 2]>> = strokes
        .iter()
        .filter(|st| st.style == Style::Eligible)
        .map(|st| {
            st.points
                .iter()
                .map(|&(px, py)| [(px - lo) as f32, py as f32])
                .collect()
        })
        .collect();
    segment_vertices(&lines)
}

/// What the chart reports to the strip below it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shared {
    /// The x mapping of the plot area.
    pub x: XMap,
}

/// Draws the chart; returns its x mapping (for the conserved-region strip), if drawn.
pub fn show(
    ui: &mut Ui,
    s: &mut Session,
    chart: &mut ChartUi,
    gpu: bool,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) -> Option<Shared> {
    let threshold = s.results.support_threshold;
    let series = Arc::clone(&s.series);
    let metric = s.metric;
    let switchable = ChartMetric::SimilarityWeighted.available(&s.results);
    if switchable {
        metric_switch(ui, metric, out);
    }
    header(ui, s, &series, t, threshold, switchable);
    let height = (ui.ctx().content_rect().height() * 0.28).clamp(150.0, 380.0);
    let (full_rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height + BOTTOM_MARGIN + TOP_MARGIN),
        egui::Sense::click_and_drag(),
    );
    // A Tab stop, for its description; outlined while it has the focus.
    super::common::focus_outline(ui, &response);
    let plot = Rect::from_min_max(
        egui::pos2(full_rect.left() + LEFT_MARGIN, full_rect.top() + TOP_MARGIN),
        egui::pos2(
            full_rect.right() - RIGHT_MARGIN,
            full_rect.bottom() - BOTTOM_MARGIN,
        ),
    );
    s.hovered = None;
    let painter = ui.painter_at(full_rect);
    let (Some(full), false) = (s.extent(), series.is_empty()) else {
        painter.text(
            full_rect.center(),
            egui::Align2::CENTER_CENTER,
            "No positions match the current filters",
            egui::FontId::proportional(t.font_size_body),
            t.text_muted,
        );
        let label = format!("{} chart: no positions shown", metric.name());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &label));
        return None;
    };
    let view = s.viewport.unwrap_or(full);
    let x = XMap {
        lo: view.0,
        hi: view.1,
        left: plot.left(),
        right: plot.right(),
    };
    let y = YMap {
        max: y_axis_max(series.y_max),
        top: plot.top(),
        bottom: plot.bottom(),
    };
    let plot_painter = ui.painter_at(plot);

    draw_ns_bands(&plot_painter, &series, x, plot, t);
    draw_mean(&plot_painter, metric.mean(&s.results), y, plot, t);
    draw_lines(
        ui,
        &plot_painter,
        &series,
        x,
        y,
        plot,
        chart,
        gpu,
        s.id,
        s.version,
        t,
    );
    draw_coverage(&plot_painter, &series, x, plot, t);
    draw_axes(&painter, x, y, plot, full_rect, metric, t);
    if let Some(sel) = s.selection.position {
        #[allow(clippy::cast_precision_loss)]
        let sx = x.to_screen(sel as f64);
        if plot.x_range().contains(sx) {
            plot_painter.line_segment(
                [egui::pos2(sx, plot.top()), egui::pos2(sx, plot.bottom())],
                Stroke::new(1.5, t.chart_selection_line),
            );
        }
    }

    // Accessibility: the chart is painted, so it describes itself (AccessKit).
    let description = chart_description(
        metric,
        s.shown.len(),
        series.count(Style::LowSupport),
        series.ns_spans.len(),
        threshold,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &description));

    interact(ui, &response, s, chart, &series, x, y, plot, full, t, out);
    Some(Shared { x })
}

/// What the chart says to screen readers (it is painted): the metric, the positions shown, how
/// many of them have support ≤ T and how many no-support spans there are, counted in words that
/// fit one ("1 span").
#[must_use]
pub fn chart_description(
    metric: ChartMetric,
    shown: usize,
    low_support: usize,
    ns_spans: usize,
    threshold: u64,
) -> String {
    let count = |n: usize, one: &str, many: &str| human::plural(n as u64, one, many);
    format!(
        "{} chart: {} shown, {} with support \u{2264} {threshold}, {} with {NS_LABEL}",
        metric.name(),
        count(shown, "position", "positions"),
        low_support,
        count(ns_spans, "span", "spans"),
    )
}

/// "Metric:" and its options, on a left-aligned row of their own above the heading row. The
/// row is one AccessKit radio group named by its visible label; a click asks `update`, which
/// refuses a metric the results lack.
fn metric_switch(ui: &mut Ui, current: ChartMetric, out: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let label = ui.label("Metric:");
        ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
            node.set_role(egui::accesskit::Role::RadioGroup);
            node.push_labelled_by(label.id.accesskit_id());
        });
        for metric in ChartMetric::ALL {
            let radio = ui.radio(current == metric, metric.name());
            if with_help(radio, &help_text::chart_metric(metric)).clicked() {
                out.push(Msg::ChartMetric(metric));
            }
        }
    });
}

/// The heading row: the heading, the legend and the gesture hint, wrapping as the width needs,
/// with Fit, + and − at its top right. The zoom buttons are laid out first and the rest wraps
/// in the width left of them, so a long legend can neither run under them nor push them down;
/// screen readers still meet the heading and the legend before the buttons, as in the row.
/// When the metric can be switched, the heading and the legend entries whose text depends on it
/// keep the width of the longer of their two texts, so the row wraps alike for both metrics.
fn header(
    ui: &mut Ui,
    s: &mut Session,
    series: &Series,
    t: &DesignTokens,
    threshold: u64,
    switchable: bool,
) {
    // The gap the buttons leave: the item spacing after the last one, and 8 px more.
    let gap = ui.spacing().item_spacing.x + t.space_8;
    let session: &Session = s;
    let ((), zoomed) = row_with_trailing(
        ui,
        gap,
        |ui| {
            if switchable {
                let font = egui::FontId::new(
                    t.font_size_heading,
                    egui::FontFamily::Name(crate::theme::fonts::SEMIBOLD.into()),
                );
                let width =
                    widest(ChartMetric::ALL.map(|m| text_width(ui, m.name(), font.clone())));
                fixed_width_slot(ui, width, |ui| heading(ui, t, session.metric.name()));
            } else {
                heading(ui, t, session.metric.name());
            }
            ui.add_space(t.space_8);
            legend_entries(ui, session, series, t, threshold, switchable);
            // One item, so it moves to the next row whole rather than wrapping inside.
            ui.add(
                egui::Label::new(
                    egui::RichText::new("drag: pan \u{2022} Shift-drag: zoom to region").weak(),
                )
                .extend(),
            );
        },
        |ui| zoom_buttons(ui, session),
    );
    if let Some(viewport) = zoomed {
        s.viewport = viewport;
    }
}

/// The larger of two widths.
fn widest(widths: [f32; 2]) -> f32 {
    widths[0].max(widths[1])
}

/// The chart's legend: the line styles shown, the no-support bands, the mean and coverage marks.
/// The line styles' captions depend on the metric; when it can be switched, each is given the
/// width of its longer caption.
fn legend_entries(
    ui: &mut Ui,
    s: &Session,
    series: &Series,
    t: &DesignTokens,
    threshold: u64,
    switchable: bool,
) {
    let estimator = s.results.run.parameters.estimator.value;
    let tip = help_text::chart_encoding(threshold, estimator, s.metric);
    let [solid, dashed] = legend(threshold, estimator, s.metric);
    let slots = if switchable {
        let [a, b] = ChartMetric::ALL.map(|m| legend(threshold, estimator, m));
        [
            widest([&a[0], &b[0]].map(|c| legend_entry_width(ui, t, c))),
            widest([&a[1], &b[1]].map(|c| legend_entry_width(ui, t, c))),
        ]
    } else {
        [0.0; 2]
    };
    if series.has(Style::Eligible) {
        let swatch = Swatch::Line(t.series_eligible);
        with_help(legend_entry_at_least(ui, t, &solid, swatch, slots[0]), &tip);
    }
    if series.has(Style::LowSupport) {
        let swatch = Swatch::Dashed(t.series_uncorrected);
        with_help(
            legend_entry_at_least(ui, t, &dashed, swatch, slots[1]),
            &tip,
        );
    }
    if !series.ns_spans.is_empty() {
        let entry = legend_entry(
            ui,
            t,
            &format!("NS: {NS_LABEL}"),
            Swatch::Hatched(t.ns_band_fill, t.ns_hatch),
        );
        with_help(entry, &tip);
    }
    if s.metric.mean(&s.results).is_some() {
        let entry = legend_entry(ui, t, "eligible mean", Swatch::Dashed(t.mean_line));
        let help = help_text::eligible_mean(s.metric, threshold, s.results.eligible_position_count);
        with_help(entry, &help);
    }
    if let (Some(c), false) = (
        s.results.coverage_summary.min_coverage,
        series.low_coverage.is_empty(),
    ) {
        let entry = legend_entry(
            ui,
            t,
            &format!("sample coverage < {c}"),
            Swatch::Tick(t.coverage_mark),
        );
        with_help(entry, &help_text::min_coverage());
    }
}

/// Fit, + and −, right to left; returns the zoom a clicked button asks for.
fn zoom_buttons(ui: &mut Ui, s: &Session) -> Option<Option<(f64, f64)>> {
    let zoomed = s.viewport.is_some();
    let mut asked = None;
    if ui
        .add_enabled(zoomed, egui::Button::new("Fit").small())
        .described("Show every shown position (or double-click the chart)")
        .clicked()
    {
        asked = Some(None);
    }
    if let Some(full) = s.extent() {
        if ui
            .small_button("+")
            .described("Zoom in (or scroll over the chart)")
            .clicked()
        {
            asked = Some(viewport::zoom(s.viewport, full, 0.8, 0.5));
        }
        if ui
            .add_enabled(zoomed, egui::Button::new("\u{2212}").small())
            .described("Zoom out")
            .clicked()
        {
            asked = Some(viewport::zoom(s.viewport, full, 1.25, 0.5));
        }
    }
    asked
}

fn band_rect(span: NsSpan, x: XMap, plot: Rect) -> Rect {
    #[allow(clippy::cast_precision_loss)]
    let (a, b) = (
        x.to_screen(span.first as f64 - 0.5),
        x.to_screen(span.last as f64 + 0.5),
    );
    Rect::from_min_max(
        egui::pos2(a.max(plot.left()), plot.top()),
        egui::pos2(b.min(plot.right()).max(a + 1.0), plot.bottom()),
    )
}

fn draw_ns_bands(p: &egui::Painter, series: &Series, x: XMap, plot: Rect, t: &DesignTokens) {
    let label_font = egui::FontId::proportional(t.font_size_caption);
    let label_width = p
        .layout_no_wrap(NS_LABEL.to_owned(), label_font.clone(), t.text_secondary)
        .size()
        .x;
    // Spans less than a pixel apart are drawn as one band (they cannot be told apart).
    let mut merged: Option<Rect> = None;
    let flush = |r: Rect| {
        p.rect_filled(r, 0.0, t.ns_band_fill);
        hatch_rect(p, r, t.ns_hatch, 6.0);
        if r.width() >= label_width + 8.0 {
            p.text(
                egui::pos2(r.center().x, r.top() + 4.0),
                egui::Align2::CENTER_TOP,
                NS_LABEL,
                label_font.clone(),
                t.text_primary,
            );
        }
    };
    for span in &series.ns_spans {
        if !span.in_view(x.lo, x.hi) {
            continue;
        }
        let r = band_rect(*span, x, plot);
        merged = match merged {
            Some(m) if r.left() - m.right() < 1.0 => Some(m.union(r)),
            Some(m) => {
                flush(m);
                Some(r)
            }
            None => Some(r),
        };
    }
    if let Some(m) = merged {
        flush(m);
    }
}

fn draw_mean(p: &egui::Painter, mean: Option<f64>, y: YMap, plot: Rect, t: &DesignTokens) {
    let Some(mean) = mean else {
        return;
    };
    let sy = y.to_screen(mean);
    if !plot.y_range().contains(sy) {
        return;
    }
    p.extend(egui::Shape::dashed_line(
        &[egui::pos2(plot.left(), sy), egui::pos2(plot.right(), sy)],
        Stroke::new(1.0, t.mean_line),
        3.0,
        3.0,
    ));
    p.text(
        egui::pos2(plot.right() - 4.0, sy - 2.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("eligible mean {}", human::fixed(mean, 4)),
        egui::FontId::proportional(t.font_size_caption),
        t.mean_line,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_lines(
    ui: &Ui,
    p: &egui::Painter,
    series: &Series,
    x: XMap,
    y: YMap,
    plot: Rect,
    chart: &mut ChartUi,
    gpu: bool,
    session: u64,
    version: u64,
    t: &DesignTokens,
) {
    let ppp = ui.ctx().pixels_per_point();
    // M4 per physical pixel column (G4: fills and lines are drawn once per stroke).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let columns = (plot.width() * ppp).ceil().max(1.0) as usize;
    let key = chart_cache_key(session, version, (x.lo, x.hi), columns);
    let lines = chart.lines(key, series, (x.lo, x.hi), columns, gpu);
    let screen = |pts: &[(f64, f64)]| -> Vec<Pos2> {
        pts.iter()
            .map(|&(px, py)| egui::pos2(x.to_screen(px), y.to_screen(py)))
            .collect()
    };
    // Fills under the line, one mesh per stroke.
    for st in &lines.strokes {
        let color = match st.style {
            Style::Eligible => t.series_fill,
            Style::LowSupport => {
                let [r, g, b, _] = t.series_uncorrected.to_srgba_unmultiplied();
                Color32::from_rgba_unmultiplied(r, g, b, 22)
            }
        };
        let pts = screen(&st.points);
        let mut mesh = egui::Mesh::default();
        for (i, q) in pts.iter().enumerate() {
            mesh.colored_vertex(*q, color);
            mesh.colored_vertex(egui::pos2(q.x, plot.bottom()), color);
            if i > 0 {
                #[allow(clippy::cast_possible_truncation)]
                let k = (i * 2) as u32;
                mesh.add_triangle(k - 2, k - 1, k);
                mesh.add_triangle(k - 1, k + 1, k);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    }
    for st in &lines.strokes {
        match st.style {
            // Drawn below from the cached vertices.
            Style::Eligible if gpu => {}
            Style::Eligible => {
                p.add(egui::Shape::line(
                    screen(&st.points),
                    Stroke::new(LINE_WIDTH, t.series_eligible),
                ));
            }
            Style::LowSupport => {
                p.extend(egui::Shape::dashed_line(
                    &screen(&st.points),
                    Stroke::new(LINE_WIDTH, t.series_uncorrected),
                    5.0,
                    3.5,
                ));
            }
        }
    }
    if gpu && let Some(vertices) = lines.vertices.as_ref().filter(|v| !v.is_empty()) {
        #[allow(clippy::cast_possible_truncation)]
        let uniforms = LineUniforms {
            x_min: 0.0,
            x_range: x.span() as f32,
            y_max: y.max as f32,
            half_width: LINE_WIDTH * ppp / 2.0,
            viewport: [plot.width() * ppp, plot.height() * ppp],
            padding: [0.0; 2],
            color: t.series_eligible.to_normalized_gamma_f32(),
        };
        p.add(egui_wgpu::Callback::new_paint_callback(
            plot,
            LineCallback {
                uniforms,
                key,
                vertices: Arc::clone(vertices),
            },
        ));
    }
    for m in &lines.markers {
        let color = match m.style {
            Style::Eligible => t.series_eligible,
            Style::LowSupport => t.series_uncorrected,
        };
        p.circle_filled(egui::pos2(x.to_screen(m.x), y.to_screen(m.y)), 3.0, color);
    }
}

fn draw_coverage(p: &egui::Painter, series: &Series, x: XMap, plot: Rect, t: &DesignTokens) {
    for &pos in &series.low_coverage {
        #[allow(clippy::cast_precision_loss)]
        let sx = x.to_screen(pos as f64);
        if plot.x_range().contains(sx) {
            p.line_segment(
                [egui::pos2(sx, plot.top()), egui::pos2(sx, plot.top() + 7.0)],
                Stroke::new(1.5, t.coverage_mark),
            );
        }
    }
}

fn draw_axes(
    p: &egui::Painter,
    x: XMap,
    y: YMap,
    plot: Rect,
    full: Rect,
    metric: ChartMetric,
    t: &DesignTokens,
) {
    let stroke = Stroke::new(1.0, t.chart_axis);
    let font = egui::FontId::proportional(t.font_size_caption);
    p.line_segment([plot.left_top(), plot.left_bottom()], stroke);
    p.line_segment([plot.left_bottom(), plot.right_bottom()], stroke);
    let (ticks, step) = nice_ticks(0.0, y.max, 6);
    for (&v, label) in ticks.iter().zip(y_tick_labels(&ticks, step)) {
        let sy = y.to_screen(v);
        if !(plot.top() - 0.5..=plot.bottom() + 0.5).contains(&sy) {
            continue;
        }
        p.line_segment(
            [
                egui::pos2(plot.left() - 4.0, sy),
                egui::pos2(plot.left(), sy),
            ],
            stroke,
        );
        p.text(
            egui::pos2(plot.left() - 6.0, sy),
            egui::Align2::RIGHT_CENTER,
            label,
            font.clone(),
            t.chart_axis,
        );
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let max_ticks = ((plot.width() / 80.0) as usize).clamp(2, 12);
    for v in integer_ticks(x.lo, x.hi, max_ticks) {
        #[allow(clippy::cast_precision_loss)]
        let sx = x.to_screen(v as f64);
        p.line_segment(
            [
                egui::pos2(sx, plot.bottom()),
                egui::pos2(sx, plot.bottom() + 4.0),
            ],
            stroke,
        );
        p.text(
            egui::pos2(sx, plot.bottom() + 6.0),
            egui::Align2::CENTER_TOP,
            v.to_string(),
            font.clone(),
            t.chart_axis,
        );
    }
    p.text(
        egui::pos2(plot.center().x, full.bottom() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "Position",
        font.clone(),
        t.chart_axis,
    );
    let galley = p.layout_no_wrap(metric.axis_title().to_owned(), font, t.chart_axis);
    // Rotated, the title runs upwards from its anchor, `galley.size().x` long.
    let center = y_title_center(plot, full, galley.size().x);
    let pos = egui::pos2(full.left() + 2.0, center + galley.size().x / 2.0);
    p.add(
        egui::epaint::TextShape::new(pos, galley, t.chart_axis)
            .with_angle(-std::f32::consts::FRAC_PI_2),
    );
}

/// The centre of the rotated value-axis title, `length` long: the plot's middle, moved just
/// enough to keep the whole title inside the chart (a long title on a short plot would reach
/// into the heading row); when the chart is shorter than the title, its bottom stays inside.
/// `max` then `min`, never `clamp`, which panics when the bounds cross.
fn y_title_center(plot: Rect, full: Rect, length: f32) -> f32 {
    plot.center()
        .y
        .max(full.top() + length / 2.0)
        .min(full.bottom() - length / 2.0)
}

/// The tooltip text of a hovered position (pure, for tests).
#[must_use]
pub fn point_tooltip(s: &Session, position: u64) -> Option<String> {
    let p = s.position(position)?;
    let class = Class::of(p);
    let t = s.results.support_threshold;
    let mut text = format!("Position {}\n", p.position);
    match p.entropy {
        Some(e) => {
            text.push_str(&format!("entropy {} bits", human::fixed(e, 4)));
            if let Some(o) = p.observed_entropy {
                text.push_str(&format!(", observed {}", human::fixed(o, 4)));
            }
            text.push('\n');
        }
        None => text.push_str("no entropy\n"),
    }
    text.push_str(&format!(
        "method: {}\nsupport {}\n{}",
        super::position_summary::method_text(p),
        p.support,
        class.describe(
            t,
            s.results.run.parameters.estimator.value,
            p.entropy_method
        )
    ));
    if p.below_min_coverage == Some(true) {
        text.push_str(&format!(
            "\nsample coverage {} below the minimum",
            crate::util::position_coverage(p, s.results.coverage_summary.min_coverage, 3)
        ));
    }
    if ChartMetric::SimilarityWeighted.available(&s.results) {
        let [similarity, observed, normalised] = super::position_summary::similarity_values(p);
        text.push_str(&format!(
            "\nsimilarity-weighted entropy {similarity}\nnormalised observed entropy \
             {observed}\nnormalised similarity-weighted entropy {normalised}"
        ));
    }
    Some(text)
}

/// The tooltip text of a hovered no-support band.
#[must_use]
pub fn band_tooltip(span: NsSpan) -> String {
    if span.first == span.last {
        format!(
            "Position {}: {NS_LABEL} (support 0, no entropy)",
            span.first
        )
    } else {
        format!(
            "Positions {}\u{2013}{}: {NS_LABEL} (support 0, no entropy)",
            span.first, span.last
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn interact(
    ui: &Ui,
    response: &egui::Response,
    s: &mut Session,
    chart: &mut ChartUi,
    series: &Series,
    x: XMap,
    y: YMap,
    plot: Rect,
    full: (f64, f64),
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    let pointer = ui.ctx().pointer_latest_pos();
    let in_plot = |p: Pos2| plot.x_range().contains(p.x);
    // Hover: a band takes precedence (it has no point), else the nearest point.
    if response.hovered()
        && let Some(pp) = pointer.filter(|p| in_plot(*p))
    {
        let dx = x.to_data(pp.x);
        if let Some(span) = ns_span_at(series, dx) {
            let r = band_rect(span, x, plot);
            ui.painter_at(plot).rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, t.text_secondary),
                egui::StrokeKind::Inside,
            );
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let at = dx.round().clamp(span.first as f64, span.last as f64) as u64;
            s.hovered = Some(at);
            response
                .clone()
                .on_hover_text_at_pointer(band_tooltip(span));
        } else if let Some(pt) = nearest_point(series, dx) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let pos = pt.x as u64;
            s.hovered = Some(pos);
            ui.painter_at(plot).circle_filled(
                egui::pos2(x.to_screen(pt.x), y.to_screen(pt.y)),
                3.5,
                t.chart_selection_line,
            );
            if let Some(text) = point_tooltip(s, pos) {
                response.clone().on_hover_text_at_pointer(text);
            }
        }
    }
    // Click selects the hovered position.
    if response.clicked()
        && let Some(pos) = s.hovered
    {
        out.push(Msg::Select(pos, SelectSource::Chart));
    }
    // Scroll zooms about the cursor.
    let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
    if response.hovered()
        && scroll.abs() > 0.1
        && let Some(pp) = pointer
    {
        let anchor = f64::from(((pp.x - plot.left()) / plot.width().max(1.0)).clamp(0.0, 1.0));
        let factor = if scroll > 0.0 { 0.85 } else { 1.18 };
        s.viewport = viewport::zoom(s.viewport, full, factor, anchor);
        // The wheel zoomed the chart; the scrolling column around it must not move as well.
        ui.ctx().input_mut(|i| i.smooth_scroll_delta.y = 0.0);
    }
    // Shift-drag zooms to a region; a plain drag pans.
    let shift = ui.ctx().input(|i| i.modifiers.shift);
    if shift && response.drag_started() {
        chart.drag_origin = response.interact_pointer_pos().map(|p| p.x);
    }
    if let Some(origin) = chart.drag_origin {
        let current = pointer.map_or(origin, |p| p.x);
        if response.dragged() {
            let band =
                Rect::from_x_y_ranges(origin.min(current)..=origin.max(current), plot.y_range());
            ui.painter_at(plot)
                .rect_filled(band, 0.0, t.selection_highlight);
        } else {
            chart.drag_origin = None;
            s.viewport = viewport::region(s.viewport, full, x.to_data(origin), x.to_data(current));
        }
    } else if response.dragged() {
        let delta = -f64::from(response.drag_delta().x) * x.data_per_pixel();
        s.viewport = viewport::pan(s.viewport, full, delta);
    }
    if response.double_clicked() {
        s.viewport = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::series::tests::{position, results_of};
    use crate::state::session::Origin;

    #[test]
    fn tooltips_name_the_class_and_never_call_ns_zero() {
        let r = results_of(vec![
            position(1, Some(0.5), 200, 100),
            position(2, None, 0, 100),
            position(3, Some(0.25), 40, 100),
        ]);
        let s = Session::new(1, Arc::new(r), Origin::Opened("x.json".into()));
        let a = point_tooltip(&s, 1).unwrap();
        assert!(
            a.contains("support 200") && a.contains(": corrected entropy"),
            "{a}"
        );
        let c = point_tooltip(&s, 3).unwrap();
        assert!(c.contains("LS") && c.contains("uncorrected"), "{c}");
        let b = point_tooltip(&s, 2).unwrap();
        assert!(b.contains("no entropy") && !b.contains("0.0000"), "{b}");
        assert!(b.contains("\nmethod: no entropy (not_estimable)\n"), "{b}");
        assert!(a.contains("entropy 0.5000 bits, observed 0.5000\n"), "{a}");
        assert!(
            a.contains("\nmethod: paper's correction (dima)\n"),
            "the method in words and as the file names it: {a}"
        );
        assert!(c.contains("method: observed entropy (plug_in)"), "{c}");
        assert_eq!(
            band_tooltip(NsSpan { first: 4, last: 9 }),
            "Positions 4\u{2013}9: no usable support (support 0, no entropy)"
        );
    }

    /// A value that is not zero never reads 0.0000, and a coverage below 1 never reads 1.000.
    #[test]
    fn tooltips_never_round_a_value_to_zero_or_to_its_bound() {
        let mut tiny = position(1, Some(3.288e-5), 200, 100);
        tiny.observed_entropy = Some(4.9e-5);
        tiny.below_min_coverage = Some(true);
        tiny.sample_coverage = Some(0.99996);
        let mut zero = position(2, Some(0.0), 200, 100);
        zero.observed_entropy = Some(-0.0);
        let r = results_of(vec![tiny, zero]);
        let s = Session::new(1, Arc::new(r), Origin::Opened("x.json".into()));
        let a = point_tooltip(&s, 1).unwrap();
        assert!(
            a.contains("entropy < 0.0001 bits, observed < 0.0001"),
            "{a}"
        );
        assert!(
            a.contains("sample coverage > 0.999 below the minimum"),
            "{a}"
        );
        let b = point_tooltip(&s, 2).unwrap();
        assert!(
            b.contains("entropy 0.0000 bits, observed 0.0000"),
            "an exact zero is zero: {b}"
        );
        // A coverage below the run's minimum never reads as the minimum.
        let mut near = position(1, Some(0.5), 200, 100);
        near.sample_coverage = Some(0.949_96);
        near.below_min_coverage = Some(true);
        let mut r = results_of(vec![near]);
        r.coverage_summary.min_coverage = Some(0.95);
        let s = Session::new(2, Arc::new(r), Origin::Opened("x.json".into()));
        let text = point_tooltip(&s, 1).unwrap();
        assert!(
            text.contains("sample coverage > 0.949 below the minimum"),
            "{text}"
        );
    }

    /// Both value-axis titles fit whole inside the chart at its smallest plot height (150 px)
    /// with the bundled fonts, and the entropy title stays centred on the plot, as before.
    #[test]
    fn both_axis_titles_fit_the_smallest_chart() {
        let ctx = egui::Context::default();
        crate::theme::fonts::install(&ctx);
        let t = DesignTokens::light();
        let mut lengths = Vec::new();
        // Fonts set in one pass are used from the next.
        for _ in 0..2 {
            lengths.clear();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let font = egui::FontId::proportional(t.font_size_caption);
                for m in ChartMetric::ALL {
                    lengths.push(text_width(ui, m.axis_title(), font.clone()));
                }
            });
            out.textures_delta.clear();
        }
        let full = Rect::from_min_size(
            egui::pos2(16.0, 200.0),
            egui::vec2(588.0, 150.0 + TOP_MARGIN + BOTTOM_MARGIN),
        );
        let plot = Rect::from_min_max(
            egui::pos2(full.left() + LEFT_MARGIN, full.top() + TOP_MARGIN),
            egui::pos2(full.right() - RIGHT_MARGIN, full.bottom() - BOTTOM_MARGIN),
        );
        assert_eq!(plot.height(), 150.0);
        for (m, &length) in ChartMetric::ALL.iter().zip(&lengths) {
            assert!(length > 60.0, "{m:?}: measured with the bundled font");
            let c = y_title_center(plot, full, length);
            assert!(
                c - length / 2.0 >= full.top() && c + length / 2.0 <= full.bottom(),
                "{m:?}: {length} px centred at {c} in {full:?}"
            );
        }
        assert!(lengths[1] > plot.height(), "longer than the plot: it moves");
        assert_eq!(y_title_center(plot, full, lengths[0]), plot.center().y);
        // A title longer than the chart keeps its bottom inside, and nothing panics.
        assert_eq!(y_title_center(plot, full, 1000.0), full.bottom() - 500.0);
    }

    /// With similarity-weighted entropy the tooltip adds its three values, marked like the
    /// position's facts; without it, nothing.
    #[test]
    fn tooltips_add_the_similarity_weighted_values_when_computed() {
        let s = Session::new(
            1,
            crate::tests::fixtures::mers_swe(),
            Origin::Opened("x".into()),
        );
        let text = point_tooltip(&s, 25).unwrap();
        assert!(
            text.ends_with(
                "\nsimilarity-weighted entropy 0.4275 bits\nnormalised observed entropy \
                 0.4282\nnormalised similarity-weighted entropy 0.1287"
            ),
            "{text}"
        );
        let off = Session::new(
            2,
            crate::tests::fixtures::mers(),
            Origin::Opened("x".into()),
        );
        assert!(!point_tooltip(&off, 25).unwrap().contains("similarity"));
        let hiv = Session::new(
            3,
            crate::tests::fixtures::hiv_pro_swe(),
            Origin::Opened("x".into()),
        );
        let low = hiv
            .results
            .results
            .iter()
            .find(|p| p.low_support.is_some() && p.diversity_motifs.len() > 1)
            .unwrap();
        let text = point_tooltip(&hiv, low.position).unwrap();
        assert_eq!(text.matches(" (low support)").count(), 2, "{text}");
    }

    /// A new load restarts the version, so two sessions showing the same positions at the same
    /// zoom differ only in their id: the line of one must never be drawn for the other.
    #[test]
    fn lines_of_two_sessions_never_share_a_key() {
        let r = Arc::new(results_of(vec![
            position(1, Some(0.5), 200, 100),
            position(2, Some(0.25), 40, 100),
        ]));
        let a = Session::new(1, Arc::clone(&r), Origin::Opened("a.json".into()));
        let b = Session::new(2, r, Origin::Opened("b.json".into()));
        assert_eq!(a.version, b.version);
        let (view, columns) = (a.extent().unwrap(), 900);
        let key = |s: &Session| chart_cache_key(s.id, s.version, view, columns);
        assert_ne!(key(&a), key(&b));
        assert_eq!(key(&a), chart_cache_key(1, a.version, view, columns));
        // Every input is part of the key.
        assert_ne!(key(&a), chart_cache_key(1, a.version + 1, view, columns));
        assert_ne!(key(&a), chart_cache_key(1, a.version, (0.5, 2.0), columns));
        assert_ne!(key(&a), chart_cache_key(1, a.version, view, columns + 1));
    }

    /// Strokes, markers and GPU vertices are one cache entry: a frame with the same key
    /// recomputes nothing (here: the cache answers even for other data), a new key recomputes.
    #[test]
    fn lines_are_computed_once_per_key() {
        let series = |e: f64| {
            let r = results_of((1..=50).map(|p| position(p, Some(e), 200, 100)).collect());
            Series::build(&r, &(0..50).collect::<Vec<_>>(), ChartMetric::Entropy)
        };
        let (a, b) = (series(0.5), series(0.25));
        let view = (0.5, 50.5);
        let key = chart_cache_key(1, 1, view, 20);
        let mut chart = ChartUi::default();
        let first = chart.lines(key, &a, view, 20, true);
        let (strokes, vertices) = (
            first.strokes.clone(),
            Arc::clone(first.vertices.as_ref().unwrap()),
        );
        assert!(!strokes.is_empty() && !vertices.is_empty());
        let hit = chart.lines(key, &b, view, 20, true);
        assert_eq!(hit.strokes, strokes, "served from the cache");
        assert!(Arc::ptr_eq(hit.vertices.as_ref().unwrap(), &vertices));
        let miss = chart.lines(chart_cache_key(2, 1, view, 20), &b, view, 20, true);
        assert_ne!(miss.strokes, strokes, "a new key recomputes");
        assert!(!Arc::ptr_eq(miss.vertices.as_ref().unwrap(), &vertices));
        // Without the GPU no vertices are built.
        let cpu = ChartUi::default()
            .lines(key, &a, view, 20, false)
            .vertices
            .is_none();
        assert!(cpu);
    }
}
