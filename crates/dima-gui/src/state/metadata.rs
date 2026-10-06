//! What the metadata section shows: which fields are open, which list every value, and the
//! order of the values of the selected variant. The order is computed once per variant and
//! field, not per frame: a field such as an accession number can carry one value per sequence,
//! millions at a common variant.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use dima_core::model::FieldCounts;

/// Values listed per field until the user asks for all of them.
pub const TOP_VALUES: usize = 5;
/// Values listed at most when showing all.
pub const MAX_VALUES: usize = 200;

/// The values of one field in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldOrder {
    /// Sum of the counts (saturating).
    pub total: u64,
    /// Number of distinct values.
    pub len: usize,
    /// Indices into `FieldCounts::values` of the (at most `MAX_VALUES`) most common values,
    /// by count (descending), ties in the library's order.
    pub top: Vec<usize>,
}

impl FieldOrder {
    /// Orders a field's values; linear in the number of values plus `MAX_VALUES` log
    /// `MAX_VALUES`.
    #[must_use]
    pub fn of(f: &FieldCounts) -> Self {
        let total = f.values.iter().fold(0u64, |a, (_, c)| a.saturating_add(*c));
        // (count descending, index) is a total order, so the unstable selection and sort are
        // deterministic and keep ties in the library's order.
        let key = |&i: &usize| (Reverse(f.values[i].1), i);
        let mut top: Vec<usize> = (0..f.values.len()).collect();
        if top.len() > MAX_VALUES {
            top.select_nth_unstable_by_key(MAX_VALUES, key);
            top.truncate(MAX_VALUES);
        }
        top.sort_unstable_by_key(key);
        Self {
            total,
            len: f.values.len(),
            top,
        }
    }
}

/// The variant whose value orders are cached: position and variant index.
type VariantKey = (u64, usize);

/// Metadata view state of one session.
#[derive(Clone, Debug, Default)]
pub struct MetadataView {
    /// Open fields; `None` until the first is opened automatically.
    pub open: Option<BTreeSet<String>>,
    /// Fields listing every value.
    pub all: BTreeSet<String>,
    /// Value orders of the variant last shown, per field index.
    cache: Option<(VariantKey, Vec<Option<FieldOrder>>)>,
}

impl MetadataView {
    /// The display order of field `index` of the variant `key`, computed on first use.
    pub fn order(&mut self, key: VariantKey, index: usize, field: &FieldCounts) -> &FieldOrder {
        if self.cache.as_ref().is_none_or(|(k, _)| *k != key) {
            self.cache = Some((key, Vec::new()));
        }
        let (_, orders) = self
            .cache
            .as_mut()
            .unwrap_or_else(|| unreachable!("the cache was set above"));
        if orders.len() <= index {
            orders.resize(index + 1, None);
        }
        orders[index].get_or_insert_with(|| FieldOrder::of(field))
    }

    /// `true` when the field is open.
    #[must_use]
    pub fn is_open(&self, field: &str) -> bool {
        self.open.as_ref().is_some_and(|o| o.contains(field))
    }

    /// Opens the first field when none was opened or closed yet.
    pub fn open_first(&mut self, first: &str) {
        if self.open.is_none() {
            self.open = Some(BTreeSet::from([first.to_owned()]));
        }
    }

    /// Opens or closes a field.
    pub fn toggle_open(&mut self, field: String) {
        let open = self.open.get_or_insert_with(BTreeSet::new);
        if !open.remove(&field) {
            open.insert(field);
        }
    }

    /// Switches a field between its top values and all of them.
    pub fn toggle_all(&mut self, field: String) {
        if !self.all.remove(&field) {
            self.all.insert(field);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(values: &[(&str, u64)]) -> FieldCounts {
        FieldCounts {
            field: "Host".into(),
            values: values.iter().map(|(v, c)| ((*v).to_owned(), *c)).collect(),
        }
    }

    fn names<'a>(f: &'a FieldCounts, o: &FieldOrder) -> Vec<&'a str> {
        o.top.iter().map(|&i| f.values[i].0.as_str()).collect()
    }

    #[test]
    fn values_by_count_keep_library_order_for_ties() {
        let f = field(&[("Bat", 2), ("Camel", 9), ("Human", 2)]);
        let o = FieldOrder::of(&f);
        assert_eq!(names(&f, &o), ["Camel", "Bat", "Human"]);
        assert_eq!((o.total, o.len), (13, 3));
    }

    #[test]
    fn only_the_most_common_values_are_kept() {
        // Counts 1..=250 in reverse, plus ties at the cut: the top 200 are the highest counts,
        // ties in library order.
        let values: Vec<(String, u64)> = (0..250u64).map(|i| (format!("v{i:03}"), i / 2)).collect();
        let f = FieldCounts {
            field: "Accession".into(),
            values,
        };
        let o = FieldOrder::of(&f);
        assert_eq!(o.len, 250);
        assert_eq!(o.top.len(), MAX_VALUES);
        let expected: Vec<usize> = {
            let mut all: Vec<usize> = (0..250).collect();
            all.sort_by_key(|&i| (Reverse(f.values[i].1), i));
            all.truncate(MAX_VALUES);
            all
        };
        assert_eq!(o.top, expected);
    }

    #[test]
    fn totals_saturate_and_empty_fields_are_empty() {
        let f = field(&[("a", u64::MAX), ("b", 1)]);
        assert_eq!(FieldOrder::of(&f).total, u64::MAX);
        let empty = FieldOrder::of(&field(&[]));
        assert_eq!((empty.total, empty.len, empty.top.len()), (0, 0, 0));
    }

    #[test]
    fn orders_are_cached_per_variant() {
        let a = field(&[("x", 1), ("y", 3)]);
        let b = field(&[("z", 5)]);
        let mut view = MetadataView::default();
        assert_eq!(view.order((7, 0), 1, &a).top, [1, 0]);
        // Same variant: the cached order is returned even for other data.
        assert_eq!(view.order((7, 0), 1, &b).top, [1, 0]);
        // Another variant: recomputed.
        assert_eq!(view.order((7, 1), 1, &b).top, [0]);
    }

    #[test]
    fn opening_and_toggling() {
        let mut view = MetadataView::default();
        view.open_first("Host");
        assert!(view.is_open("Host"));
        view.toggle_open("Host".into());
        assert!(!view.is_open("Host"));
        // Closing every field is remembered: the first is not reopened.
        view.open_first("Host");
        assert!(!view.is_open("Host"));
        view.toggle_all("Host".into());
        assert!(view.all.contains("Host"));
        view.toggle_all("Host".into());
        assert!(view.all.is_empty());
    }
}
