//! Entropy estimators (§6): the exact expectation of the paper's rarefaction procedure (D51),
//! Chao–Wang–Jost and Miller–Madow, on count profiles from a conserved position to a highly
//! diverse one, at the support sizes of the paper's datasets and of very large alignments.

use criterion::{Criterion, criterion_group, criterion_main};
use dima_core::config::Estimator;
use dima_core::entropy::{CountProfile, estimate};

/// One dominant k-mer and `rare` singletons plus a tail of doubletons, summing to `support`.
fn profile(support: u64, rare: u64) -> CountProfile {
    let doubletons = rare / 4;
    let dominant = support - rare - 2 * doubletons;
    let counts = std::iter::once(dominant)
        .chain(std::iter::repeat_n(1, rare as usize))
        .chain(std::iter::repeat_n(2, doubletons as usize));
    CountProfile::from_counts(counts).unwrap()
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("estimators");
    for (label, support, rare) in [
        ("conserved_n2637", 2_637, 3),
        ("diverse_n2637", 2_637, 900),
        ("diverse_n2_6M", 2_637_000, 400_000),
    ] {
        let p = profile(support, rare);
        for (name, estimator) in [
            ("dima", Estimator::Dima),
            ("chao_wang_jost", Estimator::ChaoWangJost),
            ("miller_madow", Estimator::MillerMadow),
        ] {
            group.bench_function(format!("{name}/{label}"), |b| {
                b.iter(|| estimate(&p, estimator, 100).unwrap());
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
