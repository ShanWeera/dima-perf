//! # Header Parsing Module
//!
//! Provides SIMD-accelerated pipe-delimited header field extraction with
//! string interning for repeated values. Uses `wide` crate for portable
//! 128-bit SIMD on x86_64 and aarch64, with automatic scalar fallback.
//!
//! ## Production entry point
//!
//! `parse_header_zero_copy` (called from `io.rs::parse_header_internal`) is
//! the sole production entry point. All other public functions are test utilities.
#![allow(dead_code)]

use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::simd_string::{SimdDelimiterParser, SimdStringTrimmer};

/// Strip invisible Unicode characters (zero-width joiners, format chars, BOM, etc.)
/// that would cause visually-identical metadata values to hash to different buckets.
/// Keeps only printable content that users can actually see and distinguish.
#[inline]
fn strip_invisible_unicode(s: &str) -> String {
    s.chars()
        .filter(|c| {
            // Keep ASCII printable, keep normal Unicode letters/digits/punctuation/whitespace
            // Strip: Category Cf (format), Cc (control except newline/tab already handled),
            //        zero-width spaces (U+200B..U+200F, U+2028..U+202F, U+2060..U+206F, U+FEFF)
            !matches!(*c,
                '\u{200B}'..='\u{200F}' |
                '\u{2028}'..='\u{202F}' |
                '\u{2060}'..='\u{206F}' |
                '\u{FEFF}' |
                '\u{00AD}' // soft hyphen
            ) && !c.is_control()
        })
        .collect()
}

/// String interning system for common header field names and values.
/// 
/// String interner for deduplicating metadata values.
/// Provides memory savings when processing many sequences with repetitive
/// header field names and common metadata values. Uses `RwLock` because
/// `ColumnarMetadata` embeds an `Arc<StringInterner>` shared across Rayon
/// worker threads (which requires `Sync`). During parallel entropy/analysis
/// the interner is only read (all writes happen during the sequential I/O phase).
#[derive(Debug)]
pub struct StringInterner {
    strings: RwLock<BTreeMap<String, Arc<str>>>,
    common_fields: HashMap<&'static str, Arc<str>>,
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl StringInterner {
    pub fn new() -> Self {
        let mut common_fields = HashMap::new();
        
        // Pre-intern common header field names
        let common_names = [
            "sample_id", "condition", "replicate", "treatment", "timepoint",
            "batch", "experiment", "group", "subject", "tissue", "cell_type",
            "protocol", "platform", "run_id", "lane", "barcode", "index",
            "organism", "strain", "genotype", "phenotype", "age", "sex",
            "Unknown", "NA", "NULL", "missing", "control", "treated"
        ];
        
        for &name in &common_names {
            let arc_str: Arc<str> = Arc::from(name);
            common_fields.insert(name, arc_str);
        }
        
        Self {
            strings: RwLock::new(BTreeMap::new()),
            common_fields,
        }
    }
    
    /// Intern a string, returning an `Arc<str>` for shared ownership.
    ///
    /// Once the interner reaches MAX_INTERN_CAPACITY, new unique strings
    /// are returned as fresh `Arc<str>` without caching — preventing unbounded
    /// memory growth from diverse short values (e.g., k-mer-like tokens).
    pub fn intern(&self, s: &str) -> Arc<str> {
        const MAX_INTERN_CAPACITY: usize = 10_000;

        // Fast path: check pre-interned common fields (no lock needed)
        if let Some(arc_str) = self.common_fields.get(s) {
            return Arc::clone(arc_str);
        }

        // Check if already interned (read lock — uncontended on thread-local path)
        {
            let strings = self.strings.read().unwrap_or_else(|p| p.into_inner());
            if let Some(arc_str) = strings.get(s) {
                return Arc::clone(arc_str);
            }
            if strings.len() >= MAX_INTERN_CAPACITY {
                return Arc::from(s);
            }
        }

        // Intern new string (write lock)
        let mut strings = self.strings.write().unwrap_or_else(|p| p.into_inner());
        // Double-check after acquiring write lock
        if let Some(arc_str) = strings.get(s) {
            Arc::clone(arc_str)
        } else if strings.len() >= MAX_INTERN_CAPACITY {
            Arc::from(s)
        } else {
            let arc_str: Arc<str> = Arc::from(s);
            strings.insert(s.to_string(), Arc::clone(&arc_str));
            arc_str
        }
    }
    
    /// Get statistics about interned strings
    pub fn stats(&self) -> (usize, usize) {
        let strings = self.strings.read().unwrap_or_else(|p| p.into_inner());
        (self.common_fields.len(), strings.len())
    }
}

// Thread-local string interner for optimal performance
thread_local! {
    static STRING_INTERNER: StringInterner = StringInterner::new();
}

/// Zero-copy header component that can either borrow or own string data
#[derive(Debug, Clone)]
pub enum HeaderComponent<'a> {
    /// Borrowed string slice (zero-copy)
    Borrowed(&'a str),
    /// Owned string (when modification is needed)
    Owned(String),
    /// Interned string (shared ownership)
    Interned(Arc<str>),
}

impl<'a> std::fmt::Display for HeaderComponent<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> HeaderComponent<'a> {
    /// Get the string value, regardless of storage type
    pub fn as_str(&self) -> &str {
        match self {
            HeaderComponent::Borrowed(s) => s,
            HeaderComponent::Owned(s) => s,
            HeaderComponent::Interned(s) => s,
        }
    }
    
    /// Convert to owned String (for HashMap compatibility)
    pub fn to_owned_string(&self) -> String {
        self.as_str().to_owned()
    }
    
    /// Create from borrowed string slice
    pub fn from_borrowed(s: &'a str) -> Self {
        HeaderComponent::Borrowed(s)
    }
    
    /// Create from owned string
    pub fn from_owned(s: String) -> Self {
        HeaderComponent::Owned(s)
    }
    
    /// Create interned component for common values
    pub fn from_interned(s: &str) -> Self {
        let arc_str = STRING_INTERNER.with(|interner| interner.intern(s));
        HeaderComponent::Interned(arc_str)
    }
}

/// Zero-copy header parser that minimizes allocations
pub struct ZeroCopyHeaderParser {
    delimiter_parser: SimdDelimiterParser,
    string_trimmer: SimdStringTrimmer,
}

impl Default for ZeroCopyHeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ZeroCopyHeaderParser {
    pub fn new() -> Self {
        Self {
            delimiter_parser: SimdDelimiterParser::new(),
            string_trimmer: SimdStringTrimmer::new(),
        }
    }
    
    /// Parse header with zero-copy optimizations
    /// 
    /// This function minimizes string allocations by:
    /// - Using string slices where possible (zero-copy)
    /// - Interning common field names and values
    /// - Only allocating when necessary (trimming, concatenation)
    pub fn parse_zero_copy(
        &self,
        header: &str,
        format: &[String],
        fill_na: &str,
    ) -> Result<HashMap<String, String>, String> {
        const MAX_HEADER_LENGTH: usize = 1_000_000; // 1 MB
        const MAX_DELIMITER_COUNT: usize = 1_000;

        if header.len() > MAX_HEADER_LENGTH {
            return Err(format!(
                "Header exceeds maximum length ({} > {} bytes)",
                header.len(), MAX_HEADER_LENGTH
            ));
        }

        let header_bytes = header.as_bytes();
        
        // Use SIMD-accelerated delimiter detection
        let delimiter_positions = self.delimiter_parser.find_pipe_delimiters(header_bytes);

        if delimiter_positions.len() > MAX_DELIMITER_COUNT {
            return Err(format!(
                "Header has too many delimiters ({} > {} max)",
                delimiter_positions.len(), MAX_DELIMITER_COUNT
            ));
        }
        
        // Pre-allocate components vector
        let mut components = Vec::with_capacity(delimiter_positions.len() + 1);
        let mut start = 0;
        
        // Process each component with zero-copy optimizations
        for &pos in &delimiter_positions {
            let component = self.extract_component(header, start, pos, fill_na);
            components.push(component);
            start = pos + 1;
        }
        
        // Handle the last component (use <= to match standard split behavior with trailing delimiters)
        if start <= header.len() {
            let component = self.extract_component(header, start, header.len(), fill_na);
            components.push(component);
        }
        
        // Validation (maintain original behavior)
        if components.iter().any(|comp| comp.as_str().is_empty()) {
            return Err(format!(
                "\n\nThe FASTA header looks invalid:\n\tFormat: {}\n\tHeader: {}\n\n",
                format.join("|"),
                header
            ));
        }
        
        if components.len() != format.len() {
            return Err(format!(
                "\n\nThe header format provided does not match the header:\n\tFormat: {}\n\tHeader: {}\n\n",
                format.join("|"),
                header
            ));
        }
        
        // Build result HashMap with optimized allocations.
        // Values are sanitized to strip invisible Unicode (zero-width spaces, format chars)
        // that would create visually-identical-but-distinct metadata buckets in charts.
        let mut result = HashMap::with_capacity(format.len());
        for (idx, format_field) in format.iter().enumerate() {
            let key = format_field.clone();
            let raw_value = components[idx].to_string();
            let value = strip_invisible_unicode(&raw_value);
            result.insert(key, value);
        }
        
        Ok(result)
    }
    
    /// Extract a header component with zero-copy optimizations
    fn extract_component<'a>(
        &self,
        header: &'a str,
        start: usize,
        end: usize,
        fill_na: &str,
    ) -> HeaderComponent<'a> {
        if start >= end {
            return if !fill_na.is_empty() {
                HeaderComponent::from_interned(fill_na)
            } else {
                HeaderComponent::from_borrowed("")
            };
        }
        
        let component_str = &header[start..end];
        
        // Use SIMD-accelerated trimming on bytes for efficiency
        let component_bytes = component_str.as_bytes();
        let trimmed_bytes = self.string_trimmer.trim_bytes(component_bytes);
        
        if trimmed_bytes.is_empty() {
            return if !fill_na.is_empty() {
                HeaderComponent::from_interned(fill_na)
            } else {
                HeaderComponent::from_borrowed("")
            };
        }
        
        // Check if trimming was needed
        if trimmed_bytes.len() == component_bytes.len() {
            // No trimming needed - use zero-copy borrowed string
            if self.is_common_value(component_str) {
                HeaderComponent::from_interned(component_str)
            } else {
                HeaderComponent::from_borrowed(component_str)
            }
        } else {
            // Trimming was needed - convert back to string
            let trimmed_str = unsafe { std::str::from_utf8_unchecked(trimmed_bytes) };
            if self.is_common_value(trimmed_str) {
                HeaderComponent::from_interned(trimmed_str)
            } else {
                HeaderComponent::from_owned(trimmed_str.to_string())
            }
        }
    }
    
    /// Check if a value is commonly used (for interning)
    fn is_common_value(&self, value: &str) -> bool {
        matches!(value,
            "Unknown" | "NA" | "NULL" | "missing" | "control" | "treated" |
            "male" | "female" | "yes" | "no" | "true" | "false" | "0" | "1"
        ) || value.len() <= 3 // Short strings are good candidates for interning
    }
}

// Thread-local zero-copy parser for optimal performance
thread_local! {
    static ZERO_COPY_PARSER: ZeroCopyHeaderParser = ZeroCopyHeaderParser::new();
}

/// Parse a single FASTA header into field-value pairs.
///
/// Uses SIMD-accelerated pipe delimiter detection and string interning for
/// repeated values. Returns `Err` if the header cannot be split according
/// to the provided format fields.
///
/// Callers should accumulate errors and decide on a threshold (e.g., fail if > 5%
/// of headers are malformed). This prevents silently dropping metadata from large
/// portions of the dataset while still allowing isolated bad headers.
pub fn parse_header_zero_copy(
    header: &str,
    format: &[String],
    fill_na: &str,
) -> Result<HashMap<String, String>, String> {
    ZERO_COPY_PARSER.with(|parser| {
        parser.parse_zero_copy(header, format, fill_na)
    })
}

#[cfg(test)]
/// Batch zero-copy header processing (test utility only).
pub fn parse_headers_batch_zero_copy(
    headers: &[String],
    format: &[String],
    fill_na: &str,
) -> Vec<HashMap<String, String>> {
    headers
        .iter()
        .map(|header| parse_header_zero_copy(header, format, fill_na).unwrap_or_default())
        .collect()
}

#[cfg(test)]
/// Memory usage statistics for zero-copy operations (test only)
#[derive(Debug)]
pub struct ZeroCopyStats {
    pub interned_common_fields: usize,
    pub interned_dynamic_strings: usize,
    pub total_interned_memory: usize,
}

#[cfg(test)]
/// Get zero-copy memory usage statistics (test only)
pub fn get_zero_copy_stats() -> ZeroCopyStats {
    let (common_fields, dynamic_strings) = STRING_INTERNER.with(|interner| interner.stats());
    
    ZeroCopyStats {
        interned_common_fields: common_fields,
        interned_dynamic_strings: dynamic_strings,
        total_interned_memory: (common_fields + dynamic_strings) * std::mem::size_of::<Arc<str>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_header_parsing() {
        let header = "sample1|condition_A|replicate_1|treatment_X";
        let format = vec![
            "sample_id".to_string(),
            "condition".to_string(),
            "replicate".to_string(),
            "treatment".to_string(),
        ];
        let fill_na = "Unknown";
        
        let result = parse_header_zero_copy(header, &format, fill_na).unwrap();
        
        assert_eq!(result.get("sample_id"), Some(&"sample1".to_string()));
        assert_eq!(result.get("condition"), Some(&"condition_A".to_string()));
        assert_eq!(result.get("replicate"), Some(&"replicate_1".to_string()));
        assert_eq!(result.get("treatment"), Some(&"treatment_X".to_string()));
    }

    #[test]
    fn test_zero_copy_with_whitespace() {
        let header = " sample1 | condition_A |  replicate_1  | treatment_X ";
        let format = vec![
            "sample_id".to_string(),
            "condition".to_string(),
            "replicate".to_string(),
            "treatment".to_string(),
        ];
        let fill_na = "Unknown";
        
        let result = parse_header_zero_copy(header, &format, fill_na).unwrap();
        
        assert_eq!(result.get("sample_id"), Some(&"sample1".to_string()));
        assert_eq!(result.get("condition"), Some(&"condition_A".to_string()));
        assert_eq!(result.get("replicate"), Some(&"replicate_1".to_string()));
        assert_eq!(result.get("treatment"), Some(&"treatment_X".to_string()));
    }

    #[test]
    fn test_header_component_types() {
        let borrowed = HeaderComponent::from_borrowed("test");
        let owned = HeaderComponent::from_owned("test".to_string());
        let interned = HeaderComponent::from_interned("test");
        
        assert_eq!(borrowed.as_str(), "test");
        assert_eq!(owned.as_str(), "test");
        assert_eq!(interned.as_str(), "test");
        
        assert_eq!(borrowed.to_string(), "test");
        assert_eq!(owned.to_string(), "test");
        assert_eq!(interned.to_string(), "test");
    }

    #[test]
    fn test_string_interning() {
        let interner = StringInterner::new();
        
        // Test common field interning
        let arc1 = interner.intern("sample_id");
        let arc2 = interner.intern("sample_id");
        
        // Should be the same Arc (pointer equality)
        assert!(Arc::ptr_eq(&arc1, &arc2));
        
        // Test dynamic string interning
        let arc3 = interner.intern("custom_field");
        let arc4 = interner.intern("custom_field");
        
        assert!(Arc::ptr_eq(&arc3, &arc4));
    }

    #[test]
    fn test_buffer_to_header_parsing() {
        let header_bytes = b"sample1|condition_A|replicate_1";
        let header_str = std::str::from_utf8(header_bytes).unwrap();
        let format = vec![
            "sample_id".to_string(),
            "condition".to_string(),
            "replicate".to_string(),
        ];
        let fill_na = "Unknown";
        
        let result = parse_header_zero_copy(header_str, &format, fill_na).unwrap();
        
        assert_eq!(result.get("sample_id"), Some(&"sample1".to_string()));
        assert_eq!(result.get("condition"), Some(&"condition_A".to_string()));
        assert_eq!(result.get("replicate"), Some(&"replicate_1".to_string()));
    }

    #[test]
    fn test_batch_processing() {
        let headers = vec![
            "sample1|condition_A|replicate_1".to_string(),
            "sample2|condition_B|replicate_2".to_string(),
            "sample3|condition_A|replicate_3".to_string(),
        ];
        let format = vec![
            "sample_id".to_string(),
            "condition".to_string(),
            "replicate".to_string(),
        ];
        let fill_na = "Unknown";
        
        let results = parse_headers_batch_zero_copy(&headers, &format, fill_na);
        
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].get("sample_id"), Some(&"sample1".to_string()));
        assert_eq!(results[1].get("condition"), Some(&"condition_B".to_string()));
        assert_eq!(results[2].get("replicate"), Some(&"replicate_3".to_string()));
    }

    #[test]
    fn test_zero_copy_stats() {
        // Process some headers to populate the interner
        let headers = vec![
            "sample1|Unknown|replicate_1".to_string(),
            "sample2|Unknown|replicate_2".to_string(),
        ];
        let format = vec![
            "sample_id".to_string(),
            "condition".to_string(),
            "replicate".to_string(),
        ];
        let fill_na = "Unknown";
        
        let _results = parse_headers_batch_zero_copy(&headers, &format, fill_na);
        let stats = get_zero_copy_stats();
        
        // Should have some interned strings
        assert!(stats.interned_common_fields > 0);
        println!("Zero-copy stats: {:?}", stats);
    }

    #[test]
    fn test_error_handling() {
        let parser = ZeroCopyHeaderParser::new();
        
        // Test mismatched format length
        let fields = &["sample_id".to_string(), "condition".to_string(), "replicate".to_string()];
        let result = parser.parse_zero_copy(
            "sample1|condition_A",
            fields,
            "Unknown"
        );
        assert!(result.is_err());
        
        // Test empty component
        let result = parser.parse_zero_copy(
            "sample1||replicate_1",
            fields,
            "" // Empty fill_na
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_performance_comparison() {
        let headers: Vec<String> = (0..1000)
            .map(|i| format!("sample{}|condition_{}|replicate_{}", i, i % 10, i % 5))
            .collect();
        let format = vec![
            "sample_id".to_string(),
            "condition".to_string(),
            "replicate".to_string(),
        ];
        let fill_na = "Unknown";
        
        // Test zero-copy performance
        let start = std::time::Instant::now();
        let _zero_copy_results = parse_headers_batch_zero_copy(&headers, &format, fill_na);
        let zero_copy_time = start.elapsed();
        
        println!("Zero-copy processing time: {:?}", zero_copy_time);
        println!("Processed {} headers", headers.len());
        
        let stats = get_zero_copy_stats();
        println!("Memory stats: {:?}", stats);
    }
}
