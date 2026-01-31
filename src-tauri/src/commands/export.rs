//! Export Commands
//!
//! Tauri commands for exporting results and charts.

use dima_lib::{Results, BinaryFormatConfig, CompressionType};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

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

/// Export results to a file
#[tauri::command]
pub async fn export_results(request: ExportRequest) -> Result<ExportResponse, String> {
    let project_path = PathBuf::from(&request.project_path);
    let results_path = project_path.join("results.json");

    // Read results
    let results_content = fs::read_to_string(&results_path)
        .await
        .map_err(|e| format!("Failed to read results: {}", e))?;

    let results: Results =
        serde_json::from_str(&results_content).map_err(|e| format!("Failed to parse results: {}", e))?;

    let output_path = PathBuf::from(&request.output_path);

    match request.format {
        ExportFormat::Json => {
            // Export as JSON
            let json = serde_json::to_string_pretty(&results)
                .map_err(|e| format!("Failed to serialize: {}", e))?;
            fs::write(&output_path, &json)
                .await
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
        ExportFormat::Dima => {
            // Export as binary .dima format
            let compression = request.compression.unwrap_or(1);
            let config = BinaryFormatConfig {
                compression: match compression {
                    0 => CompressionType::None,
                    2 => CompressionType::Zstd,
                    _ => CompressionType::Lz4,
                },
                compression_level: compression as i32,
                string_interning: true,
                buffer_size: 64 * 1024,
                validate_checksums: true,
            };

            results
                .to_binary(output_path.to_string_lossy().to_string(), Some(config))
                .map_err(|e| format!("Failed to write binary: {}", e))?;
        }
    }

    // Get output file size
    let metadata = fs::metadata(&output_path)
        .await
        .map_err(|e| format!("Failed to get file info: {}", e))?;

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

/// Export a chart image
#[tauri::command]
pub async fn export_chart(request: ChartExportRequest) -> Result<ExportResponse, String> {
    // The data URL is a base64-encoded image from ECharts
    let data_prefix = "data:image/png;base64,";
    
    let base64_data = if request.data_url.starts_with(data_prefix) {
        &request.data_url[data_prefix.len()..]
    } else if request.data_url.starts_with("data:image/svg+xml;base64,") {
        &request.data_url["data:image/svg+xml;base64,".len()..]
    } else {
        return Err("Invalid data URL format".to_string());
    };

    // Decode base64
    use base64::{engine::general_purpose::STANDARD, Engine};
    let image_data = STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // Write to file
    let output_path = PathBuf::from(&request.output_path);
    fs::write(&output_path, &image_data)
        .await
        .map_err(|e| format!("Failed to write image: {}", e))?;

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

/// Import a .dima binary file into a project
#[tauri::command]
pub async fn import_dima_file(request: ImportDimaRequest) -> Result<ExportResponse, String> {
    let file_path = PathBuf::from(&request.file_path);
    let project_path = PathBuf::from(&request.project_path);

    // Check if file exists
    if !file_path.exists() {
        return Err(format!("File not found: {}", request.file_path));
    }

    // Read binary file
    let results = Results::from_binary(file_path.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to read .dima file: {}", e))?;

    // Convert to JSON and save to project
    let json = serde_json::to_string_pretty(&results)
        .map_err(|e| format!("Failed to serialize results: {}", e))?;

    let output_path = project_path.join("results.json");
    fs::write(&output_path, &json)
        .await
        .map_err(|e| format!("Failed to write results: {}", e))?;

    let metadata = fs::metadata(&output_path)
        .await
        .map_err(|e| format!("Failed to get file info: {}", e))?;

    Ok(ExportResponse {
        success: true,
        output_path: output_path.to_string_lossy().to_string(),
        file_size: metadata.len(),
    })
}
