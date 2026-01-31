//! Analysis Commands
//!
//! Tauri commands for running DiMA analysis.

use crate::progress::{AnalysisStage, ProgressUpdate, TauriProgressReporter};
use crate::project::{
    load_project_metadata, save_project_metadata, InputFileInfo,
};
use crate::state::AppState;
use dima_lib::{get_results_objs, AnalysisConfig as DimaConfig, ValidationMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{State, Window};
use tokio::fs;

/// Analysis configuration from the frontend
#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisRequest {
    pub project_path: String,
    pub input_path: String,
    pub copy_input: bool,
    pub kmer_length: usize,
    pub support_threshold: usize,
    pub query_name: String,
    pub alphabet: String,
    pub header_format: Option<String>,
    pub metadata_fields: Option<String>,
    pub validation_mode: String,
    pub allow_lowercase: bool,
    /// HCS-only mode (future feature - currently HCS is always calculated)
    #[allow(dead_code)]
    pub hcs_enabled: bool,
    /// HCS threshold filter (future feature - threshold filtering is in UI)
    #[allow(dead_code)]
    pub hcs_threshold: Option<f32>,
}

/// Analysis result returned to frontend
#[derive(Debug, Serialize)]
pub struct AnalysisResponse {
    pub success: bool,
    pub results_path: Option<String>,
    pub sequence_count: usize,
    pub position_count: usize,
    pub average_entropy: f64,
    pub highest_entropy_position: usize,
    pub highest_entropy_value: f64,
}

/// Run the DiMA analysis
#[tauri::command]
pub async fn run_analysis(
    window: Window,
    state: State<'_, AppState>,
    request: AnalysisRequest,
) -> Result<AnalysisResponse, String> {
    let project_path = PathBuf::from(&request.project_path);

    // Start tracking analysis
    state.start_analysis(request.query_name.clone()).await;

    // Create progress reporter
    let reporter = TauriProgressReporter::new(window.clone(), state.cancel_flag.clone());

    // Report starting
    reporter.report(ProgressUpdate {
        stage: AnalysisStage::ReadingFasta,
        current: 0,
        total: 100,
        message: "Starting analysis...".to_string(),
        throughput: None,
    });

    // Copy input file if requested
    let input_path = if request.copy_input {
        let source = PathBuf::from(&request.input_path);
        let file_name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "input.fasta".to_string());
        let dest = project_path.join(&file_name);

        if !dest.exists() {
            fs::copy(&source, &dest)
                .await
                .map_err(|e| format!("Failed to copy input file: {}", e))?;
        }

        // Update project metadata
        let mut metadata = load_project_metadata(&project_path)
            .await
            .map_err(|e| e.to_string())?;
        metadata.input_file = Some(InputFileInfo {
            original_path: request.input_path.clone(),
            copied_to_project: true,
            file_name: file_name.clone(),
        });
        save_project_metadata(&project_path, &metadata)
            .await
            .map_err(|e| e.to_string())?;

        dest.to_string_lossy().to_string()
    } else {
        request.input_path.clone()
    };

    // Check for cancellation
    if state.is_cancelled() {
        state.stop_analysis().await;
        return Err("Analysis cancelled".to_string());
    }

    reporter.report(ProgressUpdate {
        stage: AnalysisStage::KmerExtraction,
        current: 25,
        total: 100,
        message: "Extracting k-mers...".to_string(),
        throughput: None,
    });

    // Convert validation mode
    let validation_mode = match request.validation_mode.as_str() {
        "strict" => ValidationMode::Strict,
        "permissive" => ValidationMode::Permissive,
        "report" => ValidationMode::ReportOnly,
        _ => ValidationMode::Strict,
    };

    // Parse header format if provided
    let header_format_vec: Option<Vec<String>> = request.header_format.as_ref().map(|hf| {
        hf.split('|').map(|s| s.to_string()).collect()
    });

    // Parse metadata fields if provided
    let metadata_fields_vec: Option<Vec<String>> = request.metadata_fields.as_ref().map(|mf| {
        mf.split(',').map(|s| s.trim().to_string()).collect()
    });

    let kmer_length = request.kmer_length;
    let support_threshold = request.support_threshold;
    let query_name = request.query_name.clone();
    let alphabet = Some(request.alphabet.clone());
    let allow_lowercase = request.allow_lowercase;

    // Run the analysis using DiMA library
    let (results, _validation_stats) = tokio::task::spawn_blocking(move || {
        // Create DiMA config
        let config = DimaConfig::new()
            .with_validation_mode(validation_mode)
            .with_allow_lowercase(allow_lowercase);

        // Call DiMA analysis
        get_results_objs(
            input_path,
            kmer_length,
            support_threshold,
            query_name,
            header_format_vec,
            alphabet,
            None, // header_fillna
            metadata_fields_vec,
            Some(config),
        )
    })
    .await
    .map_err(|e| format!("Analysis task failed: {}", e))?;

    // Check for cancellation
    if state.is_cancelled() {
        state.stop_analysis().await;
        return Err("Analysis cancelled".to_string());
    }

    reporter.report(ProgressUpdate {
        stage: AnalysisStage::OutputGeneration,
        current: 90,
        total: 100,
        message: "Saving results...".to_string(),
        throughput: None,
    });

    // Save results to project folder
    let results_path = project_path.join("results.json");
    let results_json = serde_json::to_string_pretty(&results)
        .map_err(|e| format!("Failed to serialize results: {}", e))?;
    fs::write(&results_path, results_json)
        .await
        .map_err(|e| format!("Failed to save results: {}", e))?;

    // Report completion
    reporter.report_complete();
    state.stop_analysis().await;

    Ok(AnalysisResponse {
        success: true,
        results_path: Some(results_path.to_string_lossy().to_string()),
        sequence_count: results.sequence_count,
        position_count: results.results.len(),
        average_entropy: results.average_entropy,
        highest_entropy_position: results.highest_entropy.position,
        highest_entropy_value: results.highest_entropy.entropy,
    })
}

/// Cancel the current analysis
#[tauri::command]
pub async fn cancel_analysis(state: State<'_, AppState>) -> Result<(), String> {
    state.request_cancel();
    Ok(())
}
