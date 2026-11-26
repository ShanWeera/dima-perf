//! Differential tests against the independent Python reference implementation
//! (`scripts/reference/dima_reference.py`; Kanewala & Bieman 2014, pseudo-oracles): every
//! number agrees within a relative 1e-13 of the reference value (`common::assert_agrees`, so a
//! zero must be exactly zero), everything else exactly. The test alignments are small, where the two agree to about
//! 3e-15 at worst; at millions of sequences per position the Rust code's own rounding reaches
//! 6e-11 relative (1e-9 for similarity-weighted entropy), so a test at that scale would need a
//! looser rule (docs, "Numerical accuracy").
//!
//! Requires `python3` (≥ 3.10). Set `DIMA_SKIP_REFERENCE=1` to skip where Python is unavailable;
//! CI always runs these tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams, Estimator, Percent, ValidationMode};
use dima_core::control::RunControl;
use dima_core::hcs::{self, HcsParams};
use dima_core::input::Source;
use dima_core::model::{Extremum, Results};
use serde_json::Value;

mod common;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn skip() -> bool {
    std::env::var_os("DIMA_SKIP_REFERENCE").is_some_and(|v| v == "1")
}

/// Deterministic pseudo-random numbers (SplitMix64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

struct Case {
    name: String,
    fasta: Vec<u8>,
    alphabet: Alphabet,
    k: usize,
    threshold: u64,
    header: bool,
    hcs: &'static str,
}

/// A synthetic alignment: a random ancestor, mutated per sequence with a few hot spots, gaps,
/// ambiguity codes and lower case, wrapped at random widths.
fn synthetic(seed: u64, alphabet: Alphabet, n: usize, len: usize) -> Vec<u8> {
    let mut rng = Rng(seed);
    let residues: &[u8] = match alphabet {
        Alphabet::Protein => b"ACDEFGHIKLMNPQRSTVWY",
        Alphabet::Dna => b"ACGT",
        Alphabet::Rna => b"ACGU",
    };
    let ambiguity: &[u8] = match alphabet {
        Alphabet::Protein => b"XBZ*#",
        _ => b"NRY-",
    };
    let ancestor: Vec<u8> = (0..len)
        .map(|_| residues[rng.below(residues.len() as u64) as usize])
        .collect();
    let hot: Vec<usize> = (0..len / 6)
        .map(|_| rng.below(len as u64) as usize)
        .collect();
    let hosts = ["Human", "human", "Camel", "Bat", "NA"];
    let mut out = Vec::new();
    for i in 0..n {
        let mut s = ancestor.clone();
        for &h in &hot {
            if rng.below(3) == 0 {
                s[h] = residues[rng.below(residues.len() as u64) as usize];
            }
        }
        // Gappy tail in a third of the sequences: low support there when T is above two
        // thirds of the sequences.
        if i % 3 == 0 {
            let cut = len - len / 5;
            for c in &mut s[cut..] {
                *c = b'-';
            }
        }
        for _ in 0..rng.below(3) {
            let p = rng.below(len as u64) as usize;
            s[p] = ambiguity[rng.below(ambiguity.len() as u64) as usize];
        }
        if i % 7 == 0 {
            s.make_ascii_lowercase();
        }
        let host = hosts[rng.below(hosts.len() as u64) as usize];
        let year = 2012 + rng.below(4);
        let country = if rng.below(5) == 0 {
            ""
        } else {
            "Saudi Arabia"
        };
        out.extend_from_slice(format!(">seq{i}|{host}|{country}|{year}\n").as_bytes());
        let width = 10 + rng.below(70) as usize;
        for chunk in s.chunks(width) {
            out.extend_from_slice(chunk);
            out.push(b'\n');
        }
    }
    out
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for (i, (alphabet, n, len, k, t)) in [
        (Alphabet::Protein, 150, 60, 9, 30),
        (Alphabet::Protein, 40, 25, 3, 10),
        (Alphabet::Dna, 120, 90, 27, 50),
        (Alphabet::Dna, 300, 40, 5, 100),
        (Alphabet::Rna, 60, 30, 1, 20),
        (Alphabet::Protein, 12, 15, 4, 2),
        // T between the gappy tail's support (about 40) and the rest's (about 60).
        (Alphabet::Protein, 60, 30, 3, 45),
    ]
    .into_iter()
    .enumerate()
    {
        v.push(Case {
            name: format!("synthetic-{i}"),
            fasta: synthetic(1000 + i as u64, alphabet, n, len),
            alphabet,
            k,
            threshold: t,
            header: i % 2 == 0,
            hcs: ["100", "70", "90.5"][i % 3],
        });
    }
    // Positions without support, with low support and with an Index tie (the golden `tiny`).
    v.push(Case {
        name: "tiny".into(),
        fasta: std::fs::read(root().join("tests/golden/tiny/input.fasta")).unwrap(),
        alphabet: Alphabet::Protein,
        k: 3,
        threshold: 4,
        header: false,
        hcs: "70",
    });
    v.push(Case {
        name: "mers".into(),
        fasta: std::fs::read(root().join("samples/mers_spike_aa.fasta")).unwrap(),
        alphabet: Alphabet::Protein,
        k: 9,
        threshold: 100,
        header: true,
        hcs: "100",
    });
    v
}

const FORMAT: &str = "Accession|Host|Geolocation|Year";

fn reference(case: &Case, path: &Path) -> Value {
    let mut cmd = Command::new("python3");
    cmd.arg(root().join("scripts/reference/dima_reference.py"))
        .arg("analyze")
        .arg("-i")
        .arg(path)
        .args(["-k", &case.k.to_string(), "-s", &case.threshold.to_string()])
        .args(["-a", case.alphabet.as_str(), "--validation", "permissive"])
        .args(["--hcs-min-incidence", case.hcs])
        .arg("--similarity-weighted-entropy");
    if case.header {
        cmd.args(["--header-format", FORMAT]);
    }
    let out = cmd
        .output()
        .expect("python3 must be installed (or set DIMA_SKIP_REFERENCE=1)");
    assert!(
        out.status.success(),
        "{}: reference failed: {}",
        case.name,
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn ours(case: &Case, path: &Path, estimator: Estimator, similarity: bool) -> Results {
    let params = AnalysisParams {
        alphabet: Some(case.alphabet),
        kmer_length: Some(case.k),
        support_threshold: Some(case.threshold),
        estimator: Some(estimator),
        similarity_weighted_entropy: similarity,
        validation: Some(ValidationMode::Permissive),
        header_format: case
            .header
            .then(|| FORMAT.split('|').map(str::to_owned).collect()),
        ..AnalysisParams::default()
    };
    analyze(
        &Source::Path(path.to_path_buf()),
        &params,
        &RunControl::new(),
    )
    .unwrap()
    .results
}

#[track_caller]
fn num(what: &str, got: Option<f64>, want: &Value) {
    match (got, want.as_f64()) {
        (None, None) => assert!(want.is_null(), "{what}: expected {want}"),
        (Some(g), Some(w)) => common::assert_agrees(what, g, w),
        _ => panic!("{what}: {got:?} vs reference {want}"),
    }
}

#[track_caller]
fn ext(what: &str, got: Option<Extremum>, want: &Value) {
    match got {
        None => assert!(want.is_null(), "{what}"),
        Some(e) => {
            num(what, Some(e.entropy), &want["entropy"]);
            assert_eq!(Some(e.position), want["position"].as_u64(), "{what}");
            assert_eq!(
                Some(e.tied_position_count),
                want["tied_position_count"].as_u64(),
                "{what}"
            );
        }
    }
}

#[test]
fn agrees_with_the_python_reference() {
    if skip() {
        eprintln!("DIMA_SKIP_REFERENCE=1: skipping the differential tests");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for case in cases() {
        let path = dir.path().join(format!("{}.fasta", case.name));
        std::fs::write(&path, &case.fasta).unwrap();
        let r = reference(&case, &path);
        // Similarity-weighted entropy with two estimators (it must not depend on them), not with
        // the third (it must then be absent).
        let d = ours(&case, &path, Estimator::Dima, true);
        let mm = ours(&case, &path, Estimator::MillerMadow, false);
        let cwj = ours(&case, &path, Estimator::ChaoWangJost, true);
        let n = &case.name;
        let s = d.similarity_weighted_entropy_summary.as_ref().unwrap();
        let rs = &r["similarity_weighted_entropy_summary"];
        assert_eq!(Some(s.position_count), rs["position_count"].as_u64(), "{n}");
        num(
            &format!("{n} weighted mean"),
            s.average_entropy,
            &rs["average_entropy"],
        );
        ext(
            &format!("{n} weighted lowest"),
            s.lowest_entropy,
            &rs["lowest_entropy"],
        );
        ext(
            &format!("{n} weighted highest"),
            s.highest_entropy,
            &rs["highest_entropy"],
        );
        num(
            &format!("{n} weighted observed mean"),
            s.average_observed_entropy,
            &rs["average_observed_entropy"],
        );
        assert_eq!(
            cwj.similarity_weighted_entropy_summary.as_ref(),
            Some(s),
            "{n}"
        );
        assert!(mm.similarity_weighted_entropy_summary.is_none(), "{n}");
        assert_eq!(Some(d.sequence_count), r["sequence_count"].as_u64(), "{n}");
        assert_eq!(
            Some(d.run.alignment_length),
            r["alignment_length"].as_u64(),
            "{n}"
        );
        assert_eq!(
            Some(d.low_support_count),
            r["low_support_count"].as_u64(),
            "{n}"
        );
        assert_eq!(
            Some(d.eligible_position_count),
            r["eligible_position_count"].as_u64(),
            "{n}"
        );
        assert_eq!(
            serde_json::to_value(d.support_label_counts).unwrap(),
            r["support_label_counts"],
            "{n}"
        );
        num(
            &format!("{n} mean"),
            d.average_entropy,
            &r["average_entropy"],
        );
        ext(
            &format!("{n} lowest"),
            d.lowest_entropy,
            &r["lowest_entropy"],
        );
        ext(
            &format!("{n} highest"),
            d.highest_entropy,
            &r["highest_entropy"],
        );
        let a = &r["all_positions_summary"];
        num(
            &format!("{n} all mean"),
            d.all_positions_summary.average_entropy,
            &a["average_entropy"],
        );
        ext(
            &format!("{n} all lowest"),
            d.all_positions_summary.lowest_entropy,
            &a["lowest_entropy"],
        );
        ext(
            &format!("{n} all highest"),
            d.all_positions_summary.highest_entropy,
            &a["highest_entropy"],
        );
        let positions = r["positions"].as_array().unwrap();
        assert_eq!(positions.len(), d.results.len(), "{n}");
        for (i, rp) in positions.iter().enumerate() {
            let (p, m, c) = (&d.results[i], &mm.results[i], &cwj.results[i]);
            let w = format!("{n} position {}", p.position);
            let similarity = |q: &dima_core::model::PositionResult| {
                [
                    q.similarity_weighted_entropy,
                    q.normalised_observed_entropy,
                    q.normalised_similarity_weighted_entropy,
                ]
            };
            for (key, got) in [
                "similarity_weighted_entropy",
                "normalised_observed_entropy",
                "normalised_similarity_weighted_entropy",
            ]
            .into_iter()
            .zip(similarity(p))
            {
                num(&format!("{w} {key}"), got, &rp[key]);
            }
            assert_eq!(
                similarity(c).map(|x| x.map(f64::to_bits)),
                similarity(p).map(|x| x.map(f64::to_bits)),
                "{w}: the estimator changed similarity-weighted entropy"
            );
            assert_eq!(similarity(m), [None; 3], "{w}");
            assert_eq!(Some(p.position), rp["position"].as_u64(), "{w}");
            assert_eq!(
                serde_json::to_value(p.low_support).unwrap(),
                rp["low_support"],
                "{w}"
            );
            assert_eq!(Some(p.support), rp["support"].as_u64(), "{w}");
            assert_eq!(
                Some(p.distinct_variants_count),
                rp["distinct_variants_count"].as_u64(),
                "{w}"
            );
            num(
                &format!("{w} dvi"),
                p.distinct_variants_incidence,
                &rp["distinct_variants_incidence"],
            );
            num(
                &format!("{w} tvi"),
                p.total_variants_incidence,
                &rp["total_variants_incidence"],
            );
            num(&format!("{w} entropy"), p.entropy, &rp["entropy"]);
            num(
                &format!("{w} observed"),
                p.observed_entropy,
                &rp["observed_entropy"],
            );
            assert_eq!(
                serde_json::to_value(p.entropy_method).unwrap(),
                rp["entropy_method"],
                "{w}"
            );
            assert_eq!(
                serde_json::to_value(p.entropy_fallback).unwrap(),
                rp["entropy_fallback"],
                "{w}"
            );
            num(&format!("{w} miller-madow"), m.entropy, &rp["miller_madow"]);
            num(
                &format!("{w} chao-wang-jost"),
                c.entropy,
                &rp["chao_wang_jost"],
            );
            num(
                &format!("{w} coverage"),
                p.sample_coverage,
                &rp["sample_coverage"],
            );
            let rv = rp["diversity_motifs"].as_array().unwrap();
            assert_eq!(rv.len(), p.diversity_motifs.len(), "{w}");
            for (v, x) in p.diversity_motifs.iter().zip(rv) {
                assert_eq!(Some(v.sequence.as_str()), x["sequence"].as_str(), "{w}");
                assert_eq!(Some(v.count), x["count"].as_u64(), "{w}");
                num(
                    &format!("{w} incidence"),
                    Some(v.incidence),
                    &x["incidence"],
                );
                assert_eq!(Some(v.motif.short()), x["motif_short"].as_str(), "{w}");
                assert_eq!(
                    serde_json::to_value(&v.metadata).unwrap(),
                    x["metadata"],
                    "{w} metadata"
                );
            }
        }
        eprintln!(
            "{n}: {} positions, {} variants, {} HCS regions compared",
            positions.len(),
            d.results
                .iter()
                .map(|p| p.diversity_motifs.len())
                .sum::<usize>(),
            r["hcs"]["region_count"]
        );
        let threshold: Percent = case.hcs.parse().unwrap();
        let h = hcs::compute(
            &d,
            &HcsParams {
                threshold,
                eligible_only: false,
            },
        )
        .unwrap();
        let rh = &r["hcs"];
        assert_eq!(Some(h.region_count), rh["region_count"].as_u64(), "{n} hcs");
        for (g, x) in h.regions.iter().zip(rh["regions"].as_array().unwrap()) {
            let w = format!("{n} hcs region {}", g.id);
            assert_eq!(Some(g.start_column), x["start_column"].as_u64(), "{w}");
            assert_eq!(Some(g.end_column), x["end_column"].as_u64(), "{w}");
            assert_eq!(Some(g.sequence.as_str()), x["sequence"].as_str(), "{w}");
            assert_eq!(Some(g.min_support), x["min_support"].as_u64(), "{w}");
            assert_eq!(Some(g.flagged), x["flagged"].as_bool(), "{w}");
            assert_eq!(
                Some(g.split_from_previous),
                x["split_from_previous"].as_bool(),
                "{w}"
            );
            assert_eq!(
                serde_json::to_value(&g.low_support_positions).unwrap(),
                x["low_support_positions"],
                "{w}"
            );
            num(
                &format!("{w} min incidence"),
                Some(g.min_incidence),
                &x["min_incidence"],
            );
            num(
                &format!("{w} bound"),
                Some(g.conservation_lower_bound_95),
                &x["conservation_lower_bound_95"],
            );
            let members: Vec<u64> = g.members.iter().map(|m| m.position).collect();
            let rmembers: Vec<u64> = x["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["position"].as_u64().unwrap())
                .collect();
            assert_eq!(members, rmembers, "{w}");
        }
    }
}
