//! The header-format editor (D71): one row per field with a tag (the paper's six or a custom
//! name), three sample values and a "stratify" switch; "Paste format…" and "Copy as
//! --header-format"; the library's validation shown inline (Nielsen & Molich 1990 [general]:
//! visible system status, error prevention).

use egui::{RichText, Ui};

use super::common::{card, heading};
use super::help_text as help;
use crate::app::msg::Msg;
use crate::state::AppState;
use crate::state::header_editor::PAPER_TAGS;
use crate::theme::DesignTokens;
use crate::util::truncate_display;

/// Local state of the paste box.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PasteBox {
    /// Shown.
    pub open: bool,
    /// Text being pasted.
    pub text: String,
}

/// Draws the editor (only while header stratification is on).
pub fn show(
    ui: &mut Ui,
    state: &mut AppState,
    paste: &mut PasteBox,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    if !state.setup.header.enabled {
        return;
    }
    let fill = state.setup.options.fill.clone();
    card(t).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            heading(ui, t, "Header fields");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Copy as --header-format")
                    .on_hover_text("The equivalent command-line options, for `dima analyze`")
                    .clicked()
                {
                    ui.ctx().copy_text(state.setup.header.cli_args());
                    out.push(Msg::Notify(
                        crate::state::toasts::Severity::Info,
                        "Header format copied".to_owned(),
                    ));
                }
                if ui.button("Paste format\u{2026}").clicked() {
                    paste.open = !paste.open;
                }
            });
        });
        ui.label(
            RichText::new(
                "Name each '|'-separated field of the headers. Variants are counted per value of \
                 each stratified field.",
            )
            .small()
            .color(t.text_secondary),
        )
        .on_hover_text(help::header_format());
        if paste.open {
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut paste.text)
                        .hint_text("Accession|Host|Country|Year")
                        .desired_width(ui.available_width() - 80.0),
                );
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Apply").clicked() || enter) && !paste.text.trim().is_empty() {
                    out.push(Msg::HeaderPasted(std::mem::take(&mut paste.text)));
                    paste.open = false;
                }
            });
        }
        if let Some(note) = &state.setup.header_note {
            ui.colored_label(t.warning_color, note);
        }
        let h = &mut state.setup.header;
        if h.mismatched_headers > 0 {
            ui.colored_label(
                t.warning_color,
                format!(
                    "{} of the checked headers have a different number of fields than the first.",
                    h.mismatched_headers
                ),
            );
        }
        ui.add_space(t.space_4);
        egui::Grid::new("header_rows")
            .num_columns(4)
            .striped(true)
            .spacing([t.space_12, t.space_4])
            .show(ui, |ui| {
                ui.label(RichText::new("#").strong());
                ui.label(RichText::new("Field").strong());
                ui.label(RichText::new("Stratify").strong());
                ui.label(RichText::new("Sample values").strong());
                ui.end_row();
                for (i, row) in h.rows.iter_mut().enumerate() {
                    ui.label((i + 1).to_string());
                    ui.horizontal(|ui| {
                        let selected = row.tag().unwrap_or("Custom\u{2026}");
                        egui::ComboBox::from_id_salt(("header_tag", i))
                            .width(110.0)
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for tag in PAPER_TAGS {
                                    if ui.selectable_label(row.name == tag, tag).clicked() {
                                        row.name = tag.to_owned();
                                    }
                                }
                                if ui
                                    .selectable_label(row.tag().is_none(), "Custom\u{2026}")
                                    .clicked()
                                    && row.tag().is_some()
                                {
                                    row.name = format!("field_{}", i + 1);
                                }
                            });
                        if row.tag().is_none() {
                            ui.add(
                                egui::TextEdit::singleline(&mut row.name)
                                    .desired_width(120.0)
                                    .hint_text("name"),
                            );
                        }
                    });
                    ui.checkbox(&mut row.stratify, "").on_hover_text(if row.identifier_like {
                        "Every checked value is different: stratifying by an identifier multiplies \
                         the size of the results"
                    } else {
                        "Count variants per value of this field"
                    });
                    ui.horizontal(|ui| {
                        if row.samples.is_empty() {
                            ui.weak("\u{2014}");
                        }
                        for s in &row.samples {
                            ui.label(
                                RichText::new(truncate_display(s, 24))
                                    .monospace()
                                    .size(t.font_size_caption),
                            )
                            .on_hover_text(s);
                        }
                        if row.identifier_like {
                            ui.colored_label(t.warning_color, "identifier-like")
                                .on_hover_text("All values in the checked records are different");
                        }
                    });
                    ui.end_row();
                }
            });
        if let Err(e) = h.validate(&fill) {
            ui.add_space(t.space_4);
            ui.colored_label(t.error_color, e.report());
        }
    });
}
