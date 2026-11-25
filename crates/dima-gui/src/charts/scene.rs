//! The entropy chart as a file: SVG (vector, every point) and PNG (raster, M4-downsampled), of
//! the metric the chart shows (entropy, or similarity-weighted entropy; the texts come with the
//! scene).
//!
//! Both formats are drawn from one [`Layout`], so their geometry, labels and legend agree with
//! each other (G30), and their text never overlaps (G48): the title and subtitle sit above the
//! plot, the y-axis title is rotated in the left margin, and the legend has rows of its own
//! below, as many as it needs (the plot gives up the space). Text is sized per em, as SVG and
//! the screen size it, and a title, subtitle or legend label too long for the figure is
//! shortened with "…". The encoding is the on-screen one (D87): solid blue at support > T
//! (corrected entropy with the paper's estimator), dashed vermillion at support ≤ T, hatched
//! bands for positions without usable support, a dashed eligible-mean line.

use std::fmt::Write as _;

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};

use super::axis::{integer_ticks, nice_ticks, y_tick_labels};
use super::series::{Series, Style, strokes};
use super::transform::{XMap, YMap};

/// An 8-bit RGBA colour.
pub type Rgba = [u8; 4];

/// Colours of an exported figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStyle {
    pub background: Rgba,
    pub text: Rgba,
    pub axis: Rgba,
    pub eligible: Rgba,
    pub uncorrected: Rgba,
    pub ns_fill: Rgba,
    pub ns_hatch: Rgba,
    pub mean: Rgba,
    pub coverage: Rgba,
}

impl SceneStyle {
    /// Print colours: white background and the light theme's data colours, whatever the
    /// on-screen theme (figures go into papers and slides).
    #[must_use]
    pub fn publication() -> Self {
        let t = crate::theme::DesignTokens::light();
        let c = |c: egui::Color32| c.to_srgba_unmultiplied();
        Self {
            background: [255, 255, 255, 255],
            text: c(t.text_primary),
            axis: c(t.chart_axis),
            eligible: c(t.series_eligible),
            uncorrected: c(t.series_uncorrected),
            ns_fill: c(t.ns_band_fill),
            ns_hatch: c(t.ns_hatch),
            mean: c(t.mean_line),
            coverage: c(t.coverage_mark),
        }
    }
}

/// Everything needed to draw the figure.
#[derive(Clone, Debug)]
pub struct ChartScene {
    /// Title (e.g. the query name).
    pub title: String,
    /// One line of run facts under the title.
    pub subtitle: String,
    /// Title of the value axis (e.g. "Shannon entropy (bits)").
    pub y_title: String,
    /// Width in pixels (SVG user units).
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The data, at full resolution.
    pub series: Series,
    /// Horizontal data range shown (positions, with half-position margins).
    pub view: (f64, f64),
    /// Value at the top of the y axis.
    pub y_max: f64,
    /// The plotted metric's mean over eligible positions, if any.
    pub mean: Option<f64>,
    /// Legend text of the solid and the dashed line ([`super::series::legend`]).
    pub legend: [String; 2],
    /// The run's `--min-coverage`, when coverage flags are shown.
    pub min_coverage: Option<f64>,
    /// Colours.
    pub style: SceneStyle,
}

/// Margins and text sizes, shared by both renderers.
const MARGIN_LEFT: f32 = 86.0;
const MARGIN_RIGHT: f32 = 28.0;
const MARGIN_TOP: f32 = 70.0;
/// Below the plot: the tick labels, the x-axis title and one legend row; each further legend
/// row adds [`LEGEND_ROW`].
const MARGIN_BOTTOM: f32 = 104.0;
/// Distance between legend rows.
const LEGEND_ROW: f32 = 20.0;
/// From a legend swatch's left edge to its label.
const SWATCH_ADVANCE: f32 = 28.0;
/// From the end of a legend label to the next swatch.
const LEGEND_GAP: f32 = 22.0;
/// Marks a shortened text.
const ELLIPSIS: &str = "\u{2026}";
const TITLE_SIZE: f32 = 18.0;
const SUBTITLE_SIZE: f32 = 12.0;
const LABEL_SIZE: f32 = 12.0;
const AXIS_TITLE_SIZE: f32 = 13.0;
const LINE_WIDTH: f32 = 1.8;
const DASH: (f32, f32) = (6.0, 4.0);
const MEAN_DASH: (f32, f32) = (3.0, 3.0);
const HATCH_SPACING: f32 = 6.0;
const NS_LABEL: &str = "no usable support";

/// Horizontal alignment of a text anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    const fn svg(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Middle => "middle",
            Self::End => "end",
        }
    }
}

/// The weight of a text: regular, or SemiBold for the title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Weight {
    Regular,
    SemiBold,
}

/// A text item: position of its baseline anchor.
#[derive(Clone, Debug, PartialEq)]
struct Text {
    x: f32,
    y: f32,
    size: f32,
    text: String,
    anchor: Anchor,
    color: Rgba,
    /// Rotated 90° counter-clockwise about (x, y).
    vertical: bool,
    weight: Weight,
}

impl Text {
    /// A horizontal text in the regular face.
    fn plain(x: f32, y: f32, size: f32, text: String, anchor: Anchor, color: Rgba) -> Self {
        Self {
            x,
            y,
            size,
            text,
            anchor,
            color,
            vertical: false,
            weight: Weight::Regular,
        }
    }
}

/// One legend entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Swatch {
    Solid(Rgba),
    Dashed(Rgba, (u8, u8)),
    Hatched,
    Tick(Rgba),
}

/// A legend entry: its swatch's left edge and centre line, and its label.
#[derive(Clone, Debug, PartialEq)]
struct LegendEntry {
    x: f32,
    y: f32,
    swatch: Swatch,
    label: Text,
}

/// The computed geometry of the figure.
#[derive(Clone, Debug)]
struct Layout {
    x: XMap,
    y: YMap,
    texts: Vec<Text>,
    y_ticks: Vec<(f32, String)>,
    x_ticks: Vec<(f32, String)>,
    legend: Vec<LegendEntry>,
}

impl Layout {
    /// The label of the value tick at screen y `ty`.
    fn y_tick_label(&self, ty: f32, label: &str, color: Rgba) -> Text {
        Text::plain(
            self.x.left - 9.0,
            ty + 4.0,
            LABEL_SIZE,
            label.to_owned(),
            Anchor::End,
            color,
        )
    }

    /// The label of the position tick at screen x `tx`.
    fn x_tick_label(&self, tx: f32, label: &str, color: Rgba) -> Text {
        Text::plain(
            tx,
            self.y.bottom + 20.0,
            LABEL_SIZE,
            label.to_owned(),
            Anchor::Middle,
            color,
        )
    }
}

/// Why a figure cannot be produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    /// Nothing is shown (all positions filtered out).
    NoData,
    /// The bundled font could not be read (a build defect).
    Font(String),
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoData => f.write_str("no positions are shown (check the filters)"),
            Self::Font(e) => write!(f, "the bundled font cannot be read: {e}"),
        }
    }
}

/// The bundled faces: regular, and SemiBold for the title.
struct Faces {
    regular: FontRef<'static>,
    semibold: FontRef<'static>,
}

impl Faces {
    fn read() -> Result<Self, SceneError> {
        let face =
            |bytes| FontRef::try_from_slice(bytes).map_err(|e| SceneError::Font(e.to_string()));
        Ok(Self {
            regular: face(crate::theme::fonts::PLEX_SANS)?,
            semibold: face(crate::theme::fonts::PLEX_SANS_SEMIBOLD)?,
        })
    }

    const fn of(&self, weight: Weight) -> &FontRef<'static> {
        match weight {
            Weight::Regular => &self.regular,
            Weight::SemiBold => &self.semibold,
        }
    }
}

/// The ab_glyph scale that draws `font` at `size` pixels per em, as SVG's `font-size` and the
/// screen do. ab_glyph's [`PxScale`] is the height from descent to ascent instead (1.3 em for
/// IBM Plex), so `PxScale::from(size)` would draw the text 1.3 times too small.
fn px_scale(font: &FontRef<'_>, size: f32) -> PxScale {
    match font.units_per_em() {
        Some(em) if em > 0.0 => PxScale::from(size * font.height_unscaled() / em),
        _ => PxScale::from(size),
    }
}

/// Width of `text` at `size` (pixels per em) in `font`.
fn text_width(font: &FontRef<'_>, size: f32, text: &str) -> f32 {
    let scaled = font.as_scaled(px_scale(font, size));
    text.chars()
        .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
        .sum()
}

/// `text`, or as much of it as fits in `max` pixels followed by "…" (nothing when not even
/// that fits). Cut between characters, never inside one.
fn fit(font: &FontRef<'_>, size: f32, text: &str, max: f32) -> String {
    if text_width(font, size, text) <= max {
        return text.to_owned();
    }
    let room = max - text_width(font, size, ELLIPSIS);
    if room < 0.0 {
        return String::new();
    }
    let scaled = font.as_scaled(px_scale(font, size));
    let mut width = 0.0;
    let mut end = 0;
    for (i, c) in text.char_indices() {
        width += scaled.h_advance(scaled.scaled_glyph(c).id);
        if width > room {
            break;
        }
        end = i + c.len_utf8();
    }
    format!("{}{ELLIPSIS}", text[..end].trim_end())
}

/// Places legend entries left to right in rows at most `width` wide: an entry that does not fit
/// after another starts a new row, and a label too long for a row of its own is shortened
/// (see [`fit`]). Returns each row's entries with their offsets from the row's left edge.
fn legend_rows(
    font: &FontRef<'_>,
    entries: Vec<(Swatch, String)>,
    width: f32,
) -> Vec<Vec<(f32, Swatch, String)>> {
    let mut rows: Vec<Vec<(f32, Swatch, String)>> = Vec::new();
    // Where the next entry of the last row would start.
    let mut next = 0.0;
    for (swatch, label) in entries {
        let label = fit(font, LABEL_SIZE, &label, width - SWATCH_ADVANCE);
        let advance = SWATCH_ADVANCE + text_width(font, LABEL_SIZE, &label);
        match rows.last_mut() {
            Some(row) if next + advance <= width => row.push((next, swatch, label)),
            _ => {
                next = 0.0;
                rows.push(vec![(0.0, swatch, label)]);
            }
        }
        next += advance + LEGEND_GAP;
    }
    rows
}

impl ChartScene {
    /// The legend entries that apply, in order.
    fn legend_entries(&self) -> Vec<(Swatch, String)> {
        let s = &self.style;
        let mut entries: Vec<(Swatch, String)> = Vec::new();
        let [solid, dashed] = &self.legend;
        if self.series.has(Style::Eligible) {
            entries.push((Swatch::Solid(s.eligible), solid.clone()));
        }
        if self.series.has(Style::LowSupport) {
            entries.push((Swatch::Dashed(s.uncorrected, (6, 4)), dashed.clone()));
        }
        if !self.series.ns_spans.is_empty() {
            entries.push((Swatch::Hatched, format!("NS: {NS_LABEL}")));
        }
        if self.mean.is_some() {
            entries.push((Swatch::Dashed(s.mean, (3, 3)), "eligible mean".to_owned()));
        }
        if let (Some(c), false) = (self.min_coverage, self.series.low_coverage.is_empty()) {
            entries.push((Swatch::Tick(s.coverage), format!("sample coverage < {c}")));
        }
        entries
    }

    fn layout(&self, faces: &Faces) -> Result<Layout, SceneError> {
        if self.series.is_empty() {
            return Err(SceneError::NoData);
        }
        let font = &faces.regular;
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        let left = MARGIN_LEFT.min(w / 2.0);
        let right = (w - MARGIN_RIGHT).max(left + 1.0);
        // The legend first: each row beyond the first moves the plot's bottom up.
        let rows = legend_rows(font, self.legend_entries(), right - left);
        #[allow(clippy::cast_precision_loss)]
        let extra_rows = rows.len().saturating_sub(1) as f32;
        let top = MARGIN_TOP.min(h / 2.0);
        let bottom = (h - MARGIN_BOTTOM - LEGEND_ROW * extra_rows).max(top + 1.0);
        let x = XMap {
            lo: self.view.0,
            hi: self.view.1,
            left,
            right,
        };
        let y = YMap {
            max: self.y_max,
            top,
            bottom,
        };
        let s = &self.style;
        let mut texts = vec![
            Text {
                weight: Weight::SemiBold,
                ..Text::plain(
                    left,
                    28.0,
                    TITLE_SIZE,
                    fit(&faces.semibold, TITLE_SIZE, &self.title, right - left),
                    Anchor::Start,
                    s.text,
                )
            },
            Text::plain(
                left,
                48.0,
                SUBTITLE_SIZE,
                fit(font, SUBTITLE_SIZE, &self.subtitle, right - left),
                Anchor::Start,
                s.axis,
            ),
            Text::plain(
                (left + right) / 2.0,
                bottom + 44.0,
                AXIS_TITLE_SIZE,
                "Position (alignment column of the k-mer start)".to_owned(),
                Anchor::Middle,
                s.text,
            ),
            Text {
                vertical: true,
                ..Text::plain(
                    24.0,
                    (top + bottom) / 2.0,
                    AXIS_TITLE_SIZE,
                    self.y_title.clone(),
                    Anchor::Middle,
                    s.text,
                )
            },
        ];
        if let Some(mean) = self.mean {
            let my = y.to_screen(mean);
            if my >= top && my <= bottom {
                texts.push(Text::plain(
                    right - 4.0,
                    my - 5.0,
                    LABEL_SIZE,
                    format!("eligible mean {}", dima_core::human::fixed(mean, 4)),
                    Anchor::End,
                    s.mean,
                ));
            }
        }
        let label_width = text_width(font, LABEL_SIZE, NS_LABEL);
        // Bands outside the view are skipped, as on screen (the SVG clips them).
        for span in self
            .series
            .ns_spans
            .iter()
            .filter(|s| s.in_view(self.view.0, self.view.1))
        {
            #[allow(clippy::cast_precision_loss)]
            let (a, b) = (
                x.to_screen(span.first as f64 - 0.5).max(left),
                x.to_screen(span.last as f64 + 0.5).min(right),
            );
            if b - a >= label_width + 8.0 {
                texts.push(Text::plain(
                    (a + b) / 2.0,
                    top + 16.0,
                    LABEL_SIZE,
                    NS_LABEL.to_owned(),
                    Anchor::Middle,
                    s.text,
                ));
            }
        }
        let (ticks, step) = nice_ticks(0.0, self.y_max, 6);
        let y_ticks = ticks
            .iter()
            .zip(y_tick_labels(&ticks, step))
            .filter(|(t, _)| **t <= self.y_max)
            .map(|(&t, label)| (y.to_screen(t), label))
            .collect();
        let x_ticks = integer_ticks(self.view.0, self.view.1, 8)
            .into_iter()
            .map(|t| {
                #[allow(clippy::cast_precision_loss)]
                let sx = x.to_screen(t as f64);
                (sx, t.to_string())
            })
            .collect();
        let mut legend = Vec::new();
        for (row, entries) in rows.into_iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let ly = bottom + 74.0 + LEGEND_ROW * row as f32;
            for (dx, swatch, label) in entries {
                let lx = left + dx;
                legend.push(LegendEntry {
                    x: lx,
                    y: ly,
                    swatch,
                    label: Text::plain(
                        lx + SWATCH_ADVANCE,
                        ly + 4.0,
                        LABEL_SIZE,
                        label,
                        Anchor::Start,
                        s.text,
                    ),
                });
            }
        }
        Ok(Layout {
            x,
            y,
            texts,
            y_ticks,
            x_ticks,
            legend,
        })
    }

    /// The figure as standalone SVG, with every point (no downsampling) and real text.
    ///
    /// # Errors
    /// [`SceneError::NoData`] when nothing is shown.
    pub fn to_svg(&self) -> Result<String, SceneError> {
        let faces = Faces::read()?;
        let l = self.layout(&faces)?;
        let s = &self.style;
        let (left, right, top, bottom) = (l.x.left, l.x.right, l.y.top, l.y.bottom);
        let mut out = String::with_capacity(64 * 1024);
        let _ = writeln!(
            out,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="IBM Plex Sans, Helvetica, Arial, sans-serif">
  <title>{title}</title>
  <defs>
    <clipPath id="plot"><rect x="{left:.2}" y="{top:.2}" width="{pw:.2}" height="{ph:.2}"/></clipPath>
    <pattern id="ns-hatch" patternUnits="userSpaceOnUse" width="{hs}" height="{hs}" patternTransform="rotate(45)">
      <rect width="{hs}" height="{hs}" fill="{nsfill}"/>
      <line x1="0" y1="0" x2="0" y2="{hs}" stroke="{nshatch}" stroke-width="1"/>
    </pattern>
  </defs>
  <rect width="{w}" height="{h}" fill="{bg}"/>"#,
            w = self.width,
            h = self.height,
            title = xml(&self.title),
            pw = right - left,
            ph = bottom - top,
            hs = HATCH_SPACING,
            nsfill = rgba(s.ns_fill),
            nshatch = rgb(s.ns_hatch),
            bg = rgb(s.background),
        );
        out.push_str("  <g clip-path=\"url(#plot)\">\n");
        for span in &self.series.ns_spans {
            #[allow(clippy::cast_precision_loss)]
            let (a, b) = (
                l.x.to_screen(span.first as f64 - 0.5),
                l.x.to_screen(span.last as f64 + 0.5),
            );
            let _ = writeln!(
                out,
                r#"    <rect x="{a:.2}" y="{top:.2}" width="{:.2}" height="{:.2}" fill="url(#ns-hatch)"><title>positions {}–{}: no usable support</title></rect>"#,
                (b - a).max(0.5),
                bottom - top,
                span.first,
                span.last
            );
        }
        if let Some(mean) = self.mean {
            let my = l.y.to_screen(mean);
            let _ = writeln!(
                out,
                r#"    <line x1="{left:.2}" y1="{my:.2}" x2="{right:.2}" y2="{my:.2}" stroke="{}" stroke-width="1" stroke-dasharray="{} {}"/>"#,
                rgb(s.mean),
                MEAN_DASH.0,
                MEAN_DASH.1
            );
        }
        let (all, markers) = strokes(&self.series, self.view.0, self.view.1, None);
        for stroke in &all {
            let pts: Vec<String> = stroke
                .points
                .iter()
                .map(|&(x, y)| format!("{:.2},{:.2}", l.x.to_screen(x), l.y.to_screen(y)))
                .collect();
            let (color, dash) = match stroke.style {
                Style::Eligible => (s.eligible, String::new()),
                Style::LowSupport => (
                    s.uncorrected,
                    format!(r#" stroke-dasharray="{} {}""#, DASH.0, DASH.1),
                ),
            };
            let _ = writeln!(
                out,
                r#"    <polyline points="{}" fill="none" stroke="{}" stroke-width="{LINE_WIDTH}" stroke-linejoin="round" stroke-linecap="round"{dash}/>"#,
                pts.join(" "),
                rgb(color)
            );
        }
        for m in &markers {
            let color = match m.style {
                Style::Eligible => s.eligible,
                Style::LowSupport => s.uncorrected,
            };
            let _ = writeln!(
                out,
                r#"    <circle cx="{:.2}" cy="{:.2}" r="3" fill="{}"/>"#,
                l.x.to_screen(m.x),
                l.y.to_screen(m.y),
                rgb(color)
            );
        }
        for &p in &self.series.low_coverage {
            #[allow(clippy::cast_precision_loss)]
            let px = l.x.to_screen(p as f64);
            let _ = writeln!(
                out,
                r#"    <line x1="{px:.2}" y1="{top:.2}" x2="{px:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5"/>"#,
                top + 7.0,
                rgb(s.coverage)
            );
        }
        out.push_str("  </g>\n");
        let _ = writeln!(
            out,
            r#"  <path d="M{left:.2},{top:.2} V{bottom:.2} H{right:.2}" fill="none" stroke="{}" stroke-width="1"/>"#,
            rgb(s.axis)
        );
        for (ty, label) in &l.y_ticks {
            let t = l.y_tick_label(*ty, label, s.text);
            let _ = writeln!(
                out,
                r#"  <line x1="{:.2}" y1="{ty:.2}" x2="{left:.2}" y2="{ty:.2}" stroke="{}"/>
  <text x="{:.2}" y="{:.2}" font-size="{LABEL_SIZE}" fill="{}" text-anchor="end">{label}</text>"#,
                left - 5.0,
                rgb(s.axis),
                t.x,
                t.y,
                rgb(t.color)
            );
        }
        for (tx, label) in &l.x_ticks {
            let t = l.x_tick_label(*tx, label, s.text);
            let _ = writeln!(
                out,
                r#"  <line x1="{tx:.2}" y1="{bottom:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{}"/>
  <text x="{tx:.2}" y="{:.2}" font-size="{LABEL_SIZE}" fill="{}" text-anchor="middle">{label}</text>"#,
                bottom + 5.0,
                rgb(s.axis),
                t.y,
                rgb(t.color)
            );
        }
        for e in &l.legend {
            out.push_str(&svg_swatch(e.x, e.y, e.swatch, s));
        }
        for t in l.texts.iter().chain(l.legend.iter().map(|e| &e.label)) {
            let weight = match t.weight {
                Weight::SemiBold => r#" font-weight="600""#,
                Weight::Regular => "",
            };
            let rotate = if t.vertical {
                format!(r#" transform="rotate(-90 {:.2} {:.2})""#, t.x, t.y)
            } else {
                String::new()
            };
            let _ = writeln!(
                out,
                r#"  <text x="{:.2}" y="{:.2}" font-size="{}" fill="{}" text-anchor="{}"{weight}{rotate}>{}</text>"#,
                t.x,
                t.y,
                t.size,
                rgb(t.color),
                t.anchor.svg(),
                xml(&t.text)
            );
        }
        out.push_str("</svg>\n");
        Ok(out)
    }

    /// The figure as a PNG image; lines are M4-downsampled to the plot width.
    ///
    /// # Errors
    /// [`SceneError`] when nothing is shown, or when encoding fails.
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let faces = Faces::read().map_err(|e| e.to_string())?;
        let l = self.layout(&faces).map_err(|e| e.to_string())?;
        let s = &self.style;
        let mut c = Canvas::new(self.width, self.height, s.background);
        let (left, right, top, bottom) = (l.x.left, l.x.right, l.y.top, l.y.bottom);
        let clip = (left, top, right, bottom);
        for span in &self.series.ns_spans {
            #[allow(clippy::cast_precision_loss)]
            let (a, b) = (
                l.x.to_screen(span.first as f64 - 0.5).max(left),
                l.x.to_screen(span.last as f64 + 0.5).min(right),
            );
            c.hatch_rect(a, top, b.max(a + 0.5), bottom, s.ns_fill, s.ns_hatch);
        }
        if let Some(mean) = self.mean {
            let my = l.y.to_screen(mean);
            c.polyline(
                &[(left, my), (right, my)],
                1.0,
                s.mean,
                Some(MEAN_DASH),
                clip,
            );
        }
        // Plot width in pixels bounds the points per column (M4).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let columns = (right - left).ceil().max(1.0) as usize;
        let (all, markers) = strokes(&self.series, self.view.0, self.view.1, Some(columns));
        for stroke in &all {
            let pts: Vec<(f32, f32)> = stroke
                .points
                .iter()
                .map(|&(x, y)| (l.x.to_screen(x), l.y.to_screen(y)))
                .collect();
            match stroke.style {
                Style::Eligible => c.polyline(&pts, LINE_WIDTH, s.eligible, None, clip),
                Style::LowSupport => c.polyline(&pts, LINE_WIDTH, s.uncorrected, Some(DASH), clip),
            }
        }
        for m in &markers {
            let color = match m.style {
                Style::Eligible => s.eligible,
                Style::LowSupport => s.uncorrected,
            };
            c.disc(l.x.to_screen(m.x), l.y.to_screen(m.y), 3.0, color, clip);
        }
        for &p in &self.series.low_coverage {
            #[allow(clippy::cast_precision_loss)]
            let px = l.x.to_screen(p as f64);
            c.polyline(&[(px, top), (px, top + 7.0)], 1.5, s.coverage, None, clip);
        }
        let full = (0.0, 0.0, f32::MAX, f32::MAX);
        c.polyline(
            &[(left, top), (left, bottom), (right, bottom)],
            1.0,
            s.axis,
            None,
            full,
        );
        for (ty, label) in &l.y_ticks {
            c.polyline(&[(left - 5.0, *ty), (left, *ty)], 1.0, s.axis, None, full);
            c.text(&faces, &l.y_tick_label(*ty, label, s.text));
        }
        for (tx, label) in &l.x_ticks {
            c.polyline(
                &[(*tx, bottom), (*tx, bottom + 5.0)],
                1.0,
                s.axis,
                None,
                full,
            );
            c.text(&faces, &l.x_tick_label(*tx, label, s.text));
        }
        for LegendEntry {
            x, y, swatch: sw, ..
        } in &l.legend
        {
            match *sw {
                Swatch::Solid(col) => {
                    c.polyline(&[(*x, *y), (x + 22.0, *y)], LINE_WIDTH, col, None, full)
                }
                Swatch::Dashed(col, (d, g)) => c.polyline(
                    &[(*x, *y), (x + 22.0, *y)],
                    LINE_WIDTH,
                    col,
                    Some((f32::from(d), f32::from(g))),
                    full,
                ),
                Swatch::Hatched => {
                    c.hatch_rect(*x, y - 6.0, x + 22.0, y + 6.0, s.ns_fill, s.ns_hatch)
                }
                Swatch::Tick(col) => c.polyline(
                    &[(x + 11.0, y - 6.0), (x + 11.0, *y + 6.0)],
                    1.5,
                    col,
                    None,
                    full,
                ),
            }
        }
        for t in l.texts.iter().chain(l.legend.iter().map(|e| &e.label)) {
            c.text(&faces, t);
        }
        c.encode()
    }
}

fn svg_swatch(x: f32, y: f32, swatch: Swatch, s: &SceneStyle) -> String {
    match swatch {
        Swatch::Solid(c) => format!(
            "  <line x1=\"{x:.2}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" stroke=\"{}\" stroke-width=\"{LINE_WIDTH}\"/>\n",
            x + 22.0,
            rgb(c)
        ),
        Swatch::Dashed(c, (d, g)) => format!(
            "  <line x1=\"{x:.2}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" stroke=\"{}\" stroke-width=\"{LINE_WIDTH}\" stroke-dasharray=\"{d} {g}\"/>\n",
            x + 22.0,
            rgb(c)
        ),
        Swatch::Hatched => format!(
            "  <rect x=\"{x:.2}\" y=\"{:.2}\" width=\"22\" height=\"12\" fill=\"url(#ns-hatch)\" stroke=\"{}\" stroke-width=\"0.5\"/>\n",
            y - 6.0,
            rgb(s.ns_hatch)
        ),
        Swatch::Tick(c) => format!(
            "  <line x1=\"{0:.2}\" y1=\"{1:.2}\" x2=\"{0:.2}\" y2=\"{2:.2}\" stroke=\"{3}\" stroke-width=\"1.5\"/>\n",
            x + 11.0,
            y - 6.0,
            y + 6.0,
            rgb(c)
        ),
    }
}

fn rgb(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn rgba(c: Rgba) -> String {
    format!(
        "rgba({},{},{},{:.3})",
        c[0],
        c[1],
        c[2],
        f32::from(c[3]) / 255.0
    )
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// A minimal anti-aliased RGBA rasteriser for the PNG export.
struct Canvas {
    img: image::RgbaImage,
}

impl Canvas {
    fn new(width: u32, height: u32, background: Rgba) -> Self {
        Self {
            img: image::RgbaImage::from_pixel(width.max(1), height.max(1), image::Rgba(background)),
        }
    }

    /// Blends `color` at pixel (x, y) with coverage `a` (0–1).
    fn blend(&mut self, x: i64, y: i64, color: Rgba, a: f32) {
        if a <= 0.0
            || x < 0
            || y < 0
            || x >= i64::from(self.img.width())
            || y >= i64::from(self.img.height())
        {
            return;
        }
        let alpha = a.min(1.0) * f32::from(color[3]) / 255.0;
        // In bounds by the check above.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let px = self.img.get_pixel_mut(x as u32, y as u32);
        for i in 0..3 {
            let v = f32::from(color[i]) * alpha + f32::from(px[i]) * (1.0 - alpha);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                px[i] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
        px[3] = 255;
    }

    /// A polyline of width `w`, optionally dashed `(dash, gap)`, clipped to `clip`
    /// `(left, top, right, bottom)`. Coverage is computed from the distance to the segment, so
    /// joins are round and every pixel is blended once per segment.
    fn polyline(
        &mut self,
        pts: &[(f32, f32)],
        w: f32,
        color: Rgba,
        dash: Option<(f32, f32)>,
        clip: (f32, f32, f32, f32),
    ) {
        let half = w / 2.0;
        let mut travelled = 0.0f32;
        for seg in pts.windows(2) {
            let ((x0, y0), (x1, y1)) = (seg[0], seg[1]);
            let len = libm::hypotf(x1 - x0, y1 - y0);
            match dash {
                None => self.segment(x0, y0, x1, y1, half, color, clip),
                Some((d, g)) => {
                    let period = d + g;
                    let mut s = 0.0f32;
                    while s < len {
                        let phase = (travelled + s) % period;
                        if phase < d {
                            let e = (s + (d - phase)).min(len);
                            let (ax, ay) = (x0 + (x1 - x0) * s / len, y0 + (y1 - y0) * s / len);
                            let (bx, by) = (x0 + (x1 - x0) * e / len, y0 + (y1 - y0) * e / len);
                            self.segment(ax, ay, bx, by, half, color, clip);
                            s = e;
                        } else {
                            s += period - phase;
                        }
                    }
                }
            }
            travelled += len;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn segment(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        half: f32,
        color: Rgba,
        clip: (f32, f32, f32, f32),
    ) {
        let pad = half + 1.0;
        let (minx, maxx) = (x0.min(x1) - pad, x0.max(x1) + pad);
        let (miny, maxy) = (y0.min(y1) - pad, y0.max(y1) + pad);
        let (minx, maxx) = (minx.max(clip.0 - 1.0), maxx.min(clip.2 + 1.0));
        let (miny, maxy) = (miny.max(clip.1 - 1.0), maxy.min(clip.3 + 1.0));
        if minx > maxx || miny > maxy {
            return;
        }
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = dx * dx + dy * dy;
        #[allow(clippy::cast_possible_truncation)]
        for py in (miny.floor() as i64)..=(maxy.ceil() as i64) {
            for px in (minx.floor() as i64)..=(maxx.ceil() as i64) {
                #[allow(clippy::cast_precision_loss)]
                let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
                if cx < clip.0 - 0.5 || cx > clip.2 + 0.5 || cy < clip.1 - 0.5 || cy > clip.3 + 0.5
                {
                    continue;
                }
                let t = if len2 > 0.0 {
                    (((cx - x0) * dx + (cy - y0) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let d = libm::hypotf(cx - (x0 + t * dx), cy - (y0 + t * dy));
                let coverage = (half + 0.5 - d).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(px, py, color, coverage);
                }
            }
        }
    }

    fn disc(&mut self, x: f32, y: f32, r: f32, color: Rgba, clip: (f32, f32, f32, f32)) {
        self.segment(x, y, x, y, r, color, clip);
    }

    /// A rectangle filled with `fill` and 45° hatch lines every [`HATCH_SPACING`] pixels.
    fn hatch_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Rgba, hatch: Rgba) {
        #[allow(clippy::cast_possible_truncation)]
        let (ax, bx, ay, by) = (
            x0.floor() as i64,
            x1.ceil() as i64,
            y0.floor() as i64,
            y1.ceil() as i64,
        );
        #[allow(clippy::cast_possible_truncation)]
        let spacing = HATCH_SPACING as i64;
        for py in ay..by {
            for px in ax..bx {
                #[allow(clippy::cast_precision_loss)]
                let cover = ((px as f32 + 1.0).min(x1) - (px as f32).max(x0)).clamp(0.0, 1.0);
                self.blend(px, py, fill, cover);
                if (px + py).rem_euclid(spacing) == 0 {
                    self.blend(px, py, hatch, cover);
                }
            }
        }
    }

    /// A text in its bundled face, sized per em; a vertical one is rotated 90°
    /// counter-clockwise about its anchor (rendered upright, then rotated by whole pixels, so
    /// glyphs stay crisp).
    fn text(&mut self, faces: &Faces, t: &Text) {
        let Text {
            x,
            y,
            size,
            ref text,
            color,
            anchor,
            vertical,
            weight,
        } = *t;
        let font = faces.of(weight);
        let scale = px_scale(font, size);
        let scaled = font.as_scaled(scale);
        let width = text_width(font, size, text);
        let offset = match anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => width / 2.0,
            Anchor::End => width,
        };
        let mut coverage: Vec<(i64, i64, f32)> = Vec::new();
        let mut pen = 0.0f32;
        for ch in text.chars() {
            let id = scaled.scaled_glyph(ch).id;
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(pen - offset, 0.0));
            if let Some(outline) = font.outline_glyph(glyph) {
                let b = outline.px_bounds();
                outline.draw(|gx, gy, c| {
                    #[allow(clippy::cast_possible_truncation)]
                    coverage.push((
                        b.min.x as i64 + i64::from(gx),
                        b.min.y as i64 + i64::from(gy),
                        c,
                    ));
                });
            }
            pen += scaled.h_advance(id);
        }
        #[allow(clippy::cast_possible_truncation)]
        let (ox, oy) = (x.round() as i64, y.round() as i64);
        for (dx, dy, c) in coverage {
            // Rotation by −90°: (dx, dy) → (dy, −dx).
            let (px, py) = if vertical {
                (ox + dy, oy - dx)
            } else {
                (ox + dx, oy + dy)
            };
            self.blend(px, py, color, c);
        }
    }

    fn encode(self) -> Result<Vec<u8>, String> {
        let mut buf = std::io::Cursor::new(Vec::new());
        self.img
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        Ok(buf.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::metric::ChartMetric;
    use crate::charts::series::tests::{position, results_of};
    use crate::charts::transform::position_extent;

    /// A fixed scene with every element: eligible and uncorrected segments, an NS band wide
    /// enough for its label, a filtered-out gap, a single-point path, a mean and coverage flags.
    pub(crate) fn fixture() -> ChartScene {
        let mut positions = Vec::new();
        for p in 1..=400u64 {
            #[allow(clippy::cast_precision_loss)]
            let y = libm::sin((p as f64) / 17.0).abs() * 1.6 + if p % 50 == 0 { 0.8 } else { 0.0 };
            let support = match p {
                120..=150 => 40,
                200..=260 => 0,
                300 => 100,
                _ => 191,
            };
            let mut pos = position(p, (support > 0).then_some(y), support, 100);
            if p % 97 == 0 {
                pos.below_min_coverage = Some(true);
            }
            positions.push(pos);
        }
        let r = results_of(positions);
        let shown: Vec<usize> = (0..400)
            .filter(|&i| !(330..340).contains(&i) && i != 341)
            .collect();
        let series = Series::build(&r, &shown, ChartMetric::Entropy);
        ChartScene {
            title: "test <query> & co \u{2014} Shannon entropy".to_owned(),
            subtitle: "k = 9, T = 100, 191 sequences".to_owned(),
            y_title: "Shannon entropy (bits)".to_owned(),
            width: 1200,
            height: 520,
            view: position_extent(1, 400),
            y_max: crate::charts::transform::y_axis_max(series.y_max),
            series,
            mean: Some(0.75),
            legend: crate::charts::series::legend(
                100,
                dima_core::config::Estimator::Dima,
                ChartMetric::Entropy,
            ),
            min_coverage: Some(0.95),
            style: SceneStyle::publication(),
        }
    }

    #[test]
    fn svg_is_complete_escaped_and_full_resolution() {
        let svg = fixture().to_svg().unwrap();
        assert!(svg.starts_with("<?xml") && svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("test &lt;query&gt; &amp; co"));
        assert!(!svg.contains("<query>"));
        assert!(svg.contains("no usable support"));
        assert!(
            svg.contains("stroke-dasharray=\"6 4\""),
            "dashed uncorrected segments"
        );
        assert!(svg.contains("url(#ns-hatch)"));
        assert!(svg.contains("eligible mean 0.7500"));
        assert!(svg.contains("support &gt; 100: corrected"));
        assert!(svg.contains("support \u{2264} 100: uncorrected"));
        assert!(svg.contains("sample coverage &lt; 0.95"));
        // Full resolution: every eligible point between 1 and 119 is a vertex.
        let first = svg.find("<polyline").unwrap();
        let points = svg[first..].split('"').nth(1).unwrap();
        assert!(points.split(' ').count() >= 119);
        assert!(
            svg.contains("<circle"),
            "the single point after the gap is a marker"
        );
    }

    #[test]
    fn png_draws_every_element() {
        let bytes = fixture().to_png().unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!((img.width(), img.height()), (1200, 520));
        let has = |c: Rgba| img.pixels().filter(|p| p.0[..3] == c[..3]).count();
        let s = SceneStyle::publication();
        assert!(has(s.eligible) > 100, "solid line");
        assert!(has(s.uncorrected) > 20, "dashed line");
        assert!(has(s.ns_hatch) > 100, "hatch");
    }

    #[test]
    fn empty_and_tiny_scenes() {
        let mut s = fixture();
        s.series = Series::default();
        assert_eq!(s.to_svg().unwrap_err(), SceneError::NoData);
        assert!(s.to_png().is_err());
        let mut s = fixture();
        s.width = 40;
        s.height = 30;
        assert!(s.to_svg().is_ok());
        assert!(s.to_png().is_ok());
    }

    /// The box `(left, top, right, bottom)` a text covers: its advance width in its face, and
    /// the face's ascent and descent (rotated for a vertical text).
    fn text_box(faces: &Faces, t: &Text) -> (f32, f32, f32, f32) {
        let font = faces.of(t.weight);
        let scaled = font.as_scaled(px_scale(font, t.size));
        let (ascent, descent) = (scaled.ascent(), scaled.descent());
        let width = text_width(font, t.size, &t.text);
        let start = match t.anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => -width / 2.0,
            Anchor::End => -width,
        };
        if t.vertical {
            (
                t.x - ascent,
                t.y - start - width,
                t.x - descent,
                t.y - start,
            )
        } else {
            (
                t.x + start,
                t.y - ascent,
                t.x + start + width,
                t.y - descent,
            )
        }
    }

    /// Every text of the figure inside the canvas, legend labels ending before the next
    /// swatch, a plot at least 100 px tall, and room for either value-axis title at 13 px.
    fn assert_layout_fits(scene: &ChartScene) -> Layout {
        let faces = Faces::read().unwrap();
        let l = scene.layout(&faces).unwrap();
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (scene.width as f32, scene.height as f32);
        let inside = |t: &Text| {
            let (a, b, c, d) = text_box(&faces, t);
            assert!(
                a >= 0.0 && b >= 0.0 && c <= w && d <= h,
                "{:?} at ({a}, {b})-({c}, {d}) is outside {w} x {h}",
                t.text
            );
        };
        let s = &scene.style;
        let ticks: Vec<Text> = l
            .y_ticks
            .iter()
            .map(|(ty, label)| l.y_tick_label(*ty, label, s.text))
            .chain(
                l.x_ticks
                    .iter()
                    .map(|(tx, label)| l.x_tick_label(*tx, label, s.text)),
            )
            .collect();
        for t in l.texts.iter().chain(l.legend.iter().map(|e| &e.label)) {
            inside(t);
        }
        ticks.iter().for_each(inside);
        for pair in l.legend.windows(2) {
            if pair[0].y == pair[1].y {
                let end = text_box(&faces, &pair[0].label).2;
                assert!(
                    end < pair[1].x,
                    "{:?} ends at {end}, after the next swatch at {}",
                    pair[0].label.text,
                    pair[1].x
                );
            }
        }
        assert!(
            l.y.bottom - l.y.top >= 100.0,
            "plot {}",
            l.y.bottom - l.y.top
        );
        for title in [
            "Shannon entropy (bits)",
            "Similarity-weighted entropy (bits)",
        ] {
            let y_title = l.texts.iter().find(|t| t.vertical).unwrap();
            inside(&Text {
                text: title.to_owned(),
                ..y_title.clone()
            });
            assert_eq!(y_title.size, 13.0);
        }
        l
    }

    #[test]
    fn layouts_fit_from_the_minimum_figure_size() {
        let (min_w, min_h) = (
            *crate::state::settings::FIGURE_WIDTH.start(),
            *crate::state::settings::FIGURE_HEIGHT.start(),
        );
        // The smallest figure with the longest legend: T = u64::MAX and coverage marks.
        let mut small = fixture();
        (small.width, small.height) = (min_w, min_h);
        small.legend = crate::charts::series::legend(
            u64::MAX,
            dima_core::config::Estimator::Dima,
            ChartMetric::Entropy,
        );
        small.subtitle = format!("k = 9, T = {}, 191 sequences", u64::MAX);
        let l = assert_layout_fits(&small);
        let rows = l
            .legend
            .iter()
            .map(|e| e.y.to_bits())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert_eq!(rows, 3, "the legend wraps");
        #[allow(clippy::cast_precision_loss)]
        let expected = small.height as f32 - MARGIN_BOTTOM - 2.0 * LEGEND_ROW;
        assert_eq!(l.y.bottom, expected, "20 px per extra row");
        // The default size, a 200-character query name, the golden fixture.
        let (dw, dh) = crate::state::settings::Settings::default().figure_size;
        let mut default = fixture();
        (default.width, default.height) = (dw, dh);
        assert_layout_fits(&default);
        let mut long = default.clone();
        let query: String = "Middle East respiratory syndrome coronavirus spike glycoprotein "
            .chars()
            .cycle()
            .take(200)
            .collect();
        long.title = format!("{query} \u{2014} Shannon entropy per k-mer position");
        let l = assert_layout_fits(&long);
        assert!(l.texts[0].text.ends_with('\u{2026}'), "{}", l.texts[0].text);
        assert!(
            long.title
                .starts_with(l.texts[0].text.trim_end_matches('\u{2026}'))
        );
        let svg = long.to_svg().unwrap();
        assert!(
            svg.contains(&format!("<title>{}</title>", xml(&long.title))),
            "the full title"
        );
        let l = assert_layout_fits(&fixture());
        assert_eq!(l.texts[0].weight, Weight::SemiBold, "the title");
        assert!(
            l.legend.iter().all(|e| e.y == l.legend[0].y),
            "one legend row"
        );
    }

    #[test]
    fn long_texts_are_shortened_between_characters() {
        let faces = Faces::read().unwrap();
        let f = &faces.regular;
        assert_eq!(fit(f, 12.0, "eligible mean", 1000.0), "eligible mean");
        let short = fit(
            f,
            12.0,
            "\u{3b1}\u{3b2}\u{3b3}\u{3b4}\u{3b5}\u{3b6}\u{3b7} \u{3b8}",
            40.0,
        );
        assert!(
            short.ends_with('\u{2026}') && text_width(f, 12.0, &short) <= 40.0,
            "{short}"
        );
        assert_eq!(
            fit(f, 12.0, "anything", 1.0),
            "",
            "not even the ellipsis fits"
        );
        // Text is sized per em, as SVG and the screen size it: 1.3 times ab_glyph's height scale.
        let em = text_width(f, 13.0, "Similarity-weighted entropy (bits)");
        let height_scaled = {
            let scaled = f.as_scaled(PxScale::from(13.0));
            "Similarity-weighted entropy (bits)"
                .chars()
                .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
                .sum::<f32>()
        };
        assert!(
            (em / height_scaled - 1.3).abs() < 1e-4,
            "{em} {height_scaled}"
        );
        assert!((190.0..200.0).contains(&em), "{em}");
    }

    /// The PNG is compared byte for byte with the committed golden image (G48: a layout change
    /// shows up as a diff). Re-bless with `DIMA_BLESS=1 cargo test -p dima-gui`.
    #[test]
    fn png_matches_golden_image() {
        let golden =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/entropy_chart.png");
        let bytes = fixture().to_png().unwrap();
        if std::env::var_os("DIMA_BLESS").is_some() {
            std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
            std::fs::write(&golden, &bytes).unwrap();
        }
        let expected =
            std::fs::read(&golden).expect("golden image missing; bless with DIMA_BLESS=1");
        assert!(
            bytes == expected,
            "PNG differs from {}; inspect and re-bless",
            golden.display()
        );
    }

    #[test]
    fn svg_matches_golden_file() {
        let golden =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/entropy_chart.svg");
        let svg = fixture().to_svg().unwrap();
        if std::env::var_os("DIMA_BLESS").is_some() {
            std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
            std::fs::write(&golden, &svg).unwrap();
        }
        let expected =
            std::fs::read_to_string(&golden).expect("golden SVG missing; bless with DIMA_BLESS=1");
        assert!(
            svg == expected,
            "SVG differs from {}; inspect and re-bless",
            golden.display()
        );
    }
}
