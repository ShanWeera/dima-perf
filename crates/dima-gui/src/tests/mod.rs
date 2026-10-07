//! Interface tests: shared fixtures and the egui_kittest flows. Every test is hermetic: an
//! in-memory configuration store and temporary directories, never the user's settings, recent
//! files or folders (G15).

pub mod fixtures;
mod flows;
