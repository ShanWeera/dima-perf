//! Notifications in the bottom-right corner, newest nearest the corner, at most four with a
//! "+N more" line above them; severity is written out, not only coloured; errors offer "Copy
//! details". The stack is laid out top down and placed by its height, so it always ends 16 px
//! above the window's bottom edge: a message that wraps onto more lines pushes the stack up
//! instead of running off the window. Each message is left-aligned and wraps beside its
//! buttons, which sit at the toast's top right; screen readers read the message first.

use egui::RichText;

use super::common::{CLOSE, row_with_trailing, with_name};
use crate::app::msg::Msg;
use crate::state::toasts::{MAX_VISIBLE, Severity, Toast, Toasts};
use crate::theme::DesignTokens;

/// The widest a toast's contents get.
const MAX_WIDTH: f32 = 380.0;

/// Draws the notifications.
pub fn show(ctx: &egui::Context, toasts: &Toasts, t: &DesignTokens, out: &mut Vec<Msg>) {
    if toasts.is_empty() {
        return;
    }
    let items = toasts.items();
    let hidden = items.len().saturating_sub(MAX_VISIBLE);
    let shown = egui::Area::new(egui::Id::new("dima_toasts"))
        .anchor(
            egui::Align2::RIGHT_BOTTOM,
            egui::vec2(-t.space_16, -t.space_16),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                if hidden > 0 {
                    ui.weak(format!("+{hidden} more"));
                }
                // Oldest first, so the newest is nearest the corner.
                for (n, toast) in items[hidden..].iter().enumerate() {
                    if n > 0 {
                        ui.add_space(t.space_4);
                    }
                    toast_frame(ui, toast, t, out);
                }
            });
            ui.min_rect()
        });
    // The area is placed by its size in the frame before, so a stack whose size changed (a
    // toast came or went) was drawn out of place: draw it again at once, where it belongs.
    if shown.inner.min.distance(shown.response.rect.min) > 0.5 {
        ctx.request_repaint();
    }
}

/// One toast: its severity and message, with "Copy details" and the dismiss button when it has
/// them.
fn toast_frame(ui: &mut egui::Ui, toast: &Toast, t: &DesignTokens, out: &mut Vec<Msg>) {
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
            ui.set_max_width(MAX_WIDTH);
            let mut text = format!("{}: {}", toast.severity.label(), toast.text);
            if toast.count > 1 {
                text.push_str(&format!(" (\u{00D7}{})", toast.count));
            }
            let message = |ui: &mut egui::Ui| {
                ui.add(egui::Label::new(RichText::new(text).color(t.text_primary)).wrap());
            };
            let dismiss = !toast.auto_dismiss();
            if toast.details.is_none() && !dismiss {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), message);
                return;
            }
            row_with_trailing(ui, ui.spacing().item_spacing.x, message, |ui| {
                if dismiss && with_name(ui.small_button(CLOSE), "Dismiss").clicked() {
                    out.push(Msg::DismissToast(toast.id));
                }
                if let Some(d) = &toast.details
                    && ui.small_button("Copy details").clicked()
                {
                    ui.ctx().copy_text(d.clone());
                }
            });
        });
}
