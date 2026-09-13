//! Background workers for validation and analysis.
//!
//! Uses std::thread + std::sync::mpsc + atomics (no tokio).
//! This is the idiomatic egui pattern for CPU-bound work.
