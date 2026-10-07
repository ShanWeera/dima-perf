//! Ctrl/Cmd+K command palette: jump to a position, find which positions carry a k-mer, or run
//! a command. The commands follow the screen (G7). Matches are cached per query and dataset
//! (the session id changes with every load, G23) and recomputed in the same frame as the edit,
//! so Enter always runs the entry shown for the text typed (G55); clicking the backdrop closes
//! it completely (G22).

use crate::state::View;
use crate::state::session::Session;
use crate::theme::DesignTokens;
use crate::util::truncate_display;
use crate::workers::export::ExportKind;

/// Matches listed at most.
const MAX_RESULTS: usize = 12;
/// Shortest k-mer query that scans the variants.
const MIN_SEQUENCE_QUERY: usize = 2;

/// What the user chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    /// Select a position.
    GoToPosition(u64),
    /// Export.
    Export(ExportKind),
    /// Reset the filters.
    ResetFilters,
    /// Show every position on the chart.
    ResetZoom,
    /// Switch between light and dark.
    ToggleTheme,
    /// Open a file.
    OpenFile,
    /// Show a screen.
    Show(View),
    /// Open the diagnostics.
    Diagnostics,
    /// Open the run details.
    RunDetails,
    /// Cite DiMA.
    Cite,
}

impl PaletteAction {
    fn label(self) -> String {
        match self {
            Self::GoToPosition(p) => format!("Go to position {p}"),
            Self::Export(k) => format!("Export {}", k.label().trim_end_matches('\u{2026}')),
            Self::ResetFilters => "Reset filters".to_owned(),
            Self::ResetZoom => "Show every position (reset zoom)".to_owned(),
            Self::ToggleTheme => "Switch light / dark theme".to_owned(),
            Self::OpenFile => "Open a file\u{2026}".to_owned(),
            Self::Show(View::Setup) => "Go to setup".to_owned(),
            Self::Show(View::Workspace) => "Go to the results".to_owned(),
            Self::Diagnostics => "Show diagnostics".to_owned(),
            Self::RunDetails => "Show run details".to_owned(),
            Self::Cite => "Cite DiMA".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    action: PaletteAction,
    label: String,
    detail: String,
}

/// Palette state.
#[derive(Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    cursor: usize,
    just_opened: bool,
    cache: Vec<Entry>,
    cache_key: Option<(String, Option<u64>, View)>,
}

impl CommandPalette {
    /// `true` while shown.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes it.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open = true;
            self.just_opened = true;
            self.query.clear();
            self.cursor = 0;
        }
    }

    /// Closes it and releases the cached matches.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.cursor = 0;
        self.cache = Vec::new();
        self.cache_key = None;
    }

    fn entries(&mut self, session: Option<&Session>, view: View) -> Vec<Entry> {
        let key = (self.query.clone(), session.map(|s| s.id), view);
        if self.cache_key.as_ref() != Some(&key) {
            self.cache = build(&self.query, session, view);
            self.cache_key = Some(key);
        }
        self.cache.clone()
    }

    /// Draws the palette; returns the chosen action.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        t: &DesignTokens,
        session: Option<&Session>,
        view: View,
    ) -> Option<PaletteAction> {
        if !self.open {
            return None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close();
            return None;
        }
        let (up, down, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
            )
        });
        let mut chosen = None;
        let mut backdrop_clicked = false;
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("palette_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let r = ui.allocate_rect(screen, egui::Sense::click());
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(96));
                backdrop_clicked = r.clicked();
            });
        egui::Area::new(egui::Id::new("palette"))
            .order(egui::Order::Tooltip)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 96.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t.surface_elevated)
                    .stroke(egui::Stroke::new(1.0, t.border))
                    .corner_radius(t.panel_rounding)
                    .inner_margin(t.space_12)
                    .show(ui, |ui| {
                        ui.set_width(520.0);
                        let field = ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .hint_text("Position number, k-mer, or command")
                                .desired_width(f32::INFINITY),
                        );
                        if self.just_opened {
                            field.request_focus();
                            self.just_opened = false;
                        }
                        // After the edit, so the list matches the text (G55).
                        let entries = self.entries(session, view);
                        if entries.is_empty() {
                            self.cursor = 0;
                            ui.weak("No matches");
                            return;
                        }
                        self.cursor = self.cursor.min(entries.len() - 1);
                        if down {
                            self.cursor = (self.cursor + 1) % entries.len();
                        }
                        if up {
                            self.cursor = self.cursor.checked_sub(1).unwrap_or(entries.len() - 1);
                        }
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for (i, e) in entries.iter().enumerate() {
                                    let selected = i == self.cursor;
                                    let r = ui.selectable_label(selected, &e.label);
                                    if !e.detail.is_empty() {
                                        ui.weak(&e.detail);
                                    }
                                    if r.clicked() {
                                        chosen = Some(e.action);
                                    }
                                    if selected && (up || down) {
                                        r.scroll_to_me(None);
                                    }
                                }
                            });
                        if enter && chosen.is_none() {
                            chosen = entries.get(self.cursor).map(|e| e.action);
                        }
                        ui.add_space(t.space_4);
                        ui.weak("\u{2191}\u{2193} move, Enter run, Esc close");
                    });
            });
        if chosen.is_some() || backdrop_clicked {
            self.close();
        }
        chosen
    }
}

fn build(query: &str, session: Option<&Session>, view: View) -> Vec<Entry> {
    let q = query.trim();
    let lower = q.to_lowercase();
    let mut out = Vec::new();
    if let (Ok(p), Some(s)) = (q.parse::<u64>(), session)
        && s.position(p).is_some()
    {
        let detail = if s.is_shown(p) {
            String::new()
        } else {
            "hidden by the filters".to_owned()
        };
        out.push(Entry {
            action: PaletteAction::GoToPosition(p),
            label: format!("Go to position {p}"),
            detail,
        });
    }
    if let Some(s) = session
        && q.len() >= MIN_SEQUENCE_QUERY
        && q.parse::<u64>().is_err()
    {
        let needle = q.to_uppercase();
        for p in &s.results.results {
            if out.len() >= MAX_RESULTS {
                break;
            }
            if let Some(v) = p
                .diversity_motifs
                .iter()
                .find(|v| v.sequence.contains(&needle))
            {
                out.push(Entry {
                    action: PaletteAction::GoToPosition(p.position),
                    label: format!(
                        "{} at position {}",
                        truncate_display(&v.sequence, 24),
                        p.position
                    ),
                    detail: format!("{}, {:.1}%", v.motif.long(), v.incidence),
                });
            }
        }
    }
    let mut commands = vec![
        PaletteAction::OpenFile,
        PaletteAction::ToggleTheme,
        PaletteAction::Cite,
    ];
    if session.is_some() {
        match view {
            View::Setup => commands.push(PaletteAction::Show(View::Workspace)),
            View::Workspace => commands.extend([
                PaletteAction::Show(View::Setup),
                PaletteAction::ResetFilters,
                PaletteAction::ResetZoom,
                PaletteAction::Diagnostics,
                PaletteAction::RunDetails,
            ]),
        }
        commands.extend(ExportKind::ALL.into_iter().map(PaletteAction::Export));
    }
    for a in commands {
        let label = a.label();
        if lower.is_empty() || label.to_lowercase().contains(&lower) {
            out.push(Entry {
                action: a,
                label,
                detail: String::new(),
            });
        }
    }
    out.truncate(MAX_RESULTS * 2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn session() -> Session {
        Session::new(
            9,
            crate::tests::fixtures::mers(),
            crate::state::session::Origin::Opened("x.json".into()),
        )
    }

    #[test]
    fn numbers_jump_and_kmers_are_found() {
        let s = session();
        let e = build("25", Some(&s), View::Workspace);
        assert_eq!(e[0].action, PaletteAction::GoToPosition(25));
        assert!(
            build("99999", Some(&s), View::Workspace)
                .iter()
                .all(|e| !matches!(e.action, PaletteAction::GoToPosition(_)))
        );
        let e = build("svksaciev", Some(&s), View::Workspace);
        assert!(
            matches!(e[0].action, PaletteAction::GoToPosition(_)),
            "{e:?}"
        );
        assert!(e[0].label.contains("SVKSACIEV"));
        assert!(
            build("a", Some(&s), View::Workspace)
                .iter()
                .all(|e| !e.label.contains("at position"))
        );
        assert!(build("ACDEF", Some(&s), View::Workspace).len() <= MAX_RESULTS * 2);
    }

    #[test]
    fn commands_follow_the_screen() {
        let s = session();
        let setup = build("", Some(&s), View::Setup);
        assert!(
            setup
                .iter()
                .any(|e| e.action == PaletteAction::Show(View::Workspace)),
            "back to results (G7)"
        );
        assert!(!setup.iter().any(|e| e.action == PaletteAction::ResetZoom));
        let none = build("", None, View::Setup);
        assert!(
            none.iter()
                .all(|e| !matches!(e.action, PaletteAction::Export(_)))
        );
        assert!(none.iter().any(|e| e.action == PaletteAction::OpenFile));
        let theme = build("theme", Some(&s), View::Workspace);
        assert_eq!(theme.len(), 1);
    }

    #[test]
    fn cache_follows_query_and_dataset_and_is_released() {
        let s = session();
        let mut p = CommandPalette::default();
        p.toggle();
        p.query = "25".into();
        assert_eq!(
            p.entries(Some(&s), View::Workspace)[0].action,
            PaletteAction::GoToPosition(25)
        );
        let mut other = session();
        other.id = 10;
        let mut r = (*other.results).clone();
        r.results.retain(|x| x.position != 25);
        other.results = Arc::new(r);
        assert!(
            p.entries(Some(&other), View::Workspace)
                .iter()
                .all(|e| e.action != PaletteAction::GoToPosition(25))
        );
        p.close();
        assert!(p.cache.is_empty() && p.cache_key.is_none() && !p.is_open());
    }
}
