//! Settings Commands
//!
//! Tauri commands for managing application settings.

use crate::project::get_app_base_path;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tauri::Manager;

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub decimal_precision: u8,
    pub default_output_directory: Option<String>,
    pub default_chart_dpi: u16,
    pub default_kmer_length: usize,
    pub default_support_threshold: usize,
    pub default_validation_mode: String,
    pub last_used_config: Option<serde_json::Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            decimal_precision: 4,
            default_output_directory: None,
            default_chart_dpi: 72,
            default_kmer_length: 9,
            default_support_threshold: 30,
            default_validation_mode: "strict".to_string(),
            last_used_config: None,
        }
    }
}

/// Get settings file path
fn get_settings_path() -> Result<PathBuf, String> {
    let base = get_app_base_path().map_err(|e| e.to_string())?;
    Ok(base.join("settings.json"))
}

/// Get application settings
#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    let settings_path = get_settings_path()?;

    if !settings_path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(&settings_path)
        .await
        .map_err(|e| format!("Failed to read settings: {}", e))?;

    let settings: AppSettings = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    Ok(settings)
}

/// Update application settings
#[tauri::command]
pub async fn update_settings(settings: AppSettings) -> Result<(), String> {
    let settings_path = get_settings_path()?;

    // Ensure parent directory exists
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }

    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&settings_path, content)
        .await
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    Ok(())
}

/// Get the Documents path
#[tauri::command]
pub async fn get_documents_path() -> Result<String, String> {
    let base = get_app_base_path().map_err(|e| e.to_string())?;
    Ok(base.to_string_lossy().to_string())
}

/// Reveal a path in the file explorer
#[tauri::command]
pub async fn reveal_in_explorer(path: String) -> Result<(), String> {
    let path = PathBuf::from(&path);
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open for the parent directory
        if let Some(parent) = path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to open file manager: {}", e))?;
        }
    }

    Ok(())
}

/// Create a new application window
#[tauri::command]
pub async fn create_new_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;
    
    // Generate unique window label
    let window_count = app.webview_windows().len();
    let label = format!("main-{}", window_count + 1);
    
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("index.html".into()))
        .title("DiMA Desktop")
        .inner_size(1280.0, 800.0)
        .min_inner_size(1024.0, 768.0)
        .resizable(true)
        .decorations(true)
        .build()
        .map_err(|e| format!("Failed to create window: {}", e))?;
    
    Ok(())
}
