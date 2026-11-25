//! Property-based tests (Claessen & Hughes 2000): the streaming counter equals a naive counter
//! for every k, thread count and batch split, and every analysis satisfies the definitional
//! invariants of support, motifs and incidences.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams, Estimator, ValidationMode};
use dima_core::control::RunControl;
use dima_core::engine::Engine;
use dima_core::input::Source;
use dima_core::model::{Motif, SupportLabel};
use proptest::prelude::*;

/// Rows over a 4-letter alphabet plus excluded columns (0).
fn rows() -> impl Strategy<Value = (usize, Vec<Vec<u8>>)> {
    (1usize..24, 1usize..30).prop_flat_map(|(width, n)| {
        let row = proptest::collection::vec(
            prop_oneof![8 => Just(b'A'), 4 => Just(b'C'), 2 => Just(b'G'), 1 => Just(0u8)],
            width,
        );
        (Just(width), proptest::collection::vec(row, n))
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn engine_equals_naive_counter((width, rows) in rows(), k_seed in 0usize..1000, threads in 1usize..5, split in 1usize..8) {
        let k = 1 + k_seed % width;
        let mut want: Vec<BTreeMap<Vec<u8>, u64>> = vec![BTreeMap::new(); width - k + 1];
        for row in &rows {
            for (p, w) in row.windows(k).enumerate() {
                if !w.contains(&0) {
                    *want[p].entry(w.to_vec()).or_default() += 1;
                }
            }
        }
        let mut engine = Engine::new(width, k, 0, threads).unwrap();
        let cancel = AtomicBool::new(false);
        for chunk in rows.chunks(split) {
            engine.add_batch(&chunk.concat(), chunk.len(), &[], &cancel).unwrap();
        }
        let got = engine.finish();
        prop_assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) {
            let map: BTreeMap<Vec<u8>, u64> = g.ordered_ids().into_iter().map(|id| (g.kmer(id).to_vec(), g.count(id))).collect();
            prop_assert_eq!(&map, w);
            // Output order: count descending, then bytes ascending.
            let ids = g.ordered_ids();
            for pair in ids.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                prop_assert!(g.count(a) > g.count(b) || (g.count(a) == g.count(b) && g.kmer(a) < g.kmer(b)));
            }
        }
    }

    #[test]
    fn analyses_satisfy_definitional_invariants(
        (width, rows) in rows(),
        k_seed in 0usize..1000,
        threshold in 1u64..12,
        estimator in prop_oneof![Just(Estimator::Dima), Just(Estimator::ChaoWangJost), Just(Estimator::MillerMadow), Just(Estimator::Raw)],
        similarity_weighted_entropy in any::<bool>(),
    ) {
        let k = 1 + k_seed % width;
        let mut fasta = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            fasta.extend_from_slice(format!(">s{i}\n").as_bytes());
            fasta.extend(row.iter().map(|&b| if b == 0 { b'-' } else { b }));
            fasta.push(b'\n');
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.fa");
        std::fs::write(&path, &fasta).unwrap();
        let params = AnalysisParams {
            alphabet: Some(Alphabet::Dna),
            kmer_length: Some(k),
            support_threshold: Some(threshold),
            estimator: Some(estimator),
            validation: Some(ValidationMode::Strict),
            similarity_weighted_entropy,
            ..AnalysisParams::default()
        };
        let r = analyze(&Source::Path(path), &params, &RunControl::new()).unwrap().results;
        prop_assert_eq!(r.similarity_weighted_entropy_summary.is_some(), similarity_weighted_entropy);
        prop_assert_eq!(r.sequence_count, rows.len() as u64);
        prop_assert_eq!(r.position_count, (width - k + 1) as u64);
        let mut eligible = 0;
        for p in &r.results {
            let counts: Vec<u64> = p.diversity_motifs.iter().map(|v| v.count).collect();
            prop_assert_eq!(counts.iter().sum::<u64>(), p.support);
            prop_assert!(p.support <= r.sequence_count);
            let label = match p.support {
                0 => Some(SupportLabel::NoSupport),
                s if s < threshold => Some(SupportLabel::Low),
                s if s == threshold => Some(SupportLabel::EqualToThreshold),
                _ => None,
            };
            prop_assert_eq!(p.low_support, label);
            eligible += u64::from(label.is_none());
            // The similarity-weighted values are given exactly where they are defined.
            let d = p.diversity_motifs.len();
            prop_assert_eq!(p.similarity_weighted_entropy.is_some(), similarity_weighted_entropy && p.support >= 1);
            prop_assert_eq!(p.normalised_observed_entropy.is_some(), similarity_weighted_entropy && d >= 2);
            prop_assert_eq!(p.normalised_similarity_weighted_entropy.is_some(), similarity_weighted_entropy && d >= 2);
            if p.support == 0 {
                prop_assert!(p.entropy.is_none() && p.diversity_motifs.is_empty());
                prop_assert!(p.total_variants_incidence.is_none());
                continue;
            }
            if let (Some(hs), Some(h)) = (p.similarity_weighted_entropy, p.observed_entropy) {
                // Bounds: each weight lies in [f_i + (1 − f_i)/k, 1].
                let lower: f64 = p.diversity_motifs.iter().map(|v| {
                    let f = v.count as f64 / p.support as f64;
                    -f * f.log2() * (f + (1.0 - f) / k as f64)
                }).sum();
                prop_assert!(hs <= h * (1.0 + 1e-12) && hs >= lower * (1.0 - 1e-12), "{} {} {}", hs, h, lower);
            }
            for x in [p.normalised_observed_entropy, p.normalised_similarity_weighted_entropy].into_iter().flatten() {
                prop_assert!((0.0..=1.0).contains(&x));
            }
            let incidence: f64 = p.diversity_motifs.iter().map(|v| v.incidence).sum();
            prop_assert!((incidence - 100.0).abs() < 1e-9);
            let e = p.entropy.unwrap();
            let h = p.observed_entropy.unwrap();
            prop_assert!(e >= 0.0 && h >= 0.0 && e.is_finite());
            prop_assert!(h <= (p.diversity_motifs.len() as f64).log2() + 1e-12);
            let non_index = p.diversity_motifs.iter().filter(|v| v.motif != Motif::Index).count() as u64;
            prop_assert_eq!(p.distinct_variants_count, non_index);
            for v in &p.diversity_motifs {
                prop_assert_eq!(v.motif == Motif::Unique, v.count == 1);
            }
            let cov = p.sample_coverage;
            prop_assert_eq!(cov.is_some(), p.support >= 2);
            if let Some(c) = cov {
                prop_assert!((0.0..=1.0).contains(&c));
            }
        }
        prop_assert_eq!(r.eligible_position_count, eligible);
    }
}
