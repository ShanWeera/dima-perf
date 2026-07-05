//! Application State Management
//!
//! Manages shared application state across Tauri commands.
//! Provides a single-analysis mutex to prevent overlapping runs,
//! a cooperative cancellation flag, a shared HTTP client, and RAII-style analysis tracking.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared application state.
/// Fields are `pub(crate)` — accessible within the dima-desktop crate but not
/// externally. This prevents accidental coupling from external consumers. (Fix 4.27)
pub struct AppState {
    /// Flag to signal analysis cancellation (checked cooperatively)
    pub(crate) cancel_flag: Arc<AtomicBool>,
    /// Flag to signal validation cancellation (separate from analysis since
    /// validation and analysis can overlap or run independently). (Fix 4.29)
    pub(crate) validation_cancel_flag: Arc<AtomicBool>,
    /// Currently running analysis metadata (if any)
    pub(crate) current_analysis: RwLock<Option<AnalysisState>>,
    /// Mutex preventing concurrent analyses — only one can run at a time
    pub(crate) analysis_mutex: Mutex<()>,
    /// Shared HTTP client with timeouts for external API calls (PDB, UniProt)
    pub(crate) http_client: reqwest::Client,
    /// Paths queued for opening on cold-start before the frontend listener mounts.
    /// The frontend pulls from this via `take_pending_open_paths` on mount. (Fix 4.42)
    pub(crate) pending_open_paths: std::sync::Mutex<Vec<PathBuf>>,
}

/// State of an ongoing analysis
#[derive(Clone)]
#[allow(dead_code)]
pub struct AnalysisState {
    /// The query/sequence name being analyzed (not the project name)
    pub query_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub fn new() -> Self {
        // Fail hard if the HTTP client can't be built — this indicates a systemic TLS/config
        // issue. Falling back to default() would silently lose the configured timeouts,
        // allowing PDB/UniProt requests to hang indefinitely.
        let http_client = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .user_agent(concat!("DiMA-Desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to build HTTP client — TLS or system configuration error");

        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            validation_cancel_flag: Arc::new(AtomicBool::new(false)),
            current_analysis: RwLock::new(None),
            analysis_mutex: Mutex::new(()),
            http_client,
            pending_open_paths: std::sync::Mutex::new(Vec::new()),
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

    /// Request cancellation of the current validation task (Fix 4.29)
    pub fn request_validation_cancel(&self) {
        self.validation_cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Reset the validation cancellation flag (called at start of new validation)
    pub fn reset_validation_cancel(&self) {
        self.validation_cancel_flag.store(false, Ordering::SeqCst);
    }

    /// Start tracking an analysis. Resets cancel flag.
    pub async fn start_analysis(&self, query_name: String) {
        let mut current = self.current_analysis.write().await;
        *current = Some(AnalysisState {
            query_name,
            started_at: chrono::Utc::now(),
        });
        self.reset_cancel();
    }

    /// Stop tracking the current analysis and reset the cancel flag.
    /// Resetting the flag here prevents a stale `true` from lingering
    /// in the idle state (between stop and the next start_analysis).
    pub async fn stop_analysis(&self) {
        let mut current = self.current_analysis.write().await;
        *current = None;
        self.reset_cancel();
    }

    /// Best-effort synchronous cleanup for use in Drop impls.
    /// Falls back to try_write() to avoid blocking the async runtime.
    /// If the lock is contended (shouldn't be under normal operation), the
    /// stale state is cleaned up on the next `start_analysis` call.
    pub fn stop_analysis_sync(&self) {
        if let Ok(mut current) = self.current_analysis.try_write() {
            *current = None;
        }
        self.reset_cancel();
    }

    /// Check if an analysis is currently running
    #[allow(dead_code)]
    pub async fn is_analyzing(&self) -> bool {
        self.current_analysis.read().await.is_some()
    }

    /// Queue a file path for the frontend to pick up on mount.
    /// Used during cold-start when the event listener isn't ready yet.
    pub fn push_pending_open_path(&self, path: PathBuf) {
        if let Ok(mut paths) = self.pending_open_paths.lock() {
            paths.push(path);
        }
    }

    /// Atomically drain all pending paths. Each path can only be consumed once,
    /// preventing double-open races. Called by the frontend on mount.
    pub fn take_pending_open_paths(&self) -> Vec<PathBuf> {
        match self.pending_open_paths.lock() {
            Ok(mut paths) => paths.drain(..).collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
