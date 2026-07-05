//! SIMD-Accelerated String Parsing Module
//!
//! Provides portable 128-bit SIMD operations for pipe-delimiter detection
//! and string trimming, using the `wide` crate for x86_64 and aarch64
//! with automatic scalar fallback on other architectures.
#![allow(dead_code)]

// SIMD imports for vectorized string operations
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use wide::*;

/// SIMD-optimized delimiter detection and splitting
/// 
/// Pipe-delimiter detection using SIMD (128-bit) for header field splitting.
#[allow(dead_code)]
pub struct SimdDelimiterParser {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pipe_mask: u8x16,
}

impl Default for SimdDelimiterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdDelimiterParser {
    pub fn new() -> Self {
        Self {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            pipe_mask: u8x16::splat(b'|'),
        }
    }
    
    /// SIMD-accelerated pipe delimiter detection for header parsing
    /// 
    /// This function uses SIMD instructions to find pipe delimiters in parallel,
    /// providing significant speedup over scalar string splitting operations.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub fn find_pipe_delimiters_simd(&self, text: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        
        if text.len() < 16 {
            // Use scalar fallback for small strings
            return self.find_pipe_delimiters_scalar(text);
        }
        
        let chunks = text.chunks_exact(16);
        let remainder = chunks.remainder();
        
        for (chunk_idx, chunk) in chunks.enumerate() {
            // Load 16 bytes into SIMD register
            let data = u8x16::new([
                chunk[0], chunk[1], chunk[2], chunk[3],
                chunk[4], chunk[5], chunk[6], chunk[7],
                chunk[8], chunk[9], chunk[10], chunk[11],
                chunk[12], chunk[13], chunk[14], chunk[15],
            ]);
            
            // Compare with pipe delimiter
            let matches = data.cmp_eq(self.pipe_mask);
            
            // Extract match positions
            let match_array = matches.to_array();
            for (i, &is_match) in match_array.iter().enumerate() {
                if is_match != 0 {
                    positions.push(chunk_idx * 16 + i);
                }
            }
        }
        
        // Handle remainder with scalar code
        let remainder_start = text.len() - remainder.len();
        for (i, &byte) in remainder.iter().enumerate() {
            if byte == b'|' {
                positions.push(remainder_start + i);
            }
        }
        
        positions
    }
    
    /// Scalar fallback for pipe delimiter detection
    pub fn find_pipe_delimiters_scalar(&self, text: &[u8]) -> Vec<usize> {
        text.iter()
            .enumerate()
            .filter_map(|(i, &byte)| if byte == b'|' { Some(i) } else { None })
            .collect()
    }
    
    /// Cross-platform pipe delimiter detection with automatic SIMD/scalar selection
    pub fn find_pipe_delimiters(&self, text: &[u8]) -> Vec<usize> {
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        {
            self.find_pipe_delimiters_simd(text)
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            self.find_pipe_delimiters_scalar(text)
        }
    }
}

/// SIMD-accelerated string trimming operations
pub struct SimdStringTrimmer {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    space_mask: u8x16,
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    tab_mask: u8x16,
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    newline_mask: u8x16,
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    carriage_return_mask: u8x16,
}

impl Default for SimdStringTrimmer {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdStringTrimmer {
    pub fn new() -> Self {
        Self {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            space_mask: u8x16::splat(b' '),
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            tab_mask: u8x16::splat(b'\t'),
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            newline_mask: u8x16::splat(b'\n'),
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            carriage_return_mask: u8x16::splat(b'\r'),
        }
    }
    
    /// SIMD-accelerated whitespace detection
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn is_whitespace_simd(&self, data: u8x16) -> u8x16 {
        let space_match = data.cmp_eq(self.space_mask);
        let tab_match = data.cmp_eq(self.tab_mask);
        let newline_match = data.cmp_eq(self.newline_mask);
        let cr_match = data.cmp_eq(self.carriage_return_mask);
        
        // Combine all whitespace matches — u8x16 implements BitOr directly
        space_match | tab_match | newline_match | cr_match
    }
    
    /// Find the start of non-whitespace content using SIMD
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub fn find_trim_start_simd(&self, text: &[u8]) -> usize {
        if text.len() < 16 {
            return self.find_trim_start_scalar(text);
        }
        
        let chunks = text.chunks_exact(16);
        let remainder = chunks.remainder();
        
        for (chunk_idx, chunk) in chunks.enumerate() {
            let data = u8x16::new([
                chunk[0], chunk[1], chunk[2], chunk[3],
                chunk[4], chunk[5], chunk[6], chunk[7],
                chunk[8], chunk[9], chunk[10], chunk[11],
                chunk[12], chunk[13], chunk[14], chunk[15],
            ]);
            
            let whitespace_mask = self.is_whitespace_simd(data);
            let whitespace_array = whitespace_mask.to_array();
            
            // Find first non-whitespace character in this chunk
            for (i, &is_whitespace) in whitespace_array.iter().enumerate() {
                if is_whitespace == 0 {
                    return chunk_idx * 16 + i;
                }
            }
        }
        
        // Check remainder
        let remainder_start = text.len() - remainder.len();
        remainder_start + self.find_trim_start_scalar(remainder)
    }
    
    /// Scalar fallback for finding trim start position
    pub fn find_trim_start_scalar(&self, text: &[u8]) -> usize {
        text.iter()
            .position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .unwrap_or(text.len())
    }
    
    /// Find the end of non-whitespace content using SIMD
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    pub fn find_trim_end_simd(&self, text: &[u8]) -> usize {
        if text.len() < 16 {
            return self.find_trim_end_scalar(text);
        }
        
        // Process from the end backwards
        let chunks: Vec<_> = text.rchunks_exact(16).collect();
        let remainder = &text[..text.len() % 16];
        
        for (chunk_idx, chunk) in chunks.iter().enumerate() {
            let data = u8x16::new([
                chunk[0], chunk[1], chunk[2], chunk[3],
                chunk[4], chunk[5], chunk[6], chunk[7],
                chunk[8], chunk[9], chunk[10], chunk[11],
                chunk[12], chunk[13], chunk[14], chunk[15],
            ]);
            
            let whitespace_mask = self.is_whitespace_simd(data);
            let whitespace_array = whitespace_mask.to_array();
            
            // Find last non-whitespace character in this chunk (search backwards)
            for (i, &is_whitespace) in whitespace_array.iter().enumerate().rev() {
                if is_whitespace == 0 {
                    return text.len() - (chunk_idx * 16) - (16 - i - 1);
                }
            }
        }
        
        // Check remainder
        if !remainder.is_empty() {
            let remainder_end = self.find_trim_end_scalar(remainder);
            if remainder_end > 0 {
                return remainder_end;
            }
        }
        
        0
    }
    
    /// Scalar fallback for finding trim end position
    pub fn find_trim_end_scalar(&self, text: &[u8]) -> usize {
        text.iter()
            .rposition(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .map(|pos| pos + 1)
            .unwrap_or(0)
    }
    
    /// Cross-platform string trimming with automatic SIMD/scalar selection
    pub fn trim_bytes<'a>(&self, text: &'a [u8]) -> &'a [u8] {
        if text.is_empty() {
            return text;
        }
        
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        {
            let start = self.find_trim_start_simd(text);
            if start >= text.len() {
                return &text[0..0]; // Return empty slice from the same lifetime
            }
            let end = self.find_trim_end_simd(text);
            &text[start..end]
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let start = self.find_trim_start_scalar(text);
            if start >= text.len() {
                return &text[0..0]; // Return empty slice from the same lifetime
            }
            let end = self.find_trim_end_scalar(text);
            &text[start..end]
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_delimiter_detection() {
        let parser = SimdDelimiterParser::new();
        let test_string = b"field1|field2|field3|field4";
        let positions = parser.find_pipe_delimiters(test_string);
        
        assert_eq!(positions, vec![6, 13, 20]);
    }

    #[test]
    fn test_simd_string_trimming() {
        let trimmer = SimdStringTrimmer::new();
        
        let test_cases = [
            (b"  hello  ".as_slice(), b"hello".as_slice()),
            (b"\t\nworld\r\n".as_slice(), b"world".as_slice()),
            (b"   ".as_slice(), b"".as_slice()),
            (b"no_trim".as_slice(), b"no_trim".as_slice()),
        ];
        
        for (input, expected) in test_cases {
            let result = trimmer.trim_bytes(input);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_simd_vs_scalar_consistency() {
        let parser = SimdDelimiterParser::new();
        let test_strings = [
            b"a|b|c".as_slice(),
            b"field1|field2|field3|field4|field5".as_slice(),
            b"no_delimiters_here".as_slice(),
            b"|starts_with_delimiter".as_slice(),
            b"ends_with_delimiter|".as_slice(),
            b"||double||delimiters||".as_slice(),
        ];
        
        for test_string in test_strings {
            let simd_result = parser.find_pipe_delimiters(test_string);
            let scalar_result = parser.find_pipe_delimiters_scalar(test_string);
            
            assert_eq!(simd_result, scalar_result, 
                      "SIMD and scalar results differ for: {:?}", 
                      std::str::from_utf8(test_string));
        }
    }

    #[test]
    fn test_empty_and_edge_cases() {
        let parser = SimdDelimiterParser::new();
        let trimmer = SimdStringTrimmer::new();
        
        // Empty string
        assert_eq!(parser.find_pipe_delimiters(b""), Vec::<usize>::new());
        assert_eq!(trimmer.trim_bytes(b""), b"");
        
        // Single character
        assert_eq!(parser.find_pipe_delimiters(b"|"), vec![0usize]);
        assert_eq!(trimmer.trim_bytes(b"a"), b"a");
        
        // Very long string with many delimiters
        let long_string = "a|".repeat(1000);
        let long_bytes = long_string.as_bytes();
        let positions = parser.find_pipe_delimiters(long_bytes);
        assert_eq!(positions.len(), 1000);
        
        // Verify positions are correct
        for (i, &pos) in positions.iter().enumerate() {
            assert_eq!(pos, i * 2 + 1);
        }
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn test_simd_performance_benchmark() {
        let parser = SimdDelimiterParser::new();
        let test_string = "field1|field2|field3|field4|field5|field6|field7|field8".repeat(100);
        let test_bytes = test_string.as_bytes();
        
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = parser.find_pipe_delimiters_simd(test_bytes);
        }
        let simd_time = start.elapsed();
        
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = parser.find_pipe_delimiters_scalar(test_bytes);
        }
        let scalar_time = start.elapsed();
        
        println!("SIMD time: {:?}, Scalar time: {:?}", simd_time, scalar_time);
        println!("Speedup: {:.2}x", scalar_time.as_nanos() as f64 / simd_time.as_nanos() as f64);
        
        // SIMD should be faster for large strings
        // Note: This might not always be true in debug builds
    }
}
