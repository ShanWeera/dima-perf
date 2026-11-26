//! The input section of the setup screen: open button, drop zone, path field (D100) and the
//! pre-check summary (D85): alphabet, alignment length, default k, compression, size and the
//! number of sequences ("~N (estimated)" for large files), shown in well under a second.

use egui::{RichText, Ui};

use dima_core::model::Compression;
use dima_core::validation::Inspection;

use super::common::{card, fact, heading};
use super::error_card;
use crate::app::msg::Msg;
use crate::state::AppState;
use crate::theme::DesignTokens;
use dima_core::human::{bytes, plural};

/// Draws the input section.
pub fn show(ui: &mut Ui, state: &mut AppState, t: &DesignTokens, out: &mut Vec<Msg>) {
    card(t).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        heading(ui, t, "Input");
        ui.add_space(t.space_4);
        let busy_dialog = state.dialog.pending.is_some();
        ui.horizontal(|ui| {
            let open = ui
                .add_enabled(!busy_dialog, egui::Button::new("Open\u{2026}"))
                .on_hover_text(
                    "An aligned FASTA file (optionally gzip, bzip2 or Zstandard compressed), \
                     or a DiMA results file (.json, .json.gz, .json.zst) to explore",
                );
            if open.clicked() {
                out.push(Msg::OpenDialog);
            }
            if state.dialog.pending.is_some() && state.dialog.native {
                super::header_bar::waiting_for_dialog(ui, out);
            }
        });
        ui.add_space(t.space_4);
        ui.horizontal(|ui| {
            let field = super::common::named(
                ui.add(
                    egui::TextEdit::singleline(&mut state.setup.path_text)
                        .hint_text("or paste a path here and press Enter")
                        .desired_width(ui.available_width() - 70.0),
                ),
                "Path of a file to open",
            );
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let any = crate::util::pasted_path(&state.setup.path_text, None).is_some();
            let go = ui.add_enabled(any, egui::Button::new("Open")).clicked();
            if (enter || go)
                && let Some(path) =
                    crate::util::pasted_path(&state.setup.path_text, dirs::home_dir().as_deref())
            {
                out.push(Msg::OpenPath(path));
            }
        });
        if state.setup.file.is_none() && state.setup.opening.is_none() {
            drop_zone(ui, t);
        }
        if let Some((_, path)) = &state.setup.opening {
            ui.add_space(t.space_4);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "Checking {}\u{2026}",
                    dima_core::run_report::file_name(path)
                ));
                if ui.small_button("Cancel").clicked() {
                    out.push(Msg::CancelOpen);
                }
            });
        }
        if let Some(card) = &state.setup.open_error {
            ui.add_space(t.space_8);
            error_card::show(ui, card, t, out, false);
        }
        if let (Some(path), Some(i)) = (&state.setup.file, &state.setup.inspection) {
            ui.add_space(t.space_8);
            summary(ui, path, i, t);
        }
    });
}

fn drop_zone(ui: &mut Ui, t: &DesignTokens) {
    ui.add_space(t.space_8);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::hover());
    let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    let stroke = if hovering { t.accent } else { t.border };
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.5, stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "Drop an alignment (.fasta, .gz, .bz2, .zst) or a results file here",
        egui::FontId::proportional(t.font_size_body),
        t.text_muted,
    );
}

fn compression_name(c: Compression) -> &'static str {
    match c {
        Compression::None => "none",
        Compression::Gzip => "gzip",
        Compression::Bzip2 => "bzip2",
        Compression::Zstd => "Zstandard",
    }
}

/// The pre-check facts.
pub fn summary(ui: &mut Ui, path: &std::path::Path, i: &Inspection, t: &DesignTokens) {
    ui.label(RichText::new(dima_core::run_report::file_name(path)).strong())
        .on_hover_text(path.display().to_string());
    egui::Grid::new("inspection")
        .num_columns(2)
        .spacing([t.space_16, t.space_4])
        .show(ui, |ui| {
            let sequences = match (i.complete, i.estimated_records) {
                (true, _) => plural(i.records_examined, "sequence", "sequences"),
                (false, Some(n)) => format!("~{n} sequences (estimated)"),
                (false, None) => format!("more than {} sequences", i.records_examined),
            };
            fact(ui, t, "Sequences", sequences);
            fact(
                ui,
                t,
                "Alignment length",
                format!("{} columns", i.alignment_length),
            );
            let alphabet = i.alphabet.map_or_else(
                || "undetermined \u{2014} choose one below".to_owned(),
                |a| {
                    format!(
                        "{} (detected from {} residues)",
                        a.display_name(),
                        i.residues_examined
                    )
                },
            );
            fact(ui, t, "Alphabet", alphabet);
            if let Some(a) = i.alphabet {
                fact(ui, t, "Default k", a.default_kmer_length().to_string());
            }
            let size = i.size_bytes.map_or_else(|| "unknown".to_owned(), bytes);
            fact(
                ui,
                t,
                "File",
                format!(
                    "{size}{}compression: {}",
                    dima_core::human::separators::ITEM,
                    compression_name(i.compression)
                ),
            );
            let fields = i.header_fields.len();
            let mut header = format!("{} '|'-separated", plural(fields as u64, "field", "fields"));
            if i.header_field_count_mismatches > 0 {
                header.push_str(&format!(
                    "; {} of the first {} headers differ",
                    i.header_field_count_mismatches, i.records_examined
                ));
            }
            fact(ui, t, "Headers", header);
        });
    if i.alphabet.is_none() {
        ui.colored_label(
            t.warning_color,
            "Too few residues to tell protein from nucleotide: choose the alphabet.",
        );
    }
    if !i.complete {
        ui.label(
            RichText::new(format!(
                "Checked the first {} records; the analysis reads the whole file and reports any \
                 problem with its exact location.",
                i.records_examined
            ))
            .small()
            .color(t.text_muted),
        );
    }
}
