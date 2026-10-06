//! Background jobs.
//!
//! Every long operation (pre-check, analysis, loading results, conserved regions, exports, file
//! dialogs) runs on its own thread and reports back over a one-shot channel that the app polls
//! once per frame; nothing slow runs on the interface thread. A panic inside a job is caught
//! (`catch_unwind`, D62) and reported as [`Poll::Panicked`], which the app shows as an error with
//! "Copy details" instead of losing the window. This module owns the mechanism; the submodules
//! own what each job does.

pub mod analysis;
pub mod dialogs;
pub mod error_context;
pub mod export;
pub mod hcs;
pub mod inspect;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Scheduling of a job's thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// Normal priority (short jobs the user waits for).
    Normal,
    /// Below normal (long computations; the interface stays responsive). The library's own
    /// threads are lowered too, by the analysis's `thread_start` hook: not every system lets
    /// new threads inherit the priority (Windows does not).
    Background,
}

/// Lowers the calling thread's priority; best effort (a refusal leaves it at normal priority).
pub fn lower_priority() {
    let _ = thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Min);
}

/// The state of a job.
#[derive(Debug)]
pub enum Poll<T> {
    /// Still running.
    Pending,
    /// Finished with a value.
    Ready(T),
    /// The job panicked; the message.
    Panicked(String),
    /// The job never started: the system refused a new thread (not a bug); the reason.
    Refused(String),
}

/// How a job ended without a value.
#[derive(Debug)]
enum Stopped {
    Panicked(String),
    Refused(String),
}

/// A running job: its result channel and its cancellation flag.
pub struct Worker<T> {
    rx: Receiver<Result<T, Stopped>>,
    cancel: Arc<AtomicBool>,
    done: bool,
}

impl<T: Send + 'static> Worker<T> {
    /// Runs `job` on a new thread named `name`. The job receives the cancellation flag; `ctx`
    /// (when given) is asked to repaint when the job finishes, so its result shows at once.
    pub fn spawn<F>(name: &str, priority: Priority, ctx: Option<egui::Context>, job: F) -> Self
    where
        F: FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let spawned = std::thread::Builder::new()
            .name(format!("dima-gui-{name}"))
            .spawn(move || {
                if priority == Priority::Background {
                    lower_priority();
                }
                let outcome = catch_unwind(AssertUnwindSafe(|| job(flag))).map_err(|payload| {
                    Stopped::Panicked(crate::util::panic_message(payload.as_ref()))
                });
                // A closed channel means the app no longer wants the result.
                let _ = tx.send(outcome);
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            });
        if let Err(e) = spawned {
            let (tx, rx2) = mpsc::channel();
            let _ = tx.send(Err(Stopped::Refused(e.to_string())));
            return Self {
                rx: rx2,
                cancel,
                done: false,
            };
        }
        Self {
            rx,
            cancel,
            done: false,
        }
    }

    /// Asks the job to stop at its next check.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Non-blocking poll; safe to call every frame. After `Ready`, `Panicked` or `Refused` it
    /// stays `Pending` (the value is taken once).
    pub fn poll(&mut self) -> Poll<T> {
        if self.done {
            return Poll::Pending;
        }
        match self.rx.try_recv() {
            Ok(Ok(v)) => {
                self.done = true;
                Poll::Ready(v)
            }
            Ok(Err(Stopped::Panicked(message))) => {
                self.done = true;
                Poll::Panicked(message)
            }
            Ok(Err(Stopped::Refused(reason))) => {
                self.done = true;
                Poll::Refused(reason)
            }
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Poll::Panicked("the worker thread ended without a result".to_owned())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait<T: Send + 'static>(w: &mut Worker<T>) -> Poll<T> {
        let start = Instant::now();
        loop {
            match w.poll() {
                Poll::Pending if start.elapsed() < Duration::from_secs(10) => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                other => return other,
            }
        }
    }

    #[test]
    fn delivers_a_result_once() {
        let mut w = Worker::spawn("test", Priority::Normal, None, |_| 42);
        assert!(matches!(wait(&mut w), Poll::Ready(42)));
        assert!(matches!(w.poll(), Poll::Pending));
    }

    #[test]
    fn panics_are_caught_and_reported() {
        let mut w: Worker<()> = Worker::spawn("test", Priority::Background, None, |_| {
            panic!("injected panic")
        });
        match wait(&mut w) {
            Poll::Panicked(m) => assert!(m.contains("injected panic"), "{m}"),
            other => panic!("expected a caught panic, got {other:?}"),
        }
    }

    #[test]
    fn jobs_observe_cancellation() {
        let mut w = Worker::spawn("test", Priority::Normal, None, |cancel| {
            let start = Instant::now();
            while !cancel.load(Ordering::Relaxed) {
                if start.elapsed() > Duration::from_secs(10) {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            true
        });
        assert!(matches!(w.poll(), Poll::Pending));
        w.cancel();
        assert!(matches!(wait(&mut w), Poll::Ready(true)));
    }
}
