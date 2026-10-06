//! DiMA desktop app: the sequence diversity dynamics analyser for viruses (Tharanga S, Ünlü ES,
//! Hu Y, Sjaugi MF, Çelik MA, Hekimoğlu H, Miotto O, Öncel MM, Khan AM. Brief Bioinform
//! 2025;26(1):bbae607, doi:10.1093/bib/bbae607) with a graphical interface.
//!
//! Every computation, default, validation rule, citation and output format comes from the
//! `dima_core` library, which the command line uses too (plan D23).

// No console window behind the app on Windows release builds (G49).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod app;
mod charts;
mod panels;
mod persist;
mod platform;
mod report;
mod state;
mod theme;
mod util;
mod views;
mod workers;

#[cfg(test)]
mod tests;

fn main() -> std::process::ExitCode {
    match app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            let text = format!(
                "DiMA could not start: {e}\n\nIf this mentions graphics, updating the graphics \
                 driver usually helps. The command-line tool `dima` does not need a display."
            );
            eprintln!("error: {text}");
            // Windows release builds have no console, so the message box is the only place the
            // reason shows; best effort elsewhere (Linux needs a desktop portal or zenity).
            let _ = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("DiMA")
                .set_description(text)
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            std::process::ExitCode::FAILURE
        }
    }
}
