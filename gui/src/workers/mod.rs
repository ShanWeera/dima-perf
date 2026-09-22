//! Background workers for validation, analysis, and file I/O.
//!
//! Every long-running job runs on a plain `std::thread` and reports back over a
//! one-shot `mpsc` channel. There is no async runtime: the UI polls each worker
//! once per frame from `logic()`, and workers wake the UI with
//! `Context::request_repaint()` when they finish.
//!
//! This module owns the *mechanism* (spawn, cancel, poll, repaint); the
//! submodules own the *policy* for each specific job (SRP).

pub mod analysis;
pub mod io;
pub mod validation;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

/// Outcome of polling a [`Worker`] without blocking.
pub enum Poll<T> {
    /// The job is still running.
    Pending,
    /// The job finished and produced a value. The worker should now be dropped.
    Ready(T),
    /// The worker thread ended without sending a result.
    ///
    /// In debug builds this indicates a panic in the job closure. In release
    /// builds the workspace sets `panic = "abort"`, so a panicking job takes the
    /// whole process down and this variant is unreachable — it exists so the UI
    /// degrades gracefully rather than hanging on a job that will never report.
    Disconnected,
}

/// Handle to one background job: a one-shot result channel plus a cooperative
/// cancellation flag.
///
/// Dropping the handle drops the `Receiver`. The worker's `send` then fails
/// harmlessly and its thread exits once the job returns, so abandoning a job is
/// always safe and leaks nothing.
pub struct Worker<T> {
    rx: Receiver<T>,
    cancel: Arc<AtomicBool>,
}

impl<T: Send + 'static> Worker<T> {
    /// Spawn `job` on a background thread.
    ///
    /// The job receives the cancellation flag so it can hand it to cancellable
    /// library calls (`dima_lib` checks it cooperatively inside its parallel
    /// loops). When the job returns, the result is sent and `ctx` — when
    /// provided — is repainted so the UI observes the result immediately rather
    /// than on the next incidental frame.
    pub fn spawn<F>(ctx: Option<egui::Context>, job: F) -> Self
    where
        F: FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_for_job = Arc::clone(&cancel);

        std::thread::spawn(move || {
            let result = job(cancel_for_job);
            // A failed send just means the UI dropped the handle (job abandoned).
            let _ = tx.send(result);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });

        Self { rx, cancel }
    }

    /// Request cooperative cancellation. The job stops at its next check point.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Non-blocking poll. Safe to call every frame.
    pub fn poll(&self) -> Poll<T> {
        match self.rx.try_recv() {
            Ok(value) => Poll::Ready(value),
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => Poll::Disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivers_result() {
        let worker: Worker<u32> = Worker::spawn(None, |_cancel| 42);
        // Spin until the thread reports; bounded so a regression fails fast.
        for _ in 0..10_000 {
            match worker.poll() {
                Poll::Ready(v) => {
                    assert_eq!(v, 42);
                    return;
                }
                Poll::Pending => std::thread::yield_now(),
                Poll::Disconnected => panic!("worker disconnected without a result"),
            }
        }
        panic!("worker did not deliver a result");
    }

    #[test]
    fn job_observes_cancellation() {
        let worker: Worker<bool> = Worker::spawn(None, |cancel| {
            // Wait until cancellation is observed, then report that we saw it.
            for _ in 0..10_000_000 {
                if cancel.load(Ordering::Relaxed) {
                    return true;
                }
                std::hint::spin_loop();
            }
            false
        });
        worker.cancel();
        for _ in 0..10_000 {
            match worker.poll() {
                Poll::Ready(saw_cancel) => {
                    assert!(saw_cancel, "job should have observed the cancel flag");
                    return;
                }
                Poll::Pending => std::thread::yield_now(),
                Poll::Disconnected => panic!("worker disconnected without a result"),
            }
        }
        panic!("cancelled worker did not report");
    }

    #[test]
    fn poll_is_pending_before_completion() {
        // A job that never finishes must always read as Pending, never Ready.
        let worker: Worker<()> = Worker::spawn(None, |cancel| {
            while !cancel.load(Ordering::Relaxed) {
                std::hint::spin_loop();
            }
        });
        assert!(matches!(worker.poll(), Poll::Pending));
        worker.cancel();
    }
}
