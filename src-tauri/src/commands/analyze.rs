//! Analysis Commands
//!
//! Tauri commands for running DiMA analysis.

use crate::error::AppError;
use crate::progress::{AnalysisStage, ProgressUpdate, TauriProgressReporter};
use crate::project::{
    self, file_mtime_fingerprint, load_project_metadata, save_project_metadata,
    validate_path_confinement, InputFileInfo,
};
use crate::state::AppState;
use dima_lib::{analyze, InputSource, max_kmer_length, AnalysisConfig as DimaConfig, ValidationMode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{Emitter, State, Window};
use tokio::fs;

/// RAII guard that ensures `stop_analysis` is called even if the async task
/// panics (unwinding through an await point). On normal return the explicit
/// `stop_analysis().await` runs first; the Drop impl is the safety net. (Fix 5.9)
struct AnalysisCleanupGuard<'a> {
    state: &'a AppState,
    disarmed: bool,
}

impl<'a> AnalysisCleanupGuard<'a> {
    fn new(state: &'a AppState) -> Self {
        Self { state, disarmed: false }
    }

    /// Called on the normal (non-panic) path after the async cleanup succeeds.
    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for AnalysisCleanupGuard<'_> {
    fn drop(&mut self) {
        if !self.disarmed {
            self.state.stop_analysis_sync();
        }
    }
}

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
    /// File fingerprint from validation — used to detect if the file changed
    /// between validation and analysis start (TOCTOU binding, Fix 4.30).
    /// Both fields are optional for backward compatibility.
    #[serde(default)]
    pub validated_file_size: Option<u64>,
    #[serde(default)]
    pub validated_file_modified_at: Option<String>,
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
    /// Non-fatal warnings about the analysis (e.g., threshold above sequence count)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Run the DiMA analysis.
/// Uses an analysis mutex to prevent concurrent analyses and ensures
/// `stop_analysis()` is always called on both success and error paths.
#[tauri::command]
pub async fn run_analysis(
    window: Window,
    state: State<'_, AppState>,
    request: AnalysisRequest,
) -> Result<AnalysisResponse, AppError> {
    let _analysis_guard = state
        .analysis_mutex
        .try_lock()
        .map_err(|_| AppError::AnalysisError("An analysis is already running. Please wait or cancel it first.".to_string()))?;

    // Validate project path is confined to the projects directory
    let projects_base = project::get_projects_path()
        .map_err(|e| AppError::ProjectError(e.to_string()))?;
    let project_path = PathBuf::from(&request.project_path);
    validate_path_confinement(&project_path, &projects_base)
        .map_err(|e| AppError::ProjectError(e.to_string()))?;

    // Start tracking analysis (also resets cancel flag)
    state.start_analysis(request.query_name.clone()).await;

    // RAII guard: if a panic bypasses our explicit stop_analysis().await below,
    // the Drop impl will call stop_analysis_sync() as a safety net. (Fix 5.9)
    let mut cleanup_guard = AnalysisCleanupGuard::new(&state);

    let reporter = TauriProgressReporter::new(window.clone(), state.cancel_flag.clone());

    let result = run_analysis_inner(&window, &state, &request, &project_path).await;

    // Always emit a terminal progress event so the frontend's progress UI
    // doesn't get stuck showing a stale percentage on error/cancel. (Fix 5.10)
    if result.is_ok() {
        reporter.report_complete();
    } else {
        reporter.report(ProgressUpdate {
            stage: AnalysisStage::Complete,
            current: 0,
            total: 0,
            message: "Analysis failed".to_string(),
            throughput: None,
        });
    }

    // Normal cleanup path — disarm the Drop guard since we're doing explicit async cleanup
    state.stop_analysis().await;
    cleanup_guard.disarm();

    result
}

/// Inner analysis logic separated for clean error handling.
/// Any error returned here will trigger stop_analysis() in the caller.
async fn run_analysis_inner(
    window: &Window,
    state: &State<'_, AppState>,
    request: &AnalysisRequest,
    project_path: &Path,
) -> Result<AnalysisResponse, AppError> {
    // Explicitly validate alphabet rather than silently defaulting to protein
    // if the frontend sends an unexpected value (Fix 4.41).
    let is_protein = match request.alphabet.as_str() {
        "protein" => true,
        "nucleotide" => false,
        other => {
            return Err(AppError::ValidationError(format!(
                "Invalid alphabet '{}'. Must be 'protein' or 'nucleotide'.",
                other
            )));
        }
    };
    let max_kmer = max_kmer_length(is_protein);
    if request.kmer_length == 0 || request.kmer_length > max_kmer {
        return Err(AppError::ValidationError(format!(
            "K-mer length must be between 1 and {} for {} sequences",
            max_kmer, request.alphabet
        )));
    }
    if request.support_threshold == 0 {
        return Err(AppError::ValidationError("Support threshold must be at least 1".to_string()));
    }
    if request.support_threshold > 10_000 {
        return Err(AppError::ValidationError(format!(
            "Support threshold {} exceeds maximum (10,000). Values above 10,000 are \
             scientifically uncommon and often indicate a configuration error.",
            request.support_threshold
        )));
    }

    // Verify file fingerprint from validation hasn't changed (Fix 4.30).
    // Detects if the FASTA file was modified between validation and analysis start.
    if request.validated_file_size.is_some() || request.validated_file_modified_at.is_some() {
        let source_path = PathBuf::from(&request.input_path);
        match fs::metadata(&source_path).await {
            Ok(meta) => {
                if let Some(expected_size) = request.validated_file_size {
                    if meta.len() != expected_size {
                        return Err(AppError::ValidationError(format!(
                            "Input file has changed since validation (size: {} → {}). Please re-validate before analysis.",
                            expected_size, meta.len()
                        )));
                    }
                }
                if let Some(ref expected_mtime) = request.validated_file_modified_at {
                    let actual_mtime_str = file_mtime_fingerprint(&meta);
                    if actual_mtime_str.as_deref() != Some(expected_mtime.as_str()) {
                        return Err(AppError::ValidationError(
                            "Input file has been modified since validation. Please re-validate before analysis.".to_string()
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(AppError::FileError(format!(
                    "Cannot access input file for fingerprint verification: {}", e
                )));
            }
        }
    }

    // Create progress reporter
    let reporter = TauriProgressReporter::new(window.clone(), state.cancel_flag.clone());

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

        // Always copy if source differs from destination.
        // Comparing file sizes is fast and catches most cases where the input
        // changed between analyses (e.g., user re-aligned with different params).
        let should_copy = if dest.exists() {
            let src_meta = fs::metadata(&source).await.ok();
            let dst_meta = fs::metadata(&dest).await.ok();
            match (src_meta, dst_meta) {
                (Some(s), Some(d)) => s.len() != d.len(),
                _ => true, // If we can't compare, re-copy to be safe
            }
        } else {
            true
        };

        if should_copy {
            fs::copy(&source, &dest)
                .await
                .map_err(|e| AppError::FileError(format!("Failed to copy input file: {}", e)))?;
        }

        // Update project metadata with input file info
        let mut metadata = load_project_metadata(project_path)
            .await
            .map_err(|e| AppError::ProjectError(e.to_string()))?;
        metadata.input_file = Some(InputFileInfo {
            original_path: Some(request.input_path.clone()),
            copied_to_project: true,
            file_name: file_name.clone(),
        });
        save_project_metadata(project_path, &metadata)
            .await
            .map_err(|e| AppError::ProjectError(e.to_string()))?;

        dest.to_string_lossy().to_string()
    } else {
        request.input_path.clone()
    };

    // Check for cancellation after file copy
    if state.is_cancelled() {
        return Err(AppError::Cancelled("Analysis cancelled".to_string()));
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

    // Parse header format (pipe-delimited field names)
    let header_format_vec: Option<Vec<String>> = request.header_format.as_ref().map(|hf| {
        hf.split('|').map(|s| s.to_string()).collect()
    });

    // Parse metadata fields (pipe-delimited to match header_format convention)
    let metadata_fields_vec: Option<Vec<String>> = request.metadata_fields.as_ref().map(|mf| {
        mf.split('|').map(|s| s.trim().to_string()).collect()
    });

    let kmer_length = request.kmer_length;
    let support_threshold = request.support_threshold;
    let query_name = request.query_name.clone();
    let alphabet = Some(request.alphabet.clone());
    let allow_lowercase = request.allow_lowercase;

    reporter.report(ProgressUpdate {
        stage: AnalysisStage::EntropyCalculation,
        current: 30,
        total: 100,
        message: "Computing diversity metrics (entropy, motifs, support)...".to_string(),
        throughput: None,
    });

    // Clone the cancel flag so the blocking thread can check it during computation.
    // The token is passed into AnalysisConfig so the parallel Rayon loops inside
    // get_results_objs can cooperatively check it, enabling sub-second cancellation
    // even mid-entropy-calculation. (Fix 3.3 + 4.8)
    let cancel_flag = state.cancel_flag.clone();

    let (results, _validation_stats) = tokio::task::spawn_blocking(move || {
        let config = DimaConfig::new()
            .with_validation_mode(validation_mode)
            .with_allow_lowercase(allow_lowercase)
            .with_cancel_token(cancel_flag);

        let input_source = InputSource::File(PathBuf::from(&input_path));
        let (results, validation_stats, _perf) = analyze(
            input_source,
            kmer_length,
            support_threshold,
            query_name,
            header_format_vec,
            alphabet,
            None,
            metadata_fields_vec,
            Some(config),
        ).map_err(|e| match e {
            dima_lib::AnalysisError::Cancelled => AppError::Cancelled("Analysis cancelled".to_string()),
            other => AppError::AnalysisError(format!("Analysis failed: {}", other)),
        })?;

        Ok((results, validation_stats))
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Analysis task failed: {}", e)))?
    .map_err(|e: AppError| e)?;

    // Check for cancellation after computation
    if state.is_cancelled() {
        return Err(AppError::Cancelled("Analysis cancelled".to_string()));
    }

    reporter.report(ProgressUpdate {
        stage: AnalysisStage::OutputGeneration,
        current: 90,
        total: 100,
        message: "Saving results...".to_string(),
        throughput: None,
    });

    // Atomic save: write to temp then rename to prevent torn reads
    let results_path = project_path.join("results.json");
    let results_json = serde_json::to_string_pretty(&results)?;

    let tmp_path = results_path.with_extension("json.tmp");
    fs::write(&tmp_path, &results_json).await?;
    fs::rename(&tmp_path, &results_path).await?;

    // NOTE: report_complete() is intentionally NOT called here — it's handled
    // by the outer run_analysis() function after this returns Ok, preventing
    // duplicate Complete events on the frontend. (Fix 2.7)

    // Persist analysis config to project.json for session restoration
    let saved_config = serde_json::json!({
        "kmerLength": request.kmer_length,
        "supportThreshold": request.support_threshold,
        "queryName": request.query_name,
        "alphabet": request.alphabet,
        "headerFormat": request.header_format,
        "metadataFields": request.metadata_fields,
        "validationMode": request.validation_mode,
        "allowLowercase": request.allow_lowercase,
    });
    if let Ok(mut meta) = load_project_metadata(project_path).await {
        meta.config = Some(saved_config);
        if let Err(e) = save_project_metadata(project_path, &meta).await {
            // Analysis succeeded but config didn't save — warn the user so they know
            // their settings may not be restored on next open. (Fix 5.11)
            let warn_msg = format!("Analysis completed but settings could not be saved: {}", e);
            eprintln!("{}", warn_msg);
            let _ = window.emit("analysis-warning", &warn_msg);
        }
    }

    // Collect non-fatal warnings about analysis parameters.
    let mut warnings = Vec::new();
    if support_threshold > results.sequence_count {
        warnings.push(format!(
            "Support threshold ({}) exceeds number of sequences ({}). All positions will be \
             tagged as low-support and rarefaction will always be skipped.",
            support_threshold, results.sequence_count
        ));
    }

    Ok(AnalysisResponse {
        success: true,
        results_path: Some(results_path.to_string_lossy().to_string()),
        sequence_count: results.sequence_count,
        position_count: results.results.len(),
        average_entropy: results.average_entropy,
        highest_entropy_position: results.highest_entropy.position,
        highest_entropy_value: results.highest_entropy.entropy,
        warnings,
    })
}

/// Cancel the current analysis
#[tauri::command]
pub async fn cancel_analysis(state: State<'_, AppState>) -> Result<(), AppError> {
    state.request_cancel();
    Ok(())
}
