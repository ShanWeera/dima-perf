//! Diagnostics and errors of whole analyses (D3, D18, D45, D57–D60, D78).

use std::path::PathBuf;

use dima_core::DimaError;
use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams, Estimator, Origin, ValidationMode};
use dima_core::control::RunControl;
use dima_core::diagnostics::Diagnostic;
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

/// The diagnostic with `code`, which must be present.
#[track_caller]
fn find<'a>(r: &'a Results, code: &str) -> &'a Diagnostic {
    r.run
        .diagnostics
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("{code} not in {:?}", codes(r)))
}

/// A committed dataset (tests/data/README.md).
fn data(path: &str) -> Source {
    Source::Path(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path),
    )
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
    assert_eq!(
        e.hint(),
        Some(
            "fix the headers, change --header-format, or use --validation permissive to give \
             mismatched headers the fill value"
        )
    );
    let permissive = AnalysisParams {
        validation: Some(ValidationMode::Permissive),
        ..params.clone()
    };
    let r = run(&bad, permissive.clone()).unwrap();
    assert!(codes(&r).contains(&"header_field_count_mismatch"));
    assert_eq!(
        find(&r, "header_field_count_mismatch").hint.as_deref(),
        Some("run.metadata.mismatches lists the first headers in full")
    );
    assert_eq!(r.run.metadata.unwrap().mismatches.count, 1);
    // When no header matches, the format is the likely mistake, and the hint says so first.
    let four = AnalysisParams {
        header_format: Some(vec![
            "Id".into(),
            "Host".into(),
            "Year".into(),
            "Country".into(),
        ]),
        ..params
    };
    let e = run(DNA, four.clone()).unwrap_err();
    assert_eq!(
        e.hint(),
        Some(
            "no header has the 4 fields of --header-format, so the format is probably wrong: \
             check it against the headers; otherwise fix the headers, or use --validation \
             permissive to give mismatched headers the fill value"
        )
    );
    let r = run(
        DNA,
        AnalysisParams {
            validation: Some(ValidationMode::Permissive),
            ..four
        },
    )
    .unwrap();
    assert_eq!(
        find(&r, "header_field_count_mismatch").hint.as_deref(),
        Some(
            "no header has the 4 fields of --header-format, so the format is probably wrong: \
             check it against the headers; run.metadata.mismatches lists the first headers in \
             full"
        )
    );
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
    let d = find(&r, "threshold_exceeds_sequence_count");
    assert_eq!(
        d.message,
        "the support threshold (3) is not below the number of sequences (3), so no position can \
         have support above it: the summaries are empty and every entropy is the observed value"
    );
    assert!(d.examples.is_empty(), "the message says it all");
    assert_eq!(
        d.hint.as_deref(),
        Some(
            "lower --support-threshold to 1 or less, so that positions supported by all 3 \
             sequences are eligible and corrected; or add sequences"
        )
    );
    // Another estimator still estimates every position with two sequences.
    let r = run(
        DNA,
        AnalysisParams {
            support_threshold: Some(3),
            estimator: Some(Estimator::MillerMadow),
            ..dna()
        },
    )
    .unwrap();
    let d = find(&r, "threshold_exceeds_sequence_count");
    assert!(
        d.message.ends_with(
            "the summaries are empty, but the miller-madow estimate still applies to every \
             position with at least 2 sequences"
        ),
        "{}",
        d.message
    );
    assert_eq!(
        d.hint.as_deref(),
        Some(
            "lower --support-threshold to 2 or less, so that positions supported by all 3 \
             sequences are eligible; or add sequences"
        )
    );
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
    assert_eq!(
        e.to_string(),
        "k-mer length 11 is longer than the alignment (10 columns)"
    );
    assert_eq!(e.hint(), Some("choose a k-mer length from 1 to 10 with -k"));
    // A default k is named as such, so the user knows where it came from.
    let e = run(
        DNA,
        AnalysisParams {
            kmer_length: None,
            ..dna()
        },
    )
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "k-mer length 27 (the default for DNA and RNA) is longer than the alignment (10 columns)"
    );
    let e = run(
        ">a\nMKVLA\n>b\nMKVLA\n",
        AnalysisParams {
            alphabet: Some(Alphabet::Protein),
            kmer_length: None,
            ..dna()
        },
    )
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "k-mer length 9 (the default for proteins) is longer than the alignment (5 columns)"
    );
    let e = run(
        ">a\nA\n>b\nC\n",
        AnalysisParams {
            kmer_length: Some(2),
            ..dna()
        },
    )
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "k-mer length 2 is longer than the alignment (1 column)"
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

/// `uncorrected_eligible_positions` (with the paper's estimator only): eligible positions with
/// more than one k-mer whose support is too low for the correction's five regression points
/// (below ⌈1.25·T⌉), or whose subsamples all have one size (T = 2, support 3).
#[test]
fn uncorrected_eligible_positions_are_reported() {
    // HIV-1 pol protein, the defaults: positions 42 and 43 are supported by 105 sequences.
    let r = analyze(
        &data("tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2"),
        &AnalysisParams::default(),
        &RunControl::new(),
    )
    .unwrap()
    .results;
    let d = find(&r, "uncorrected_eligible_positions");
    assert_eq!(
        d.message,
        "2 of the 476 eligible positions kept their observed entropy: the paper's correction \
         needs at least five regression points (a minimum taken from DiMA v5.0.9), which \
         support below 125 (1.25 × T) does not give; they are included in the summaries"
    );
    assert_eq!(d.count, 2);
    assert_eq!(
        d.examples,
        [
            "position 42 (support 105, 38 distinct k-mers)",
            "position 43 (support 105, 33 distinct k-mers)"
        ]
    );
    assert_eq!(
        d.hint.as_deref(),
        Some(
            "to have them corrected, raise their support to at least 125 (more sequences with a \
             valid k-mer there) or lower --support-threshold to at most 84, and report the \
             threshold you used; entropy_fallback marks them"
        )
    );
    for p in [&r.results[41], &r.results[42]] {
        assert_eq!(p.entropy, p.observed_entropy, "position {}", p.position);
    }
    // The 26 sequences of DiMA v5.0.9's test data: with T = 25 no eligible position is
    // corrected (support 26 is below 32); with T = 20 every one is; other estimators are silent.
    let fixture = data("tests/data/dima-v5.0.9/input.afa.bz2");
    let at = |t: u64, estimator: Option<Estimator>| {
        let params = AnalysisParams {
            support_threshold: Some(t),
            estimator,
            ..AnalysisParams::default()
        };
        analyze(&fixture, &params, &RunControl::new())
            .unwrap()
            .results
    };
    let r = at(25, None);
    let d = find(&r, "uncorrected_eligible_positions");
    assert_eq!(
        d.message,
        "none of the 997 eligible positions was corrected: the paper's correction needs at \
         least five regression points (a minimum taken from DiMA v5.0.9), which support below \
         32 (1.25 × T) does not give; they kept their observed entropy and are included in the \
         summaries; 916 of them have one k-mer, where the correction would also give 0 bits"
    );
    assert_eq!(d.count, 81, "positions with more than one k-mer");
    assert!(
        d.hint.as_deref().unwrap().contains(
            "at least 32 (more sequences with a valid k-mer there) or lower \
             --support-threshold to at most 20,"
        ),
        "{d:?}"
    );
    for quiet in [
        at(20, None),
        at(25, Some(Estimator::Raw)),
        at(25, Some(Estimator::ChaoWangJost)),
        at(25, Some(Estimator::MillerMadow)),
    ] {
        assert!(
            !codes(&quiet).contains(&"uncorrected_eligible_positions"),
            "{:?}",
            codes(&quiet)
        );
    }
    // T = 2 and 3 sequences: every subsample has 3 sequences, so no regression line exists.
    let r = run(DNA, dna()).unwrap();
    let d = find(&r, "uncorrected_eligible_positions");
    assert_eq!(
        d.message,
        "none of the 8 eligible positions was corrected: all subsamples of the paper's \
         correction have the same size there, so no regression line exists; they kept their \
         observed entropy and are included in the summaries; 7 of them have one k-mer, where \
         the correction would also give 0 bits"
    );
    assert_eq!(d.examples, ["position 8 (support 3, 2 distinct k-mers)"]);
}

/// The coverage in a `below_min_coverage` example is bounded by the minimum: a coverage just
/// below it never reads as reaching it.
#[test]
fn coverage_examples_never_read_as_the_minimum() {
    // Counts 3, 1, 1: coverage 1 − (2/5)·(4/6) = 0.73333…, below 0.73334.
    let r = run(
        ">a\nA\n>b\nA\n>c\nA\n>d\nC\n>e\nG\n",
        AnalysisParams {
            kmer_length: Some(1),
            support_threshold: Some(1),
            min_coverage: Some(0.73334),
            ..dna()
        },
    )
    .unwrap();
    assert_eq!(
        find(&r, "below_min_coverage").examples,
        ["position 1 (coverage > 0.7332)"]
    );
}

/// A nucleotide `--alphabet` on a protein alignment: in strict mode the first amino-acid letter
/// stops the run, and the hint points at `--alphabet`, saying what the residues look like,
/// before permissive mode (which would exclude almost every k-mer).
#[test]
fn a_wrong_alphabet_is_named_in_the_hint() {
    let mers = data("samples/mers_spike_aa.fasta");
    let e = analyze(
        &mers,
        &AnalysisParams {
            alphabet: Some(Alphabet::Dna),
            ..AnalysisParams::default()
        },
        &RunControl::new(),
    )
    .unwrap_err();
    assert!(
        e.to_string().contains("is not valid in a dna alignment"),
        "{e}"
    );
    assert_eq!(
        e.hint(),
        Some(
            "check --alphabet first: the residues of the first records look like protein, not \
             dna; or use --validation permissive to exclude the k-mers that contain such \
             characters (they are counted and reported)"
        )
    );
}
