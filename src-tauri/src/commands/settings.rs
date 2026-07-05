//! Settings Commands
//!
//! Tauri commands for managing application settings.
//! Includes corrupt settings recovery, value validation, and atomic writes.

use crate::error::AppError;
use crate::project::get_app_base_path;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::fs;
use tokio::sync::Mutex;

/// Serializes settings read/write operations to prevent concurrent clobber.
/// Without this, two rapid `update_settings` calls could race on the same tmp file.
static SETTINGS_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Application settings.
/// Uses camelCase serialization to match the frontend TypeScript interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Schema version for forward-compatible deserialization
    #[serde(default = "default_settings_version")]
    pub schema_version: u32,
    pub theme: String,
    pub decimal_precision: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_output_directory: Option<String>,
    pub default_chart_dpi: u16,
    pub default_kmer_length: usize,
    pub default_support_threshold: usize,
    pub default_validation_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_config: Option<serde_json::Value>,
}

fn default_settings_version() -> u32 {
    1
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: "system".to_string(),
            decimal_precision: 4,
            default_output_directory: None,
            default_chart_dpi: 72,
            default_kmer_length: 9,
            default_support_threshold: 100,
            default_validation_mode: "strict".to_string(),
            last_used_config: None,
        }
    }
}

impl AppSettings {
    /// Validate and clamp settings to sane ranges.
    /// Returns the corrected settings (never fails — applies defaults for invalid values).
    fn validated(mut self) -> Self {
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            self.theme = "system".to_string();
        }

        if self.decimal_precision > 10 {
            self.decimal_precision = 4;
        }

        self.default_chart_dpi = self.default_chart_dpi.clamp(36, 600);

        // K-mer length: 1-14 (protein max is 14 due to u64 encoding with base 20^14).
        // Nucleotide allows up to 27 but capping at the protein limit here prevents
        // saving a default that would fail at analysis time for protein sequences. (Fix 4.11)
        if self.default_kmer_length == 0 || self.default_kmer_length > 14 {
            self.default_kmer_length = 9;
        }

        if self.default_support_threshold == 0 || self.default_support_threshold > 10000 {
            self.default_support_threshold = 100;
        }

        if !["strict", "permissive", "report"].contains(&self.default_validation_mode.as_str()) {
            self.default_validation_mode = "strict".to_string();
        }

        // Cap last_used_config to prevent unbounded growth from deeply nested or
        // multi-MB JSON blobs. Expected shape: flat object with ~10 keys, < 2KB.
        if let Some(ref config) = self.last_used_config {
            let serialized_size = serde_json::to_string(config).map(|s| s.len()).unwrap_or(0);
            if serialized_size > 4096 {
                self.last_used_config = None;
            }
        }

        self
    }
}

fn get_settings_path() -> Result<PathBuf, AppError> {
    let base = get_app_base_path().map_err(|e| AppError::SettingsError(e.to_string()))?;
    Ok(base.join("settings.json"))
}

/// Get application settings.
/// Falls back to defaults if settings file is corrupt or missing.
#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, AppError> {
    let settings_path = get_settings_path()?;

    if !settings_path.exists() {
        return Ok(AppSettings::default());
    }

    let content = match fs::read_to_string(&settings_path).await {
        Ok(c) => c,
        Err(_) => return Ok(AppSettings::default()),
    };

    // Parse with fallback to defaults on corruption.
    // Archive the corrupt file and immediately write valid defaults to prevent
    // a backup loop (multiple get_settings calls each creating a new archive
    // before update_settings overwrites the file). (Fix 4.40 + 5.12)
    let settings: AppSettings = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
            let backup_path = settings_path.with_extension(format!("corrupt.{}", timestamp));
            if let Err(copy_err) = fs::copy(&settings_path, &backup_path).await {
                eprintln!("Warning: failed to archive corrupt settings: {}", copy_err);
            } else {
                eprintln!(
                    "Warning: settings.json was corrupt ({}), archived to {:?} and reset to defaults",
                    e, backup_path
                );
            }
            // Write valid defaults immediately so subsequent reads don't re-trigger archiving
            let defaults = AppSettings::default();
            if let Ok(json) = serde_json::to_string_pretty(&defaults) {
                let _ = fs::write(&settings_path, &json).await;
            }
            return Ok(defaults);
        }
    };

    Ok(settings.validated())
}

/// Update application settings with atomic write.
/// Serialized behind SETTINGS_LOCK to prevent concurrent writes racing on the tmp file.
#[tauri::command]
pub async fn update_settings(settings: AppSettings) -> Result<(), AppError> {
    let _guard = SETTINGS_LOCK.lock().await;
    let settings_path = get_settings_path()?;
    let validated = settings.validated();

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let content = serde_json::to_string_pretty(&validated)?;

    let tmp_path = settings_path.with_extension("json.tmp");
    fs::write(&tmp_path, &content).await?;
    fs::rename(&tmp_path, &settings_path).await?;

    Ok(())
}

/// Get the Documents path (base app path)
#[tauri::command]
pub async fn get_documents_path() -> Result<String, AppError> {
    let base = get_app_base_path().map_err(|e| AppError::SettingsError(e.to_string()))?;
    Ok(base.to_string_lossy().to_string())
}

/// Get the full projects directory path, constructed platform-correctly.
/// Avoids the frontend constructing paths with hardcoded forward slashes. (Fix 4.49)
#[tauri::command]
pub async fn get_projects_directory_path() -> Result<String, AppError> {
    let projects_path = crate::project::get_projects_path()
        .map_err(|e| AppError::SettingsError(e.to_string()))?;
    Ok(projects_path.to_string_lossy().to_string())
}

/// Reveal a path in the file explorer
#[tauri::command]
pub async fn reveal_in_explorer(path: String) -> Result<(), AppError> {
    let target = PathBuf::from(&path);

    if !target.exists() {
        return Err(AppError::NotFound(format!("Path does not exist: {}", target.display())));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&target)
            .spawn()
            .map_err(|e| AppError::FileError(format!("Failed to open Finder: {}", e)))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", target.display()))
            .spawn()
            .map_err(|e| AppError::FileError(format!("Failed to open Explorer: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        let dir_to_open = if target.is_dir() {
            target.clone()
        } else {
            target.parent().unwrap_or(&target).to_path_buf()
        };
        std::process::Command::new("xdg-open")
            .arg(&dir_to_open)
            .spawn()
            .map_err(|e| AppError::FileError(format!("Failed to open file manager: {}", e)))?;
    }

    Ok(())
}
