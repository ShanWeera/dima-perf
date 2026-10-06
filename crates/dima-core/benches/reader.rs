//! Input throughput: opening, decompressing, splitting records and normalising them (§3 steps
//! 1–6) on the paper's MERS-CoV spike nucleotide alignment (bzip2) and protein alignment (plain).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use dima_core::control::Progress;
use dima_core::input::{self, RawBatch, Source};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Reads every record; returns the number of records.
fn read_all(path: &Path) -> u64 {
    let progress = Arc::new(Progress::new());
    let opened = input::open(&Source::Path(path.to_path_buf()), &progress).unwrap();
    let mut reader = opened.batches;
    let cancel = AtomicBool::new(false);
    let mut batch = RawBatch::default();
    let mut records = 0u64;
    while reader.read_batch(&mut batch, &cancel).unwrap() {
        records += batch.len() as u64;
    }
    records
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("reader");
    for (name, path) in [
        (
            "mers_spike_aa.fasta",
            root().join("samples/mers_spike_aa.fasta"),
        ),
        (
            "MERS_Spike_nt.fasta.bz2",
            root().join("tests/data/MERS_Spike_nt.fasta.bz2"),
        ),
    ] {
        // Records per second: stored bytes would flatter compressed inputs.
        group.throughput(Throughput::Elements(read_all(&path)));
        group.bench_function(name, |b| b.iter(|| read_all(&path)));
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
