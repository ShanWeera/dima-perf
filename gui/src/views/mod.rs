//! Application views (Setup and Workspace).

/// The two phases of the application UX.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum View {
    /// Phase 1: File selection, configuration, analysis
    #[default]
    Setup,
    /// Phase 2: Results dashboard with panels
    Workspace,
}
