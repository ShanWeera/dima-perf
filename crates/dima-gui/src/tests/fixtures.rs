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

fn analyse(path: PathBuf, params: &AnalysisParams) -> Arc<Results> {
    let a = dima_core::analyze(&Source::Path(path), params, &RunControl::new())
        .expect("fixture analysis");
    Arc::new(a.results)
}

/// Default parameters with similarity-weighted entropy.
fn with_swe() -> AnalysisParams {
    AnalysisParams {
        similarity_weighted_entropy: true,
        ..AnalysisParams::default()
    }
}

/// The MERS spike protein results with default parameters.
#[must_use]
pub fn mers() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(mers_path(), &AnalysisParams::default())))
}

/// The MERS spike protein results with similarity-weighted entropy (the guide's worked example:
/// similarity-weighted mean 0.0363 bits, observed 0.1207).
#[must_use]
pub fn mers_swe() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(mers_path(), &with_swe())))
}

/// The MERS spike nucleotide results with default parameters (LS positions, a flagged region).
#[must_use]
pub fn mers_nt() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(mers_nt_path(), &AnalysisParams::default())))
}

/// The MERS spike nucleotide results with similarity-weighted entropy (LS positions, some with
/// one distinct k-mer).
#[must_use]
pub fn mers_nt_swe() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(mers_nt_path(), &with_swe())))
}

/// `tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2` (the paper's HIV-1 pol protein data set).
#[must_use]
pub fn hiv_pro_path() -> PathBuf {
    repo().join("tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2")
}

/// The HIV-1 pol protein results with similarity-weighted entropy: 162 LS/ELS positions, 41
/// of them with normalised values (16 where every sequence carries a different k-mer) and 121
/// with one distinct k-mer, and 551 NS positions.
#[must_use]
pub fn hiv_pro_swe() -> Arc<Results> {
    static R: OnceLock<Arc<Results>> = OnceLock::new();
    Arc::clone(R.get_or_init(|| analyse(hiv_pro_path(), &with_swe())))
}

/// A complete results value (from a tiny alignment) whose positions tests replace.
#[must_use]
pub fn results(_n: usize) -> Results {
    (*mers()).clone()
}
