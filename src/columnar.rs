//! Columnar Metadata Storage Module
//!
//! Stores metadata fields in separate contiguous arrays (column-oriented layout)
//! rather than as individual HashMaps per sequence, improving cache locality
//! and memory efficiency for bulk metadata aggregation operations.
//!
//! Performance characteristics:
//! - Better cache locality through column-oriented layout (sequential field access)
//! - Memory reduction through string interning (deduplication of repeated values)
//! - Efficient indexed lookups via optional inverted indices (bitmap-based)

use hashbrown::HashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::indexing::{IndexConfig, MetadataIndexManager};
use crate::zero_copy::StringInterner;

/// Column-oriented metadata storage container
///
/// This structure stores metadata in a columnar format where each field
/// is stored as a separate contiguous vector, improving cache locality
/// for sequential scans and bulk aggregation.
#[derive(Debug, Clone)]
pub struct ColumnarMetadata {
    /// Number of sequences stored
    sequence_count: usize,
    /// Field names in order
    field_names: Vec<String>,
    /// Column data for each field (field_index -> values)
    columns: Vec<Vec<Option<Arc<str>>>>,
    /// String interner for deduplication
    interner: Arc<StringInterner>,
    /// Index mapping for fast lookups (field_name -> column_index)
    field_index: HashMap<String, usize>,
}

impl ColumnarMetadata {
    /// Create a new columnar metadata container
    pub fn new(field_names: Vec<String>) -> Self {
        let field_count = field_names.len();
        let mut field_index = HashMap::with_capacity(field_count);

        for (idx, field_name) in field_names.iter().enumerate() {
            if field_index.contains_key(field_name) {
                tracing::warn!(
                    field = %field_name,
                    index = idx,
                    "duplicate metadata field — earlier column will be shadowed"
                );
            }
            field_index.insert(field_name.clone(), idx);
        }

        Self {
            sequence_count: 0,
            field_names,
            columns: vec![Vec::new(); field_count],
            interner: Arc::new(StringInterner::new()),
            field_index,
        }
    }

    /// Create from existing row-oriented metadata
    pub fn from_row_metadata(
        field_names: Vec<String>,
        row_metadata: &[Option<HashMap<String, String>>],
    ) -> Self {
        let mut columnar = Self::new(field_names.clone());

        // Pre-allocate columns
        for column in &mut columnar.columns {
            column.reserve(row_metadata.len());
        }

        // Convert row-oriented to columnar
        for row in row_metadata {
            columnar.add_sequence_metadata(row);
        }

        columnar
    }

    /// Add metadata for a single sequence
    pub fn add_sequence_metadata(&mut self, metadata: &Option<HashMap<String, String>>) {
        self.sequence_count += 1;

        match metadata {
            Some(meta_map) => {
                for (field_idx, field_name) in self.field_names.iter().enumerate() {
                    let value = meta_map.get(field_name).map(|v| self.interner.intern(v));
                    self.columns[field_idx].push(value);
                }
                // Warn once about extra keys that don't match any declared field
                if meta_map.len() > self.field_names.len() {
                    for key in meta_map.keys() {
                        if !self.field_index.contains_key(key) {
                            tracing::warn!(
                                key = %key,
                                sequence = self.sequence_count,
                                "metadata key not in field_names, value dropped"
                            );
                            break; // One warning per sequence is sufficient
                        }
                    }
                }
            }
            None => {
                for column in &mut self.columns {
                    column.push(None);
                }
            }
        }
    }

    /// Get metadata for a specific sequence (converts back to HashMap for compatibility)
    pub fn get_sequence_metadata(&self, sequence_idx: usize) -> Option<HashMap<String, String>> {
        if sequence_idx >= self.sequence_count {
            return None;
        }

        let mut metadata = HashMap::with_capacity(self.field_names.len());
        let mut has_data = false;

        for (field_idx, field_name) in self.field_names.iter().enumerate() {
            if let Some(Some(value)) = self.columns[field_idx].get(sequence_idx) {
                metadata.insert(field_name.clone(), value.to_string());
                has_data = true;
            }
        }

        if has_data {
            Some(metadata)
        } else {
            None
        }
    }

    /// Get all values for a specific field (column)
    pub fn get_field_column(&self, field_name: &str) -> Option<&Vec<Option<Arc<str>>>> {
        self.field_index
            .get(field_name)
            .and_then(|&idx| self.columns.get(idx))
    }

    /// Get unique values in a field with their counts
    pub fn get_field_value_counts(&self, field_name: &str) -> Option<HashMap<String, usize>> {
        self.get_field_column(field_name).map(|column| {
            let mut counts = HashMap::new();
            for value in column.iter().flatten() {
                *counts.entry(value.to_string()).or_insert(0) += 1;
            }
            counts
        })
    }

    /// Filter sequences by field value (returns sequence indices)
    pub fn filter_by_field_value(&self, field_name: &str, target_value: &str) -> Vec<usize> {
        self.get_field_column(field_name)
            .map(|column| {
                column
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, value_opt)| {
                        value_opt
                            .as_ref()
                            .filter(|value| value.as_ref() == target_value)
                            .map(|_| idx)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get metadata statistics
    pub fn get_stats(&self) -> ColumnarMetadataStats {
        let total_values = self.sequence_count * self.field_names.len();
        let mut non_null_values = 0;
        let mut unique_values = 0;

        for column in &self.columns {
            let column_non_null = column.iter().filter(|v| v.is_some()).count();
            non_null_values += column_non_null;

            let mut unique_in_column = std::collections::HashSet::new();
            for value in column.iter().flatten() {
                unique_in_column.insert(value.as_ptr());
            }
            unique_values += unique_in_column.len();
        }

        let (interned_common, interned_dynamic) = self.interner.stats();

        ColumnarMetadataStats {
            sequence_count: self.sequence_count,
            field_count: self.field_names.len(),
            total_values,
            non_null_values,
            unique_values,
            memory_usage: self.estimate_memory_usage(),
            interned_common_strings: interned_common,
            interned_dynamic_strings: interned_dynamic,
        }
    }

    /// Estimate memory usage in bytes
    fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;

        // Field names (String heap allocations)
        total += self.field_names.iter().map(|s| s.len()).sum::<usize>();

        // Column vectors: pointer overhead + per-cell Option<Arc<str>> overhead
        total += self.columns.len() * std::mem::size_of::<Vec<Option<Arc<str>>>>();
        total += self
            .columns
            .iter()
            .map(|col| col.capacity() * std::mem::size_of::<Option<Arc<str>>>())
            .sum::<usize>();

        // Arc<str> string payload bytes (the actual heap-allocated string data).
        // Note: interned strings share allocations, so this is an upper bound.
        for col in &self.columns {
            for arc in col.iter().flatten() {
                total += arc.len();
            }
        }

        // Field index HashMap entry overhead
        total +=
            self.field_index.len() * (std::mem::size_of::<String>() + std::mem::size_of::<usize>());

        total
    }

    /// Convert back to row-oriented format for compatibility
    pub fn to_row_metadata(&self) -> Vec<Option<HashMap<String, String>>> {
        (0..self.sequence_count)
            .map(|idx| self.get_sequence_metadata(idx))
            .collect()
    }

    /// Parallel processing of metadata aggregation for variants
    pub fn aggregate_metadata_for_indices_parallel(
        &self,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>> {
        // Always use sequential aggregation per-position. The outer analysis loop
        // (build_positions in analysis.rs) already parallelizes across positions via
        // Rayon's par_iter(). Adding nested parallelism here causes oversubscription
        // and work-stealing overhead that degrades performance for typical workloads
        // (usually <500 indices per variant × <8 fields per position).
        self.aggregate_metadata_for_indices_sequential(indices, fields)
    }

    /// Sequential metadata aggregation (for small datasets)
    fn aggregate_metadata_for_indices_sequential(
        &self,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>> {
        let mut result = HashMap::new();

        for field_name in fields {
            if let Some(column) = self.get_field_column(field_name) {
                let mut field_counts = HashMap::new();

                for &idx in indices {
                    if let Some(Some(value)) = column.get(idx) {
                        *field_counts.entry(value.to_string()).or_insert(0) += 1;
                    }
                }

                if !field_counts.is_empty() {
                    result.insert(field_name.clone(), field_counts);
                }
            }
        }

        result
    }

    /// Bulk update of field values (for data transformation)
    pub fn bulk_update_field(&mut self, field_name: &str, updates: &[(usize, String)]) {
        if let Some(&field_idx) = self.field_index.get(field_name) {
            for &(seq_idx, ref new_value) in updates {
                if seq_idx < self.sequence_count {
                    let interned_value = self.interner.intern(new_value);
                    self.columns[field_idx][seq_idx] = Some(interned_value);
                }
            }
        }
    }

    /// Get field names
    pub fn field_names(&self) -> &[String] {
        &self.field_names
    }

    /// Get sequence count
    pub fn sequence_count(&self) -> usize {
        self.sequence_count
    }
}

/// Statistics about columnar metadata storage
#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnarMetadataStats {
    pub sequence_count: usize,
    pub field_count: usize,
    pub total_values: usize,
    pub non_null_values: usize,
    pub unique_values: usize,
    pub memory_usage: usize,
    pub interned_common_strings: usize,
    pub interned_dynamic_strings: usize,
}

/// Vectorized operations for columnar metadata
pub struct ColumnarOperations;

impl ColumnarOperations {
    /// Column-wise field value comparison (returns a boolean mask).
    /// Iterates sequentially over the column — name retained for API compatibility.
    pub fn compare_field_values_vectorized(
        column: &[Option<Arc<str>>],
        target_value: &str,
    ) -> Vec<bool> {
        column
            .iter()
            .map(|value_opt| {
                value_opt
                    .as_ref()
                    .map(|value| value.as_ref() == target_value)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Parallel field aggregation with chunking.
    /// Uses `chunk_size.max(1)` to prevent Rayon panic on zero.
    pub fn parallel_field_aggregation(
        column: &[Option<Arc<str>>],
        chunk_size: usize,
    ) -> HashMap<String, usize> {
        let safe_chunk_size = chunk_size.max(1);
        column
            .par_chunks(safe_chunk_size)
            .map(|chunk| {
                let mut local_counts = HashMap::new();
                for value in chunk.iter().flatten() {
                    *local_counts.entry(value.to_string()).or_insert(0) += 1;
                }
                local_counts
            })
            .reduce(HashMap::new, |mut acc1, acc2| {
                for (key, count) in acc2 {
                    *acc1.entry(key).or_insert(0) += count;
                }
                acc1
            })
    }

    /// Batch field filtering with indices
    pub fn batch_filter_by_indices(
        columns: &[Vec<Option<Arc<str>>>],
        indices: &[usize],
    ) -> Vec<Vec<Option<String>>> {
        columns
            .par_iter()
            .map(|column| {
                indices
                    .iter()
                    .map(|&idx| {
                        column
                            .get(idx)
                            .and_then(|value_opt| value_opt.as_ref())
                            .map(|value| value.to_string())
                    })
                    .collect()
            })
            .collect()
    }
}

/// Enhanced compatibility layer with indexing support
pub struct ColumnarMetadataAdapter {
    columnar: ColumnarMetadata,
    index_manager: Option<MetadataIndexManager>,
    indexing_enabled: bool,
}

/// O(n + m) sorted intersection of two sorted slices.
/// Both inputs MUST be sorted in ascending order.
fn sorted_intersect(a: &[usize], b: &[usize]) -> Vec<usize> {
    let mut result = Vec::with_capacity(a.len().min(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                result.push(a[i]);
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    result
}

impl ColumnarMetadataAdapter {
    /// Create adapter from existing row-oriented metadata
    pub fn from_row_metadata(
        field_names: Vec<String>,
        row_metadata: Vec<Option<HashMap<String, String>>>,
    ) -> Self {
        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);
        Self {
            columnar,
            index_manager: None,
            indexing_enabled: false,
        }
    }

    /// Create adapter with indexing enabled
    pub fn from_row_metadata_with_indexing(
        field_names: Vec<String>,
        row_metadata: Vec<Option<HashMap<String, String>>>,
    ) -> Self {
        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);
        let index_manager = columnar.build_indices();

        Self {
            columnar,
            index_manager: Some(index_manager),
            indexing_enabled: true,
        }
    }

    /// Create adapter with custom index configuration
    pub fn from_row_metadata_with_config(
        field_names: Vec<String>,
        row_metadata: Vec<Option<HashMap<String, String>>>,
        index_config: IndexConfig,
    ) -> Self {
        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);
        let index_manager = columnar.build_indices_with_config(index_config);

        Self {
            columnar,
            index_manager: Some(index_manager),
            indexing_enabled: true,
        }
    }

    /// Enable indexing on existing adapter
    pub fn enable_indexing(&mut self) {
        if !self.indexing_enabled {
            let index_manager = self.columnar.build_indices();
            self.index_manager = Some(index_manager);
            self.indexing_enabled = true;
        }
    }

    /// Enable indexing with custom configuration
    pub fn enable_indexing_with_config(&mut self, config: IndexConfig) {
        let index_manager = self.columnar.build_indices_with_config(config);
        self.index_manager = Some(index_manager);
        self.indexing_enabled = true;
    }

    /// Disable indexing to save memory
    pub fn disable_indexing(&mut self) {
        self.index_manager = None;
        self.indexing_enabled = false;
    }

    /// Get metadata in original format for compatibility
    pub fn get_row_metadata(&self) -> Vec<Option<HashMap<String, String>>> {
        self.columnar.to_row_metadata()
    }

    /// Get columnar metadata for optimized operations
    pub fn get_columnar(&self) -> &ColumnarMetadata {
        &self.columnar
    }

    /// Get mutable columnar metadata.
    /// Invalidates cached indices since column data may be changed by the caller.
    /// Maintains the invariant: `indexing_enabled == index_manager.is_some()`.
    pub fn get_columnar_mut(&mut self) -> &mut ColumnarMetadata {
        self.index_manager = None;
        self.indexing_enabled = false;
        &mut self.columnar
    }

    /// Aggregate metadata for variant processing (optimized with indexing)
    pub fn aggregate_for_variant_indices(
        &mut self,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>> {
        // Use indexed aggregation if available
        if self.indexing_enabled {
            if let Some(ref mut index_manager) = self.index_manager {
                return Self::aggregate_with_indices_static(index_manager, indices, fields);
            }
        }

        // Fallback to columnar aggregation
        self.columnar
            .aggregate_metadata_for_indices_parallel(indices, fields)
    }

    /// Fast field value lookup using indices
    pub fn lookup_field_value(&mut self, field_name: &str, value: &str) -> Vec<usize> {
        if self.indexing_enabled {
            if let Some(ref mut index_manager) = self.index_manager {
                return index_manager.lookup_field_value(field_name, value);
            }
        }

        // Fallback to columnar lookup
        self.columnar.filter_by_field_value(field_name, value)
    }

    /// Multi-field query with AND logic (indexed)
    pub fn query_and(&mut self, field_queries: &[(&str, &str)]) -> Vec<usize> {
        if self.indexing_enabled {
            if let Some(ref mut index_manager) = self.index_manager {
                return index_manager.query_and(field_queries);
            }
        }

        // Fallback: sorted-merge intersection O(n + m) instead of O(n * m).
        // Each filter_by_field_value returns indices in ascending order, so
        // we can use a merge-intersect rather than Vec::contains.
        let mut result: Option<Vec<usize>> = None;
        for (field_name, value) in field_queries {
            let mut field_results = self.columnar.filter_by_field_value(field_name, value);
            field_results.sort_unstable();
            match result {
                Some(ref current) => {
                    let intersection = sorted_intersect(current, &field_results);
                    result = Some(intersection);
                }
                None => {
                    result = Some(field_results);
                }
            }
        }
        result.unwrap_or_default()
    }

    /// Multi-field query with OR logic (indexed)
    pub fn query_or(&mut self, field_queries: &[(&str, &str)]) -> Vec<usize> {
        if self.indexing_enabled {
            if let Some(ref mut index_manager) = self.index_manager {
                return index_manager.query_or(field_queries);
            }
        }

        // Fallback to sequential filtering with union
        let mut result = std::collections::HashSet::new();
        for (field_name, value) in field_queries {
            let field_results = self.columnar.filter_by_field_value(field_name, value);
            result.extend(field_results);
        }
        result.into_iter().collect()
    }

    /// Get field value counts (optimized with indexing)
    pub fn get_field_value_counts(&mut self, field_name: &str) -> HashMap<String, usize> {
        if self.indexing_enabled {
            if let Some(ref mut index_manager) = self.index_manager {
                return index_manager.get_field_value_counts(field_name);
            }
        }

        // Fallback to columnar method
        self.columnar
            .get_field_value_counts(field_name)
            .unwrap_or_default()
    }

    /// Get unique field values (optimized with indexing)
    pub fn get_unique_field_values(&mut self, field_name: &str) -> Vec<String> {
        if self.indexing_enabled {
            if let Some(ref mut index_manager) = self.index_manager {
                return index_manager.get_unique_field_values(field_name);
            }
        }

        // Fallback to columnar method
        self.columnar
            .get_field_value_counts(field_name)
            .map(|counts| counts.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Optimized metadata aggregation using indices (static method)
    fn aggregate_with_indices_static(
        index_manager: &mut MetadataIndexManager,
        indices: &[usize],
        fields: &[String],
    ) -> HashMap<String, HashMap<String, usize>> {
        use hashbrown::HashSet;
        let mut result = HashMap::new();

        // Build a HashSet for O(1) membership checks on our target indices
        // instead of the previous O(n) Vec::contains per value per index.
        let target_set: HashSet<usize> = indices.iter().copied().collect();

        for field_name in fields {
            let mut field_counts = HashMap::new();

            let unique_values = index_manager.get_unique_field_values(field_name);

            for value in unique_values {
                let value_indices = index_manager.lookup_field_value(field_name, &value);

                // O(value_indices.len()) total — each lookup is O(1) amortized
                let count = value_indices
                    .iter()
                    .filter(|&&idx| target_set.contains(&idx))
                    .count();

                if count > 0 {
                    field_counts.insert(value, count);
                }
            }

            if !field_counts.is_empty() {
                result.insert(field_name.clone(), field_counts);
            }
        }

        result
    }

    /// Get indexing statistics
    pub fn get_index_stats(&self) -> Option<crate::indexing::IndexManagerStats> {
        if self.indexing_enabled {
            self.index_manager
                .as_ref()
                .map(|manager| manager.get_comprehensive_stats())
        } else {
            None
        }
    }

    /// Check if indexing is enabled
    pub fn is_indexing_enabled(&self) -> bool {
        self.indexing_enabled
    }

    /// Get total memory usage including indices
    pub fn get_total_memory_usage(&self) -> usize {
        let mut total = self.columnar.estimate_memory_usage();

        if let Some(ref index_manager) = self.index_manager {
            total += index_manager.get_total_memory_usage();
        }

        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metadata() -> Vec<Option<HashMap<String, String>>> {
        vec![
            Some({
                let mut map = HashMap::new();
                map.insert("Date".to_string(), "2023-01-01".to_string());
                map.insert("Country".to_string(), "USA".to_string());
                map.insert("Species".to_string(), "Human".to_string());
                map
            }),
            Some({
                let mut map = HashMap::new();
                map.insert("Date".to_string(), "2023-01-02".to_string());
                map.insert("Country".to_string(), "Canada".to_string());
                map.insert("Species".to_string(), "Human".to_string());
                map
            }),
            Some({
                let mut map = HashMap::new();
                map.insert("Date".to_string(), "2023-01-01".to_string());
                map.insert("Country".to_string(), "USA".to_string());
                map.insert("Species".to_string(), "Mouse".to_string());
                map
            }),
            None,
        ]
    }

    #[test]
    fn test_columnar_metadata_creation() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names.clone(), &row_metadata);

        assert_eq!(columnar.sequence_count(), 4);
        assert_eq!(columnar.field_names(), &field_names);
    }

    #[test]
    fn test_row_to_columnar_conversion() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);
        let converted_back = columnar.to_row_metadata();

        assert_eq!(converted_back.len(), row_metadata.len());

        // Check first sequence
        let first_original = &row_metadata[0];
        let first_converted = &converted_back[0];

        match (first_original, first_converted) {
            (Some(orig), Some(conv)) => {
                assert_eq!(orig.get("Date"), conv.get("Date"));
                assert_eq!(orig.get("Country"), conv.get("Country"));
                assert_eq!(orig.get("Species"), conv.get("Species"));
            }
            _ => panic!("Conversion failed"),
        }
    }

    #[test]
    fn test_field_value_counts() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);

        let country_counts = columnar.get_field_value_counts("Country").unwrap();
        assert_eq!(country_counts.get("USA"), Some(&2));
        assert_eq!(country_counts.get("Canada"), Some(&1));

        let species_counts = columnar.get_field_value_counts("Species").unwrap();
        assert_eq!(species_counts.get("Human"), Some(&2));
        assert_eq!(species_counts.get("Mouse"), Some(&1));
    }

    #[test]
    fn test_field_filtering() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);

        let usa_sequences = columnar.filter_by_field_value("Country", "USA");
        assert_eq!(usa_sequences, vec![0, 2]);

        let human_sequences = columnar.filter_by_field_value("Species", "Human");
        assert_eq!(human_sequences, vec![0, 1]);
    }

    #[test]
    fn test_metadata_aggregation() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names.clone(), &row_metadata);

        let indices = vec![0, 1, 2]; // Exclude None entry
        let aggregated = columnar.aggregate_metadata_for_indices_parallel(&indices, &field_names);

        assert!(aggregated.contains_key("Country"));
        let country_agg = &aggregated["Country"];
        assert_eq!(country_agg.get("USA"), Some(&2));
        assert_eq!(country_agg.get("Canada"), Some(&1));
    }

    #[test]
    fn test_columnar_adapter() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let mut adapter =
            ColumnarMetadataAdapter::from_row_metadata(field_names.clone(), row_metadata.clone());

        let converted_back = adapter.get_row_metadata();
        assert_eq!(converted_back.len(), row_metadata.len());

        // Test optimized aggregation
        let indices = vec![0, 2];
        let aggregated = adapter.aggregate_for_variant_indices(&indices, &field_names);

        assert!(aggregated.contains_key("Country"));
        assert_eq!(aggregated["Country"].get("USA"), Some(&2));
    }

    #[test]
    fn test_vectorized_operations() {
        let field_names = vec!["Country".to_string()];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);
        let country_column = columnar.get_field_column("Country").unwrap();

        let usa_matches =
            ColumnarOperations::compare_field_values_vectorized(country_column, "USA");
        assert_eq!(usa_matches, vec![true, false, true, false]);

        let counts = ColumnarOperations::parallel_field_aggregation(country_column, 2);
        assert_eq!(counts.get("USA"), Some(&2));
        assert_eq!(counts.get("Canada"), Some(&1));
    }

    #[test]
    fn test_memory_stats() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];
        let row_metadata = create_test_metadata();

        let columnar = ColumnarMetadata::from_row_metadata(field_names, &row_metadata);
        let stats = columnar.get_stats();

        assert_eq!(stats.sequence_count, 4);
        assert_eq!(stats.field_count, 3);
        assert_eq!(stats.total_values, 12);
        assert_eq!(stats.non_null_values, 9); // 3 sequences with 3 fields each
        assert!(stats.memory_usage > 0);

        println!("Columnar metadata stats: {:?}", stats);
    }

    #[test]
    fn test_bulk_operations() {
        let field_names = vec!["Country".to_string(), "Status".to_string()];
        let mut columnar = ColumnarMetadata::new(field_names);

        // Add some initial data
        let mut meta1 = HashMap::new();
        meta1.insert("Country".to_string(), "USA".to_string());
        meta1.insert("Status".to_string(), "Active".to_string());
        columnar.add_sequence_metadata(&Some(meta1));

        let mut meta2 = HashMap::new();
        meta2.insert("Country".to_string(), "Canada".to_string());
        meta2.insert("Status".to_string(), "Inactive".to_string());
        columnar.add_sequence_metadata(&Some(meta2));

        // Bulk update
        let updates = vec![
            (0, "Updated_USA".to_string()),
            (1, "Updated_Canada".to_string()),
        ];
        columnar.bulk_update_field("Country", &updates);

        let updated_meta = columnar.get_sequence_metadata(0).unwrap();
        assert_eq!(
            updated_meta.get("Country"),
            Some(&"Updated_USA".to_string())
        );
    }

    #[test]
    fn test_performance_comparison() {
        let field_names = vec![
            "Date".to_string(),
            "Country".to_string(),
            "Species".to_string(),
        ];

        // Create larger dataset for performance testing
        let mut large_metadata = Vec::new();
        for i in 0..10000 {
            let mut map = HashMap::new();
            map.insert(
                "Date".to_string(),
                format!("2023-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1),
            );
            map.insert(
                "Country".to_string(),
                ["USA", "Canada", "Mexico", "Brazil"][i % 4].to_string(),
            );
            map.insert(
                "Species".to_string(),
                ["Human", "Mouse", "Rat"][i % 3].to_string(),
            );
            large_metadata.push(Some(map));
        }

        let start = std::time::Instant::now();
        let columnar = ColumnarMetadata::from_row_metadata(field_names.clone(), &large_metadata);
        let creation_time = start.elapsed();

        let start = std::time::Instant::now();
        let indices: Vec<usize> = (0..5000).collect();
        let _aggregated = columnar.aggregate_metadata_for_indices_parallel(&indices, &field_names);
        let aggregation_time = start.elapsed();

        println!("Columnar creation time: {:?}", creation_time);
        println!("Columnar aggregation time: {:?}", aggregation_time);

        let stats = columnar.get_stats();
        println!("Performance test stats: {:?}", stats);

        // Verify correctness
        assert_eq!(columnar.sequence_count(), 10000);
        let usa_count = columnar.filter_by_field_value("Country", "USA").len();
        assert_eq!(usa_count, 2500); // 10000 / 4 countries
    }
}
