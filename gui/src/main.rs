//! DiMA GUI — GPU-rendered desktop application for diversity motif analysis.
//!
//! Uses egui + eframe + wgpu for native, GPU-accelerated rendering.
//! No WebView, no JavaScript, no IPC serialization overhead.

mod app;
mod charts;
mod error;
mod panels;
mod state;
mod theme;
mod util;
mod views;
mod workers;

use app::DimaApp;

fn main() -> eframe::Result {
    // Initialize tracing for structured logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_drag_and_drop(true)
            .with_title("DiMA GUI"),
        // Persist window size/position across sessions
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "DiMA GUI",
        native_options,
        Box::new(|cc| Ok(Box::new(DimaApp::new(cc)))),
    )
}
