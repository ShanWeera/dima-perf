//! Self-contained UI surfaces that own their state and report user intent.
//!
//! Panels here never mutate application state directly: they render, and return
//! a value describing what the user chose. The app applies it. That keeps the
//! borrow rules simple in egui's immediate mode and makes each panel testable
//! without an app instance.

pub mod command_palette;

pub use command_palette::{CommandPalette, PaletteAction};
