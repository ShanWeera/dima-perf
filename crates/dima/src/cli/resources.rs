//! Resource usage of the process: peak resident memory and CPU time (D103, `-v`).

use std::time::Duration;

/// CPU time and peak memory of the process so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// User-mode CPU time.
    pub user: Duration,
    /// Kernel-mode CPU time.
    pub system: Duration,
    /// Peak resident set size, when the platform reports it.
    pub peak_rss_bytes: Option<u64>,
}

/// Reads the process's resource usage (zeros where unavailable).
#[cfg(unix)]
#[must_use]
pub fn usage() -> Usage {
    let mut ru = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage writes a complete `rusage` on success; it is read only then.
    let ok = unsafe { libc::getrusage(libc::RUSAGE_SELF, ru.as_mut_ptr()) } == 0;
    if !ok {
        return Usage::default();
    }
    // SAFETY: initialised by the successful call above.
    let ru = unsafe { ru.assume_init() };
    let tv = |t: libc::timeval| {
        Duration::from_secs(u64::try_from(t.tv_sec).unwrap_or(0))
            + Duration::from_micros(u64::try_from(t.tv_usec).unwrap_or(0))
    };
    let maxrss = u64::try_from(ru.ru_maxrss).unwrap_or(0);
    // macOS reports bytes; Linux and the BSDs report KiB.
    let peak = if cfg!(target_os = "macos") {
        maxrss
    } else {
        maxrss * 1024
    };
    Usage {
        user: tv(ru.ru_utime),
        system: tv(ru.ru_stime),
        peak_rss_bytes: (peak > 0).then_some(peak),
    }
}

/// Reads the process's resource usage (zeros where unavailable).
#[cfg(windows)]
#[must_use]
pub fn usage() -> Usage {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let ft = |f: FILETIME| {
        Duration::from_nanos(
            ((u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)) * 100,
        )
    };
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut c, mut e, mut k, mut u) = (zero, zero, zero, zero);
    let mut mem = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    // SAFETY: the pseudo-handle of the current process is always valid; the out-parameters are
    // valid, correctly sized structures.
    let (times_ok, mem_ok) = unsafe {
        let p = GetCurrentProcess();
        (
            GetProcessTimes(p, &mut c, &mut e, &mut k, &mut u) != 0,
            GetProcessMemoryInfo(p, &mut mem, mem.cb) != 0,
        )
    };
    Usage {
        user: if times_ok { ft(u) } else { Duration::ZERO },
        system: if times_ok { ft(k) } else { Duration::ZERO },
        peak_rss_bytes: mem_ok.then_some(mem.PeakWorkingSetSize as u64),
    }
}

/// Reads the process's resource usage (unavailable on this platform).
#[cfg(not(any(unix, windows)))]
#[must_use]
pub fn usage() -> Usage {
    Usage::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_is_plausible() {
        let u = usage();
        if cfg!(any(unix, windows)) {
            assert!(u.peak_rss_bytes.unwrap_or(0) > 1_000_000);
        }
    }
}
