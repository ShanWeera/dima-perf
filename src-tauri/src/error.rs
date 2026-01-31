//! Error types for DiMA Desktop
//!
//! Defines custom error types that can be serialized and sent to the frontend.

use serde::Serialize;
use thiserror::Error;

/// Application-level errors that can occur during operation
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

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::FileError(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::InternalError(err.to_string())
    }
}

/// Result type alias for AppError
pub type AppResult<T> = Result<T, AppError>;

/// Convert AppError to a string for Tauri command results
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}
