//! Column-sort state for TableBuilder-based tables.
//!
//! Provides a generic sort model (column index + direction) that works with
//! egui_extras::TableBuilder. Uses `Cell`-based accumulation so header click
//! events can be captured inside the `header()` closure and applied after
//! the table finishes rendering (egui's closure-based API prevents &mut self
//! access during header rendering).

/// Sort direction for a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// Toggle between ascending and descending.
    pub fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    /// Unicode arrow indicator for display in column headers.
    /// Uses U+23F6 (⏶) and U+23F7 (⏷) which are confirmed in egui's
    /// special_emojis module and guaranteed to render in the default font.
    pub fn indicator(self) -> &'static str {
        match self {
            Self::Ascending => " \u{23F6}",  // ⏶
            Self::Descending => " \u{23F7}", // ⏷
        }
    }
}

/// Identifies which column is sorted and in which direction.
/// `column` is a 0-based index matching the column order in the TableBuilder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSort {
    pub column: usize,
    pub direction: SortDirection,
}

impl TableSort {
    pub fn new(column: usize, direction: SortDirection) -> Self {
        Self { column, direction }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_direction() {
        assert_eq!(SortDirection::Ascending.toggle(), SortDirection::Descending);
        assert_eq!(SortDirection::Descending.toggle(), SortDirection::Ascending);
    }

    #[test]
    fn test_indicators_are_non_empty() {
        assert!(!SortDirection::Ascending.indicator().is_empty());
        assert!(!SortDirection::Descending.indicator().is_empty());
    }
}
