//! Golden outputs (D94): every file `dima` writes for a case is compared byte for byte with
//! `tests/golden/<case>/`. Re-bless with `DIMA_BLESS=1 cargo test -p dima --test golden` and
//! review the diff.
//!
//! The outputs must be identical on every platform, thread count and batch size; the cases
//! run with two thread counts to check the latter here too.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn dima(dir: &Path, args: &[&str]) {
    let o = Command::new(env!("CARGO_BIN_EXE_dima"))
        .args(args)
        .arg("-q")
        .current_dir(dir)
        .env_remove("RAYON_NUM_THREADS")
        .env_remove("DIMA_LOG")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "dima {args:?} failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

/// Every regular file under `dir`, by path relative to it.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_path_buf();
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

/// Runs `commands` in a fresh directory and compares what they wrote with the golden files
/// (the case's inputs, named in `inputs`, are not outputs).
fn case(name: &str, inputs: &[&str], commands: &[Vec<String>]) {
    let golden = repo().join("tests/golden").join(name);
    for threads in ["1", "3"] {
        let work = tempfile::tempdir().unwrap();
        for c in commands {
            let mut args: Vec<&str> = c.iter().map(String::as_str).collect();
            if args[0] == "analyze" {
                args.extend(["--threads", threads]);
            }
            dima(work.path(), &args);
        }
        let actual = files(work.path());
        if std::env::var_os("DIMA_BLESS").is_some() {
            for (rel, bytes) in &actual {
                let p = golden.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, bytes).unwrap();
            }
            for rel in files(&golden).keys() {
                let is_input = inputs.iter().any(|i| rel == Path::new(i));
                if !is_input && !actual.contains_key(rel) {
                    std::fs::remove_file(golden.join(rel)).unwrap();
                }
            }
            continue;
        }
        let mut expected = files(&golden);
        for i in inputs {
            expected.remove(Path::new(i));
        }
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "{name}: the set of output files differs (re-bless with DIMA_BLESS=1)"
        );
        for (rel, bytes) in &actual {
            if *bytes != expected[rel] {
                let first = bytes
                    .iter()
                    .zip(&expected[rel])
                    .position(|(a, b)| a != b)
                    .unwrap_or(bytes.len().min(expected[rel].len()));
                panic!(
                    "{name}/{} differs from the golden file at byte {first} (threads {threads}); \
                     re-bless with DIMA_BLESS=1 and review the diff",
                    rel.display()
                );
            }
        }
    }
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// A hand-made alignment covering NS/LS/ELS labels, ties, lower case, wrapped lines, interior
/// white space, an unknown character (permissive), ambiguity codes, metadata with a fill value
/// and a flagged conserved region; every output format.
#[test]
fn tiny() {
    let input = repo().join("tests/golden/tiny/input.fasta");
    let input = input.to_str().unwrap();
    case(
        "tiny",
        &["input.fasta"],
        &[
            args(&[
                "analyze",
                "-i",
                input,
                "-a",
                "protein",
                "-k",
                "3",
                "-s",
                "4",
                "--validation",
                "permissive",
                "--header-format",
                "Accession|Host|Year",
                "--metadata-fields",
                "Host|Year",
                "--hcs-min-incidence",
                "60",
                "-o",
                "results.json",
                "--tsv-dir",
                "tables",
                "--hcs-output",
                "hcs.json",
            ]),
            args(&[
                "view",
                "-i",
                "results.json",
                "--pretty",
                "-o",
                "results.pretty.json",
            ]),
            args(&["view", "-i", "results.json", "-o", "results.jsonl"]),
            args(&[
                "hcs",
                "-i",
                "results.json",
                "--hcs-min-incidence",
                "60",
                "--hcs-eligible-only",
                "-o",
                "hcs.eligible.tsv",
            ]),
        ],
    );
}

/// The paper's MERS-CoV spike protein dataset (Fig. 1C) with the default parameters.
#[test]
fn mers_spike_aa() {
    let input = repo().join("samples/mers_spike_aa.fasta");
    case(
        "mers_spike_aa",
        &[],
        &[args(&[
            "analyze",
            "-i",
            input.to_str().unwrap(),
            "--tsv-dir",
            "tables",
        ])],
    );
}
