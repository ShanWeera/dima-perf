//! Benchmarks for k-mer encoding — the I/O-phase hot path.
//!
//! Measures the single-pass sliding window encoder which validates and
//! encodes k-mers in one iteration (vs. the old two-pass approach).

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use dima_lib::{CharacterValidator, AlphabetType, ValidationMode};
use dima_lib::kmer::sliding_window_validated;

fn make_protein_sequence(len: usize) -> Vec<u8> {
    let alphabet = b"ACDEFGHIKLMNPQRSTVWY";
    (0..len).map(|i| alphabet[i % alphabet.len()]).collect()
}

fn make_nucleotide_sequence(len: usize) -> Vec<u8> {
    let alphabet = b"ACGT";
    (0..len).map(|i| alphabet[i % alphabet.len()]).collect()
}

fn make_protein_with_gaps(len: usize, gap_fraction: f64) -> Vec<u8> {
    let alphabet = b"ACDEFGHIKLMNPQRSTVWY";
    let gap_interval = (1.0 / gap_fraction) as usize;
    (0..len).map(|i| {
        if gap_interval > 0 && i % gap_interval == 0 { b'-' }
        else { alphabet[i % alphabet.len()] }
    }).collect()
}

fn bench_sliding_window_protein(c: &mut Criterion) {
    let mut group = c.benchmark_group("kmer/protein");
    let validator = CharacterValidator::with_options(
        AlphabetType::Protein, ValidationMode::default(), true,
    );

    for &seq_len in &[100, 500, 1500, 5000] {
        let seq = make_protein_sequence(seq_len);
        group.bench_with_input(
            BenchmarkId::from_parameter(seq_len),
            &seq_len,
            |b, _| {
                b.iter(|| sliding_window_validated(&seq, 9, &validator));
            },
        );
    }
    group.finish();
}

fn bench_sliding_window_nucleotide(c: &mut Criterion) {
    let mut group = c.benchmark_group("kmer/nucleotide");
    let validator = CharacterValidator::with_options(
        AlphabetType::Nucleotide, ValidationMode::default(), true,
    );

    for &seq_len in &[100, 500, 1500, 5000] {
        let seq = make_nucleotide_sequence(seq_len);
        group.bench_with_input(
            BenchmarkId::from_parameter(seq_len),
            &seq_len,
            |b, _| {
                b.iter(|| sliding_window_validated(&seq, 9, &validator));
            },
        );
    }
    group.finish();
}

fn bench_sliding_window_with_gaps(c: &mut Criterion) {
    let mut group = c.benchmark_group("kmer/gaps");
    let validator = CharacterValidator::with_options(
        AlphabetType::Protein, ValidationMode::default(), true,
    );
    let seq_len = 1500;

    for &gap_frac in &[0.0_f64, 0.05, 0.1, 0.2] {
        let seq = make_protein_with_gaps(seq_len, gap_frac.max(0.001));
        let label = format!("{:.0}%", gap_frac * 100.0);
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &gap_frac,
            |b, _| {
                b.iter(|| sliding_window_validated(&seq, 9, &validator));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_sliding_window_protein,
    bench_sliding_window_nucleotide,
    bench_sliding_window_with_gaps,
);
criterion_main!(benches);
