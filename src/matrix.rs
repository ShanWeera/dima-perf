//! Adaptive k-mer matrix storage: RAM-backed or disk-backed depending on available memory.
//!
//! The k-mer matrix is the largest data structure during analysis (sequences × positions × 8 bytes).
//! For large datasets (>10GB matrices), this module can transparently spill to disk while
//! maintaining identical output via memory-mapped access patterns.

use std::path::{Path, PathBuf};

/// Estimate matrix memory requirements by peeking at the first FASTA record.
/// MSA requires all sequences to be the same length, so the first record gives
/// the exact alignment_length. Combined with file_size gives an accurate estimate.
///
/// Cost: <1ms even on HDD (reads first ~500 bytes of the file).
pub fn estimate_matrix_bytes(path: &Path, kmer_length: usize) -> Option<u64> {
    let mut reader = needletail::parse_fastx_file(path).ok()?;
    let first_record = reader.next()?.ok()?;
    let alignment_length = first_record.seq().len();
    drop(reader);

    let file_size = std::fs::metadata(path).ok()?.len();
    // Estimate avg record size: sequence + ~80 bytes header overhead
    let avg_record_bytes = alignment_length as u64 + 80;
    let estimated_seq_count = file_size / avg_record_bytes.max(1);
    let position_count = alignment_length.saturating_sub(kmer_length).saturating_add(1);

    // 8 bytes per u64 k-mer encoding
    Some(estimated_seq_count * position_count as u64 * 8)
}

/// Detect available physical RAM on the current system.
/// Returns None if detection fails (graceful fallback to RAM mode).
#[cfg(target_os = "linux")]
pub fn available_ram_bytes() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    // Prefer MemAvailable (includes reclaimable buffers/cache)
    for line in content.lines() {
        if line.starts_with("MemAvailable:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    // Fallback: MemFree + Buffers + Cached
    let mut free = 0u64;
    let mut buffers = 0u64;
    let mut cached = 0u64;
    for line in content.lines() {
        if line.starts_with("MemFree:") {
            free = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if line.starts_with("Buffers:") {
            buffers = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if line.starts_with("Cached:") && !line.starts_with("CachedSwap") {
            cached = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    if free > 0 {
        Some((free + buffers + cached) * 1024)
    } else {
        None
    }
}

/// macOS: Uses Mach kernel host_statistics64 API to get actual available memory.
/// Formula: available = (free_count + inactive_count) × page_size
#[cfg(target_os = "macos")]
#[allow(deprecated)] // mach_host_self is deprecated in libc; mach2 crate not used here
pub fn available_ram_bytes() -> Option<u64> {
    unsafe {
        let mut count = libc::HOST_VM_INFO64_COUNT as u32;
        let mut stats: libc::vm_statistics64 = std::mem::zeroed();
        let kr = libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            &mut stats as *mut _ as *mut i32,
            &mut count,
        );
        if kr != libc::KERN_SUCCESS { return None; }
        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as u64;
        Some((stats.free_count as u64 + stats.inactive_count as u64) * page_size)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn available_ram_bytes() -> Option<u64> {
    None // Fallback: assume unlimited → always use RAM mode
}

/// Resolve the directory for temporary matrix files.
/// Priority: --temp-dir flag > $TMPDIR env > input file's parent directory.
/// Never falls back to system /tmp (may be on a small tmpfs with limited space).
pub fn resolve_temp_dir(
    flag: Option<&Path>,
    input_path: Option<&Path>,
) -> PathBuf {
    if let Some(dir) = flag {
        return dir.to_path_buf();
    }
    if let Ok(dir) = std::env::var("TMPDIR") {
        let p = PathBuf::from(&dir);
        if p.is_dir() { return p; }
        tracing::warn!(path = %dir, "$TMPDIR does not exist, falling back to input directory");
    }
    match input_path {
        Some(path) => path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Determine whether disk-backed mode should be engaged.
/// Threshold: matrix estimate > 75% of available RAM.
pub fn should_use_disk_mode(
    matrix_estimate_bytes: u64,
    force_ram: bool,
    force_disk: bool,
) -> bool {
    if force_ram { return false; }
    if force_disk { return true; }

    match available_ram_bytes() {
        Some(available) => {
            let threshold = available * 3 / 4; // 75%
            if matrix_estimate_bytes > threshold {
                tracing::info!(
                    matrix_estimate = matrix_estimate_bytes,
                    available_ram = available,
                    threshold = threshold,
                    "estimated matrix exceeds 75% of available RAM, using disk-backed mode"
                );
                true
            } else {
                false
            }
        }
        None => {
            // Cannot detect RAM — default to RAM mode (safe for most workstations)
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_temp_dir_flag_wins() {
        let flag = Path::new("/custom/tmp");
        assert_eq!(
            resolve_temp_dir(Some(flag), Some(Path::new("/data/input.fasta"))),
            PathBuf::from("/custom/tmp")
        );
    }

    #[test]
    fn test_resolve_temp_dir_input_parent_fallback() {
        // Temporarily unset TMPDIR so we can test the fallback to input parent
        let saved_tmpdir = std::env::var("TMPDIR").ok();
        unsafe { std::env::remove_var("TMPDIR"); }

        let result = resolve_temp_dir(None, Some(Path::new("/data/sequences/input.fasta")));
        assert_eq!(result, PathBuf::from("/data/sequences"));

        // Restore TMPDIR
        if let Some(val) = saved_tmpdir {
            unsafe { std::env::set_var("TMPDIR", val); }
        }
    }

    #[test]
    fn test_should_use_disk_force_flags() {
        assert!(!should_use_disk_mode(u64::MAX, true, false));
        assert!(should_use_disk_mode(0, false, true));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_available_ram_is_positive() {
        let ram = available_ram_bytes();
        assert!(ram.is_some(), "RAM detection should succeed on this platform");
        assert!(ram.unwrap() > 0, "available RAM should be positive");
    }
}
