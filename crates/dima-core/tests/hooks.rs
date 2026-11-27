//! The run's hooks: `thread_start` runs in every thread the analysis starts (the desktop app
//! lowers their priority there, D101), and `threads_ready` once they all run, before counting
//! (the command line starts its progress display there, D110). A `thread_start` hook that
//! delays the reader also shows where the wait for the first batch is counted.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use dima_core::config::AnalysisParams;
use dima_core::control::{Hook, Phase, RunControl};
use dima_core::input::Source;

#[test]
fn hooks_run_in_every_analysis_thread_and_once_before_counting() {
    let source = Source::Path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../samples/mers_spike_aa.fasta"
        )
        .into(),
    );
    let names = Arc::new(Mutex::new(Vec::new()));
    let ready = Arc::new(AtomicUsize::new(0));
    let control = RunControl {
        threads: NonZeroUsize::new(3),
        thread_start: Some(Hook::new({
            let names = Arc::clone(&names);
            move || {
                let name = std::thread::current().name().unwrap_or("").to_owned();
                names.lock().unwrap().push(name);
            }
        })),
        ..RunControl::new()
    };
    let progress = Arc::clone(&control.progress);
    let control = RunControl {
        threads_ready: Some(Hook::new({
            let ready = Arc::clone(&ready);
            move || {
                // Every thread runs, and counting has not started.
                assert_eq!(progress.snapshot().phase, Phase::Opening);
                ready.fetch_add(1, Ordering::Relaxed);
            }
        })),
        ..control
    };
    dima_core::analyze(&source, &AnalysisParams::default(), &control).unwrap();
    assert_eq!(ready.load(Ordering::Relaxed), 1);
    let mut names = names.lock().unwrap().clone();
    names.sort();
    assert_eq!(
        names,
        [
            "dima-open",
            "dima-reader",
            "dima-sha256",
            "dima-worker-0",
            "dima-worker-1",
            "dima-worker-2"
        ]
    );
}

/// The wait for the first batch belongs to the opening phase: a reader that is slow to start
/// (a slow disk, a pipe that has not written yet) is not counted as input that limited the
/// counting (`waited_for_input`), which the command line reports with `-v`.
#[test]
fn the_first_batch_is_waited_for_while_opening() {
    let source = Source::Path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../samples/mers_spike_aa.fasta"
        )
        .into(),
    );
    const DELAY: std::time::Duration = std::time::Duration::from_millis(800);
    let control = RunControl {
        thread_start: Some(Hook::new(|| {
            if std::thread::current().name() == Some("dima-reader") {
                std::thread::sleep(DELAY);
            }
        })),
        ..RunControl::new()
    };
    let e = dima_core::analyze(&source, &AnalysisParams::default(), &control)
        .unwrap()
        .execution;
    let (first, opening) = e.phases[0];
    assert_eq!(first, Phase::Opening);
    assert!(opening >= DELAY * 3 / 4, "opening took {opening:?}");
    // The sample is one batch: counting waits only for the end of the input.
    assert!(
        e.waited_for_input < DELAY / 2,
        "waited {:?} while counting",
        e.waited_for_input
    );
}
