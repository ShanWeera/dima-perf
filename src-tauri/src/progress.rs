//! Progress Reporting for Analysis
//!
//! Implements a progress reporter that emits events to the Tauri frontend.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};

/// Analysis stages for progress reporting
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum AnalysisStage {
    ReadingFasta,
    KmerExtraction,
    EntropyCalculation, // Reserved for future granular progress
    OutputGeneration,
    Complete,
}

impl std::fmt::Display for AnalysisStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisStage::ReadingFasta => write!(f, "Reading FASTA"),
            AnalysisStage::KmerExtraction => write!(f, "K-mer Extraction"),
            AnalysisStage::EntropyCalculation => write!(f, "Entropy Calculation"),
            AnalysisStage::OutputGeneration => write!(f, "Output Generation"),
            AnalysisStage::Complete => write!(f, "Complete"),
        }
    }
}

/// Progress update sent to the frontend
#[derive(Debug, Clone, Serialize)]
pub struct ProgressUpdate {
    pub stage: AnalysisStage,
    pub current: usize,
    pub total: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<f64>,
}

/// Progress reporter that emits events to Tauri
pub struct TauriProgressReporter {
    window: Window,
    /// Reserved for future cancellation support within progress reporter
    #[allow(dead_code)]
    cancel_flag: Arc<AtomicBool>,
}

impl TauriProgressReporter {
    /// Create a new progress reporter
    pub fn new(window: Window, cancel_flag: Arc<AtomicBool>) -> Self {
        Self {
            window,
            cancel_flag,
        }
    }

    /// Report progress to the frontend. Logs emit failures (e.g. window
    /// closed mid-analysis) rather than silently swallowing them. (Fix 5.10)
    pub fn report(&self, update: ProgressUpdate) {
        if let Err(e) = self.window.emit("analysis-progress", &update) {
            eprintln!("Failed to emit progress event ({}): {}", update.stage, e);
        }
    }

    /// Check if the analysis should be cancelled (reserved for future use)
    #[allow(dead_code)]
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    /// Report a specific stage with progress (reserved for future granular reporting)
    #[allow(dead_code)]
    pub fn report_stage(&self, stage: AnalysisStage, current: usize, total: usize) {
        self.report(ProgressUpdate {
            stage: stage.clone(),
            current,
            total,
            message: stage.to_string(),
            throughput: None,
        });
    }

    /// Report completion
    pub fn report_complete(&self) {
        self.report(ProgressUpdate {
            stage: AnalysisStage::Complete,
            current: 100,
            total: 100,
            message: "Analysis complete".to_string(),
            throughput: None,
        });
    }
}
