//! The diagnostics window (D60, G38, G39): every condition the run and the conserved-region
//! computation reported (code, severity, count, examples, hint), then the input validation
//! record: alphabet determination, character classes, whitespace, header cleaning and header
//! mismatches.

use egui::{RichText, Ui};

use dima_core::diagnostics::{Diagnostic, Severity as DiagSeverity};
use dima_core::model::{HcsResults, Results};

use super::common::fact;
use crate::theme::DesignTokens;

/// Draws the window when open.
pub fn window(
    ctx: &egui::Context,
    open: &mut bool,
    r: &Results,
    hcs: Option<&HcsResults>,
    t: &DesignTokens,
) {
    egui::Window::new("Diagnostics")
        .open(open)
        .default_width(560.0)
        .default_height(520.0)
        .vscroll(true)
        .show(ctx, |ui| contents(ui, r, hcs, t));
}

fn contents(ui: &mut Ui, r: &Results, hcs: Option<&HcsResults>, t: &DesignTokens) {
    let all: Vec<&Diagnostic> = r
        .run
        .diagnostics
        .iter()
        .chain(hcs.iter().flat_map(|h| &h.diagnostics))
        .collect();
    if all.is_empty() {
        ui.label("No conditions were reported for this run.");
    }
    for d in all {
        egui::Frame::new()
            .fill(t.surface_secondary)
            .corner_radius(t.panel_rounding)
            .inner_margin(t.space_8)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let (label, color) = match d.severity {
                    DiagSeverity::Warning => ("Warning", t.warning_color),
                    DiagSeverity::Info => ("Note", t.text_secondary),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).strong().color(color));
                    ui.monospace(&d.code);
                    ui.weak(format!("\u{00D7}{}", d.count));
                });
                ui.add(egui::Label::new(&d.message).wrap());
                for e in &d.examples {
                    ui.label(
                        RichText::new(format!("  \u{2022} {e}"))
                            .small()
                            .color(t.text_secondary),
                    );
                }
                if let Some(h) = &d.hint {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("Hint: {h}")).color(t.text_secondary),
                        )
                        .wrap(),
                    );
                }
            });
        ui.add_space(t.space_4);
    }
    ui.separator();
    egui::CollapsingHeader::new("Input validation")
        .default_open(false)
        .show(ui, |ui| validation(ui, r, t));
}

fn validation(ui: &mut Ui, r: &Results, t: &DesignTokens) {
    let a = &r.run.alphabet;
    let v = &r.run.validation;
    egui::Grid::new("validation_facts")
        .num_columns(2)
        .show(ui, |ui| {
            fact(
                ui,
                t,
                "Alphabet",
                format!(
                    "{} ({}; {} residues in {} records examined)",
                    a.resolved.display_name(),
                    match a.method {
                        dima_core::model::AlphabetMethod::Detected => "detected",
                        dima_core::model::AlphabetMethod::User => "chosen",
                    },
                    a.residues_examined,
                    a.records_examined
                ),
            );
            if let Some(c) = &a.contradiction {
                fact(ui, t, "Contradiction", c.clone());
            }
            fact(ui, t, "T/U folded", a.t_u_folded.to_string());
            fact(ui, t, "Validation mode", v.mode.as_str().to_owned());
            fact(ui, t, "Records", v.records.to_string());
            fact(ui, t, "Lower case folded", v.lowercase_folded.to_string());
            fact(
                ui,
                t,
                "Whitespace lines",
                format!(
                    "{} trailing, {} interior",
                    v.whitespace.trailing, v.whitespace.interior
                ),
            );
            fact(ui, t, "Unknown characters", v.unknown_total.to_string());
            fact(
                ui,
                t,
                "Header UTF-8 replacements",
                v.header_utf8_replacements.to_string(),
            );
            if let Some(c) = &v.header_cleaning {
                fact(
                    ui,
                    t,
                    "Header values cleaned",
                    format!(
                        "{} trimmed, {} NFC-normalised, {} filled, {} invisible removed",
                        c.trimmed,
                        c.nfc_changed,
                        c.fill_uses,
                        c.invisible_removed.iter().map(|x| x.count).sum::<u64>()
                    ),
                );
            }
            if let Some(m) = &r.run.metadata {
                fact(ui, t, "Header fields", m.fields.join("|"));
                fact(ui, t, "Mismatched headers", m.mismatches.count.to_string());
            }
        });
    if let Some(m) = &r.run.metadata {
        for x in &m.mismatches.first {
            ui.label(
                RichText::new(format!(
                    "record {} (line {}): {} fields, expected {}: {}",
                    x.record,
                    x.line,
                    x.found,
                    x.expected,
                    crate::util::truncate_display(&x.header, 80)
                ))
                .small(),
            );
        }
    }
    ui.add_space(t.space_8);
    ui.label(RichText::new("Characters in sequence lines").strong());
    egui::Grid::new("characters")
        .striped(true)
        .num_columns(4)
        .show(ui, |ui| {
            for h in ["Character", "Byte", "Class", "Count"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for c in &v.characters {
                ui.monospace(c.character.as_deref().unwrap_or("\u{00B7}"));
                ui.monospace(format!("0x{:02X}", c.byte));
                ui.label(c.class.as_str());
                ui.label(c.count.to_string());
                ui.end_row();
            }
        });
}
