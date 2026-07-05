//! Benchmarks for entropy computation — the core hot path of DiMA.
//!
//! These benchmarks cover the rarefaction-based entropy correction pipeline
//! with varying dataset sizes and diversity levels. Use `cargo bench` to run,
//! or `cargo bench -- --save-baseline <name>` to save a baseline for regression detection.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use dima_lib::calculate_entropy_encoded_at_position;

/// Simulate k-mer columns with controllable diversity.
fn make_kmer_column(num_sequences: usize, num_distinct: usize) -> Vec<u64> {
    (0..num_sequences).map(|i| (i % num_distinct) as u64).collect()
}

/// Simulate a column with some invalid (gap) entries.
fn make_kmer_column_with_gaps(num_sequences: usize, num_distinct: usize, gap_fraction: f64) -> Vec<u64> {
    let gap_count = (num_sequences as f64 * gap_fraction) as usize;
    let mut col: Vec<u64> = (0..num_sequences)
        .map(|i| {
            if i < gap_count { u64::MAX } else { (i % num_distinct) as u64 }
        })
        .collect();
    col.sort(); // mix gaps with valid entries
    col
}

fn bench_entropy_varying_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("entropy/size");
    
    for &size in &[100, 1000, 5000, 10000] {
        let kmers = make_kmer_column(size, 50);
        let threshold = size / 2;
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, _| {
                b.iter(|| calculate_entropy_encoded_at_position(&kmers, &threshold, 0));
            },
        );
    }
    group.finish();
}

fn bench_entropy_varying_diversity(c: &mut Criterion) {
    let mut group = c.benchmark_group("entropy/diversity");
    let size = 5000;
    
    for &distinct in &[2, 10, 50, 200, 1000] {
        let kmers = make_kmer_column(size, distinct);
        let threshold = size / 2;
        group.bench_with_input(
            BenchmarkId::from_parameter(distinct),
            &distinct,
            |b, _| {
                b.iter(|| calculate_entropy_encoded_at_position(&kmers, &threshold, 42));
            },
        );
    }
    group.finish();
}

fn bench_entropy_with_gaps(c: &mut Criterion) {
    let mut group = c.benchmark_group("entropy/gaps");
    let size = 5000;
    
    for &gap_frac in &[0.0, 0.1, 0.3, 0.5] {
        let kmers = make_kmer_column_with_gaps(size, 50, gap_frac);
        let threshold = size / 2;
        let label = format!("{:.0}%", gap_frac * 100.0);
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &gap_frac,
            |b, _| {
                b.iter(|| calculate_entropy_encoded_at_position(&kmers, &threshold, 7));
            },
        );
    }
    group.finish();
}

fn bench_entropy_below_threshold(c: &mut Criterion) {
    // When support <= threshold, raw Shannon is returned directly (no rarefaction)
    let kmers = make_kmer_column(100, 20);
    let threshold = 200; // threshold > support → direct Shannon
    c.bench_function("entropy/below_threshold_100seq", |b| {
        b.iter(|| calculate_entropy_encoded_at_position(&kmers, &threshold, 0));
    });
}

criterion_group!(
    benches,
    bench_entropy_varying_size,
    bench_entropy_varying_diversity,
    bench_entropy_with_gaps,
    bench_entropy_below_threshold,
);
criterion_main!(benches);
