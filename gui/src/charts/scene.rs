//! Backend-agnostic description of the entropy chart, renderable to file.
//!
//! The on-screen chart is painted by egui; a file export has no egui frame, so
//! this module owns an independent description of the same figure and renders
//! it to either SVG (vector, for publication figures) or PNG (raster, for quick
//! sharing). Keeping one scene description behind both renderers means the two
//! formats cannot drift apart (SRP: geometry here, pixels/markup in the two
//! `render_*` functions).
//!
//! Axis ticks reuse the same "nice numbers" generator as the interactive chart
//! (Heckbert 1990), so exported axes match the on-screen ones.

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};

use crate::charts::axis::nice_ticks;

/// RGBA colour, kept backend-neutral (egui types stay out of this module).
pub type Rgba = [u8; 4];

/// Colours for an exported figure, derived from the active theme.
#[derive(Debug, Clone, Copy)]
pub struct SceneStyle {
    pub background: Rgba,
    pub axis: Rgba,
    pub text: Rgba,
    pub line: Rgba,
    /// Soft area under the entropy line.
    pub fill: Rgba,
    pub average: Rgba,
}

impl SceneStyle {
    /// Light, print-friendly defaults: exported figures normally go into papers
    /// and slides on a white background regardless of the in-app theme.
    pub fn publication() -> Self {
        Self {
            background: [255, 255, 255, 255],
            axis: [107, 114, 128, 255],
            text: [25, 25, 30, 255],
            line: [37, 99, 235, 255],
            fill: [37, 99, 235, 38],
            average: [220, 38, 38, 255],
        }
    }
}

/// Everything needed to draw one entropy figure.
#[derive(Debug, Clone)]
pub struct ChartScene {
    pub title: String,
    /// `(position, entropy)` pairs, ascending by position, already filtered.
    pub points: Vec<(f64, f64)>,
    pub average_entropy: f64,
    pub width: u32,
    pub height: u32,
    pub style: SceneStyle,
}

/// Pixel geometry of the plotting area, computed once and shared by both
/// renderers so SVG and PNG agree exactly.
#[derive(Debug, Clone, Copy)]
struct Geometry {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    x_min: f64,
    x_max: f64,
    y_max: f64,
}

impl Geometry {
    fn width(&self) -> f64 {
        self.right - self.left
    }
    fn height(&self) -> f64 {
        self.bottom - self.top
    }

    /// Data x -> pixel x.
    fn px(&self, x: f64) -> f64 {
        let span = self.x_max - self.x_min;
        if span <= 0.0 {
            self.left
        } else {
            self.left + (x - self.x_min) / span * self.width()
        }
    }

    /// Data y -> pixel y (screen axis points down).
    fn py(&self, y: f64) -> f64 {
        if self.y_max <= 0.0 {
            self.bottom
        } else {
            self.bottom - (y / self.y_max) * self.height()
        }
    }
}

/// Margins reserved for axis labels and titles.
const MARGIN_LEFT: f64 = 78.0;
const MARGIN_RIGHT: f64 = 26.0;
const MARGIN_TOP: f64 = 44.0;
const MARGIN_BOTTOM: f64 = 58.0;

/// Reason an export could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    /// No finite data points to draw.
    NoData,
    /// All entropies are zero/non-finite, so there is no vertical range.
    NoRange,
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoData => write!(f, "no positions to plot (check your filters)"),
            Self::NoRange => write!(f, "all entropy values are zero — nothing to plot"),
        }
    }
}

impl ChartScene {
    /// Compute the plotting geometry, rejecting degenerate data up front so both
    /// renderers can assume a well-formed scene.
    fn geometry(&self) -> Result<Geometry, SceneError> {
        let finite: Vec<(f64, f64)> = self
            .points
            .iter()
            .copied()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect();
        if finite.is_empty() {
            return Err(SceneError::NoData);
        }

        let x_min = finite.first().map(|(x, _)| *x).unwrap_or(0.0);
        let x_max = finite.last().map(|(x, _)| *x).unwrap_or(x_min + 1.0);
        let y_max = finite.iter().map(|(_, y)| *y).fold(0.0_f64, f64::max);
        if y_max <= 0.0 {
            return Err(SceneError::NoRange);
        }

        Ok(Geometry {
            left: MARGIN_LEFT,
            right: (self.width as f64 - MARGIN_RIGHT).max(MARGIN_LEFT + 1.0),
            top: MARGIN_TOP,
            bottom: (self.height as f64 - MARGIN_BOTTOM).max(MARGIN_TOP + 1.0),
            x_min,
            // Guard a single-position dataset, which would give a zero span.
            x_max: if x_max > x_min { x_max } else { x_min + 1.0 },
            y_max,
        })
    }

    /// Points with non-finite values removed, in draw order.
    fn finite_points(&self) -> Vec<(f64, f64)> {
        self.points
            .iter()
            .copied()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect()
    }

    /// Render to standalone SVG markup.
    ///
    /// Text is emitted as real `<text>`, so exported figures stay sharp at any
    /// zoom and remain editable in Illustrator/Inkscape.
    pub fn to_svg(&self) -> Result<String, SceneError> {
        let g = self.geometry()?;
        let pts = self.finite_points();
        let s = &self.style;

        let mut out = String::with_capacity(8 * 1024);
        out.push_str(&format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="IBM Plex Sans, Helvetica, Arial, sans-serif">
"#,
            w = self.width,
            h = self.height
        ));
        out.push_str(&format!(
            r#"  <rect width="{}" height="{}" fill="{}"/>
"#,
            self.width,
            self.height,
            css_rgb(s.background)
        ));

        // Title.
        out.push_str(&format!(
            r#"  <text x="{x:.1}" y="{y:.1}" fill="{c}" font-size="18" font-weight="600">{t}</text>
"#,
            x = g.left,
            y = MARGIN_TOP - 18.0,
            c = css_rgb(s.text),
            t = escape_xml(&self.title)
        ));

        // Area fill under the line, then the line itself.
        let mut area = String::new();
        area.push_str(&format!("{:.2},{:.2} ", g.px(pts[0].0), g.bottom));
        for (x, y) in &pts {
            area.push_str(&format!("{:.2},{:.2} ", g.px(*x), g.py(*y)));
        }
        area.push_str(&format!(
            "{:.2},{:.2}",
            g.px(pts[pts.len() - 1].0),
            g.bottom
        ));
        out.push_str(&format!(
            r#"  <polygon points="{area}" fill="{fill}" stroke="none"/>
"#,
            fill = css_rgba(s.fill)
        ));

        let line: String = pts
            .iter()
            .map(|(x, y)| format!("{:.2},{:.2}", g.px(*x), g.py(*y)))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            r#"  <polyline points="{line}" fill="none" stroke="{c}" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>
"#,
            c = css_rgb(s.line)
        ));

        // Average reference line.
        if self.average_entropy.is_finite() && self.average_entropy > 0.0 {
            let y = g.py(self.average_entropy);
            if y >= g.top && y <= g.bottom {
                out.push_str(&format!(
                    r#"  <line x1="{x1:.1}" y1="{y:.2}" x2="{x2:.1}" y2="{y:.2}" stroke="{c}" stroke-width="1" stroke-dasharray="6 4"/>
  <text x="{tx:.1}" y="{ty:.2}" fill="{c}" font-size="12" text-anchor="end">avg {avg:.3}</text>
"#,
                    x1 = g.left,
                    x2 = g.right,
                    c = css_rgb(s.average),
                    tx = g.right - 4.0,
                    ty = y - 4.0,
                    avg = self.average_entropy
                ));
            }
        }

        // Axes.
        out.push_str(&format!(
            r#"  <line x1="{l:.1}" y1="{t:.1}" x2="{l:.1}" y2="{b:.1}" stroke="{c}" stroke-width="1"/>
  <line x1="{l:.1}" y1="{b:.1}" x2="{r:.1}" y2="{b:.1}" stroke="{c}" stroke-width="1"/>
"#,
            l = g.left,
            r = g.right,
            t = g.top,
            b = g.bottom,
            c = css_rgb(s.axis)
        ));

        // Y ticks + labels.
        for tick in nice_ticks(0.0, g.y_max, 6) {
            if tick < 0.0 || tick > g.y_max {
                continue;
            }
            let y = g.py(tick);
            out.push_str(&format!(
                r#"  <line x1="{x1:.1}" y1="{y:.2}" x2="{x2:.1}" y2="{y:.2}" stroke="{c}" stroke-width="1"/>
  <text x="{tx:.1}" y="{ty:.2}" fill="{tc}" font-size="12" text-anchor="end">{v:.2}</text>
"#,
                x1 = g.left - 5.0,
                x2 = g.left,
                y = y,
                c = css_rgb(s.axis),
                tx = g.left - 9.0,
                ty = y + 4.0,
                tc = css_rgb(s.text),
                v = tick
            ));
        }

        // X ticks + labels.
        for tick in nice_ticks(g.x_min, g.x_max, 8) {
            if tick < g.x_min || tick > g.x_max {
                continue;
            }
            let x = g.px(tick);
            out.push_str(&format!(
                r#"  <line x1="{x:.2}" y1="{y1:.1}" x2="{x:.2}" y2="{y2:.1}" stroke="{c}" stroke-width="1"/>
  <text x="{x:.2}" y="{ty:.1}" fill="{tc}" font-size="12" text-anchor="middle">{v}</text>
"#,
                x = x,
                y1 = g.bottom,
                y2 = g.bottom + 5.0,
                c = css_rgb(s.axis),
                ty = g.bottom + 20.0,
                tc = css_rgb(s.text),
                v = tick.round() as i64
            ));
        }

        // Axis titles.
        out.push_str(&format!(
            r#"  <text x="{x:.1}" y="{y:.1}" fill="{c}" font-size="13" text-anchor="middle">Position</text>
  <text x="{ry:.1}" y="{rx:.1}" fill="{c}" font-size="13" text-anchor="middle" transform="rotate(-90 {ry:.1} {rx:.1})">Shannon entropy (bits)</text>
</svg>
"#,
            x = (g.left + g.right) / 2.0,
            y = g.bottom + 42.0,
            c = css_rgb(s.text),
            ry = 20.0,
            rx = (g.top + g.bottom) / 2.0
        ));

        Ok(out)
    }

    /// Render to an RGBA raster image.
    pub fn to_png_bytes(&self) -> Result<Vec<u8>, String> {
        let g = self.geometry().map_err(|e| e.to_string())?;
        let pts = self.finite_points();
        let s = &self.style;

        let mut img =
            image::RgbaImage::from_pixel(self.width, self.height, image::Rgba(s.background));

        // Same face as the UI, so an exported figure matches the screen.
        let font = FontRef::try_from_slice(crate::theme::fonts::PLEX_SANS)
            .map_err(|e| format!("bundled figure font is unusable: {e}"))?;

        // Soft area fill: one vertical span per pixel column under the line.
        for w in pts.windows(2) {
            let (x0, y0) = (g.px(w[0].0), g.py(w[0].1));
            let (x1, y1) = (g.px(w[1].0), g.py(w[1].1));
            let steps = (x1 - x0).abs().ceil().max(1.0) as i64;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let x = x0 + t * (x1 - x0);
                let y = y0 + t * (y1 - y0);
                fill_column(&mut img, x, y, g.bottom, s.fill);
            }
        }

        // Average reference line (dashed), drawn under the data line.
        if self.average_entropy.is_finite() && self.average_entropy > 0.0 {
            let y = g.py(self.average_entropy);
            if y >= g.top && y <= g.bottom {
                let mut x = g.left;
                while x < g.right {
                    let seg_end = (x + 6.0).min(g.right);
                    draw_line(&mut img, x, y, seg_end, y, 1.0, s.average);
                    x += 10.0;
                }
                draw_text(
                    &mut img,
                    &font,
                    12.0,
                    g.right - 4.0,
                    y - 6.0,
                    &format!("avg {:.3}", self.average_entropy),
                    s.average,
                    Anchor::End,
                );
            }
        }

        // Entropy line.
        for w in pts.windows(2) {
            draw_line(
                &mut img,
                g.px(w[0].0),
                g.py(w[0].1),
                g.px(w[1].0),
                g.py(w[1].1),
                1.8,
                s.line,
            );
        }

        // Axes.
        draw_line(&mut img, g.left, g.top, g.left, g.bottom, 1.0, s.axis);
        draw_line(&mut img, g.left, g.bottom, g.right, g.bottom, 1.0, s.axis);

        // Y ticks + labels.
        for tick in nice_ticks(0.0, g.y_max, 6) {
            if tick < 0.0 || tick > g.y_max {
                continue;
            }
            let y = g.py(tick);
            draw_line(&mut img, g.left - 5.0, y, g.left, y, 1.0, s.axis);
            draw_text(
                &mut img,
                &font,
                12.0,
                g.left - 9.0,
                y + 4.0,
                &format!("{tick:.2}"),
                s.text,
                Anchor::End,
            );
        }

        // X ticks + labels.
        for tick in nice_ticks(g.x_min, g.x_max, 8) {
            if tick < g.x_min || tick > g.x_max {
                continue;
            }
            let x = g.px(tick);
            draw_line(&mut img, x, g.bottom, x, g.bottom + 5.0, 1.0, s.axis);
            draw_text(
                &mut img,
                &font,
                12.0,
                x,
                g.bottom + 20.0,
                &format!("{}", tick.round() as i64),
                s.text,
                Anchor::Middle,
            );
        }

        // Titles.
        draw_text(
            &mut img,
            &font,
            18.0,
            g.left,
            MARGIN_TOP - 18.0,
            &self.title,
            s.text,
            Anchor::Start,
        );
        draw_text(
            &mut img,
            &font,
            13.0,
            (g.left + g.right) / 2.0,
            g.bottom + 42.0,
            "Position",
            s.text,
            Anchor::Middle,
        );
        // The Y title is drawn horizontally at the top-left of the axis: rotating
        // raster glyphs would soften them, and this keeps the label crisp.
        draw_text(
            &mut img,
            &font,
            13.0,
            g.left - 5.0,
            g.top - 10.0,
            "Entropy (bits)",
            s.text,
            Anchor::Start,
        );

        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| format!("failed to encode PNG: {e}"))?;
        Ok(buf.into_inner())
    }
}

/// Horizontal anchoring for rasterised text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Start,
    Middle,
    End,
}

fn css_rgb(c: Rgba) -> String {
    format!("rgb({},{},{})", c[0], c[1], c[2])
}

fn css_rgba(c: Rgba) -> String {
    format!(
        "rgba({},{},{},{:.3})",
        c[0],
        c[1],
        c[2],
        c[3] as f64 / 255.0
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Alpha-blend `color` onto the pixel at `(x, y)` with coverage `a` in `0..=1`.
fn blend(img: &mut image::RgbaImage, x: i64, y: i64, color: Rgba, a: f32) {
    if a <= 0.0 || x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let a = (a.min(1.0)) * (color[3] as f32 / 255.0);
    if a <= 0.0 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    for i in 0..3 {
        px[i] = (color[i] as f32 * a + px[i] as f32 * (1.0 - a)).round() as u8;
    }
    px[3] = 255;
}

/// Draw a straight line with the given thickness, anti-aliased along its length.
fn draw_line(img: &mut image::RgbaImage, x0: f64, y0: f64, x1: f64, y1: f64, w: f64, color: Rgba) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    // Oversample so diagonal strokes stay continuous.
    let steps = (len * 2.0).ceil().max(1.0) as i64;
    let half = (w / 2.0).max(0.5);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = x0 + t * dx;
        let y = y0 + t * dy;
        // Stamp a small square whose extent matches the requested thickness.
        let r = half.ceil() as i64;
        for oy in -r..=r {
            for ox in -r..=r {
                let d = ((ox as f64).powi(2) + (oy as f64).powi(2)).sqrt();
                if d <= half {
                    blend(
                        img,
                        x.round() as i64 + ox,
                        y.round() as i64 + oy,
                        color,
                        1.0,
                    );
                }
            }
        }
    }
}

/// Fill the vertical span between the curve and the baseline at one column.
fn fill_column(img: &mut image::RgbaImage, x: f64, y_top: f64, y_bottom: f64, color: Rgba) {
    let xi = x.round() as i64;
    let (mut a, b) = (y_top.round() as i64, y_bottom.round() as i64);
    while a <= b {
        blend(img, xi, a, color, 1.0);
        a += 1;
    }
}

/// Rasterise `text` with the bundled font.
#[allow(clippy::too_many_arguments)]
fn draw_text(
    img: &mut image::RgbaImage,
    font: &FontRef<'_>,
    size: f32,
    x: f64,
    baseline_y: f64,
    text: &str,
    color: Rgba,
    anchor: Anchor,
) {
    let scaled = font.as_scaled(PxScale::from(size));

    // Measure first so the anchor can be applied.
    let width: f32 = text
        .chars()
        .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
        .sum();
    let start_x = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - width as f64 / 2.0,
        Anchor::End => x - width as f64,
    };

    let mut pen = start_x as f32;
    for ch in text.chars() {
        let glyph_id = scaled.scaled_glyph(ch).id;
        let advance = scaled.h_advance(glyph_id);
        let glyph = glyph_id
            .with_scale_and_position(PxScale::from(size), ab_glyph::point(pen, baseline_y as f32));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                blend(
                    img,
                    bounds.min.x as i64 + gx as i64,
                    bounds.min.y as i64 + gy as i64,
                    color,
                    coverage,
                );
            });
        }
        pen += advance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(points: Vec<(f64, f64)>) -> ChartScene {
        ChartScene {
            title: "test <query> & co".to_string(),
            points,
            average_entropy: 0.5,
            width: 800,
            height: 400,
            style: SceneStyle::publication(),
        }
    }

    fn sample_points() -> Vec<(f64, f64)> {
        (1..=200)
            .map(|i| (i as f64, ((i as f64) / 20.0).sin().abs() * 2.0))
            .collect()
    }

    #[test]
    fn svg_is_well_formed_and_escaped() {
        let svg = scene(sample_points()).to_svg().expect("svg");
        assert!(svg.starts_with("<?xml"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("<polyline"));
        assert!(svg.contains("Position"));
        assert!(svg.contains("Shannon entropy"));
        // Title must be XML-escaped, never injected raw.
        assert!(svg.contains("test &lt;query&gt; &amp; co"));
        assert!(!svg.contains("test <query>"));
    }

    #[test]
    fn png_encodes_and_round_trips() {
        let bytes = scene(sample_points()).to_png_bytes().expect("png");
        assert!(bytes.len() > 1000);
        // Valid PNG magic.
        assert_eq!(
            &bytes[0..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        let decoded = image::load_from_memory(&bytes).expect("decodable png");
        assert_eq!(decoded.width(), 800);
        assert_eq!(decoded.height(), 400);
    }

    #[test]
    fn png_actually_draws_ink() {
        // Guards against a silently blank export: some pixels must differ from
        // the background, proving line and glyph rasterisation ran.
        let bytes = scene(sample_points()).to_png_bytes().expect("png");
        let img = image::load_from_memory(&bytes).expect("png").to_rgba8();
        let non_background = img.pixels().filter(|p| p.0 != [255, 255, 255, 255]).count();
        assert!(
            non_background > 500,
            "expected substantial ink, found {non_background} non-background pixels"
        );
    }

    #[test]
    fn rejects_empty_and_flat_data() {
        assert_eq!(scene(vec![]).to_svg().unwrap_err(), SceneError::NoData);
        // All-zero entropy has no vertical range to plot.
        let flat = vec![(1.0, 0.0), (2.0, 0.0)];
        assert_eq!(scene(flat).to_svg().unwrap_err(), SceneError::NoRange);
    }

    #[test]
    fn ignores_non_finite_points() {
        let pts = vec![
            (1.0, 1.0),
            (2.0, f64::NAN),
            (3.0, 2.0),
            (f64::INFINITY, 1.0),
        ];
        // Must not panic and must still produce a figure from the finite subset.
        assert!(scene(pts).to_svg().is_ok());
    }

    #[test]
    fn handles_single_point_without_zero_span() {
        // A one-position dataset would otherwise divide by a zero x-span.
        assert!(scene(vec![(42.0, 1.0)]).to_svg().is_ok());
        assert!(scene(vec![(42.0, 1.0)]).to_png_bytes().is_ok());
    }

    #[test]
    fn tiny_canvas_does_not_panic() {
        // Margins exceed the canvas: geometry must clamp rather than invert.
        let mut s = scene(sample_points());
        s.width = 40;
        s.height = 40;
        assert!(s.to_svg().is_ok());
        assert!(s.to_png_bytes().is_ok());
    }
}
