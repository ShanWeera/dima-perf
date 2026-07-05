//! Snapshot tests for DiMA JSON output stability.
//!
//! Uses `insta` to lock in the structure and key fields of the output JSON.
//! Any unintentional change to field names, nesting, or types will cause a
//! snapshot mismatch, preventing silent regressions.
//!
//! To update snapshots after intentional changes:
//!   cargo insta review

use assert_cmd::Command;

/// Run the CLI with given args and return stdout as a string.
fn run_dima(args: &[&str]) -> String {
    let output = Command::cargo_bin("dima")
        .unwrap()
        .args(args)
        .output()
        .expect("failed to run dima");
    assert!(
        output.status.success(),
        "dima failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Snapshot test: verify the JSON structure of analyze output.
/// Locks in field names, nesting, and top-level structure without
/// pinning exact floating-point values (uses redactions for entropy).
#[test]
fn test_analyze_json_structure() {
    let output = run_dima(&["analyze", "-i", "samples/mers_spike_aa.fasta", "-k", "9"]);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Snapshot the top-level structure (redact volatile values)
    insta::assert_json_snapshot!("analyze_toplevel", json, {
        ".highest_entropy.entropy" => "[entropy_f64]",
        ".highest_entropy.position" => "[position]",
        ".average_entropy" => "[avg_entropy_f64]",
        ".results" => insta::sorted_redaction(),
        ".results[].entropy" => "[entropy_f64]",
        ".results[].support" => "[support]",
        ".results[].distinct_variants_count" => "[count]",
        ".results[].distinct_variants_incidence" => "[incidence]",
        ".results[].total_variants_incidence" => "[incidence]",
        ".results[].diversity_motifs" => "[motifs_array]",
    });
}

/// Snapshot test: verify the first position's detailed structure.
/// Locks in the exact schema of a single result position including
/// variant/motif structure.
#[test]
fn test_analyze_first_position_schema() {
    let output = run_dima(&["analyze", "-i", "samples/mers_spike_aa.fasta", "-k", "9"]);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();

    let first_position = &json["results"][0];
    insta::assert_json_snapshot!("first_position_schema", first_position, {
        ".entropy" => "[entropy_f64]",
        ".support" => "[support]",
        ".distinct_variants_count" => "[count]",
        ".distinct_variants_incidence" => "[incidence]",
        ".total_variants_incidence" => "[incidence]",
        ".diversity_motifs[].count" => "[count]",
        ".diversity_motifs[].incidence" => "[incidence_f64]",
        ".diversity_motifs[].sequence" => "[kmer_sequence]",
        ".diversity_motifs[].metadata" => "[metadata]",
    });
}

/// Snapshot test: verify the structure of identical-sequence output
/// (zero entropy, single Index variant).
#[test]
fn test_identical_sequences_output_schema() {
    // Create a simple input with identical sequences
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("identical.fasta");
    std::fs::write(
        &input,
        ">s1\nACDEFGHIKL\n>s2\nACDEFGHIKL\n>s3\nACDEFGHIKL\n",
    )
    .unwrap();

    let output = Command::cargo_bin("dima")
        .unwrap()
        .args(["analyze", "-i", input.to_str().unwrap(), "-k", "3"])
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).unwrap()).unwrap();

    // For identical sequences: entropy=0, single Index variant at 100%
    insta::assert_json_snapshot!("identical_sequences", json, {
        ".results[].support" => "[support]",
        ".results[].distinct_variants_count" => "[count]",
        ".results[].distinct_variants_incidence" => "[incidence]",
        ".results[].total_variants_incidence" => "[incidence]",
        ".results[].diversity_motifs[].sequence" => "[kmer_sequence]",
        ".results[].diversity_motifs[].metadata" => "[metadata]",
    });
}
