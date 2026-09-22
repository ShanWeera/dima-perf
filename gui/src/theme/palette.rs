//! Light and dark palette definitions.
//!
//! Maps design tokens to egui's style system. The `apply_theme()` function
//! modifies egui's `Style` and `Visuals` to match our design tokens.

use egui::{style::WidgetVisuals, CornerRadius, Stroke, Style, Visuals};

use super::tokens::DesignTokens;

/// Theme preference enum, persisted across sessions.
/// Light is the default: positive display polarity (dark text on light background)
/// yields better reading performance in data analysis contexts
/// (Piepenbrock et al. 2013, Ergonomics; Buchner & Baumgartner 2007, Ergonomics).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

/// Build a complete egui Style from our design tokens.
/// Separated from `apply_theme` so both light and dark styles can be
/// built independently for egui's dual-style Options system.
fn build_style(tokens: &DesignTokens, theme: Theme) -> Style {
    let mut style = Style::default();
    let is_dark = matches!(theme, Theme::Dark);

    style.visuals = if is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    // Override egui defaults with our design tokens
    style.visuals.panel_fill = tokens.surface_primary;
    style.visuals.window_fill = tokens.surface_elevated;
    style.visuals.extreme_bg_color = tokens.surface_secondary;
    style.visuals.faint_bg_color = tokens.surface_secondary;

    // Semi-transparent accent tint for selection background, with text_primary
    // for the foreground stroke. This ensures selected text remains readable —
    // egui PR #7691 changed selection.stroke.color to control the TEXT color
    // of selected items (ComboBox, selectable_value).
    style.visuals.selection.bg_fill = tokens.selection_bg;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, tokens.text_primary);

    style.visuals.hyperlink_color = tokens.accent;
    style.visuals.warn_fg_color = tokens.warning_color;
    style.visuals.error_fg_color = tokens.error_color;

    // Widget styling (egui 0.34 renamed Rounding -> CornerRadius, now takes u8)
    let corner_radius = CornerRadius::same(tokens.panel_rounding as u8);

    // All widget states use expansion: 0.0 and bg_stroke.width: 1.0
    // to prevent Cumulative Layout Shift (CLS) on hover/active transitions.
    // A non-zero expansion causes widgets to grow, shifting adjacent elements.
    style.visuals.widgets.noninteractive = WidgetVisuals {
        bg_fill: tokens.surface_secondary,
        weak_bg_fill: tokens.surface_secondary,
        bg_stroke: Stroke::new(1.0_f32, tokens.border),
        corner_radius,
        fg_stroke: Stroke::new(1.0_f32, tokens.text_primary),
        expansion: 0.0,
    };

    style.visuals.widgets.inactive = WidgetVisuals {
        bg_fill: tokens.surface_secondary,
        weak_bg_fill: tokens.surface_secondary,
        bg_stroke: Stroke::new(1.0_f32, tokens.border),
        corner_radius,
        fg_stroke: Stroke::new(1.0_f32, tokens.text_secondary),
        expansion: 0.0,
    };

    style.visuals.widgets.hovered = WidgetVisuals {
        bg_fill: tokens.hover_highlight,
        weak_bg_fill: tokens.hover_highlight,
        bg_stroke: Stroke::new(1.0_f32, tokens.accent),
        corner_radius,
        fg_stroke: Stroke::new(1.5_f32, tokens.text_primary),
        expansion: 0.0,
    };

    style.visuals.widgets.active = WidgetVisuals {
        bg_fill: tokens.selection_highlight,
        weak_bg_fill: tokens.selection_highlight,
        bg_stroke: Stroke::new(1.0_f32, tokens.accent),
        corner_radius,
        fg_stroke: Stroke::new(2.0_f32, tokens.text_primary),
        expansion: 0.0,
    };

    // Spacing
    style.spacing.item_spacing = egui::vec2(tokens.panel_gap, tokens.panel_gap);
    style.spacing.window_margin = egui::Margin::same(tokens.panel_padding as i8);

    style
}

/// Install our styles into **both** theme slots.
///
/// egui keeps one `Style` per theme and a separate `ThemePreference` that
/// selects between them. Populating both slots up front means the active theme
/// — whichever it turns out to be — already uses our design tokens.
///
/// Deliberately does **not** call `ctx.set_theme`. The preference defaults to
/// `ThemePreference::System` and is persisted across sessions by eframe, so
/// forcing a concrete theme here would both ignore the user's OS setting and
/// discard their saved choice on every launch. The app instead *observes*
/// `ctx.theme()` each frame and syncs its tokens to match.
///
/// Must be called once during `DimaApp::new()`.
///
/// Reference: egui 0.34 changelog, PR #4744 (emilk/egui).
pub fn init_both_theme_styles(ctx: &egui::Context) {
    let light_style = build_style(&DesignTokens::light(), Theme::Light);
    let dark_style = build_style(&DesignTokens::dark(), Theme::Dark);

    ctx.set_style_of(egui::Theme::Light, light_style);
    ctx.set_style_of(egui::Theme::Dark, dark_style);
}
