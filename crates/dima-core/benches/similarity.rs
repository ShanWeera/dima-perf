//! Similarity-weighted entropy (Wu 2025) of one position: the per-site residue counts that
//! replace Wu's pairwise sums, for few and many distinct k-mers at short and long k.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use dima_core::similarity::similarity_weighted_entropy;

/// `distinct` different k-mers of length `k` over the 20 amino-acid letters, from a fixed linear
/// congruential generator (deterministic, no RNG dependency), with counts 1, 2, 3, ….
fn position(distinct: usize, k: usize) -> Vec<(Vec<u8>, u64)> {
    const RESIDUES: &[u8] = b"ACDEFGHIKLMNPQRSTVWY";
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < distinct {
        let kmer: Vec<u8> = (0..k)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                RESIDUES[((state >> 33) % RESIDUES.len() as u64) as usize]
            })
            .collect();
        seen.insert(kmer);
    }
    seen.into_iter().zip(1u64..).collect()
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("similarity_weighted_entropy");
    group.sample_size(10);
    for (distinct, k) in [
        (10, 9),
        (1_000, 9),
        (100_000, 9),
        (1_000, 27),
        (100_000, 27),
        (10, 1_000),
        (10_000, 1_000),
    ] {
        let p = position(distinct, k);
        group.throughput(Throughput::Elements((distinct * k) as u64));
        group.bench_function(format!("d{distinct}_k{k}"), |b| {
            b.iter(|| {
                similarity_weighted_entropy(p.iter().map(|(s, n)| (s.as_slice(), *n))).unwrap()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
