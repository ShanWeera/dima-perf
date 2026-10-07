//! Interface pieces. A panel draws from the state, edits form values in place and reports
//! every other intent as a `Msg`; it never starts work or touches files itself.

pub mod analysis_options;
pub mod command_palette;
pub mod common;
pub mod confirm;
pub mod diagnostics;
pub mod entropy_chart;
pub mod error_card;
pub mod export_menu;
pub mod filter_bar;
pub mod hcs_strip;
pub mod header_bar;
pub mod header_editor;
pub mod help_text;
pub mod help_windows;
pub mod input;
pub mod metadata;
pub mod motif_composition;
pub mod position_explorer;
pub mod position_summary;
pub mod preferences;
pub mod recent_files;
pub mod run;
pub mod run_details;
pub mod summary;
pub mod toasts;
pub mod variant_table;
