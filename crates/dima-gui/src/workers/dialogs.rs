//! File dialogs (D100).
//!
//! The native dialog (rfd's `AsyncFileDialog`: the desktop portal on Linux, the system dialogs
//! on macOS and Windows) is created on the interface thread, parented to the window, and awaited
//! on a worker, so a slow or hung portal never freezes the app. While it is open, the app offers
//! "Use built-in browser" (`egui-file-dialog`), which it also opens by itself when the native
//! dialog is not available or returns at once without a choice.

use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll as TaskPoll, Wake, Waker};
use std::time::{Duration, Instant};

use super::{Priority, Worker};

/// File types offered when opening an input: the library's FASTA and compression extensions,
/// and results JSON.
#[must_use]
pub fn input_extensions() -> Vec<&'static str> {
    use dima_core::input::{COMPRESSION_EXTENSIONS, FASTA_EXTENSIONS};
    FASTA_EXTENSIONS
        .iter()
        .chain(COMPRESSION_EXTENSIONS)
        .map(|e| e.trim_start_matches('.'))
        .chain(["json"])
        .collect()
}

/// A native dialog that returns without a choice faster than this is taken to have failed
/// (no portal, no display), and the built-in browser opens instead.
pub const FAILED_FAST: Duration = Duration::from_millis(300);

/// What a dialog asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogKind {
    /// An existing file to open.
    Open,
    /// A file to write, with a suggested name and its extensions.
    Save {
        /// Suggested file name.
        default_name: String,
        /// Filter name and extensions (without dots).
        filter: Option<(String, Vec<String>)>,
    },
    /// A folder (the TSV tables).
    Folder,
}

/// Whether the native dialog can work here: on Linux it needs the session bus (desktop portal)
/// or a local display for zenity; a remote shell without either gets the built-in browser.
#[must_use]
pub fn native_available() -> bool {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        return true;
    }
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    let bus = set("DBUS_SESSION_BUS_ADDRESS") || set("XDG_RUNTIME_DIR");
    let remote = set("SSH_CONNECTION") || set("SSH_TTY");
    bus && !(remote && !set("DISPLAY") && !set("WAYLAND_DISPLAY"))
}

/// A native dialog being shown.
pub struct NativeDialog {
    /// The worker awaiting the dialog.
    pub worker: Worker<Option<PathBuf>>,
    /// When it was opened (to detect an immediate failure).
    pub opened: Instant,
}

/// Opens a native dialog. Must be called on the interface thread (macOS requires it); `parent`
/// is the app window when available.
#[must_use]
pub fn spawn_native(
    kind: &DialogKind,
    title: &str,
    start_dir: Option<&std::path::Path>,
    parent: Option<&eframe::Frame>,
    ctx: Option<egui::Context>,
) -> NativeDialog {
    let mut d = rfd::AsyncFileDialog::new().set_title(title);
    if let Some(dir) = start_dir {
        d = d.set_directory(dir);
    }
    if let Some(frame) = parent {
        d = d.set_parent(frame);
    }
    let future: std::pin::Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>> = match kind
    {
        DialogKind::Open => Box::pin(
            d.add_filter("Alignments and DiMA results", &input_extensions())
                .add_filter("All files", &["*"])
                .pick_file(),
        ),
        DialogKind::Save {
            default_name,
            filter,
        } => {
            let mut d = d
                .set_file_name(default_name)
                .set_can_create_directories(true);
            if let Some((name, exts)) = filter {
                d = d.add_filter(name, exts);
            }
            Box::pin(d.save_file())
        }
        DialogKind::Folder => Box::pin(d.set_can_create_directories(true).pick_folder()),
    };
    NativeDialog {
        worker: Worker::spawn("dialog", Priority::Normal, ctx, move |_| {
            block_on(future).map(|h| h.path().to_path_buf())
        }),
        opened: Instant::now(),
    }
}

struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Runs a future to completion on the current thread (parking between polls).
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let TaskPoll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_runs_futures_woken_from_other_threads() {
        assert_eq!(block_on(async { 7 }), 7);
        // A future that is woken by another thread after a delay.
        struct Delayed {
            ready: Arc<std::sync::atomic::AtomicBool>,
            started: bool,
        }
        impl Future for Delayed {
            type Output = u8;
            fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> TaskPoll<u8> {
                if self.ready.load(std::sync::atomic::Ordering::SeqCst) {
                    return TaskPoll::Ready(3);
                }
                if !self.started {
                    self.started = true;
                    let (flag, waker) = (Arc::clone(&self.ready), cx.waker().clone());
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(20));
                        flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        waker.wake();
                    });
                }
                TaskPoll::Pending
            }
        }
        let f = Delayed {
            ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            started: false,
        };
        assert_eq!(block_on(f), 3);
    }
}
