//! The two screens and the overlays shared by both (top bar, windows, palette, questions,
//! notifications). `render` draws one frame and returns the user's intents as messages.

pub mod setup;
pub mod workspace;

use crate::app::msg::Msg;
use crate::panels::command_palette::{CommandPalette, PaletteAction};
use crate::panels::entropy_chart::ChartUi;
use crate::panels::header_editor::PasteBox;
use crate::panels::run::RunProgress;
use crate::panels::{
    confirm, diagnostics, header_bar, help_windows, preferences, run_details, toasts,
};
use crate::state::session::Nav;
use crate::state::{AppState, View};
use crate::theme::DesignTokens;

/// Interface state that is not application state.
#[derive(Default)]
pub struct UiState {
    /// Command palette.
    pub palette: CommandPalette,
    /// Chart gestures and GPU cache.
    pub chart: ChartUi,
    /// Header-format paste box.
    pub paste: PasteBox,
    /// A wgpu device is available for the chart line.
    pub gpu: bool,
    /// Progress of the running analysis (set by the app each frame).
    pub progress: Option<RunProgress>,
}

/// Draws one frame.
pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    u: &mut UiState,
    t: &DesignTokens,
) -> Vec<Msg> {
    let mut out = Vec::new();
    let ctx = ui.ctx().clone();
    egui::Panel::top("top_bar").show(ui, |ui| header_bar::show(ui, state, t, &mut out));
    match state.view {
        View::Setup => setup::show(ui, state, u, t, &mut out),
        View::Workspace => {
            if state.session.is_some() {
                keyboard(&ctx, u, &mut out);
                workspace::show(ui, state, u, t, &mut out);
            } else {
                setup::show(ui, state, u, t, &mut out);
            }
        }
    }
    windows(&ctx, state, t, &mut out);
    confirm::show(&ctx, state.confirm.as_ref(), &mut out);
    palette(&ctx, state, u, t, &mut out);
    toasts::show(&ctx, &state.toasts, t, &mut out);
    out
}

fn windows(ctx: &egui::Context, state: &mut AppState, t: &DesignTokens, out: &mut Vec<Msg>) {
    help_windows::show(ctx, &mut state.windows, t);
    if state.windows.preferences {
        preferences::window(
            ctx,
            &mut state.windows.preferences,
            &mut state.settings,
            out,
        );
    }
    if let Some(s) = state.session.as_ref() {
        if state.windows.diagnostics {
            let hcs = state.hcs.current(s.id).cloned();
            diagnostics::window(
                ctx,
                &mut state.windows.diagnostics,
                &s.results,
                hcs.as_deref(),
                t,
            );
        }
        if state.windows.run_details {
            run_details::window(ctx, &mut state.windows.run_details, s, t, out);
        }
    }
}

/// Arrow keys, Home and End move along the position table; never while a text field has focus
/// or the palette is open.
fn keyboard(ctx: &egui::Context, u: &UiState, out: &mut Vec<Msg>) {
    if ctx.egui_wants_keyboard_input() || u.palette.is_open() {
        return;
    }
    let nav = ctx.input(|i| {
        if i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowRight) {
            Some(Nav::Next)
        } else if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowLeft) {
            Some(Nav::Previous)
        } else if i.key_pressed(egui::Key::Home) {
            Some(Nav::First)
        } else if i.key_pressed(egui::Key::End) {
            Some(Nav::Last)
        } else {
            None
        }
    });
    if let Some(n) = nav {
        out.push(Msg::Navigate(n));
    }
}

fn palette(
    ctx: &egui::Context,
    state: &mut AppState,
    u: &mut UiState,
    t: &DesignTokens,
    out: &mut Vec<Msg>,
) {
    let toggle = ctx.input_mut(|i| {
        i.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::K,
        ))
    });
    if toggle {
        u.palette.toggle();
    }
    let Some(action) = u.palette.show(ctx, t, state.session.as_ref(), state.view) else {
        return;
    };
    match action {
        PaletteAction::GoToPosition(p) => {
            out.push(Msg::ShowView(View::Workspace));
            out.push(Msg::Select(p, crate::state::session::SelectSource::Palette));
        }
        PaletteAction::Export(k) => out.push(Msg::Export(k)),
        PaletteAction::ResetFilters => out.push(Msg::ResetFilters),
        PaletteAction::ResetZoom => {
            if let Some(s) = state.session.as_mut() {
                s.viewport = None;
            }
        }
        PaletteAction::ToggleTheme => ctx.set_theme(match ctx.theme() {
            egui::Theme::Dark => egui::Theme::Light,
            egui::Theme::Light => egui::Theme::Dark,
        }),
        PaletteAction::OpenFile => out.push(Msg::OpenDialog),
        PaletteAction::Show(v) => out.push(Msg::ShowView(v)),
        PaletteAction::Diagnostics => state.windows.diagnostics = true,
        PaletteAction::RunDetails => state.windows.run_details = true,
        PaletteAction::Cite => state.windows.cite = true,
    }
}
