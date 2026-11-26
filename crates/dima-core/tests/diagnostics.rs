//! Diagnostics and errors of whole analyses (D3, D18, D45, D57–D60, D78).

use std::path::PathBuf;

use dima_core::DimaError;
use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams, Origin, ValidationMode};
use dima_core::control::RunControl;
use dima_core::error::ExitClass;
use dima_core::input::Source;
use dima_core::model::Results;

fn file(text: &str) -> (tempfile::TempDir, Source) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("in.fasta");
    std::fs::write(&p, text).unwrap();
    (dir, Source::Path(p))
}

fn run(text: &str, params: AnalysisParams) -> Result<Results, DimaError> {
    let (_d, s) = file(text);
    analyze(&s, &params, &RunControl::new()).map(|a| a.results)
}

fn codes(r: &Results) -> Vec<&str> {
    r.run.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

const DNA: &str =
    ">a|Human|2015\nACGTACGTAC\n>b|human |2016\nACGTACGTAA\n>c|Camel|nan\nACGTACGTAC\n";

fn dna() -> AnalysisParams {
    AnalysisParams {
        alphabet: Some(Alphabet::Dna),
        kmer_length: Some(3),
        support_threshold: Some(2),
        ..AnalysisParams::default()
    }
}

#[test]
fn strict_mode_rejects_unknown_characters_with_location() {
    let e = run(">a\nACGTJ\n>b\nACGTA\n", dna()).unwrap_err();
    assert_eq!(e.class(), ExitClass::InvalidInput);
    assert!(e.to_string().contains("record 1 (line 2, column 5"), "{e}");
    let permissive = AnalysisParams {
        validation: Some(ValidationMode::Permissive),
        support_threshold: Some(1),
        ..dna()
    };
    let r = run(">a\nACGTJ\n>b\nACGTA\n", permissive).unwrap();
    assert_eq!(codes(&r), ["unknown_characters_excluded"]);
    assert_eq!(r.run.validation.unknown_total, 1);
    // The k-mers containing J are excluded: position 3 (columns 3–5) has support 1.
    assert_eq!(r.results[2].support, 1);
}

#[test]
fn header_metadata_diagnostics() {
    let params = AnalysisParams {
        header_format: Some(vec!["Id".into(), "Host".into(), "Year".into()]),
        ..dna()
    };
    let r = run(DNA, params.clone()).unwrap();
    let c = codes(&r);
    assert!(c.contains(&"header_values_cleaned"), "{c:?}");
    assert!(c.contains(&"near_duplicate_values"), "Human / human: {c:?}");
    assert!(c.contains(&"missing_like_values"), "nan: {c:?}");
    let md = r.run.metadata.as_ref().unwrap();
    assert_eq!(md.stratified, ["Id", "Host", "Year"]);
    assert_eq!(
        r.run.validation.header_cleaning.as_ref().unwrap().trimmed,
        1
    );
    // Mismatched headers: strict lists them after reading everything; permissive fills.
    let bad = format!("{DNA}>d|Bat\nACGTACGTAC\n");
    let e = run(&bad, params.clone()).unwrap_err();
    assert_eq!(e.class(), ExitClass::InvalidInput);
    assert!(
        e.to_string().contains("record 4 (line 7): 2 fields: d|Bat"),
        "{e}"
    );
    let r = run(
        &bad,
        AnalysisParams {
            validation: Some(ValidationMode::Permissive),
            ..params
        },
    )
    .unwrap();
    assert!(codes(&r).contains(&"header_field_count_mismatch"));
    assert_eq!(r.run.metadata.unwrap().mismatches.count, 1);
}

#[test]
fn identifier_like_fields_are_reported() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/mers_spike_aa.fasta");
    let params = AnalysisParams {
        header_format: Some(vec![
            "Accession".into(),
            "Host".into(),
            "Geolocation".into(),
            "Year".into(),
        ]),
        ..AnalysisParams::default()
    };
    let r = analyze(&Source::Path(p), &params, &RunControl::new())
        .unwrap()
        .results;
    let d = r
        .run
        .diagnostics
        .iter()
        .find(|d| d.code == "identifier_like_field")
        .unwrap();
    assert!(d.message.contains("\"Accession\""));
    assert_eq!(
        d.hint.as_deref(),
        Some("to stratify without them: --metadata-fields 'Host|Geolocation|Year'")
    );
}

#[test]
fn threshold_and_parameter_diagnostics() {
    let r = run(
        DNA,
        AnalysisParams {
            support_threshold: Some(3),
            ..dna()
        },
    )
    .unwrap();
    assert_eq!(codes(&r), ["threshold_exceeds_sequence_count"]);
    assert_eq!(r.eligible_position_count, 0);
    assert_eq!(r.average_entropy, None);
    let r = run(
        DNA,
        AnalysisParams {
            header_fill: Some(" NA\t".into()),
            ..dna()
        },
    )
    .unwrap();
    assert!(codes(&r).contains(&"flag_has_no_effect"));
    // Provenance records the value as used (trimmed), with its origin.
    assert_eq!(r.run.parameters.header_fill.value, "NA");
    assert_eq!(r.run.parameters.header_fill.origin, Origin::User);
    let r = run(
        DNA,
        AnalysisParams {
            min_coverage: Some(1.0),
            ..dna()
        },
    )
    .unwrap();
    assert!(codes(&r).contains(&"below_min_coverage"), "{:?}", codes(&r));
    assert_eq!(r.coverage_summary.min_coverage, Some(1.0));
}

#[test]
fn usage_errors() {
    let e = run(
        DNA,
        AnalysisParams {
            kmer_length: Some(11),
            ..dna()
        },
    )
    .unwrap_err();
    assert_eq!(e.class(), ExitClass::Usage);
    assert!(
        e.to_string()
            .contains("longer than the alignment (10 columns)")
    );
    let e = run(
        DNA,
        AnalysisParams {
            metadata_fields: Some(vec!["Host".into()]),
            ..dna()
        },
    )
    .unwrap_err();
    assert_eq!(e.class(), ExitClass::Usage);
    // Too few residues to detect the alphabet: ask for --alphabet (D78).
    let e = run(
        ">a\nACG\n",
        AnalysisParams {
            alphabet: None,
            kmer_length: Some(1),
            ..dna()
        },
    )
    .unwrap_err();
    assert_eq!(e.class(), ExitClass::InvalidInput);
    assert!(e.hint().unwrap().contains("--alphabet"));
}

#[test]
fn provenance_records_origins_and_defaults() {
    let long = DNA
        .replace("ACGTACGTAC\n", "ACGTACGTACGTACGTACGTACGTACGTAC\n")
        .replace("ACGTACGTAA\n", "ACGTACGTACGTACGTACGTACGTACGTAA\n");
    let r = run(
        &long,
        AnalysisParams {
            kmer_length: None,
            alphabet: None,
            ..dna()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let p = &r.run.parameters;
    assert_eq!(
        (p.alphabet.value, p.alphabet.origin),
        (Alphabet::Dna, Origin::Derived)
    );
    assert_eq!(
        (p.kmer_length.value, p.kmer_length.origin),
        (27, Origin::Derived)
    );
    assert_eq!(p.support_threshold.origin, Origin::User);
    assert_eq!(p.estimator.origin, Origin::Default);
    assert_eq!(
        (p.query_name.value.as_str(), p.query_name.origin),
        ("in", Origin::Derived)
    );
    assert_eq!(r.query_name, "in");
    assert_eq!(r.run.input.name, "in.fasta");
    assert_eq!(r.run.input.checksum.kind, "sha-256");
    assert!(
        r.run.references.iter().any(|x| x.key == "eddy2011"),
        "alphabet detection is cited"
    );
    assert_eq!(
        (
            p.similarity_weighted_entropy.value,
            p.similarity_weighted_entropy.origin
        ),
        (false, Origin::Default)
    );
    assert!(!r.run.references.iter().any(|x| x.key == "wu2025"));
    let on = run(
        &long,
        AnalysisParams {
            similarity_weighted_entropy: true,
            ..dna()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let q = &on.run.parameters;
    assert_eq!(
        (
            q.similarity_weighted_entropy.value,
            q.similarity_weighted_entropy.origin
        ),
        (true, Origin::User)
    );
    let keys: Vec<&str> = on.run.references.iter().map(|x| x.key.as_str()).collect();
    assert!(
        keys.contains(&"wu2025") && !keys.contains(&"grishin1995"),
        "{keys:?}"
    );
    // The methods sentence cites what the references list: Grishin only for protein runs.
    let s = dima_core::methods::sentence(&on);
    assert!(s.contains("Wu (2025)") && !s.contains("Grishin"), "{s}");
}
