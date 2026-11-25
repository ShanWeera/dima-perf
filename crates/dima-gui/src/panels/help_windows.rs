//! Help windows: Cite DiMA (D89: the paper and this software, in-text and full forms, with
//! Copy citation and Copy BibTeX), keyboard shortcuts, and About (version and licences).

use egui::{RichText, Ui};

use dima_core::citations::{self, Cite};
use dima_core::methods::{self, CitationFormat, Cited};

use crate::state::Windows;
use crate::theme::DesignTokens;

const LICENSE: &str = include_str!("../../../../LICENSE");
const NOTICES: &str = include_str!("../../../../THIRD-PARTY-NOTICES.md");
const OFL: &str = include_str!("../../assets/fonts/OFL.txt");

/// The plain-text citation copied by "Copy citation": the paper, then this software.
#[must_use]
pub fn citation_text() -> String {
    format!(
        "{}\n{}",
        citations::dima_paper().text(),
        citations::software_text(citations::SOFTWARE_VERSION)
    )
}

/// Draws the open help windows.
pub fn show(ctx: &egui::Context, w: &mut Windows, t: &DesignTokens) {
    egui::Window::new("Cite DiMA")
        .open(&mut w.cite)
        .default_width(520.0)
        .show(ctx, cite);
    egui::Window::new("Keyboard shortcuts")
        .open(&mut w.shortcuts)
        .resizable(false)
        .show(ctx, |ui| shortcuts(ui, t));
    egui::Window::new("About DiMA")
        .open(&mut w.about)
        .default_width(560.0)
        .default_height(480.0)
        .vscroll(true)
        .show(ctx, |ui| about(ui, t));
}

fn cite(ui: &mut Ui) {
    let paper = citations::dima_paper();
    ui.label("If you use DiMA, please cite the paper and the software version you used:");
    ui.add_space(6.0);
    ui.label(RichText::new("In text").strong());
    ui.label(Cite::new(citations::DIMA_PAPER).short());
    ui.add_space(6.0);
    ui.label(RichText::new("Reference").strong());
    ui.add(egui::Label::new(paper.text()).wrap().selectable(true));
    if let Some(href) = paper.href() {
        ui.hyperlink(href);
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Software").strong());
    ui.add(
        egui::Label::new(citations::software_text(citations::SOFTWARE_VERSION))
            .wrap()
            .selectable(true),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Copy citation").clicked() {
            ui.ctx().copy_text(citation_text());
        }
        if ui.button("Copy BibTeX").clicked() {
            ui.ctx().copy_text(methods::references(
                Cited::default(),
                CitationFormat::Bibtex,
            ));
        }
    });
    ui.label(
        RichText::new("Export \u{25B8} Methods & references lists the references of an analysis.")
            .small(),
    );
}

fn shortcuts(ui: &mut Ui, t: &DesignTokens) {
    let cmd = if cfg!(target_os = "macos") {
        "\u{2318}"
    } else {
        "Ctrl+"
    };
    egui::Grid::new("shortcuts")
        .num_columns(2)
        .spacing([t.space_16, t.space_4])
        .show(ui, |ui| {
            for (k, what) in [
                (
                    format!("{cmd}K"),
                    "Command palette: go to a position, find a k-mer, run a command",
                ),
                (
                    "\u{2190} \u{2191} / \u{2192} \u{2193}".to_owned(),
                    "Previous / next row of the position table",
                ),
                ("Home / End".to_owned(), "First / last row"),
                ("Tab / Shift+Tab".to_owned(), "Move between controls"),
                ("Space".to_owned(), "Press or select"),
                ("Scroll over the chart".to_owned(), "Zoom about the cursor"),
                ("Drag / Shift-drag".to_owned(), "Pan / zoom to a region"),
                ("Double-click the chart".to_owned(), "Show every position"),
                (
                    "Esc".to_owned(),
                    "Close the palette; also gives the arrow keys back to the table",
                ),
            ] {
                ui.monospace(k);
                ui.label(what);
                ui.end_row();
            }
        });
}

/// A Markdown text split at its headings: `(heading, text up to the next heading)`; text
/// before the first heading is titled "Notices". Empty parts are left out, and lines in code
/// blocks (licence texts) are never headings.
fn sections(text: &str) -> Vec<(&str, &str)> {
    let mut parts = Vec::new();
    let mut title = "Notices";
    let mut start = 0;
    let mut offset = 0;
    let mut in_code = false;
    for line in text.split_inclusive('\n') {
        if line.starts_with("```") {
            in_code = !in_code;
        }
        if let Some(heading) = line.strip_prefix('#').filter(|_| !in_code) {
            let body = text[start..offset].trim();
            if !body.is_empty() {
                parts.push((title, body));
            }
            title = heading.trim_start_matches('#').trim();
            start = offset + line.len();
        }
        offset += line.len();
    }
    let body = text[start..].trim();
    if !body.is_empty() {
        parts.push((title, body));
    }
    parts
}

fn about(ui: &mut Ui, t: &DesignTokens) {
    ui.label(RichText::new(format!("DiMA desktop app {}", env!("CARGO_PKG_VERSION"))).strong());
    ui.label(format!("Built for {}", env!("DIMA_GUI_TARGET")));
    ui.hyperlink_to("Documentation", citations::DOCS_URL);
    ui.hyperlink_to("Source code", env!("CARGO_PKG_REPOSITORY"));
    ui.add_space(t.space_8);
    ui.label(format!(
        "Implements the method of {}.",
        Cite::new(citations::DIMA_PAPER).reference()
    ));
    ui.add_space(t.space_8);
    egui::CollapsingHeader::new("Licence (MIT)").show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(LICENSE).monospace().small()).wrap());
    });
    egui::CollapsingHeader::new("Third-party notices").show(ui, |ui| {
        // One collapsible part per heading: the file is hundreds of kilobytes, too much to lay
        // out as one label every frame.
        static PARTS: std::sync::OnceLock<Vec<(&'static str, &'static str)>> =
            std::sync::OnceLock::new();
        for (i, (title, body)) in PARTS.get_or_init(|| sections(NOTICES)).iter().enumerate() {
            egui::CollapsingHeader::new(*title)
                .id_salt(("notices", i))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(*body).small()).wrap());
                });
        }
    });
    egui::CollapsingHeader::new("IBM Plex fonts (SIL Open Font License 1.1)").show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(OFL).monospace().small()).wrap());
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn citation_names_the_paper_and_the_software() {
        let c = super::citation_text();
        assert!(c.contains("10.1093/bib/bbae607"));
        assert!(c.contains("version"));
        assert!(super::LICENSE.contains("MIT"));
        assert!(!super::OFL.is_empty() && !super::NOTICES.is_empty());
    }

    #[test]
    fn notices_are_split_at_headings() {
        let parts = super::sections("intro\n# A\nx\n## B\n\n### C\ny\n```\n# not\n```\n");
        assert_eq!(
            parts,
            [
                ("Notices", "intro"),
                ("A", "x"),
                ("C", "y\n```\n# not\n```")
            ]
        );
        let all = super::sections(super::NOTICES);
        assert!(all.len() > 100);
        assert!(all.iter().all(|(t, b)| !t.is_empty() && !b.is_empty()));
    }
}
