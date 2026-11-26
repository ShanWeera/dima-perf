//! Contract tests: every output validates against its JSON Schema (Draft 2020-12; D10, D94),
//! the schemas validate against the meta-schema, and the schema version matches the code.

use std::path::PathBuf;

use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams, Estimator, ValidationMode};
use dima_core::control::RunControl;
use dima_core::hcs::{self, HcsParams};
use dima_core::input::Source;
use dima_core::model::{HCS_SCHEMA_VERSION, SCHEMA_VERSION};
use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("schemas").join(name)).unwrap())
        .unwrap()
}

/// Serves the results schema for the HCS schema's cross-references, offline.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri
            .path()
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_owned();
        Ok(schema(&name))
    }
}

fn validator(name: &str) -> jsonschema::Validator {
    let s = schema(name);
    jsonschema::meta::validate(&s)
        .unwrap_or_else(|e| panic!("{name} is not a valid 2020-12 schema: {e}"));
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_retriever(Local)
        .build(&s)
        .unwrap()
}

#[track_caller]
fn check(v: &jsonschema::Validator, what: &str, value: &Value) {
    let errors: Vec<String> = v
        .iter_errors(value)
        .take(5)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{what}:\n{}", errors.join("\n"));
}

fn fixture(text: &str, params: AnalysisParams) -> dima_core::model::Results {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("f.fasta");
    std::fs::write(&p, text).unwrap();
    analyze(&Source::Path(p), &params, &RunControl::new())
        .unwrap()
        .results
}

#[test]
fn schema_versions_match_the_code() {
    for (name, version) in [
        ("results.schema.json", SCHEMA_VERSION),
        ("hcs.schema.json", HCS_SCHEMA_VERSION),
    ] {
        assert_eq!(
            schema(name)["properties"]["schema_version"]["const"],
            version,
            "{name}"
        );
    }
}

#[test]
fn every_output_validates() {
    let results = validator("results.schema.json");
    let hcs_schema = validator("hcs.schema.json");
    let dna = AnalysisParams {
        alphabet: Some(Alphabet::Dna),
        kmer_length: Some(3),
        support_threshold: Some(2),
        ..AnalysisParams::default()
    };
    let mut cases = vec![
        ("MERS with metadata", {
            let params = AnalysisParams {
                header_format: Some(vec![
                    "Accession".into(),
                    "Host".into(),
                    "Geolocation".into(),
                    "Year".into(),
                ]),
                min_coverage: Some(0.99),
                ..AnalysisParams::default()
            };
            analyze(
                &Source::Path(root().join("samples/mers_spike_aa.fasta")),
                &params,
                &RunControl::new(),
            )
            .unwrap()
            .results
        }),
        (
            "no support anywhere",
            fixture(">a\nAC--GT\n>b\nNN-NNN\n", dna.clone()),
        ),
        (
            "RNA, all unique",
            fixture(
                ">a\nACGU\n>b\nUGCA\n>c\nCAGU\n",
                AnalysisParams {
                    alphabet: Some(Alphabet::Rna),
                    kmer_length: Some(4),
                    ..dna.clone()
                },
            ),
        ),
        (
            "permissive with unknown characters and mismatched headers",
            fixture(
                ">a|x\nACGTJ\n>b\nACGTA\n",
                AnalysisParams {
                    validation: Some(ValidationMode::Permissive),
                    header_format: Some(vec!["Id".into(), "Host".into()]),
                    ..dna.clone()
                },
            ),
        ),
    ];
    for (what, text) in [
        (
            "similarity-weighted, no support and one k-mer",
            ">a\nACG--TTTT\n>b\nACG--TTTA\n>c\nACC--TTTT\n",
        ),
        (
            "similarity-weighted, diverse",
            ">a\nACGTACGT\n>b\nACGAACGT\n>c\nACGTACGA\n",
        ),
    ] {
        cases.push((
            what,
            fixture(
                text,
                AnalysisParams {
                    similarity_weighted_entropy: true,
                    ..dna.clone()
                },
            ),
        ));
    }
    for estimator in Estimator::ALL {
        cases.push((
            "estimator",
            fixture(
                ">a\nACGTACGT\n>b\nACGAACGT\n>c\nACGTACGA\n",
                AnalysisParams {
                    estimator: Some(estimator),
                    support_threshold: Some(1),
                    ..dna.clone()
                },
            ),
        ));
    }
    for (what, r) in &cases {
        let value = serde_json::to_value(r).unwrap();
        check(&results, what, &value);
        for eligible_only in [false, true] {
            let h = hcs::compute(
                r,
                &HcsParams {
                    threshold: "60".parse().unwrap(),
                    eligible_only,
                },
            )
            .unwrap();
            check(&hcs_schema, what, &serde_json::to_value(&h).unwrap());
        }
    }
    // A misspelt or extra key is rejected (additionalProperties: false).
    let mut bad = serde_json::to_value(&cases[1].1).unwrap();
    bad["unexpected"] = Value::Bool(true);
    assert!(!results.is_valid(&bad));
    // Nested definitions are enforced …
    let mut bad = serde_json::to_value(&cases[0].1).unwrap();
    bad["results"][24]["diversity_motifs"][0]["motif_short"] = Value::from("X");
    assert!(!results.is_valid(&bad));
    // The presence rules of the similarity-weighted values hold in both directions.
    let on = serde_json::to_value(&cases[5].1).unwrap();
    // Every position of the flag-off MERS file is supported, so a value there breaks only the
    // rule for runs without the option.
    let off = serde_json::to_value(&cases[0].1).unwrap();
    let multi = on["results"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["diversity_motifs"].as_array().unwrap().len() >= 2)
        .unwrap();
    // The flag-on fixture has positions without support and positions with one k-mer.
    let sparse = serde_json::to_value(&cases[4].1).unwrap();
    let at = |v: &Value, keep: fn(&Value) -> bool| -> usize {
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .position(keep)
            .unwrap()
    };
    let unsupported = at(&sparse, |p| p["support"] == 0);
    let single = at(&sparse, |p| {
        p["support"].as_u64().unwrap() > 0 && p["diversity_motifs"].as_array().unwrap().len() == 1
    });
    for (what, i, key) in [
        (
            "on with a value without support",
            unsupported,
            "similarity_weighted_entropy",
        ),
        (
            "on with a normalised value at one k-mer",
            single,
            "normalised_observed_entropy",
        ),
        (
            "on with a normalised value at one k-mer",
            single,
            "normalised_similarity_weighted_entropy",
        ),
    ] {
        let mut bad = sparse.clone();
        bad["results"][i][key] = Value::from(0.5);
        assert!(!results.is_valid(&bad), "{what}: {key}");
    }
    type Damage<'a> = (&'a str, &'a Value, fn(&mut Value, usize));
    let damaged: [Damage<'_>; 6] = [
        ("off with a value", &off, |v, _| {
            v["results"][0]["similarity_weighted_entropy"] = Value::from(0.5);
        }),
        ("off with a normalised value", &off, |v, _| {
            v["results"][0]["normalised_observed_entropy"] = Value::from(0.5);
        }),
        ("off with a summary", &off, |v, _| {
            v["similarity_weighted_entropy_summary"] = serde_json::json!({
                "position_count": 0, "average_entropy": null, "lowest_entropy": null,
                "highest_entropy": null, "average_observed_entropy": null
            });
        }),
        ("on without a value", &on, |v, i| {
            v["results"][i]["similarity_weighted_entropy"] = Value::Null;
        }),
        ("on without a normalised value", &on, |v, i| {
            v["results"][i]["normalised_observed_entropy"] = Value::Null;
        }),
        ("on without a summary", &on, |v, _| {
            v["similarity_weighted_entropy_summary"] = Value::Null;
        }),
    ];
    for (what, original, damage) in damaged {
        let mut bad = original.clone();
        damage(&mut bad, multi);
        assert!(!results.is_valid(&bad), "{what}");
    }
    // … and so are references into the results schema from the HCS schema.
    let h = hcs::compute(&cases[0].1, &HcsParams::default()).unwrap();
    let mut bad = serde_json::to_value(&h).unwrap();
    bad["references"][0]["doi"] = Value::from("not-a-doi");
    assert!(!hcs_schema.is_valid(&bad));
}

/// A results file written by dima 1.0.0, read and written again, validates against this
/// version's schema (its new keys null and its new parameter off).
#[test]
fn rewritten_1_0_0_files_validate() {
    let frozen = root().join("tests/data/results-schema-1.0.0.json.gz");
    let r = dima_core::output::read_results(&frozen).unwrap();
    assert_eq!(r.schema_version, "1.0.0");
    let dir = tempfile::tempdir().unwrap();
    let again = dir.path().join("again.json");
    let mut sink = dima_core::output::Sink::create(
        &dima_core::output::Target::Path(again.clone()),
        dima_core::output::OutputCompression::None,
    )
    .unwrap();
    dima_core::output::write_json(&r, false, &mut sink).unwrap();
    sink.commit().unwrap();
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&again).unwrap()).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    check(
        &validator("results.schema.json"),
        "rewritten 1.0.0 file",
        &value,
    );
}

/// The schema's lists of method and fallback names are exactly the code's: a value the code can
/// write is never refused by the schema, and the schema lists no value the code never writes.
#[test]
fn enumerations_match_the_code() {
    use dima_core::model::{EntropyFallback, EntropyMethod};
    // Exhaustive matches: adding a variant does not compile until it is listed here.
    let methods = [
        EntropyMethod::Dima,
        EntropyMethod::ChaoWangJost,
        EntropyMethod::MillerMadow,
        EntropyMethod::PlugIn,
        EntropyMethod::NotEstimable,
    ];
    for m in methods {
        match m {
            EntropyMethod::Dima
            | EntropyMethod::ChaoWangJost
            | EntropyMethod::MillerMadow
            | EntropyMethod::PlugIn
            | EntropyMethod::NotEstimable => {}
        }
    }
    let fallbacks = [
        EntropyFallback::SupportAtOrBelowThreshold,
        EntropyFallback::TooFewPoints,
        EntropyFallback::DegenerateRegression,
        EntropyFallback::SingleObservation,
    ];
    for f in fallbacks {
        match f {
            EntropyFallback::SupportAtOrBelowThreshold
            | EntropyFallback::TooFewPoints
            | EntropyFallback::DegenerateRegression
            | EntropyFallback::SingleObservation => {}
        }
    }
    let results = schema("results.schema.json");
    let listed = |field: &str| -> Vec<Value> {
        let mut v = results["$defs"]["position"]["properties"][field]["enum"]
            .as_array()
            .unwrap()
            .clone();
        v.retain(|x| !x.is_null());
        v
    };
    let names = |v: Vec<&str>| -> Vec<Value> { v.into_iter().map(Value::from).collect() };
    assert_eq!(
        listed("entropy_method"),
        names(methods.iter().map(|m| m.as_str()).collect())
    );
    assert_eq!(
        listed("entropy_fallback"),
        names(fallbacks.iter().map(|f| f.as_str()).collect())
    );
}
