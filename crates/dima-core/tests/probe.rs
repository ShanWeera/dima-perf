//! D108: the pre-check and the run compute the first batch's facts with one function, so a run
//! given the pre-check's facts proceeds exactly when the input is unchanged, and stops before
//! counting when it is not.

use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dima_core::DimaError;
use dima_core::config::AnalysisParams;
use dima_core::control::{Phase, Progress, RunControl};
use dima_core::input::{BatchLimits, Source};
use dima_core::validation::inspect;

fn sample() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../samples/mers_spike_aa.fasta"
    ))
    .unwrap()
}

/// The sample stored plain and in every supported compression.
fn stored(dir: &Path) -> Vec<PathBuf> {
    let plain = sample();
    let mut files = Vec::new();
    let mut put = |name: &str, bytes: Vec<u8>| {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        files.push(p);
    };
    put("a.fasta", plain.clone());
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&plain).unwrap();
    put("a.fasta.gz", gz.finish().unwrap());
    let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    bz.write_all(&plain).unwrap();
    put("a.fasta.bz2", bz.finish().unwrap());
    put(
        "a.fasta.zst",
        zstd::encode_all(plain.as_slice(), 3).unwrap(),
    );
    files
}

fn control(
    threads: usize,
    limits: BatchLimits,
    probe: Option<dima_core::control::ProbeFacts>,
) -> RunControl {
    RunControl {
        threads: NonZeroUsize::new(threads),
        batch_limits: limits,
        expected_probe: probe,
        ..RunControl::new()
    }
}

#[test]
fn an_unchanged_input_matches_its_pre_check() {
    let dir = tempfile::tempdir().unwrap();
    let tiny = BatchLimits {
        max_records: 7,
        max_bytes: 1000,
    };
    for path in stored(dir.path()) {
        let source = Source::Path(path.clone());
        let checked = inspect(&source).unwrap();
        for threads in [1, 2, 4] {
            for limits in [BatchLimits::default(), tiny] {
                let c = control(threads, limits, Some(checked.probe.clone()));
                let r = dima_core::analyze(&source, &AnalysisParams::default(), &c);
                assert!(
                    r.is_ok(),
                    "{} threads={threads}: {:?}",
                    path.display(),
                    r.err()
                );
            }
        }
    }
}

#[test]
fn a_changed_input_stops_before_counting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.fasta");
    std::fs::write(&path, sample()).unwrap();
    let source = Source::Path(path.clone());
    let checked = inspect(&source).unwrap();
    // The same size, other letters: a protein alignment rewritten as nucleotides.
    let swapped: Vec<u8> = sample()
        .iter()
        .map(|&b| {
            if b.is_ascii_uppercase() && b != b'N' {
                b"ACGT"[usize::from(b % 4)]
            } else {
                b
            }
        })
        .collect();
    std::fs::write(&path, swapped).unwrap();
    let c = control(2, BatchLimits::default(), Some(checked.probe.clone()));
    let progress: Arc<Progress> = Arc::clone(&c.progress);
    match dima_core::analyze(&source, &AnalysisParams::default(), &c) {
        Err(e @ DimaError::InputChanged { .. }) => {
            assert_eq!(e.class().code(), 3);
            assert!(
                e.to_string().contains("changed after it was checked"),
                "{e}"
            );
        }
        other => panic!("expected InputChanged, got {:?}", other.map(|_| ())),
    }
    assert_eq!(
        progress.phase_started(Phase::Counting),
        None,
        "nothing was counted"
    );
    // Without an expectation (the command line), the new content is simply analysed.
    let c = control(2, BatchLimits::default(), None);
    assert!(dima_core::analyze(&source, &AnalysisParams::default(), &c).is_ok());
}
