//! Interrupts and termination (D65, D96).
//!
//! The first SIGINT/SIGTERM/SIGHUP (Ctrl-C/Ctrl-Break/console close on Windows) sets the
//! cancellation flag the analysis polls; the run then stops, its sinks are dropped (removing
//! their temporary files) and the process ends *by the same signal*, so shells and workflow
//! managers see the conventional status (130, 143, 129). A second signal, or 3 seconds without
//! progress, removes any remaining temporary files and ends the process at once. Signals that
//! were ignored when the process started (`nohup`, background jobs) stay ignored.
//!
//! On Unix the other signals that end a process by default and that batch systems send ahead
//! of a limit are handled like SIGTERM: SIGUSR1 and SIGUSR2 (Grid Engine's warnings before a
//! run-time limit), SIGXCPU (a CPU-time limit) and SIGALRM. SIGXFSZ is ignored, so a file-size
//! limit (`ulimit -f`) fails the write with an error, which removes the temporary file, instead
//! of killing the process mid-write. SIGQUIT keeps its default (a core dump, for debugging);
//! SIGKILL cannot be handled and can leave a hidden temporary file (`.NAME.dima-PID-N.tmp`).

#[cfg(unix)]
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
#[cfg(unix)]
use std::time::Duration;

/// Grace period after the first signal before the process is ended regardless.
#[cfg(unix)]
const GRACE: Duration = Duration::from_secs(3);

/// Shared state between the handler and the main thread.
#[derive(Clone, Debug)]
pub struct Signals {
    /// Set on the first signal.
    pub cancel: Arc<AtomicBool>,
    received: Arc<AtomicI32>,
}

impl Signals {
    /// The exit status for a cancelled run: 128 + signal number (130 for Ctrl-C).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self.received.load(Ordering::SeqCst) {
            0 => 130,
            n => u8::try_from(128 + n).unwrap_or(130),
        }
    }

    /// Ends the process the way the received signal would have (call after cleaning up).
    pub fn terminate(&self) -> ! {
        terminate(self.received.load(Ordering::SeqCst), self.exit_code())
    }
}

/// Ends the process at once from the signal or grace thread: removes the temporary outputs,
/// then terminates without running exit handlers (the main thread may be anywhere).
fn force(sig: i32, code: u8) -> ! {
    dima_core::output::remove_temporaries();
    #[cfg(unix)]
    {
        reraise(sig);
        // SAFETY: `_exit` never returns and only ends the process.
        unsafe { libc::_exit(i32::from(code)) }
    }
    #[cfg(not(unix))]
    terminate(sig, code)
}

#[cfg(unix)]
fn terminate(sig: i32, code: u8) -> ! {
    reraise(sig);
    // Still alive: PID 1 (e.g. in a container) ignores signals whose action is the default.
    std::process::exit(i32::from(code))
}

/// Restores the default action of `sig`, unblocks it in this thread and raises it, so the
/// process ends *by* the signal. Unlike `signal_hook::low_level::emulate_default_handler`, it
/// never falls back to `abort()`: as PID 1 the signal is ignored and the caller exits with
/// 128 + n instead (D96).
#[cfg(unix)]
fn reraise(sig: i32) {
    if sig == 0 {
        return;
    }
    // SAFETY: plain libc calls on zero-initialised, then properly initialised, local values;
    // `sigaction` with a null old-action pointer and `raise` have no other preconditions.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&raw mut action.sa_mask);
        if libc::sigaction(sig, &raw const action, std::ptr::null_mut()) != 0 {
            return;
        }
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&raw mut set);
        libc::sigaddset(&raw mut set, sig);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &raw const set, std::ptr::null_mut());
        libc::raise(sig);
    }
}

#[cfg(not(unix))]
fn terminate(_sig: i32, code: u8) -> ! {
    std::process::exit(i32::from(code))
}

#[cfg(unix)]
fn ignored_at_startup(sig: i32) -> bool {
    let mut old = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    // SAFETY: querying (null new action) only writes the current action into `old`.
    let ok = unsafe { libc::sigaction(sig, std::ptr::null(), old.as_mut_ptr()) } == 0;
    // SAFETY: zero-initialised and filled by a successful call.
    ok && unsafe { old.assume_init() }.sa_sigaction == libc::SIG_IGN
}

/// Ignores `sig` (best effort: if it fails, the signal keeps its default action).
#[cfg(unix)]
fn ignore(sig: i32) {
    // SAFETY: a zero-initialised `sigaction` with SIG_IGN, an emptied mask and a null old-action
    // pointer is a complete, valid argument.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = libc::SIG_IGN;
        libc::sigemptyset(&raw mut action.sa_mask);
        libc::sigaction(sig, &raw const action, std::ptr::null_mut());
    }
}

/// Installs the handlers. `announce` prints the "cancelling..." notice.
///
/// # Errors
/// If the handler thread cannot be started.
#[cfg(unix)]
pub fn install(announce: bool) -> std::io::Result<Signals> {
    use signal_hook::consts::{SIGALRM, SIGHUP, SIGINT, SIGTERM, SIGUSR1, SIGUSR2, SIGXCPU};
    let state = Signals {
        cancel: Arc::new(AtomicBool::new(false)),
        received: Arc::new(AtomicI32::new(0)),
    };
    ignore(libc::SIGXFSZ);
    let wanted: Vec<i32> = [SIGINT, SIGTERM, SIGHUP, SIGUSR1, SIGUSR2, SIGXCPU, SIGALRM]
        .into_iter()
        .filter(|&s| !ignored_at_startup(s))
        .collect();
    let mut iter = signal_hook::iterator::Signals::new(&wanted)?;
    let s = state.clone();
    std::thread::Builder::new()
        .name("dima-signals".into())
        .spawn(move || {
            for sig in iter.forever() {
                if s.received
                    .compare_exchange(0, sig, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    s.cancel.store(true, Ordering::SeqCst);
                    if announce {
                        // On a fresh line: after the erased progress line, or after the
                        // terminal's echo of ^C (only Ctrl-C on a terminal echoes).
                        let echoed = sig == SIGINT && std::io::stderr().is_terminal();
                        let fresh = if !crate::cli::progress::clear() && echoed {
                            "\n"
                        } else {
                            ""
                        };
                        let _ = writeln!(
                            std::io::stderr(),
                            "{fresh}cancelling... (press Ctrl-C again to stop immediately)"
                        );
                    }
                    let code = s.exit_code();
                    let _ =
                        std::thread::Builder::new()
                            .name("dima-grace".into())
                            .spawn(move || {
                                std::thread::sleep(GRACE);
                                force(sig, code);
                            });
                } else {
                    // Ended by the first signal, as the exit status and run report say.
                    force(s.received.load(Ordering::SeqCst), s.exit_code());
                }
            }
        })?;
    Ok(state)
}

#[cfg(windows)]
mod windows {
    use super::{Arc, AtomicBool, AtomicI32, Ordering, Signals};
    use std::io::Write;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{FALSE, TRUE};
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
        SetConsoleCtrlHandler,
    };
    use windows_sys::core::BOOL;

    static STATE: OnceLock<(Signals, bool)> = OnceLock::new();
    /// Set by the main thread once cleanup is complete.
    pub static DONE: AtomicBool = AtomicBool::new(false);

    /// Pseudo signal numbers giving the documented exit codes (130, 129, 143).
    const fn pseudo(ctrl: u32) -> i32 {
        match ctrl {
            CTRL_CLOSE_EVENT => 1,
            CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => 15,
            _ => 2,
        }
    }

    unsafe extern "system" fn handler(ctrl: u32) -> BOOL {
        let Some((s, announce)) = STATE.get() else {
            return FALSE;
        };
        let first = s
            .received
            .compare_exchange(0, pseudo(ctrl), Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        s.cancel.store(true, Ordering::SeqCst);
        match ctrl {
            CTRL_C_EVENT | CTRL_BREAK_EVENT => {
                if first {
                    if *announce {
                        // On a fresh line: after the erased progress line, or after the
                        // terminal's echo of ^C.
                        let fresh = if crate::cli::progress::clear() {
                            ""
                        } else {
                            "\n"
                        };
                        let _ = writeln!(
                            std::io::stderr(),
                            "{fresh}cancelling... (press Ctrl-C again to stop immediately)"
                        );
                    }
                    TRUE
                } else {
                    super::force(0, s.exit_code())
                }
            }
            _ => {
                // The console is closing: Windows ends the process ~5 s after this returns.
                for _ in 0..40 {
                    if DONE.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                dima_core::output::remove_temporaries();
                FALSE
            }
        }
    }

    pub fn install(announce: bool) -> std::io::Result<Signals> {
        let state = Signals {
            cancel: Arc::new(AtomicBool::new(false)),
            received: Arc::new(AtomicI32::new(0)),
        };
        let _ = STATE.set((state.clone(), announce));
        // SAFETY: registers a handler with the documented signature for the process lifetime.
        if unsafe { SetConsoleCtrlHandler(Some(handler), TRUE) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(state)
    }
}

/// Installs the handlers. `announce` prints the "cancelling..." notice.
///
/// # Errors
/// If the console handler cannot be registered.
#[cfg(windows)]
pub fn install(announce: bool) -> std::io::Result<Signals> {
    windows::install(announce)
}

/// Marks cleanup as finished (lets a closing Windows console end the process promptly).
pub fn cleanup_done() {
    #[cfg(windows)]
    windows::DONE.store(true, Ordering::SeqCst);
}

#[cfg(not(any(unix, windows)))]
pub fn install(_announce: bool) -> std::io::Result<Signals> {
    Ok(Signals {
        cancel: Arc::new(AtomicBool::new(false)),
        received: Arc::new(AtomicI32::new(0)),
    })
}
