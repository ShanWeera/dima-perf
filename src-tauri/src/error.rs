//! Error types for DiMA Desktop
//!
//! Defines a structured error enum that serializes to a tagged JSON object for
//! the frontend. Tauri 2 commands return `Result<T, AppError>` directly; the
//! `Serialize` derive provides the `Into<InvokeError>` impl automatically.
//! (Fix 5.1)

use serde::Serialize;
use std::path::Path;
use thiserror::Error;

/// Application-level errors that can occur during operation.
/// Serialized as `{ type: "<Variant>", message: "<detail>" }` for the frontend.
#[derive(Error, Debug, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum AppError {
    #[error("File error: {0}")]
    FileError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Analysis error: {0}")]
    AnalysisError(String),

    #[error("Project error: {0}")]
    ProjectError(String),

    #[error("Export error: {0}")]
    ExportError(String),

    #[error("Settings error: {0}")]
    SettingsError(String),

    #[error("Path security violation: {0}")]
    PathSecurity(String),

    #[error("Operation cancelled")]
    Cancelled(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Request timed out: {0}")]
    Timeout(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::FileError(sanitize_error_message(&err.to_string()))
    }
}

/// Cached home directory string for path sanitization.
/// Using `OnceLock` avoids repeated syscalls and external dependencies.
static HOME_DIR: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

fn get_home_dir() -> &'static Option<String> {
    HOME_DIR.get_or_init(|| {
        std::env::var("HOME")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())
    })
}

/// Strip absolute path prefixes from error messages to avoid leaking
/// the user's full directory structure to the frontend. Replaces the
/// home-directory portion with `~` and otherwise keeps only the final
/// path component for other absolute paths that aren't under $HOME.
pub fn sanitize_error_message(msg: &str) -> String {
    let mut result = msg.to_string();

    // Phase 1: Replace $HOME/… with ~/…
    if let Some(home) = get_home_dir() {
        result = result.replace(home.as_str(), "~");
    }

    // Phase 2: For any remaining absolute paths (multi-component), keep
    // only the filename. We scan for slash-prefixed paths with at least one
    // internal separator so we don't false-positive on short strings like "/dev".
    let mut sanitized = String::with_capacity(result.len());
    let chars: Vec<char> = result.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Detect start of an absolute path (Unix `/` with at least one more `/` ahead)
        if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] != ' ' {
            let start = i;
            let mut j = i + 1;
            let mut has_separator = false;
            while j < chars.len() && !chars[j].is_whitespace() && chars[j] != ':' {
                if chars[j] == '/' { has_separator = true; }
                j += 1;
            }
            if has_separator {
                // Extract just the filename component
                let path_str: String = chars[start..j].iter().collect();
                if let Some(name) = Path::new(&path_str).file_name() {
                    sanitized.push_str(&name.to_string_lossy());
                } else {
                    sanitized.push_str(&path_str);
                }
                i = j;
                continue;
            }
        }
        sanitized.push(chars[i]);
        i += 1;
    }

    sanitized
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::InternalError(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AppError::Timeout(err.to_string())
        } else if err.is_connect() {
            AppError::NetworkError(format!("Connection failed: {}", err))
        } else {
            AppError::NetworkError(err.to_string())
        }
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(err: tokio::task::JoinError) -> Self {
        AppError::InternalError(format!("Background task failed: {}", err))
    }
}

/// Result type alias for AppError
pub type AppResult<T> = Result<T, AppError>;
