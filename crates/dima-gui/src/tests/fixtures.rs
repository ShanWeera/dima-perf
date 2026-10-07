//! Results used by many tests, computed once per test binary by the library itself.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use dima_core::config::AnalysisParams;
use dima_core::control::RunControl;
use dima_core::input::Source;
use dima_core::model::Results;

/// The repository root.
#[must_use]
pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `samples/mers_spike_aa.fasta` (the paper's Fig. 1C data set).
#[must_use]
pub fn mers_path() -> PathBuf {
    repo().join("samples/mers_spike_aa.fasta")
}

/// `tests/data/MERS_Spike_nt.fasta.bz2`.
#[must_use]
pub fn mers_nt_path() -> PathBuf {
    repo().join("tests/data/MERS_Spike_nt.fasta.bz2")
}

fn analyse(path: PathBuf) -> Arc<Results> {
    let a = dima_core::analyze(
        &Source::Path(path),
        &AnalysisParams::default(),
        &RunControl::new(),
    )
    .expect("fixture analysis");
    Arc::new(a.results)
}

/// The MERS spike protein results with default parameters.
#[must_use]
pub fn mers() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(mers_path())))
}

/// The MERS spike nucleotide results with default parameters (LS positions, a flagged region).
#[must_use]
pub fn mers_nt() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(mers_nt_path())))
}

/// A complete results value (from a tiny alignment) whose positions tests replace.
#[must_use]
pub fn results(_n: usize) -> Results {
    (*mers()).clone()
}
