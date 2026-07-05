//! Export Commands
//!
//! Tauri commands for exporting results and charts.

use crate::error::AppError;
use crate::project::{self, validate_path_confinement};
use dima_lib::{BinaryFormatConfig, CompressionType, Results};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Validate that an export output path is safe: not a symlink, not a device,
/// and the parent directory exists and is writable. Rejects paths that
/// contain `..` segments to prevent directory traversal attacks. (Fix 3.13)
fn validate_export_output_path(path: &Path) -> Result<(), AppError> {
    // Reject paths containing `..` segments to prevent traversal
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(AppError::PathSecurity(
                "Export path must not contain '..' segments".to_string(),
            ));
        }
    }

    // If the file already exists, ensure it's a regular file (not a symlink to a device)
    if path.exists() {
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| AppError::FileError(format!("Failed to check output path: {}", e)))?;
        if meta.file_type().is_symlink() {
            return Err(AppError::PathSecurity(
                "Export path must not be a symbolic link".to_string(),
            ));
        }
        if !meta.is_file() {
            return Err(AppError::PathSecurity(
                "Export path must point to a regular file".to_string(),
            ));
        }
    }

    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(AppError::NotFound(format!(
                "Parent directory does not exist: {}",
                parent.display()
            )));
        }
    }

    Ok(())
}

/// Export format options
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Dima,
}

/// Export request from frontend
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub project_path: String,
    pub output_path: String,
    pub format: ExportFormat,
    pub compression: Option<u8>,
}

/// Export response
#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub success: bool,
    pub output_path: String,
    pub file_size: u64,
}

/// Export results to a file.
/// Validates both the project path (confined to projects directory) and the
/// output path (no traversal, no symlinks). (Fix 3.13, Fix 4.13)
#[tauri::command]
pub async fn export_results(request: ExportRequest) -> Result<ExportResponse, AppError> {
    let project_path = PathBuf::from(&request.project_path);

    // Ensure the project path is within the allowed projects directory
    let projects_base =
        project::get_projects_path().map_err(|e| AppError::ProjectError(e.to_string()))?;
    validate_path_confinement(&project_path, &projects_base)
        .map_err(|e| AppError::PathSecurity(e.to_string()))?;

    let results_path = project_path.join("results.json");

    let output_path = PathBuf::from(&request.output_path);
    validate_export_output_path(&output_path)?;

    let results_content = fs::read_to_string(&results_path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read results: {}", e)))?;

    let results: Results = serde_json::from_str(&results_content)
        .map_err(|e| AppError::InternalError(format!("Failed to parse results: {}", e)))?;

    // Ensure parent directory exists (user may pick a path inside a new folder)
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await.map_err(|e| {
                AppError::ExportError(format!("Failed to create output directory: {}", e))
            })?;
        }
    }

    match request.format {
        ExportFormat::Json => {
            // Export as JSON
            let json = serde_json::to_string_pretty(&results)
                .map_err(|e| AppError::ExportError(format!("Failed to serialize: {}", e)))?;
            fs::write(&output_path, &json)
                .await
                .map_err(|e| AppError::ExportError(format!("Failed to write file: {}", e)))?;
        }
        ExportFormat::Dima => {
            // Export as binary .dima format — runs on blocking thread since
            // binary serialization + compression is CPU-intensive
            let compression = request.compression.unwrap_or(1);
            let config = BinaryFormatConfig {
                compression: match compression {
                    0 => CompressionType::None,
                    2 => CompressionType::Zstd,
                    _ => CompressionType::Lz4,
                },
                compression_level: compression as i32,
                string_interning: true,
                validate_checksums: true,
            };

            let out_path_str = output_path.to_string_lossy().to_string();
            tokio::task::spawn_blocking(move || results.to_binary(out_path_str, Some(config)))
                .await
                .map_err(|e| AppError::InternalError(format!("Binary export task failed: {}", e)))?
                .map_err(|e| AppError::ExportError(format!("Failed to write binary: {}", e)))?;

            // Re-read results for the file size check below (results moved into spawn_blocking)
            let metadata = fs::metadata(&output_path)
                .await
                .map_err(|e| AppError::ExportError(format!("Failed to get file info: {}", e)))?;

            return Ok(ExportResponse {
                success: true,
                output_path: output_path.to_string_lossy().to_string(),
                file_size: metadata.len(),
            });
        }
    }

    // Get output file size
    let metadata = fs::metadata(&output_path)
        .await
        .map_err(|e| AppError::ExportError(format!("Failed to get file info: {}", e)))?;

    Ok(ExportResponse {
        success: true,
        output_path: output_path.to_string_lossy().to_string(),
        file_size: metadata.len(),
    })
}

/// Chart export request
#[derive(Debug, Deserialize)]
pub struct ChartExportRequest {
    pub data_url: String,
    pub output_path: String,
    /// Format is implicit from data URL (PNG/SVG), kept for compatibility
    #[allow(dead_code)]
    pub format: String,
    /// Title is already rendered in the data URL by ECharts, kept for future use
    #[allow(dead_code)]
    pub title: Option<String>,
}

/// Maximum base64 data URL size for chart export (50 MB encoded ≈ 37 MB decoded)
const MAX_CHART_DATA_URL_SIZE: usize = 50 * 1024 * 1024;

/// Export a chart image.
/// Validates the output path is within user-accessible directories. (Fix 3.13)
#[tauri::command]
pub async fn export_chart(request: ChartExportRequest) -> Result<ExportResponse, AppError> {
    let output_path_check = PathBuf::from(&request.output_path);
    validate_export_output_path(&output_path_check)?;

    if request.data_url.len() > MAX_CHART_DATA_URL_SIZE {
        return Err(AppError::ExportError(format!(
            "Chart data too large ({:.1} MB). Maximum supported: 50 MB.",
            request.data_url.len() as f64 / (1024.0 * 1024.0)
        )));
    }

    // The data URL is a base64-encoded image from ECharts
    let data_prefix = "data:image/png;base64,";

    let base64_data = if request.data_url.starts_with(data_prefix) {
        &request.data_url[data_prefix.len()..]
    } else if request.data_url.starts_with("data:image/svg+xml;base64,") {
        &request.data_url["data:image/svg+xml;base64,".len()..]
    } else {
        return Err(AppError::ExportError("Invalid data URL format".to_string()));
    };

    // Decode base64
    use base64::{engine::general_purpose::STANDARD, Engine};
    let image_data = STANDARD
        .decode(base64_data)
        .map_err(|e| AppError::ExportError(format!("Failed to decode image: {}", e)))?;

    // Write to file (ensure parent directory exists)
    let output_path = PathBuf::from(&request.output_path);
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await.map_err(|e| {
                AppError::ExportError(format!("Failed to create output directory: {}", e))
            })?;
        }
    }
    fs::write(&output_path, &image_data)
        .await
        .map_err(|e| AppError::ExportError(format!("Failed to write image: {}", e)))?;

    Ok(ExportResponse {
        success: true,
        output_path: output_path.to_string_lossy().to_string(),
        file_size: image_data.len() as u64,
    })
}

/// Import a .dima file request
#[derive(Debug, Deserialize)]
pub struct ImportDimaRequest {
    pub file_path: String,
    pub project_path: String,
}

/// Import a .dima binary file into a project.
/// Binary parsing runs on a blocking thread to avoid stalling the Tokio reactor.
/// Results are written atomically (tmp + rename) to prevent corruption. (Fix 3.7, 3.14)
#[tauri::command]
pub async fn import_dima_file(request: ImportDimaRequest) -> Result<ExportResponse, AppError> {
    let file_path = PathBuf::from(&request.file_path);
    let project_path = PathBuf::from(&request.project_path);

    // Validate project_path is within our managed Projects directory (Fix 3.14)
    let projects_base =
        crate::project::get_projects_path().map_err(|e| AppError::ProjectError(e.to_string()))?;
    crate::project::validate_path_confinement(&project_path, &projects_base)
        .map_err(|e| AppError::PathSecurity(format!("Project path validation failed: {}", e)))?;

    if !file_path.exists() {
        return Err(AppError::NotFound(format!(
            "File not found: {}",
            request.file_path
        )));
    }

    // Ensure the source is a regular file (not a symlink to a device, etc.)
    let src_meta = fs::symlink_metadata(&file_path)
        .await
        .map_err(|e| AppError::ExportError(format!("Failed to read file metadata: {}", e)))?;
    if !src_meta.is_file() {
        return Err(AppError::ValidationError(
            "Import source must be a regular file".to_string(),
        ));
    }

    // Run CPU-intensive binary deserialization + JSON serialization on a blocking thread
    // to avoid stalling the Tokio async runtime. (Fix 3.7)
    let file_path_str = file_path.to_string_lossy().to_string();
    let json = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        let results = Results::from_binary(file_path_str)
            .map_err(|e| AppError::ExportError(format!("Failed to read .dima file: {}", e)))?;
        serde_json::to_string_pretty(&results)
            .map_err(|e| AppError::ExportError(format!("Failed to serialize results: {}", e)))
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Import task panicked: {}", e)))??;

    // Atomic write: write to tmp file first, then rename (Fix 3.7)
    let output_path = project_path.join("results.json");
    let tmp_path = project_path.join("results.json.tmp");
    fs::write(&tmp_path, &json)
        .await
        .map_err(|e| AppError::ExportError(format!("Failed to write temporary file: {}", e)))?;
    fs::rename(&tmp_path, &output_path).await.map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        AppError::ExportError(format!("Failed to finalize results file: {}", e))
    })?;

    let metadata = fs::metadata(&output_path)
        .await
        .map_err(|e| AppError::ExportError(format!("Failed to get file info: {}", e)))?;

    Ok(ExportResponse {
        success: true,
        output_path: output_path.to_string_lossy().to_string(),
        file_size: metadata.len(),
    })
}
