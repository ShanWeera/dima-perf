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
    // A directory given as the output file.
    std::fs::create_dir(dir.join("adir")).unwrap();
    let mut folder = args.clone();
    folder.extend(["-o", "adir"]);
    let o = run(&folder, dir);
    assert_ne!(code(&o), 0, "{}", stderr(&o));
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
        ("1.9.0", Some("newer_schema")),
        ("1.1.0-dev", Some("newer_schema")),
    ] {
        let mut v = json(&dir.join("new.json"));
        v["schema_version"] = version.into();
        std::fs::write(dir.join("v.json"), serde_json::to_vec(&v).unwrap()).unwrap();
        let o = run(&["view", "-i", "v.json", "-o", "-"], dir);
        assert_eq!(code(&o), 0, "{version}: {}", stderr(&o));
        let e = stderr(&o);
        match expected {
            Some(code) => assert!(e.contains(code), "{version}: {e}"),
            None => assert!(
                !e.contains("note:") && !e.contains("warning"),
                "{version}: {e}"
            ),
        }
    }
}
