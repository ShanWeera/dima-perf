//! Maps the design tokens onto egui's style, for both themes.

use egui::{CornerRadius, Stroke, Style, Visuals, style::WidgetVisuals};

use super::tokens::DesignTokens;

/// The egui style for a token set.
#[must_use]
pub fn build_style(tokens: &DesignTokens) -> Style {
    let mut style = Style {
        visuals: if tokens.dark {
            Visuals::dark()
        } else {
            Visuals::light()
        },
        ..Style::default()
    };
    let v = &mut style.visuals;
    v.panel_fill = tokens.surface_primary;
    v.window_fill = tokens.surface_elevated;
    v.extreme_bg_color = tokens.surface_secondary;
    v.faint_bg_color = tokens.surface_secondary;
    // Since egui 0.33 the selection stroke colours the *text* of selected items, so it must be
    // the primary text colour on a translucent tint, never the accent itself.
    v.selection.bg_fill = tokens.selection_bg;
    v.selection.stroke = Stroke::new(1.0, tokens.text_primary);
    v.hyperlink_color = tokens.accent;
    v.warn_fg_color = tokens.warning_color;
    v.error_fg_color = tokens.error_color;

    // Widgets never grow on hover or press (expansion 0), so nothing around them shifts.
    let corner_radius = CornerRadius::same(6);
    let widget = |bg, stroke_color, fg, fg_width| WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: bg,
        bg_stroke: Stroke::new(1.0, stroke_color),
        corner_radius,
        fg_stroke: Stroke::new(fg_width, fg),
        expansion: 0.0,
    };
    v.widgets.noninteractive = widget(
        tokens.surface_secondary,
        tokens.border,
        tokens.text_primary,
        1.0,
    );
    v.widgets.inactive = widget(
        tokens.surface_secondary,
        tokens.border,
        tokens.text_secondary,
        1.0,
    );
    v.widgets.hovered = widget(
        tokens.hover_highlight,
        tokens.accent,
        tokens.text_primary,
        1.5,
    );
    v.widgets.active = widget(
        tokens.selection_highlight,
        tokens.accent,
        tokens.text_primary,
        2.0,
    );
    v.widgets.open = widget(
        tokens.surface_secondary,
        tokens.accent,
        tokens.text_primary,
        1.0,
    );

    style.spacing.item_spacing = egui::vec2(tokens.panel_gap, tokens.panel_gap);
    // Panel padding is 12 px by design; the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    let margin = tokens.panel_padding as i8;
    style.spacing.window_margin = egui::Margin::same(margin);
    style
}

/// Installs the styles of both themes. The theme preference itself (system, light or dark) is
/// egui's and is persisted with egui's memory; the app reads `ctx.theme()` every frame.
pub fn install(ctx: &egui::Context) {
    ctx.set_style_of(egui::Theme::Light, build_style(&DesignTokens::light()));
    ctx.set_style_of(egui::Theme::Dark, build_style(&DesignTokens::dark()));
    // Ctrl/Cmd+scroll reaches the chart (zoom) instead of scaling the whole interface; keyboard
    // zoom (Ctrl/Cmd +/-/0) and pinch are unaffected.
    ctx.options_mut(|o| o.input_options.zoom_modifier = egui::Modifiers::NONE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_follow_the_tokens() {
        for t in [DesignTokens::light(), DesignTokens::dark()] {
            let s = build_style(&t);
            assert_eq!(s.visuals.panel_fill, t.surface_primary);
            assert_eq!(s.visuals.selection.stroke.color, t.text_primary);
            assert_eq!(s.visuals.dark_mode, t.dark);
            assert!(s.visuals.widgets.hovered.expansion.abs() < f32::EPSILON);
        }
    }
}
