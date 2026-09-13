//! Theme system for DiMA GUI.
//!
//! Provides centralized design tokens, light/dark palettes, and theme
//! application to egui's style system. All UI code references `DesignTokens`,
//! never hardcoded colors.

pub mod palette;
pub mod tokens;

pub use palette::{apply_theme, init_both_theme_styles, Theme};
pub use tokens::DesignTokens;
