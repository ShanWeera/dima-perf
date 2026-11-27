//! The input section of the setup screen: open button, drop zone, path field (D100) and the
//! pre-check summary (D85): alphabet, alignment length, default k, compression, size and the
//! number of sequences ("~N (estimated)" for large files), shown in well under a second.

use egui::{RichText, Ui};

use dima_core::model::Compression;
use dima_core::validation::Inspection;

use super::common::Described;
use super::common::{card, fact, heading};
use super::error_card;
use crate::app::msg::Msg;
use crate::state::AppState;
use crate::state::options::{AlphabetChoice, undetected_alphabet};
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
                .described(
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
            summary(ui, path, i, state.setup.options.alphabet, t);
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

/// The "Alphabet" fact: what Auto detected and from how many letters, or that it could not,
/// with the choice that replaces it.
fn alphabet_fact(i: &Inspection, choice: AlphabetChoice) -> String {
    match (i.alphabet, choice) {
        (Some(a), _) => format!(
            "{} (detected from {} residues)",
            a.display_name(),
            i.residues_examined
        ),
        (None, AlphabetChoice::Auto) => "undetermined \u{2014} choose one below".to_owned(),
        (None, chosen) => format!("undetermined ({} chosen below)", chosen.label()),
    }
}

/// Why the alphabet must be chosen, while Auto is chosen and could not detect it: the
/// library's reason (too few letters, both T and U, or an undecided composition).
fn alphabet_warning(i: &Inspection, choice: AlphabetChoice) -> Option<String> {
    if i.alphabet.is_some() || choice != AlphabetChoice::Auto {
        return None;
    }
    let why = undetected_alphabet(i).unwrap_or_else(|| "it could not be detected".to_owned());
    Some(format!(
        "Alphabet not detected: {why}. Choose it under Alphabet below."
    ))
}

fn compression_name(c: Compression) -> &'static str {
    match c {
        Compression::None => "none",
        Compression::Gzip => "gzip",
        Compression::Bzip2 => "bzip2",
        Compression::Zstd => "Zstandard",
    }
}

/// The pre-check facts; `choice` is the form's alphabet, which decides whether an alphabet the
/// pre-check could not detect still needs choosing.
pub fn summary(
    ui: &mut Ui,
    path: &std::path::Path,
    i: &Inspection,
    choice: AlphabetChoice,
    t: &DesignTokens,
) {
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
                dima_core::human::plural(i.alignment_length as u64, "column", "columns"),
            );
            fact(ui, t, "Alphabet", alphabet_fact(i, choice));
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
    if let Some(warning) = alphabet_warning(i, choice) {
        ui.colored_label(t.warning_color, warning);
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

#[cfg(test)]
mod tests {
    use super::{AlphabetChoice, alphabet_fact, alphabet_warning};

    fn inspected(text: &str) -> dima_core::validation::Inspection {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.fasta");
        std::fs::write(&path, text).unwrap();
        dima_core::validation::inspect(&dima_core::input::Source::Path(path)).unwrap()
    }

    /// The warning gives the library's reason while Auto is chosen, and goes once an alphabet
    /// is chosen; the fact then names the choice.
    #[test]
    fn the_alphabet_warning_gives_the_reason_while_auto() {
        let few = inspected(">a\nACD\n");
        assert_eq!(
            alphabet_warning(&few, AlphabetChoice::Auto).as_deref(),
            Some(
                "Alphabet not detected: only 3 residue letters in the first record; that is too \
                 few to tell protein from nucleotide sequences. Choose it under Alphabet below."
            )
        );
        assert_eq!(
            alphabet_fact(&few, AlphabetChoice::Auto),
            "undetermined \u{2014} choose one below"
        );
        assert_eq!(alphabet_warning(&few, AlphabetChoice::Protein), None);
        assert_eq!(
            alphabet_fact(&few, AlphabetChoice::Protein),
            "undetermined (Protein chosen below)"
        );
        let mers = dima_core::validation::inspect(&dima_core::input::Source::Path(
            crate::tests::fixtures::mers_path(),
        ))
        .unwrap();
        assert_eq!(alphabet_warning(&mers, AlphabetChoice::Auto), None);
        assert_eq!(
            alphabet_fact(&mers, AlphabetChoice::Dna),
            "Protein (detected from 236346 residues)",
            "what was detected, whatever is chosen"
        );
    }
}
