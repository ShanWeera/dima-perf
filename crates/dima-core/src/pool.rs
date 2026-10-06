//! The worker pool of an analysis, fitted to the threads the system allows (D110).
//!
//! A system can refuse a new thread: a process or container limit (`ulimit -u`, a cgroup's
//! pids limit, macOS's per-process thread limit) or a lack of memory. The run then continues
//! with the threads it got, because results never depend on the thread count; only a refusal of
//! the first worker stops it ([`DimaError::System`], exit 3).
//!
//! rayon tells the threads of a failed build to exit but does not wait for them, so they still
//! count against the limit when the build is retried. Each build therefore spawns its threads
//! itself, keeps their handles and, after a refusal, joins the ones it started, pauses for 1 ms
//! (the system releases an exited thread shortly after `join` returns) and retries with as many
//! threads as it managed to start ("join and fit"). When not even one started, it pauses once and
//! tries a single thread. A refusal means fewer threads started than asked for, so the size
//! strictly decreases and the loop ends.

use std::io;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::control::Hook;
use crate::error::{DimaError, Result};

/// The pause before a retry, so the threads of the failed attempt are released.
const RELEASE_PAUSE: Duration = Duration::from_millis(1);

/// A worker pool and the number of threads it runs.
pub(crate) struct Fitted {
    /// The pool.
    pub pool: rayon::ThreadPool,
    /// Threads started: `requested`, or fewer when the system refused some.
    pub started: usize,
}

/// Starts a thread; a seam for tests, which simulate a system that refuses threads.
pub(crate) trait Spawner {
    /// Starts `run` on a new thread named `name`.
    ///
    /// # Errors
    /// The operating system's refusal.
    fn spawn(
        &mut self,
        name: Option<String>,
        run: Box<dyn FnOnce() + Send>,
    ) -> io::Result<JoinHandle<()>>;
}

/// The operating system's threads.
pub(crate) struct OsThreads;

impl Spawner for OsThreads {
    fn spawn(
        &mut self,
        name: Option<String>,
        run: Box<dyn FnOnce() + Send>,
    ) -> io::Result<JoinHandle<()>> {
        let mut b = thread::Builder::new();
        if let Some(name) = name {
            b = b.name(name);
        }
        b.spawn(run)
    }
}

/// Builds a pool of `requested` worker threads, or of as many as the system allows.
///
/// # Errors
/// [`DimaError::System`] when not even one worker thread can be started.
pub(crate) fn build(requested: usize, thread_start: Option<Hook>) -> Result<Fitted> {
    build_with(requested, thread_start, &mut OsThreads)
}

/// [`build`] with a given way of starting threads.
pub(crate) fn build_with(
    requested: usize,
    thread_start: Option<Hook>,
    spawner: &mut dyn Spawner,
) -> Result<Fitted> {
    let mut size = requested.max(1);
    let mut paused = false;
    loop {
        let mut started: Vec<JoinHandle<()>> = Vec::new();
        let start = thread_start.clone();
        let built = rayon::ThreadPoolBuilder::new()
            .num_threads(size)
            .thread_name(|i| format!("dima-worker-{i}"))
            .start_handler(move |_| {
                if let Some(h) = &start {
                    h.call();
                }
            })
            .spawn_handler(|t| {
                let name = t.name().map(str::to_owned);
                started.push(spawner.spawn(name, Box::new(move || t.run()))?);
                Ok(())
            })
            .build();
        match built {
            Ok(pool) => {
                return Ok(Fitted {
                    pool,
                    started: size,
                });
            }
            Err(e) => {
                let count = started.len();
                // rayon has told them to exit; wait until they have, so they no longer count
                // against the system's limit. A worker that panicked has exited too.
                for h in started {
                    let _ = h.join();
                }
                if count > 0 {
                    // `count < size`, because the build failed.
                    thread::sleep(RELEASE_PAUSE);
                    size = count;
                } else if !paused {
                    paused = true;
                    thread::sleep(RELEASE_PAUSE);
                    size = 1;
                } else {
                    return Err(DimaError::system("a worker thread", os_error(&e)));
                }
            }
        }
    }
}

/// The operating system's error inside a failed pool build. rayon's error both displays and
/// returns (as its source) the same `io::Error`; wrapping it would print the cause twice.
fn os_error(e: &rayon::ThreadPoolBuildError) -> io::Error {
    match std::error::Error::source(e).and_then(|s| s.downcast_ref::<io::Error>()) {
        Some(io) => io.raw_os_error().map_or_else(
            || io::Error::new(io.kind(), io.to_string()),
            io::Error::from_raw_os_error,
        ),
        None => io::Error::other(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A system that allows `limit` live threads (counting those of earlier attempts until
    /// they are joined) and refuses the rest with EAGAIN.
    struct Limited {
        limit: usize,
        live: Arc<AtomicUsize>,
        attempts: usize,
    }

    impl Limited {
        fn new(limit: usize) -> Self {
            Self {
                limit,
                live: Arc::new(AtomicUsize::new(0)),
                attempts: 0,
            }
        }
    }

    impl Spawner for Limited {
        fn spawn(
            &mut self,
            name: Option<String>,
            run: Box<dyn FnOnce() + Send>,
        ) -> io::Result<JoinHandle<()>> {
            self.attempts += 1;
            if self.live.fetch_add(1, Ordering::SeqCst) >= self.limit {
                self.live.fetch_sub(1, Ordering::SeqCst);
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            let live = Arc::clone(&self.live);
            OsThreads.spawn(
                name,
                Box::new(move || {
                    run();
                    live.fetch_sub(1, Ordering::SeqCst);
                }),
            )
        }
    }

    fn workers(pool: &rayon::ThreadPool) -> usize {
        pool.current_num_threads()
    }

    #[test]
    fn a_pool_the_system_allows_is_built_as_requested() {
        let mut s = Limited::new(8);
        let f = build_with(8, None, &mut s).unwrap();
        assert_eq!((f.started, workers(&f.pool)), (8, 8));
        assert_eq!(s.attempts, 8);
    }

    #[test]
    fn a_refused_pool_continues_with_fewer_threads() {
        for (requested, limit) in [(64, 16), (8, 7), (2, 1), (5, 3), (100, 2)] {
            let mut s = Limited::new(limit);
            let f = build_with(requested, None, &mut s).unwrap();
            // As many as the system allowed.
            assert_eq!(f.started, limit, "{requested} of {limit}");
            assert_eq!(workers(&f.pool), f.started);
            // The pool works.
            let sum: u64 = f.pool.install(|| {
                use rayon::prelude::*;
                (1..=1000u64).into_par_iter().sum()
            });
            assert_eq!(sum, 500_500);
        }
    }

    #[test]
    fn no_thread_at_all_is_a_system_error() {
        let mut s = Limited::new(0);
        let Err(e) = build_with(4, None, &mut s) else {
            panic!("a pool without threads")
        };
        assert!(matches!(e, DimaError::System { .. }), "{e:?}");
        assert_eq!(e.class().code(), 3);
        assert_eq!(e.to_string(), "cannot start a worker thread");
        // 4 refused, then one retry with a single thread after the pause.
        assert_eq!(s.attempts, 2);
        let mut s = Limited::new(0);
        assert!(build_with(1, None, &mut s).is_err());
        assert_eq!(
            s.attempts, 2,
            "a single thread is retried once after the pause"
        );
    }

    #[test]
    fn the_threads_of_a_failed_attempt_are_released_first() {
        let mut s = Limited::new(6);
        let live = Arc::clone(&s.live);
        let f = build_with(32, None, &mut s).unwrap();
        assert_eq!(f.started, 6);
        // Only the final pool's threads are alive.
        assert_eq!(live.load(Ordering::SeqCst), 6);
        drop(f);
    }
}
