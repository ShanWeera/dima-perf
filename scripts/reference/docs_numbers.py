#!/usr/bin/env python3
"""Computes the numbers of the documentation that come from expectations and simulations.

Most numbers in the documentation are outputs of dima runs, fixed by the tests. Some are not:
how the similarity-weighted values change with the number of sequences, and the values of Wu
2025's ToBRFV samples. This script computes them and writes them between markers in the pages
that show them:

    <!-- docs-numbers:NAME:start -->...<!-- docs-numbers:NAME:end -->

A region holds only what is computed here (a table, or a sentence whose numbers are computed);
the prose around it is written by hand. A region stands on its own lines and holds whole
paragraphs (wrapped at 100 columns) or tables. `--check` prints a unified diff and exits with 1
when a region is out of date (CI); without it the regions are rewritten in place.

Standard library only (Python >= 3.10). It imports the reference implementation
(dima_reference.py, in this directory) for parsing, entropies and the pairwise similarity code;
it inserts its own directory into sys.path and writes no bytecode, so
`python3 -I -B scripts/reference/docs_numbers.py` works. About a second.

Readings
--------
* Wu 2025's Fig. 1b, quasispecies 2: a master sequence at 90% and four variants at 4%, 4%, 1%
  and 1% carrying 2, 4, 2 and 4 substitutions (the red triangles), drawn at distinct sites and
  read so: k = 12, the shortest length that holds the 12 substitutions at distinct sites
  (FIG1B_K), so that two variants differ at the sum of their substitutions.
* ToBRFV: the unique sequences of each sample (the distinct 120-mers; tests/data/README.md);
  their pairwise Hamming distances are averaged over the pairs of distinct unique sequences, the
  reading that gives Wu's 2.12 and 2.80 (Section 3.3).

What is computed, and how
-------------------------
Exactly, from the exact distributions (multinomial, multivariate hypergeometric), not simulated.

* Table 1 (entropy.md): averages over all C(170, n) subsets of the 170 sequences at MERS-CoV
  spike position 25 (k = 9). E[H] from the hypergeometric marginals; E[H_sim] =
  sum_i E[h(X_i)] - sum_i sum_{j != i} s_ij E[h(X_i) X_j] / n with E[X_j | X_i = x] =
  (n - x) c_j / (N - c_i); the means of H_n and H_nsim over the subsets with at least two
  distinct 9-mers (means of ratios) from a dynamic program over the other variants that counts,
  with exact integer binomial coefficients, the subsets by size and number of variants present,
  and carries the similarity sum. E[D] and P(D = 1) exactly. The dynamic program also gives
  E[H] and E[H_sim], which must equal the closed forms; and it is checked against enumerating
  every subset of a toy profile.
* Table 2 (entropy.md): the same for samples drawn with replacement (multinomial), from Wu's
  Fig. 1b population and from two 9-mers at 50% each one site apart.
* ToBRFV (paper-comparison.md): Wu's four values and the Hamming distances between the unique
  sequences of the two samples, with the reference implementation's pairwise code.

Usage: python3 scripts/reference/docs_numbers.py [--check]
"""

from __future__ import annotations

import argparse
import difflib
import math
import os
import re
import sys
from collections import Counter
from fractions import Fraction
from itertools import combinations, product
from math import comb
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import dima_reference as R  # noqa: E402

ROOT = HERE.parents[1]
ENTROPY = ROOT / "docs/src/content/docs/explanation/entropy.md"
COMPARISON = ROOT / "docs/src/content/docs/explanation/paper-comparison.md"

MERS_AA = "samples/mers_spike_aa.fasta"
TOBRFV = (("tm-2", "tests/data/tobrfv_tm2.fasta.bz2"), ("Tm-2²", "tests/data/tobrfv_tm22.fasta.bz2"))

#: Table 1: subset sizes at MERS-CoV spike position 25 (its support is 170).
TABLE1_N = (10, 25, 50, 100, 170)
#: Table 2: sample sizes (with replacement).
TABLE2_N = (10, 25, 50, 100)
#: Wu 2025, Fig. 1b, quasispecies 2: (frequency in %, substitutions from the master).
FIG1B = ((90, 0), (4, 2), (4, 4), (1, 2), (1, 4))
#: The shortest k-mer length that holds Fig. 1b's 12 substitutions at distinct sites.
FIG1B_K = 12

# ---------------------------------------------------------------------------------------------
# Shared numerics
# ---------------------------------------------------------------------------------------------


_H_TABLES: dict[int, tuple[float, ...]] = {}


def h_table(n: int) -> tuple[float, ...]:
    """(h(0), ..., h(n)), h(x) = -(x/n) log2(x/n): the reference implementation's table."""
    t = _H_TABLES.get(n)
    if t is None:
        t = R._h_table(n)
        _H_TABLES[n] = t
    return t


# ---------------------------------------------------------------------------------------------
# Datasets
# ---------------------------------------------------------------------------------------------


def read_alignment(rel: str) -> tuple[int, list[list[tuple[bytes, int]]]]:
    """(k, variants per position): the k-mers counted at every position by a default run.

    Uses the reference implementation's parsing, alphabet guess, default k, symbol classes and
    validity rules; the variants of a position are sorted by count, then k-mer (sort_variants).
    """
    records = R.parse_fasta(R.read_input(str(ROOT / rel)))
    length = R.check_equal_lengths(records)
    resolved, _ = R.easel_guess_alphabet(R.letter_counts(records))
    if resolved is None:
        raise RuntimeError(f"{rel}: alphabet not determined")
    k = 9 if resolved == R.PROTEIN else 27
    seqs, masks, _, _ = R.validate_symbols(records, R.symbol_table(resolved), "strict")
    positions = length - k + 1
    valids = [R.valid_starts(mask, k, positions) for mask in masks]
    out = []
    for p in range(positions):
        kmers = [s[p:p + k] for s, v in zip(seqs, valids, strict=True) if v[p]]
        out.append(R.sort_variants(Counter(kmers)))
    return k, out


def read_unique(rel: str) -> list[tuple[bytes, int]]:
    """The distinct sequences of a FASTA file with their counts (ToBRFV: one record per read)."""
    records = R.parse_fasta(R.read_input(str(ROOT / rel)))
    return R.sort_variants(Counter(r.seq for r in records))


# ---------------------------------------------------------------------------------------------
# Tables 1 and 2: averages over subsets and samples
# ---------------------------------------------------------------------------------------------


def identical_residues(variants: list[tuple[bytes, int]]) -> list[list[int]]:
    """K[i][j]: identical residues of k-mers i and j (dima_reference.matches), so s_ij = K/k."""
    k = len(variants[0][0])
    ints = [int.from_bytes(s, "big") for s, _ in variants]
    return [[R.matches(x, y, k) for y in ints] for x in ints]


def other_variants_dp(weights: list[int], others: list[int], sims: list[int], m_max: int, with_replacement: bool):
    """Dynamic program over the variants ``others``: for every total m <= m_max and number d of
    those variants present, W[m][d] = the summed weight of the configurations, and Z[m][d] =
    the summed weight times sum_j sims[j] y_j (y_j copies of variant j).

    Without replacement the weight of y copies of variant j is C(c_j, y) (``weights`` are the
    counts c_j), so W[m][d] counts subsets; with replacement it is the multinomial weight
    m! / prod y_j! prod a_j^y_j (``weights`` are integer frequencies a_j), built up as
    C(m + y, y) a_j^y. All integers, so exact.
    """
    d_max = len(others)
    W = [[0] * (d_max + 1) for _ in range(m_max + 1)]
    Z = [[0] * (d_max + 1) for _ in range(m_max + 1)]
    W[0][0] = 1
    for j in others:
        c, s = weights[j], sims[j]
        W2 = [[0] * (d_max + 1) for _ in range(m_max + 1)]
        Z2 = [[0] * (d_max + 1) for _ in range(m_max + 1)]
        for m in range(m_max + 1):
            Wm, Zm = W[m], Z[m]
            if not any(Wm):
                continue
            y_max = m_max - m if with_replacement else min(c, m_max - m)
            for y in range(y_max + 1):
                f = comb(m + y, y) * c ** y if with_replacement else comb(c, y)
                row_w, row_z = W2[m + y], Z2[m + y]
                shift = 1 if y else 0
                for d in range(d_max + 1 - shift):
                    if Wm[d]:
                        row_w[d + shift] += f * Wm[d]
                        row_z[d + shift] += f * (Zm[d] + s * y * Wm[d])
        W, Z = W2, Z2
    return W, Z


def averages(variants: list[tuple[bytes, int]], sizes: tuple[int, ...], with_replacement: bool, total: int | None = None) -> list[dict]:
    """Exact averages over subsets (without replacement; counts c_i, N = sum c_i) or samples
    (with replacement; integer frequencies a_i out of ``total``) of each size n: H, H_sim, the
    means of H_n and H_nsim over those with at least two distinct k-mers, E[D] and P(D = 1)."""
    k = len(variants[0][0])
    w = [c for _, c in variants]
    big_n = total if with_replacement else sum(w)
    K = identical_residues(variants)
    D = len(variants)
    m_top = max(sizes)
    dps = []
    for i in range(D):
        others = [j for j in range(D) if j != i]
        dps.append(other_variants_dp(w, others, K[i], m_top, with_replacement))
    rows = []
    for n in sizes:
        h = h_table(n)
        # Every probability below is an exact integer over ``denom`` (int / int division is
        # correctly rounded in Python).
        denom = big_n ** n if with_replacement else comb(big_n, n)
        e_h, e_hs, e_hn, e_hns, e_h_dp, e_hs_dp = [], [], [], [], [], []
        p_one = 0
        absent = 0  # sum_i P(X_i = 0) * denom
        for i in range(D):
            ci = w[i]
            W, Z = dps[i]
            similar = sum(w[j] * K[i][j] for j in range(D) if j != i)  # sum_j c_j K_ij
            if with_replacement:
                p_one += ci ** n
                absent += (big_n - ci) ** n
                lead = [comb(n, x) * ci ** x for x in range(n + 1)]
                pmf = [lead[x] * (big_n - ci) ** (n - x) for x in range(n + 1)]
            else:
                p_one += comb(ci, n)
                absent += comb(big_n - ci, n)
                lead = [comb(ci, x) for x in range(n + 1)]
                pmf = [lead[x] * comb(big_n - ci, n - x) for x in range(n + 1)]
            for x in range(1, n):
                if pmf[x] == 0:
                    continue
                # Closed forms: E[h(X_i)], and E[h(X_i) (1 - S_i)] through
                # E[sum_j K_ij X_j | X_i = x] = (n - x) sum_j c_j K_ij / (N - c_i).
                e_h.append(pmf[x] / denom * h[x])
                kept = k * n * (big_n - ci) - (n - x) * similar  # (1 - E[S_i | x]) k n (N - c_i)
                e_hs.append(pmf[x] * kept / (denom * k * n * (big_n - ci)) * h[x])
                m = n - x
                for d in range(1, D):
                    if W[m][d] == 0:
                        continue
                    weight = lead[x] * W[m][d]  # P(X_i = x, d other variants present) * denom
                    wz = lead[x] * (k * n * W[m][d] - Z[m][d])  # the same times (1 - S_i) k n
                    log_d = math.log2(1 + d)
                    q = weight / denom
                    qs = wz / (denom * k * n)
                    e_h_dp.append(q * h[x])
                    e_hs_dp.append(qs * h[x])
                    e_hn.append(q * h[x] / log_d)
                    e_hns.append(qs * h[x] / log_d)
        p_many = (denom - p_one) / denom
        row = {
            "n": n,
            "H": math.fsum(e_h),
            "H_sim": math.fsum(e_hs),
            "H_n": math.fsum(e_hn) / p_many if p_many else None,
            "H_nsim": math.fsum(e_hns) / p_many if p_many else None,
            "D": (D * denom - absent) / denom,
            "P1": p_one / denom,
        }
        for name, dp in (("H", e_h_dp), ("H_sim", e_hs_dp)):
            if abs(math.fsum(dp) - row[name]) > 1e-12 * max(1.0, row[name]):
                raise RuntimeError(f"dynamic program disagrees with the closed form for E[{name}] at n = {n}")
        rows.append(row)
    return rows


def population_values(variants: list[tuple[bytes, int]]) -> dict:
    """H, H_sim, H_n and H_nsim of a population given as k-mers with integer weights."""
    n_obs = sum(c for _, c in variants)
    mult = R.multiplicities([c for _, c in variants])
    h = R.plug_in_entropy_decimal(mult, n_obs)
    hs = R.similarity_weighted_entropy_decimal(variants, n_obs)
    return {
        "H": float(h),
        "H_sim": float(hs),
        "H_n": R.normalised_entropy(h, len(variants)),
        "H_nsim": R.normalised_entropy(hs, len(variants)),
    }


def enumerated_averages(variants: list[tuple[bytes, int]], n: int, with_replacement: bool) -> dict:
    """The averages of ``averages`` by enumerating every subset or every ordered sample."""
    items = [s for s, c in variants for _ in range(c)]
    if with_replacement:
        total = sum(c for _, c in variants)
        outcomes = [(Fraction(1, total ** n), tuple(items[i] for i in idx)) for idx in product(range(total), repeat=n)]
    else:
        outcomes = [(Fraction(1, comb(len(items), n)), tuple(items[i] for i in idx)) for idx in combinations(range(len(items)), n)]
    acc = {"H": [], "H_sim": [], "H_n": [], "H_nsim": [], "D": [], "P1": []}
    p_many = Fraction(0)
    for p, sample in outcomes:
        v = R.sort_variants(Counter(sample))
        vals = population_values(v) if len(v) > 1 else {"H": 0.0, "H_sim": 0.0, "H_n": None, "H_nsim": None}
        acc["H"].append(float(p) * vals["H"])
        acc["H_sim"].append(float(p) * vals["H_sim"])
        acc["D"].append(float(p) * len(v))
        if len(v) == 1:
            acc["P1"].append(float(p))
        else:
            p_many += p
            acc["H_n"].append(float(p) * vals["H_n"])
            acc["H_nsim"].append(float(p) * vals["H_nsim"])
    out = {key: math.fsum(vals) for key, vals in acc.items()}
    for key in ("H_n", "H_nsim"):
        out[key] = out[key] / float(p_many) if p_many else None
    return out


def check_averages() -> None:
    """The dynamic program against enumeration, on toy profiles with and without replacement."""
    toy = [(b"ACGT", 3), (b"ACGA", 2), (b"TCGA", 1), (b"GGGG", 1)]
    for with_replacement, sizes in ((False, (1, 2, 3, 5, 7)), (True, (1, 2, 4))):
        total = 7 if with_replacement else None
        rows = averages(toy, sizes, with_replacement, total)
        for row in rows:
            ref = enumerated_averages(toy, row["n"], with_replacement)
            for key in ("H", "H_sim", "H_n", "H_nsim", "D", "P1"):
                a, b = row[key], ref[key]
                if (a is None) != (b is None) or (a is not None and abs(a - b) > 1e-12):
                    raise RuntimeError(f"averages: {key} at n = {row['n']} is {a!r}, enumeration {b!r}")


def table1() -> dict:
    k, positions = read_alignment(MERS_AA)
    variants = positions[24]
    return {"variants": variants, "k": k, "rows": averages(variants, TABLE1_N, False)}


def fig1b_population() -> list[tuple[bytes, int]]:
    """Fig. 1b's quasispecies 2 as k-mers: substitutions (A -> C) at distinct sites."""
    out, site = [], 0
    for freq, subs in FIG1B:
        seq = bytearray(b"A" * FIG1B_K)
        seq[site:site + subs] = b"C" * subs
        site += subs
        out.append((bytes(seq), freq))
    if site > FIG1B_K:
        raise RuntimeError("Fig. 1b: more substitutions than sites")
    return out


def table2() -> dict:
    pops = {
        "fig1b": fig1b_population(),
        "half": [(b"AAAAAAAAA", 50), (b"CAAAAAAAA", 50)],
    }
    out = {}
    for name, variants in pops.items():
        out[name] = {
            "variants": variants,
            "population": population_values(variants),
            "rows": averages(variants, TABLE2_N, True, 100),
        }
    return out


# ---------------------------------------------------------------------------------------------
# ToBRFV
# ---------------------------------------------------------------------------------------------


def tobrfv() -> list[dict]:
    out = []
    for label, rel in TOBRFV:
        variants = read_unique(rel)
        k = len(variants[0][0])
        values = population_values(variants)
        ints = [int.from_bytes(s, "big") for s, _ in variants]
        dist = [k - R.matches(ints[i], ints[j], k) for i, j in combinations(range(len(ints)), 2)]
        values.update(
            sample=label,
            reads=sum(c for _, c in variants),
            unique=len(variants),
            mean_distance=Fraction(sum(dist), len(dist)),
            max_distance=max(dist),
        )
        out.append(values)
    return out


# ---------------------------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------------------------


def f4(x: float) -> str:
    return f"{x:.4f}"


def pct(x: float, digits: int = 1) -> str:
    return f"{100 * x:.{digits}f}%"


def listing(items: list[str]) -> str:
    """'a, b and c'."""
    return items[0] if len(items) == 1 else ", ".join(items[:-1]) + " and " + items[-1]


def thousands(n: int) -> str:
    return f"{n:,}"


def share(p: float) -> str:
    """A probability as a percentage, '< 0.01%' for a positive one that would round to 0."""
    if p == 0:
        return "0%"
    if p < 0.00005:
        return "< 0.01%"
    return pct(p, 2)


def render_subsets(s: dict) -> str:
    t = s["table1"]
    counts = listing([str(c) for _, c in t["variants"]])
    lines = [
        f"**Table 1.** MERS-CoV spike position 25 ({sum(c for _, c in t['variants'])} sequences; its "
        f"{len(t['variants'])} distinct 9-mers are carried by {counts} of them): the average over all subsets "
        "of $n$ of its sequences, computed exactly. The normalised values are averaged over the subsets with "
        "at least two distinct 9-mers.",
        "",
        "| $n$ | $\\hat H$ | $H_\\text{sim}$ | $H_n$ | $H_\\text{nsim}$ | Distinct 9-mers | Subsets with one 9-mer |",
        "|---|---|---|---|---|---|---|",
    ]
    for r in t["rows"]:
        lines.append(f"| {r['n']} | {f4(r['H'])} | {f4(r['H_sim'])} | {f4(r['H_n'])} | {f4(r['H_nsim'])} | {r['D']:.2f} | {share(r['P1'])} |")
    return "\n".join(lines)


def render_populations(s: dict) -> str:
    t = s["table2"]
    fig = t["fig1b"]
    subs = [subs for _, subs in FIG1B[1:]]
    lines = [
        "**Table 2.** The values of two populations, and their average over samples of $n$ sequences drawn at "
        "random with replacement, computed exactly. (a) Quasispecies 2 of Wu's Fig. 1b [@wu2025, Fig. 1b]: a "
        f"master sequence at {FIG1B[0][0]}% and four variants at {listing([f'{f}%' for f, _ in FIG1B[1:]])} that "
        f"differ from it at {listing([str(x) for x in subs])} sites, no two variants at the same site, as drawn "
        f"(so $k = {FIG1B_K}$, the shortest length that holds them). "
        "(b) Two 9-mers at 50% each that differ at one site. The normalised values are averaged over the "
        "samples with at least two distinct k-mers.",
        "",
        "| Population | $n$ | $\\hat H$ | $H_\\text{sim}$ | $H_n$ | $H_\\text{nsim}$ | Samples with one k-mer |",
        "|---|---|---|---|---|---|---|",
    ]
    for label, key in (("(a)", "fig1b"), ("(b)", "half")):
        pop = t[key]["population"]
        for r in t[key]["rows"]:
            lines.append(f"| {label} | {r['n']} | {f4(r['H'])} | {f4(r['H_sim'])} | {f4(r['H_n'])} | {f4(r['H_nsim'])} | {share(r['P1'])} |")
        lines.append(f"| {label} | population | {f4(pop['H'])} | {f4(pop['H_sim'])} | {f4(pop['H_n'])} | {f4(pop['H_nsim'])} | — |")
    del fig
    return "\n".join(lines)


def render_tobrfv(s: dict) -> str:
    rows = s["tobrfv"]
    head = "| Quantity | " + " | ".join(r["sample"] for r in rows) + " |"
    lines = [head, "|---" * (len(rows) + 1) + "|"]
    lines.append("| Reads | " + " | ".join(thousands(r["reads"]) for r in rows) + " |")
    lines.append("| Unique sequences ($D$) | " + " | ".join(str(r["unique"]) for r in rows) + " |")
    for label, key in (("$H$", "H"), ("$H_n$", "H_n"), ("$H_\\text{sim}$", "H_sim"), ("$H_\\text{nsim}$", "H_nsim")):
        lines.append(f"| {label} | " + " | ".join(f4(r[key]) for r in rows) + " |")
    lines.append("| Mean Hamming distance between unique sequences | " + " | ".join(f"{float(r['mean_distance']):.2f}" for r in rows) + " |")
    lines.append("| Largest Hamming distance between unique sequences | " + " | ".join(str(r["max_distance"]) for r in rows) + " |")
    return "\n".join(lines)


#: (file, region name, renderer)
REGIONS = [
    (ENTROPY, "swe-subsets", render_subsets),
    (ENTROPY, "swe-populations", render_populations),
    (COMPARISON, "tobrfv", render_tobrfv),
]

WIDTH = 100
_MARKER = re.compile(r"<!-- docs-numbers:([a-z0-9-]+):(start|end) -->")
#: A line that would start a block in Markdown instead of continuing the paragraph.
_BLOCK_START = re.compile(r"(?:[-+*](?:\s|$)|\d+[.)](?:\s|$)|#|>|\||<|=+\s*$|`{3}|~{3}|\$\$)")


def wrap(text: str, indent: str, width: int = WIDTH, first: str | None = None) -> list[str]:
    """Greedy wrap that never breaks inside $...$ and never starts a line with a block marker."""
    protected = re.sub(r"\$[^$]+\$", lambda m: m.group(0).replace(" ", "\x00"), text)
    words = protected.split(" ")
    lines: list[str] = []
    cur = ""
    lead = indent if first is None else first
    for word in words:
        cand = word if not cur else cur + " " + word
        if cur and len((lead if not lines else indent) + cand) > width and not _BLOCK_START.match(word):
            lines.append(cur)
            cur = word
        else:
            cur = cand
    lines.append(cur)
    out = []
    for i, line in enumerate(lines):
        out.append((lead if i == 0 else indent) + line.replace("\x00", " "))
    return out


def apply_regions(text: str, path: Path, rendered: dict[str, str]) -> str:
    for match in _MARKER.finditer(text):
        if match.group(1) not in rendered:
            raise RuntimeError(f"{path.relative_to(ROOT)}: unknown region {match.group(1)!r}")
    for name, content in rendered.items():
        start = f"<!-- docs-numbers:{name}:start -->"
        end = f"<!-- docs-numbers:{name}:end -->"
        if text.count(start) != 1 or text.count(end) != 1:
            raise RuntimeError(f"{path.relative_to(ROOT)}: region {name!r} needs exactly one start and one end marker")
        i = text.index(start) + len(start)
        j = text.index(end)
        if j < i:
            raise RuntimeError(f"{path.relative_to(ROOT)}: region {name!r} ends before it starts")
        line_start = text.rfind("\n", 0, i) + 1
        before = text[line_start:i][: -len(start)]
        if before.strip():
            raise RuntimeError(f"{path.relative_to(ROOT)}: region {name!r} must start on its own line")
        lines = []
        for line in content.split("\n"):
            if not line:
                lines.append("")
            elif line.startswith("|"):  # table rows stay whole
                lines.append(before + line)
            else:
                lines.extend(wrap(line, before))
        text = text[:i] + "\n\n" + "\n".join(lines) + "\n\n" + before + text[j:]
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="fail if a region is out of date (prints the difference)")
    args = parser.parse_args()
    check_averages()
    summary = {"table1": table1(), "table2": table2(), "tobrfv": tobrfv()}
    by_file: dict[Path, dict[str, str]] = {}
    for path, name, render in REGIONS:
        by_file.setdefault(path, {})[name] = render(summary)
    stale = []
    for path, rendered in by_file.items():
        current = path.read_text(encoding="utf-8")
        expected = apply_regions(current, path, rendered)
        if current == expected:
            continue
        rel = str(path.relative_to(ROOT))
        if args.check:
            sys.stdout.writelines(difflib.unified_diff(current.splitlines(True), expected.splitlines(True), rel, rel + " (expected)"))
            stale.append(rel)
        else:
            tmp = path.with_name(path.name + ".tmp")
            tmp.write_text(expected, encoding="utf-8", newline="\n")
            os.chmod(tmp, os.stat(path).st_mode)
            os.replace(tmp, path)
            print(f"wrote {rel}", file=sys.stderr)
    if stale:
        print(f"out of date: {', '.join(stale)}: run python3 scripts/reference/docs_numbers.py", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
