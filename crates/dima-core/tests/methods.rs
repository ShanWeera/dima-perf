//! The methods paragraph of `dima cite --from` and the desktop app's "Methods & references"
//! (D89): it cites every reference its run recorded, on real analyses, and the example on the
//! documentation's *Citing DiMA* page is the paragraph DiMA writes.

use std::path::PathBuf;

use dima_core::analyze;
use dima_core::citations::{self, Cite};
use dima_core::config::{AnalysisParams, Estimator, Percent};
use dima_core::control::RunControl;
use dima_core::hcs::{self, HcsParams};
use dima_core::input::Source;
use dima_core::methods;
use dima_core::model::{HcsResults, Reference, Results};
use dima_core::provenance::SOFTWARE_REFERENCE_KEY;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(path: &str, params: &AnalysisParams) -> Results {
    analyze(&Source::Path(root().join(path)), params, &RunControl::new())
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .results
}

fn golden(path: &str) -> Results {
    serde_json::from_slice(&std::fs::read(root().join(path)).unwrap()).unwrap()
}

/// `true` when `text` cites the reference in one of the paragraph's forms: `(Rao 1945)`, or
/// narratively `Chao & Jost (2012)` and `Tharanga et al. (2025, Fig. 2)`.
fn cites(text: &str, r: &Reference) -> bool {
    let c = citations::by_key(&r.key).unwrap_or_else(|| panic!("{} is unknown", r.key));
    let cite = Cite::new(c.key);
    let narrative = cite.narrative();
    text.contains(&cite.short()) || text.contains(narrative.trim_end_matches(')'))
}

/// Every reference of `refs` is cited in `text`; the software by its version.
#[track_caller]
fn every_reference_is_cited(what: &str, text: &str, refs: &[Reference], version: &str) {
    for r in refs {
        if r.key == SOFTWARE_REFERENCE_KEY {
            assert!(
                text.contains(&format!("using dima {version}")),
                "{what}: the software is not named: {text}"
            );
        } else {
            assert!(cites(text, r), "{what}: {} is not cited: {text}", r.key);
        }
    }
}

fn hcs_at(r: &Results, threshold: &str, eligible_only: bool) -> HcsResults {
    let params = HcsParams {
        threshold: threshold.parse::<Percent>().unwrap(),
        eligible_only,
    };
    hcs::compute(r, &params).unwrap()
}

#[test]
fn the_paragraph_cites_every_reference_of_the_run() {
    let defaults = AnalysisParams::default();
    let with = |estimator: Option<Estimator>, similarity: bool| AnalysisParams {
        estimator,
        similarity_weighted_entropy: similarity,
        ..AnalysisParams::default()
    };
    let mut cases: Vec<(String, Results)> = Vec::new();
    for (name, params) in [
        ("MERS-CoV spike protein", defaults.clone()),
        (
            "MERS-CoV spike protein, similarity-weighted",
            with(None, true),
        ),
        (
            "MERS-CoV spike protein, chao-wang-jost",
            with(Some(Estimator::ChaoWangJost), false),
        ),
        (
            "MERS-CoV spike protein, miller-madow",
            with(Some(Estimator::MillerMadow), false),
        ),
        (
            "MERS-CoV spike protein, raw",
            with(Some(Estimator::Raw), false),
        ),
    ] {
        cases.push((name.to_owned(), run("samples/mers_spike_aa.fasta", &params)));
    }
    cases.push((
        "MERS-CoV spike gene, similarity-weighted".to_owned(),
        run("tests/data/MERS_Spike_nt.fasta.bz2", &with(None, true)),
    ));
    cases.push((
        "HIV-1 pol DNA".to_owned(),
        run("tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2", &defaults),
    ));
    for golden_file in [
        "tests/golden/tiny/results.json",
        "tests/golden/tiny_similarity/results.json",
    ] {
        cases.push((golden_file.to_owned(), golden(golden_file)));
    }
    let mut keys = std::collections::BTreeSet::new();
    for (name, r) in &cases {
        let version = &r.run.tool.version;
        every_reference_is_cited(name, &methods::sentence(r), &r.run.references, version);
        keys.extend(r.run.references.iter().map(|x| x.key.clone()));
        // With the conserved sequences: their references too, in both eligibility modes.
        for eligible_only in [false, true] {
            let h = hcs_at(r, "95", eligible_only);
            let text = methods::methods_text(methods::Cited {
                results: Some(r),
                hcs: Some(&h),
            })
            .unwrap();
            every_reference_is_cited(name, &text, &h.references, version);
            // The sentence alone too (`dima cite --from` with the conserved sequences only).
            for x in &h.references {
                assert!(cites(&methods::hcs_sentence(&h), x), "{name}: {}", x.key);
            }
        }
    }
    // The cases cover every reference a run can record.
    for key in [
        "eddy2011",
        "cornishbowden1985",
        "wu2025",
        "grishin1995",
        "rao1945",
        "blackwell1947",
        "chao2013",
        "miller1955",
    ] {
        assert!(keys.contains(key), "{key} not covered");
    }
}

/// The blockquotes of a documentation page, each joined into one line.
fn blockquotes(page: &str) -> Vec<String> {
    let mut quotes = Vec::new();
    let mut current: Option<String> = None;
    for line in page.lines() {
        if let Some(text) = line.strip_prefix("> ") {
            let q = current.get_or_insert_with(String::new);
            if !q.is_empty() {
                q.push(' ');
            }
            q.push_str(text.trim_end());
        } else if let Some(q) = current.take() {
            quotes.push(q);
        }
    }
    quotes.extend(current);
    quotes
}

/// The examples on *Citing DiMA* are DiMA's own output for the MERS-CoV spike sample, at this
/// version: the paragraph with the defaults, the sentence that similarity-weighted entropy adds,
/// and the sentence of the conserved sequences at the default 100%.
#[test]
fn the_citing_page_shows_the_paragraph_dima_writes() {
    let page =
        std::fs::read_to_string(root().join("docs/src/content/docs/reference/citing.mdx")).unwrap();
    let quotes = blockquotes(&page);
    let sample = "samples/mers_spike_aa.fasta";
    let r = run(sample, &AnalysisParams::default());
    let paragraph = methods::sentence(&r);
    assert!(
        quotes.contains(&paragraph),
        "citing.mdx should quote, for dima {}:\n\n{paragraph}\n\nIt quotes:\n{quotes:#?}",
        citations::SOFTWARE_VERSION
    );
    let similarity = methods::sentence(&run(
        sample,
        &AnalysisParams {
            similarity_weighted_entropy: true,
            ..AnalysisParams::default()
        },
    ));
    let added = similarity
        .strip_prefix(paragraph.as_str())
        .expect("the option only adds a sentence")
        .trim_start()
        .to_owned();
    assert!(
        quotes.contains(&added),
        "citing.mdx should quote:\n\n{added}\n\nIt quotes:\n{quotes:#?}"
    );
    let regions = methods::hcs_sentence(&hcs::compute(&r, &HcsParams::default()).unwrap());
    assert!(
        quotes.contains(&regions),
        "citing.mdx should quote:\n\n{regions}\n\nIt quotes:\n{quotes:#?}"
    );
}
