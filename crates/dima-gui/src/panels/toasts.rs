//! Notifications in the bottom-right corner, newest nearest the corner, at most four with a
//! "+N more" line; severity is written out, not only coloured; errors offer "Copy details".

use egui::RichText;

use crate::app::msg::Msg;
use crate::state::toasts::{MAX_VISIBLE, Severity, Toasts};
use crate::theme::DesignTokens;

/// Draws the notifications.
pub fn show(ctx: &egui::Context, toasts: &Toasts, t: &DesignTokens, out: &mut Vec<Msg>) {
    if toasts.is_empty() {
        return;
    }
    let items = toasts.items();
    let hidden = items.len().saturating_sub(MAX_VISIBLE);
    egui::Area::new(egui::Id::new("dima_toasts"))
        .anchor(
            egui::Align2::RIGHT_BOTTOM,
            egui::vec2(-t.space_16, -t.space_16),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                for toast in items.iter().rev().take(MAX_VISIBLE) {
                    let accent = match toast.severity {
                        Severity::Error => t.error_color,
                        Severity::Warning => t.warning_color,
                        Severity::Success => t.success_color,
                        Severity::Info => t.info_color,
                    };
                    egui::Frame::new()
                        .fill(t.surface_elevated)
                        .stroke(egui::Stroke::new(1.0, accent))
                        .corner_radius(t.panel_rounding)
                        .inner_margin(t.space_8)
                        .show(ui, |ui| {
                            ui.set_max_width(380.0);
                            ui.horizontal(|ui| {
                                let mut text =
                                    format!("{}: {}", toast.severity.label(), toast.text);
                                if toast.count > 1 {
                                    text.push_str(&format!(" (\u{00D7}{})", toast.count));
                                }
                                ui.add(
                                    egui::Label::new(RichText::new(text).color(t.text_primary))
                                        .wrap(),
                                );
                                if let Some(d) = &toast.details
                                    && ui.small_button("Copy details").clicked()
                                {
                                    ui.ctx().copy_text(d.clone());
                                }
                                if !toast.auto_dismiss()
                                    && ui
                                        .small_button("\u{2715}")
                                        .on_hover_text("Dismiss")
                                        .clicked()
                                {
                                    out.push(Msg::DismissToast(toast.id));
                                }
                            });
                        });
                    ui.add_space(t.space_4);
                }
                if hidden > 0 {
                    ui.weak(format!("+{hidden} more"));
                }
            });
        });
}
