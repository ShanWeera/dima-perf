//! Project Management Commands
//!
//! Tauri commands for creating, opening, and managing projects.

use crate::project::{
    self, add_to_recent_projects, create_new_project, delete_project_folder,
    load_project_metadata, load_recent_projects, RecentProject,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Response from creating a project
#[derive(Debug, Serialize)]
pub struct CreateProjectResponse {
    pub path: String,
    pub name: String,
}

/// Create a new project
#[tauri::command]
pub async fn create_project(name: String) -> Result<CreateProjectResponse, String> {
    let project_path = create_new_project(&name)
        .await
        .map_err(|e| e.to_string())?;

    // Add to recent projects
    let recent = RecentProject {
        name: name.clone(),
        path: project_path.to_string_lossy().to_string(),
        last_opened: Utc::now(),
        input_file_name: None,
        sequence_count: None,
    };
    add_to_recent_projects(recent)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CreateProjectResponse {
        path: project_path.to_string_lossy().to_string(),
        name,
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
}

/// Open an existing project
#[tauri::command]
pub async fn open_project(path: String) -> Result<ProjectInfo, String> {
    let project_path = PathBuf::from(&path);

    if !project_path.exists() {
        return Err("Project folder does not exist".to_string());
    }

    let metadata = load_project_metadata(&project_path)
        .await
        .map_err(|e| e.to_string())?;

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
        .map_err(|e| e.to_string())?;

    Ok(ProjectInfo {
        name: metadata.name,
        path,
        created_at: metadata.created_at.to_rfc3339(),
        has_results,
        has_input_file,
        input_file_name,
    })
}

/// List recent projects
#[tauri::command]
pub async fn list_recent_projects() -> Result<Vec<RecentProject>, String> {
    load_recent_projects().await.map_err(|e| e.to_string())
}

/// Delete a project
#[tauri::command]
pub async fn delete_project(path: String) -> Result<(), String> {
    let project_path = PathBuf::from(&path);
    delete_project_folder(&project_path)
        .await
        .map_err(|e| e.to_string())
}

/// Clear all recent projects from the list
#[tauri::command]
pub async fn clear_recent_projects() -> Result<(), String> {
    project::clear_all_recent_projects()
        .await
        .map_err(|e| e.to_string())
}

/// Get the path to a project folder
#[tauri::command]
pub async fn get_project_path(name: String) -> Result<String, String> {
    let projects_path = project::get_projects_path().map_err(|e| e.to_string())?;
    let project_path = projects_path.join(&name);
    Ok(project_path.to_string_lossy().to_string())
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

/// Save dashboard layout to project
#[tauri::command]
pub async fn save_layout(project_path: String, layout: DashboardLayout) -> Result<(), String> {
    let path = PathBuf::from(&project_path).join("layout.json");
    let content = serde_json::to_string_pretty(&layout)
        .map_err(|e| format!("Failed to serialize layout: {}", e))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to save layout: {}", e))?;
    Ok(())
}

/// Load dashboard layout from project
#[tauri::command]
pub async fn load_layout(project_path: String) -> Result<Option<DashboardLayout>, String> {
    let path = PathBuf::from(&project_path).join("layout.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read layout: {}", e))?;
    let layout: DashboardLayout = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse layout: {}", e))?;
    Ok(Some(layout))
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

/// Save annotations to project
#[tauri::command]
pub async fn save_annotations(project_path: String, annotations: Vec<Annotation>) -> Result<(), String> {
    let path = PathBuf::from(&project_path).join("annotations.json");
    let data = AnnotationsData { annotations };
    let content = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize annotations: {}", e))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to save annotations: {}", e))?;
    Ok(())
}

/// Load annotations from project
#[tauri::command]
pub async fn load_annotations(project_path: String) -> Result<Vec<Annotation>, String> {
    let path = PathBuf::from(&project_path).join("annotations.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read annotations: {}", e))?;
    let data: AnnotationsData = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse annotations: {}", e))?;
    Ok(data.annotations)
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

/// Save filters to project
#[tauri::command]
pub async fn save_filters(project_path: String, filters: SearchFilters) -> Result<(), String> {
    let path = PathBuf::from(&project_path).join("filters.json");
    let content = serde_json::to_string_pretty(&filters)
        .map_err(|e| format!("Failed to serialize filters: {}", e))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to save filters: {}", e))?;
    Ok(())
}

/// Load filters from project
#[tauri::command]
pub async fn load_filters(project_path: String) -> Result<Option<SearchFilters>, String> {
    let path = PathBuf::from(&project_path).join("filters.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read filters: {}", e))?;
    let filters: SearchFilters = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse filters: {}", e))?;
    Ok(Some(filters))
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

/// Save global filter presets
#[tauri::command]
pub async fn save_filter_presets(presets: Vec<FilterPreset>) -> Result<(), String> {
    let base_path = project::get_app_base_path().map_err(|e| e.to_string())?;
    let path = base_path.join("filter-presets.json");
    let data = FilterPresetsData { presets };
    let content = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize filter presets: {}", e))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to save filter presets: {}", e))?;
    Ok(())
}

/// Load global filter presets
#[tauri::command]
pub async fn load_filter_presets() -> Result<Vec<FilterPreset>, String> {
    let base_path = project::get_app_base_path().map_err(|e| e.to_string())?;
    let path = base_path.join("filter-presets.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read filter presets: {}", e))?;
    let data: FilterPresetsData = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse filter presets: {}", e))?;
    Ok(data.presets)
}
