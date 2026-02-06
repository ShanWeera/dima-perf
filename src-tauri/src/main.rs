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

use state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Project commands
            commands::project::create_project,
            commands::project::open_project,
            commands::project::list_recent_projects,
            commands::project::delete_project,
            commands::project::clear_recent_projects,
            commands::project::get_project_path,
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
            // Validation commands
            commands::validate::validate_fasta,
            commands::validate::detect_header_format,
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
            commands::settings::reveal_in_explorer,
            commands::settings::create_new_window,
            // PDB commands
            commands::pdb::fetch_pdb,
            commands::pdb::parse_pdb_sequence,
            commands::pdb::align_sequences,
            commands::pdb::create_direct_mapping,
        ])
        .setup(|app| {
            // Initialize app data directory
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = project::ensure_app_directories(&app_handle).await {
                    eprintln!("Failed to create app directories: {}", e);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running DiMA Desktop");
}
