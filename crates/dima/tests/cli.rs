//! End-to-end tests of the `dima` binary: exit codes (D49), standard input, outputs, the run
//! report (D103), broken pipes and signals (D65, D96).

#[cfg(unix)]
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tiny() -> PathBuf {
    repo().join("tests/golden/tiny/input.fasta")
}

fn mers() -> PathBuf {
    repo().join("samples/mers_spike_aa.fasta")
}

/// Options that make the tiny fixture analysable (it contains a '1').
const TINY: [&str; 6] = ["-a", "protein", "-k", "3", "--validation", "permissive"];

/// `dima` with a clean, deterministic environment.
fn dima() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_dima"));
    for var in [
        "DIMA_LOG",
        "DIMA_NO_PROGRESS",
        "DIMA_PROGRESS",
        "RAYON_NUM_THREADS",
        "OMP_NUM_THREADS",
        "SLURM_JOB_ID",
        "PBS_JOBID",
        "PE_HOSTFILE",
        "LSB_MCPU_HOSTS",
        "FORCE_COLOR",
        "CLICOLOR_FORCE",
    ] {
        c.env_remove(var);
    }
    c.env("NO_COLOR", "1").stdin(Stdio::null());
    c
}

fn run(args: &[&str], dir: &Path) -> Output {
    dima().args(args).current_dir(dir).output().unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn help_version_and_bare_command() {
    let d = tmp();
    let o = run(&["--version"], d.path());
    assert_eq!(code(&o), 0);
    assert_eq!(stdout(&o), format!("dima {}\n", env!("CARGO_PKG_VERSION")));
    let o = run(&["--help"], d.path());
    assert_eq!(code(&o), 0);
    assert!(stdout(&o).contains("doi:10.1093/bib/bbae607"));
    assert!(stdout(&o).contains("dima cite"));
    // Long help cites the sources of each method; -h stays short.
    let long = stdout(&run(&["analyze", "--help"], d.path()));
    assert!(long.contains("Ref: "), "{long}");
    assert!(
        long.contains("10.1111/2041-210X.12108"),
        "CWJ DOI in --estimator"
    );
    let short = stdout(&run(&["analyze", "-h"], d.path()));
    assert!(!short.contains("Ref: "), "{short}");
    // A bare `dima` is a usage error: help on stderr, nothing on stdout.
    let o = run(&[], d.path());
    assert_eq!(code(&o), 2);
    assert!(o.stdout.is_empty());
    assert!(stderr(&o).contains("Usage:"));
    // DiMA's own licence, then those of its components.
    let o = run(&["--licenses"], d.path());
    assert_eq!(code(&o), 0);
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.starts_with("MIT License"), "{}", &text[..80]);
    assert!(text.contains("Third-party"), "no notices");
    // Not with a subcommand, which it would silently skip; global options still precede one.
    let o = run(&["--licenses", "analyze", "-i", s(&tiny())], d.path());
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains("error: the argument '--licenses' cannot be used with 'analyze'"),
        "{}",
        stderr(&o)
    );
    assert!(o.stdout.is_empty());
    let o = run(&["-q", "cite"], d.path());
    assert_eq!(code(&o), 0, "{}", stderr(&o));
}

#[test]
fn exit_codes() {
    let d = tmp();
    let dir = d.path();
    std::fs::write(dir.join("empty.fa"), "").unwrap();
    std::fs::write(dir.join("reads.fq"), "@r\nACGT\n+\nIIII\n").unwrap();
    std::fs::write(dir.join("unequal.fa"), ">a\nACGTACGTAC\n>b\nACGTAC\n").unwrap();
    std::fs::write(dir.join("nul.fa"), ">a\nACG\0TACGTAC\n>b\nACGTACGTAC\n").unwrap();
    std::fs::write(dir.join("notes.json"), "{\"a\": 1}\n").unwrap();
    std::fs::write(dir.join("headers.fa"), ">a\n>b\n").unwrap();
    std::fs::write(dir.join("few.fa"), ">a\nACG\n").unwrap();
    // The xz magic and stream flags: xz is refused (D116).
    std::fs::write(dir.join("a.fasta.xz"), b"\xFD7zXZ\x00\x00\x04").unwrap();
    let t = s(&tiny()).to_owned();
    let cases: Vec<(Vec<&str>, i32, &str)> = vec![
        // I/O (3)
        (
            vec!["analyze", "-i", "missing.fasta"],
            3,
            "cannot open 'missing.fasta'",
        ),
        (
            vec![
                "analyze",
                "-i",
                &t,
                "-a",
                "protein",
                "-k",
                "3",
                "--validation",
                "permissive",
                "-o",
                "no/such/dir/x.json",
            ],
            3,
            "cannot create",
        ),
        // Usage (2)
        (vec!["analyze", "-i", "."], 2, "is a directory"),
        (vec!["analyze", "-i", &t, "-k", "0"], 2, "kmer-length"),
        (
            vec![
                "analyze",
                "-i",
                &t,
                "-a",
                "protein",
                "--validation",
                "permissive",
                "-k",
                "100",
            ],
            2,
            "longer than",
        ),
        (vec!["analyze", "-i", &t, "-o", "out.tsv"], 2, "--table"),
        (
            vec!["analyze", "-i", &t, "-o", "out.json", "-O", "tsv"],
            2,
            "does not match the extension",
        ),
        (
            vec!["analyze", "-i", &t, "-o", "out.dat"],
            2,
            "cannot tell the output format",
        ),
        (
            vec!["analyze", "-i", &t, "--hcs-min-incidence", "50"],
            2,
            "50",
        ),
        (vec!["analyze", "-i", &t, "--header-fillna", ""], 2, "fill"),
        (
            vec!["analyze", "-i", &t, "--metadata-fields", "Host"],
            2,
            "--header-format",
        ),
        (vec!["analyze", "-i", &t, "--header-format", "A|A"], 2, "A"),
        (
            vec![
                "analyze",
                "-i",
                &t,
                "-o",
                "a.json",
                "--hcs-output",
                "a.json",
            ],
            2,
            "written twice",
        ),
        (
            vec!["analyze", "-i", &t, "-O", "json", "-o", &t],
            2,
            "is the input file",
        ),
        (
            vec!["analyze", "-i", &t, "--frobnicate"],
            2,
            "unexpected argument",
        ),
        (
            vec!["view", "-i", "notes.json", "-o", "x.tsv"],
            2,
            "--table",
        ),
        (vec!["view", "-i", "."], 2, "is a directory"),
        // Invalid input data (4)
        (
            vec!["analyze", "-i", &t, "-a", "protein", "-k", "3"],
            4,
            "record 7 (line 15, column 3, byte 227): character '1' (0x31)",
        ),
        (vec!["analyze", "-i", "empty.fa"], 4, "no FASTA records"),
        (
            vec!["analyze", "-i", "a.fasta.xz"],
            4,
            "the input is xz (.xz), which DiMA cannot read",
        ),
        (vec!["analyze", "-i", "reads.fq"], 4, "FASTQ"),
        // An empty first record is reported as such, even without -a.
        (
            vec!["analyze", "-i", "headers.fa"],
            4,
            "headers.fa: record 1 (line 1): the sequence is empty",
        ),
        // Alphabet errors name the input.
        (
            vec!["analyze", "-i", "few.fa"],
            4,
            "few.fa: only 3 residue letters",
        ),
        (
            vec!["analyze", "-i", "unequal.fa", "-a", "dna", "-k", "3"],
            4,
            "record 2 (line 3)",
        ),
        (
            vec!["analyze", "-i", "nul.fa", "-a", "dna", "-k", "3"],
            4,
            "NUL byte",
        ),
        (vec!["view", "-i", &t], 4, "not a DiMA results file"),
        (
            vec!["view", "-i", "notes.json"],
            4,
            "not a DiMA results file",
        ),
    ];
    for (args, expected, message) in cases {
        let o = run(&args, dir);
        assert_eq!(code(&o), expected, "{args:?}\n{}", stderr(&o));
        assert!(
            stderr(&o).contains(message),
            "{args:?}: expected {message:?} in\n{}",
            stderr(&o)
        );
        assert!(stderr(&o).starts_with("error"), "{args:?}: {}", stderr(&o));
        assert!(
            o.stdout.is_empty(),
            "{args:?}: nothing on stdout after an error"
        );
    }
    // Nothing was created by the failed runs except the fixtures.
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "a.fasta.xz",
            "empty.fa",
            "few.fa",
            "headers.fa",
            "notes.json",
            "nul.fa",
            "reads.fq",
            "unequal.fa"
        ]
    );
    // Environment: a malformed RAYON_NUM_THREADS is a usage error.
    let o = dima()
        .args(["analyze", "-i", &t])
        .args(TINY)
        .env("RAYON_NUM_THREADS", "zero")
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("RAYON_NUM_THREADS"));
}

#[test]
fn standard_input_gives_the_same_results() {
    let d = tmp();
    let table = |stdin: Option<&Path>| -> Vec<u8> {
        let mut c = dima();
        c.args([
            "analyze",
            "-O",
            "tsv",
            "--table",
            "positions",
            "-o",
            "-",
            "-q",
        ])
        .args(TINY);
        match stdin {
            Some(p) => c.stdin(std::fs::File::open(p).unwrap()),
            None => c.args(["-i", s(&tiny())]),
        };
        let o = c.current_dir(d.path()).output().unwrap();
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        o.stdout
    };
    assert_eq!(table(None), table(Some(&tiny())));
    // `-i -` is standard input too; provenance names it '-'.
    let o = dima()
        .args(["analyze", "-i", "-", "-q"])
        .args(TINY)
        .stdin(std::fs::File::open(tiny()).unwrap())
        .current_dir(d.path())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0);
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["run"]["input"]["name"], "-");
    assert_eq!(r["query_name"], "Unknown Protein");
}

#[test]
fn outputs_formats_and_compression() {
    let d = tmp();
    let dir = d.path();
    let t = tiny();
    // Default: compact JSON with a final newline on stdout (not a terminal here).
    let o = dima()
        .args(["analyze", "-i", s(&t), "-q"])
        .args(TINY)
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(
        o.stderr.is_empty(),
        "-q prints nothing on success: {}",
        stderr(&o)
    );
    let text = stdout(&o);
    assert_eq!(text.matches('\n').count(), 1);
    assert!(text.ends_with("}\n"));
    let compact = o.stdout;
    let r: Value = serde_json::from_slice(&compact).unwrap();
    assert_eq!(r["sequence_count"], 8);
    assert_eq!(r["run"]["input"]["name"], "input.fasta");
    // The same results through files, compressed or not, and back through `view`.
    for name in ["r.json", "r.json.gz", "r.JSON.ZST"] {
        let o = dima()
            .args(["analyze", "-i", s(&t), "-q", "-o", name])
            .args(TINY)
            .current_dir(dir)
            .output()
            .unwrap();
        assert_eq!(code(&o), 0, "{name}: {}", stderr(&o));
        let o = run(&["view", "-i", name, "-o", "-", "-q"], dir);
        assert_eq!(code(&o), 0, "{name}: {}", stderr(&o));
        assert_eq!(o.stdout, compact, "{name}");
    }
    assert_eq!(&std::fs::read(dir.join("r.json")).unwrap(), &compact);
    // --pretty indents.
    let o = dima()
        .args(["analyze", "-i", s(&t), "-q", "--pretty"])
        .args(TINY)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(stdout(&o).lines().count() > 100);
    let pretty: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(pretty, r);
    // JSONL: one position per line, and a note on stderr.
    let o = run(&["view", "-i", "r.json", "-O", "jsonl", "-o", "-"], dir);
    assert_eq!(code(&o), 0);
    let lines: Vec<&str> = std::str::from_utf8(&o.stdout).unwrap().lines().collect();
    assert_eq!(lines.len(), 18);
    assert!(stderr(&o).contains("JSONL holds one position per line"));
    // --tsv-dir writes the four tables, named after the query.
    let o = run(&["view", "-i", "r.json", "--tsv-dir", "tables", "-q"], dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    for table in ["positions", "variants", "metadata", "hcs"] {
        let p = dir.join(format!("tables/input.{table}.tsv"));
        assert!(p.is_file(), "{}", p.display());
    }
    // `view -O tsv --table` equals the --tsv-dir table.
    let o = run(
        &[
            "view", "-i", "r.json", "-O", "tsv", "--table", "variants", "-o", "-",
        ],
        dir,
    );
    assert_eq!(
        o.stdout,
        std::fs::read(dir.join("tables/input.variants.tsv")).unwrap()
    );
    // `dima hcs` equals --hcs-output for the same threshold.
    let o = dima()
        .args([
            "analyze",
            "-i",
            s(&t),
            "-q",
            "-o",
            "h.json",
            "--hcs-output",
            "a.hcs.json",
            "--hcs-min-incidence",
            "60",
        ])
        .args(TINY)
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(
        &[
            "hcs",
            "-i",
            "h.json",
            "--hcs-min-incidence",
            "60",
            "-o",
            "b.hcs.json",
            "-q",
        ],
        dir,
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(
        std::fs::read(dir.join("a.hcs.json")).unwrap(),
        std::fs::read(dir.join("b.hcs.json")).unwrap()
    );
    let h = json(&dir.join("a.hcs.json"));
    assert!(h["flagged_region_count"].as_u64().unwrap() >= 1, "{h}");
    // No temporary files are left behind.
    for e in std::fs::read_dir(dir).unwrap() {
        let name = e.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            !name.starts_with(".tmp") && !name.contains(".dima-"),
            "{name}"
        );
    }
}

#[cfg(unix)]
#[test]
fn special_files_are_written_directly() {
    let d = tmp();
    let o = dima()
        .args([
            "analyze",
            "-i",
            s(&tiny()),
            "-q",
            "-o",
            "/dev/stdout",
            "-O",
            "json",
        ])
        .args(TINY)
        .current_dir(d.path())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["sequence_count"], 8);
    // `-o /dev/stdout >> log` appends to the log; it never replaces it.
    let log = d.path().join("log.txt");
    std::fs::write(&log, "EXISTING LOG LINE\n").unwrap();
    let appending = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    let o = dima()
        .args([
            "analyze",
            "-i",
            s(&tiny()),
            "-q",
            "-o",
            "/dev/stdout",
            "-O",
            "json",
        ])
        .args(TINY)
        .stdout(appending)
        .current_dir(d.path())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.starts_with("EXISTING LOG LINE\n{"), "{text:.80}");
}

/// D104: every message dima writes itself is ASCII, so it reads the same in any locale, code
/// page, pager or screen reader (text echoed from the input is the only exception).
#[test]
fn messages_are_ascii() {
    let d = tmp();
    let t = s(&tiny()).to_owned();
    let frozen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/results-schema-1.0.0.json.gz")
        .to_string_lossy()
        .into_owned();
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "analyze",
            "-i",
            &t,
            "-o",
            "r.json",
            "-a",
            "protein",
            "-k",
            "3",
            "--validation",
            "report",
            "--hcs-output",
            "h.json",
            "--hcs-min-incidence",
            "60",
            "-vv",
        ],
        vec![
            "analyze",
            "-i",
            &t,
            "-o",
            "r.json",
            "-a",
            "dna",
            "-k",
            "3",
            "--validation",
            "permissive",
        ],
        vec!["analyze", "-i", &t, "-o", "r.json", "-k", "3"],
        vec!["analyze", "-i", &t, "-o", "x.tsv", "-a", "protein"],
        vec![
            "analyze",
            "-i",
            &t,
            "-o",
            "s.json",
            "-a",
            "protein",
            "-k",
            "3",
            "--validation",
            "permissive",
            "--similarity-weighted-entropy",
        ],
        vec![
            "analyze",
            "-i",
            &t,
            "-o",
            "v.tsv",
            "--table",
            "variants",
            "-a",
            "protein",
            "--validation",
            "permissive",
            "--similarity-weighted-entropy",
        ],
        vec![
            "analyze",
            "-i",
            &t,
            "--hcs-min-incidence",
            "40",
            "-a",
            "protein",
        ],
        vec!["analyze", "-i", "missing.fasta"],
        vec!["view", "-i", "r.json", "-O", "tsv", "--tsv-dir", "t", "-v"],
        vec![
            "hcs",
            "-i",
            "r.json",
            "--hcs-min-incidence",
            "60",
            "-o",
            "h2.json",
            "-v",
        ],
        vec!["view", "-i", &t],
        vec!["view", "-i", &frozen, "-o", "old.json"],
        vec!["analyze", "--help"],
        vec!["--help"],
        vec!["cite", "--from", "r.json"],
    ];
    for args in cases {
        let o = run(&args, d.path());
        let err = stderr(&o);
        assert!(err.is_ascii(), "{args:?}: {err}");
        if args.contains(&"--help") {
            // Help is ASCII words and formulas (D107); only authors' names in the references
            // keep their letters (Bock is Böck).
            let out = stdout(&o);
            let symbol = out.chars().find(|c| !c.is_ascii() && !c.is_alphabetic());
            assert_eq!(symbol, None, "{args:?}: {out}");
            // The similarity-weighted entropy help, references aside, is plain ASCII.
            if let Some(start) = out.find("--similarity-weighted-entropy\n") {
                let text = &out[start..];
                let text = &text[..text.find("Ref:").unwrap()];
                assert!(text.is_ascii(), "{text}");
            }
        }
    }
}

/// D109: `--progress plain` appends phase lines anywhere; `off`, `-q` and the old
/// `--no-progress` print none; `auto` off a terminal prints them only with `-v`.
#[test]
fn progress_options() {
    let d = tmp();
    let t = s(&tiny()).to_owned();
    let phases = |args: &[&str], env: &[(&str, &str)]| {
        let mut c = dima();
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c
            .args(["analyze", "-i", &t, "-o", "r.json"])
            .args(TINY)
            .args(args)
            .current_dir(d.path())
            .output()
            .unwrap();
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        let err = stderr(&o);
        let lines = [
            "] opening",
            "] counting k-mers",
            "] computing statistics",
            "] writing",
        ];
        let found = lines.iter().filter(|l| err.contains(*l)).count();
        assert!(
            found == 0 || found == lines.len(),
            "{args:?} {env:?}: {err}"
        );
        found == lines.len()
    };
    assert!(!phases(&[], &[]), "auto, not a terminal");
    assert!(phases(&["-v"], &[]), "auto with -v");
    assert!(phases(&["--progress", "plain"], &[]));
    assert!(phases(&[], &[("DIMA_PROGRESS", "plain")]));
    assert!(!phases(&["--progress", "off", "-v"], &[]));
    assert!(!phases(&["--no-progress", "-v"], &[]), "the old flag");
    assert!(!phases(&["-v"], &[("DIMA_NO_PROGRESS", "1")]));
    assert!(!phases(&["-q", "--progress", "plain"], &[]));
    // The old flag is accepted but no longer advertised.
    let help = stdout(&run(&["analyze", "--help"], d.path()));
    assert!(help.contains("--progress <WHEN>") && !help.contains("--no-progress"));
}

/// D98: `--color`, then `NO_COLOR`, then `FORCE_COLOR`/`CLICOLOR_FORCE`, then the terminal.
#[test]
fn colour_follows_the_environment() {
    let d = tmp();
    let coloured = |env: &[(&str, &str)], extra: &[&str]| {
        let mut c = dima();
        c.env_remove("NO_COLOR");
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c
            .args(["analyze", "-i", "missing.fasta"])
            .args(extra)
            .current_dir(d.path())
            .output()
            .unwrap();
        assert_eq!(code(&o), 3, "{}", stderr(&o));
        stderr(&o).contains('\u{1b}')
    };
    assert!(!coloured(&[], &[]), "not a terminal");
    assert!(coloured(&[("FORCE_COLOR", "1")], &[]));
    assert!(
        coloured(&[("FORCE_COLOR", "0")], &[]),
        "any non-empty value"
    );
    assert!(!coloured(&[("FORCE_COLOR", "")], &[]));
    assert!(coloured(&[("CLICOLOR_FORCE", "1")], &[]));
    assert!(!coloured(&[("FORCE_COLOR", "1"), ("NO_COLOR", "1")], &[]));
    assert!(!coloured(&[("FORCE_COLOR", "1")], &["--color", "never"]));
    assert!(coloured(&[], &["--color", "always"]));
}

#[test]
fn messages_respect_verbosity() {
    let d = tmp();
    let base = |extra: &[&str]| {
        let o = dima()
            .args([
                "analyze",
                "-i",
                s(&tiny()),
                "-o",
                "r.json",
                "-a",
                "protein",
                "-k",
                "3",
            ])
            .args(if extra.contains(&"--validation") {
                &[][..]
            } else {
                &["--validation", "permissive"][..]
            })
            .args(extra)
            .current_dir(d.path())
            .output()
            .unwrap();
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        stderr(&o)
    };
    let normal = base(&[]);
    assert!(
        normal.contains("warning[unknown_characters_excluded]"),
        "{normal}"
    );
    assert!(normal.contains("warning[interior_whitespace]"), "{normal}");
    assert!(normal.contains("analysed 8 sequences"), "{normal}");
    assert!(
        !normal.contains("Cite:"),
        "the citation line is for terminals only"
    );
    assert!(base(&["-q"]).is_empty());
    let verbose = base(&["-v"]);
    assert!(verbose.contains("threads: "), "{verbose}");
    assert!(verbose.contains("note[lowercase_folded]"), "{verbose}");
    // --validation report prints the character table.
    let report = base(&["--validation", "report"]);
    assert!(report.contains("characters in sequence lines"), "{report}");
    assert!(report.contains("0x31"), "{report}");
}

#[test]
fn completions_and_citations() {
    let d = tmp();
    for shell in ["bash", "elvish", "fish", "powershell", "zsh"] {
        let o = run(&["completions", shell], d.path());
        assert_eq!(code(&o), 0, "{shell}");
        assert!(stdout(&o).contains("dima"), "{shell}");
    }
    let text = stdout(&run(&["cite"], d.path()));
    assert!(text.contains("Tharanga"), "{text}");
    assert!(text.contains("10.1093/bib/bbae607"), "{text}");
    for (format, marker) in [
        ("bibtex", "@article{"),
        ("ris", "TY  - JOUR"),
        ("cff", "cff-version: 1.2.0"),
    ] {
        let o = run(&["cite", "--format", format], d.path());
        assert_eq!(code(&o), 0, "{format}");
        assert!(stdout(&o).contains(marker), "{format}: {}", stdout(&o));
    }
    let o = dima()
        .args([
            "analyze",
            "-i",
            s(&tiny()),
            "-q",
            "-o",
            "r.json",
            "--estimator",
            "chao-wang-jost",
            "--hcs-output",
            "h.json",
            "--hcs-min-incidence",
            "75",
        ])
        .args(TINY)
        .current_dir(d.path())
        .output()
        .unwrap();
    assert_eq!(code(&o), 0);
    let o = run(&["cite", "--from", "r.json"], d.path());
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let text = stdout(&o);
    assert!(text.starts_with("Methods:"), "{text}");
    assert!(
        text.contains("Chao"),
        "the run's estimator is cited: {text}"
    );
    assert!(!text.contains("Clopper"), "{text}");
    // With the conserved sequences, in either order: their method and references are added.
    for from in [["r.json", "h.json"], ["h.json", "r.json"]] {
        let o = run(&["cite", "--from", from[0], "--from", from[1]], d.path());
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        let text = stdout(&o);
        assert!(
            text.contains("Chao") && text.contains("at least 75%"),
            "{text}"
        );
        assert!(text.contains("Clopper CJ, Pearson ES"), "{text}");
    }
    // Two results files are one too many.
    let o = run(&["cite", "--from", "r.json", "--from", "r.json"], d.path());
    assert_eq!(code(&o), 2, "{}", stderr(&o));
}

/// Spawns `dima`, closes its standard output at once, and expects a quiet success (D65).
fn closed_stdout(args: &[&str], dir: &Path) {
    let mut child = dima()
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let o = child.wait_with_output().unwrap();
    assert_eq!(code(&o), 0, "{args:?}: {}", stderr(&o));
    assert!(!stderr(&o).contains("error"), "{args:?}: {}", stderr(&o));
}

#[test]
fn broken_pipes_end_quietly() {
    let d = tmp();
    closed_stdout(&["completions", "bash"], d.path());
    closed_stdout(&["cite", "--format", "bibtex"], d.path());
    closed_stdout(&["analyze", "-i", s(&mers()), "-q"], d.path());
    // Like `dima … | head -n 1`: the reader goes away after the first line.
    let mut child = dima()
        .args(["analyze", "-i", s(&mers()), "-O", "jsonl", "-o", "-", "-q"])
        .current_dir(d.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut first = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut first)
        .unwrap();
    assert!(first.starts_with("{\"position\":1,"), "{first}");
    let o = child.wait_with_output().unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(o.stderr.is_empty(), "{}", stderr(&o));
}

#[test]
fn a_closed_pipe_never_costs_a_file_output() {
    // Standard output is written last: the file outputs are complete when the reader goes away,
    // and the run report agrees with the exit status.
    let d = tmp();
    let mut child = dima()
        .args([
            "analyze",
            "-i",
            s(&mers()),
            "-q",
            "-o",
            "-",
            "--hcs-output",
            "h.json",
        ])
        .args(["--tsv-dir", "tables", "--run-report", "run.json"])
        .current_dir(d.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let o = child.wait_with_output().unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(json(&d.path().join("h.json"))["region_count"].is_u64());
    assert_eq!(
        std::fs::read_dir(d.path().join("tables")).unwrap().count(),
        4
    );
    let report = json(&d.path().join("run.json"));
    assert_eq!(report["status"], "completed");
    assert_eq!(report["exit_code"], 0);
}

#[test]
fn destinations_are_checked_before_the_analysis() {
    let d = tmp();
    let dir = d.path();
    let t = s(&tiny()).to_owned();
    let mut args: Vec<&str> = vec!["analyze", "-i", &t];
    args.extend(TINY);
    // A missing directory: the run stops before reading the input (no counting phase).
    let mut missing = args.clone();
    missing.extend(["-o", "missing/x.json", "--run-report", "run.json"]);
    let o = run(&missing, dir);
    assert_eq!(code(&o), 3, "{}", stderr(&o));
    assert!(
        stderr(&o).contains("cannot create 'missing/x.json'"),
        "{}",
        stderr(&o)
    );
    let report = json(&dir.join("run.json"));
    assert_eq!(report["phases"], serde_json::json!([]), "{report}");
    // A directory given as the output file: a usage error, with or without -O.
    std::fs::create_dir(dir.join("adir")).unwrap();
    for extra in [&["-o", "adir"][..], &["-o", "adir", "-O", "json"][..]] {
        let mut folder = args.clone();
        folder.extend(extra);
        let o = run(&folder, dir);
        assert_eq!(code(&o), 2, "{}", stderr(&o));
        assert!(
            stderr(&o).contains("'adir' is a folder, not a file"),
            "{}",
            stderr(&o)
        );
    }
    // A long file name (its temporary file must still fit the file system's limit).
    let long = format!("{}.json", "r".repeat(240));
    let mut long_args = args.clone();
    long_args.extend(["-o", &long, "-q"]);
    let o = run(&long_args, dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(dir.join(&long).is_file());
    // A failed run removes the --tsv-dir it created.
    std::fs::write(dir.join("empty.fa"), "").unwrap();
    let o = run(
        &["analyze", "-i", "empty.fa", "--tsv-dir", "newtables", "-q"],
        dir,
    );
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(!dir.join("newtables").exists());
}

#[test]
fn outputs_reaching_one_file_are_refused() {
    let d = tmp();
    let dir = d.path();
    let t = s(&tiny()).to_owned();
    let mut args: Vec<&str> = vec!["analyze", "-i", &t];
    args.extend(TINY);
    // The same file through a directory that does not exist yet.
    let mut through_new = args.clone();
    through_new.extend([
        "--tsv-dir",
        "newdir",
        "-o",
        "./newdir/input.variants.tsv",
        "--table",
        "positions",
    ]);
    let o = run(&through_new, dir);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("written twice"), "{}", stderr(&o));
    // The same file through a dangling symbolic link.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("target.json", dir.join("link.json")).unwrap();
        let mut linked = args.clone();
        linked.extend(["-o", "link.json", "--hcs-output", "target.json"]);
        let o = run(&linked, dir);
        assert_eq!(code(&o), 2, "{}", stderr(&o));
        assert!(!dir.join("target.json").exists());
    }
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(repo().join("schemas/run-report.schema.json")).unwrap(),
    )
    .unwrap();
    jsonschema::meta::validate(&schema).unwrap();
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap()
}

fn check_report(v: &jsonschema::Validator, report: &Value, dir: &Path) {
    let errors: Vec<String> = v
        .iter_errors(report)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{errors:#?}\n{report:#}");
    // No directories: neither the working directory nor the repository appears anywhere.
    let text = report.to_string();
    for p in [
        dir.to_path_buf(),
        repo().canonicalize().unwrap(),
        dir.canonicalize().unwrap(),
    ] {
        assert!(!text.contains(s(&p)), "{} in {text}", p.display());
    }
    assert!(
        !text.contains("/home/") && !text.contains("/tmp/"),
        "{text}"
    );
}

#[test]
fn run_report() {
    let d = tmp();
    let dir = d.path();
    let v = validator();
    let input = dir.join("input copy.fasta");
    std::fs::copy(tiny(), &input).unwrap();
    // Completed: every input and output with its size and SHA-256.
    let o = dima()
        .args(["analyze", "-i", s(&input), "-q", "-t", "2"])
        .args(TINY)
        .args([
            "-o",
            s(&dir.join("r.json.gz")),
            "--tsv-dir",
            s(&dir.join("tables")),
            "--hcs-output",
            "h.json",
        ])
        .args(["--run-report", s(&dir.join("run.json"))])
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let r = json(&dir.join("run.json"));
    check_report(&v, &r, dir);
    assert_eq!(r["status"], "completed");
    assert_eq!(r["exit_code"], 0);
    assert_eq!(r["error"], Value::Null);
    assert_eq!(
        r["threads"],
        serde_json::json!({"count": 2, "started": 2, "source": "--threads"})
    );
    assert_eq!(r["work"]["records"], 8);
    assert_eq!(r["work"]["positions"], 18);
    let command: Vec<&str> = r["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(
        &command[..4],
        ["dima", "analyze", "--input", "input copy.fasta"]
    );
    assert!(
        command.contains(&"r.json.gz")
            && command.contains(&"tables")
            && command.contains(&"run.json")
    );
    let phases: Vec<&str> = r["phases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(phases, ["opening", "counting", "finalising", "writing"]);
    let files = r["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .chain(r["outputs"].as_array().unwrap());
    let mut roles = Vec::new();
    for f in files {
        let name = f["name"].as_str().unwrap();
        let path = [dir.join(name), dir.join("tables").join(name)]
            .into_iter()
            .find(|p| p.is_file())
            .unwrap();
        let path = if name == "input copy.fasta" {
            input.clone()
        } else {
            path
        };
        assert_eq!(
            f["size_bytes"],
            std::fs::metadata(&path).unwrap().len(),
            "{name}"
        );
        assert_eq!(
            f["sha256"],
            dima_core::input::sha256_file(&path).unwrap(),
            "{name}"
        );
        roles.push(f["role"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        roles,
        [
            "alignment",
            "results",
            "positions_table",
            "variants_table",
            "metadata_table",
            "hcs_table",
            "hcs"
        ]
    );
    // Failed: written with the error, its directories removed.
    let o = dima()
        .args([
            "analyze",
            "-i",
            s(&dir.join("missing.fasta")),
            "--run-report",
            "failed.json",
        ])
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 3);
    let r = json(&dir.join("failed.json"));
    check_report(&v, &r, dir);
    assert_eq!(r["status"], "failed");
    assert_eq!(r["exit_code"], 3);
    assert_eq!(r["threads"]["source"], "available_parallelism");
    assert_eq!(
        r["threads"]["started"],
        Value::Null,
        "stopped before the workers"
    );
    let error = r["error"].as_str().unwrap();
    assert!(error.starts_with("cannot open 'missing.fasta'"), "{error}");
    assert!(r["inputs"].as_array().unwrap().is_empty());
    // Failed after the workers started: they are still reported (D110).
    std::fs::write(
        dir.join("unequal.fa"),
        ">a\nACGTACGTAC\n>b\nACGTACGTAC\n>c\nACGTAC\n",
    )
    .unwrap();
    let o = dima()
        .args([
            "analyze",
            "-i",
            "unequal.fa",
            "-a",
            "dna",
            "-k",
            "3",
            "-t",
            "2",
            "--run-report",
            "late.json",
        ])
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    let r = json(&dir.join("late.json"));
    assert_eq!(r["status"], "failed");
    assert_eq!(
        r["threads"],
        serde_json::json!({"count": 2, "started": 2, "source": "--threads"})
    );
    // A usage error before the thread count is known.
    let o = dima()
        .args([
            "analyze",
            "-i",
            s(&tiny()),
            "-o",
            "x.tsv",
            "--run-report",
            "usage.json",
        ])
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 2);
    let r = json(&dir.join("usage.json"));
    check_report(&v, &r, dir);
    assert_eq!(r["threads"], Value::Null);
    assert_eq!(r["exit_code"], 2);
}

#[test]
fn run_report_never_replaces_inputs_or_outputs() {
    let d = tmp();
    let dir = d.path();
    let input = dir.join("in.fasta");
    std::fs::copy(tiny(), &input).unwrap();
    let before = std::fs::read(&input).unwrap();
    // Even when the run fails before its own collision check (here: a usage error).
    for args in [
        vec![
            "analyze",
            "-i",
            "in.fasta",
            "-k",
            "0",
            "--run-report",
            "in.fasta",
        ],
        vec![
            "analyze",
            "-i",
            "in.fasta",
            "-o",
            "x.tsv",
            "--run-report",
            "./in.fasta",
        ],
        vec![
            "analyze",
            "-i",
            "in.fasta",
            "-o",
            "r.json",
            "--run-report",
            "r.json",
        ],
    ] {
        let o = run(&args, dir);
        assert_eq!(code(&o), 2, "{args:?}: {}", stderr(&o));
        assert_eq!(std::fs::read(&input).unwrap(), before, "{args:?}");
    }
    assert!(!dir.join("r.json").exists());
}

/// Reads standard error on a thread; lines arrive through the channel.
fn stderr_lines(child: &mut Child) -> mpsc::Receiver<String> {
    let err = child.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(err).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// Waits until a line starting with `prefix` arrives; returns everything read.
fn wait_for(rx: &mpsc::Receiver<String>, prefix: &str) -> Vec<String> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(line) = rx.recv_timeout(Duration::from_millis(100)) {
            let found = line.starts_with(prefix);
            seen.push(line);
            if found {
                return seen;
            }
        }
    }
    panic!("no line starting with {prefix:?}; got {seen:?}");
}

fn wait_exit(child: &mut Child, limit: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "dima did not exit within {limit:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn kill(child: &Child, signal: &str) {
    let status = Command::new("kill")
        .args(["-s", signal, &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

/// `dima analyze -v` reading an open pipe that never ends: it waits for more input.
#[cfg(unix)]
fn waiting_on_stdin(
    dir: &Path,
    wrapper: Option<&str>,
) -> (Child, std::process::ChildStdin, mpsc::Receiver<String>) {
    let mut c = match wrapper {
        Some(script) => {
            let mut c = Command::new("sh");
            c.args(["-c", script, env!("CARGO_BIN_EXE_dima")]);
            c.env("NO_COLOR", "1")
                .env_remove("RAYON_NUM_THREADS")
                .env_remove("DIMA_LOG");
            c
        }
        None => dima(),
    };
    let mut child = c
        .args(["analyze", "-v", "-o", "r.json", "--run-report", "run.json"])
        .args(TINY)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut data = Vec::new();
    std::fs::File::open(tiny())
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    stdin.write_all(&data).unwrap();
    stdin.flush().unwrap();
    let rx = stderr_lines(&mut child);
    // "threads:" is printed after the signal handlers are installed.
    wait_for(&rx, "threads: ");
    (child, stdin, rx)
}

#[cfg(unix)]
#[test]
fn signals_cancel_cleanly() {
    use std::os::unix::process::ExitStatusExt;
    for (signal, number) in [
        ("TERM", libc::SIGTERM),
        ("INT", libc::SIGINT),
        ("HUP", libc::SIGHUP),
        ("USR1", libc::SIGUSR1),
        ("XCPU", libc::SIGXCPU),
    ] {
        let exit_code = 128 + number;
        let d = tmp();
        let (mut child, stdin, rx) = waiting_on_stdin(d.path(), None);
        std::thread::sleep(Duration::from_millis(100));
        let sent = Instant::now();
        kill(&child, signal);
        // Cancelled promptly (not by the 3 s watchdog), and terminated by the same signal.
        let status = wait_exit(&mut child, Duration::from_secs(10));
        assert!(
            sent.elapsed() < Duration::from_secs(2),
            "{signal}: {:?}",
            sent.elapsed()
        );
        assert_eq!(status.signal(), Some(number), "{signal}: {status:?}");
        let lines: Vec<String> = rx.try_iter().collect();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("cancelled; no results were written")),
            "{signal}: {lines:?}"
        );
        // The notice names the signal; only Ctrl-C (SIGINT) is a key to press again.
        let notice = if signal == "INT" {
            "cancelling... (press Ctrl-C again to stop immediately)".to_owned()
        } else {
            format!("cancelling on SIG{signal}... (a second signal stops immediately)")
        };
        assert!(
            lines.iter().any(|l| l == &notice),
            "{signal}: {notice:?} in {lines:?}"
        );
        drop(stdin);
        // No results and no temporary files; the run report says what happened.
        let names: Vec<String> = std::fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["run.json"], "{signal}");
        let r = json(&d.path().join("run.json"));
        assert_eq!(r["status"], "cancelled", "{signal}");
        assert_eq!(r["exit_code"], exit_code, "{signal}");
        assert_eq!(r["error"], Value::Null);
        // The phases it reached (it was waiting for the rest of the first batch).
        let phases: Vec<&str> = r["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(phases.first(), Some(&"opening"), "{signal}: {phases:?}");
    }
}

#[cfg(unix)]
#[test]
fn ignored_hangup_stays_ignored() {
    use std::os::unix::process::ExitStatusExt;
    // `nohup`-style: SIGHUP ignored before dima starts must stay ignored (D96).
    let d = tmp();
    let (mut child, stdin, _rx) =
        waiting_on_stdin(d.path(), Some("trap '' HUP; exec \"$0\" \"$@\""));
    kill(&child, "HUP");
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "SIGHUP was ignored at startup"
    );
    kill(&child, "TERM");
    let status = wait_exit(&mut child, Duration::from_secs(10));
    assert_eq!(status.signal(), Some(15));
    drop(stdin);
}

#[cfg(unix)]
#[test]
fn a_second_signal_forces_exit() {
    use std::os::unix::process::ExitStatusExt;
    // Opening a FIFO without a writer blocks the main thread: the first signal can only cancel
    // at the next check, so a second one must end the process at once.
    let d = tmp();
    let fifo = d.path().join("in.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let spawn = || {
        let mut child = dima()
            .args(["analyze", "-v", "-i", "in.fifo", "-o", "r.json"])
            .current_dir(d.path())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let rx = stderr_lines(&mut child);
        wait_for(&rx, "threads: ");
        std::thread::sleep(Duration::from_millis(100));
        child
    };
    let mut child = spawn();
    kill(&child, "INT");
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        child.try_wait().unwrap().is_none(),
        "still blocked after the first signal"
    );
    let sent = Instant::now();
    kill(&child, "INT");
    let status = wait_exit(&mut child, Duration::from_secs(10));
    assert!(
        sent.elapsed() < Duration::from_secs(1),
        "{:?}",
        sent.elapsed()
    );
    assert_eq!(status.signal(), Some(2));
    // Without a second signal, the grace period ends the process.
    let mut child = spawn();
    let sent = Instant::now();
    kill(&child, "TERM");
    let status = wait_exit(&mut child, Duration::from_secs(15));
    assert!(
        sent.elapsed() >= Duration::from_secs(2),
        "{:?}",
        sent.elapsed()
    );
    assert_eq!(status.signal(), Some(15));
    assert!(!d.path().join("r.json").exists());
}

/// D96 on Windows: Ctrl-Break (delivered to the process group) cancels the run, removes its
/// outputs and exits with 130. Skipped when the test process has no console to deliver it.
#[cfg(windows)]
#[test]
fn ctrl_break_cancels() {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let d = tmp();
    let mut child = dima()
        .args(["analyze", "-v", "-o", "r.json"])
        .args(TINY)
        .current_dir(d.path())
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&std::fs::read(tiny()).unwrap()).unwrap();
    stdin.flush().unwrap();
    let rx = stderr_lines(&mut child);
    wait_for(&rx, "threads: ");
    // SAFETY: sends a console control event to the child's own process group; no memory is
    // passed and failure is reported through the return value.
    let sent = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) };
    if sent == 0 {
        eprintln!("skipped: this test process has no console to deliver Ctrl-Break");
        let _ = child.kill();
        let _ = child.wait();
        return;
    }
    let status = wait_exit(&mut child, Duration::from_secs(10));
    assert_eq!(status.code(), Some(130), "{status:?}");
    drop(stdin);
    assert!(!d.path().join("r.json").exists());
}

/// `dima view` tells how a file of another schema version is written: an older 1.x file gets a
/// note, only when the outputs carry the values its schema did not record; a newer or
/// unrecognised version a `newer_schema` warning.
#[test]
fn view_reports_older_and_newer_schemas() {
    let tmp = tmp();
    let dir = tmp.path();
    let frozen =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/results-schema-1.0.0.json.gz");
    let o = run(&["view", "-i", s(&frozen), "-o", "new.json"], dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let e = stderr(&o);
    assert!(e.is_ascii(), "{e}");
    assert!(
        e.contains(
            "note: results file written with schema 1.0.0; values that schema did not record \
             are written as null (NA in the positions table), and options it did not have as off"
        ),
        "{e}"
    );
    assert!(!e.contains("newer_schema"), "{e}");
    let written = json(&dir.join("new.json"));
    assert_eq!(written["schema_version"], dima_core::model::SCHEMA_VERSION);
    assert_eq!(
        written["run"]["parameters"]["similarity_weighted_entropy"]["value"],
        false
    );
    assert!(written["results"][0]["similarity_weighted_entropy"].is_null());
    // The variants table carries none of those values: no note.
    let o = run(
        &[
            "view",
            "-i",
            s(&frozen),
            "-O",
            "tsv",
            "--table",
            "variants",
            "-o",
            "-",
        ],
        dir,
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!stderr(&o).contains("note:"), "{}", stderr(&o));
    // The current version: nothing to say. Newer or unrecognised versions: a warning.
    for (version, expected) in [
        (dima_core::model::SCHEMA_VERSION, None),
        ("1.9.0", Some("schema 1.9.0;")),
        ("1.1.0-dev", Some("schema \"1.1.0-dev\";")),
        // An unrecognised version is quoted, so a stray space shows.
        ("1.1.0 ", Some("schema \"1.1.0 \";")),
    ] {
        let mut v = json(&dir.join("new.json"));
        v["schema_version"] = version.into();
        std::fs::write(dir.join("v.json"), serde_json::to_vec(&v).unwrap()).unwrap();
        let o = run(&["view", "-i", "v.json", "-o", "-"], dir);
        assert_eq!(code(&o), 0, "{version}: {}", stderr(&o));
        let e = stderr(&o);
        match expected {
            Some(shown) => assert!(
                e.contains("warning[newer_schema]") && e.contains(shown),
                "{version}: {e}"
            ),
            None => assert!(
                !e.contains("note:") && !e.contains("warning"),
                "{version}: {e}"
            ),
        }
    }
    // An older file that already holds the values: no note.
    let o = dima()
        .args(["analyze", "-i", s(&tiny()), "-o", "on.json", "-s", "4"])
        .args(TINY)
        .arg("--similarity-weighted-entropy")
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let mut v = json(&dir.join("on.json"));
    v["schema_version"] = "1.0.0".into();
    std::fs::write(dir.join("v.json"), serde_json::to_vec(&v).unwrap()).unwrap();
    let o = run(&["view", "-i", "v.json", "-o", "-"], dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(!stderr(&o).contains("note:"), "{}", stderr(&o));
}

/// `--similarity-weighted-entropy`: the values in the results and the end-of-run line; a
/// warning, and nothing computed, when no output carries them (the run report still records
/// the command as typed); the methods text cites Wu 2025.
#[test]
fn similarity_weighted_entropy_on_the_command_line() {
    let d = tmp();
    let dir = d.path();
    let t = tiny();
    let o = dima()
        .args(["analyze", "-i", s(&t), "-o", "r.json", "-s", "4"])
        .args(TINY)
        .arg("--similarity-weighted-entropy")
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let line = stderr(&o);
    // The means of tests/golden/tiny_similarity/results.json: 0.1794… and observed 0.2700….
    assert!(
        line.contains("; similarity-weighted mean 0.1795 bits (observed 0.2700)"),
        "{line}"
    );
    let r = json(&dir.join("r.json"));
    assert_eq!(
        r["run"]["parameters"]["similarity_weighted_entropy"],
        serde_json::json!({"value": true, "origin": "user"})
    );
    // Position 6 has two equally common k-mers one residue apart: (k + d)/(2k) = 2/3.
    let p6 = &r["results"][5];
    assert_eq!(p6["diversity_motifs"].as_array().unwrap().len(), 2);
    assert!((p6["similarity_weighted_entropy"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-15);
    assert_eq!(p6["normalised_observed_entropy"], 1.0);
    assert!(r["similarity_weighted_entropy_summary"].is_object());
    // Without the option: the line is as before, and the values are null.
    let o = dima()
        .args(["analyze", "-i", s(&t), "-o", "plain.json"])
        .args(TINY)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        !stderr(&o).contains("similarity-weighted"),
        "{}",
        stderr(&o)
    );
    let plain = json(&dir.join("plain.json"));
    assert!(plain["results"][5]["similarity_weighted_entropy"].is_null());
    // No output carries the values: a warning with its hint, and they are not computed.
    let o = dima()
        .args(["analyze", "-i", s(&t), "-o", "v.tsv", "--table", "variants"])
        .args(TINY)
        .args(["--similarity-weighted-entropy", "--run-report", "run.json"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let e = stderr(&o);
    assert!(
        e.contains("warning[flag_has_no_effect]: --similarity-weighted-entropy has no effect"),
        "{e}"
    );
    assert!(
        e.contains("hint: write the results (-o PATH.json) or the positions table"),
        "{e}"
    );
    assert!(!e.contains("similarity-weighted mean"), "{e}");
    let report = json(&dir.join("run.json"));
    assert!(
        report["command"]
            .as_array()
            .unwrap()
            .contains(&Value::from("--similarity-weighted-entropy"))
    );
    // The methods text cites the method.
    let o = run(&["cite", "--from", "r.json"], dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stdout(&o).contains("Wu (2025)"), "{}", stdout(&o));
    // A protein run also cites the amino-acid identity measure.
    assert!(stdout(&o).contains("(Grishin 1995)"), "{}", stdout(&o));
    // No eligible position: no summary, so no similarity-weighted group on the line.
    let o = dima()
        .args(["analyze", "-i", s(&t), "-o", "none.json", "-s", "100"])
        .args(TINY)
        .arg("--similarity-weighted-entropy")
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let e = stderr(&o);
    assert!(e.contains("no eligible positions"), "{e}");
    assert!(!e.contains("similarity-weighted mean"), "{e}");
    let none = json(&dir.join("none.json"));
    assert_eq!(
        none["similarity_weighted_entropy_summary"]["position_count"],
        0
    );
    assert!(none["similarity_weighted_entropy_summary"]["average_entropy"].is_null());
}

/// The names of the files in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// `dima analyze` of the tiny fixture (copied to `in.fasta`) with `extra`, in `dir`.
fn analyze_tiny(dir: &Path, extra: &[&str]) -> Output {
    if !dir.join("in.fasta").exists() {
        std::fs::copy(tiny(), dir.join("in.fasta")).unwrap();
    }
    dima()
        .args(["analyze", "-i", "in.fasta"])
        .args(TINY)
        .args(extra)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// B2: every output name is read once, before anything runs. Names DiMA cannot honour are
/// refused (the run report still tells what happened, unless its own name is refused); a file
/// of conserved sequences named neither .json nor .tsv gets JSON with a warning.
#[test]
fn output_names_are_read_before_anything_runs() {
    let d = tmp();
    let dir = d.path();
    for (extra, message) in [
        (
            vec!["-o", "r.json.bz2"],
            "'r.json.bz2' asks for bzip2 compression, which DiMA does not write",
        ),
        (
            vec!["-o", "r.json.xz", "-O", "json"],
            "asks for xz compression",
        ),
        (
            vec!["-o", "r.json.gz.zst"],
            "asks for two compressions (.gz, then .zst)",
        ),
        (vec!["-o", "out/"], "'out/' names a folder, not a file"),
        (
            vec!["-o", "r.json", "--hcs-output", "h.jsonl"],
            "'h.jsonl' names JSON Lines, but conserved sequences are written as JSON or TSV",
        ),
        (
            vec!["-o", "r.json", "--hcs-output", "h.tsv.zip"],
            "asks for a ZIP archive",
        ),
        (vec!["--tsv-dir", "t", "-o", "x.tsv.7z"], "a 7-Zip archive"),
    ] {
        let mut args = extra.clone();
        args.extend(["--run-report", "run.json"]);
        let o = analyze_tiny(dir, &args);
        assert_eq!(code(&o), 2, "{args:?}: {}", stderr(&o));
        assert!(stderr(&o).contains(message), "{args:?}: {}", stderr(&o));
        assert!(stderr(&o).contains("hint: "), "{args:?}: {}", stderr(&o));
        let r = json(&dir.join("run.json"));
        assert_eq!(
            (&r["status"], &r["exit_code"]),
            (&Value::from("failed"), &Value::from(2))
        );
        assert_eq!(
            r["phases"],
            serde_json::json!([]),
            "{args:?}: before the analysis"
        );
        std::fs::remove_file(dir.join("run.json")).unwrap();
        assert_eq!(names(dir), ["in.fasta"], "{args:?}");
    }
    // A run report whose own name is refused writes no report.
    for (name, message) in [
        (
            "run.tsv",
            "the run report is JSON, but 'run.tsv' names a TSV table",
        ),
        ("run.jsonl.gz", "names JSON Lines"),
        ("run.json.bz2", "bzip2"),
        ("run/", "names a folder"),
    ] {
        let o = analyze_tiny(dir, &["-o", "r.json", "--run-report", name]);
        assert_eq!(code(&o), 2, "{name}: {}", stderr(&o));
        assert!(stderr(&o).contains(message), "{name}: {}", stderr(&o));
        assert_eq!(names(dir), ["in.fasta"], "{name}");
    }
    // `dima view` and `dima hcs` read their names alike.
    assert_eq!(code(&analyze_tiny(dir, &["-o", "r.json", "-q"])), 0);
    for args in [
        vec!["view", "-i", "r.json", "-o", "v.json.lz4"],
        vec!["hcs", "-i", "r.json", "-o", "h.json.br"],
        vec!["hcs", "-i", "r.json", "-o", "h.json", "-O", "tsv"],
    ] {
        let o = run(&args, dir);
        assert_eq!(code(&o), 2, "{args:?}: {}", stderr(&o));
    }
    assert_eq!(names(dir), ["in.fasta", "r.json"]);
    // A compressed run report, by name.
    let o = analyze_tiny(dir, &["-o", "r.json", "--run-report", "run.json.gz", "-q"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(
        std::fs::read(dir.join("run.json.gz")).unwrap()[..2],
        [0x1F, 0x8B]
    );
    // The documented JSON for a name that gives no format: warned about and counted.
    let o = analyze_tiny(dir, &["-o", "r.json", "--hcs-output", "regions.txt"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let e = stderr(&o);
    assert!(
        e.contains(
            "warning[output_format_assumed]: 'regions.txt' does not name JSON or TSV, so the \
             conserved sequences are written to it as JSON\n  hint: name it regions.txt.json or \
             regions.txt.tsv (optionally followed by .gz or .zst)\n"
        ),
        "{e}"
    );
    // Counted in the summary line, like every warning.
    let warnings = |e: &str| -> u64 {
        let end = e.rfind(" warning").unwrap();
        let start = e[..end].rfind("; ").unwrap() + 2;
        e[start..end].parse().unwrap()
    };
    let named = stderr(&analyze_tiny(
        dir,
        &["-o", "r.json", "--hcs-output", "regions.json"],
    ));
    assert_eq!(warnings(&e), warnings(&named) + 1, "{e}\n{named}");
    assert!(json(&dir.join("regions.txt"))["region_count"].is_u64());
    let o = analyze_tiny(dir, &["-o", "r.json", "--hcs-output", "regions.txt", "-q"]);
    assert!(o.stderr.is_empty(), "-q: {}", stderr(&o));
    // `dima hcs` can choose the format with -O instead; standard output takes JSON silently.
    let o = run(&["hcs", "-i", "r.json", "-o", "h.out"], dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(
        stderr(&o).contains(", or choose the format with -O json or -O tsv"),
        "{}",
        stderr(&o)
    );
    let assumed = "output_format_assumed";
    let o = run(&["hcs", "-i", "r.json", "-o", "h.out", "-O", "tsv"], dir);
    assert!(!stderr(&o).contains(assumed), "{}", stderr(&o));
    assert!(
        std::fs::read_to_string(dir.join("h.out"))
            .unwrap()
            .starts_with("region_id\t")
    );
    for args in [
        vec!["hcs", "-i", "r.json"],
        vec!["hcs", "-i", "r.json", "-o", "-"],
    ] {
        let o = run(&args, dir);
        assert_eq!(code(&o), 0, "{args:?}");
        assert!(!stderr(&o).contains(assumed), "{args:?}: {}", stderr(&o));
    }
    #[cfg(unix)]
    {
        let o = run(&["hcs", "-i", "r.json", "-o", "/dev/stdout"], dir);
        assert!(!stderr(&o).contains(assumed), "{}", stderr(&o));
        assert!(serde_json::from_slice::<Value>(&o.stdout).is_ok());
    }
}

/// B5: a folder is never an output file, and `--tsv-dir` is never a file: usage errors before
/// anything runs, on every platform.
#[test]
fn folders_and_files_are_told_apart() {
    let d = tmp();
    let dir = d.path();
    std::fs::create_dir(dir.join("adir")).unwrap();
    std::fs::write(dir.join("file.txt"), "x").unwrap();
    for (extra, message) in [
        (
            vec!["-o", "adir", "-O", "json"],
            "'adir' is a folder, not a file",
        ),
        (
            vec!["-o", "r.json", "--hcs-output", "adir"],
            "'adir' is a folder, not a file",
        ),
        (
            vec!["--tsv-dir", "file.txt"],
            "'file.txt' is a file, not a folder",
        ),
    ] {
        let o = analyze_tiny(dir, &extra);
        assert_eq!(code(&o), 2, "{extra:?}: {}", stderr(&o));
        assert!(stderr(&o).contains(message), "{extra:?}: {}", stderr(&o));
    }
    // The run report's own folder: refused, and no report.
    let o = analyze_tiny(dir, &["-o", "r.json", "--run-report", "adir"]);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains("give a file name, such as"),
        "{}",
        stderr(&o)
    );
    assert_eq!(code(&analyze_tiny(dir, &["-o", "r.json", "-q"])), 0);
    for args in [
        vec!["view", "-i", "r.json", "-o", "adir", "-O", "json"],
        vec!["hcs", "-i", "r.json", "-o", "adir"],
        vec!["view", "-i", "r.json", "--tsv-dir", "file.txt"],
    ] {
        let o = run(&args, dir);
        assert_eq!(code(&o), 2, "{args:?}: {}", stderr(&o));
    }
    assert!(
        std::fs::read_dir(dir.join("adir"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// B4: every pair of destinations, and each destination and the input, is checked before
/// anything runs; the message names both in words. Nothing is written, not even the run
/// report when it is the one that clashes.
#[test]
fn every_pair_of_destinations_is_checked() {
    let d = tmp();
    let dir = d.path();
    std::fs::copy(tiny(), dir.join("in.fasta")).unwrap();
    let before = std::fs::read(dir.join("in.fasta")).unwrap();
    std::fs::create_dir(dir.join("t")).unwrap();
    for (extra, message) in [
        (
            vec!["-o", "a.json", "--hcs-output", "./a.json"],
            "'a.json' would be written twice: as the results (-o) and as the conserved sequences \
             (--hcs-output)",
        ),
        (
            vec![
                "--tsv-dir",
                "t",
                "-o",
                "t/in.variants.tsv",
                "--table",
                "positions",
            ],
            "would be written twice: as the results (-o) and as the variants table (--tsv-dir)",
        ),
        (
            vec!["--tsv-dir", "t", "--hcs-output", "t/../t/in.hcs.tsv"],
            "would be written twice: as the hcs table (--tsv-dir) and as the conserved sequences \
             (--hcs-output)",
        ),
        (
            vec!["-o", "r.json", "--run-report", "r.json"],
            "'r.json' would be written twice: as the run report (--run-report) and as the \
             results (-o)",
        ),
        (
            vec![
                "-o",
                "x.json",
                "--hcs-output",
                "h.json",
                "--run-report",
                "./h.json",
            ],
            "as the run report (--run-report) and as the conserved sequences (--hcs-output)",
        ),
        (
            vec!["-o", "x.json", "--run-report", "in.fasta"],
            "output 'in.fasta' (the run report (--run-report)) is the input file",
        ),
        (
            vec!["-O", "json", "-o", "./in.fasta"],
            "output './in.fasta' (the results (-o)) is the input file",
        ),
        (
            vec!["--tsv-dir", "t", "-o", "t", "-O", "json"],
            "'t' is a folder, not a file",
        ),
        (
            vec!["--tsv-dir", "new", "-o", "new", "-O", "json"],
            "'new' is given both as the --tsv-dir folder and as the results (-o)",
        ),
        (
            vec!["--tsv-dir", "new", "--run-report", "new"],
            "'new' is given both as the --tsv-dir folder and as the run report (--run-report)",
        ),
        (
            vec!["-o", "-", "--hcs-output", "-"],
            "two outputs would go to standard output: the results (-o) and the conserved \
             sequences (--hcs-output)",
        ),
        (
            vec!["--run-report", "-"],
            "two outputs would go to standard output: the run report (--run-report) and the \
             results (standard output by default)",
        ),
    ] {
        let o = analyze_tiny(dir, &extra);
        assert_eq!(code(&o), 2, "{extra:?}: {}", stderr(&o));
        assert!(stderr(&o).contains(message), "{extra:?}: {}", stderr(&o));
        assert!(o.stdout.is_empty(), "{extra:?}");
        assert_eq!(names(dir), ["in.fasta", "t"], "{extra:?}");
        assert!(
            std::fs::read_dir(dir.join("t")).unwrap().next().is_none(),
            "{extra:?}"
        );
        assert_eq!(std::fs::read(dir.join("in.fasta")).unwrap(), before);
    }
    // The default results and a report on standard output: the hint explains the default.
    let o = analyze_tiny(dir, &["--run-report", "-"]);
    assert!(
        stderr(&o).contains("write them to a file with -o FILE, or discard them with -o"),
        "{}",
        stderr(&o)
    );
    // Names that differ only in letter case may be one file (macOS, Windows): refused anywhere.
    let o = analyze_tiny(dir, &["-o", "A.json", "--hcs-output", "a.JSON"]);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains("may be the same file as") || stderr(&o).contains("written twice"),
        "{}",
        stderr(&o)
    );
    // Devices take any number of outputs.
    #[cfg(unix)]
    {
        let o = analyze_tiny(
            dir,
            &[
                "-o",
                "/dev/null",
                "-O",
                "json",
                "--hcs-output",
                "/dev/null",
                "--run-report",
                "/dev/null",
                "-q",
            ],
        );
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        // /dev/stdout is standard output: one output only.
        let o = analyze_tiny(dir, &["-o", "-", "--hcs-output", "/dev/stdout"]);
        assert_eq!(code(&o), 2, "{}", stderr(&o));
        assert!(
            stderr(&o).contains("two outputs would go to standard output"),
            "{}",
            stderr(&o)
        );
    }
}

/// The data-loss bug of 1.1.1 and its two variants: a run report named like a `--tsv-dir`
/// table replaced the table, even when the run failed before its own collision check. The
/// table is now left as it was, and no report is written.
#[test]
fn a_run_report_never_replaces_a_table() {
    let d = tmp();
    let dir = d.path();
    let o = analyze_tiny(dir, &["--tsv-dir", "tables", "-q"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let table = dir.join("tables/in.positions.tsv");
    let before = std::fs::read(&table).unwrap();
    let report = "tables/in.positions.tsv";
    for extra in [
        vec!["--tsv-dir", "tables", "--run-report", report],
        vec!["--tsv-dir", "tables", "-o", "x.tsv", "--run-report", report],
        vec!["--header-format", "A|A", "--run-report", report],
        vec!["--hcs-min-incidence", "50", "--run-report", report],
    ] {
        let o = analyze_tiny(dir, &extra);
        assert_eq!(code(&o), 2, "{extra:?}: {}", stderr(&o));
        assert_eq!(std::fs::read(&table).unwrap(), before, "{extra:?}");
        assert_eq!(names(&dir.join("tables")).len(), 4, "{extra:?}");
        assert_eq!(names(dir), ["in.fasta", "tables"], "{extra:?}");
    }
    // A report name that passes the name rules but leads to a table of the run (a symbolic
    // link): the collision check refuses it, before anything else, and writes no report.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("in.positions.tsv", dir.join("tables/report.json")).unwrap();
        for extra in [
            vec!["--tsv-dir", "tables", "--run-report", "tables/report.json"],
            vec![
                "--tsv-dir",
                "tables",
                "-o",
                "x.tsv",
                "--run-report",
                "tables/report.json",
            ],
            vec![
                "--tsv-dir",
                "tables",
                "--header-format",
                "A|A",
                "--run-report",
                "tables/report.json",
            ],
        ] {
            let o = analyze_tiny(dir, &extra);
            assert_eq!(code(&o), 2, "{extra:?}: {}", stderr(&o));
            assert_eq!(std::fs::read(&table).unwrap(), before, "{extra:?}");
        }
        let o = analyze_tiny(
            dir,
            &["--tsv-dir", "tables", "--run-report", "tables/report.json"],
        );
        assert!(
            stderr(&o).contains(
                "'tables/report.json' would be written twice: as the run report (--run-report) \
                 and as the positions table (--tsv-dir)"
            ),
            "{}",
            stderr(&o)
        );
        assert!(
            stderr(&o).contains("such as 'tables/report.run-report.json'"),
            "{}",
            stderr(&o)
        );
    }
}

/// B4 on Unix: standard input and output have no name, so they are compared with the files the
/// run replaces by the file behind them. Appending to a log and devices are never a clash.
#[cfg(unix)]
#[test]
fn standard_input_and_output_are_compared_by_file() {
    let d = tmp();
    let dir = d.path();
    std::fs::copy(tiny(), dir.join("in.fasta")).unwrap();
    let before = std::fs::read(dir.join("in.fasta")).unwrap();
    let from_stdin = |extra: &[&str], stdout: Stdio| {
        dima()
            .args(["analyze"])
            .args(TINY)
            .args(extra)
            .stdin(std::fs::File::open(dir.join("in.fasta")).unwrap())
            .stdout(stdout)
            .current_dir(dir)
            .output()
            .unwrap()
    };
    // `dima analyze < in.fasta -o in.fasta`: the input is never replaced.
    for extra in [
        vec!["-O", "json", "-o", "in.fasta"],
        vec!["-o", "r.json", "--run-report", "in.fasta"],
    ] {
        let o = from_stdin(&extra, Stdio::piped());
        assert_eq!(code(&o), 2, "{extra:?}: {}", stderr(&o));
        assert!(
            stderr(&o).contains("is the input file (read from standard input)"),
            "{extra:?}: {}",
            stderr(&o)
        );
        assert_eq!(std::fs::read(dir.join("in.fasta")).unwrap(), before);
    }
    // `-o r.json --hcs-output - > r.json`: the results would replace what standard output
    // writes.
    let o = dima()
        .args(["analyze", "-i", "in.fasta"])
        .args(TINY)
        .args(["-o", "r.json", "--hcs-output", "-"])
        .stdout(std::fs::File::create(dir.join("r.json")).unwrap())
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains(
            "standard output, where the conserved sequences (--hcs-output) would go, is the file \
             'r.json', which is also written as the results (-o)"
        ),
        "{}",
        stderr(&o)
    );
    // Standard output appended to the input file.
    let appending = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("in.fasta"))
        .unwrap();
    let o = from_stdin(&["-O", "json", "-o", "-"], Stdio::from(appending));
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("is the input file"), "{}", stderr(&o));
    assert_eq!(std::fs::read(dir.join("in.fasta")).unwrap(), before);
    // Appending to a log (with the run report elsewhere) and devices are fine.
    let log = dir.join("log.txt");
    std::fs::write(&log, "LOG\n").unwrap();
    let appending = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    let o = dima()
        .args(["analyze", "-i", "in.fasta", "-q"])
        .args(TINY)
        .args([
            "-o",
            "/dev/stdout",
            "-O",
            "json",
            "--run-report",
            "run.json",
        ])
        .stdout(appending)
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(std::fs::read_to_string(&log).unwrap().starts_with("LOG\n{"));
    let o = from_stdin(
        &["-q", "-O", "json", "-o", "-", "--hcs-output", "/dev/null"],
        Stdio::from(std::fs::File::create("/dev/null").unwrap()),
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
}

/// C2/C3: the run report is opened before the analysis, so a report that cannot be created
/// stops the run at once; a failed run removes every folder level it created, but keeps the one
/// that holds its report.
#[test]
fn failed_runs_leave_only_their_report() {
    let d = tmp();
    let dir = d.path();
    std::fs::write(dir.join("empty.fa"), "").unwrap();
    let o = run(
        &["analyze", "-i", "empty.fa", "--tsv-dir", "new/sub", "-q"],
        dir,
    );
    assert_eq!(code(&o), 4, "{}", stderr(&o));
    assert!(!dir.join("new").exists(), "every level the run created");
    // The report inside a --tsv-dir that does not exist yet: the folder is created for it and
    // kept, holding the report only.
    for (tsv_dir, report) in [("out", "out/run.json"), ("out2/sub", "out2/run.json")] {
        let o = run(
            &[
                "analyze",
                "-i",
                "empty.fa",
                "--tsv-dir",
                tsv_dir,
                "--run-report",
                report,
            ],
            dir,
        );
        assert_eq!(code(&o), 4, "{}", stderr(&o));
        assert_eq!(json(&dir.join(report))["status"], "failed", "{report}");
        let parent = dir.join(report).parent().unwrap().to_path_buf();
        assert_eq!(names(&parent), ["run.json"], "{report}");
    }
    // A report that cannot be created: the run stops before reading the input.
    let o = run(
        &[
            "analyze",
            "-i",
            "empty.fa",
            "--run-report",
            "missing/run.json",
        ],
        dir,
    );
    assert_eq!(code(&o), 3, "{}", stderr(&o));
    assert!(
        stderr(&o).contains("cannot create 'missing/run.json'"),
        "{}",
        stderr(&o)
    );
    assert!(!stderr(&o).contains("no FASTA records"), "{}", stderr(&o));
}

/// `dima view --tsv-dir` checks the tables with the names they get, the results' query name.
#[test]
fn view_checks_the_real_table_names() {
    let d = tmp();
    let dir = d.path();
    assert_eq!(code(&analyze_tiny(dir, &["-o", "r.json", "-q"])), 0);
    // No false alarm for a file that only the placeholder name would have reached.
    let o = run(
        &[
            "view",
            "-i",
            "r.json",
            "--tsv-dir",
            "t",
            "-o",
            "t/dima.positions.tsv",
            "-O",
            "tsv",
            "--table",
            "positions",
            "-q",
        ],
        dir,
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(names(&dir.join("t")).len(), 5);
    // A real clash is refused, and the folder is not created.
    let o = run(
        &[
            "view",
            "-i",
            "r.json",
            "--tsv-dir",
            "t2",
            "-o",
            "t2/in.positions.tsv",
            "-O",
            "tsv",
            "--table",
            "positions",
        ],
        dir,
    );
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains("as the results (-o) and as the positions table (--tsv-dir)"),
        "{}",
        stderr(&o)
    );
    assert!(!dir.join("t2").exists());
}

/// B7: `DIMA_PROGRESS` is read by `dima analyze` itself: an empty value is unset, an invalid
/// one is a usage error that does not repeat it; an explicit `--progress` beats
/// `DIMA_NO_PROGRESS`, and `--progress` and `--no-progress` override each other.
#[test]
fn progress_settings_and_their_order() {
    let d = tmp();
    let dir = d.path();
    let shown = |args: &[&str], env: &[(&str, &str)]| -> Option<bool> {
        let mut c = dima();
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c
            .args(["analyze", "-i", s(&tiny()), "-o", "r.json"])
            .args(TINY)
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        (code(&o) == 0).then(|| stderr(&o).contains("] counting k-mers"))
    };
    assert_eq!(
        shown(&[], &[("DIMA_PROGRESS", "")]),
        Some(false),
        "empty = unset"
    );
    assert_eq!(shown(&[], &[("DIMA_PROGRESS", "plain")]), Some(true));
    assert_eq!(
        shown(&[], &[("DIMA_PROGRESS", "Plain")]),
        None,
        "case-sensitive"
    );
    assert_eq!(
        shown(&["--progress", "plain"], &[("DIMA_NO_PROGRESS", "1")]),
        Some(true),
        "the option beats the variable"
    );
    assert_eq!(
        shown(
            &[],
            &[("DIMA_NO_PROGRESS", "yes"), ("DIMA_PROGRESS", "plain")]
        ),
        Some(false)
    );
    assert_eq!(
        shown(
            &[],
            &[("DIMA_NO_PROGRESS", "0"), ("DIMA_PROGRESS", "plain")]
        ),
        Some(true),
        "a false DIMA_NO_PROGRESS"
    );
    assert_eq!(
        shown(&["--progress", "plain", "--no-progress"], &[]),
        Some(false)
    );
    assert_eq!(
        shown(&["--no-progress", "--progress", "plain"], &[]),
        Some(true)
    );
    // The invalid value: exit 2, and neither the message nor the run report repeats it.
    let o = dima()
        .env("DIMA_PROGRESS", "sparkles")
        .args([
            "analyze",
            "-i",
            s(&tiny()),
            "-o",
            "r.json",
            "--run-report",
            "run.json",
        ])
        .args(TINY)
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains(
            "error: DIMA_PROGRESS is set, but not to a progress mode (auto, plain or off)"
        ),
        "{}",
        stderr(&o)
    );
    assert!(!stderr(&o).contains("sparkles"));
    assert!(
        !std::fs::read_to_string(dir.join("run.json"))
            .unwrap()
            .contains("sparkles")
    );
    // Only `dima analyze` shows progress, so only it reads the variable.
    let o = dima()
        .env("DIMA_PROGRESS", "sparkles")
        .args(["view", "-i", "r.json", "-o", "v.json"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
}

/// Spawns `dima` with standard output closed at once; returns its standard error and the exit
/// code.
fn with_closed_stdout(args: &[&str], dir: &Path) -> (String, i32) {
    let mut child = dima()
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let o = child.wait_with_output().unwrap();
    (stderr(&o), code(&o))
}

/// B8: a reader of standard output that goes away (`dima ... | head`) ends that output only:
/// the warnings, notes and summary line are still printed, and the run report lists the files
/// that were written.
#[test]
fn a_closed_pipe_keeps_the_messages() {
    let d = tmp();
    let dir = d.path();
    std::fs::copy(tiny(), dir.join("in.fasta")).unwrap();
    let t: Vec<&str> = ["analyze", "-i", "in.fasta"]
        .into_iter()
        .chain(TINY)
        .collect();
    let mut args = t.clone();
    args.extend([
        "-s",
        "100",
        "-o",
        "-",
        "--hcs-output",
        "h.json",
        "--run-report",
        "run.json",
    ]);
    let (e, status) = with_closed_stdout(&args, dir);
    assert_eq!(status, 0, "{e}");
    assert!(
        e.contains("warning[threshold_exceeds_sequence_count]"),
        "{e}"
    );
    assert!(e.contains("analysed 8 sequences"), "{e}");
    let r = json(&dir.join("run.json"));
    assert_eq!(r["status"], "completed");
    let roles: Vec<&str> = r["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["hcs"], "the file, not the cut-off standard output");
    // `dima hcs`: the summary line; `dima view`: the JSONL note.
    let mut args = t.clone();
    args.extend(["-o", "r.json", "-q"]);
    assert_eq!(code(&run(&args, dir)), 0);
    let (e, status) =
        with_closed_stdout(&["hcs", "-i", "r.json", "--hcs-min-incidence", "60"], dir);
    assert_eq!(status, 0, "{e}");
    assert!(e.contains("conserved regions"), "{e}");
    let (e, status) = with_closed_stdout(&["view", "-i", "r.json", "-O", "jsonl", "-o", "-"], dir);
    assert_eq!(status, 0, "{e}");
    assert!(e.contains("note: JSONL holds one position per line"), "{e}");
}

/// Standard output is written last, after the other streams too: a reader of standard output
/// that goes away never costs an output written in place, such as a FIFO.
#[cfg(unix)]
#[test]
fn standard_output_is_written_last() {
    let d = tmp();
    let dir = d.path();
    let fifo = dir.join("regions.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let reader = std::thread::spawn({
        let fifo = fifo.clone();
        move || std::fs::read(fifo).unwrap()
    });
    let input = tiny();
    let mut args: Vec<&str> = vec!["analyze", "-i", s(&input)];
    args.extend(TINY);
    args.extend(["-o", "-", "--hcs-output", "regions.fifo", "-q"]);
    let (e, status) = with_closed_stdout(&args, dir);
    assert_eq!(status, 0, "{e}");
    let regions: Value = serde_json::from_slice(&reader.join().unwrap()).unwrap();
    assert!(regions["region_count"].is_u64(), "{regions}");
}

/// B9: `dima cite --from` describes one run: conserved sequences computed from other results
/// (another input, k or T) are refused.
#[test]
fn cite_describes_one_run() {
    let d = tmp();
    let dir = d.path();
    let o = analyze_tiny(dir, &["-o", "a.json", "--hcs-output", "a.hcs.json", "-q"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = analyze_tiny(
        dir,
        &[
            "-s",
            "4",
            "-o",
            "b.json",
            "--hcs-output",
            "b.hcs.json",
            "-q",
        ],
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(&["cite", "--from", "a.json", "--from", "a.hcs.json"], dir);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(&["cite", "--from", "b.hcs.json", "--from", "a.json"], dir);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(
        stderr(&o).contains(
            "error: the conserved sequences in 'b.hcs.json' were not computed from the results in \
             'a.json': T (100 and 4) differs"
        ),
        "{}",
        stderr(&o)
    );
    assert!(
        stderr(&o).contains(
            "hint: compute the conserved sequences from these results: dima hcs -i a.json"
        ),
        "{}",
        stderr(&o)
    );
    assert!(o.stdout.is_empty());
    // Another input: the checksum tells.
    let other = dir.join("other.fasta");
    let text = std::fs::read_to_string(tiny()).unwrap();
    std::fs::write(&other, text.replace("MKTAY", "MKTAW")).unwrap();
    let o = dima()
        .args([
            "analyze",
            "-i",
            "other.fasta",
            "-o",
            "c.json",
            "--hcs-output",
            "c.hcs.json",
            "-q",
        ])
        .args(TINY)
        .current_dir(dir)
        .output()
        .unwrap();
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(&["cite", "--from", "a.json", "--from", "c.hcs.json"], dir);
    assert_eq!(code(&o), 2, "{}", stderr(&o));
    assert!(stderr(&o).contains("the input differs"), "{}", stderr(&o));
}

/// B6: an interruption is an error, so its message is printed with -q too.
#[cfg(unix)]
#[test]
fn an_interruption_is_reported_with_q() {
    use std::os::unix::process::ExitStatusExt;
    let d = tmp();
    let mut child = dima()
        .args(["analyze", "-q", "-o", "r.json", "--run-report", "run.json"])
        .args(TINY)
        .current_dir(d.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // An open pipe that never ends, as in `waiting_on_stdin`: the run waits for more input.
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&std::fs::read(tiny()).unwrap()).unwrap();
    stdin.flush().unwrap();
    // Nothing is printed with -q: the temporary file of the results, opened just before the
    // analysis (well after the interrupt handlers are installed), tells it is waiting.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !names(d.path())
        .iter()
        .any(|n| n.starts_with(".r.json.dima-"))
    {
        assert!(Instant::now() < deadline, "the outputs were never opened");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(100));
    let sent = Instant::now();
    kill(&child, "TERM");
    let status = wait_exit(&mut child, Duration::from_secs(10));
    assert!(
        sent.elapsed() < Duration::from_secs(2),
        "not the grace period"
    );
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    drop(stdin);
    let mut e = String::new();
    child.stderr.take().unwrap().read_to_string(&mut e).unwrap();
    assert_eq!(e, "error: cancelled; no results were written\n");
    assert_eq!(json(&d.path().join("run.json"))["status"], "cancelled");
}

/// Windows has no /dev/null: its null device NUL takes any output.
#[cfg(windows)]
#[test]
fn the_null_device_takes_outputs() {
    let d = tmp();
    for extra in [
        vec!["-o", "NUL", "-O", "json"],
        vec!["-o", "r.json", "--hcs-output", "NUL"],
    ] {
        let o = analyze_tiny(d.path(), &extra);
        assert_eq!(code(&o), 0, "{extra:?}: {}", stderr(&o));
    }
}
