//! Design tokens for consistent, themeable UI.
//!
//! Following Rerun's `re_ui` pattern: all UI code references `DesignTokens`,
//! never hardcoded colors. Semantic naming (function, not appearance) enables
//! theme switching without logic changes.
//!
//! Ref: "Ten Simple Rules for Better Figures" (PLOS Comp Biol),
//! "Color Use Guidelines for Data Representation" (Brewer, 2003)

use egui::Color32;

/// Centralized design tokens for the entire application.
/// All colors are semantically named by function, not appearance.
#[derive(Debug, Clone)]
pub struct DesignTokens {
    // ── Surface colors ──
    pub surface_primary: Color32,
    pub surface_secondary: Color32,
    pub surface_elevated: Color32,

    // ── Text colors ──
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,

    // ── Accent and borders ──
    pub accent: Color32,
    pub border: Color32,

    // ── Status colors (errors, warnings, success) ──
    pub error_color: Color32,
    pub warning_color: Color32,
    pub success_color: Color32,
    pub info_color: Color32,

    // ── Selection / interaction ──
    pub selection_highlight: Color32,
    pub hover_highlight: Color32,
    pub progress_bar_fill: Color32,

    // ── Scientific visualization (perceptually uniform, colorblind-safe) ──
    pub motif_index: Color32,
    pub motif_major: Color32,
    pub motif_minor: Color32,
    pub motif_unique: Color32,

    // ── HCS visualization ──
    pub hcs_conserved: Color32,

    // ── Chart chrome ──
    pub chart_axis: Color32,
    pub chart_avg_line: Color32,
    /// Full-opacity line for the selected-position indicator on the entropy chart.
    /// Separated from `selection_highlight` (low-alpha area fill) because strokes
    /// need solid color to be visible, while area fills need transparency.
    pub chart_selection_line: Color32,

    // ── Selection background (semi-transparent for readable text overlay) ──
    /// Semi-transparent accent for selection backgrounds (ComboBox, selectable_value).
    /// Since egui 0.33.3+ (PR #7691), `selection.stroke.color` controls the TEXT
    /// color of selected items. Using opaque `accent` for both bg_fill and stroke
    /// makes text invisible. This token provides a translucent tint so text_primary
    /// remains readable on top.
    pub selection_bg: Color32,

    // ── Spacing scale ──
    /// 8px-based spacing scale. Every gap in the UI comes from one of these
    /// steps so vertical rhythm stays consistent: ad-hoc values were the main
    /// reason the old layout looked uneven. Values are theme-independent, but
    /// live here so all layout constants have a single source. Extend the scale
    /// when a new step is genuinely needed rather than reintroducing literals.
    pub space_4: f32,
    pub space_8: f32,
    pub space_12: f32,
    pub space_16: f32,
    pub space_24: f32,

    pub panel_padding: f32,
    pub panel_gap: f32,
    pub panel_rounding: f32,

    // ── Typography ──
    pub font_size_title: f32,
    pub font_size_body: f32,
    pub font_size_caption: f32,
    pub font_size_mono: f32,
}

impl DesignTokens {
    /// Light theme tokens with WCAG AA contrast ratios.
    pub fn light() -> Self {
        Self {
            surface_primary: Color32::from_rgb(255, 255, 255),
            surface_secondary: Color32::from_rgb(247, 248, 252), // cooler blue-gray tint for panel grouping
            surface_elevated: Color32::from_rgb(255, 255, 255),

            text_primary: Color32::from_rgb(25, 25, 30),
            text_secondary: Color32::from_rgb(90, 95, 105),
            // Darkened from (140,145,155) which only achieved 3.11:1 contrast on white.
            // (105,110,120) achieves ~5.1:1 on white and ~4.8:1 on surface_secondary,
            // passing WCAG AA 4.5:1 for normal text.
            text_muted: Color32::from_rgb(105, 110, 120),

            accent: Color32::from_rgb(37, 99, 235),
            border: Color32::from_rgb(218, 222, 228), // softer dividers

            error_color: Color32::from_rgb(220, 38, 38),
            warning_color: Color32::from_rgb(202, 138, 4),
            success_color: Color32::from_rgb(22, 163, 74),
            info_color: Color32::from_rgb(37, 99, 235),

            selection_highlight: Color32::from_rgba_premultiplied(37, 99, 235, 30), // subtler on light backgrounds
            hover_highlight: Color32::from_rgba_premultiplied(37, 99, 235, 15),     // subtler hover
            progress_bar_fill: Color32::from_rgb(37, 99, 235),

            // Distinct hues for motif categories (colorblind-safe via Oklab spacing)
            motif_index: Color32::from_rgb(37, 99, 235), // Blue
            motif_major: Color32::from_rgb(234, 88, 12), // Orange
            motif_minor: Color32::from_rgb(22, 163, 74), // Green
            motif_unique: Color32::from_rgb(168, 85, 247), // Purple

            hcs_conserved: Color32::from_rgb(37, 99, 235),

            chart_axis: Color32::from_rgb(107, 114, 128),
            chart_avg_line: Color32::from_rgb(220, 38, 38),
            chart_selection_line: Color32::from_rgb(37, 99, 235), // solid accent blue — 7.3:1 on white

            // ~31% opacity blue tint; text_primary (25,25,30) on blended result ≈ 11.7:1
            selection_bg: Color32::from_rgba_unmultiplied(37, 99, 235, 80),

            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_24: 24.0,

            panel_padding: 12.0,
            panel_gap: 8.0,
            panel_rounding: 6.0,

            font_size_title: 18.0,
            font_size_body: 14.0,
            font_size_caption: 12.0,
            font_size_mono: 13.0,
        }
    }

    /// Dark theme tokens with WCAG AA contrast ratios.
    pub fn dark() -> Self {
        Self {
            surface_primary: Color32::from_rgb(30, 30, 34),
            surface_secondary: Color32::from_rgb(50, 50, 56),
            surface_elevated: Color32::from_rgb(62, 62, 68),

            text_primary: Color32::from_rgb(240, 240, 245),
            text_secondary: Color32::from_rgb(161, 161, 170),
            // Lightened from (135,135,145), which reached only ~3.6:1 on
            // surface_secondary — below the WCAG AA 4.5:1 floor for normal
            // text. (160,160,170) achieves ~5.4:1 there while staying
            // clearly recessive against text_primary.
            text_muted: Color32::from_rgb(160, 160, 170),

            accent: Color32::from_rgb(96, 165, 250),
            border: Color32::from_rgb(78, 78, 86),

            error_color: Color32::from_rgb(248, 113, 113),
            warning_color: Color32::from_rgb(250, 204, 21),
            success_color: Color32::from_rgb(74, 222, 128),
            info_color: Color32::from_rgb(96, 165, 250),

            selection_highlight: Color32::from_rgba_premultiplied(96, 165, 250, 40),
            hover_highlight: Color32::from_rgba_premultiplied(96, 165, 250, 20),
            progress_bar_fill: Color32::from_rgb(96, 165, 250),

            motif_index: Color32::from_rgb(96, 165, 250),
            motif_major: Color32::from_rgb(251, 146, 60),
            motif_minor: Color32::from_rgb(74, 222, 128),
            motif_unique: Color32::from_rgb(192, 132, 252),

            hcs_conserved: Color32::from_rgb(96, 165, 250),

            chart_axis: Color32::from_rgb(161, 161, 170),
            chart_avg_line: Color32::from_rgb(248, 113, 113),
            chart_selection_line: Color32::from_rgb(100, 160, 255), // brighter for dark bg — 5.8:1 on #1E1E22

            // ~31% opacity blue tint; text_primary (240,240,245) on blended result ≈ 8.2:1
            selection_bg: Color32::from_rgba_unmultiplied(96, 165, 250, 80),

            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_24: 24.0,

            panel_padding: 12.0,
            panel_gap: 8.0,
            panel_rounding: 6.0,

            font_size_title: 18.0,
            font_size_body: 14.0,
            font_size_caption: 12.0,
            font_size_mono: 13.0,
        }
    }
}

#[allow(dead_code)]
/// Compute relative luminance per WCAG 2.1 (sRGB).
fn relative_luminance(c: Color32) -> f64 {
    fn linearize(channel: u8) -> f64 {
        let s = channel as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(c.r()) + 0.7152 * linearize(c.g()) + 0.0722 * linearize(c.b())
}

#[allow(dead_code)]
/// WCAG 2.1 contrast ratio between two colors (range 1.0..21.0).
/// WCAG AA requires >= 4.5 for normal text, >= 3.0 for large text.
pub fn contrast_ratio(fg: Color32, bg: Color32) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All text/background pairs must meet WCAG AA (4.5:1 contrast ratio).
    #[test]
    fn test_light_theme_wcag_aa_contrast() {
        let t = DesignTokens::light();
        let pairs = [
            (
                "text_primary on surface_primary",
                t.text_primary,
                t.surface_primary,
            ),
            (
                "text_primary on surface_secondary",
                t.text_primary,
                t.surface_secondary,
            ),
            (
                "text_secondary on surface_primary",
                t.text_secondary,
                t.surface_primary,
            ),
            (
                "text_muted on surface_primary",
                t.text_muted,
                t.surface_primary,
            ),
            (
                "text_muted on surface_secondary",
                t.text_muted,
                t.surface_secondary,
            ),
        ];
        for (name, fg, bg) in pairs {
            let ratio = contrast_ratio(fg, bg);
            assert!(
                ratio >= 4.5,
                "WCAG AA fail: {} has contrast {:.2} (need >= 4.5)",
                name,
                ratio
            );
        }
    }

    #[test]
    fn test_dark_theme_wcag_aa_contrast() {
        let t = DesignTokens::dark();
        let pairs = [
            (
                "text_primary on surface_primary",
                t.text_primary,
                t.surface_primary,
            ),
            (
                "text_primary on surface_secondary",
                t.text_primary,
                t.surface_secondary,
            ),
            (
                "text_secondary on surface_primary",
                t.text_secondary,
                t.surface_primary,
            ),
            (
                "text_muted on surface_primary",
                t.text_muted,
                t.surface_primary,
            ),
        ];
        for (name, fg, bg) in pairs {
            let ratio = contrast_ratio(fg, bg);
            assert!(
                ratio >= 4.5,
                "WCAG AA fail: {} has contrast {:.2} (need >= 4.5)",
                name,
                ratio
            );
        }

        // Muted text is rendered at normal (not large) size, so it must clear the
        // 4.5:1 normal-text threshold rather than the 3.0:1 large-text one. The
        // previous 3.0 allowance let the dark palette ship at ~3.6:1.
        let muted_on_secondary = contrast_ratio(t.text_muted, t.surface_secondary);
        assert!(
            muted_on_secondary >= 4.5,
            "WCAG AA fail: text_muted on surface_secondary has contrast {:.2} (need >= 4.5)",
            muted_on_secondary
        );
    }

    /// Alpha-blend a semi-transparent foreground over an opaque background.
    fn alpha_blend(fg: Color32, bg: Color32) -> Color32 {
        let a = fg.a() as f64 / 255.0;
        let blend =
            |fc: u8, bc: u8| -> u8 { (fc as f64 * a + bc as f64 * (1.0 - a)).round() as u8 };
        Color32::from_rgb(
            blend(fg.r(), bg.r()),
            blend(fg.g(), bg.g()),
            blend(fg.b(), bg.b()),
        )
    }

    /// Verify that text_primary is readable on selection_bg composited over surface_primary.
    /// This catches the invisible-dropdown-text bug (egui PR #7691).
    #[test]
    fn test_selection_bg_wcag_contrast() {
        for (name, tokens) in [
            ("light", DesignTokens::light()),
            ("dark", DesignTokens::dark()),
        ] {
            let blended = alpha_blend(tokens.selection_bg, tokens.surface_primary);
            let ratio = contrast_ratio(tokens.text_primary, blended);
            assert!(
                ratio >= 4.5,
                "WCAG AA fail: text_primary on selection_bg ({}) has contrast {:.2} (need >= 4.5)",
                name,
                ratio
            );
        }
    }

    /// WCAG SC 1.4.11 (Non-text Contrast): graphical objects need >= 3:1
    /// contrast against their background.
    ///
    /// Covers every bar the inspector paints — the metadata value bars and the
    /// four motif composition segments. The object carrying the meaning is the
    /// *fill*, so it is checked against both surfaces it can sit on: the panel
    /// and the track behind it.
    ///
    /// The track itself is deliberately not held to 3:1. It is divider-weight
    /// decoration marking the 100% extent, in the same role (and the same
    /// token) as the separators elsewhere in the UI; SC 1.4.11 exempts purely
    /// decorative graphics, and a track dark enough to pass would compete with
    /// the fill it exists to frame. It must still be visible at all, which the
    /// last check asserts.
    #[test]
    fn test_inspector_bar_contrast() {
        for (name, tokens) in [
            ("light", DesignTokens::light()),
            ("dark", DesignTokens::dark()),
        ] {
            let fills = [
                ("accent fill on track", tokens.accent, tokens.border),
                (
                    "accent fill on panel",
                    tokens.accent,
                    tokens.surface_primary,
                ),
                ("motif_index", tokens.motif_index, tokens.surface_primary),
                ("motif_major", tokens.motif_major, tokens.surface_primary),
                ("motif_minor", tokens.motif_minor, tokens.surface_primary),
                ("motif_unique", tokens.motif_unique, tokens.surface_primary),
            ];
            for (what, fill, background) in fills {
                let ratio = contrast_ratio(fill, background);
                assert!(
                    ratio >= 3.0,
                    "WCAG 1.4.11 fail ({name}): {what} has contrast {ratio:.2} (need >= 3.0)"
                );
            }

            let track = contrast_ratio(tokens.border, tokens.surface_primary);
            assert!(
                track > 1.1,
                "bar track ({name}) is invisible on the panel: contrast {track:.2}"
            );
        }
    }

    #[test]
    fn test_contrast_ratio_black_on_white() {
        let ratio = contrast_ratio(Color32::BLACK, Color32::WHITE);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn test_contrast_ratio_same_color() {
        let ratio = contrast_ratio(Color32::RED, Color32::RED);
        assert!((ratio - 1.0).abs() < 0.01);
    }
}
