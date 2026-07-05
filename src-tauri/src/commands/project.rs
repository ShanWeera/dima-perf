//! Project Management Commands
//!
//! Tauri commands for creating, opening, and managing projects.

use crate::error::AppError;
use crate::project::{
    self, add_to_recent_projects, create_new_project, delete_project_folder, load_project_metadata,
    load_recent_projects, validate_path_confinement, RecentProject,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Validates that a project_path string points within the app's Projects directory.
/// Returns the PathBuf on success, or an error message on failure.
async fn validate_project_path(project_path: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(project_path);
    let projects_base =
        project::get_projects_path().map_err(|e| AppError::ProjectError(e.to_string()))?;
    validate_path_confinement(&path, &projects_base)
        .map_err(|e| AppError::ProjectError(e.to_string()))?;
    Ok(path)
}

/// Response from creating a project
#[derive(Debug, Serialize)]
pub struct CreateProjectResponse {
    pub path: String,
    pub name: String,
}

/// Create a new project. Returns the sanitized display name (which matches
/// the directory name) rather than the raw user input. (Fix 4.25)
#[tauri::command]
pub async fn create_project(name: String) -> Result<CreateProjectResponse, AppError> {
    let project_path = create_new_project(&name)
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))?;

    // Read back the metadata name (sanitized, matches folder name)
    let display_name = project_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| name.clone());

    let recent = RecentProject {
        name: display_name.clone(),
        path: project_path.to_string_lossy().to_string(),
        last_opened: Utc::now(),
        input_file_name: None,
        sequence_count: None,
    };
    add_to_recent_projects(recent)
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))?;

    Ok(CreateProjectResponse {
        path: project_path.to_string_lossy().to_string(),
        name: display_name,
    })
}

/// Project info returned when opening a project
#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub has_results: bool,
    pub has_input_file: bool,
    pub input_file_name: Option<String>,
    /// Saved analysis config (alphabet, k-mer length, etc.) for project reopening
    pub config: Option<serde_json::Value>,
}

/// Open an existing project.
/// Validates path confinement to prevent reading arbitrary directories.
#[tauri::command]
pub async fn open_project(path: String) -> Result<ProjectInfo, AppError> {
    let project_path = validate_project_path(&path).await?;

    if !project_path.exists() {
        return Err(AppError::NotFound(
            "Project folder does not exist".to_string(),
        ));
    }
    if !project_path.is_dir() {
        return Err(AppError::ValidationError(
            "The specified path is not a directory. Projects must be folders.".to_string(),
        ));
    }

    let metadata = load_project_metadata(&project_path)
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))?;

    // Check if results exist
    let results_path = project_path.join("results.json");
    let has_results = results_path.exists();

    // Get input file info
    let (has_input_file, input_file_name) = match &metadata.input_file {
        Some(info) => (true, Some(info.file_name.clone())),
        None => (false, None),
    };

    // Update recent projects
    let recent = RecentProject {
        name: metadata.name.clone(),
        path: path.clone(),
        last_opened: Utc::now(),
        input_file_name: input_file_name.clone(),
        sequence_count: None,
    };
    add_to_recent_projects(recent)
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))?;

    Ok(ProjectInfo {
        name: metadata.name,
        path,
        created_at: metadata.created_at.to_rfc3339(),
        has_results,
        has_input_file,
        input_file_name,
        config: metadata.config,
    })
}

/// List recent projects
#[tauri::command]
pub async fn list_recent_projects() -> Result<Vec<RecentProject>, AppError> {
    load_recent_projects()
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))
}

/// Delete a project. Acquires the analysis mutex to prevent deletion while an
/// analysis is starting or running (eliminates the check-then-act TOCTOU race).
#[tauri::command]
pub async fn delete_project(
    path: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<(), AppError> {
    // Hold the analysis mutex during delete to prevent any analysis from
    // starting on this path between our check and the actual deletion.
    let _guard = state.analysis_mutex.try_lock().map_err(|_| {
        AppError::ProjectError(
            "Cannot delete a project while an analysis is running. Cancel the analysis first."
                .to_string(),
        )
    })?;

    let project_path = PathBuf::from(&path);
    delete_project_folder(&project_path)
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))
}

/// Clear all recent projects from the list
#[tauri::command]
pub async fn clear_recent_projects() -> Result<(), AppError> {
    project::clear_all_recent_projects()
        .await
        .map_err(|e| AppError::ProjectError(e.to_string()))
}

// ============================================================================
// Layout Persistence
// ============================================================================

/// Layout item for dashboard grid
#[derive(Debug, Serialize, Deserialize)]
pub struct LayoutItem {
    pub i: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(rename = "minW")]
    pub min_w: Option<i32>,
    #[serde(rename = "minH")]
    pub min_h: Option<i32>,
}

/// Dashboard layout state
#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub layout: Vec<LayoutItem>,
    pub hidden_panels: Vec<String>,
}

/// Save dashboard layout to project using atomic writes (tmp+rename)
/// to prevent data corruption from crashes or concurrent saves. (Fix 4.37)
#[tauri::command]
pub async fn save_layout(project_path: String, layout: DashboardLayout) -> Result<(), AppError> {
    let base = validate_project_path(&project_path).await?;
    let path = base.join("layout.json");
    project::write_json_atomic(&path, &layout)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to save layout: {}", e)))?;
    Ok(())
}

/// Load dashboard layout from project
/// Gracefully returns None on corrupt JSON instead of blocking project load. (Fix 4.38)
#[tauri::command]
pub async fn load_layout(project_path: String) -> Result<Option<DashboardLayout>, AppError> {
    let base = validate_project_path(&project_path).await?;
    let path = base.join("layout.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read layout: {}", e)))?;
    match serde_json::from_str::<DashboardLayout>(&content) {
        Ok(layout) => Ok(Some(layout)),
        Err(e) => {
            eprintln!("Warning: corrupt layout.json, using defaults: {}", e);
            Ok(None)
        }
    }
}

// ============================================================================
// Annotation Persistence
// ============================================================================

/// Annotation color type
pub type AnnotationColor = String;

/// Annotation data
#[derive(Debug, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub position_number: i32,
    pub color: AnnotationColor,
    pub label: String,
    pub note: String,
    pub created_at: String,
}

/// Annotations container
#[derive(Debug, Serialize, Deserialize)]
pub struct AnnotationsData {
    pub annotations: Vec<Annotation>,
}

/// Save annotations to project using atomic writes (Fix 4.37)
#[tauri::command]
pub async fn save_annotations(
    project_path: String,
    annotations: Vec<Annotation>,
) -> Result<(), AppError> {
    let base = validate_project_path(&project_path).await?;
    let path = base.join("annotations.json");
    let data = AnnotationsData { annotations };
    project::write_json_atomic(&path, &data)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to save annotations: {}", e)))?;
    Ok(())
}

/// Load annotations from project. Gracefully returns empty on corrupt JSON. (Fix 4.38)
#[tauri::command]
pub async fn load_annotations(project_path: String) -> Result<Vec<Annotation>, AppError> {
    let base = validate_project_path(&project_path).await?;
    let path = base.join("annotations.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read annotations: {}", e)))?;
    match serde_json::from_str::<AnnotationsData>(&content) {
        Ok(data) => Ok(data.annotations),
        Err(e) => {
            eprintln!("Warning: corrupt annotations.json, using empty: {}", e);
            Ok(Vec::new())
        }
    }
}

// ============================================================================
// Filter Persistence
// ============================================================================

/// Search filters state
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchFilters {
    pub position_from: Option<i32>,
    pub position_to: Option<i32>,
    pub sequence_query: String,
    pub entropy_min: Option<f64>,
    pub entropy_max: Option<f64>,
    pub motif_types: Vec<String>,
    pub include_low_support: bool,
}

/// Save filters to project using atomic writes (Fix 4.37)
#[tauri::command]
pub async fn save_filters(project_path: String, filters: SearchFilters) -> Result<(), AppError> {
    let base = validate_project_path(&project_path).await?;
    let path = base.join("filters.json");
    project::write_json_atomic(&path, &filters)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to save filters: {}", e)))?;
    Ok(())
}

/// Load filters from project. Gracefully returns None on corrupt JSON. (Fix 4.38)
#[tauri::command]
pub async fn load_filters(project_path: String) -> Result<Option<SearchFilters>, AppError> {
    let base = validate_project_path(&project_path).await?;
    let path = base.join("filters.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read filters: {}", e)))?;
    match serde_json::from_str::<SearchFilters>(&content) {
        Ok(filters) => Ok(Some(filters)),
        Err(e) => {
            eprintln!("Warning: corrupt filters.json, using defaults: {}", e);
            Ok(None)
        }
    }
}

/// Filter preset
#[derive(Debug, Serialize, Deserialize)]
pub struct FilterPreset {
    pub id: String,
    pub name: String,
    pub filters: SearchFilters,
}

/// Global filter presets container
#[derive(Debug, Serialize, Deserialize)]
pub struct FilterPresetsData {
    pub presets: Vec<FilterPreset>,
}

/// Save global filter presets using atomic writes (Fix 4.37)
#[tauri::command]
pub async fn save_filter_presets(presets: Vec<FilterPreset>) -> Result<(), AppError> {
    let base_path =
        project::get_app_base_path().map_err(|e| AppError::ProjectError(e.to_string()))?;
    let path = base_path.join("filter-presets.json");
    let data = FilterPresetsData { presets };
    project::write_json_atomic(&path, &data)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to save filter presets: {}", e)))?;
    Ok(())
}

/// Load global filter presets
#[tauri::command]
pub async fn load_filter_presets() -> Result<Vec<FilterPreset>, AppError> {
    let base_path =
        project::get_app_base_path().map_err(|e| AppError::ProjectError(e.to_string()))?;
    let path = base_path.join("filter-presets.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read filter presets: {}", e)))?;
    let data: FilterPresetsData = serde_json::from_str(&content)
        .map_err(|e| AppError::InternalError(format!("Failed to parse filter presets: {}", e)))?;
    Ok(data.presets)
}

/// Atomically drain all file paths queued during cold-start before the frontend mounted.
/// The frontend calls this once on mount to handle file-association launches without
/// timing races. Each path can only be consumed once. (Fix 4.42)
#[tauri::command]
pub fn take_pending_open_paths(state: tauri::State<'_, crate::state::AppState>) -> Vec<String> {
    state
        .take_pending_open_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Load analysis results from a project directory.
/// Validates that the file exists and is valid JSON before returning.
#[tauri::command]
pub async fn load_results(project_path: String) -> Result<serde_json::Value, AppError> {
    let base = validate_project_path(&project_path).await?;
    let results_path = base.join("results.json");
    if !results_path.exists() {
        return Err(AppError::NotFound(
            "Results file not found. Has the analysis completed?".to_string(),
        ));
    }
    // Cap results file size before reading to prevent OOM from extremely large
    // analyses or corrupt files. 500 MB is generous for any practical analysis. (Fix 4.23)
    const MAX_RESULTS_SIZE: u64 = 500 * 1024 * 1024;
    let metadata = tokio::fs::metadata(&results_path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read results metadata: {}", e)))?;
    if metadata.len() > MAX_RESULTS_SIZE {
        return Err(AppError::FileError(format!(
            "Results file is too large ({:.1} MB, max {} MB). Consider exporting to .dima format.",
            metadata.len() as f64 / (1024.0 * 1024.0),
            MAX_RESULTS_SIZE / (1024 * 1024)
        )));
    }
    let content = tokio::fs::read_to_string(&results_path)
        .await
        .map_err(|e| AppError::FileError(format!("Failed to read results file: {}", e)))?;
    let parsed: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        AppError::InternalError(format!("Results file is corrupted or invalid: {}", e))
    })?;
    Ok(parsed)
}
