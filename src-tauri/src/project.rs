//! Project Management
//!
//! Handles project creation, storage, and file operations.

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;
use tokio::fs;

/// Name of the application data folder
const APP_FOLDER_NAME: &str = "DiMA Desktop";

/// Project metadata stored in project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub input_file: Option<InputFileInfo>,
    pub config: Option<serde_json::Value>,
}

/// Information about the input file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFileInfo {
    pub original_path: String,
    pub copied_to_project: bool,
    pub file_name: String,
}

/// Recent project entry for the sidebar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: String,
    pub last_opened: DateTime<Utc>,
    pub input_file_name: Option<String>,
    pub sequence_count: Option<usize>,
}

/// Get the base path for DiMA Desktop data
pub fn get_app_base_path() -> AppResult<PathBuf> {
    let user_dirs = UserDirs::new()
        .ok_or_else(|| AppError::ProjectError("Could not determine user directories".into()))?;

    let documents = user_dirs
        .document_dir()
        .ok_or_else(|| AppError::ProjectError("Could not find Documents folder".into()))?;

    Ok(documents.join(APP_FOLDER_NAME))
}

/// Get the projects directory path
pub fn get_projects_path() -> AppResult<PathBuf> {
    Ok(get_app_base_path()?.join("Projects"))
}

/// Ensure all required app directories exist
pub async fn ensure_app_directories(_app: &AppHandle) -> AppResult<()> {
    let base_path = get_app_base_path()?;
    let projects_path = get_projects_path()?;

    fs::create_dir_all(&base_path).await?;
    fs::create_dir_all(&projects_path).await?;

    Ok(())
}

/// Create a new project
pub async fn create_new_project(name: &str) -> AppResult<PathBuf> {
    let projects_path = get_projects_path()?;
    
    // Sanitize project name for filesystem
    let safe_name = sanitize_project_name(name);
    let project_path = projects_path.join(&safe_name);

    // Check if project already exists
    if project_path.exists() {
        return Err(AppError::ProjectError(format!(
            "Project '{}' already exists",
            name
        )));
    }

    // Create project directory
    fs::create_dir_all(&project_path).await?;

    // Create project metadata
    let metadata = ProjectMetadata {
        name: name.to_string(),
        created_at: Utc::now(),
        input_file: None,
        config: None,
    };

    // Save metadata
    let metadata_path = project_path.join("project.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_path, metadata_json).await?;

    Ok(project_path)
}

/// Sanitize a project name for use as a folder name
fn sanitize_project_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Load project metadata
pub async fn load_project_metadata(project_path: &PathBuf) -> AppResult<ProjectMetadata> {
    let metadata_path = project_path.join("project.json");
    let content = fs::read_to_string(&metadata_path).await?;
    let metadata: ProjectMetadata = serde_json::from_str(&content)?;
    Ok(metadata)
}

/// Save project metadata
pub async fn save_project_metadata(
    project_path: &PathBuf,
    metadata: &ProjectMetadata,
) -> AppResult<()> {
    let metadata_path = project_path.join("project.json");
    let content = serde_json::to_string_pretty(metadata)?;
    fs::write(&metadata_path, content).await?;
    Ok(())
}

/// Load recent projects list
pub async fn load_recent_projects() -> AppResult<Vec<RecentProject>> {
    let base_path = get_app_base_path()?;
    let recent_path = base_path.join("recent-projects.json");

    if !recent_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&recent_path).await?;
    let projects: Vec<RecentProject> = serde_json::from_str(&content).unwrap_or_default();
    Ok(projects)
}

/// Save recent projects list
pub async fn save_recent_projects(projects: &[RecentProject]) -> AppResult<()> {
    let base_path = get_app_base_path()?;
    let recent_path = base_path.join("recent-projects.json");
    let content = serde_json::to_string_pretty(projects)?;
    fs::write(&recent_path, content).await?;
    Ok(())
}

/// Add a project to the recent list
pub async fn add_to_recent_projects(project: RecentProject) -> AppResult<()> {
    let mut projects = load_recent_projects().await?;

    // Remove if already exists
    projects.retain(|p| p.path != project.path);

    // Add to front
    projects.insert(0, project);

    // Save
    save_recent_projects(&projects).await?;
    Ok(())
}

/// Delete a project
pub async fn delete_project_folder(project_path: &PathBuf) -> AppResult<()> {
    if project_path.exists() {
        fs::remove_dir_all(project_path).await?;
    }

    // Remove from recent projects
    let mut projects = load_recent_projects().await?;
    let path_str = project_path.to_string_lossy().to_string();
    projects.retain(|p| p.path != path_str);
    save_recent_projects(&projects).await?;

    Ok(())
}

/// Clear all recent projects from the list (does not delete project files)
pub async fn clear_all_recent_projects() -> AppResult<()> {
    save_recent_projects(&[]).await
}
