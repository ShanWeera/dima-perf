//! Application State Management
//!
//! Manages shared application state across Tauri commands.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state
pub struct AppState {
    /// Flag to signal analysis cancellation
    pub cancel_flag: Arc<AtomicBool>,
    /// Currently running analysis (if any)
    pub current_analysis: RwLock<Option<AnalysisState>>,
}

/// State of an ongoing analysis (reserved for future UI display of running analysis)
#[derive(Clone)]
#[allow(dead_code)]
pub struct AnalysisState {
    pub project_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            current_analysis: RwLock::new(None),
        }
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    /// Request cancellation of the current analysis
    pub fn request_cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Reset the cancellation flag
    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    /// Start tracking an analysis
    pub async fn start_analysis(&self, project_name: String) {
        let mut current = self.current_analysis.write().await;
        *current = Some(AnalysisState {
            project_name,
            started_at: chrono::Utc::now(),
        });
        self.reset_cancel();
    }

    /// Stop tracking the current analysis
    pub async fn stop_analysis(&self) {
        let mut current = self.current_analysis.write().await;
        *current = None;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
