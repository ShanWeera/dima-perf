//! Performance reporting utilities for DiMA analysis.
//!
//! Provides phase-level timing and peak memory reporting. Phase timing is collected
//! inside the library's `analyze()` function using `Instant` measurements. Peak memory
//! is retrieved from kernel accounting (zero-overhead: kernel tracks it regardless).

use std::time::Duration;

/// Performance timing data collected during analysis.
/// Phases 1-3 are timed inside the library's `analyze()` function.
/// `output_duration` is filled by the CLI binary after `analyze()` returns.
#[derive(Debug, Clone)]
pub struct PerfReport {
    /// Phase 1: FASTA I/O + validation + k-mer matrix construction
    pub io_duration: Duration,
    /// Phase 2: Shannon entropy computation (parallel via rayon)
    pub entropy_duration: Duration,
    /// Phase 3: Position building + motif classification (parallel via rayon)
    pub building_duration: Duration,
    /// Phase 4: Output serialization (JSON/TSV/binary) — filled by caller
    pub output_duration: Duration,
    pub sequence_count: usize,
    pub position_count: usize,
    pub input_size_bytes: Option<u64>,
}

impl Default for PerfReport {
    fn default() -> Self {
        Self {
            io_duration: Duration::ZERO,
            entropy_duration: Duration::ZERO,
            building_duration: Duration::ZERO,
            output_duration: Duration::ZERO,
            sequence_count: 0,
            position_count: 0,
            input_size_bytes: None,
        }
    }
}

impl PerfReport {
    /// Total wall time across all phases.
    pub fn total_duration(&self) -> Duration {
        self.io_duration + self.entropy_duration + self.building_duration + self.output_duration
    }

    /// Print performance summary to stderr (called at -v verbosity).
    /// Format designed for easy scanning: aligned columns, percentage breakdowns.
    pub fn print(&self) {
        let total = self.total_duration();
        let total_secs = total.as_secs_f64();

        // Avoid division by zero for extremely fast (sub-microsecond) runs
        let pct = |d: Duration| -> f64 {
            if total_secs > 0.0 { d.as_secs_f64() / total_secs * 100.0 } else { 0.0 }
        };

        eprintln!("  Phase timing:");
        eprintln!(
            "    I/O + validation:     {:>7.2}s  ({:>5.1}%)",
            self.io_duration.as_secs_f64(), pct(self.io_duration)
        );
        eprintln!(
            "    Entropy computation:  {:>7.2}s  ({:>5.1}%)",
            self.entropy_duration.as_secs_f64(), pct(self.entropy_duration)
        );
        eprintln!(
            "    Position building:    {:>7.2}s  ({:>5.1}%)",
            self.building_duration.as_secs_f64(), pct(self.building_duration)
        );
        eprintln!(
            "    Output serialization: {:>7.2}s  ({:>5.1}%)",
            self.output_duration.as_secs_f64(), pct(self.output_duration)
        );

        eprintln!("  Resources:");
        if let Some(peak) = peak_memory_bytes() {
            eprintln!("    Peak memory:  {}", format_bytes(peak));
        }
        eprintln!("    Threads used: {}", rayon::current_num_threads());
        if let Some(size) = self.input_size_bytes {
            eprintln!(
                "    Input size:   {} ({} sequences, {} positions)",
                format_bytes(size), self.sequence_count, self.position_count
            );
        } else {
            eprintln!(
                "    Input:        {} sequences, {} positions",
                self.sequence_count, self.position_count
            );
        }
    }
}

/// Format a byte count as a human-readable string (e.g., "1.23 GB").
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Zero-overhead peak RSS via kernel accounting.
/// The kernel tracks this regardless of whether we read it, so querying it
/// has no performance impact on the analysis itself.
///
/// Linux: ru_maxrss in kilobytes.
/// macOS: ru_maxrss in bytes.
#[cfg(unix)]
pub fn peak_memory_bytes() -> Option<u64> {
    unsafe {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) == 0 {
            let maxrss = usage.assume_init().ru_maxrss;
            #[cfg(target_os = "linux")]
            { Some(maxrss as u64 * 1024) } // Linux reports kilobytes
            #[cfg(target_os = "macos")]
            { Some(maxrss as u64) }         // macOS reports bytes
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            { Some(maxrss as u64 * 1024) }  // BSD variants report kilobytes
        } else {
            None
        }
    }
}

/// Fallback for non-Unix platforms (Windows handled separately in future).
#[cfg(not(unix))]
pub fn peak_memory_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1536), "2 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
        assert_eq!(format_bytes(18_000_000_000), "16.76 GB");
    }

    #[test]
    fn test_perf_report_total_duration() {
        let report = PerfReport {
            io_duration: Duration::from_secs(1),
            entropy_duration: Duration::from_secs(2),
            building_duration: Duration::from_secs(3),
            output_duration: Duration::from_secs(4),
            ..Default::default()
        };
        assert_eq!(report.total_duration(), Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn test_peak_memory_returns_some() {
        // On any Unix system, getrusage should succeed for the current process
        let peak = peak_memory_bytes();
        assert!(peak.is_some(), "peak_memory_bytes should return Some on Unix");
        assert!(peak.unwrap() > 0, "peak memory should be positive");
    }
}
