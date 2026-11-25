//! Output formats: JSON round trips, JSONL, tidy TSV tables, compression, schema checks.

use std::io::Read;
use std::path::{Path, PathBuf};

use dima_core::analyze;
use dima_core::config::{Alphabet, AnalysisParams};
use dima_core::control::RunControl;
use dima_core::hcs::{self, HcsParams};
use dima_core::input::Source;
use dima_core::model::Results;
use dima_core::output::{self, OutputCompression, Sink, Table, Target};

fn results() -> Results {
    results_with(false)
}

/// The small test analysis, with or without similarity-weighted entropy.
fn results_with(similarity_weighted_entropy: bool) -> Results {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("t.fa");
    let mut fasta = String::new();
    for (i, (seq, host)) in [
        ("ACGTACGTAC", "Hu\tman"),
        ("ACGTACGTAA", "Camel"),
        ("ACGTAC--AC", "Camel"),
        ("ACGTACGTAC", "\"Bat\""),
    ]
    .iter()
    .enumerate()
    {
        fasta.push_str(&format!(">s{i}|{host}\n{seq}\n"));
    }
    std::fs::write(&p, fasta).unwrap();
    let params = AnalysisParams {
        alphabet: Some(Alphabet::Dna),
        kmer_length: Some(3),
        support_threshold: Some(3),
        header_format: Some(vec!["Id".into(), "Host".into()]),
        metadata_fields: Some(vec!["Host".into()]),
        similarity_weighted_entropy,
        ..AnalysisParams::default()
    };
    analyze(&Source::Path(p), &params, &RunControl::new())
        .unwrap()
        .results
}

fn write(path: &Path, f: impl FnOnce(&mut Sink)) {
    let (_, c) = output::detect(path);
    let mut s = Sink::create(&Target::Path(path.to_path_buf()), c).unwrap();
    f(&mut s);
    s.commit().unwrap();
}

fn text(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    let mut s = String::new();
    match output::detect(path).1 {
        OutputCompression::Gzip => {
            flate2::read::MultiGzDecoder::new(&bytes[..])
                .read_to_string(&mut s)
                .unwrap();
        }
        OutputCompression::Zstd => {
            s = String::from_utf8(zstd::decode_all(&bytes[..]).unwrap()).unwrap()
        }
        OutputCompression::None => s = String::from_utf8(bytes).unwrap(),
    }
    s
}

#[test]
fn json_round_trips_in_every_form() {
    let r = results_with(true);
    assert!(
        r.results
            .iter()
            .any(|p| p.similarity_weighted_entropy > Some(0.0))
    );
    let dir = tempfile::tempdir().unwrap();
    for (name, pretty) in [
        ("a.json", false),
        ("b.json", true),
        ("c.json.gz", false),
        ("d.JSON.ZST", true),
    ] {
        let p = dir.path().join(name);
        write(&p, |s| output::write_json(&r, pretty, s).unwrap());
        let t = text(&p);
        assert!(t.ends_with('\n') && !t.ends_with("\n\n"), "{name}");
        let version = dima_core::model::SCHEMA_VERSION;
        assert!(
            t.starts_with(&if pretty {
                format!("{{\n  \"schema_version\": \"{version}\"")
            } else {
                format!("{{\"schema_version\":\"{version}\"")
            }),
            "{name}"
        );
        assert_eq!(t.lines().count() > 1, pretty, "{name}");
        let back = output::read_results(&p).unwrap();
        assert_eq!(back, r, "{name}");
        // Writing what was read gives identical bytes.
        let q = dir.path().join(format!("again-{name}"));
        write(&q, |s| output::write_json(&back, pretty, s).unwrap());
        assert_eq!(text(&q), t, "{name}");
    }
}

#[test]
fn jsonl_has_one_position_per_line() {
    let r = results();
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.jsonl");
    write(&p, |s| output::write_jsonl(&r, s).unwrap());
    let t = text(&p);
    assert_eq!(t.lines().count() as u64, r.position_count);
    assert!(t.ends_with("}\n"));
    let first: serde_json::Value = serde_json::from_str(t.lines().next().unwrap()).unwrap();
    assert_eq!(first["position"], 1);
}

#[test]
fn tsv_tables_are_tidy_and_quoted_minimally() {
    let r = results();
    let h = hcs::compute(&r, &HcsParams::default()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut tables = Vec::new();
    for t in Table::ALL {
        let p: PathBuf = dir.path().join(format!("x.{}.tsv", t.name()));
        write(&p, |s| output::write_tsv(&r, Some(&h), t, s).unwrap());
        tables.push(text(&p));
    }
    let header: Vec<&str> = tables[0].lines().next().unwrap().split('\t').collect();
    assert_eq!(header, Table::Positions.columns());
    assert_eq!(tables[0].lines().count() as u64, r.position_count + 1);
    // Undefined values are NA; positions with support 4 > T = 3 have no label.
    let row1: Vec<&str> = tables[0].lines().nth(1).unwrap().split('\t').collect();
    assert_eq!(row1[0], "1");
    assert_eq!(row1[1], "NA");
    assert_eq!(row1[11], "NA", "below_min_coverage without --min-coverage");
    let variants: usize = r.results.iter().map(|p| p.diversity_motifs.len()).sum();
    assert_eq!(tables[1].lines().count(), variants + 1);
    // A TAB inside a value is quoted (RFC 4180), and so is a double quote.
    assert!(tables[2].contains("\t\"Hu\tman\"\t"), "{}", tables[2]);
    assert!(tables[2].contains("\t\"\"\"Bat\"\"\"\t"), "{}", tables[2]);
    assert!(tables[3].starts_with("region_id\tstart_column"));
    // Every row of a table has the same number of fields (a tidy rectangle).
    for (t, s) in Table::ALL.iter().zip(&tables) {
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(s.as_bytes());
        for rec in rdr.records() {
            assert_eq!(rec.unwrap().len(), t.columns().len(), "{}", t.name());
        }
    }
}

#[test]
fn reading_rejects_foreign_files() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.json");
    std::fs::write(&p, "{\"hello\": 1}\n").unwrap();
    let e = output::read_results(&p).unwrap_err();
    assert!(e.to_string().contains("not a DiMA results file"), "{e}");
    std::fs::write(&p, "{\"schema_version\":\"2.0.0\"}\n").unwrap();
    let e = output::read_results(&p).unwrap_err();
    assert!(
        e.to_string().contains("written with results schema 2.0.0"),
        "{e}"
    );
    std::fs::write(&p, "{\"schema_version\":\"1.0.0\",\"query_name\":3}\n").unwrap();
    let e = output::read_results(&p).unwrap_err();
    assert!(
        e.to_string().contains("not a valid DiMA results file"),
        "{e}"
    );
    let e = output::read_results(&dir.path().join("missing.json")).unwrap_err();
    assert_eq!(e.class(), dima_core::error::ExitClass::Io);
}

#[test]
fn non_finite_values_are_refused() {
    let mut r = results();
    r.results[0].entropy = Some(f64::NAN);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("bad.json");
    let mut s = Sink::create(&Target::Path(p.clone()), OutputCompression::None).unwrap();
    let e = output::write_json(&r, false, &mut s).unwrap_err();
    assert_eq!(e.class(), dima_core::error::ExitClass::Internal);
    drop(s);
    assert!(!p.exists(), "nothing is written");
}

/// Edited or damaged results are invalid input (exit 4), never a crash in `dima hcs`.
#[test]
fn inconsistent_results_are_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("r.json");
    // Through `serde_json::Value`, keys are written in alphabetical order, as `jq -S` or Python
    // would: the schema version is then no longer the first key.
    let original = serde_json::to_value(results()).unwrap();
    type Damage = (&'static str, fn(&mut serde_json::Value));
    let damage: [Damage; 10] = [
        ("k changed", |v| v["kmer_length"] = 12.into()),
        ("k is 0", |v| v["kmer_length"] = 0.into()),
        ("count above support", |v| {
            v["results"][0]["diversity_motifs"][0]["count"] = 100_000.into();
        }),
        ("lower-case variant", |v| {
            let s = v["results"][0]["diversity_motifs"][0]["sequence"]
                .as_str()
                .unwrap()
                .to_lowercase();
            v["results"][0]["diversity_motifs"][0]["sequence"] = s.into();
        }),
        ("a repeated position", |v| {
            v["results"][0]["position"] = 2.into()
        }),
        ("a missing position", |v| {
            v["results"][1]["position"] = 1_000_000.into()
        }),
        ("position 0", |v| v["results"][0]["position"] = 0.into()),
        // Values outside the schema's ranges.
        ("negative entropy", |v| {
            v["results"][0]["entropy"] = (-0.5).into()
        }),
        ("incidence above 100", |v| {
            v["results"][0]["diversity_motifs"][0]["incidence"] = 100.5.into();
        }),
        ("coverage above 1", |v| {
            v["results"][0]["sample_coverage"] = 1.5.into()
        }),
    ];
    for (what, f) in damage {
        let mut v = original.clone();
        f(&mut v);
        std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
        let e = output::read_results(&p).unwrap_err();
        assert_eq!(
            e.class(),
            dima_core::error::ExitClass::InvalidInput,
            "{what}: {e}"
        );
        // Reported by the consistency check, not mistaken for another kind of file.
        assert!(
            e.hint().unwrap().contains("edited or damaged"),
            "{what}: {e}"
        );
    }
    // The undamaged file reads back, and so does one with its positions shuffled (G25).
    std::fs::write(&p, serde_json::to_vec(&original).unwrap()).unwrap();
    let back = output::read_results(&p).unwrap();
    let mut shuffled = original.clone();
    shuffled["results"].as_array_mut().unwrap().reverse();
    std::fs::write(&p, serde_json::to_vec(&shuffled).unwrap()).unwrap();
    assert_eq!(output::read_results(&p).unwrap(), back);
}

/// The similarity-weighted values must be present exactly where the run computed them: a file
/// that breaks the rule was edited or damaged (exit 4).
#[test]
fn similarity_weighted_values_follow_the_presence_rules() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("r.json");
    let on = serde_json::to_value(results_with(true)).unwrap();
    let off = serde_json::to_value(results()).unwrap();
    // The first position has one distinct k-mer; another has two.
    let with = |n: usize| {
        on["results"]
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["diversity_motifs"].as_array().unwrap().len() == n)
            .unwrap()
    };
    let (single, multi) = (with(1), with(2));
    assert!(on["results"][multi]["normalised_observed_entropy"].is_number());
    type Damage = (&'static str, bool, Box<dyn Fn(&mut serde_json::Value)>);
    let damage: Vec<Damage> = vec![
        (
            "off, with a value",
            false,
            Box::new(|v| {
                v["results"][0]["similarity_weighted_entropy"] = 0.5.into();
            }),
        ),
        (
            "off, with a normalised value",
            false,
            Box::new(move |v| {
                v["results"][multi]["normalised_observed_entropy"] = 0.5.into();
            }),
        ),
        (
            "off, with a summary",
            false,
            Box::new(move |v| {
                v["similarity_weighted_entropy_summary"] = on_summary();
            }),
        ),
        (
            "on, value missing",
            true,
            Box::new(|v| {
                v["results"][0]["similarity_weighted_entropy"] = serde_json::Value::Null;
            }),
        ),
        (
            "on, normalised value at one k-mer",
            true,
            Box::new(move |v| {
                v["results"][single]["normalised_observed_entropy"] = 0.5.into();
            }),
        ),
        (
            "on, normalised value missing",
            true,
            Box::new(move |v| {
                v["results"][multi]["normalised_similarity_weighted_entropy"] =
                    serde_json::Value::Null;
            }),
        ),
        (
            "on, summary missing",
            true,
            Box::new(|v| {
                v["similarity_weighted_entropy_summary"] = serde_json::Value::Null;
            }),
        ),
        (
            "on, negative value",
            true,
            Box::new(|v| {
                v["results"][0]["similarity_weighted_entropy"] = (-0.1).into();
            }),
        ),
        (
            "on, normalised above 1",
            true,
            Box::new(move |v| {
                v["results"][multi]["normalised_observed_entropy"] = 1.5.into();
            }),
        ),
    ];
    for (what, computed, f) in damage {
        let mut v = if computed { on.clone() } else { off.clone() };
        f(&mut v);
        std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
        let e = output::read_results(&p).unwrap_err();
        assert_eq!(
            e.class(),
            dima_core::error::ExitClass::InvalidInput,
            "{what}: {e}"
        );
        assert!(
            e.hint().unwrap().contains("edited or damaged"),
            "{what}: {e}"
        );
    }
    for v in [&on, &off] {
        std::fs::write(&p, serde_json::to_vec(v).unwrap()).unwrap();
        output::read_results(&p).unwrap();
    }
    // Similarity-weighted entropy is undefined only without support.
    let fasta = dir.path().join("gaps.fa");
    std::fs::write(&fasta, ">a\nACG--TTTT\n>b\nACG--TTTA\n>c\nACC--TTTT\n").unwrap();
    let params = AnalysisParams {
        alphabet: Some(Alphabet::Dna),
        kmer_length: Some(3),
        support_threshold: Some(2),
        similarity_weighted_entropy: true,
        ..AnalysisParams::default()
    };
    let gaps = analyze(&Source::Path(fasta), &params, &RunControl::new())
        .unwrap()
        .results;
    let mut v = serde_json::to_value(&gaps).unwrap();
    let unsupported = gaps.results.iter().position(|p| p.support == 0).unwrap();
    v["results"][unsupported]["similarity_weighted_entropy"] = 0.5.into();
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let e = output::read_results(&p).unwrap_err().to_string();
    assert!(e.contains("is undefined here (no support)"), "{e}");
}

fn on_summary() -> serde_json::Value {
    serde_json::to_value(results_with(true).similarity_weighted_entropy_summary).unwrap()
}

/// A results file written by dima 1.0.0 (schema 1.0.0, frozen in `tests/data`) is read as a
/// run without similarity-weighted entropy, and is written again with this version's schema.
#[test]
fn schema_1_0_0_files_are_runs_without_the_option() {
    let frozen =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/results-schema-1.0.0.json.gz");
    let r = output::read_results(&frozen).unwrap();
    assert_eq!(r.schema_version, "1.0.0");
    assert!(!r.run.parameters.similarity_weighted_entropy.value);
    assert_eq!(
        r.run.parameters.similarity_weighted_entropy.origin,
        dima_core::config::Origin::Default
    );
    assert!(r.similarity_weighted_entropy_summary.is_none());
    assert!(r.results.iter().all(|p| {
        p.similarity_weighted_entropy.is_none()
            && p.normalised_observed_entropy.is_none()
            && p.normalised_similarity_weighted_entropy.is_none()
    }));
    let dir = tempfile::tempdir().unwrap();
    let again = dir.path().join("again.json");
    write(&again, |s| output::write_json(&r, false, s).unwrap());
    let back = output::read_results(&again).unwrap();
    assert_eq!(back.schema_version, dima_core::model::SCHEMA_VERSION);
    assert_eq!(back.results, r.results);
}

#[test]
fn schema_versions_parse_exactly() {
    use dima_core::output::parse_schema_version as parse;
    assert_eq!(parse("1.1.0"), Some([1, 1, 0]));
    assert_eq!(parse("0.0.0"), Some([0, 0, 0]));
    for bad in [
        "1.1",
        "1.1.0-dev",
        "01.1.0",
        "+1.1.0",
        "1..0",
        "1.1.0.0",
        "",
        "a.b.c",
        "1.1.00",
    ] {
        assert_eq!(parse(bad), None, "{bad}");
    }
    assert!(parse("1.10.0") > parse("1.9.0"));
}

/// A value this version does not know is refused; in a file of a newer 1.x schema the message
/// says so, instead of calling the file damaged.
#[test]
fn unknown_values_in_newer_files_are_named() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("r.json");
    let mut v = serde_json::to_value(results()).unwrap();
    v["results"][0]["entropy_method"] = "from_the_future".into();
    v.as_object_mut().unwrap().remove("schema_version");
    for (version, expected) in [
        ("1.9.0", "written by a newer DiMA (results schema 1.9.0)"),
        (
            dima_core::model::SCHEMA_VERSION,
            "not a valid DiMA results file",
        ),
    ] {
        // DiMA writes `schema_version` first, where the reader finds it early.
        let text = format!(
            "{{\"schema_version\":{},{}",
            serde_json::to_string(version).unwrap(),
            &serde_json::to_string(&v).unwrap()[1..]
        );
        std::fs::write(&p, text).unwrap();
        let e = output::read_results(&p).unwrap_err();
        assert_eq!(e.class(), dima_core::error::ExitClass::InvalidInput, "{e}");
        assert!(e.to_string().contains(expected), "{version}: {e}");
        assert!(e.to_string().contains("from_the_future"), "{version}: {e}");
    }
    // Conserved-sequences files: their own schema name, and the key that holds their version.
    // Serialised in DiMA's own key order, which tells the reader the kind of file.
    let h = hcs::compute(&results(), &HcsParams::default()).unwrap();
    let text = serde_json::to_string(&h).unwrap();
    let eligibility = format!(
        "\"eligibility\":{}",
        serde_json::to_string(&h.parameters.eligibility).unwrap()
    );
    assert!(text.contains(&eligibility));
    let text = text.replace(&eligibility, "\"eligibility\":\"from_the_future\"");
    let current = format!(
        "\"schema_version\":\"{}\"",
        dima_core::model::HCS_SCHEMA_VERSION
    );
    assert!(text.starts_with(&format!("{{{current}")));
    for (version, expected) in [
        (
            "1.9.0",
            "written by a newer DiMA (conserved-sequences schema 1.9.0)",
        ),
        ("2.0.0", "written with conserved-sequences schema 2.0.0"),
    ] {
        let text = text.replacen(&current, &format!("\"schema_version\":\"{version}\""), 1);
        std::fs::write(&p, text).unwrap();
        let e = output::read_hcs(&p).unwrap_err();
        assert_eq!(e.class(), dima_core::error::ExitClass::InvalidInput, "{e}");
        assert!(e.to_string().contains(expected), "{version}: {e}");
        if version == "1.9.0" {
            let hint = e.hint().unwrap();
            assert!(
                hint.contains("its tool.version") && !hint.contains("run."),
                "{hint}"
            );
        }
    }
}

/// Summaries are refused with values outside their ranges, as the values of a position are, so
/// an edited file is never written on (exit 4).
#[test]
fn summaries_outside_their_ranges_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("r.json");
    let on = serde_json::to_value(results_with(true)).unwrap();
    type Damage = (&'static str, fn(&mut serde_json::Value));
    let damage: [Damage; 5] = [
        ("average_entropy", |v| v["average_entropy"] = (-1.0).into()),
        ("coverage_summary.mean", |v| {
            v["coverage_summary"]["mean"] = 1.5.into()
        }),
        ("highest_entropy", |v| {
            v["highest_entropy"]["position"] = 0.into()
        }),
        ("similarity_weighted_entropy_summary.average_entropy", |v| {
            v["similarity_weighted_entropy_summary"]["average_entropy"] = (-1.0).into();
        }),
        ("similarity_weighted_entropy_summary.lowest_entropy", |v| {
            v["similarity_weighted_entropy_summary"]["lowest_entropy"]["entropy"] = (-1.0).into();
        }),
    ];
    for (field, f) in damage {
        let mut v = on.clone();
        f(&mut v);
        std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
        let e = output::read_results(&p).unwrap_err();
        assert_eq!(
            e.class(),
            dima_core::error::ExitClass::InvalidInput,
            "{field}: {e}"
        );
        assert!(e.to_string().contains(field), "{field}: {e}");
        assert!(
            e.hint().unwrap().contains("edited or damaged"),
            "{field}: {e}"
        );
    }
}

/// Damaged compressed results are invalid input (exit 4) with a hint naming the test command;
/// errors reading the file are I/O errors (exit 3); a byte-order mark is accepted.
#[test]
fn damaged_compressed_results_are_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let r = results();
    for (name, check) in [("r.json.gz", "gzip -t"), ("r.json.zst", "zstd -t")] {
        let p = dir.path().join(name);
        let compression = output::detect(&p).1;
        let mut s = Sink::create(&Target::Path(p.clone()), compression).unwrap();
        output::write_json(&r, false, &mut s).unwrap();
        s.commit().unwrap();
        let bytes = std::fs::read(&p).unwrap();
        // Truncated in the middle, and with one byte changed in the middle.
        let truncated = dir.path().join(format!("truncated.{name}"));
        std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
        let mut flipped = bytes.clone();
        let mid = flipped.len() / 2;
        flipped[mid] ^= 0xFF;
        let corrupt = dir.path().join(format!("corrupt.{name}"));
        std::fs::write(&corrupt, &flipped).unwrap();
        let e = output::read_results(&truncated).unwrap_err();
        assert_eq!(e.class(), dima_core::error::ExitClass::InvalidInput, "{e}");
        assert!(e.to_string().contains("truncated"), "{e}");
        assert!(e.hint().unwrap().contains(check), "{e}");
        // A changed byte is found by the decoder or, when it decodes to damaged JSON, by the
        // parser: invalid input either way.
        let e = output::read_results(&corrupt).unwrap_err();
        assert_eq!(e.class(), dima_core::error::ExitClass::InvalidInput, "{e}");
    }
    let p = dir.path().join("bom.json");
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(serde_json::to_vec(&r).unwrap());
    std::fs::write(&p, bytes).unwrap();
    assert_eq!(output::read_results(&p).unwrap(), r);
}

#[test]
fn reading_results_can_be_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("r.json");
    std::fs::write(&p, serde_json::to_vec(&results()).unwrap()).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let e = output::read_results_cancellable(&p, &cancel).unwrap_err();
    assert!(matches!(e, dima_core::DimaError::Cancelled), "{e}");
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        output::read_results_cancellable(&p, &cancel).unwrap(),
        results()
    );
}

#[test]
fn each_kind_of_file_is_recognised() {
    let dir = tempfile::tempdir().unwrap();
    let r = results();
    let h = hcs::compute(&r, &HcsParams::default()).unwrap();
    let rp = dir.path().join("r.json");
    let hp = dir.path().join("h.json");
    for (path, pretty) in [(&rp, false), (&hp, true)] {
        let mut s = Sink::create(&Target::Path(path.clone()), OutputCompression::None).unwrap();
        if path == &rp {
            output::write_json(&r, pretty, &mut s).unwrap();
        } else {
            output::write_hcs_json(&h, pretty, &mut s).unwrap();
        }
        s.commit().unwrap();
    }
    assert!(matches!(
        output::read_document(&rp).unwrap(),
        output::Document::Results(_)
    ));
    assert!(matches!(
        output::read_document(&hp).unwrap(),
        output::Document::Hcs(_)
    ));
    assert_eq!(output::read_hcs(&hp).unwrap(), h);
    let e = output::read_results(&hp).unwrap_err();
    assert!(
        e.to_string()
            .contains("this is a DiMA conserved-sequences file, not a results file"),
        "{e}"
    );
    let e = output::read_hcs(&rp).unwrap_err();
    assert!(
        e.to_string()
            .contains("this is a DiMA results file, not a conserved-sequences file"),
        "{e}"
    );
}
