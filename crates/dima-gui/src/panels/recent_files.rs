//! Recent files (at most ten): click to open again (with a confirmation while an analysis
//! runs, G35), remove one, or clear the list. A file that no longer exists is reported when
//! opened and removed from the list.

use egui::{RichText, Ui};

use super::common::{card, heading};
use crate::app::msg::Msg;
use crate::state::recent::{RecentFiles, RecentKind, now_unix};
use crate::theme::DesignTokens;
use crate::util::format_relative_time;

/// Draws the list (nothing when empty).
pub fn show(ui: &mut Ui, recent: &RecentFiles, t: &DesignTokens, out: &mut Vec<Msg>) {
    if recent.files.is_empty() {
        return;
    }
    let now = now_unix();
    card(t).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            heading(ui, t, "Recent files");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Clear all")
                    .on_hover_text("Forget every recent file")
                    .clicked()
                {
                    out.push(Msg::RecentCleared);
                }
            });
        });
        for f in &recent.files {
            let name = f.path.file_name().map_or_else(
                || f.path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            let mut detail = Vec::new();
            match (f.kind, f.sequences) {
                (RecentKind::Results, _) => detail.push("results".to_owned()),
                (RecentKind::Alignment, Some(n)) if f.estimated => {
                    detail.push(format!("~{n} sequences"))
                }
                (RecentKind::Alignment, Some(n)) => {
                    detail.push(dima_core::human::plural(n, "sequence", "sequences"))
                }
                (RecentKind::Alignment, None) => {}
            }
            if let Some(a) = &f.alphabet {
                detail.push(a.clone());
            }
            let when = format_relative_time(now, f.opened);
            if !when.is_empty() {
                detail.push(when);
            }
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if super::common::with_name(
                        ui.small_button(super::common::CLOSE),
                        "Remove from recent files",
                    )
                    .clicked()
                    {
                        out.push(Msg::RecentRemoved(f.path.clone()));
                    }
                    ui.weak(detail.join(dima_core::human::separators::ITEM));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let link = ui
                            .add(
                                egui::Label::new(RichText::new(&name).color(t.accent))
                                    .sense(egui::Sense::click())
                                    .truncate(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(f.path.display().to_string());
                        if link.clicked() {
                            out.push(Msg::OpenPath(f.path.clone()));
                        }
                    });
                });
            });
        }
    });
}
