//! Project Management
//!
//! Handles project creation, storage, and file operations.
//! Includes path security validation, atomic writes, and sanitization collision prevention.

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::Mutex;

// Guards concurrent access to recent-projects.json to prevent lost-update
// races when multiple project opens/creates happen rapidly. (Fix 4.24)
static RECENT_PROJECTS_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

/// Name of the application data folder
const APP_FOLDER_NAME: &str = "DiMA Desktop";

/// Maximum project name length (filesystem friendly)
const MAX_PROJECT_NAME_LENGTH: usize = 100;

/// Maximum number of recent projects to track
const MAX_RECENT_PROJECTS: usize = 50;

/// Current schema version for project.json.
/// Increment when the ProjectMetadata structure changes.
const PROJECT_SCHEMA_VERSION: u32 = 1;

/// Project metadata stored in project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Schema version for forward-compatible deserialization
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub input_file: Option<InputFileInfo>,
    pub config: Option<serde_json::Value>,
}

fn default_schema_version() -> u32 {
    1
}

/// Information about the input file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFileInfo {
    pub file_name: String,
    pub copied_to_project: bool,
    /// Only store filename, not full system path (avoid information disclosure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
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

/// Get the base path for DiMA Desktop data.
/// Falls back to Tauri's app_data_dir equivalent if Documents is unavailable (Linux without ~/Documents).
pub fn get_app_base_path() -> AppResult<PathBuf> {
    let user_dirs = UserDirs::new()
        .ok_or_else(|| AppError::ProjectError("Could not determine user directories".into()))?;

    // Try Documents first, then fall back to home directory
    let base_dir = user_dirs
        .document_dir()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| {
            // Fallback: use home directory on Linux without ~/Documents
            user_dirs.home_dir().to_path_buf()
        });

    Ok(base_dir.join(APP_FOLDER_NAME))
}

/// Get the projects directory path
pub fn get_projects_path() -> AppResult<PathBuf> {
    Ok(get_app_base_path()?.join("Projects"))
}

/// Convert a file's modification time to a stable fingerprint string (epoch seconds).
/// Used by validation and analysis to detect TOCTOU file changes.
/// Returns None if the OS cannot provide a modification time.
///
/// Uses seconds granularity — platform-safe across APFS, ext4, and NTFS.
/// Both callers use the same function to guarantee format parity.
pub fn file_mtime_fingerprint(metadata: &std::fs::Metadata) -> Option<String> {
    metadata.modified().ok().map(|t| {
        let secs = t
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        secs.to_string()
    })
}

/// Validate that a path is confined within the expected base directory.
/// Prevents path traversal attacks (e.g., "../../etc/passwd" as project name).
///
/// Security: if canonicalization fails for the path (it doesn't exist yet),
/// we canonicalize the parent directory instead and verify confinement there.
/// Falling back to unchecked paths would allow `..` segment bypasses.
pub fn validate_path_confinement(path: &Path, expected_base: &Path) -> AppResult<()> {
    // Canonicalize the base directory — it must exist
    let canonical_base = expected_base.canonicalize().map_err(|e| {
        AppError::ProjectError(format!(
            "Base directory does not exist or is inaccessible: {}",
            e
        ))
    })?;

    // Try to canonicalize the target path directly
    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Path doesn't exist yet (e.g., creating a new project).
            // Canonicalize the parent directory and append the filename.
            let parent = path
                .parent()
                .ok_or_else(|| AppError::ProjectError("Path has no parent directory".into()))?;
            let file_name = path
                .file_name()
                .ok_or_else(|| AppError::ProjectError("Path has no file name component".into()))?;

            // The parent must exist and be canonicalizable
            let canonical_parent = parent.canonicalize().map_err(|e| {
                AppError::ProjectError(format!(
                    "Parent directory does not exist or is inaccessible: {}",
                    e
                ))
            })?;
            canonical_parent.join(file_name)
        }
    };

    if !canonical_path.starts_with(&canonical_base) {
        return Err(AppError::ProjectError(
            "Path traversal detected: path is outside the allowed directory".into(),
        ));
    }
    Ok(())
}

/// Ensure all required app directories exist (synchronous, for use in setup)
pub fn ensure_app_directories_sync() -> AppResult<()> {
    let base_path = get_app_base_path()?;
    let projects_path = get_projects_path()?;

    std::fs::create_dir_all(&base_path)?;
    std::fs::create_dir_all(&projects_path)?;

    Ok(())
}

/// Create a new project
pub async fn create_new_project(name: &str) -> AppResult<PathBuf> {
    let projects_path = get_projects_path()?;

    // Sanitize project name for filesystem
    let safe_name = sanitize_project_name(name);

    if safe_name.is_empty() {
        return Err(AppError::ProjectError(
            "Project name cannot be empty or contain only special characters".into(),
        ));
    }

    let mut project_path = projects_path.join(&safe_name);

    // Handle collisions by appending a hash suffix
    if project_path.exists() {
        let hash = &format!("{:x}", fnv1a_hash(name))[..6];
        let collision_name = format!("{}_{}", safe_name, hash);
        project_path = projects_path.join(&collision_name);

        if project_path.exists() {
            return Err(AppError::ProjectError(format!(
                "Project '{}' already exists",
                name
            )));
        }
    }

    // Validate confinement before creating
    validate_path_confinement(&project_path, &projects_path)?;

    // Use create_dir (NOT create_dir_all) so the operation is atomic:
    // if two concurrent calls race, only one will succeed — the other gets
    // AlreadyExists. This prevents double-creation overwriting metadata.
    match fs::create_dir(&project_path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AppError::ProjectError(format!(
                "Project '{}' was just created by another operation",
                project_path.display()
            )));
        }
        Err(e) => return Err(e.into()),
    }

    // Store the sanitized name in metadata so the display name matches the
    // folder name on disk. Using the raw input can cause name/folder divergence
    // and collisions when two different raw names sanitize identically. (Fix 4.25)
    let display_name = project_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| safe_name.clone());

    let metadata = ProjectMetadata {
        schema_version: PROJECT_SCHEMA_VERSION,
        name: display_name,
        created_at: Utc::now(),
        input_file: None,
        config: None,
    };

    // Save metadata atomically
    write_json_atomic(&project_path.join("project.json"), &metadata).await?;

    Ok(project_path)
}

/// Windows reserved device names that cannot be used as folder names.
/// Includes COM1-9, LPT1-9, CON, PRN, AUX, NUL (case-insensitive).
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Sanitize a project name for use as a folder name.
/// Strips BiDi control characters, replaces unsafe chars, rejects Windows reserved
/// names, strips trailing dots/spaces (invalid on Windows), and enforces length limits.
pub(crate) fn sanitize_project_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|c| !c.is_control() && !is_bidi_control(*c))
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        // Windows rejects trailing dots and spaces on directory names
        .trim_end_matches(['.', ' '])
        .to_string();

    // Enforce maximum length using char boundaries
    let truncated = if sanitized.chars().count() > MAX_PROJECT_NAME_LENGTH {
        sanitized
            .chars()
            .take(MAX_PROJECT_NAME_LENGTH)
            .collect::<String>()
            .trim_end()
            .to_string()
    } else {
        sanitized
    };

    // Reject Windows reserved device names (case-insensitive, with or without extension)
    let stem_upper = truncated.split('.').next().unwrap_or("").to_uppercase();
    let is_reserved = WINDOWS_RESERVED.contains(&stem_upper.as_str());

    // Prevent names that are all underscores/spaces (from emoji-only inputs)
    let meaningful: String = truncated.chars().filter(|c| c.is_alphanumeric()).collect();
    if meaningful.is_empty() || is_reserved {
        if is_reserved {
            format!("{}_project", truncated)
        } else {
            format!("project_{}", Utc::now().format("%Y%m%d_%H%M%S"))
        }
    } else {
        truncated
    }
}

/// Check if a character is a BiDi control character
fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

/// FNV-1a hash for collision avoidance (not cryptographic).
/// Used to generate unique suffixes when project names collide after sanitization.
fn fnv1a_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    hash
}

/// Load project metadata
pub async fn load_project_metadata(project_path: &Path) -> AppResult<ProjectMetadata> {
    let metadata_path = project_path.join("project.json");
    let content = fs::read_to_string(&metadata_path).await?;
    let metadata: ProjectMetadata = serde_json::from_str(&content)?;
    Ok(metadata)
}

/// Save project metadata atomically
pub async fn save_project_metadata(
    project_path: &Path,
    metadata: &ProjectMetadata,
) -> AppResult<()> {
    write_json_atomic(&project_path.join("project.json"), metadata).await
}

/// Write JSON to a file atomically (write to tmp, then rename).
/// Prevents data corruption from crashes or power loss during write.
pub async fn write_json_atomic<T: Serialize>(path: &Path, data: &T) -> AppResult<()> {
    let content = serde_json::to_string_pretty(data)?;
    let tmp_path = path.with_extension("json.tmp");

    fs::write(&tmp_path, &content).await?;
    fs::rename(&tmp_path, path).await?;

    Ok(())
}

/// Load recent projects list.
/// Returns empty list (not error) if file is missing or corrupt.
pub async fn load_recent_projects() -> AppResult<Vec<RecentProject>> {
    let base_path = get_app_base_path()?;
    let recent_path = base_path.join("recent-projects.json");

    if !recent_path.exists() {
        return Ok(Vec::new());
    }

    let content = match fs::read_to_string(&recent_path).await {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    match serde_json::from_str::<Vec<RecentProject>>(&content) {
        Ok(projects) => {
            // Filter out projects whose directories no longer exist
            let valid_projects: Vec<RecentProject> = projects
                .into_iter()
                .filter(|p| Path::new(&p.path).exists())
                .take(MAX_RECENT_PROJECTS)
                .collect();
            Ok(valid_projects)
        }
        Err(_) => {
            // Corrupt file — return empty and it'll be overwritten on next save
            Ok(Vec::new())
        }
    }
}

/// Save recent projects list atomically
pub async fn save_recent_projects(projects: &[RecentProject]) -> AppResult<()> {
    let base_path = get_app_base_path()?;
    let recent_path = base_path.join("recent-projects.json");
    let content = serde_json::to_string_pretty(projects).map_err(|e| {
        AppError::ProjectError(format!("Failed to serialize recent projects: {}", e))
    })?;

    let tmp_path = recent_path.with_extension("json.tmp");
    fs::write(&tmp_path, &content).await?;
    fs::rename(&tmp_path, &recent_path).await?;

    Ok(())
}

/// Add a project to the recent list.
/// Uses a Mutex to prevent lost-update races from concurrent calls. (Fix 4.24)
pub async fn add_to_recent_projects(project: RecentProject) -> AppResult<()> {
    let _guard = RECENT_PROJECTS_LOCK.lock().await;
    let mut projects = load_recent_projects().await?;

    // Remove if already exists (will be re-added at front)
    projects.retain(|p| p.path != project.path);

    // Add to front
    projects.insert(0, project);

    // Enforce maximum count
    projects.truncate(MAX_RECENT_PROJECTS);

    // Save
    save_recent_projects(&projects).await?;
    Ok(())
}

/// Delete a project, with confinement check to prevent deleting arbitrary paths.
/// Also refuses to follow symlinks at the top level to avoid escaping the projects directory.
/// Requires the path to be a STRICT child of the projects directory (not equal to it)
/// and to contain a project.json file as a safety check.
pub async fn delete_project_folder(project_path: &PathBuf) -> AppResult<()> {
    let projects_path = get_projects_path()?;

    // Security: ensure the path is within our Projects directory
    validate_path_confinement(project_path, &projects_path)?;

    // Strict child check: refuse to delete the projects root itself (Fix 7.1c)
    let canonical_target = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.clone());
    let canonical_base = projects_path
        .canonicalize()
        .unwrap_or_else(|_| projects_path.clone());
    if canonical_target == canonical_base {
        return Err(AppError::ProjectError(
            "Cannot delete the Projects root directory".into(),
        ));
    }

    // Safety: require project.json to exist — prevents deleting arbitrary subdirectories
    if !project_path.join("project.json").exists() {
        return Err(AppError::ProjectError(
            "Target does not appear to be a DiMA project (missing project.json)".into(),
        ));
    }

    if project_path.exists() {
        // Refuse to remove a symlink target — only real directories are valid projects
        let metadata = std::fs::symlink_metadata(project_path)
            .map_err(|e| AppError::FileError(e.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::ProjectError(
                "Cannot delete: path is a symlink".into(),
            ));
        }
        fs::remove_dir_all(project_path).await?;
    }

    // Remove from recent projects — acquire lock to prevent lost-update race
    // with concurrent open/create operations that also modify the list.
    let _guard = RECENT_PROJECTS_LOCK.lock().await;
    let mut projects = load_recent_projects().await?;
    let path_str = project_path.to_string_lossy().to_string();
    projects.retain(|p| p.path != path_str);
    save_recent_projects(&projects).await?;

    Ok(())
}

/// Clear all recent projects from the list (does not delete project files).
/// Acquires RECENT_PROJECTS_LOCK to prevent race with concurrent modifications.
pub async fn clear_all_recent_projects() -> AppResult<()> {
    let _guard = RECENT_PROJECTS_LOCK.lock().await;
    save_recent_projects(&[]).await
}
