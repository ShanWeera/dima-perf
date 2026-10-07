//! k-mer counting (§3 steps 7–8): the streaming engine on synthetic aligned rows of low and high
//! diversity, short and long k (the transition cache applies for k > 32), and the full
//! analysis of the paper's MERS-CoV spike protein alignment end to end.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use dima_core::config::AnalysisParams;
use dima_core::control::RunControl;
use dima_core::engine::Engine;
use dima_core::input::Source;

/// `n` rows of `width` canonical residues; `diversity` substitutions per thousand residues,
/// from a fixed linear congruential generator (deterministic, no RNG dependency).
fn rows(n: usize, width: usize, diversity: u64) -> Vec<u8> {
    const RESIDUES: &[u8] = b"ACDEFGHIKLMNPQRSTVWY";
    let consensus: Vec<u8> = (0..width)
        .map(|i| RESIDUES[(i * 7) % RESIDUES.len()])
        .collect();
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    let mut out = Vec::with_capacity(n * width);
    for _ in 0..n {
        for &c in &consensus {
            if next() % 1000 < diversity {
                out.push(RESIDUES[(next() % RESIDUES.len() as u64) as usize]);
            } else {
                out.push(c);
            }
        }
    }
    out
}

fn count(data: &[u8], n: usize, width: usize, k: usize, threads: usize) -> usize {
    let mut engine = Engine::new(width, k, 0, threads).unwrap();
    engine
        .add_batch(data, n, &[], &AtomicBool::new(false))
        .unwrap();
    engine.finish().len()
}

fn engine(c: &mut Criterion) {
    let (n, width) = (2_000, 1_200);
    let threads = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let mut group = c.benchmark_group("engine");
    group.throughput(Throughput::Bytes((n * width) as u64));
    group.sample_size(20);
    for (label, diversity) in [("low_diversity", 5), ("high_diversity", 200)] {
        let data = rows(n, width, diversity);
        for k in [9, 27, 45] {
            group.bench_function(format!("{label}/k{k}"), |b| {
                b.iter(|| count(&data, n, width, k, threads));
            });
        }
    }
    group.finish();
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn analysis(c: &mut Criterion) {
    let source = Source::Path(root().join("samples/mers_spike_aa.fasta"));
    let params = AnalysisParams::default();
    let mut group = c.benchmark_group("analysis");
    group.sample_size(20);
    group.bench_function("mers_spike_aa", |b| {
        b.iter_batched(
            RunControl::new,
            |control| dima_core::analyze(&source, &params, &control).unwrap(),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(benches, engine, analysis);
criterion_main!(benches);
