//! Conserved-region rules on crafted alignments (D12, D44, D86).

use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams, Percent};
use dima_core::control::RunControl;
use dima_core::hcs::{self, HcsParams};
use dima_core::input::Source;
use dima_core::model::{HcsResults, Results};

fn results(seqs: &[&str], k: usize, t: u64) -> Results {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.fa");
    let fasta: String = seqs
        .iter()
        .enumerate()
        .map(|(i, s)| format!(">s{i}\n{s}\n"))
        .collect();
    std::fs::write(&path, fasta).unwrap();
    let params = AnalysisParams {
        alphabet: Some(Alphabet::Protein),
        kmer_length: Some(k),
        support_threshold: Some(t),
        ..AnalysisParams::default()
    };
    analyze(&Source::Path(path), &params, &RunControl::new())
        .unwrap()
        .results
}

fn hcs(r: &Results, threshold: &str, eligible_only: bool) -> HcsResults {
    let threshold: Percent = threshold.parse().unwrap();
    hcs::compute(
        r,
        &HcsParams {
            threshold,
            eligible_only,
        },
    )
    .unwrap()
}

fn spans(h: &HcsResults) -> Vec<(u64, u64, bool)> {
    h.regions
        .iter()
        .map(|g| (g.start_column, g.end_column, g.split_from_previous))
        .collect()
}

#[test]
fn overlapping_kmers_join_into_one_region() {
    let r = results(&["ACDEFGHIKLMN"; 4], 3, 2);
    let h = hcs(&r, "100", false);
    assert_eq!(spans(&h), [(1, 12, false)]);
    let g = &h.regions[0];
    assert_eq!(g.sequence, "ACDEFGHIKLMN");
    assert_eq!(g.length, 12);
    assert_eq!(g.members.len(), 10);
    assert!(!g.flagged);
    assert!((g.conservation_lower_bound_95 - 0.025f64.powf(0.25)).abs() < 1e-15);
    assert_eq!(h.region_count, 1);
    assert_eq!(h.references[0].key, "tharanga2025");
}

#[test]
fn disagreeing_overlap_splits_the_region() {
    // Position 1 (columns 1–2) is AC in three sequences; position 2 (columns 2–3) is GE in three
    // others: both are 100% conserved but disagree on column 2.
    let r = results(&["AC-", "AC-", "AC-", "-GE", "-GE", "-GE"], 2, 1);
    let h = hcs(&r, "100", false);
    assert_eq!(spans(&h), [(1, 2, false), (2, 3, true)]);
    assert_eq!(h.regions[1].sequence, "GE");
}

#[test]
fn a_disagreeing_overlap_is_a_split_also_when_low_support_breaks_the_region() {
    // Position 1 is AAA (columns 1-3), position 2 has no support, positions 3-4 are CCC and
    // CCG; column 3 is A in the first region and C in the second.
    let mut seqs = vec!["AAA-GG"; 3];
    seqs.extend(["G-CCCG"; 3]);
    let r = results(&seqs, 3, 2);
    for eligible_only in [false, true] {
        let h = hcs(&r, "100", eligible_only);
        assert_eq!(
            spans(&h),
            [(1, 3, false), (3, 6, true)],
            "eligible_only {eligible_only}"
        );
        assert_eq!(h.regions[1].sequence, "CCCG");
    }
}

#[test]
fn low_support_positions_flag_or_break_regions() {
    // Columns 3–4 are gaps in all but two sequences: positions 2–4 have support 2 ≤ T = 5.
    let mut seqs = vec!["MKLVWY"; 2];
    seqs.extend(["MK--WY"; 6]);
    let r = results(&seqs, 2, 5);
    let all = hcs(&r, "100", false);
    assert_eq!(spans(&all), [(1, 6, false)]);
    let g = &all.regions[0];
    assert!(g.flagged);
    assert_eq!(g.low_support_positions, [2, 3, 4]);
    assert_eq!(g.min_support, 2);
    assert!((g.conservation_lower_bound_95 - 0.025f64.sqrt()).abs() < 1e-15);
    assert_eq!(all.flagged_region_count, 1);
    assert_eq!(all.diagnostics.len(), 1);
    assert_eq!(all.diagnostics[0].code, "hcs_low_support_regions");
    // Eligible-only: the low-support positions break the region and cannot join it.
    let elig = hcs(&r, "100", true);
    assert_eq!(spans(&elig), [(1, 2, false), (5, 6, false)]);
    assert!(elig.regions.iter().all(|g| !g.flagged));
    assert!(elig.diagnostics.is_empty());
}

#[test]
fn thresholds_are_exact_and_abutting_kmers_join() {
    // 7 of 10 sequences share the Index at every position: 70% exactly.
    let mut seqs = vec!["ACDEFG"; 7];
    seqs.extend(["WWWWWW"; 3]);
    let r = results(&seqs, 3, 1);
    assert_eq!(hcs(&r, "70", false).region_count, 1);
    assert_eq!(hcs(&r, "70.000000001", false).region_count, 0);
    // k = 1 joins abutting single columns.
    let r = results(&seqs, 1, 1);
    let h = hcs(&r, "70", false);
    assert_eq!(spans(&h), [(1, 6, false)]);
    assert_eq!(h.regions[0].sequence, "ACDEFG");
}

#[test]
fn threshold_range_is_checked() {
    let r = results(&["ACDEFG"; 3], 3, 1);
    let at = |s: &str| {
        hcs::compute(
            &r,
            &HcsParams {
                threshold: s.parse().unwrap(),
                eligible_only: false,
            },
        )
    };
    assert!(at("50").is_err());
    assert!(at("50.5").is_ok());
    assert!(at("100").is_ok());
}
