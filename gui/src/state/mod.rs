//! Application state management.
//!
//! In egui's immediate-mode paradigm, state lives in the App struct and is
//! read each frame. No event bus, no pub/sub, no store subscriptions needed.

pub mod filter;
pub mod recent;
pub mod sort;

pub use filter::{FilterState, MotifType};
pub use recent::RecentFileStore;
pub use sort::{SortDirection, TableSort};
