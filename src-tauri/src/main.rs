//! DiMA Desktop - Main Tauri Application Entry Point
//!
//! This is the main entry point for the DiMA Desktop application.
//! It initializes Tauri with all required plugins and command handlers.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod error;
mod progress;
mod project;
mod state;

#[cfg(test)]
mod tests;

use state::AppState;
use tauri::Emitter;
use tauri::Manager;

/// Validate and emit a .dima file-open event to the frontend.
/// Canonicalizes the path, verifies the file exists and is a regular file,
/// and uses a custom event name instead of the system event. (Fix 3.12)
fn emit_dima_file_open<R: tauri::Runtime>(app: &impl Emitter<R>, file_arg: &str) {
    if !file_arg.to_lowercase().ends_with(".dima") {
        return;
    }
    let path = std::path::Path::new(file_arg);
    // Canonicalize to resolve symlinks and `..` segments
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return, // File doesn't exist or is inaccessible
    };
    // Verify it's a regular file (not a directory, device, etc.)
    match std::fs::metadata(&canonical) {
        Ok(meta) if meta.is_file() => {}
        _ => return,
    }
    let _ = app.emit(
        "dima://file-open",
        serde_json::json!({ "path": canonical.to_string_lossy() }),
    );
}

fn main() {
    tauri::Builder::default()
        // Single-instance MUST be first: if a second instance is launched,
        // this plugin focuses the existing window and passes CLI args to it.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Emit validated file-open event using a custom event name (Fix 3.12)
            if let Some(file_arg) = argv.get(1) {
                emit_dima_file_open(app, file_arg);
            }
            // Focus the existing main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Project commands
            commands::project::create_project,
            commands::project::open_project,
            commands::project::list_recent_projects,
            commands::project::delete_project,
            commands::project::clear_recent_projects,
            commands::project::take_pending_open_paths,
            // Layout persistence commands
            commands::project::save_layout,
            commands::project::load_layout,
            // Annotation persistence commands
            commands::project::save_annotations,
            commands::project::load_annotations,
            // Filter persistence commands
            commands::project::save_filters,
            commands::project::load_filters,
            commands::project::save_filter_presets,
            commands::project::load_filter_presets,
            commands::project::load_results,
            // Validation commands
            commands::validate::validate_fasta,
            commands::validate::detect_header_format,
            commands::validate::cancel_validation,
            // Analysis commands
            commands::analyze::run_analysis,
            commands::analyze::cancel_analysis,
            // Export commands
            commands::export::export_results,
            commands::export::export_chart,
            commands::export::import_dima_file,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_documents_path,
            commands::settings::get_projects_directory_path,
            commands::settings::reveal_in_explorer,
            // PDB commands
            commands::pdb::fetch_pdb,
            commands::pdb::parse_pdb_sequence,
            commands::pdb::align_sequences,
            commands::pdb::create_direct_mapping,
            // UniProt commands
            commands::uniprot::fetch_uniprot_accession,
            commands::uniprot::fetch_uniprot_features,
        ])
        .setup(|app| {
            // Create app directories synchronously during setup to guarantee
            // they exist before any command handler can fire.
            if let Err(e) = project::ensure_app_directories_sync() {
                eprintln!("Failed to create app directories: {}", e);
            }

            // Handle first-launch .dima file association (Fix 4.42):
            // When the app is cold-started by double-clicking a .dima file,
            // the path is in std::env::args(). Instead of racing with a 500ms delay,
            // we queue the path in AppState. The frontend pulls it on mount via
            // `take_pending_open_paths`, eliminating the timing dependency entirely.
            if let Some(file_arg) = std::env::args().nth(1) {
                if file_arg.to_lowercase().ends_with(".dima") {
                    let path = std::path::Path::new(&file_arg);
                    if let Ok(canonical) = path.canonicalize() {
                        if std::fs::metadata(&canonical)
                            .map(|m| m.is_file())
                            .unwrap_or(false)
                        {
                            let state = app.state::<AppState>();
                            state.push_pending_open_path(canonical);
                        }
                    }
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running DiMA Desktop");
}
