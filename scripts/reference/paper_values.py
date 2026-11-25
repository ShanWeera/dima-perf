#!/usr/bin/env python3
"""Writes the expected values of the paper test (crates/dima-core/tests/paper.rs).

Runs the reference implementation (dima_reference.py, in this directory) on the four bundled
datasets and Wu 2025's two ToBRFV samples with the parameters the test uses, and writes the
numbers the test compares with `common::assert_agrees` to
crates/dima-core/tests/paper_expected/mod.rs, a generated file that is never edited by hand.
Every run but the T = 30 one computes similarity-weighted entropy (pairwise, as Wu defines it),
so the unchanged values also show that the option changes nothing else. `--check` fails if that
file is out of date (CI), and prints the difference. Standard library only; the seven runs take
about two minutes on four cores, almost all of it in the HIV-1 datasets; about 250 MB of
memory.

The dima entropy (and so every mean and extreme) uses the platform's `math.log`, whose last bit
may differ between operating systems (README.md, "Determinism"); the committed file is the
output on Linux, where CI checks it.

Usage: python3 scripts/reference/paper_values.py [--check]
"""
import argparse
import concurrent.futures
import difflib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ORACLE = Path(__file__).resolve().parent / "dima_reference.py"
OUT = ROOT / "crates/dima-core/tests/paper_expected/mod.rs"

# (module, input, oracle options, description); paths from the repository root.
SWE = "--similarity-weighted-entropy"
RUNS = [
    ("mers_aa_t30", "samples/mers_spike_aa.fasta", ["-s", "30"], "MERS-CoV spike protein, T = 30"),
    ("mers_aa_t100", "samples/mers_spike_aa.fasta", [SWE], "MERS-CoV spike protein, defaults (T = 100)"),
    ("mers_nt", "tests/data/MERS_Spike_nt.fasta.bz2", [SWE], "MERS-CoV spike gene, defaults"),
    ("hiv_pro", "tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2", [SWE], "HIV-1 pol protein, defaults"),
    ("hiv_dna", "tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2", [SWE], "HIV-1 pol gene, defaults"),
    (
        "tobrfv_tm2",
        "tests/data/tobrfv_tm2.fasta.bz2",
        ["-a", "dna", "-k", "120", SWE],
        "ToBRFV in tm-2 plants (Wu 2025, Supplementary File 2), one 120-mer position",
    ),
    (
        "tobrfv_tm22",
        "tests/data/tobrfv_tm22.fasta.bz2",
        ["-a", "dna", "-k", "120", SWE],
        "ToBRFV in Tm-2^2 plants (Wu 2025, Supplementary File 2), one 120-mer position",
    ),
]


def without_motifs(obj: dict) -> dict:
    """json object hook: drops the variant lists as they are parsed (keeps memory small)."""
    obj.pop("diversity_motifs", None)
    return obj


def run(module: str, data: str, options: list[str], work: Path) -> dict:
    out = work / f"{module}.json"
    done = subprocess.run(
        [sys.executable, str(ORACLE), "analyze", "-i", data, *options, "-o", str(out)],
        cwd=ROOT, capture_output=True, text=True,
    )
    if done.returncode != 0:
        raise SystemExit(f"{module}: the reference implementation failed (exit {done.returncode}):\n{done.stderr}")
    with out.open(encoding="utf-8") as f:
        return json.load(f, object_hook=without_motifs)


TYPES = """\
/// Summary of one run.
pub struct Summary {
    /// Mean entropy over the eligible positions.
    pub mean: f64,
    /// Mean entropy over all positions (no support counts as 0).
    pub all_mean: f64,
    /// Lowest entropy over the eligible positions.
    pub lowest: f64,
    /// Highest entropy over the eligible positions.
    pub highest: f64,
}

/// One position.
pub struct Position {
    /// Entropy (the run's estimator).
    pub entropy: f64,
    /// Observed (plug-in) entropy.
    pub observed: f64,
    /// Sample coverage.
    pub coverage: f64,
}

/// Similarity-weighted entropy summary of one run (eligible positions).
pub struct SimilaritySummary {
    /// Mean similarity-weighted entropy.
    pub mean: f64,
    /// Mean observed entropy of the same positions.
    pub observed_mean: f64,
    /// Lowest similarity-weighted entropy.
    pub lowest: f64,
    /// Highest similarity-weighted entropy.
    pub highest: f64,
}

/// The similarity-weighted values of one position.
pub struct SimilarityPosition {
    /// Similarity-weighted entropy.
    pub similarity_weighted: f64,
    /// Observed entropy / log2 D.
    pub normalised_observed: f64,
    /// Similarity-weighted entropy / log2 D.
    pub normalised_similarity_weighted: f64,
}
"""


def where(e: dict) -> str:
    ties = e["tied_position_count"]
    return f"position {e['position']}" + ("" if ties == 1 else f", {ties} tied")


def summary(name: str, data: str, options: list[str], description: str, r: dict) -> str:
    command = " ".join(["dima_reference.py analyze -i", data, *options])
    lo, hi = r["lowest_entropy"], r["highest_entropy"]
    return (
        f"/// {description}:\n/// `{command}`.\n"
        f"pub const {name.upper()}: Summary = Summary {{\n"
        f"    mean: {r['average_entropy']!r},\n"
        f"    all_mean: {r['all_positions_summary']['average_entropy']!r},\n"
        f"    // {where(lo)}\n    lowest: {lo['entropy']!r},\n"
        f"    // {where(hi)}\n    highest: {hi['entropy']!r},\n"
        "};\n"
    )


def position(name: str, r: dict, number: int) -> str:
    p = r["positions"][number - 1]
    assert p["position"] == number
    return (
        f"/// Position {number} of the run above.\n"
        f"pub const {name.upper()}_POS{number}: Position = Position {{\n"
        f"    entropy: {p['entropy']!r},\n"
        f"    observed: {p['observed_entropy']!r},\n"
        f"    coverage: {p['sample_coverage']!r},\n"
        "};\n"
    )


def similarity_summary(name: str, r: dict) -> str:
    s = r["similarity_weighted_entropy_summary"]
    lo, hi = s["lowest_entropy"], s["highest_entropy"]
    return (
        "/// Similarity-weighted entropy of the run above.\n"
        f"pub const {name.upper()}_SIMILARITY: SimilaritySummary = SimilaritySummary {{\n"
        f"    mean: {s['average_entropy']!r},\n"
        f"    observed_mean: {s['average_observed_entropy']!r},\n"
        f"    // {where(lo)}\n    lowest: {lo['entropy']!r},\n"
        f"    // {where(hi)}\n    highest: {hi['entropy']!r},\n"
        "};\n"
    )


def similarity_position(name: str, r: dict, number: int) -> str:
    p = r["positions"][number - 1]
    assert p["position"] == number
    return (
        f"/// The similarity-weighted values of position {number} of the run above.\n"
        f"pub const {name.upper()}_POS{number}_SIMILARITY: SimilarityPosition = SimilarityPosition {{\n"
        f"    similarity_weighted: {p['similarity_weighted_entropy']!r},\n"
        f"    normalised_observed: {p['normalised_observed_entropy']!r},\n"
        f"    normalised_similarity_weighted: {p['normalised_similarity_weighted_entropy']!r},\n"
        "};\n"
    )


def render(results: dict) -> str:
    versions = {r["reference_version"] for r in results.values()}
    assert len(versions) == 1, versions
    parts = [
        "//! Expected values of `paper.rs`: the output of the reference implementation\n"
        f"//! (`scripts/reference/dima_reference.py`, reference_version {versions.pop()}) on the bundled\n"
        "//! datasets, with paths from the repository root. Generated by\n"
        "//! `python3 scripts/reference/paper_values.py`; do not edit.\n",
        TYPES,
    ]
    for name, data, options, description in RUNS:
        r = results[name]
        text = summary(name, data, options, description, r)
        if name.startswith("mers_aa"):
            # The paper's Fig. 1C.
            text += position(name, r, 25)
        if name.startswith("tobrfv"):
            text += position(name, r, 1)
        if SWE in options:
            text += similarity_summary(name, r)
            if name == "mers_aa_t100":
                text += similarity_position(name, r, 25)
            if name.startswith("tobrfv"):
                text += similarity_position(name, r, 1)
        parts.append(text)
    return "\n".join(parts)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--check", action="store_true", help=f"fail if {OUT.relative_to(ROOT)} is out of date")
    args = p.parse_args()
    with tempfile.TemporaryDirectory() as tmp, concurrent.futures.ThreadPoolExecutor(os.cpu_count() or 1) as pool:
        jobs = {n: pool.submit(run, n, d, o, Path(tmp)) for n, d, o, _ in RUNS}
        text = render({n: j.result() for n, j in jobs.items()})
    current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
    if args.check:
        if current == text:
            return 0
        rel = str(OUT.relative_to(ROOT))
        sys.stdout.writelines(difflib.unified_diff(current.splitlines(True), text.splitlines(True), rel, rel + " (expected)"))
        print(f"{rel} is out of date: run python3 scripts/reference/paper_values.py", file=sys.stderr)
        return 1
    if current != text:
        tmp = OUT.with_suffix(".rs.tmp")
        tmp.write_text(text, encoding="utf-8", newline="\n")
        os.replace(tmp, OUT)
        print(f"wrote {OUT.relative_to(ROOT)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
