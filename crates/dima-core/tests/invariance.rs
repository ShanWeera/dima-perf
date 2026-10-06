//! Results do not depend on how the input is stored or processed: thread count, batch size,
//! line wrapping, line endings, letter case, byte-order mark and compression (D1, D2, D37,
//! D93; findings A1, A2, A5).

use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use dima_core::analyze;
use dima_core::config::AnalysisParams;
use dima_core::control::RunControl;
use dima_core::input::{BatchLimits, Source};
use dima_core::model::Results;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn mers_params() -> AnalysisParams {
    AnalysisParams {
        header_format: Some(vec![
            "Accession".into(),
            "Host".into(),
            "Geolocation".into(),
            "Year".into(),
        ]),
        metadata_fields: Some(vec!["Host".into(), "Year".into()]),
        ..AnalysisParams::default()
    }
}

fn run_with(source: &Source, params: &AnalysisParams, control: &RunControl) -> Results {
    analyze(source, params, control).unwrap().results
}

/// Everything except the input's identity and the raw character statistics, which legitimately
/// differ between storage variants.
fn content(r: &Results) -> String {
    let mut r = r.clone();
    r.run.input.name.clear();
    r.run.input.size_bytes = 0;
    r.run.input.checksum.value.clear();
    r.run.input.compression = dima_core::model::Compression::None;
    r.run.validation.characters.clear();
    r.run.validation.lowercase_folded = 0;
    r.run.validation.whitespace = Default::default();
    r.run.diagnostics.retain(|d| d.code != "lowercase_folded");
    r.query_name.clear();
    r.run.parameters.query_name.value.clear();
    serde_json::to_string(&r).unwrap()
}

/// HIV-1 pol protein repeated three times: 11,268 records, so batches follow the 10,000-record
/// probe (the first batch).
fn large_input(dir: &Path) -> Source {
    use std::io::Read;
    let f = std::fs::File::open(root().join("tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2")).unwrap();
    let mut text = Vec::new();
    bzip2::read::MultiBzDecoder::new(f)
        .read_to_end(&mut text)
        .unwrap();
    write(dir, "hiv3.fa", &text.repeat(3))
}

#[test]
fn threads_and_batches_do_not_change_a_single_byte() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        // Metadata, low-support positions, a single batch.
        (
            Source::Path(root().join("tests/data/MERS_Spike_nt.fasta.bz2")),
            mers_params(),
        ),
        // NS/LS positions and several batches after the probe.
        (large_input(dir.path()), AnalysisParams::default()),
    ];
    for (source, params) in &cases {
        let reference =
            serde_json::to_string(&run_with(source, params, &RunControl::new())).unwrap();
        for threads in [1, 3, 8] {
            for limits in [
                BatchLimits::default(),
                BatchLimits {
                    max_records: 7,
                    max_bytes: usize::MAX,
                },
                BatchLimits {
                    max_records: usize::MAX,
                    max_bytes: 1,
                },
            ] {
                let control = RunControl {
                    threads: NonZeroUsize::new(threads),
                    batch_limits: limits,
                    ..RunControl::new()
                };
                let got = serde_json::to_string(&run_with(source, params, &control)).unwrap();
                assert!(
                    got == reference,
                    "{}: threads={threads} limits={limits:?} changed the output",
                    source.display_name()
                );
            }
        }
    }
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> Source {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    Source::Path(p)
}

/// Re-wraps every sequence at `width` columns with `eol` line endings.
fn rewrap(fasta: &str, width: usize, eol: &str, lower: bool) -> String {
    let mut out = String::new();
    let mut seq = String::new();
    let flush = |out: &mut String, seq: &mut String| {
        for chunk in seq.as_bytes().chunks(width) {
            let s = std::str::from_utf8(chunk).unwrap();
            out.push_str(&if lower {
                s.to_ascii_lowercase()
            } else {
                s.to_owned()
            });
            out.push_str(eol);
        }
        seq.clear();
    };
    for line in fasta.lines() {
        if let Some(h) = line.strip_prefix('>') {
            flush(&mut out, &mut seq);
            out.push('>');
            out.push_str(h);
            out.push_str(eol);
        } else {
            seq.push_str(line.trim_end());
        }
    }
    flush(&mut out, &mut seq);
    out
}

#[test]
fn storage_variants_give_identical_results() {
    let original = std::fs::read_to_string(root().join("samples/mers_spike_aa.fasta")).unwrap();
    let params = mers_params();
    let control = RunControl::new();
    let reference = content(&run_with(
        &Source::Path(root().join("samples/mers_spike_aa.fasta")),
        &params,
        &control,
    ));
    let dir = tempfile::tempdir().unwrap();
    let one_line = rewrap(&original, usize::MAX, "\n", false);
    let mut variants: Vec<(&str, Vec<u8>)> = vec![
        ("one-line", one_line.clone().into_bytes()),
        (
            "wrap-80-crlf",
            rewrap(&original, 80, "\r\n", false).into_bytes(),
        ),
        (
            "wrap-7-lower",
            rewrap(&original, 7, "\n", true).into_bytes(),
        ),
        (
            "bom",
            [&[0xEF, 0xBB, 0xBF][..], one_line.as_bytes()].concat(),
        ),
        ("no-final-newline", one_line.trim_end().as_bytes().to_vec()),
        (
            "leading-blank-lines",
            [&b"\n \t\r\n\x0B\x0C\n"[..], one_line.as_bytes()].concat(),
        ),
    ];
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(one_line.as_bytes()).unwrap();
    variants.push(("gz", gz.finish().unwrap()));
    let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    bz.write_all(one_line.as_bytes()).unwrap();
    variants.push(("bz2", bz.finish().unwrap()));
    variants.push(("zst", zstd::encode_all(one_line.as_bytes(), 1).unwrap()));
    for (name, bytes) in variants {
        let source = write(dir.path(), name, &bytes);
        let got = content(&run_with(&source, &params, &control));
        assert!(got == reference, "variant {name} changed the results");
    }
}
