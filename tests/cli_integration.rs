//! CLI integration tests for the `dima` binary.
//!
//! Uses `assert_cmd` to run the compiled binary and validate:
//! - Exit codes (success, usage error, I/O error, cancelled)
//! - Argument validation (fail-fast before expensive I/O)
//! - JSON output structure correctness
//! - Binary round-trip (analyze → deflate)

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper: create a minimal valid FASTA file for testing.
fn create_test_fasta(sequences: &[(&str, &str)]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    for (header, seq) in sequences {
        writeln!(file, ">{}", header).unwrap();
        writeln!(file, "{}", seq).unwrap();
    }
    file.flush().unwrap();
    file
}

/// Helper: create an aligned protein MSA (all same length).
fn aligned_protein_fasta() -> NamedTempFile {
    create_test_fasta(&[
        ("seq1|USA|2023", "ACDEFGHIKLMNPQRSTVWY"),
        ("seq2|CAN|2023", "ACDEFGHIKLMNPQRSTVWY"),
        ("seq3|MEX|2024", "ACDEFGHIKLMNPQRSTVWF"),
    ])
}

// ─── Exit Code Tests ─────────────────────────────────────────────────────────

#[test]
fn test_missing_input_file_returns_usage_error() {
    Command::cargo_bin("dima")
        .unwrap()
        .args(["analyze", "-i", "/nonexistent/path.fasta"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_kmer_zero_returns_usage_error() {
    let fasta = aligned_protein_fasta();
    Command::cargo_bin("dima")
        .unwrap()
        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--kmer must be >= 1"));
}

#[test]
fn test_kmer_exceeds_max_returns_usage_error() {
    let fasta = aligned_protein_fasta();
    Command::cargo_bin("dima")
        .unwrap()
        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "15"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeds maximum"));
}

#[test]
fn test_dima_output_without_file_returns_usage_error() {
    let fasta = aligned_protein_fasta();
    Command::cargo_bin("dima")
        .unwrap()
        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-O", "dima"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output required for -O dima"));
}

#[test]
fn test_invalid_compression_returns_usage_error() {
    let fasta = aligned_protein_fasta();
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-O", "dima", "-o", "/tmp/test.dima",
            "--compression", "5",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid --compression"));
}

#[test]
fn test_duplicate_header_fields_returns_usage_error() {
    let fasta = aligned_protein_fasta();
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "--header-format", "country|date|country",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate field name"));
}

// ─── Successful Analysis Tests ───────────────────────────────────────────────

#[test]
fn test_basic_analysis_stdout_json() {
    let fasta = aligned_protein_fasta();
    let output = Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3"])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("invalid JSON output");

    assert_eq!(json["sequence_count"], 3);
    assert_eq!(json["kmer_length"], 3);
    assert!(json["results"].is_array());
    assert!(!json["results"].as_array().unwrap().is_empty());
}

#[test]
fn test_jsonl_output_format() {
    let fasta = aligned_protein_fasta();
    let output = Command::cargo_bin("dima")
        .unwrap()
        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3", "-O", "jsonl"])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // JSONL: each non-empty line should be valid JSON (one Position per line)
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(!lines.is_empty(), "JSONL output should have at least one line");
    for line in &lines {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .expect("each JSONL line should be valid JSON");
        assert!(parsed["position"].is_u64(), "each line should have a 'position' field");
    }
}

#[test]
fn test_output_to_file() {
    let fasta = aligned_protein_fasta();
    let output_file = NamedTempFile::new().unwrap();
    let output_path = output_file.path().to_str().unwrap().to_string();

    Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3", "-o", &output_path])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output_path).expect("failed to read output");
    let json: serde_json::Value = serde_json::from_str(&content).expect("invalid JSON in file");
    assert_eq!(json["sequence_count"], 3);
}

#[test]
fn test_binary_roundtrip() {
    let fasta = aligned_protein_fasta();
    let dir = tempfile::tempdir().unwrap();
    let binary_path = dir.path().join("results.dima");
    let json_path = dir.path().join("results.json");

    // Analyze → binary (.dima extension auto-detected)
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3", "-O", "dima",
            "-o", binary_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(binary_path.exists(), "binary file should be created");

    // View → JSON
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "view", "-i", binary_path.to_str().unwrap(),
            "-o", json_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&json_path).expect("failed to read JSON");
    let json: serde_json::Value = serde_json::from_str(&content).expect("invalid JSON");
    assert_eq!(json["sequence_count"], 3);
    assert_eq!(json["kmer_length"], 3);
}

// ─── Alphabet-Aware K-mer Default Tests ──────────────────────────────────────

#[test]
fn test_protein_default_kmer_is_9() {
    let fasta = create_test_fasta(&[
        ("seq1", "ACDEFGHIKLMNPQRSTVWYACDEFGHIKLM"),
        ("seq2", "ACDEFGHIKLMNPQRSTVWYACDEFGHIKLM"),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap()])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["kmer_length"], 9, "protein default k-mer should be 9");
}

#[test]
fn test_nucleotide_default_kmer_is_27() {
    // Need at least 27 nt for a single k-mer window
    let seq = "ACGTACGTACGTACGTACGTACGTACGTACGT"; // 32 nt
    let fasta = create_test_fasta(&[
        ("seq1", seq),
        ("seq2", seq),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "--alphabet", "nucleotide"])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["kmer_length"], 27, "nucleotide default k-mer should be 27");
}

// ─── ELS Tag Test ────────────────────────────────────────────────────────────

#[test]
fn test_els_tag_when_support_equals_threshold() {
    // Create exactly 5 sequences (support = 5), with threshold = 5
    // This should produce ELS (Exceptional Low Support) per the DiMA publication
    let fasta = create_test_fasta(&[
        ("s1", "ACDEFGHIK"),
        ("s2", "ACDEFGHIK"),
        ("s3", "ACDEFGHIK"),
        ("s4", "ACDEFGHIK"),
        ("s5", "ACDEFGHIK"),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "9", "-t", "5",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    // When support == threshold, low_support should be "ELS"
    let results = json["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let first_position = &results[0];
    assert_eq!(
        first_position["low_support"].as_str(),
        Some("ELS"),
        "support == threshold should produce ELS tag"
    );
}

// ─── View Subcommand Tests ───────────────────────────────────────────────────

#[test]
fn test_view_nonexistent_file() {
    Command::cargo_bin("dima")
        .unwrap()
        .args(["view", "-i", "/nonexistent/file.dima"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_view_invalid_binary() {
    let mut bad_file = NamedTempFile::new().unwrap();
    bad_file.write_all(b"not a valid dima file").unwrap();
    bad_file.flush().unwrap();

    let dir = tempfile::tempdir().unwrap();
    let dima_path = dir.path().join("bad.dima");
    std::fs::copy(bad_file.path(), &dima_path).unwrap();

    Command::cargo_bin("dima")
        .unwrap()
        .args(["view", "-i", dima_path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to read binary file"));
}

// ─── Version Flag Test ───────────────────────────────────────────────────────

#[test]
fn test_version_flag() {
    Command::cargo_bin("dima")
        .unwrap()
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dima"));
}

// ─── Header Format Tests ─────────────────────────────────────────────────────

#[test]
fn test_metadata_with_header_format() {
    let fasta = create_test_fasta(&[
        ("USA|2023|Patient1", "ACDEFGHIKLM"),
        ("CAN|2024|Patient2", "ACDEFGHIKLM"),
        ("MEX|2023|Patient3", "ACDEFGHIKLM"),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3", "--header-format", "country|year|patient",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    // Check that metadata is present in variants
    let results = json["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let first_pos = &results[0];
    let variants = first_pos["diversity_motifs"].as_array().unwrap();
    assert!(!variants.is_empty());
    // At least one variant should have metadata
    let has_metadata = variants.iter().any(|v| !v["metadata"].is_null());
    assert!(has_metadata, "variants should have metadata when --header-format is specified");
}

// ─── Empty Field Name Validation ─────────────────────────────────────────────

#[test]
fn test_empty_field_name_in_header_format_returns_error() {
    let fasta = aligned_protein_fasta();

    // "a||b" has an empty field between pipes
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "--header-format", "country||year",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty field name"));
}

#[test]
fn test_leading_pipe_in_header_format_returns_error() {
    let fasta = aligned_protein_fasta();

    // "|field" has an empty first field
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "--header-format", "|country|year",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty field name"));
}

// ─── No-Op Warning Tests ─────────────────────────────────────────────────────

#[test]
fn test_hcs_threshold_without_output_warns() {
    let fasta = aligned_protein_fasta();

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3", "--hcs-threshold", "50.0",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--hcs-threshold has no effect without --hcs-output"),
        "Expected warning about --hcs-threshold, got stderr: {}", stderr
    );
}

// ─── Post-Analysis Summary ───────────────────────────────────────────────────

#[test]
fn test_analysis_prints_summary_to_stderr() {
    let fasta = aligned_protein_fasta();

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("analysis complete") && stderr.contains("positions"),
        "Expected completion summary on stderr, got: {}", stderr
    );
}

// ─── Library-Level K-mer Enforcement ─────────────────────────────────────────

#[test]
fn test_kmer_exceeds_protein_max_via_alphabet() {
    let fasta = aligned_protein_fasta();

    // Protein max is 14; k=15 should fail
    Command::cargo_bin("dima")
        .unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "15", "--alphabet", "protein",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeds maximum"));
}

// ─── Alphabet-Aware Query Name Default ───────────────────────────────────────

#[test]
fn test_nucleotide_default_query_name() {
    // 27-char nucleotide sequences (matching default k=27)
    let fasta = create_test_fasta(&[
        ("seq1", "ACGTACGTACGTACGTACGTACGTACGTACG"),
        ("seq2", "ACGTACGTACGTACGTACGTACGTACGTACG"),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3", "--alphabet", "nucleotide",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["query_name"].as_str().unwrap(), "Unknown Nucleotide");
}

#[test]
fn test_protein_default_query_name() {
    let fasta = aligned_protein_fasta();

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["query_name"].as_str().unwrap(), "Unknown Protein");
}

// ─── Edge Case Tests ─────────────────────────────────────────────────────────

#[test]
fn test_identical_sequences_zero_entropy() {
    // When all sequences are identical, entropy must be 0 at every position
    let fasta = create_test_fasta(&[
        ("s1", "ACDEFGHIKL"),
        ("s2", "ACDEFGHIKL"),
        ("s3", "ACDEFGHIKL"),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3"])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let results = json["results"].as_array().unwrap();
    for pos in results {
        assert_eq!(pos["entropy"].as_f64().unwrap(), 0.0,
            "Expected zero entropy for identical sequences at position {}", pos["position"]);
        // Single variant should be 100% Index motif
        let motifs = pos["diversity_motifs"].as_array().unwrap();
        assert_eq!(motifs.len(), 1);
        assert_eq!(motifs[0]["incidence"].as_f64().unwrap(), 100.0);
    }
}

#[test]
fn test_single_sequence_zero_entropy() {
    // A single sequence should produce zero entropy (no diversity possible)
    let fasta = create_test_fasta(&[("only", "ACDEFGHIKL")]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3"])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["sequence_count"].as_u64().unwrap(), 1);
    assert_eq!(json["average_entropy"].as_f64().unwrap(), 0.0);
}

#[test]
fn test_unequal_length_sequences_error() {
    // MSA validation: sequences must have equal length
    let fasta = create_test_fasta(&[
        ("s1", "ACDEFGHIKL"),
        ("s2", "ACDEFG"),  // shorter — invalid MSA
    ]);

    Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("equal length"));
}

#[test]
fn test_low_support_labels_ns_and_ls() {
    // With 3 sequences and threshold 5: support=3 < threshold=5 → "LS"
    let fasta = create_test_fasta(&[
        ("s1", "ACDEFGHIKL"),
        ("s2", "ACDEFGHIKL"),
        ("s3", "ACDEFGHIKL"),
    ]);

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "3", "-t", "5",
        ])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let results = json["results"].as_array().unwrap();
    // All positions have support=3 < threshold=5, so all should be "LS"
    for pos in results {
        assert_eq!(pos["low_support"].as_str().unwrap(), "LS",
            "position {} should have LS label", pos["position"]);
    }
}

#[test]
fn test_json_output_has_required_fields() {
    let fasta = aligned_protein_fasta();

    let output = Command::cargo_bin("dima")
        .unwrap()

        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "3"])
        .output()
        .expect("failed to run dima");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    // Verify top-level schema
    assert!(json["sequence_count"].is_u64());
    assert!(json["support_threshold"].is_u64());
    assert!(json["low_support_count"].is_u64());
    assert!(json["query_name"].is_string());
    assert!(json["kmer_length"].is_u64());
    assert!(json["average_entropy"].is_f64());
    assert!(json["highest_entropy"].is_object());
    assert!(json["results"].is_array());

    // Verify position schema
    let first = &json["results"][0];
    assert!(first["position"].is_u64());
    assert!(first["entropy"].is_number());
    assert!(first["support"].is_u64());
    assert!(first["diversity_motifs"].is_array());
}

// ─── Shell Completions Tests ────────────────────────────────────────────────

#[test]
fn test_completions_bash() {
    Command::cargo_bin("dima").unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_dima"));
}

#[test]
fn test_completions_zsh() {
    Command::cargo_bin("dima").unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef dima"));
}

#[test]
fn test_completions_fish() {
    Command::cargo_bin("dima").unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

// ─── Header Length Cap Tests ────────────────────────────────────────────────

#[test]
fn test_oversized_header_rejected() {
    let long_header = "x".repeat(20_000); // 20 KB >> 10 KB limit
    let fasta = create_test_fasta(&[
        (&long_header, "ACDEFGHIKLMNPQRSTVWY"),
        ("normal", "ACDEFGHIKLMNPQRSTVWY"),
    ]);

    Command::cargo_bin("dima").unwrap()
        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "9"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeds maximum"));
}

#[test]
fn test_quiet_suppresses_summary() {
    let fasta = aligned_protein_fasta();
    let output = Command::cargo_bin("dima").unwrap()
        .args(["analyze", "-i", fasta.path().to_str().unwrap(), "-k", "9", "--quiet"])
        .assert()
        .success();

    // --quiet should suppress the "Completed: ..." summary line on stderr
    output.stderr(predicate::str::contains("Completed").not());
}

#[test]
fn test_dima_extension_auto_binary() {
    let fasta = aligned_protein_fasta();
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("results.dima");

    // .dima extension auto-triggers binary format without --binary flag
    Command::cargo_bin("dima").unwrap()
        .args([
            "analyze", "-i", fasta.path().to_str().unwrap(),
            "-k", "9", "-o", output_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Verify the file was written and starts with DIMA magic bytes
    let content = std::fs::read(&output_path).unwrap();
    assert_eq!(&content[..4], b"DIMA", "file should start with DIMA magic bytes");
}
