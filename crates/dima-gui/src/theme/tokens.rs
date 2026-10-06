//! Design tokens: every colour, spacing and type size the interface uses, named by function.
//!
//! Data colours come from the Okabe–Ito palette, which stays distinguishable under the common
//! colour-vision deficiencies (Wong 2011). They are the same in both themes, so a figure looks
//! the same on screen and in an export, and each passes the 3:1 non-text contrast of WCAG 2.2
//! SC 1.4.11 against the chart background of both themes; colour is never the only cue
//! (SC 1.4.1: uncorrected values are also dashed, no-support spans are also hatched and
//! labelled). Text colours pass 4.5:1 (SC 1.4.3) on every surface and on the selected-row tint
//! (over plain and striped rows), where tables keep explicitly coloured text. The tests below
//! check these pairs; data colours are drawn only on the chart background, never as text.

use egui::Color32;

/// Okabe–Ito blue: entropy at eligible positions (support > T), corrected (D87).
pub const OKABE_BLUE: Color32 = Color32::from_rgb(0x00, 0x72, 0xB2);
/// Okabe–Ito vermillion: uncorrected entropy at LS/ELS positions (support ≤ T).
pub const OKABE_VERMILLION: Color32 = Color32::from_rgb(0xD5, 0x5E, 0x00);
/// Okabe–Ito bluish green.
pub const OKABE_GREEN: Color32 = Color32::from_rgb(0x00, 0x9E, 0x73);
/// Okabe–Ito reddish purple.
pub const OKABE_PURPLE: Color32 = Color32::from_rgb(0xCC, 0x79, 0xA7);

/// Every colour and size of the interface.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignTokens {
    /// `true` for the dark theme.
    pub dark: bool,
    // Surfaces.
    pub surface_primary: Color32,
    pub surface_secondary: Color32,
    pub surface_elevated: Color32,
    // Text.
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    // Interface accent (interactive elements); distinct from the data colours.
    pub accent: Color32,
    pub border: Color32,
    // Status.
    pub error_color: Color32,
    pub warning_color: Color32,
    pub success_color: Color32,
    pub info_color: Color32,
    // Interaction tints (unmultiplied alpha).
    pub selection_highlight: Color32,
    pub hover_highlight: Color32,
    pub selection_bg: Color32,
    // Entropy chart (D87).
    pub series_eligible: Color32,
    pub series_uncorrected: Color32,
    pub series_fill: Color32,
    pub ns_band_fill: Color32,
    pub ns_hatch: Color32,
    pub mean_line: Color32,
    pub coverage_mark: Color32,
    pub chart_axis: Color32,
    pub chart_selection_line: Color32,
    // Conserved regions (D86).
    pub hcs_region: Color32,
    pub hcs_flag_hatch: Color32,
    // Diversity motifs.
    pub motif_index: Color32,
    pub motif_major: Color32,
    pub motif_minor: Color32,
    pub motif_unique: Color32,
    // Spacing (8-point scale) and shape.
    pub space_4: f32,
    pub space_8: f32,
    pub space_12: f32,
    pub space_16: f32,
    pub space_24: f32,
    pub panel_padding: f32,
    pub panel_gap: f32,
    pub panel_rounding: f32,
    // Type sizes.
    pub font_size_title: f32,
    pub font_size_heading: f32,
    pub font_size_body: f32,
    pub font_size_caption: f32,
    pub font_size_mono: f32,
}

impl DesignTokens {
    /// Tokens for the given egui theme.
    #[must_use]
    pub fn for_theme(theme: egui::Theme) -> Self {
        match theme {
            egui::Theme::Dark => Self::dark(),
            egui::Theme::Light => Self::light(),
        }
    }

    /// Light theme.
    #[must_use]
    pub fn light() -> Self {
        let accent = Color32::from_rgb(0x3B, 0x5B, 0xDB);
        Self {
            dark: false,
            surface_primary: Color32::from_rgb(255, 255, 255),
            surface_secondary: Color32::from_rgb(247, 248, 252),
            surface_elevated: Color32::from_rgb(255, 255, 255),
            text_primary: Color32::from_rgb(25, 25, 30),
            text_secondary: Color32::from_rgb(90, 95, 105),
            text_muted: Color32::from_rgb(105, 110, 120),
            accent,
            border: Color32::from_rgb(218, 222, 228),
            error_color: Color32::from_rgb(192, 28, 28),
            warning_color: Color32::from_rgb(146, 64, 14),
            success_color: Color32::from_rgb(21, 128, 61),
            info_color: accent,
            selection_highlight: Color32::from_rgba_unmultiplied(0x3B, 0x5B, 0xDB, 40),
            hover_highlight: Color32::from_rgba_unmultiplied(0x3B, 0x5B, 0xDB, 22),
            selection_bg: Color32::from_rgba_unmultiplied(0x3B, 0x5B, 0xDB, 70),
            series_eligible: OKABE_BLUE,
            series_uncorrected: OKABE_VERMILLION,
            series_fill: Color32::from_rgba_unmultiplied(0x00, 0x72, 0xB2, 26),
            ns_band_fill: Color32::from_rgba_unmultiplied(105, 110, 120, 28),
            ns_hatch: Color32::from_rgb(105, 110, 120),
            mean_line: Color32::from_rgb(70, 74, 82),
            coverage_mark: OKABE_PURPLE,
            chart_axis: Color32::from_rgb(90, 95, 105),
            chart_selection_line: Color32::from_rgb(0, 0, 0),
            hcs_region: OKABE_GREEN,
            hcs_flag_hatch: Color32::from_rgb(25, 25, 30),
            motif_index: OKABE_BLUE,
            motif_major: OKABE_VERMILLION,
            motif_minor: OKABE_GREEN,
            motif_unique: OKABE_PURPLE,
            ..Self::shared()
        }
    }

    /// Dark theme.
    #[must_use]
    pub fn dark() -> Self {
        let accent = Color32::from_rgb(0x9A, 0xAC, 0xF9);
        Self {
            dark: true,
            surface_primary: Color32::from_rgb(30, 30, 34),
            surface_secondary: Color32::from_rgb(50, 50, 56),
            surface_elevated: Color32::from_rgb(62, 62, 68),
            text_primary: Color32::from_rgb(240, 240, 245),
            text_secondary: Color32::from_rgb(188, 188, 198),
            text_muted: Color32::from_rgb(170, 170, 180),
            accent,
            border: Color32::from_rgb(78, 78, 86),
            error_color: Color32::from_rgb(252, 140, 140),
            warning_color: Color32::from_rgb(250, 204, 21),
            success_color: Color32::from_rgb(74, 222, 128),
            info_color: accent,
            selection_highlight: Color32::from_rgba_unmultiplied(0x9A, 0xAC, 0xF9, 46),
            hover_highlight: Color32::from_rgba_unmultiplied(0x9A, 0xAC, 0xF9, 26),
            selection_bg: Color32::from_rgba_unmultiplied(0x9A, 0xAC, 0xF9, 70),
            series_eligible: OKABE_BLUE,
            series_uncorrected: OKABE_VERMILLION,
            series_fill: Color32::from_rgba_unmultiplied(0x00, 0x72, 0xB2, 40),
            ns_band_fill: Color32::from_rgba_unmultiplied(160, 160, 170, 30),
            ns_hatch: Color32::from_rgb(150, 150, 160),
            mean_line: Color32::from_rgb(200, 200, 208),
            coverage_mark: OKABE_PURPLE,
            chart_axis: Color32::from_rgb(170, 170, 180),
            chart_selection_line: Color32::from_rgb(255, 255, 255),
            hcs_region: OKABE_GREEN,
            hcs_flag_hatch: Color32::from_rgb(240, 240, 245),
            motif_index: OKABE_BLUE,
            motif_major: OKABE_VERMILLION,
            motif_minor: OKABE_GREEN,
            motif_unique: OKABE_PURPLE,
            ..Self::shared()
        }
    }

    /// Theme-independent values (light colours are placeholders, always overridden).
    fn shared() -> Self {
        let c = Color32::PLACEHOLDER;
        Self {
            dark: false,
            surface_primary: c,
            surface_secondary: c,
            surface_elevated: c,
            text_primary: c,
            text_secondary: c,
            text_muted: c,
            accent: c,
            border: c,
            error_color: c,
            warning_color: c,
            success_color: c,
            info_color: c,
            selection_highlight: c,
            hover_highlight: c,
            selection_bg: c,
            series_eligible: c,
            series_uncorrected: c,
            series_fill: c,
            ns_band_fill: c,
            ns_hatch: c,
            mean_line: c,
            coverage_mark: c,
            chart_axis: c,
            chart_selection_line: c,
            hcs_region: c,
            hcs_flag_hatch: c,
            motif_index: c,
            motif_major: c,
            motif_minor: c,
            motif_unique: c,
            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_24: 24.0,
            panel_padding: 12.0,
            panel_gap: 8.0,
            panel_rounding: 6.0,
            font_size_title: 20.0,
            font_size_heading: 15.0,
            font_size_body: 14.0,
            font_size_caption: 12.0,
            font_size_mono: 13.0,
        }
    }

    /// Every colour token with its name (tests).
    #[cfg(test)]
    #[must_use]
    pub fn colors(&self) -> [(&'static str, Color32); 30] {
        [
            ("surface_primary", self.surface_primary),
            ("surface_secondary", self.surface_secondary),
            ("surface_elevated", self.surface_elevated),
            ("text_primary", self.text_primary),
            ("text_secondary", self.text_secondary),
            ("text_muted", self.text_muted),
            ("accent", self.accent),
            ("border", self.border),
            ("error_color", self.error_color),
            ("warning_color", self.warning_color),
            ("success_color", self.success_color),
            ("info_color", self.info_color),
            ("selection_highlight", self.selection_highlight),
            ("hover_highlight", self.hover_highlight),
            ("selection_bg", self.selection_bg),
            ("series_eligible", self.series_eligible),
            ("series_uncorrected", self.series_uncorrected),
            ("series_fill", self.series_fill),
            ("ns_band_fill", self.ns_band_fill),
            ("ns_hatch", self.ns_hatch),
            ("mean_line", self.mean_line),
            ("coverage_mark", self.coverage_mark),
            ("chart_axis", self.chart_axis),
            ("chart_selection_line", self.chart_selection_line),
            ("hcs_region", self.hcs_region),
            ("hcs_flag_hatch", self.hcs_flag_hatch),
            ("motif_index", self.motif_index),
            ("motif_major", self.motif_major),
            ("motif_minor", self.motif_minor),
            ("motif_unique", self.motif_unique),
        ]
    }

    /// The four motif colours in Index, Major, Minor, Unique order.
    #[must_use]
    pub const fn motif_colors(&self) -> [Color32; 4] {
        [
            self.motif_index,
            self.motif_major,
            self.motif_minor,
            self.motif_unique,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance of an sRGB colour (WCAG 2.2, "relative luminance").
    #[must_use]
    pub fn relative_luminance(c: Color32) -> f64 {
        fn linear(channel: u8) -> f64 {
            let s = f64::from(channel) / 255.0;
            if s <= 0.040_45 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    }

    /// WCAG contrast ratio (1–21) between two opaque colours.
    #[must_use]
    pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (relative_luminance(a), relative_luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// `fg` (with unmultiplied alpha) composited over the opaque `bg`.
    #[must_use]
    pub fn composite(fg: Color32, bg: Color32) -> Color32 {
        let [r, g, b, a] = fg.to_srgba_unmultiplied();
        let alpha = f64::from(a) / 255.0;
        let mix = |f: u8, b: u8| {
            let v = f64::from(f) * alpha + f64::from(b) * (1.0 - alpha);
            // In 0..=255 by construction.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = v.round() as u8;
            v
        };
        Color32::from_rgb(mix(r, bg.r()), mix(g, bg.g()), mix(b, bg.b()))
    }

    fn themes() -> [(&'static str, DesignTokens); 2] {
        [
            ("light", DesignTokens::light()),
            ("dark", DesignTokens::dark()),
        ]
    }

    #[test]
    fn contrast_reference_values() {
        assert!((contrast_ratio(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Color32::RED, Color32::RED) - 1.0).abs() < 0.001);
        // The reason orange is not used (D87): Okabe–Ito orange is 2.2:1 on white.
        let orange = Color32::from_rgb(0xE6, 0x9F, 0x00);
        assert!(contrast_ratio(orange, Color32::WHITE) < 3.0);
    }

    /// WCAG 2.2 SC 1.4.3: text at 4.5:1 on every surface it is drawn on.
    #[test]
    fn text_contrast_in_both_themes() {
        for (name, t) in themes() {
            for (what, fg) in [
                ("text_primary", t.text_primary),
                ("text_secondary", t.text_secondary),
                ("text_muted", t.text_muted),
                ("accent", t.accent),
                ("error", t.error_color),
                ("warning", t.warning_color),
                ("success", t.success_color),
            ] {
                for (surface, bg) in [
                    ("primary", t.surface_primary),
                    ("secondary", t.surface_secondary),
                    ("elevated", t.surface_elevated),
                ] {
                    let r = contrast_ratio(fg, bg);
                    assert!(r >= 4.5, "{name}: {what} on {surface} is {r:.2}:1");
                }
            }
            // Selected (or chart-hovered) table rows, plain and striped: egui keeps explicit
            // text colours there, and draws other text in the selection stroke (primary text).
            for (row, bg) in [
                ("plain", t.surface_primary),
                ("striped", t.surface_secondary),
            ] {
                let selected = composite(t.selection_bg, bg);
                for (what, fg) in [
                    ("text_primary", t.text_primary),
                    ("warning", t.warning_color),
                ] {
                    let r = contrast_ratio(fg, selected);
                    assert!(
                        r >= 4.5,
                        "{name}: {what} on a selected {row} row is {r:.2}:1"
                    );
                }
            }
        }
    }

    /// WCAG 2.2 SC 1.4.11 and D87: every data mark at 3:1 against the chart background.
    #[test]
    fn chart_and_data_colours_contrast_in_both_themes() {
        for (name, t) in themes() {
            for (what, c) in [
                ("eligible line", t.series_eligible),
                ("uncorrected line", t.series_uncorrected),
                ("no-support hatch", t.ns_hatch),
                ("eligible mean", t.mean_line),
                ("coverage mark", t.coverage_mark),
                ("axis", t.chart_axis),
                ("selection", t.chart_selection_line),
                ("conserved region", t.hcs_region),
                ("flag hatch", t.hcs_flag_hatch),
                ("Index", t.motif_index),
                ("Major", t.motif_major),
                ("Minor", t.motif_minor),
                ("Unique", t.motif_unique),
                ("accent bar", t.accent),
            ] {
                let r = contrast_ratio(c, t.surface_primary);
                assert!(r >= 3.0, "{name}: {what} is {r:.2}:1");
            }
            // The hatch must also stand out on the band's own tint.
            let band = composite(t.ns_band_fill, t.surface_primary);
            assert!(
                contrast_ratio(t.ns_hatch, band) >= 3.0,
                "{name}: hatch on band"
            );
            // The flag hatch is drawn over the conserved-region fill.
            assert!(
                contrast_ratio(t.hcs_flag_hatch, t.hcs_region) >= 3.0,
                "{name}: flag hatch on region"
            );
            assert!(
                contrast_ratio(t.border, t.surface_primary) > 1.1,
                "{name}: border"
            );
        }
    }

    #[test]
    fn data_colours_are_the_okabe_ito_values_of_d87() {
        for (_, t) in themes() {
            assert_eq!(t.series_eligible, Color32::from_rgb(0x00, 0x72, 0xB2));
            assert_eq!(t.series_uncorrected, Color32::from_rgb(0xD5, 0x5E, 0x00));
        }
    }

    /// The plan's theme rule: HCS, Index and accent are three different colours.
    #[test]
    fn conserved_index_and_accent_are_distinct() {
        for (name, t) in themes() {
            assert_ne!(t.hcs_region, t.motif_index, "{name}");
            assert_ne!(t.hcs_region, t.accent, "{name}");
            assert_ne!(t.motif_index, t.accent, "{name}");
            let motifs = t.motif_colors();
            for i in 0..4 {
                for j in i + 1..4 {
                    assert_ne!(motifs[i], motifs[j], "{name}: motifs {i} and {j}");
                }
            }
        }
    }

    #[test]
    fn every_colour_is_set_in_both_themes() {
        for (name, t) in themes() {
            for (field, c) in t.colors() {
                assert_ne!(c, Color32::PLACEHOLDER, "{name}: {field} was not set");
            }
        }
    }
}
